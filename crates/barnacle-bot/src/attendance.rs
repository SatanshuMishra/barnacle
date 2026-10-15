use std::collections::BTreeMap;
use std::collections::HashMap;
use std::collections::HashSet;
use std::future::Future;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicI64;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering::SeqCst;

use barnacle_guess::Snowflake;
use barnacle_guess::UserId;
use jiff::civil::Date;
use tokio::sync::Mutex;

use crate::attendance_store::Attendance;
use crate::attendance_store::AttendanceError;
use crate::attendance_store::CreateOutcome;
use crate::attendance_store::EditOutcome;
use crate::attendance_store::Event;
use crate::attendance_store::EventChange;
use crate::attendance_store::Mark;
use crate::attendance_store::NewEvent;
use crate::attendance_store::Post;
use crate::attendance_store::PostState;
use crate::attendance_store::UnreadableEvent;
use crate::failure;
use crate::failure::Failure;
use crate::failure::Scope;
use crate::ids::ChannelId;
use crate::ids::GuildId;
use crate::ids::Ping;
use crate::ids::RoleId;
use crate::schedule::Hour;
use crate::schedule::Hours;
use crate::schedule::Session;
use crate::schedule::Span;
use crate::wiring;

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
    OutsideSchedule,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PostsReport {
    pub touched: usize,
    pub failures: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PostTag {
    pub event: i64,
    pub session: Session,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RosterRow {
    pub user: UserId,
    pub cells: Vec<Cell>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignupView {
    pub event: i64,
    pub name: String,
    pub session: Session,
    pub open: bool,
    pub pings: Vec<Ping>,
    pub hours: Vec<HourTally>,
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
    pub event: i64,
    pub date: Date,
    pub target: Target,
    pub attending: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClickOutcome {
    Recorded,
    Closed { start_unix: i64 },
    UnknownEvent,
    Failed(Failure),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MenuOutcome {
    Open {
        view: SignupView,
        attending: Vec<Hour>,
    },
    Closed {
        start_unix: i64,
    },
    UnknownEvent,
    Failed(Failure),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HoursPick {
    pub guild: GuildId,
    pub channel: ChannelId,
    pub user: UserId,
    pub event: i64,
    pub date: Date,
    pub values: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChooseOutcome {
    Saved { attending: Vec<Hour> },
    Closed { start_unix: i64 },
    UnknownEvent,
    BadChoice,
    Failed(Failure),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RepostPing {
    Quiet,
    Again {
        checked: Vec<RoleId>,
        channel: ChannelId,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RepostOutcome {
    Reposted {
        session: Session,
        previous: Snowflake,
        message: Snowflake,
        adopted: bool,
        pinged: Vec<Ping>,
        unheard: Vec<Ping>,
    },
    NotFound,
    NothingOpen {
        next_post_at: Option<i64>,
        sessions_left: usize,
    },
    NothingToPing,
    PingsChanged,
    Superseded,
    Failed(Failure),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditReport {
    Edited {
        before: Box<Event>,
        after: Box<Event>,
        cleared: PostsReport,
        refreshed: PostsReport,
    },
    ClearFailed {
        failures: usize,
    },
    NameTaken,
    BadRange,
    TooFar,
    NotFound,
    Failed(Failure),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RehearsalOutcome {
    Started(Event),
    Running,
    NameTaken,
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
    pub events: usize,
    pub messages: usize,
    pub answers: usize,
    pub failures: usize,
}

pub const ROSTER_LIMIT: usize = 60;

const MILLIS_PER_SECOND: i64 = 1_000;
const HOURS_TARGET: &str = "hours";

#[derive(Default)]
struct Slot {
    edit: Mutex<Settled>,
    wanted: AtomicU64,
    latest: AtomicI64,
    owed: AtomicBool,
}

#[derive(Default)]
struct Settled {
    through: u64,
    drawn: Option<PostState>,
}

enum Judged {
    Open {
        event: Box<Event>,
        session: Session,
        now_unix: i64,
        now_ms: u64,
    },
    Closed {
        start_unix: i64,
    },
    Gone,
    Failed(Failure),
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
        let _beating = self.beats.lock().await;
        let live = match self.store.live_events(now_unix).await {
            Ok(live) => live,
            Err(error) => {
                post_failed(
                    "the live events could not be read",
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
        for guild in live.events.iter().map(|event| event.guild) {
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
        let mut report = TickReport {
            failures: skipped(&live.unreadable),
            ..TickReport::default()
        };
        let mut groups: BTreeMap<i64, Vec<Event>> = BTreeMap::new();
        for event in live.events {
            match offsets.get(&event.guild).copied().flatten() {
                Some(offset) => groups.entry(offset).or_default().push(event),
                None => report.failures += 1,
            }
        }
        for (offset, events) in groups {
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
            let beaten = self.beat(events, instant, instant_ms).await;
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
        let _beating = self.beats.lock().await;
        let live = match self.store.live_events(now_unix).await {
            Ok(live) => live,
            Err(error) => {
                post_failed(
                    "the live events could not be read",
                    &error,
                    &Scope::default().guild(guild),
                );
                return TickReport {
                    failures: 1,
                    ..TickReport::default()
                };
            }
        };
        let unreadable: Vec<UnreadableEvent> = live
            .unreadable
            .into_iter()
            .filter(|unreadable| unreadable.guild == guild)
            .collect();
        let events = live
            .events
            .into_iter()
            .filter(|event| event.guild == guild)
            .collect();
        let skipped = TickReport {
            failures: skipped(&unreadable),
            ..TickReport::default()
        };
        skipped.joined(self.beat(events, now_unix, now_ms).await)
    }

    pub async fn purge(self: &Arc<Self>, guild: GuildId, now_unix: i64) -> PurgeReport {
        let events = match self.store.events_in_any_state(guild).await {
            Ok(events) => events,
            Err(error) => {
                post_failed(
                    "the server's events could not be read for a purge",
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
        for event in &events {
            let posts = self.clear_posts(event, ClearScope::All, now_unix).await;
            cleared.touched += posts.touched;
            cleared.failures += posts.failures;
        }
        if cleared.failures != 0 {
            return PurgeReport {
                events: 0,
                messages: cleared.touched,
                answers: 0,
                failures: cleared.failures,
            };
        }
        let mut report = PurgeReport {
            messages: cleared.touched,
            ..PurgeReport::default()
        };
        for event in &events {
            match self
                .store
                .purge_event(guild, event.id, event.created_at_ms)
                .await
            {
                Ok(None) => {}
                Ok(Some(answers)) => {
                    report.events += 1;
                    report.answers += usize::try_from(answers).unwrap_or(usize::MAX);
                }
                Err(error) => {
                    post_failed("an event could not be purged", &error, &event_scope(event));
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

    pub async fn start_rehearsal(&self, new: NewEvent, now_unix: i64) -> RehearsalOutcome {
        let events = match self.store.events_in(new.guild).await {
            Ok(events) => events,
            Err(error) => return RehearsalOutcome::Failed(Failure::from_error(&error)),
        };
        if !listed_events(&events, now_unix).is_empty() {
            return RehearsalOutcome::Running;
        }
        match self.store.create_event(&new).await {
            Ok(CreateOutcome::Created(event)) => RehearsalOutcome::Started(event),
            Ok(CreateOutcome::NameTaken) => RehearsalOutcome::NameTaken,
            Err(error) => RehearsalOutcome::Failed(Failure::from_error(&error)),
        }
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

    async fn beat(&self, events: Vec<Event>, now_unix: i64, now_ms: u64) -> TickReport {
        let mut report = TickReport::default();
        for event in &events {
            let posts = match self.store.posts(event).await {
                Ok(posts) => posts,
                Err(error) => {
                    post_failed(
                        "an event's sign-up posts could not be read",
                        &error,
                        &event_scope(event),
                    );
                    report.failures += 1;
                    continue;
                }
            };
            let mut swept = Vec::new();
            for post in posts
                .iter()
                .filter(|post| post.state != PostState::Removed)
                .filter(|post| post.session.remove_at_unix() <= now_unix)
            {
                let tag = PostTag {
                    event: event.id,
                    session: post.session,
                };
                if self.remove(event, post).await {
                    swept.push(post.session);
                    report.removed.push(tag);
                } else {
                    report.failures += 1;
                }
            }
            for post in posts
                .iter()
                .filter(|post| post.state != PostState::Removed)
                .filter(|post| !swept.contains(&post.session))
                .filter(|post| {
                    post.state != state_at(post.session, now_unix) || self.owes(post.message)
                })
            {
                let tag = PostTag {
                    event: event.id,
                    session: post.session,
                };
                let scope = post_scope(event, post.session).message(post.message);
                let Some(drawn) = self
                    .redraw(event, post.session.date(), post.message, now_unix)
                    .await
                else {
                    report.failures += 1;
                    continue;
                };
                if drawn == post.state {
                    continue;
                }
                if let Err(error) = self
                    .store
                    .set_post_state(event.id, post.session.date(), drawn)
                    .await
                {
                    let summary = match drawn {
                        PostState::Closed => "a closed sign-up post could not be saved",
                        PostState::Open | PostState::Removed => {
                            "a reopened sign-up post could not be saved"
                        }
                    };
                    post_failed(summary, &error, &scope);
                    report.failures += 1;
                    continue;
                }
                match drawn {
                    PostState::Closed => {
                        failure::record("signup.post.closed", "a sign-up post closed", &scope);
                        report.closed.push(tag);
                    }
                    PostState::Open | PostState::Removed => {
                        failure::record("signup.post.reopened", "a sign-up post reopened", &scope);
                    }
                }
            }
            for session in event.span.due_sessions(now_unix) {
                let tag = PostTag {
                    event: event.id,
                    session,
                };
                let scope = post_scope(event, session);
                let delivery = match self.store.post(event, session.date()).await {
                    Ok(Some(post)) if post.state != PostState::Removed => continue,
                    Ok(Some(_)) => Delivery::Redraw,
                    Ok(None) => Delivery::New,
                    Err(error) => {
                        post_failed("a due sign-up post could not be looked up", &error, &scope);
                        report.failures += 1;
                        continue;
                    }
                };
                let event = &match self.store.event(event.id).await {
                    Ok(Some(fresh)) if fresh.ended_at_ms.is_none() => fresh,
                    Ok(_) => continue,
                    Err(error) => {
                        post_failed(
                            "an event could not be read before publishing its sign-up post",
                            &error,
                            &scope,
                        );
                        report.failures += 1;
                        continue;
                    }
                };
                let scope = post_scope(event, session);
                match self.board.find_post(event.channel, tag).await {
                    Ok(Some(message)) => {
                        let scope = scope.message(message);
                        if self
                            .record(event.id, session, message, now_ms, &scope)
                            .await
                        {
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
                        let view = match self.view(event, session, now_unix).await {
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
                        match self.board.send_post(event.channel, &view, delivery).await {
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
                                    .record(event.id, session, sent.message, now_ms, &scope)
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
                                post_failed(
                                    "a sign-up post could not be published",
                                    &error,
                                    &scope,
                                );
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
        }
        report
    }

    pub async fn create_event(&self, new: &NewEvent) -> Result<CreateOutcome, AttendanceError> {
        let _beating = self.beats.lock().await;
        self.store.create_event(new).await
    }

    pub async fn clear_posts(
        self: &Arc<Self>,
        event: &Event,
        scope: ClearScope,
        _now_unix: i64,
    ) -> PostsReport {
        let _beating = self.beats.lock().await;
        self.clear_held(event, scope).await
    }

    async fn clear_held(&self, event: &Event, scope: ClearScope) -> PostsReport {
        let posts = match self.store.posts(event).await {
            Ok(posts) => posts,
            Err(error) => {
                post_failed(
                    "an event's sign-up posts could not be read",
                    &error,
                    &event_scope(event),
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
                ClearScope::OutsideSchedule => !event.span.holds(post.session.date()),
            })
        {
            if self.remove(event, post).await {
                report.touched += 1;
            } else {
                report.failures += 1;
            }
        }
        report
    }

    pub async fn edit_event(
        self: &Arc<Self>,
        guild: GuildId,
        id: i64,
        created_at_ms: u64,
        change: &EventChange,
        today: Date,
        now_unix: i64,
    ) -> EditReport {
        let _beating = self.beats.lock().await;
        let moved = match change.channel {
            None => PostsReport::default(),
            Some(channel) => {
                let held = match self
                    .store
                    .judge_edit(guild, id, created_at_ms, change, today, now_unix)
                    .await
                {
                    Ok(EditOutcome::Edited { before, .. }) => before,
                    Ok(EditOutcome::NameTaken) => return EditReport::NameTaken,
                    Ok(EditOutcome::BadRange) => return EditReport::BadRange,
                    Ok(EditOutcome::TooFar) => return EditReport::TooFar,
                    Ok(EditOutcome::NotFound) => return EditReport::NotFound,
                    Err(error) => return EditReport::Failed(Failure::from_error(&error)),
                };
                if held.channel == channel {
                    PostsReport::default()
                } else {
                    let cleared = self.clear_held(&held, ClearScope::All).await;
                    if cleared.failures > 0 {
                        return EditReport::ClearFailed {
                            failures: cleared.failures,
                        };
                    }
                    cleared
                }
            }
        };
        match self
            .store
            .edit_event(guild, id, created_at_ms, change, today, now_unix)
            .await
        {
            Ok(EditOutcome::Edited { before, after }) => {
                let outside = self.clear_held(&after, ClearScope::OutsideSchedule).await;
                let refreshed = self.refresh_held(&after, now_unix).await;
                EditReport::Edited {
                    before,
                    after,
                    cleared: PostsReport {
                        touched: moved.touched + outside.touched,
                        failures: outside.failures,
                    },
                    refreshed,
                }
            }
            Ok(EditOutcome::NameTaken) => EditReport::NameTaken,
            Ok(EditOutcome::BadRange) => EditReport::BadRange,
            Ok(EditOutcome::TooFar) => EditReport::TooFar,
            Ok(EditOutcome::NotFound) => EditReport::NotFound,
            Err(error) => EditReport::Failed(Failure::from_error(&error)),
        }
    }

    pub async fn refresh_posts(self: &Arc<Self>, event: &Event, now_unix: i64) -> PostsReport {
        let _beating = self.beats.lock().await;
        self.refresh_held(event, now_unix).await
    }

    async fn refresh_held(&self, event: &Event, now_unix: i64) -> PostsReport {
        let posts = match self.store.posts(event).await {
            Ok(posts) => posts,
            Err(error) => {
                post_failed(
                    "an event's sign-up posts could not be read",
                    &error,
                    &event_scope(event),
                );
                return PostsReport {
                    touched: 0,
                    failures: 1,
                };
            }
        };
        let mut report = PostsReport::default();
        for post in posts.iter().filter(|post| post.state != PostState::Removed) {
            let Some(drawn) = self
                .redraw(event, post.session.date(), post.message, now_unix)
                .await
            else {
                report.failures += 1;
                continue;
            };
            if drawn != post.state
                && let Err(error) = self
                    .store
                    .set_post_state(event.id, post.session.date(), drawn)
                    .await
            {
                post_failed(
                    "an edited sign-up post's state could not be saved",
                    &error,
                    &post_scope(event, post.session).message(post.message),
                );
                report.failures += 1;
                continue;
            }
            report.touched += 1;
        }
        report
    }

    pub async fn click(self: &Arc<Self>, click: Click, now_unix: i64, now_ms: u64) -> ClickOutcome {
        let hour = match click.target {
            Target::All => None,
            Target::One(hour) => Some(hour),
        };
        let (event, session, now_unix, now_ms) = match self
            .judge(click.guild, click.event, click.date, hour, now_unix, now_ms)
            .await
        {
            Judged::Open {
                event,
                session,
                now_unix,
                now_ms,
            } => (event, session, now_unix, now_ms),
            Judged::Closed { start_unix } => return ClickOutcome::Closed { start_unix },
            Judged::Gone => return ClickOutcome::UnknownEvent,
            Judged::Failed(failure) => return ClickOutcome::Failed(failure),
        };
        let hours: Vec<Hour> = match click.target {
            Target::All => session.hours().each().collect(),
            Target::One(hour) => vec![hour],
        };
        if let Err(error) = self
            .store
            .mark(
                event.id,
                session.date(),
                click.user,
                &hours,
                session.hours(),
                click.attending,
                now_ms,
            )
            .await
        {
            return ClickOutcome::Failed(Failure::from_error(&error));
        }
        self.answered(&event, session, Some(click.message), now_unix)
            .await;
        failure::click_recorded(
            &target_name(click.target),
            choice_name(click.attending),
            &Scope::default()
                .guild(event.guild)
                .channel(click.channel)
                .season(event.id)
                .night(session.label())
                .message(click.message)
                .user(click.user),
        );
        ClickOutcome::Recorded
    }

    pub async fn hours_menu(
        self: &Arc<Self>,
        guild: GuildId,
        event_id: i64,
        date: Date,
        user: UserId,
        now_unix: i64,
        now_ms: u64,
    ) -> MenuOutcome {
        let (event, session, now_unix) = match self
            .judge(guild, event_id, date, None, now_unix, now_ms)
            .await
        {
            Judged::Open {
                event,
                session,
                now_unix,
                ..
            } => (event, session, now_unix),
            Judged::Closed { start_unix } => return MenuOutcome::Closed { start_unix },
            Judged::Gone => return MenuOutcome::UnknownEvent,
            Judged::Failed(failure) => return MenuOutcome::Failed(failure),
        };
        let marks = match self.store.roster(event.id, session.date()).await {
            Ok(marks) => marks,
            Err(error) => return MenuOutcome::Failed(Failure::from_error(&error)),
        };
        let attending: Vec<Hour> = marks
            .iter()
            .filter(|mark| mark.user == user && mark.attending)
            .map(|mark| mark.hour)
            .collect();
        match self.view(&event, session, now_unix).await {
            Ok(view) => MenuOutcome::Open { view, attending },
            Err(error) => MenuOutcome::Failed(Failure::from_error(&error)),
        }
    }

    pub async fn choose_hours(
        self: &Arc<Self>,
        pick: HoursPick,
        now_unix: i64,
        now_ms: u64,
    ) -> ChooseOutcome {
        let (event, session, now_unix, now_ms) = match self
            .judge(pick.guild, pick.event, pick.date, None, now_unix, now_ms)
            .await
        {
            Judged::Open {
                event,
                session,
                now_unix,
                now_ms,
            } => (event, session, now_unix, now_ms),
            Judged::Closed { start_unix } => return ChooseOutcome::Closed { start_unix },
            Judged::Gone => return ChooseOutcome::UnknownEvent,
            Judged::Failed(failure) => return ChooseOutcome::Failed(failure),
        };
        let Some(attending) = wiring::hour_choices(&pick.values, session.hours()) else {
            return ChooseOutcome::BadChoice;
        };
        if let Err(error) = self
            .store
            .set_hours(
                event.id,
                session.date(),
                pick.user,
                &attending,
                session.hours(),
                now_ms,
            )
            .await
        {
            return ChooseOutcome::Failed(Failure::from_error(&error));
        }
        let redrawn = self.answered(&event, session, None, now_unix).await;
        let scope = Scope::default()
            .guild(event.guild)
            .channel(pick.channel)
            .season(event.id)
            .night(session.label());
        let scope = match redrawn {
            Some(message) => scope.message(message),
            None => scope,
        };
        failure::click_recorded(
            HOURS_TARGET,
            choice_name(!attending.is_empty()),
            &scope.user(pick.user),
        );
        ChooseOutcome::Saved { attending }
    }

    async fn judge(
        &self,
        guild: GuildId,
        event: i64,
        date: Date,
        hour: Option<Hour>,
        now_unix: i64,
        now_ms: u64,
    ) -> Judged {
        let event = match self.store.event(event).await {
            Ok(Some(event)) => event,
            Ok(None) => return Judged::Gone,
            Err(error) => return Judged::Failed(Failure::from_error(&error)),
        };
        let Some(session) = (event.guild == guild && event.ended_at_ms.is_none())
            .then(|| event.span.session_on(date))
            .flatten()
            .filter(|session| hour.is_none_or(|hour| session.hours().holds(hour)))
        else {
            return Judged::Gone;
        };
        let (now_unix, now_ms) = self.rehearsal_instant(event.guild, now_unix, now_ms).await;
        if session.close_unix() <= now_unix {
            return Judged::Closed {
                start_unix: session.start_unix(),
            };
        }
        Judged::Open {
            event: Box::new(event),
            session,
            now_unix,
            now_ms,
        }
    }

    async fn answered(
        &self,
        event: &Event,
        session: Session,
        message: Option<Snowflake>,
        now_unix: i64,
    ) -> Option<Snowflake> {
        let current = match self.store.post(event, session.date()).await {
            Ok(Some(post)) if post.state != PostState::Removed => Some(post.message),
            Ok(_) => message,
            Err(error) => {
                let scope = match message {
                    Some(message) => post_scope(event, session).message(message),
                    None => post_scope(event, session),
                };
                post_failed(
                    "a sign-up post could not be looked up to redraw it after a click",
                    &error,
                    &scope,
                );
                message
            }
        }?;
        self.redraw(event, session.date(), current, now_unix).await;
        Some(current)
    }

    pub async fn repost(
        self: &Arc<Self>,
        guild: GuildId,
        event_id: i64,
        ping: RepostPing,
        now_unix: i64,
        now_ms: u64,
    ) -> RepostOutcome {
        let _beating = self.beats.lock().await;
        let scope = Scope::default().guild(guild);
        let event = match self.store.live_event(guild, event_id).await {
            Ok(Some(event)) => event,
            Ok(None) => return RepostOutcome::NotFound,
            Err(error) => {
                return repost_failed(
                    "an event could not be read to repost its sign-up post",
                    &error,
                    &scope,
                );
            }
        };
        if matches!(ping, RepostPing::Again { .. }) && event.ping_roles.is_empty() {
            return RepostOutcome::NothingToPing;
        }
        if let RepostPing::Again { checked, channel } = &ping
            && (*checked != event.ping_roles || *channel != event.channel)
        {
            return RepostOutcome::PingsChanged;
        }
        let (instant, _) = self.rehearsal_instant(guild, now_unix, now_ms).await;
        let posts = match self.store.posts(&event).await {
            Ok(posts) => posts,
            Err(error) => {
                return repost_failed(
                    "an event's sign-up posts could not be read to repost one",
                    &error,
                    &event_scope(&event),
                );
            }
        };
        let Some(post) = posts
            .iter()
            .find(|post| post.state == PostState::Open && instant < post.session.close_unix())
        else {
            return RepostOutcome::NothingOpen {
                next_post_at: next_post_at(&event.span, instant),
                sessions_left: event.span.sessions_left(instant),
            };
        };
        let session = post.session;
        let previous = post.message;
        let scope = post_scope(&event, session).message(previous);
        let view = match self.view(&event, session, instant).await {
            Ok(view) => view,
            Err(error) => {
                return repost_failed(
                    "the roster for a reposted sign-up post could not be read",
                    &error,
                    &scope,
                );
            }
        };
        if let Err(error) = self.board.delete_post(event.channel, previous).await {
            return repost_failed(
                "a sign-up post could not be deleted to repost it",
                &error,
                &scope,
            );
        }
        self.forget(previous);
        let tag = PostTag {
            event: event.id,
            session,
        };
        let scope = post_scope(&event, session);
        let (message, adopted, pinged, unheard) = match self
            .board
            .find_post(event.channel, tag)
            .await
        {
            Ok(Some(message)) => (message, true, Vec::new(), Vec::new()),
            Ok(None) => {
                let delivery = match ping {
                    RepostPing::Again { .. } => Delivery::New,
                    RepostPing::Quiet => Delivery::Redraw,
                };
                match self.board.send_post(event.channel, &view, delivery).await {
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
            .replace_post_message(event.id, session.date(), previous, message)
            .await
        {
            Ok(true) => {}
            Ok(false) => {
                self.discard(&event, message, &scope).await;
                return RepostOutcome::Superseded;
            }
            Err(error) => {
                let outcome =
                    repost_failed("a reposted sign-up post could not be saved", &error, &scope);
                self.discard(&event, message, &scope).await;
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
        self.redraw(&event, session.date(), message, instant).await;
        RepostOutcome::Reposted {
            session,
            previous,
            message,
            adopted,
            pinged,
            unheard,
        }
    }

    async fn discard(&self, event: &Event, message: Snowflake, scope: &Scope) {
        if let Err(error) = self.board.delete_post(event.channel, message).await {
            post_failed(
                "a reposted sign-up post that could not be kept could not be deleted",
                &error,
                scope,
            );
        }
    }

    pub async fn view(
        &self,
        event: &Event,
        session: Session,
        now_unix: i64,
    ) -> Result<SignupView, AttendanceError> {
        let marks = self.store.roster(event.id, session.date()).await?;
        Ok(build_view(event, session, now_unix, &marks))
    }

    async fn remove(&self, event: &Event, post: &Post) -> bool {
        let scope = post_scope(event, post.session).message(post.message);
        match self.board.delete_post(event.channel, post.message).await {
            Ok(Removal::Deleted | Removal::Gone) => {
                match self
                    .store
                    .set_post_state(event.id, post.session.date(), PostState::Removed)
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
        event: &Event,
        date: Date,
        message: Snowflake,
        now_unix: i64,
    ) -> Option<PostState> {
        let slot = self.slot(message);
        slot.latest.fetch_max(now_unix, SeqCst);
        let mine = slot.wanted.fetch_add(1, SeqCst) + 1;
        let mut settled = slot.edit.lock().await;
        if settled.through >= mine {
            return settled.drawn;
        }
        let through = slot.wanted.load(SeqCst);
        let drawn = self
            .draw(event, date, message, slot.latest.load(SeqCst))
            .await;
        slot.owed.store(drawn.is_none(), SeqCst);
        *settled = Settled { through, drawn };
        drawn
    }

    async fn draw(
        &self,
        event: &Event,
        date: Date,
        message: Snowflake,
        now_unix: i64,
    ) -> Option<PostState> {
        let scope = event_scope(event).night(date.to_string()).message(message);
        let event = match self.store.event(event.id).await {
            Ok(Some(event)) => event,
            Ok(None) => {
                failure::report(
                    "signup.post.failed",
                    "a sign-up post could not be redrawn because its event is gone",
                    &Failure::internal(format!("event {} no longer exists", event.id)),
                    &scope,
                );
                return None;
            }
            Err(error) => {
                post_failed(
                    "an event could not be read to redraw its sign-up post",
                    &error,
                    &scope,
                );
                return None;
            }
        };
        let Some(session) = event.span.session_on(date) else {
            failure::report(
                "signup.post.failed",
                "a sign-up post could not be redrawn because its event no longer holds its session",
                &Failure::internal(format!("event {} holds no session on {date}", event.id)),
                &scope,
            );
            return None;
        };
        let view = match self.view(&event, session, now_unix).await {
            Ok(view) => view,
            Err(error) => {
                post_failed(
                    "the roster for a sign-up post could not be read",
                    &error,
                    &scope,
                );
                return None;
            }
        };
        if let Err(error) = self.board.edit_post(event.channel, message, &view).await {
            post_failed("a sign-up post could not be redrawn", &error, &scope);
            return None;
        }
        Some(if view.open {
            PostState::Open
        } else {
            PostState::Closed
        })
    }

    async fn record(
        &self,
        event: i64,
        session: Session,
        message: Snowflake,
        now_ms: u64,
        scope: &Scope,
    ) -> bool {
        self.store
            .record_post(event, session.date(), message, now_ms)
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

    fn owes(&self, message: Snowflake) -> bool {
        self.slots
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(&message)
            .is_some_and(|slot| slot.owed.load(SeqCst))
    }

    fn forget(&self, message: Snowflake) {
        let mut slots = self
            .slots
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        slots.remove(&message);
    }
}

pub fn listed_events(events: &[Event], now_unix: i64) -> Vec<&Event> {
    events
        .iter()
        .filter(|event| event.running(now_unix))
        .collect()
}

pub fn next_post_at(span: &Span, now_unix: i64) -> Option<i64> {
    span.due_sessions(now_unix)
        .next()
        .is_none()
        .then(|| span.next_session(now_unix).map(Session::post_at_unix))
        .flatten()
}

pub fn next_moment(events: &[Event], posts: &[(i64, Vec<Post>)], now_unix: i64) -> Option<i64> {
    events
        .iter()
        .flat_map(|event| {
            let posted = posts
                .iter()
                .find(|(id, _)| *id == event.id)
                .map_or(&[][..], |(_, posts)| posts.as_slice());
            moments(event, posted, now_unix)
        })
        .filter(|moment| *moment > now_unix)
        .min()
}

fn moments<'a>(
    event: &'a Event,
    posts: &'a [Post],
    now_unix: i64,
) -> impl Iterator<Item = i64> + 'a {
    let removals = posts
        .iter()
        .filter(|post| post.state != PostState::Removed)
        .map(|post| post.session.remove_at_unix());
    let closes = posts
        .iter()
        .filter(|post| post.state == PostState::Open)
        .map(|post| post.session.close_unix());
    let postings = event
        .span
        .sessions()
        .filter(move |session| session.start_unix() > now_unix)
        .filter(|session| {
            !posts.iter().any(|post| {
                post.session.date() == session.date() && post.state != PostState::Removed
            })
        })
        .map(Session::post_at_unix);
    removals.chain(closes).chain(postings)
}

fn skipped(unreadable: &[UnreadableEvent]) -> usize {
    for event in unreadable {
        post_failed(
            "an event could not be read, so its sign-up posts were skipped",
            &event.error,
            &Scope::default().guild(event.guild).season(event.id),
        );
    }
    unreadable.len()
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

fn event_scope(event: &Event) -> Scope {
    Scope::default()
        .guild(event.guild)
        .channel(event.channel)
        .season(event.id)
}

fn post_scope(event: &Event, session: Session) -> Scope {
    event_scope(event).night(session.label())
}

fn state_at(session: Session, now_unix: i64) -> PostState {
    if now_unix < session.close_unix() {
        PostState::Open
    } else {
        PostState::Closed
    }
}

fn shifted(now_unix: i64, now_ms: u64, offset: i64) -> Option<(i64, u64)> {
    let instant = now_unix.checked_add(offset)?;
    let instant_ms = now_ms.checked_add_signed(offset.checked_mul(MILLIS_PER_SECOND)?)?;
    Some((instant, instant_ms))
}

fn build_view(event: &Event, session: Session, now_unix: i64, marks: &[Mark]) -> SignupView {
    let hours = session.hours();
    let marks: Vec<Mark> = marks
        .iter()
        .copied()
        .filter(|mark| hours.holds(mark.hour))
        .collect();
    let rows = roster_rows(&marks, hours);
    let hidden = rows.len().saturating_sub(ROSTER_LIMIT);
    SignupView {
        event: event.id,
        name: event.name.clone(),
        session,
        open: now_unix < session.close_unix(),
        pings: event
            .ping_roles
            .iter()
            .map(|role| Ping::of(event.guild, *role))
            .collect(),
        hours: hours.each().map(|hour| tally(hour, &marks)).collect(),
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

fn roster_rows(marks: &[Mark], hours: Hours) -> Vec<RosterRow> {
    let mut rows: Vec<RosterRow> = Vec::new();
    let mut seen: HashMap<UserId, usize> = HashMap::new();
    for mark in marks {
        let index = *seen.entry(mark.user).or_insert_with(|| {
            rows.push(RosterRow {
                user: mark.user,
                cells: vec![Cell::Out; usize::from(hours.get())],
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
