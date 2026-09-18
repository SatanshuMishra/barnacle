use std::path::Path;
use std::time::Duration;

use barnacle_guess::Snowflake;
use barnacle_guess::UserId;
use jiff::Timestamp;
use jiff::civil::Date;
use jiff::tz::Offset;
use sqlx::sqlite::SqliteConnectOptions;
use sqlx::sqlite::SqlitePool;
use sqlx::sqlite::SqlitePoolOptions;

use crate::ids::ChannelId;
use crate::ids::GuildId;
use crate::ids::RoleId;
use crate::schedule::Hour;
use crate::schedule::Night;
use crate::schedule::Range;
use crate::schedule::SECONDS_PER_HOUR;
use crate::schedule::parse_day;
use crate::solves::Column;

const BUSY_TIMEOUT: Duration = Duration::from_secs(5);
const LIVE_LOOKBACK_SECONDS: i64 = 48 * SECONDS_PER_HOUR;

const CB_SEASONS: &str = "cb_seasons";
const LIVE_NUMBER_INDEX: &str = "cb_seasons_live_number";
const CB_POSTS: &str = "cb_posts";
const CB_MARKS: &str = "cb_marks";

type ColumnSpec = (&'static str, &'static str, bool, bool);
type TableSpec = (&'static str, &'static [ColumnSpec]);

const EXPECTED_COLUMNS: [TableSpec; 3] = [
    (
        CB_SEASONS,
        &[
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
];

const SEASON_BY_ID: &str = "SELECT id, guild_id, channel_id, number, codename, first_day, last_day, created_by, created_at_ms, ping_role_id, ended_at_ms FROM cb_seasons WHERE id = ?";
const SEASON_LIVE_BY_NUMBER: &str = "SELECT id, guild_id, channel_id, number, codename, first_day, last_day, created_by, created_at_ms, ping_role_id, ended_at_ms FROM cb_seasons WHERE guild_id = ? AND number = ? AND ended_at_ms IS NULL";
const SEASONS_IN_GUILD: &str = "SELECT id, guild_id, channel_id, number, codename, first_day, last_day, created_by, created_at_ms, ping_role_id, ended_at_ms FROM cb_seasons WHERE guild_id = ? AND ended_at_ms IS NULL ORDER BY first_day";
const SEASONS_IN_GUILD_ANY_STATE: &str = "SELECT id, guild_id, channel_id, number, codename, first_day, last_day, created_by, created_at_ms, ping_role_id, ended_at_ms FROM cb_seasons WHERE guild_id = ? ORDER BY first_day";
const SEASONS_LIVE: &str = "SELECT id, guild_id, channel_id, number, codename, first_day, last_day, created_by, created_at_ms, ping_role_id, ended_at_ms, EXISTS (SELECT 1 FROM cb_posts WHERE cb_posts.season_id = cb_seasons.id AND cb_posts.state <> 'removed') FROM cb_seasons WHERE ended_at_ms IS NULL AND (last_day >= ? OR EXISTS (SELECT 1 FROM cb_posts WHERE cb_posts.season_id = cb_seasons.id AND cb_posts.state <> 'removed')) ORDER BY first_day";
const SEASON_OVERLAPPING: &str = "SELECT id, guild_id, channel_id, number, codename, first_day, last_day, created_by, created_at_ms, ping_role_id, ended_at_ms FROM cb_seasons WHERE guild_id = ? AND ended_at_ms IS NULL AND first_day <= ? AND last_day >= ? ORDER BY first_day LIMIT 1";
const SEASON_OVERLAPPING_OTHER: &str = "SELECT id, guild_id, channel_id, number, codename, first_day, last_day, created_by, created_at_ms, ping_role_id, ended_at_ms FROM cb_seasons WHERE guild_id = ? AND ended_at_ms IS NULL AND id <> ? AND first_day <= ? AND last_day >= ? ORDER BY first_day LIMIT 1";
const NUMBER_HELD_BY_ANOTHER: &str = "SELECT id FROM cb_seasons WHERE guild_id = ? AND number = ? AND id <> ? AND ended_at_ms IS NULL";

type SeasonRow = (
    i64,
    i64,
    i64,
    i64,
    Option<String>,
    String,
    String,
    i64,
    i64,
    Option<i64>,
    Option<i64>,
);

type LiveSeasonRow = (
    i64,
    i64,
    i64,
    i64,
    Option<String>,
    String,
    String,
    i64,
    i64,
    Option<i64>,
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
    #[error("{value} does not fit in a SQLite integer")]
    OutOfRange { value: u128 },
    #[error("the attendance database holds a negative value, {value}")]
    Negative { value: i64 },
    #[error("the attendance database holds a player ID below 1, {value}")]
    InvalidPlayer { value: i64 },
    #[error("the attendance database holds {value}, which is not a role ID")]
    InvalidRole { value: i64 },
    #[error("the attendance database holds {value}, which is not a CB night")]
    BadNight { value: String },
    #[error("the attendance database holds {value}, which is not a post state")]
    BadState { value: String },
    #[error("the attendance database holds {value}, which is not an hour")]
    BadHour { value: i64 },
    #[error("the attendance database holds {value}, which is not a season number")]
    BadSeasonNumber { value: i64 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Season {
    pub id: i64,
    pub guild: GuildId,
    pub channel: ChannelId,
    pub number: u32,
    pub codename: Option<String>,
    pub range: Range,
    pub created_by: UserId,
    pub created_at_ms: u64,
    pub ping_role: Option<RoleId>,
    pub ended_at_ms: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewSeason {
    pub guild: GuildId,
    pub channel: ChannelId,
    pub number: u32,
    pub codename: Option<String>,
    pub range: Range,
    pub created_by: UserId,
    pub created_at_ms: u64,
    pub ping_role: Option<RoleId>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SeasonChange {
    pub number: Option<u32>,
    pub first_day: Option<Date>,
    pub last_day: Option<Date>,
    pub codename: Option<Option<String>>,
    pub ping_role: Option<Option<RoleId>>,
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
    pub night: Night,
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
    Created(Season),
    NumberTaken,
    Overlaps(Season),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditOutcome {
    Edited { before: Season, after: Season },
    NumberTaken,
    Overlaps(Season),
    BadRange,
    TooFar,
    NotFound,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EndOutcome {
    Removed,
    Ended { season: Season },
    NotFound,
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
                .bind(LIVE_NUMBER_INDEX)
                .fetch_optional(&pool)
                .await?;
        if index.is_none() {
            return Err(AttendanceError::MissingIndex {
                index: LIVE_NUMBER_INDEX,
            });
        }
        Ok(Self { pool })
    }

    pub async fn create_season(&self, new: &NewSeason) -> Result<CreateOutcome, AttendanceError> {
        let guild = to_integer(new.guild.get().into())?;
        let channel = to_integer(new.channel.get().into())?;
        let created_by = to_integer(new.created_by.get().into())?;
        let created_at = to_integer(new.created_at_ms.into())?;
        let ping_role = to_role_id(new.ping_role)?;
        let number = i64::from(new.number);
        let first_day = new.range.first_day().to_string();
        let last_day = new.range.last_day().to_string();
        let mut transaction = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let taken: Option<i64> = sqlx::query_scalar(
            "SELECT id FROM cb_seasons WHERE guild_id = ? AND number = ? AND ended_at_ms IS NULL",
        )
        .bind(guild)
        .bind(number)
        .fetch_optional(&mut *transaction)
        .await?;
        if taken.is_some() {
            return Ok(CreateOutcome::NumberTaken);
        }
        let overlap: Option<SeasonRow> = sqlx::query_as(SEASON_OVERLAPPING)
            .bind(guild)
            .bind(&last_day)
            .bind(&first_day)
            .fetch_optional(&mut *transaction)
            .await?;
        if let Some(row) = overlap {
            return Ok(CreateOutcome::Overlaps(to_season(row)?));
        }
        let inserted = sqlx::query(
            "INSERT INTO cb_seasons (guild_id, channel_id, number, codename, first_day, last_day, created_by, created_at_ms, ping_role_id) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(guild)
        .bind(channel)
        .bind(number)
        .bind(new.codename.as_deref())
        .bind(&first_day)
        .bind(&last_day)
        .bind(created_by)
        .bind(created_at)
        .bind(ping_role)
        .execute(&mut *transaction)
        .await?;
        let id = inserted.last_insert_rowid();
        transaction.commit().await?;
        Ok(CreateOutcome::Created(Season {
            id,
            guild: new.guild,
            channel: new.channel,
            number: new.number,
            codename: new.codename.clone(),
            range: new.range,
            created_by: new.created_by,
            created_at_ms: new.created_at_ms,
            ping_role: new.ping_role,
            ended_at_ms: None,
        }))
    }

    pub async fn season(&self, id: i64) -> Result<Option<Season>, AttendanceError> {
        let row: Option<SeasonRow> = sqlx::query_as(SEASON_BY_ID)
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        row.map(to_season).transpose()
    }

    pub async fn live_season(
        &self,
        guild: GuildId,
        number: u32,
    ) -> Result<Option<Season>, AttendanceError> {
        let row: Option<SeasonRow> = sqlx::query_as(SEASON_LIVE_BY_NUMBER)
            .bind(to_integer(guild.get().into())?)
            .bind(i64::from(number))
            .fetch_optional(&self.pool)
            .await?;
        row.map(to_season).transpose()
    }

    pub async fn seasons_in(&self, guild: GuildId) -> Result<Vec<Season>, AttendanceError> {
        let rows: Vec<SeasonRow> = sqlx::query_as(SEASONS_IN_GUILD)
            .bind(to_integer(guild.get().into())?)
            .fetch_all(&self.pool)
            .await?;
        rows.into_iter().map(to_season).collect()
    }

    pub async fn seasons_in_any_state(
        &self,
        guild: GuildId,
    ) -> Result<Vec<Season>, AttendanceError> {
        let rows: Vec<SeasonRow> = sqlx::query_as(SEASONS_IN_GUILD_ANY_STATE)
            .bind(to_integer(guild.get().into())?)
            .fetch_all(&self.pool)
            .await?;
        rows.into_iter().map(to_season).collect()
    }

    pub async fn live_seasons(&self, now_unix: i64) -> Result<Vec<Season>, AttendanceError> {
        let rows: Vec<LiveSeasonRow> = sqlx::query_as(SEASONS_LIVE)
            .bind(lookback_day(now_unix))
            .fetch_all(&self.pool)
            .await?;
        rows.into_iter()
            .filter_map(|row| {
                let (
                    id,
                    guild,
                    channel,
                    number,
                    codename,
                    first_day,
                    last_day,
                    created_by,
                    created_at_ms,
                    ping_role_id,
                    ended_at_ms,
                    unremoved_post,
                ) = row;
                let season = to_season((
                    id,
                    guild,
                    channel,
                    number,
                    codename,
                    first_day,
                    last_day,
                    created_by,
                    created_at_ms,
                    ping_role_id,
                    ended_at_ms,
                ));
                match season {
                    Ok(season) => {
                        let nights_ahead = season
                            .range
                            .last_moment_unix()
                            .is_some_and(|moment| moment > now_unix);
                        (nights_ahead || unremoved_post != 0).then_some(Ok(season))
                    }
                    Err(error) => Some(Err(error)),
                }
            })
            .collect()
    }

    pub async fn edit_season(
        &self,
        guild: GuildId,
        number: u32,
        change: &SeasonChange,
        today: Date,
    ) -> Result<EditOutcome, AttendanceError> {
        let guild_id = to_integer(guild.get().into())?;
        let mut transaction = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let found: Option<SeasonRow> = sqlx::query_as(SEASON_LIVE_BY_NUMBER)
            .bind(guild_id)
            .bind(i64::from(number))
            .fetch_optional(&mut *transaction)
            .await?;
        let Some(row) = found else {
            return Ok(EditOutcome::NotFound);
        };
        let before = to_season(row)?;
        let Some(range) = Range::new(
            change.first_day.unwrap_or(before.range.first_day()),
            change.last_day.unwrap_or(before.range.last_day()),
        ) else {
            return Ok(EditOutcome::BadRange);
        };
        if !range.near(today) {
            return Ok(EditOutcome::TooFar);
        }
        let after = Season {
            id: before.id,
            guild: before.guild,
            channel: before.channel,
            number: change.number.unwrap_or(before.number),
            codename: change
                .codename
                .clone()
                .unwrap_or_else(|| before.codename.clone()),
            range,
            created_by: before.created_by,
            created_at_ms: before.created_at_ms,
            ping_role: change.ping_role.unwrap_or(before.ping_role),
            ended_at_ms: before.ended_at_ms,
        };
        let first_day = after.range.first_day().to_string();
        let last_day = after.range.last_day().to_string();
        let taken: Option<i64> = sqlx::query_scalar(NUMBER_HELD_BY_ANOTHER)
            .bind(guild_id)
            .bind(i64::from(after.number))
            .bind(before.id)
            .fetch_optional(&mut *transaction)
            .await?;
        if taken.is_some() {
            return Ok(EditOutcome::NumberTaken);
        }
        let overlap: Option<SeasonRow> = sqlx::query_as(SEASON_OVERLAPPING_OTHER)
            .bind(guild_id)
            .bind(before.id)
            .bind(&last_day)
            .bind(&first_day)
            .fetch_optional(&mut *transaction)
            .await?;
        if let Some(row) = overlap {
            return Ok(EditOutcome::Overlaps(to_season(row)?));
        }
        sqlx::query(
            "UPDATE cb_seasons SET number = ?, codename = ?, first_day = ?, last_day = ?, ping_role_id = ? WHERE id = ?",
        )
        .bind(i64::from(after.number))
        .bind(after.codename.as_deref())
        .bind(&first_day)
        .bind(&last_day)
        .bind(to_role_id(after.ping_role)?)
        .bind(after.id)
        .execute(&mut *transaction)
        .await?;
        transaction.commit().await?;
        Ok(EditOutcome::Edited { before, after })
    }

    pub async fn move_season(
        &self,
        guild: GuildId,
        id: i64,
        channel: ChannelId,
    ) -> Result<bool, AttendanceError> {
        let changed = sqlx::query(
            "UPDATE cb_seasons SET channel_id = ? WHERE id = ? AND guild_id = ? AND ended_at_ms IS NULL",
        )
        .bind(to_integer(channel.get().into())?)
        .bind(id)
        .bind(to_integer(guild.get().into())?)
        .execute(&self.pool)
        .await?
        .rows_affected();
        Ok(changed > 0)
    }

    pub async fn end_season(
        &self,
        guild: GuildId,
        number: u32,
        now_ms: u64,
    ) -> Result<EndOutcome, AttendanceError> {
        let ended_at = to_integer(now_ms.into())?;
        let mut transaction = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let found: Option<SeasonRow> = sqlx::query_as(SEASON_LIVE_BY_NUMBER)
            .bind(to_integer(guild.get().into())?)
            .bind(i64::from(number))
            .fetch_optional(&mut *transaction)
            .await?;
        let Some(row) = found else {
            return Ok(EndOutcome::NotFound);
        };
        let season = to_season(row)?;
        let posted: Option<i64> =
            sqlx::query_scalar("SELECT 1 FROM cb_posts WHERE season_id = ? LIMIT 1")
                .bind(season.id)
                .fetch_optional(&mut *transaction)
                .await?;
        let outcome = if posted.is_none() {
            sqlx::query("DELETE FROM cb_seasons WHERE id = ?")
                .bind(season.id)
                .execute(&mut *transaction)
                .await?;
            EndOutcome::Removed
        } else {
            sqlx::query("UPDATE cb_seasons SET ended_at_ms = ? WHERE id = ?")
                .bind(ended_at)
                .bind(season.id)
                .execute(&mut *transaction)
                .await?;
            EndOutcome::Ended { season }
        };
        transaction.commit().await?;
        Ok(outcome)
    }

    pub async fn purge_season(&self, guild: GuildId, id: i64) -> Result<u64, AttendanceError> {
        let guild_id = to_integer(guild.get().into())?;
        let mut transaction = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let held: Option<i64> =
            sqlx::query_scalar("SELECT id FROM cb_seasons WHERE id = ? AND guild_id = ?")
                .bind(id)
                .bind(guild_id)
                .fetch_optional(&mut *transaction)
                .await?;
        if held.is_none() {
            return Ok(0);
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
        sqlx::query("DELETE FROM cb_seasons WHERE id = ? AND guild_id = ?")
            .bind(id)
            .bind(guild_id)
            .execute(&mut *transaction)
            .await?;
        transaction.commit().await?;
        Ok(marks)
    }

    pub async fn post(&self, season: i64, night: Night) -> Result<Option<Post>, AttendanceError> {
        let row: Option<PostRow> = sqlx::query_as(
            "SELECT night, message_id, state, posted_at_ms FROM cb_posts WHERE season_id = ? AND night = ?",
        )
        .bind(season)
        .bind(night.label())
        .fetch_optional(&self.pool)
        .await?;
        row.map(to_post).transpose()
    }

    pub async fn posts(&self, season: i64) -> Result<Vec<Post>, AttendanceError> {
        let rows: Vec<PostRow> = sqlx::query_as(
            "SELECT night, message_id, state, posted_at_ms FROM cb_posts WHERE season_id = ? ORDER BY night",
        )
        .bind(season)
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter().map(to_post).collect()
    }

    pub async fn record_post(
        &self,
        season: i64,
        night: Night,
        message: Snowflake,
        at_ms: u64,
    ) -> Result<(), AttendanceError> {
        sqlx::query(
            "INSERT INTO cb_posts (season_id, night, message_id, state, posted_at_ms) VALUES (?, ?, ?, ?, ?) ON CONFLICT (season_id, night) DO UPDATE SET message_id = excluded.message_id, state = 'open', posted_at_ms = excluded.posted_at_ms",
        )
        .bind(season)
        .bind(night.label())
        .bind(to_integer(message.get().into())?)
        .bind(PostState::Open.as_text())
        .bind(to_integer(at_ms.into())?)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn set_post_state(
        &self,
        season: i64,
        night: Night,
        state: PostState,
    ) -> Result<(), AttendanceError> {
        sqlx::query("UPDATE cb_posts SET state = ? WHERE season_id = ? AND night = ?")
            .bind(state.as_text())
            .bind(season)
            .bind(night.label())
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn mark(
        &self,
        season: i64,
        night: Night,
        user: UserId,
        hours: &[Hour],
        attending: bool,
        at_ms: u64,
    ) -> Result<(), AttendanceError> {
        let label = night.label();
        let user = to_integer(user.get().into())?;
        let at = to_integer(at_ms.into())?;
        let mut transaction = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        for hour in hours {
            sqlx::query(
                "INSERT INTO cb_marks (season_id, night, user_id, hour, attending, answered_at_ms, changed_at_ms) VALUES (?, ?, ?, ?, ?, ?, ?) ON CONFLICT (season_id, night, user_id, hour) DO UPDATE SET attending = excluded.attending, changed_at_ms = excluded.changed_at_ms",
            )
            .bind(season)
            .bind(&label)
            .bind(user)
            .bind(i64::from(hour.get()))
            .bind(i64::from(attending))
            .bind(at)
            .bind(at)
            .execute(&mut *transaction)
            .await?;
        }
        transaction.commit().await?;
        Ok(())
    }

    pub async fn roster(&self, season: i64, night: Night) -> Result<Vec<Mark>, AttendanceError> {
        let rows: Vec<MarkRow> = sqlx::query_as(
            "SELECT user_id, hour, attending, answered_at_ms FROM cb_marks WHERE season_id = ? AND night = ? ORDER BY answered_at_ms, user_id, hour",
        )
        .bind(season)
        .bind(night.label())
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

fn to_season(row: SeasonRow) -> Result<Season, AttendanceError> {
    let (
        id,
        guild,
        channel,
        number,
        codename,
        first_day,
        last_day,
        created_by,
        created_at_ms,
        ping_role_id,
        ended_at_ms,
    ) = row;
    let range = Range::new(to_day(&first_day)?, to_day(&last_day)?)
        .ok_or(AttendanceError::BadNight { value: last_day })?;
    Ok(Season {
        id,
        guild: GuildId::new(to_unsigned(guild)?),
        channel: ChannelId::new(to_unsigned(channel)?),
        number: to_number(number)?,
        codename,
        range,
        created_by: to_player(created_by)?,
        created_at_ms: to_unsigned(created_at_ms)?,
        ping_role: ping_role_id.map(to_role).transpose()?,
        ended_at_ms: ended_at_ms.map(to_unsigned).transpose()?,
    })
}

fn to_post(row: PostRow) -> Result<Post, AttendanceError> {
    let (night, message, state, posted_at_ms) = row;
    Ok(Post {
        night: to_night(&night)?,
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

fn to_night(value: &str) -> Result<Night, AttendanceError> {
    Night::parse(value).ok_or_else(|| AttendanceError::BadNight {
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

fn to_role_id(role: Option<RoleId>) -> Result<Option<i64>, AttendanceError> {
    role.map(|role| to_integer(role.get().into())).transpose()
}

fn to_number(value: i64) -> Result<u32, AttendanceError> {
    u32::try_from(value).map_err(|_| AttendanceError::BadSeasonNumber { value })
}

fn to_unsigned(value: i64) -> Result<u64, AttendanceError> {
    u64::try_from(value).map_err(|_| AttendanceError::Negative { value })
}

fn to_integer(value: u128) -> Result<i64, AttendanceError> {
    i64::try_from(value).map_err(|_| AttendanceError::OutOfRange { value })
}
