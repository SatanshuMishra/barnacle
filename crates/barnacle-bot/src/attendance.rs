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
use crate::failure;
use crate::failure::Failure;
use crate::failure::Scope;
use crate::ids::ChannelId;
use crate::ids::GuildId;
use crate::ids::Ping;
use crate::ids::RoleId;
use crate::schedule::Hour;
use crate::schedule::Night;
use crate::schedule::Range;

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
    pub pings: Vec<Ping>,
    pub hours: [HourTally; 4],
    pub rows: Vec<RosterRow>,
    pub hidden: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sent {
    pub message: Snowflake,
    pub unheard: Vec<Ping>,
}

pub trait Board: Send + Sync + 'static {
    fn send_post(
        &self,
        channel: ChannelId,
        view: &SignupView,
        delivery: Delivery,
    ) -> impl Future<Output = Result<Sent, BoardError>> + Send;

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClickOutcome {
    Recorded,
    Closed { start_unix: i64 },
    UnknownSeason,
    Failed(Failure),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RepostPing {
    Quiet,
    Again { checked: Vec<RoleId> },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RepostOutcome {
    Reposted {
        night: Night,
        previous: Snowflake,
        message: Snowflake,
        adopted: bool,
        pinged: Vec<Ping>,
        unheard: Vec<Ping>,
    },
    NotFound,
    NothingOpen {
        next_post_at: Option<i64>,
        nights_left: usize,
    },
    PingsChanged,
    Superseded,
    Failed(Failure),
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
        let seasons = match self.store.live_seasons(now_unix).await {
            Ok(seasons) => seasons,
            Err(error) => {
                post_failed(
                    "the live CB seasons could not be read",
                    &error,
                    &Scope::default(),
                );
                return TickReport {
                    failures: 1,
                    ..TickReport::default()
                };
            }
        };
        let mut offsets: HashMap<GuildId, Option<i64>> = HashMap::new();
        for guild in seasons.iter().map(|season| season.guild) {
            if offsets.contains_key(&guild) {
                continue;
            }
            let offset = if self.rehearsal.contains(&guild) {
                self.store
                    .rehearsal_clock(guild)
                    .await
                    .inspect_err(|error| {
                        post_failed(
                            "a rehearsal server's clock could not be read, so its sign-up posts were skipped",
                            error,
                            &Scope::default().guild(guild),
                        );
                    })
                    .ok()
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
                failure::report(
                    "signup.post.failed",
                    "a rehearsal clock moves the time out of range, so its sign-up posts were skipped",
                    &Failure::internal(format!(
                        "a rehearsal clock offset of {offset} seconds moves the time out of range"
                    )),
                    &Scope::default(),
                );
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
        let seasons = match self.store.live_seasons(now_unix).await {
            Ok(seasons) => seasons,
            Err(error) => {
                post_failed(
                    "the live CB seasons could not be read",
                    &error,
                    &Scope::default().guild(guild),
                );
                return TickReport {
                    failures: 1,
                    ..TickReport::default()
                };
            }
        };
        let seasons = seasons
            .into_iter()
            .filter(|season| season.guild == guild)
            .collect();
        self.beat(seasons, now_unix, now_ms).await
    }

    pub async fn purge(self: &Arc<Self>, guild: GuildId, now_unix: i64) -> PurgeReport {
        let seasons = match self.store.seasons_in_any_state(guild).await {
            Ok(seasons) => seasons,
            Err(error) => {
                post_failed(
                    "the server's CB seasons could not be read for a purge",
                    &error,
                    &Scope::default().guild(guild),
                );
                return PurgeReport {
                    failures: 1,
                    ..PurgeReport::default()
                };
            }
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
                Err(error) => {
                    post_failed(
                        "a CB season could not be purged",
                        &error,
                        &season_scope(season),
                    );
                    report.failures += 1;
                }
            }
        }
        if report.failures == 0
            && let Err(error) = self.store.clear_rehearsal_clock(guild).await
        {
            post_failed(
                "a purged server's rehearsal clock could not be cleared",
                &error,
                &Scope::default().guild(guild),
            );
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
        let offset = match self.store.rehearsal_clock(guild).await {
            Ok(offset) => offset,
            Err(error) => {
                post_failed(
                    "a rehearsal server's clock could not be read, so the real time was used",
                    &error,
                    &Scope::default().guild(guild),
                );
                return (now_unix, now_ms);
            }
        };
        shifted(now_unix, now_ms, offset).unwrap_or((now_unix, now_ms))
    }

    async fn beat(&self, seasons: Vec<Season>, now_unix: i64, now_ms: u64) -> TickReport {
        let _beating = self.beats.lock().await;
        let mut report = TickReport::default();
        for season in &seasons {
            let posts = match self.store.posts(season.id).await {
                Ok(posts) => posts,
                Err(error) => {
                    post_failed(
                        "a season's sign-up posts could not be read",
                        &error,
                        &season_scope(season),
                    );
                    report.failures += 1;
                    continue;
                }
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
                if self.remove(season, post).await {
                    swept.push(post.night);
                    report.removed.push(tag);
                } else {
                    report.failures += 1;
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
                let scope = post_scope(season, post.night).message(post.message);
                if !self
                    .redraw(season, post.night, post.message, now_unix)
                    .await
                {
                    report.failures += 1;
                    continue;
                }
                match self
                    .store
                    .set_post_state(season.id, post.night, PostState::Closed)
                    .await
                {
                    Ok(()) => {
                        failure::record("signup.post.closed", "a sign-up post closed", &scope);
                        report.closed.push(tag);
                    }
                    Err(error) => {
                        post_failed("a closed sign-up post could not be saved", &error, &scope);
                        report.failures += 1;
                    }
                }
            }
            let Some(night) = season.range.due_night(now_unix) else {
                continue;
            };
            let tag = PostTag {
                season: season.id,
                night,
            };
            let scope = post_scope(season, night);
            let delivery = match self.store.post(season.id, night).await {
                Ok(Some(post)) if post.state != PostState::Removed => continue,
                Ok(Some(_)) => Delivery::Redraw,
                Ok(None) => Delivery::New,
                Err(error) => {
                    post_failed("a due sign-up post could not be looked up", &error, &scope);
                    report.failures += 1;
                    continue;
                }
            };
            let season = &match self.store.season(season.id).await {
                Ok(Some(fresh)) if fresh.ended_at_ms.is_none() => fresh,
                Ok(_) => continue,
                Err(error) => {
                    post_failed(
                        "a season could not be read before publishing its sign-up post",
                        &error,
                        &scope,
                    );
                    report.failures += 1;
                    continue;
                }
            };
            let scope = post_scope(season, night);
            match self.board.find_post(season.channel, tag).await {
                Ok(Some(message)) => {
                    let scope = scope.message(message);
                    if self.record(season.id, night, message, now_ms, &scope).await {
                        failure::record(
                            "signup.post.published",
                            "a sign-up post already in the channel was adopted",
                            &scope,
                        );
                        report.adopted.push(tag);
                    } else {
                        report.failures += 1;
                    }
                }
                Ok(None) => {
                    let view = match self.view(season, night, now_unix).await {
                        Ok(view) => view,
                        Err(error) => {
                            post_failed(
                                "the roster for a new sign-up post could not be read",
                                &error,
                                &scope,
                            );
                            report.failures += 1;
                            continue;
                        }
                    };
                    match self.board.send_post(season.channel, &view, delivery).await {
                        Ok(sent) => {
                            let scope = scope.message(sent.message);
                            let pings = match delivery {
                                Delivery::New => view.pings.clone(),
                                Delivery::Redraw => Vec::new(),
                            };
                            if !pings.is_empty() && !sent.unheard.is_empty() {
                                failure::ping_silent(&pings, &sent.unheard, &scope);
                            }
                            if self
                                .record(season.id, night, sent.message, now_ms, &scope)
                                .await
                            {
                                if pings.is_empty() {
                                    failure::record(
                                        "signup.post.published",
                                        "a sign-up post was published",
                                        &scope,
                                    );
                                } else {
                                    failure::ping_published(
                                        "a sign-up post was published",
                                        &pings,
                                        sent.unheard.is_empty(),
                                        &scope,
                                    );
                                }
                                report.posted.push(tag);
                            } else {
                                report.failures += 1;
                            }
                        }
                        Err(error) => {
                            post_failed("a sign-up post could not be published", &error, &scope);
                            report.failures += 1;
                        }
                    }
                }
                Err(error) => {
                    post_failed(
                        "the channel could not be searched for an existing sign-up post",
                        &error,
                        &scope,
                    );
                    report.failures += 1;
                }
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
        let _beating = self.beats.lock().await;
        let posts = match self.store.posts(season.id).await {
            Ok(posts) => posts,
            Err(error) => {
                post_failed(
                    "a season's sign-up posts could not be read",
                    &error,
                    &season_scope(season),
                );
                return PostsReport {
                    touched: 0,
                    failures: 1,
                };
            }
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
            if self.remove(season, post).await {
                report.touched += 1;
            } else {
                report.failures += 1;
            }
        }
        report
    }

    pub async fn refresh_posts(self: &Arc<Self>, season: &Season, now_unix: i64) -> PostsReport {
        let posts = match self.store.posts(season.id).await {
            Ok(posts) => posts,
            Err(error) => {
                post_failed(
                    "a season's sign-up posts could not be read",
                    &error,
                    &season_scope(season),
                );
                return PostsReport {
                    touched: 0,
                    failures: 1,
                };
            }
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
            Err(error) => return ClickOutcome::Failed(Failure::from_error(&error)),
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
        if let Err(error) = self
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
        {
            return ClickOutcome::Failed(Failure::from_error(&error));
        }
        let current = match self.store.post(season.id, click.night).await {
            Ok(Some(post)) if post.state != PostState::Removed => post.message,
            Ok(_) | Err(_) => click.message,
        };
        self.redraw(&season, click.night, current, now_unix).await;
        failure::click_recorded(
            &target_name(click.target),
            choice_name(click.attending),
            &Scope::default()
                .guild(season.guild)
                .channel(click.channel)
                .season(season.id)
                .night(click.night.label())
                .message(click.message)
                .user(click.user),
        );
        ClickOutcome::Recorded
    }

    pub async fn repost(
        self: &Arc<Self>,
        guild: GuildId,
        number: u32,
        ping: RepostPing,
        now_unix: i64,
        now_ms: u64,
    ) -> RepostOutcome {
        let _beating = self.beats.lock().await;
        let scope = Scope::default().guild(guild);
        let season = match self.store.live_season(guild, number).await {
            Ok(Some(season)) => season,
            Ok(None) => return RepostOutcome::NotFound,
            Err(error) => {
                return repost_failed(
                    "a season could not be read to repost its sign-up post",
                    &error,
                    &scope,
                );
            }
        };
        if let RepostPing::Again { checked } = &ping
            && *checked != season.ping_roles
        {
            return RepostOutcome::PingsChanged;
        }
        let (instant, _) = self.rehearsal_instant(guild, now_unix, now_ms).await;
        let posts = match self.store.posts(season.id).await {
            Ok(posts) => posts,
            Err(error) => {
                return repost_failed(
                    "a season's sign-up posts could not be read to repost one",
                    &error,
                    &season_scope(&season),
                );
            }
        };
        let Some(post) = posts
            .iter()
            .find(|post| post.state == PostState::Open && instant < post.night.start_unix())
        else {
            return RepostOutcome::NothingOpen {
                next_post_at: next_post_at(season.range, instant),
                nights_left: season.range.nights_left(instant),
            };
        };
        let night = post.night;
        let previous = post.message;
        let scope = post_scope(&season, night).message(previous);
        let view = match self.view(&season, night, instant).await {
            Ok(view) => view,
            Err(error) => {
                return repost_failed(
                    "the roster for a reposted sign-up post could not be read",
                    &error,
                    &scope,
                );
            }
        };
        if let Err(error) = self.board.delete_post(season.channel, previous).await {
            return repost_failed(
                "a sign-up post could not be deleted to repost it",
                &error,
                &scope,
            );
        }
        self.forget(previous);
        let tag = PostTag {
            season: season.id,
            night,
        };
        let scope = post_scope(&season, night);
        let (message, adopted, pinged, unheard) = match self
            .board
            .find_post(season.channel, tag)
            .await
        {
            Ok(Some(message)) => (message, true, Vec::new(), Vec::new()),
            Ok(None) => {
                let delivery = match ping {
                    RepostPing::Again { .. } => Delivery::New,
                    RepostPing::Quiet => Delivery::Redraw,
                };
                match self.board.send_post(season.channel, &view, delivery).await {
                    Ok(sent) => {
                        let pinged = match delivery {
                            Delivery::New => view.pings.clone(),
                            Delivery::Redraw => Vec::new(),
                        };
                        let unheard = if pinged.is_empty() {
                            Vec::new()
                        } else {
                            sent.unheard
                        };
                        (sent.message, false, pinged, unheard)
                    }
                    Err(error) => {
                        return repost_failed(
                            "a reposted sign-up post could not be sent",
                            &error,
                            &scope,
                        );
                    }
                }
            }
            Err(error) => {
                return repost_failed(
                    "the channel could not be searched for a sign-up post left by an earlier repost",
                    &error,
                    &scope,
                );
            }
        };
        let scope = scope.message(message);
        match self
            .store
            .replace_post_message(season.id, night, previous, message)
            .await
        {
            Ok(true) => {}
            Ok(false) => {
                self.discard(&season, message, &scope).await;
                return RepostOutcome::Superseded;
            }
            Err(error) => {
                let outcome =
                    repost_failed("a reposted sign-up post could not be saved", &error, &scope);
                self.discard(&season, message, &scope).await;
                return outcome;
            }
        }
        if !unheard.is_empty() {
            failure::ping_silent(&pinged, &unheard, &scope);
        }
        failure::post_reposted(
            previous,
            matches!(ping, RepostPing::Again { .. }),
            adopted,
            &pinged,
            unheard.is_empty(),
            &scope,
        );
        self.redraw(&season, night, message, instant).await;
        RepostOutcome::Reposted {
            night,
            previous,
            message,
            adopted,
            pinged,
            unheard,
        }
    }

    async fn discard(&self, season: &Season, message: Snowflake, scope: &Scope) {
        if let Err(error) = self.board.delete_post(season.channel, message).await {
            post_failed(
                "a reposted sign-up post that could not be kept could not be deleted",
                &error,
                scope,
            );
        }
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

    async fn remove(&self, season: &Season, post: &Post) -> bool {
        let scope = post_scope(season, post.night).message(post.message);
        match self.board.delete_post(season.channel, post.message).await {
            Ok(Removal::Deleted | Removal::Gone) => {
                match self
                    .store
                    .set_post_state(season.id, post.night, PostState::Removed)
                    .await
                {
                    Ok(()) => {
                        self.forget(post.message);
                        failure::record(
                            "signup.post.removed",
                            "a sign-up post was removed",
                            &scope,
                        );
                        true
                    }
                    Err(error) => {
                        post_failed("a removed sign-up post could not be saved", &error, &scope);
                        false
                    }
                }
            }
            Err(error) => {
                post_failed("a sign-up post could not be removed", &error, &scope);
                false
            }
        }
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
        let scope = post_scope(season, night).message(message);
        let season = match self.store.season(season.id).await {
            Ok(Some(season)) => season,
            Ok(None) => {
                failure::report(
                    "signup.post.failed",
                    "a sign-up post could not be redrawn because its season is gone",
                    &Failure::internal(format!("season {} no longer exists", season.id)),
                    &scope,
                );
                return false;
            }
            Err(error) => {
                post_failed(
                    "a season could not be read to redraw its sign-up post",
                    &error,
                    &scope,
                );
                return false;
            }
        };
        let view = match self.view(&season, night, now_unix).await {
            Ok(view) => view,
            Err(error) => {
                post_failed(
                    "the roster for a sign-up post could not be read",
                    &error,
                    &scope,
                );
                return false;
            }
        };
        if let Err(error) = self.board.edit_post(season.channel, message, &view).await {
            post_failed("a sign-up post could not be redrawn", &error, &scope);
            return false;
        }
        slot.drawn.store(target, SeqCst);
        true
    }

    async fn record(
        &self,
        season: i64,
        night: Night,
        message: Snowflake,
        now_ms: u64,
        scope: &Scope,
    ) -> bool {
        self.store
            .record_post(season, night, message, now_ms)
            .await
            .inspect_err(|error| {
                post_failed("a published sign-up post could not be saved", error, scope);
            })
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

pub fn next_post_at(range: Range, now_unix: i64) -> Option<i64> {
    range
        .due_night(now_unix)
        .is_none()
        .then(|| range.next_night(now_unix).map(Night::post_at_unix))
        .flatten()
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

fn post_failed(summary: &str, error: &(dyn std::error::Error + 'static), scope: &Scope) {
    failure::report(
        "signup.post.failed",
        summary,
        &Failure::from_error(error),
        scope,
    );
}

fn repost_failed(
    summary: &str,
    error: &(dyn std::error::Error + 'static),
    scope: &Scope,
) -> RepostOutcome {
    let failure = Failure::from_error(error);
    failure::report("signup.post.failed", summary, &failure, scope);
    RepostOutcome::Failed(failure)
}

fn target_name(target: Target) -> String {
    match target {
        Target::All => "all".to_owned(),
        Target::One(hour) => hour.get().to_string(),
    }
}

fn choice_name(attending: bool) -> &'static str {
    if attending { "attend" } else { "nope" }
}

fn season_scope(season: &Season) -> Scope {
    Scope::default()
        .guild(season.guild)
        .channel(season.channel)
        .season(season.id)
}

fn post_scope(season: &Season, night: Night) -> Scope {
    season_scope(season).night(night.label())
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
        pings: season
            .ping_roles
            .iter()
            .map(|role| Ping::of(season.guild, *role))
            .collect(),
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
