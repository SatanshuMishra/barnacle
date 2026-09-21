use std::collections::BTreeMap;
use std::collections::HashMap;
use std::collections::HashSet;
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
use crate::attendance_store::Post;
use crate::attendance_store::PostState;
use crate::attendance_store::Season;
use crate::ids::ChannelId;
use crate::ids::GuildId;
use crate::ids::RoleId;
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
pub enum Delivery {
    New,
    Redraw,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClearScope {
    All,
    OutsideRange,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PostsReport {
    pub touched: usize,
    pub failures: usize,
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
    pub ping: Option<RoleId>,
    pub hours: [HourTally; 4],
    pub rows: Vec<RosterRow>,
    pub hidden: usize,
}

pub trait Board: Send + Sync + 'static {
    fn send_post(
        &self,
        channel: ChannelId,
        view: &SignupView,
        delivery: Delivery,
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

impl TickReport {
    fn joined(self, other: Self) -> Self {
        Self {
            posted: [self.posted, other.posted].concat(),
            adopted: [self.adopted, other.adopted].concat(),
            closed: [self.closed, other.closed].concat(),
            removed: [self.removed, other.removed].concat(),
            failures: self.failures + other.failures,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PurgeReport {
    pub seasons: usize,
    pub messages: usize,
    pub answers: usize,
    pub failures: usize,
}

pub const ROSTER_LIMIT: usize = 60;

const MILLIS_PER_SECOND: i64 = 1_000;

#[derive(Default)]
struct Slot {
    edit: Mutex<()>,
    wanted: AtomicU64,
    drawn: AtomicU64,
}

pub struct Signups<B: Board> {
    board: B,
    store: Attendance,
    rehearsal: Vec<GuildId>,
    slots: std::sync::Mutex<HashMap<Snowflake, Arc<Slot>>>,
    beats: Mutex<()>,
}

impl<B: Board> Signups<B> {
    pub fn new(board: B, store: Attendance) -> Arc<Self> {
        Self::rehearsing(board, store, Vec::new())
    }

    pub fn rehearsing(board: B, store: Attendance, rehearsal: Vec<GuildId>) -> Arc<Self> {
        Arc::new(Self {
            board,
            store,
            rehearsal,
            slots: std::sync::Mutex::new(HashMap::new()),
            beats: Mutex::new(()),
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
        let mut offsets: HashMap<GuildId, Option<i64>> = HashMap::new();
        for guild in seasons.iter().map(|season| season.guild) {
            if offsets.contains_key(&guild) {
                continue;
            }
            let offset = if self.rehearsal.contains(&guild) {
                self.store.rehearsal_clock(guild).await.ok()
            } else {
                Some(0)
            };
            offsets.insert(guild, offset);
        }
        let mut report = TickReport::default();
        let mut groups: BTreeMap<i64, Vec<Season>> = BTreeMap::new();
        for season in seasons {
            match offsets.get(&season.guild).copied().flatten() {
                Some(offset) => groups.entry(offset).or_default().push(season),
                None => report.failures += 1,
            }
        }
        for (offset, seasons) in groups {
            let Some((instant, instant_ms)) = shifted(now_unix, now_ms, offset) else {
                report.failures += 1;
                continue;
            };
            let beaten = self.beat(seasons, instant, instant_ms).await;
            report = report.joined(beaten);
        }
        report
    }

    pub async fn tick_in(
        self: &Arc<Self>,
        guild: GuildId,
        now_unix: i64,
        now_ms: u64,
    ) -> TickReport {
        let Ok(seasons) = self.store.live_seasons(now_unix).await else {
            return TickReport {
                failures: 1,
                ..TickReport::default()
            };
        };
        let seasons = seasons
            .into_iter()
            .filter(|season| season.guild == guild)
            .collect();
        self.beat(seasons, now_unix, now_ms).await
    }

    pub async fn purge(self: &Arc<Self>, guild: GuildId, now_unix: i64) -> PurgeReport {
        let Ok(seasons) = self.store.seasons_in_any_state(guild).await else {
            return PurgeReport {
                failures: 1,
                ..PurgeReport::default()
            };
        };
        let mut cleared = PostsReport::default();
        for season in &seasons {
            let posts = self.clear_posts(season, ClearScope::All, now_unix).await;
            cleared.touched += posts.touched;
            cleared.failures += posts.failures;
        }
        if cleared.failures != 0 {
            return PurgeReport {
                seasons: 0,
                messages: cleared.touched,
                answers: 0,
                failures: cleared.failures,
            };
        }
        let mut report = PurgeReport {
            messages: cleared.touched,
            ..PurgeReport::default()
        };
        for season in &seasons {
            match self
                .store
                .purge_season(guild, season.id, season.number)
                .await
            {
                Ok(None) => {}
                Ok(Some(answers)) => {
                    report.seasons += 1;
                    report.answers += usize::try_from(answers).unwrap_or(usize::MAX);
                }
                Err(_) => report.failures += 1,
            }
        }
        if report.failures == 0 && self.store.clear_rehearsal_clock(guild).await.is_err() {
            report.failures += 1;
        }
        report
    }

    pub async fn rehearsal_instant(
        &self,
        guild: GuildId,
        now_unix: i64,
        now_ms: u64,
    ) -> (i64, u64) {
        if !self.rehearsal.contains(&guild) {
            return (now_unix, now_ms);
        }
        let Ok(offset) = self.store.rehearsal_clock(guild).await else {
            return (now_unix, now_ms);
        };
        shifted(now_unix, now_ms, offset).unwrap_or((now_unix, now_ms))
    }

    async fn beat(&self, seasons: Vec<Season>, now_unix: i64, now_ms: u64) -> TickReport {
        let _beating = self.beats.lock().await;
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
            let delivery = match self.store.post(season.id, night).await {
                Ok(Some(post)) if post.state != PostState::Removed => continue,
                Ok(Some(_)) => Delivery::Redraw,
                Ok(None) => Delivery::New,
                Err(_) => {
                    report.failures += 1;
                    continue;
                }
            };
            let season = &match self.store.season(season.id).await {
                Ok(Some(fresh)) if fresh.ended_at_ms.is_none() => fresh,
                Ok(_) => continue,
                Err(_) => {
                    report.failures += 1;
                    continue;
                }
            };
            match self.board.find_post(season.channel, tag).await {
                Ok(Some(message)) => {
                    if self.record(season.id, night, message, now_ms).await {
                        report.adopted.push(tag);
                    } else {
                        report.failures += 1;
                    }
                }
                Ok(None) => {
                    let Ok(view) = self.view(season, night, now_unix).await else {
                        report.failures += 1;
                        continue;
                    };
                    match self.board.send_post(season.channel, &view, delivery).await {
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

    pub async fn clear_posts(
        self: &Arc<Self>,
        season: &Season,
        scope: ClearScope,
        _now_unix: i64,
    ) -> PostsReport {
        let Ok(posts) = self.store.posts(season.id).await else {
            return PostsReport {
                touched: 0,
                failures: 1,
            };
        };
        let mut report = PostsReport::default();
        for post in posts
            .iter()
            .filter(|post| post.state != PostState::Removed)
            .filter(|post| match scope {
                ClearScope::All => true,
                ClearScope::OutsideRange => !season.range.holds(post.night),
            })
        {
            match self.board.delete_post(season.channel, post.message).await {
                Ok(Removal::Deleted | Removal::Gone) => {
                    if self
                        .store
                        .set_post_state(season.id, post.night, PostState::Removed)
                        .await
                        .is_ok()
                    {
                        self.forget(post.message);
                        report.touched += 1;
                    } else {
                        report.failures += 1;
                    }
                }
                Err(_) => report.failures += 1,
            }
        }
        report
    }

    pub async fn refresh_posts(self: &Arc<Self>, season: &Season, now_unix: i64) -> PostsReport {
        let Ok(posts) = self.store.posts(season.id).await else {
            return PostsReport {
                touched: 0,
                failures: 1,
            };
        };
        let mut report = PostsReport::default();
        for post in posts.iter().filter(|post| post.state != PostState::Removed) {
            if self
                .redraw(season, post.night, post.message, now_unix)
                .await
            {
                report.touched += 1;
            } else {
                report.failures += 1;
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
        if season.guild != click.guild
            || season.ended_at_ms.is_some()
            || !season.range.holds(click.night)
        {
            return ClickOutcome::UnknownSeason;
        }
        let (now_unix, now_ms) = self.rehearsal_instant(season.guild, now_unix, now_ms).await;
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
        let Ok(Some(season)) = self.store.season(season.id).await else {
            return false;
        };
        let Ok(view) = self.view(&season, night, now_unix).await else {
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
        slots.remove(&message);
    }
}

pub fn next_moment(seasons: &[Season], posts: &[(i64, Vec<Post>)], now_unix: i64) -> Option<i64> {
    seasons
        .iter()
        .flat_map(|season| {
            let posted = posts
                .iter()
                .find(|(id, _)| *id == season.id)
                .map_or(&[][..], |(_, posts)| posts.as_slice());
            moments(season, posted, now_unix)
        })
        .filter(|moment| *moment > now_unix)
        .min()
}

fn moments<'a>(
    season: &'a Season,
    posts: &'a [Post],
    now_unix: i64,
) -> impl Iterator<Item = i64> + 'a {
    let removals = posts
        .iter()
        .filter(|post| post.state != PostState::Removed)
        .map(|post| post.night.remove_at_unix());
    let closes = posts
        .iter()
        .filter(|post| post.state == PostState::Open)
        .map(|post| post.night.start_unix());
    let postings = season
        .range
        .nights()
        .filter(move |night| night.start_unix() > now_unix)
        .filter(|night| {
            !posts
                .iter()
                .any(|post| post.night == *night && post.state != PostState::Removed)
        })
        .map(Night::post_at_unix);
    removals.chain(closes).chain(postings)
}

fn shifted(now_unix: i64, now_ms: u64, offset: i64) -> Option<(i64, u64)> {
    let instant = now_unix.checked_add(offset)?;
    let instant_ms = now_ms.checked_add_signed(offset.checked_mul(MILLIS_PER_SECOND)?)?;
    Some((instant, instant_ms))
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
        ping: season.ping_role,
        hours: std::array::from_fn(|index| tally(Hour::ALL[index], marks)),
        rows: rows.into_iter().take(ROSTER_LIMIT).collect(),
        hidden,
    }
}

fn tally(hour: Hour, marks: &[Mark]) -> HourTally {
    let players = marks
        .iter()
        .map(|mark| mark.user)
        .collect::<HashSet<_>>()
        .len();
    let attending = marks
        .iter()
        .filter(|mark| mark.hour == hour && mark.attending)
        .count();
    HourTally {
        hour,
        attending: u32::try_from(attending).unwrap_or(u32::MAX),
        nope: u32::try_from(players.saturating_sub(attending)).unwrap_or(u32::MAX),
    }
}

fn roster_rows(marks: &[Mark]) -> Vec<RosterRow> {
    let mut rows: Vec<RosterRow> = Vec::new();
    let mut seen: HashMap<UserId, usize> = HashMap::new();
    for mark in marks {
        let index = *seen.entry(mark.user).or_insert_with(|| {
            rows.push(RosterRow {
                user: mark.user,
                cells: [Cell::Out; 4],
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
