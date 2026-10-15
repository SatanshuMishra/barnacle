use std::path::Path;
use std::time::Duration;

use barnacle_guess::Snowflake;
use barnacle_guess::UserId;
use jiff::Timestamp;
use jiff::civil::Date;
use jiff::tz::Offset;
use sqlx::sqlite::SqliteConnectOptions;
use sqlx::sqlite::SqliteExecutor;
use sqlx::sqlite::SqlitePool;
use sqlx::sqlite::SqlitePoolOptions;

use crate::ids::ChannelId;
use crate::ids::GuildId;
use crate::ids::RoleId;
use crate::schedule::Hour;
use crate::schedule::Hours;
use crate::schedule::SECONDS_PER_HOUR;
use crate::schedule::Schedule;
use crate::schedule::Session;
use crate::schedule::Span;
use crate::schedule::StartTime;
use crate::schedule::Weekdays;
use crate::schedule::Zone;
use crate::schedule::parse_day;
use crate::solves::Column;
use crate::wiring::same_name;

const BUSY_TIMEOUT: Duration = Duration::from_secs(5);
const LIVE_LOOKBACK_SECONDS: i64 = 48 * SECONDS_PER_HOUR;
const MILLIS_PER_SECOND: u64 = 1_000;

const CB_SEASONS: &str = "cb_seasons";
const CB_SEASON_PING_ROLES: &str = "cb_season_ping_roles";
const LIVE_NAME_INDEX: &str = "cb_seasons_live_name";
const CB_POSTS: &str = "cb_posts";
const CB_MARKS: &str = "cb_marks";
const CB_REHEARSAL_CLOCK: &str = "cb_rehearsal_clock";

type ColumnSpec = (&'static str, &'static str, bool, bool);
type TableSpec = (&'static str, &'static [ColumnSpec]);

const SEASONS_BEFORE_PING_ROLES: [ColumnSpec; 12] = [
    ("id", "INTEGER", false, true),
    ("guild_id", "INTEGER", true, false),
    ("channel_id", "INTEGER", true, false),
    ("number", "INTEGER", true, false),
    ("codename", "TEXT", false, false),
    ("first_day", "TEXT", true, false),
    ("last_day", "TEXT", true, false),
    ("created_by", "INTEGER", true, false),
    ("created_at_ms", "INTEGER", true, false),
    ("ping_role_id", "INTEGER", false, false),
    ("ended_at_ms", "INTEGER", false, false),
    ("every_day", "INTEGER", true, false),
];

const SEASONS_BEFORE_EVENTS: [ColumnSpec; 11] = [
    ("id", "INTEGER", false, true),
    ("guild_id", "INTEGER", true, false),
    ("channel_id", "INTEGER", true, false),
    ("number", "INTEGER", true, false),
    ("codename", "TEXT", false, false),
    ("first_day", "TEXT", true, false),
    ("last_day", "TEXT", true, false),
    ("created_by", "INTEGER", true, false),
    ("created_at_ms", "INTEGER", true, false),
    ("ended_at_ms", "INTEGER", false, false),
    ("every_day", "INTEGER", true, false),
];

const EXPECTED_COLUMNS: [TableSpec; 5] = [
    (
        CB_SEASONS,
        &[
            ("id", "INTEGER", false, true),
            ("guild_id", "INTEGER", true, false),
            ("channel_id", "INTEGER", true, false),
            ("name", "TEXT", true, false),
            ("first_day", "TEXT", true, false),
            ("last_day", "TEXT", true, false),
            ("weekdays", "INTEGER", true, false),
            ("start_minute", "INTEGER", true, false),
            ("time_zone", "TEXT", true, false),
            ("hours", "INTEGER", true, false),
            ("created_by", "INTEGER", true, false),
            ("created_at_ms", "INTEGER", true, false),
            ("ended_at_ms", "INTEGER", false, false),
        ],
    ),
    (
        CB_SEASON_PING_ROLES,
        &[
            ("season_id", "INTEGER", true, true),
            ("position", "INTEGER", true, true),
            ("role_id", "INTEGER", true, false),
        ],
    ),
    (
        CB_POSTS,
        &[
            ("season_id", "INTEGER", true, true),
            ("night", "TEXT", true, true),
            ("message_id", "INTEGER", true, false),
            ("state", "TEXT", true, false),
            ("posted_at_ms", "INTEGER", true, false),
        ],
    ),
    (
        CB_MARKS,
        &[
            ("season_id", "INTEGER", true, true),
            ("night", "TEXT", true, true),
            ("user_id", "INTEGER", true, true),
            ("hour", "INTEGER", true, true),
            ("attending", "INTEGER", true, false),
            ("answered_at_ms", "INTEGER", true, false),
            ("changed_at_ms", "INTEGER", true, false),
        ],
    ),
    (
        CB_REHEARSAL_CLOCK,
        &[
            ("guild_id", "INTEGER", false, true),
            ("offset_s", "INTEGER", true, false),
        ],
    ),
];

const EVENT_BY_ID: &str = "SELECT id, guild_id, channel_id, name, first_day, last_day, weekdays, start_minute, time_zone, hours, created_by, created_at_ms, ended_at_ms, EXISTS (SELECT 1 FROM cb_posts WHERE cb_posts.season_id = cb_seasons.id AND cb_posts.state <> 'removed') FROM cb_seasons WHERE id = ?";
const EVENT_LIVE_BY_ID: &str = "SELECT id, guild_id, channel_id, name, first_day, last_day, weekdays, start_minute, time_zone, hours, created_by, created_at_ms, ended_at_ms, EXISTS (SELECT 1 FROM cb_posts WHERE cb_posts.season_id = cb_seasons.id AND cb_posts.state <> 'removed') FROM cb_seasons WHERE guild_id = ? AND id = ? AND ended_at_ms IS NULL";
const EVENT_LIVE_BY_CREATION: &str = "SELECT id, guild_id, channel_id, name, first_day, last_day, weekdays, start_minute, time_zone, hours, created_by, created_at_ms, ended_at_ms, EXISTS (SELECT 1 FROM cb_posts WHERE cb_posts.season_id = cb_seasons.id AND cb_posts.state <> 'removed') FROM cb_seasons WHERE guild_id = ? AND id = ? AND created_at_ms = ? AND ended_at_ms IS NULL";
const EVENTS_IN_GUILD: &str = "SELECT id, guild_id, channel_id, name, first_day, last_day, weekdays, start_minute, time_zone, hours, created_by, created_at_ms, ended_at_ms, EXISTS (SELECT 1 FROM cb_posts WHERE cb_posts.season_id = cb_seasons.id AND cb_posts.state <> 'removed') FROM cb_seasons WHERE guild_id = ? AND ended_at_ms IS NULL ORDER BY first_day, id";
const EVENTS_IN_GUILD_ANY_STATE: &str = "SELECT id, guild_id, channel_id, name, first_day, last_day, weekdays, start_minute, time_zone, hours, created_by, created_at_ms, ended_at_ms, EXISTS (SELECT 1 FROM cb_posts WHERE cb_posts.season_id = cb_seasons.id AND cb_posts.state <> 'removed') FROM cb_seasons WHERE guild_id = ? ORDER BY first_day, id";
const EVENTS_LIVE: &str = "SELECT id, guild_id, channel_id, name, first_day, last_day, weekdays, start_minute, time_zone, hours, created_by, created_at_ms, ended_at_ms, EXISTS (SELECT 1 FROM cb_posts WHERE cb_posts.season_id = cb_seasons.id AND cb_posts.state <> 'removed') FROM cb_seasons WHERE ended_at_ms IS NULL AND (last_day >= ? OR EXISTS (SELECT 1 FROM cb_posts WHERE cb_posts.season_id = cb_seasons.id AND cb_posts.state <> 'removed')) ORDER BY first_day, id";

type EventRow = (
    i64,
    i64,
    i64,
    String,
    String,
    String,
    i64,
    i64,
    String,
    i64,
    i64,
    i64,
    Option<i64>,
    i64,
);

type PostRow = (String, i64, String, i64);

type MarkRow = (i64, i64, i64, i64);

#[derive(Debug, thiserror::Error)]
pub enum AttendanceError {
    #[error("the attendance database could not be used")]
    Database(#[from] sqlx::Error),
    #[error("the attendance database has no {table} table")]
    MissingTable { table: &'static str },
    #[error("{table} does not have the expected columns; found {found:?}")]
    UnexpectedColumns {
        table: &'static str,
        found: Vec<Column>,
    },
    #[error("the attendance database has no {index} index")]
    MissingIndex { index: &'static str },
    #[error(
        "the attendance database still keeps one ping role in cb_seasons and has no cb_season_ping_roles table"
    )]
    MissingPingRoles,
    #[error("the attendance database keeps season numbers in cb_seasons and has no event columns")]
    BeforeEvents,
    #[error("{value} does not fit in a SQLite integer")]
    OutOfRange { value: u128 },
    #[error("the attendance database holds a negative value, {value}")]
    Negative { value: i64 },
    #[error("the attendance database holds a player ID below 1, {value}")]
    InvalidPlayer { value: i64 },
    #[error("the attendance database holds {value}, which is not a role ID")]
    InvalidRole { value: i64 },
    #[error("the attendance database holds {value}, which is not a session date")]
    BadNight { value: String },
    #[error("the attendance database holds {value}, which is not a post state")]
    BadState { value: String },
    #[error("the attendance database holds {value}, which is not an hour")]
    BadHour { value: i64 },
    #[error("the attendance database holds {value}, which is not a set of weekdays")]
    BadWeekdays { value: i64 },
    #[error("the attendance database holds {value}, which is not a start time")]
    BadStart { value: i64 },
    #[error("the attendance database holds {value}, which is not a time zone Barnacle knows")]
    BadZone { value: String },
    #[error("the attendance database holds {value}, which is not an hour count from 1 to 24")]
    BadHours { value: i64 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Event {
    pub id: i64,
    pub guild: GuildId,
    pub channel: ChannelId,
    pub name: String,
    pub span: Span,
    pub created_by: UserId,
    pub created_at_ms: u64,
    pub ping_roles: Vec<RoleId>,
    pub ended_at_ms: Option<u64>,
    pub posts_up: bool,
}

impl Event {
    pub fn running(&self, now_unix: i64) -> bool {
        self.ended_at_ms.is_none()
            && (self
                .span
                .last_moment_unix()
                .is_some_and(|moment| moment > now_unix)
                || self.posts_up)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewEvent {
    pub guild: GuildId,
    pub channel: ChannelId,
    pub name: String,
    pub span: Span,
    pub created_by: UserId,
    pub created_at_ms: u64,
    pub ping_roles: Vec<RoleId>,
}

#[derive(Debug)]
pub struct LiveEvents {
    pub events: Vec<Event>,
    pub unreadable: Vec<UnreadableEvent>,
}

#[derive(Debug)]
pub struct UnreadableEvent {
    pub id: i64,
    pub guild: GuildId,
    pub error: AttendanceError,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PostState {
    Open,
    Closed,
    Removed,
}

impl PostState {
    fn as_text(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Closed => "closed",
            Self::Removed => "removed",
        }
    }

    fn from_text(text: &str) -> Option<Self> {
        match text {
            "open" => Some(Self::Open),
            "closed" => Some(Self::Closed),
            "removed" => Some(Self::Removed),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Post {
    pub session: Session,
    pub message: Snowflake,
    pub state: PostState,
    pub posted_at_ms: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Mark {
    pub user: UserId,
    pub hour: Hour,
    pub attending: bool,
    pub answered_at_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CreateOutcome {
    Created(Event),
    NameTaken,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EndOutcome {
    Removed,
    Ended { event: Event },
    NotFound,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EventChange {
    pub name: Option<String>,
    pub first_day: Option<Date>,
    pub last_day: Option<Date>,
    pub weekdays: Option<Weekdays>,
    pub start: Option<StartTime>,
    pub zone: Option<Zone>,
    pub hours: Option<Hours>,
    pub channel: Option<ChannelId>,
    pub ping_roles: Option<Vec<RoleId>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditOutcome {
    Edited {
        before: Box<Event>,
        after: Box<Event>,
    },
    NameTaken,
    BadRange,
    TooFar,
    NotFound,
}

struct NameHolders {
    namesakes: Vec<Event>,
    others: Vec<Result<Event, AttendanceError>>,
}

#[derive(Debug, Clone)]
pub struct Attendance {
    pool: SqlitePool,
}

impl Attendance {
    pub async fn open(path: &Path) -> Result<Self, AttendanceError> {
        let options = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(false)
            .busy_timeout(BUSY_TIMEOUT);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await?;
        Self::with_pool(pool).await
    }

    pub async fn with_pool(pool: SqlitePool) -> Result<Self, AttendanceError> {
        let seasons = columns(&pool, CB_SEASONS).await?;
        if seasons == expected_columns(&SEASONS_BEFORE_PING_ROLES)
            && columns(&pool, CB_SEASON_PING_ROLES).await?.is_empty()
        {
            return Err(AttendanceError::MissingPingRoles);
        }
        if seasons == expected_columns(&SEASONS_BEFORE_EVENTS) {
            return Err(AttendanceError::BeforeEvents);
        }
        for (table, expected) in EXPECTED_COLUMNS {
            let found = columns(&pool, table).await?;
            if found.is_empty() {
                return Err(AttendanceError::MissingTable { table });
            }
            if found != expected_columns(expected) {
                return Err(AttendanceError::UnexpectedColumns { table, found });
            }
        }
        let index: Option<i64> =
            sqlx::query_scalar("SELECT 1 FROM sqlite_master WHERE type = 'index' AND name = ?")
                .bind(LIVE_NAME_INDEX)
                .fetch_optional(&pool)
                .await?;
        if index.is_none() {
            return Err(AttendanceError::MissingIndex {
                index: LIVE_NAME_INDEX,
            });
        }
        Ok(Self { pool })
    }

    pub async fn create_event(&self, new: &NewEvent) -> Result<CreateOutcome, AttendanceError> {
        let guild = to_integer(new.guild.get().into())?;
        let channel = to_integer(new.channel.get().into())?;
        let created_by = to_integer(new.created_by.get().into())?;
        let created_at = to_integer(new.created_at_ms.into())?;
        let now_unix = to_integer((new.created_at_ms / MILLIS_PER_SECOND).into())?;
        let schedule = new.span.schedule();
        let mut transaction = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let holders = name_holders(&mut transaction, guild, &new.name, None).await?;
        if holders
            .namesakes
            .iter()
            .any(|event| event.running(now_unix))
        {
            return Ok(CreateOutcome::NameTaken);
        }
        let clock_free = holders
            .others
            .iter()
            .all(|read| read.as_ref().is_ok_and(|event| !event.running(now_unix)));
        end_finished(&mut transaction, &holders.namesakes, created_at).await?;
        let inserted = sqlx::query(
            "INSERT INTO cb_seasons (guild_id, channel_id, name, first_day, last_day, weekdays, start_minute, time_zone, hours, created_by, created_at_ms) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(guild)
        .bind(channel)
        .bind(&new.name)
        .bind(new.span.first_day().to_string())
        .bind(new.span.last_day().to_string())
        .bind(i64::from(schedule.weekdays().bits()))
        .bind(schedule.start().minutes())
        .bind(schedule.zone().name())
        .bind(i64::from(schedule.hours().get()))
        .bind(created_by)
        .bind(created_at)
        .execute(&mut *transaction)
        .await?;
        let id = inserted.last_insert_rowid();
        insert_ping_roles(&mut transaction, id, &new.ping_roles).await?;
        if clock_free {
            sqlx::query("DELETE FROM cb_rehearsal_clock WHERE guild_id = ?")
                .bind(guild)
                .execute(&mut *transaction)
                .await?;
        }
        transaction.commit().await?;
        Ok(CreateOutcome::Created(Event {
            id,
            guild: new.guild,
            channel: new.channel,
            name: new.name.clone(),
            span: new.span.clone(),
            created_by: new.created_by,
            created_at_ms: new.created_at_ms,
            ping_roles: new.ping_roles.clone(),
            ended_at_ms: None,
            posts_up: false,
        }))
    }

    pub async fn event(&self, id: i64) -> Result<Option<Event>, AttendanceError> {
        let row: Option<EventRow> = sqlx::query_as(EVENT_BY_ID)
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        self.event_of(row).await
    }

    pub async fn live_event(
        &self,
        guild: GuildId,
        id: i64,
    ) -> Result<Option<Event>, AttendanceError> {
        let row: Option<EventRow> = sqlx::query_as(EVENT_LIVE_BY_ID)
            .bind(to_integer(guild.get().into())?)
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        self.event_of(row).await
    }

    pub async fn events_in(&self, guild: GuildId) -> Result<Vec<Event>, AttendanceError> {
        let rows: Vec<EventRow> = sqlx::query_as(EVENTS_IN_GUILD)
            .bind(to_integer(guild.get().into())?)
            .fetch_all(&self.pool)
            .await?;
        self.events_of(rows).await
    }

    pub async fn events_in_any_state(&self, guild: GuildId) -> Result<Vec<Event>, AttendanceError> {
        let rows: Vec<EventRow> = sqlx::query_as(EVENTS_IN_GUILD_ANY_STATE)
            .bind(to_integer(guild.get().into())?)
            .fetch_all(&self.pool)
            .await?;
        self.events_of(rows).await
    }

    pub async fn live_events(&self, now_unix: i64) -> Result<LiveEvents, AttendanceError> {
        let rows: Vec<EventRow> = sqlx::query_as(EVENTS_LIVE)
            .bind(lookback_day(now_unix))
            .fetch_all(&self.pool)
            .await?;
        let mut read = Vec::with_capacity(rows.len());
        for row in rows {
            let (id, guild) = (row.0, row.1);
            let roles = role_ids(&self.pool, id).await?;
            read.push((id, guild, to_event(row, roles)));
        }
        let (events, unreadable): (Vec<_>, Vec<_>) =
            read.into_iter().partition(|(_, _, event)| event.is_ok());
        Ok(LiveEvents {
            events: events
                .into_iter()
                .filter_map(|(_, _, event)| event.ok())
                .filter(|event| event.running(now_unix))
                .collect(),
            unreadable: unreadable
                .into_iter()
                .filter_map(|(id, guild, event)| {
                    event.err().map(|error| UnreadableEvent {
                        id,
                        guild: GuildId::new(u64::try_from(guild).unwrap_or_default()),
                        error,
                    })
                })
                .collect(),
        })
    }

    async fn event_of(&self, row: Option<EventRow>) -> Result<Option<Event>, AttendanceError> {
        match row {
            Some(row) => Ok(Some(event_from(&self.pool, row).await?)),
            None => Ok(None),
        }
    }

    async fn events_of(&self, rows: Vec<EventRow>) -> Result<Vec<Event>, AttendanceError> {
        let mut events = Vec::with_capacity(rows.len());
        for row in rows {
            events.push(event_from(&self.pool, row).await?);
        }
        Ok(events)
    }

    pub async fn end_event(
        &self,
        guild: GuildId,
        id: i64,
        now_ms: u64,
    ) -> Result<EndOutcome, AttendanceError> {
        let ended_at = to_integer(now_ms.into())?;
        let mut transaction = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let found: Option<EventRow> = sqlx::query_as(EVENT_LIVE_BY_ID)
            .bind(to_integer(guild.get().into())?)
            .bind(id)
            .fetch_optional(&mut *transaction)
            .await?;
        let Some(row) = found else {
            return Ok(EndOutcome::NotFound);
        };
        let event = event_from(&mut *transaction, row).await?;
        let posted: Option<i64> =
            sqlx::query_scalar("SELECT 1 FROM cb_posts WHERE season_id = ? LIMIT 1")
                .bind(event.id)
                .fetch_optional(&mut *transaction)
                .await?;
        let outcome = if posted.is_none() {
            sqlx::query("DELETE FROM cb_season_ping_roles WHERE season_id = ?")
                .bind(event.id)
                .execute(&mut *transaction)
                .await?;
            sqlx::query("DELETE FROM cb_seasons WHERE id = ?")
                .bind(event.id)
                .execute(&mut *transaction)
                .await?;
            EndOutcome::Removed
        } else {
            sqlx::query("UPDATE cb_seasons SET ended_at_ms = ? WHERE id = ?")
                .bind(ended_at)
                .bind(event.id)
                .execute(&mut *transaction)
                .await?;
            EndOutcome::Ended { event }
        };
        transaction.commit().await?;
        Ok(outcome)
    }

    pub async fn edit_event(
        &self,
        guild: GuildId,
        id: i64,
        created_at_ms: u64,
        change: &EventChange,
        today: Date,
        now_unix: i64,
    ) -> Result<EditOutcome, AttendanceError> {
        let guild_id = to_integer(guild.get().into())?;
        let mut transaction = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let found: Option<EventRow> = sqlx::query_as(EVENT_LIVE_BY_CREATION)
            .bind(guild_id)
            .bind(id)
            .bind(to_integer(created_at_ms.into())?)
            .fetch_optional(&mut *transaction)
            .await?;
        let Some(row) = found else {
            return Ok(EditOutcome::NotFound);
        };
        let before = event_from(&mut *transaction, row).await?;
        let held = before.span.schedule();
        let schedule = Schedule::new(
            change.weekdays.unwrap_or(held.weekdays()),
            change.start.unwrap_or(held.start()),
            change.zone.clone().unwrap_or_else(|| held.zone().clone()),
            change.hours.unwrap_or(held.hours()),
        );
        let Some(span) = Span::new(
            change.first_day.unwrap_or(before.span.first_day()),
            change.last_day.unwrap_or(before.span.last_day()),
            schedule,
        ) else {
            return Ok(EditOutcome::BadRange);
        };
        if !span.near(today) {
            return Ok(EditOutcome::TooFar);
        }
        let after = Event {
            channel: change.channel.unwrap_or(before.channel),
            name: change.name.clone().unwrap_or_else(|| before.name.clone()),
            span,
            ping_roles: change
                .ping_roles
                .clone()
                .unwrap_or_else(|| before.ping_roles.clone()),
            ..before.clone()
        };
        if change.name.is_some() {
            let holders =
                name_holders(&mut transaction, guild_id, &after.name, Some(before.id)).await?;
            if holders
                .namesakes
                .iter()
                .any(|event| event.running(now_unix))
            {
                return Ok(EditOutcome::NameTaken);
            }
            let ended_at =
                to_integer(u128::from(to_unsigned(now_unix)?) * u128::from(MILLIS_PER_SECOND))?;
            end_finished(&mut transaction, &holders.namesakes, ended_at).await?;
        }
        let schedule = after.span.schedule();
        sqlx::query(
            "UPDATE cb_seasons SET channel_id = ?, name = ?, first_day = ?, last_day = ?, weekdays = ?, start_minute = ?, time_zone = ?, hours = ? WHERE id = ?",
        )
        .bind(to_integer(after.channel.get().into())?)
        .bind(&after.name)
        .bind(after.span.first_day().to_string())
        .bind(after.span.last_day().to_string())
        .bind(i64::from(schedule.weekdays().bits()))
        .bind(schedule.start().minutes())
        .bind(schedule.zone().name())
        .bind(i64::from(schedule.hours().get()))
        .bind(after.id)
        .execute(&mut *transaction)
        .await?;
        if let Some(roles) = &change.ping_roles {
            sqlx::query("DELETE FROM cb_season_ping_roles WHERE season_id = ?")
                .bind(after.id)
                .execute(&mut *transaction)
                .await?;
            insert_ping_roles(&mut transaction, after.id, roles).await?;
        }
        if change.hours.is_some() {
            sqlx::query("DELETE FROM cb_marks WHERE season_id = ? AND hour > ?")
                .bind(after.id)
                .bind(i64::from(schedule.hours().get()))
                .execute(&mut *transaction)
                .await?;
        }
        transaction.commit().await?;
        Ok(EditOutcome::Edited {
            before: Box::new(before),
            after: Box::new(after),
        })
    }

    pub async fn purge_event(
        &self,
        guild: GuildId,
        id: i64,
        created_at_ms: u64,
    ) -> Result<Option<u64>, AttendanceError> {
        let guild_id = to_integer(guild.get().into())?;
        let created_at = to_integer(created_at_ms.into())?;
        let mut transaction = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let held: Option<i64> = sqlx::query_scalar(
            "SELECT id FROM cb_seasons WHERE id = ? AND guild_id = ? AND created_at_ms = ?",
        )
        .bind(id)
        .bind(guild_id)
        .bind(created_at)
        .fetch_optional(&mut *transaction)
        .await?;
        if held.is_none() {
            return Ok(None);
        }
        let marks = sqlx::query("DELETE FROM cb_marks WHERE season_id = ?")
            .bind(id)
            .execute(&mut *transaction)
            .await?
            .rows_affected();
        sqlx::query("DELETE FROM cb_posts WHERE season_id = ?")
            .bind(id)
            .execute(&mut *transaction)
            .await?;
        sqlx::query("DELETE FROM cb_season_ping_roles WHERE season_id = ?")
            .bind(id)
            .execute(&mut *transaction)
            .await?;
        sqlx::query("DELETE FROM cb_seasons WHERE id = ? AND guild_id = ? AND created_at_ms = ?")
            .bind(id)
            .bind(guild_id)
            .bind(created_at)
            .execute(&mut *transaction)
            .await?;
        transaction.commit().await?;
        Ok(Some(marks))
    }

    pub async fn rehearsal_clock(&self, guild: GuildId) -> Result<i64, AttendanceError> {
        let offset: Option<i64> =
            sqlx::query_scalar("SELECT offset_s FROM cb_rehearsal_clock WHERE guild_id = ?")
                .bind(to_integer(guild.get().into())?)
                .fetch_optional(&self.pool)
                .await?;
        Ok(offset.unwrap_or(0))
    }

    pub async fn set_rehearsal_clock(
        &self,
        guild: GuildId,
        offset_s: i64,
    ) -> Result<(), AttendanceError> {
        sqlx::query(
            "INSERT INTO cb_rehearsal_clock (guild_id, offset_s) VALUES (?, ?) ON CONFLICT (guild_id) DO UPDATE SET offset_s = excluded.offset_s",
        )
        .bind(to_integer(guild.get().into())?)
        .bind(offset_s)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn clear_rehearsal_clock(&self, guild: GuildId) -> Result<(), AttendanceError> {
        sqlx::query("DELETE FROM cb_rehearsal_clock WHERE guild_id = ?")
            .bind(to_integer(guild.get().into())?)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn clear_rehearsal_clocks_except(
        &self,
        keep: &[GuildId],
    ) -> Result<usize, AttendanceError> {
        let held: Vec<i64> = sqlx::query_scalar("SELECT guild_id FROM cb_rehearsal_clock")
            .fetch_all(&self.pool)
            .await?;
        let mut cleared = 0;
        for guild_id in held {
            let guild = GuildId::new(to_unsigned(guild_id)?);
            if keep.contains(&guild) {
                continue;
            }
            self.clear_rehearsal_clock(guild).await?;
            cleared += 1;
        }
        Ok(cleared)
    }

    pub async fn post(&self, event: &Event, date: Date) -> Result<Option<Post>, AttendanceError> {
        let row: Option<PostRow> = sqlx::query_as(
            "SELECT night, message_id, state, posted_at_ms FROM cb_posts WHERE season_id = ? AND night = ?",
        )
        .bind(event.id)
        .bind(date.to_string())
        .fetch_optional(&self.pool)
        .await?;
        row.map(|row| to_post(row, event.span.schedule()))
            .transpose()
    }

    pub async fn posts(&self, event: &Event) -> Result<Vec<Post>, AttendanceError> {
        let rows: Vec<PostRow> = sqlx::query_as(
            "SELECT night, message_id, state, posted_at_ms FROM cb_posts WHERE season_id = ? ORDER BY night",
        )
        .bind(event.id)
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter()
            .map(|row| to_post(row, event.span.schedule()))
            .collect()
    }

    pub async fn record_post(
        &self,
        event: i64,
        date: Date,
        message: Snowflake,
        at_ms: u64,
    ) -> Result<(), AttendanceError> {
        sqlx::query(
            "INSERT INTO cb_posts (season_id, night, message_id, state, posted_at_ms) VALUES (?, ?, ?, ?, ?) ON CONFLICT (season_id, night) DO UPDATE SET message_id = excluded.message_id, state = 'open', posted_at_ms = excluded.posted_at_ms",
        )
        .bind(event)
        .bind(date.to_string())
        .bind(to_integer(message.get().into())?)
        .bind(PostState::Open.as_text())
        .bind(to_integer(at_ms.into())?)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn replace_post_message(
        &self,
        event: i64,
        date: Date,
        previous: Snowflake,
        message: Snowflake,
    ) -> Result<bool, AttendanceError> {
        let changed = sqlx::query(
            "UPDATE cb_posts SET message_id = ? WHERE season_id = ? AND night = ? AND message_id = ? AND state = 'open'",
        )
        .bind(to_integer(message.get().into())?)
        .bind(event)
        .bind(date.to_string())
        .bind(to_integer(previous.get().into())?)
        .execute(&self.pool)
        .await?
        .rows_affected();
        Ok(changed > 0)
    }

    pub async fn set_post_state(
        &self,
        event: i64,
        date: Date,
        state: PostState,
    ) -> Result<(), AttendanceError> {
        sqlx::query("UPDATE cb_posts SET state = ? WHERE season_id = ? AND night = ?")
            .bind(state.as_text())
            .bind(event)
            .bind(date.to_string())
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn mark(
        &self,
        event: i64,
        date: Date,
        user: UserId,
        hours: &[Hour],
        all: Hours,
        attending: bool,
        at_ms: u64,
    ) -> Result<(), AttendanceError> {
        let label = date.to_string();
        let user = to_integer(user.get().into())?;
        let at = to_integer(at_ms.into())?;
        let mut transaction = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        for hour in hours {
            sqlx::query(
                "INSERT INTO cb_marks (season_id, night, user_id, hour, attending, answered_at_ms, changed_at_ms) VALUES (?, ?, ?, ?, ?, ?, ?) ON CONFLICT (season_id, night, user_id, hour) DO UPDATE SET attending = excluded.attending, changed_at_ms = excluded.changed_at_ms",
            )
            .bind(event)
            .bind(&label)
            .bind(user)
            .bind(i64::from(hour.get()))
            .bind(i64::from(attending))
            .bind(at)
            .bind(at)
            .execute(&mut *transaction)
            .await?;
        }
        for hour in all.each().filter(|hour| !hours.contains(hour)) {
            sqlx::query(
                "INSERT INTO cb_marks (season_id, night, user_id, hour, attending, answered_at_ms, changed_at_ms) VALUES (?, ?, ?, ?, 0, ?, ?) ON CONFLICT (season_id, night, user_id, hour) DO NOTHING",
            )
            .bind(event)
            .bind(&label)
            .bind(user)
            .bind(i64::from(hour.get()))
            .bind(at)
            .bind(at)
            .execute(&mut *transaction)
            .await?;
        }
        transaction.commit().await?;
        Ok(())
    }

    pub async fn set_hours(
        &self,
        event: i64,
        date: Date,
        user: UserId,
        attending: &[Hour],
        all: Hours,
        at_ms: u64,
    ) -> Result<(), AttendanceError> {
        let label = date.to_string();
        let user = to_integer(user.get().into())?;
        let at = to_integer(at_ms.into())?;
        let mut transaction = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        for hour in all.each() {
            sqlx::query(
                "INSERT INTO cb_marks (season_id, night, user_id, hour, attending, answered_at_ms, changed_at_ms) VALUES (?, ?, ?, ?, ?, ?, ?) ON CONFLICT (season_id, night, user_id, hour) DO UPDATE SET attending = excluded.attending, changed_at_ms = excluded.changed_at_ms",
            )
            .bind(event)
            .bind(&label)
            .bind(user)
            .bind(i64::from(hour.get()))
            .bind(i64::from(attending.contains(&hour)))
            .bind(at)
            .bind(at)
            .execute(&mut *transaction)
            .await?;
        }
        transaction.commit().await?;
        Ok(())
    }

    pub async fn roster(&self, event: i64, date: Date) -> Result<Vec<Mark>, AttendanceError> {
        let rows: Vec<MarkRow> = sqlx::query_as(
            "SELECT user_id, hour, attending, answered_at_ms FROM cb_marks WHERE season_id = ? AND night = ? ORDER BY answered_at_ms, user_id, hour",
        )
        .bind(event)
        .bind(date.to_string())
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter().map(to_mark).collect()
    }
}

async fn columns(pool: &SqlitePool, table: &str) -> Result<Vec<Column>, AttendanceError> {
    let rows: Vec<(String, String, i64, i64)> =
        sqlx::query_as("SELECT name, type, \"notnull\", pk FROM pragma_table_info(?) ORDER BY cid")
            .bind(table)
            .fetch_all(pool)
            .await?;
    Ok(rows
        .into_iter()
        .map(|(name, kind, not_null, primary_key)| Column {
            name,
            kind,
            not_null: not_null != 0,
            primary_key: primary_key != 0,
        })
        .collect())
}

fn expected_columns(expected: &[ColumnSpec]) -> Vec<Column> {
    expected
        .iter()
        .map(|(name, kind, not_null, primary_key)| Column {
            name: (*name).to_owned(),
            kind: (*kind).to_owned(),
            not_null: *not_null,
            primary_key: *primary_key,
        })
        .collect()
}

fn lookback_day(now_unix: i64) -> String {
    let moment = Timestamp::from_second(now_unix.saturating_sub(LIVE_LOOKBACK_SECONDS))
        .unwrap_or(Timestamp::MIN);
    Offset::UTC.to_datetime(moment).date().to_string()
}

async fn role_ids<'c>(
    executor: impl SqliteExecutor<'c>,
    event: i64,
) -> Result<Vec<i64>, AttendanceError> {
    Ok(sqlx::query_scalar(
        "SELECT role_id FROM cb_season_ping_roles WHERE season_id = ? ORDER BY position",
    )
    .bind(event)
    .fetch_all(executor)
    .await?)
}

async fn event_from<'c>(
    executor: impl SqliteExecutor<'c>,
    row: EventRow,
) -> Result<Event, AttendanceError> {
    let roles = role_ids(executor, row.0).await?;
    to_event(row, roles)
}

async fn name_holders(
    transaction: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    guild: i64,
    name: &str,
    except: Option<i64>,
) -> Result<NameHolders, AttendanceError> {
    let rows: Vec<EventRow> = sqlx::query_as(EVENTS_IN_GUILD)
        .bind(guild)
        .fetch_all(&mut **transaction)
        .await?;
    let mut read = Vec::with_capacity(rows.len());
    for row in rows.into_iter().filter(|row| Some(row.0) != except) {
        let namesake = same_name(&row.3, name);
        let roles = role_ids(&mut **transaction, row.0).await?;
        read.push((namesake, to_event(row, roles)));
    }
    let (namesakes, others): (Vec<_>, Vec<_>) =
        read.into_iter().partition(|(namesake, _)| *namesake);
    Ok(NameHolders {
        namesakes: namesakes
            .into_iter()
            .map(|(_, read)| read)
            .collect::<Result<Vec<Event>, AttendanceError>>()?,
        others: others.into_iter().map(|(_, read)| read).collect(),
    })
}

async fn end_finished(
    transaction: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    finished: &[Event],
    ended_at: i64,
) -> Result<(), AttendanceError> {
    for event in finished {
        sqlx::query("UPDATE cb_seasons SET ended_at_ms = ? WHERE id = ?")
            .bind(ended_at)
            .bind(event.id)
            .execute(&mut **transaction)
            .await?;
    }
    Ok(())
}

async fn insert_ping_roles(
    transaction: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    event: i64,
    roles: &[RoleId],
) -> Result<(), AttendanceError> {
    for (position, role) in (1_i64..).zip(roles) {
        sqlx::query(
            "INSERT INTO cb_season_ping_roles (season_id, position, role_id) VALUES (?, ?, ?)",
        )
        .bind(event)
        .bind(position)
        .bind(to_integer(role.get().into())?)
        .execute(&mut **transaction)
        .await?;
    }
    Ok(())
}

fn to_event(row: EventRow, roles: Vec<i64>) -> Result<Event, AttendanceError> {
    let (
        id,
        guild,
        channel,
        name,
        first_day,
        last_day,
        weekdays,
        start_minute,
        time_zone,
        hours,
        created_by,
        created_at_ms,
        ended_at_ms,
        posts_up,
    ) = row;
    let schedule = Schedule::new(
        Weekdays::from_bits(weekdays).ok_or(AttendanceError::BadWeekdays { value: weekdays })?,
        StartTime::from_minutes(start_minute).ok_or(AttendanceError::BadStart {
            value: start_minute,
        })?,
        Zone::find(&time_zone).ok_or(AttendanceError::BadZone { value: time_zone })?,
        Hours::new(hours).ok_or(AttendanceError::BadHours { value: hours })?,
    );
    let span = Span::new(to_day(&first_day)?, to_day(&last_day)?, schedule)
        .ok_or(AttendanceError::BadNight { value: last_day })?;
    Ok(Event {
        id,
        guild: GuildId::new(to_unsigned(guild)?),
        channel: ChannelId::new(to_unsigned(channel)?),
        name,
        span,
        created_by: to_player(created_by)?,
        created_at_ms: to_unsigned(created_at_ms)?,
        ping_roles: roles
            .into_iter()
            .map(to_role)
            .collect::<Result<Vec<RoleId>, AttendanceError>>()?,
        ended_at_ms: ended_at_ms.map(to_unsigned).transpose()?,
        posts_up: posts_up != 0,
    })
}

fn to_post(row: PostRow, schedule: &Schedule) -> Result<Post, AttendanceError> {
    let (date, message, state, posted_at_ms) = row;
    Ok(Post {
        session: to_session(&date, schedule)?,
        message: Snowflake::new(to_unsigned(message)?),
        state: PostState::from_text(&state).ok_or(AttendanceError::BadState { value: state })?,
        posted_at_ms: to_unsigned(posted_at_ms)?,
    })
}

fn to_mark(row: MarkRow) -> Result<Mark, AttendanceError> {
    let (user, hour, attending, answered_at_ms) = row;
    Ok(Mark {
        user: to_player(user)?,
        hour: u8::try_from(hour)
            .ok()
            .and_then(Hour::new)
            .ok_or(AttendanceError::BadHour { value: hour })?,
        attending: attending != 0,
        answered_at_ms: to_unsigned(answered_at_ms)?,
    })
}

fn to_session(value: &str, schedule: &Schedule) -> Result<Session, AttendanceError> {
    parse_day(value)
        .and_then(|date| schedule.session_at(date))
        .ok_or_else(|| AttendanceError::BadNight {
            value: value.to_owned(),
        })
}

fn to_day(value: &str) -> Result<Date, AttendanceError> {
    parse_day(value).ok_or_else(|| AttendanceError::BadNight {
        value: value.to_owned(),
    })
}

fn to_player(value: i64) -> Result<UserId, AttendanceError> {
    u64::try_from(value)
        .ok()
        .filter(|id| *id > 0)
        .map(UserId::new)
        .ok_or(AttendanceError::InvalidPlayer { value })
}

fn to_role(value: i64) -> Result<RoleId, AttendanceError> {
    u64::try_from(value)
        .ok()
        .filter(|id| *id > 0)
        .map(RoleId::new)
        .ok_or(AttendanceError::InvalidRole { value })
}

fn to_unsigned(value: i64) -> Result<u64, AttendanceError> {
    u64::try_from(value).map_err(|_| AttendanceError::Negative { value })
}

fn to_integer(value: u128) -> Result<i64, AttendanceError> {
    i64::try_from(value).map_err(|_| AttendanceError::OutOfRange { value })
}
