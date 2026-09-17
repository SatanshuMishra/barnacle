use std::collections::HashMap;
use std::future::Future;
use std::sync::Arc;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering::SeqCst;

use barnacle_guess::Snowflake;
use barnacle_guess::UserId;
use tokio::sync::Mutex;

use crate::attendance_store::Attendance;
use crate::attendance_store::AttendanceError;
use crate::attendance_store::Mark;
use crate::attendance_store::PostState;
use crate::attendance_store::Season;
use crate::ids::ChannelId;
use crate::ids::GuildId;
use crate::schedule::Hour;
use crate::schedule::Night;

#[derive(Debug, thiserror::Error)]
#[error("the Discord call failed")]
pub struct BoardError(#[source] pub Box<dyn std::error::Error + Send + Sync>);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Removal {
    Deleted,
    Gone,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PostTag {
    pub season: i64,
    pub night: Night,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cell {
    In,
    Out,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HourTally {
    pub hour: Hour,
    pub attending: u32,
    pub nope: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RosterRow {
    pub user: UserId,
    pub cells: [Cell; 4],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignupView {
    pub season: i64,
    pub number: u32,
    pub codename: Option<String>,
    pub night: Night,
    pub open: bool,
    pub hours: [HourTally; 4],
    pub rows: Vec<RosterRow>,
    pub hidden: usize,
}

pub trait Board: Send + Sync + 'static {
    fn send_post(
        &self,
        channel: ChannelId,
        view: &SignupView,
    ) -> impl Future<Output = Result<Snowflake, BoardError>> + Send;

    fn find_post(
        &self,
        channel: ChannelId,
        tag: PostTag,
    ) -> impl Future<Output = Result<Option<Snowflake>, BoardError>> + Send;

    fn edit_post(
        &self,
        channel: ChannelId,
        message: Snowflake,
        view: &SignupView,
    ) -> impl Future<Output = Result<(), BoardError>> + Send;

    fn delete_post(
        &self,
        channel: ChannelId,
        message: Snowflake,
    ) -> impl Future<Output = Result<Removal, BoardError>> + Send;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    All,
    One(Hour),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Click {
    pub guild: GuildId,
    pub channel: ChannelId,
    pub message: Snowflake,
    pub user: UserId,
    pub season: i64,
    pub night: Night,
    pub target: Target,
    pub attending: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClickOutcome {
    Recorded,
    Closed { start_unix: i64 },
    UnknownSeason,
    Failed,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TickReport {
    pub posted: Vec<PostTag>,
    pub adopted: Vec<PostTag>,
    pub closed: Vec<PostTag>,
    pub removed: Vec<PostTag>,
    pub failures: usize,
}

pub const ROSTER_LIMIT: usize = 60;

#[derive(Default)]
struct Slot {
    edit: Mutex<()>,
    wanted: AtomicU64,
    drawn: AtomicU64,
}

pub struct Signups<B: Board> {
    board: B,
    store: Attendance,
    slots: std::sync::Mutex<HashMap<Snowflake, Arc<Slot>>>,
}

impl<B: Board> Signups<B> {
    pub fn new(board: B, store: Attendance) -> Arc<Self> {
        Arc::new(Self {
            board,
            store,
            slots: std::sync::Mutex::new(HashMap::new()),
        })
    }

    pub fn store(&self) -> &Attendance {
        &self.store
    }

    pub async fn tick(self: &Arc<Self>, now_unix: i64, now_ms: u64) -> TickReport {
        let Ok(seasons) = self.store.live_seasons(now_unix).await else {
            return TickReport {
                failures: 1,
                ..TickReport::default()
            };
        };
        let mut report = TickReport::default();
        for season in &seasons {
            let Ok(posts) = self.store.posts(season.id).await else {
                report.failures += 1;
                continue;
            };
            let mut swept = Vec::new();
            for post in posts
                .iter()
                .filter(|post| post.state != PostState::Removed)
                .filter(|post| post.night.remove_at_unix() <= now_unix)
            {
                let tag = PostTag {
                    season: season.id,
                    night: post.night,
                };
                match self.board.delete_post(season.channel, post.message).await {
                    Ok(Removal::Deleted | Removal::Gone) => {
                        if self
                            .store
                            .set_post_state(season.id, post.night, PostState::Removed)
                            .await
                            .is_ok()
                        {
                            self.forget(post.message);
                            swept.push(post.night);
                            report.removed.push(tag);
                        } else {
                            report.failures += 1;
                        }
                    }
                    Err(_) => report.failures += 1,
                }
            }
            for post in posts
                .iter()
                .filter(|post| post.state == PostState::Open)
                .filter(|post| post.night.start_unix() <= now_unix)
                .filter(|post| !swept.contains(&post.night))
            {
                let tag = PostTag {
                    season: season.id,
                    night: post.night,
                };
                if !self
                    .redraw(season, post.night, post.message, now_unix)
                    .await
                {
                    report.failures += 1;
                } else if self
                    .store
                    .set_post_state(season.id, post.night, PostState::Closed)
                    .await
                    .is_ok()
                {
                    report.closed.push(tag);
                } else {
                    report.failures += 1;
                }
            }
            let Some(night) = season.range.due_night(now_unix) else {
                continue;
            };
            let tag = PostTag {
                season: season.id,
                night,
            };
            match self.store.post(season.id, night).await {
                Ok(Some(_)) => continue,
                Ok(None) => {}
                Err(_) => {
                    report.failures += 1;
                    continue;
                }
            }
            match self.board.find_post(season.channel, tag).await {
                Ok(Some(message)) => {
                    if self.record(season.id, night, message, now_ms).await {
                        report.adopted.push(tag);
                    } else {
                        report.failures += 1;
                    }
                }
                Ok(None) => {
                    let view = fresh_view(season, night, now_unix);
                    match self.board.send_post(season.channel, &view).await {
                        Ok(message) => {
                            if self.record(season.id, night, message, now_ms).await {
                                report.posted.push(tag);
                            } else {
                                report.failures += 1;
                            }
                        }
                        Err(_) => report.failures += 1,
                    }
                }
                Err(_) => report.failures += 1,
            }
        }
        report
    }

    pub async fn click(self: &Arc<Self>, click: Click, now_unix: i64, now_ms: u64) -> ClickOutcome {
        let season = match self.store.season(click.season).await {
            Ok(Some(season)) => season,
            Ok(None) => return ClickOutcome::UnknownSeason,
            Err(_) => return ClickOutcome::Failed,
        };
        if season.guild != click.guild || !season.range.holds(click.night) {
            return ClickOutcome::UnknownSeason;
        }
        if click.night.start_unix() <= now_unix {
            return ClickOutcome::Closed {
                start_unix: click.night.start_unix(),
            };
        }
        let hours: &[Hour] = match &click.target {
            Target::All => &Hour::ALL,
            Target::One(hour) => std::slice::from_ref(hour),
        };
        if self
            .store
            .mark(
                season.id,
                click.night,
                click.user,
                hours,
                click.attending,
                now_ms,
            )
            .await
            .is_err()
        {
            return ClickOutcome::Failed;
        }
        self.redraw(&season, click.night, click.message, now_unix)
            .await;
        ClickOutcome::Recorded
    }

    pub async fn view(
        &self,
        season: &Season,
        night: Night,
        now_unix: i64,
    ) -> Result<SignupView, AttendanceError> {
        let marks = self.store.roster(season.id, night).await?;
        Ok(build_view(season, night, now_unix, &marks))
    }

    async fn redraw(
        &self,
        season: &Season,
        night: Night,
        message: Snowflake,
        now_unix: i64,
    ) -> bool {
        let slot = self.slot(message);
        let mine = slot.wanted.fetch_add(1, SeqCst) + 1;
        let _drawing = slot.edit.lock().await;
        if slot.drawn.load(SeqCst) >= mine {
            return true;
        }
        let target = slot.wanted.load(SeqCst);
        let Ok(view) = self.view(season, night, now_unix).await else {
            return false;
        };
        if self
            .board
            .edit_post(season.channel, message, &view)
            .await
            .is_err()
        {
            return false;
        }
        slot.drawn.store(target, SeqCst);
        true
    }

    async fn record(&self, season: i64, night: Night, message: Snowflake, now_ms: u64) -> bool {
        self.store
            .record_post(season, night, message, now_ms)
            .await
            .is_ok()
    }

    fn slot(&self, message: Snowflake) -> Arc<Slot> {
        let mut slots = self
            .slots
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        Arc::clone(slots.entry(message).or_default())
    }

    fn forget(&self, message: Snowflake) {
        let mut slots = self
            .slots
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if slots
            .get(&message)
            .is_some_and(|slot| Arc::strong_count(slot) == 1)
        {
            slots.remove(&message);
        }
    }
}

fn fresh_view(season: &Season, night: Night, now_unix: i64) -> SignupView {
    build_view(season, night, now_unix, &[])
}

fn build_view(season: &Season, night: Night, now_unix: i64, marks: &[Mark]) -> SignupView {
    let rows = roster_rows(marks);
    let hidden = rows.len().saturating_sub(ROSTER_LIMIT);
    SignupView {
        season: season.id,
        number: season.number,
        codename: season.codename.clone(),
        night,
        open: now_unix < night.start_unix(),
        hours: std::array::from_fn(|index| tally(Hour::ALL[index], marks)),
        rows: rows.into_iter().take(ROSTER_LIMIT).collect(),
        hidden,
    }
}

fn tally(hour: Hour, marks: &[Mark]) -> HourTally {
    let count = |attending: bool| {
        let total = marks
            .iter()
            .filter(|mark| mark.hour == hour && mark.attending == attending)
            .count();
        u32::try_from(total).unwrap_or(u32::MAX)
    };
    HourTally {
        hour,
        attending: count(true),
        nope: count(false),
    }
}

fn roster_rows(marks: &[Mark]) -> Vec<RosterRow> {
    let mut rows: Vec<RosterRow> = Vec::new();
    let mut seen: HashMap<UserId, usize> = HashMap::new();
    for mark in marks {
        let index = *seen.entry(mark.user).or_insert_with(|| {
            rows.push(RosterRow {
                user: mark.user,
                cells: [Cell::None; 4],
            });
            rows.len() - 1
        });
        let cell = if mark.attending { Cell::In } else { Cell::Out };
        let hour = usize::from(mark.hour.get() - 1);
        if let Some(row) = rows.get_mut(index)
            && let Some(slot) = row.cells.get_mut(hour)
        {
            *slot = cell;
        }
    }
    rows
}
