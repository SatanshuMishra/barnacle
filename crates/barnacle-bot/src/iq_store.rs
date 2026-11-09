use std::path::Path;
use std::time::Duration;

use barnacle_guess::UserId;
use jiff::civil::Date;
use sqlx::Row;
use sqlx::Sqlite;
use sqlx::Transaction;
use sqlx::sqlite::SqliteConnectOptions;
use sqlx::sqlite::SqlitePool;
use sqlx::sqlite::SqlitePoolOptions;
use sqlx::sqlite::SqliteRow;

use crate::ids::ChannelId;
use crate::ids::GuildId;
use crate::ids::RoleId;
use crate::iq;
use crate::iq::Criterion;
use crate::iq::CriterionKind;
use crate::iq::JudgingMarks;
use crate::iq::Mark;
use crate::iq::Score;
use crate::iq::Status;
use crate::solves::Column;

const BUSY_TIMEOUT: Duration = Duration::from_secs(5);
const DISCORD_EPOCH_MS: i64 = 1_420_070_400_000;
const SNOWFLAKE_TIME_SHIFT: u32 = 22;
const SNOWFLAKE_TIME_LIMIT: u64 = 1 << 41;
const TEMPLATE_LIMIT: i64 = 10;

macro_rules! player_points {
    () => {
        "SELECT user_id, points FROM iq_submissions WHERE competition_id = ? AND awarded_by IS NOT NULL UNION ALL SELECT p.user_id, s.points FROM iq_submission_partners p JOIN iq_submissions s ON s.message_id = p.message_id WHERE s.competition_id = ? AND s.awarded_by IS NOT NULL UNION ALL SELECT user_id, points FROM iq_adjustments WHERE competition_id = ?"
    };
}
pub const REASON_LIMIT: usize = 200;

const COMPETITIONS: &str = "iq_competitions";
const CRITERIA: &str = "iq_criteria";
const TARGET_ROLES: &str = "iq_target_roles";
const JUDGES: &str = "iq_judges";
const ANNOUNCEMENTS: &str = "iq_announcements";
const SUBMISSIONS: &str = "iq_submissions";
const ADJUSTMENTS: &str = "iq_adjustments";
const SERVERS: &str = "iq_servers";
const PARTNERS: &str = "iq_submission_partners";
const REJECTIONS: &str = "iq_rejections";
const BOARD_POSTS: &str = "iq_board_posts";
const KIND_COLUMN: &str = "kind";
const PLAYERS_TABLES: [&str; 3] = [PARTNERS, REJECTIONS, BOARD_POSTS];
const LIVE_INDEX: &str = "iq_competitions_live";
const CRITERION_NAME_INDEX: &str = "iq_criteria_name";
const UNAWARDED_INDEX: &str = "iq_submissions_unawarded";
const PRE_MARKS_COLUMN: &str = "judging_reaction";

type ColumnSpec = (&'static str, &'static str, bool, bool);
type TableSpec = (&'static str, &'static [ColumnSpec]);

const EXPECTED_COLUMNS: [TableSpec; 11] = [
    (
        COMPETITIONS,
        &[
            ("id", "INTEGER", false, true),
            ("guild_id", "INTEGER", true, false),
            ("name", "TEXT", true, false),
            ("first_day", "TEXT", true, false),
            ("last_day", "TEXT", true, false),
            ("time_zone", "TEXT", true, false),
            ("status", "TEXT", true, false),
            ("category_id", "INTEGER", true, false),
            ("announcements_id", "INTEGER", true, false),
            ("submissions_id", "INTEGER", true, false),
            ("judge_log_channel_id", "INTEGER", false, false),
            ("denied_mark", "TEXT", true, false),
            ("awarded_mark", "TEXT", true, false),
            ("confirm_mark", "TEXT", true, false),
            ("rules", "TEXT", true, false),
            ("last_seen_message", "INTEGER", false, false),
            ("created_by", "INTEGER", true, false),
            ("created_at_ms", "INTEGER", true, false),
        ],
    ),
    (
        CRITERIA,
        &[
            ("competition_id", "INTEGER", true, true),
            ("position", "INTEGER", true, true),
            ("name", "TEXT", true, false),
            ("description", "TEXT", true, false),
            ("points", "INTEGER", true, false),
            ("kind", "TEXT", true, false),
        ],
    ),
    (
        TARGET_ROLES,
        &[
            ("competition_id", "INTEGER", true, true),
            ("role_id", "INTEGER", true, true),
        ],
    ),
    (
        JUDGES,
        &[
            ("competition_id", "INTEGER", true, true),
            ("user_id", "INTEGER", true, true),
        ],
    ),
    (
        ANNOUNCEMENTS,
        &[
            ("competition_id", "INTEGER", true, true),
            ("position", "INTEGER", true, true),
            ("message_id", "INTEGER", true, false),
        ],
    ),
    (
        SUBMISSIONS,
        &[
            ("message_id", "INTEGER", false, true),
            ("competition_id", "INTEGER", true, false),
            ("user_id", "INTEGER", true, false),
            ("criterion", "TEXT", true, false),
            ("points", "INTEGER", true, false),
            ("posted_at_ms", "INTEGER", true, false),
            ("awarded_by", "INTEGER", false, false),
            ("awarded_at_ms", "INTEGER", false, false),
        ],
    ),
    (
        ADJUSTMENTS,
        &[
            ("id", "INTEGER", false, true),
            ("competition_id", "INTEGER", true, false),
            ("user_id", "INTEGER", true, false),
            ("points", "INTEGER", true, false),
            ("reason", "TEXT", false, false),
            ("judge_id", "INTEGER", true, false),
            ("made_at_ms", "INTEGER", true, false),
        ],
    ),
    (
        SERVERS,
        &[
            ("guild_id", "INTEGER", false, true),
            ("judge_role_id", "INTEGER", true, false),
        ],
    ),
    (
        PARTNERS,
        &[
            ("message_id", "INTEGER", true, true),
            ("user_id", "INTEGER", true, true),
        ],
    ),
    (
        REJECTIONS,
        &[
            ("message_id", "INTEGER", false, true),
            ("competition_id", "INTEGER", true, false),
            ("user_id", "INTEGER", true, false),
            ("reason", "TEXT", true, false),
            ("rejected_by", "INTEGER", true, false),
            ("rejected_at_ms", "INTEGER", true, false),
        ],
    ),
    (
        BOARD_POSTS,
        &[
            ("competition_id", "INTEGER", false, true),
            ("mark", "INTEGER", true, false),
            ("digest", "TEXT", true, false),
            ("message_id", "INTEGER", false, false),
        ],
    ),
];

const EXPECTED_INDEXES: [&str; 2] = [LIVE_INDEX, CRITERION_NAME_INDEX];

const LIVE_IN_GUILD: &str = "SELECT id, guild_id, name, first_day, last_day, time_zone, status, category_id, announcements_id, submissions_id, judge_log_channel_id, denied_mark, awarded_mark, confirm_mark, rules, last_seen_message, created_by, created_at_ms FROM iq_competitions WHERE guild_id = ? AND status <> 'archived' ORDER BY id DESC LIMIT 1";
const LIVE_BY_SUBMISSIONS: &str = "SELECT id, guild_id, name, first_day, last_day, time_zone, status, category_id, announcements_id, submissions_id, judge_log_channel_id, denied_mark, awarded_mark, confirm_mark, rules, last_seen_message, created_by, created_at_ms FROM iq_competitions WHERE submissions_id = ? AND status <> 'archived' ORDER BY id DESC LIMIT 1";
const BY_ID: &str = "SELECT id, guild_id, name, first_day, last_day, time_zone, status, category_id, announcements_id, submissions_id, judge_log_channel_id, denied_mark, awarded_mark, confirm_mark, rules, last_seen_message, created_by, created_at_ms FROM iq_competitions WHERE id = ?";
const LAST_ARCHIVED: &str = "SELECT id, guild_id, name, first_day, last_day, time_zone, status, category_id, announcements_id, submissions_id, judge_log_channel_id, denied_mark, awarded_mark, confirm_mark, rules, last_seen_message, created_by, created_at_ms FROM iq_competitions WHERE guild_id = ? AND status = 'archived' ORDER BY created_at_ms DESC, id DESC LIMIT 1";
const NEWEST_IN_GUILD: &str = "SELECT id, guild_id, name, first_day, last_day, time_zone, status, category_id, announcements_id, submissions_id, judge_log_channel_id, denied_mark, awarded_mark, confirm_mark, rules, last_seen_message, created_by, created_at_ms FROM iq_competitions WHERE guild_id = ? ORDER BY created_at_ms DESC, id DESC LIMIT ?";

struct CompetitionRow {
    id: i64,
    guild: i64,
    name: String,
    first_day: String,
    last_day: String,
    zone: String,
    status: String,
    category: i64,
    announcements: i64,
    submissions: i64,
    judge_log: Option<i64>,
    denied: String,
    awarded: String,
    confirm: String,
    rules: String,
    last_seen: Option<i64>,
    created_by: i64,
    created_at_ms: i64,
}

impl CompetitionRow {
    fn read(row: &SqliteRow) -> Result<CompetitionRow, sqlx::Error> {
        Ok(CompetitionRow {
            id: row.try_get("id")?,
            guild: row.try_get("guild_id")?,
            name: row.try_get("name")?,
            first_day: row.try_get("first_day")?,
            last_day: row.try_get("last_day")?,
            zone: row.try_get("time_zone")?,
            status: row.try_get("status")?,
            category: row.try_get("category_id")?,
            announcements: row.try_get("announcements_id")?,
            submissions: row.try_get("submissions_id")?,
            judge_log: row.try_get("judge_log_channel_id")?,
            denied: row.try_get("denied_mark")?,
            awarded: row.try_get("awarded_mark")?,
            confirm: row.try_get("confirm_mark")?,
            rules: row.try_get("rules")?,
            last_seen: row.try_get("last_seen_message")?,
            created_by: row.try_get("created_by")?,
            created_at_ms: row.try_get("created_at_ms")?,
        })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum IqStoreError {
    #[error("the IQ Competition database could not be used")]
    Database(#[from] sqlx::Error),
    #[error("the IQ Competition database has no {table} table")]
    MissingTable { table: &'static str },
    #[error("{table} does not have the expected columns; found {found:?}")]
    UnexpectedColumns {
        table: &'static str,
        found: Vec<Column>,
    },
    #[error("the IQ Competition database has no {index} index")]
    MissingIndex { index: &'static str },
    #[error("the IQ Competition database has no judging marks or judge log")]
    BeforeMarks,
    #[error(
        "the IQ Competition database has no multi-player criteria, partners, rejections or leaderboard posts"
    )]
    BeforePlayers,
    #[error("{value} does not fit in a SQLite integer")]
    OutOfRange { value: u128 },
    #[error("the IQ Competition database holds a negative value, {value}")]
    Negative { value: i64 },
    #[error("the IQ Competition database holds a user ID below 1, {value}")]
    InvalidUser { value: i64 },
    #[error("the IQ Competition database holds {value}, which is not a date")]
    BadDate { value: String },
    #[error("the IQ Competition database holds {value}, which is not a competition status")]
    BadStatus { value: String },
    #[error("the IQ Competition database holds {value}, which is not a judging mark")]
    BadMark { value: String },
    #[error("the IQ Competition database holds a competition whose judging marks repeat")]
    RepeatedMarks,
    #[error("the IQ Competition database holds {value}, which is not a criterion's points")]
    BadPoints { value: i64 },
    #[error("the IQ Competition database holds {value}, which is not a criterion type")]
    BadCriterionKind { value: String },
    #[error("{points} is not a points change between -1000 and 1000 other than 0")]
    BadAdjustment { points: i64 },
    #[error("a points change or rejection has no reason")]
    ReasonMissing,
    #[error("a points change or rejection reason is longer than {REASON_LIMIT} characters")]
    ReasonTooLong,
    #[error("{at_unix} is outside the moments a Discord message ID can hold")]
    NotASnowflakeMoment { at_unix: i64 },
    #[error("there is no IQ Competition with ID {id}")]
    UnknownCompetition { id: i64 },
    #[error("IQ Competition {id} is completed or archived, so it cannot resume")]
    NotResumable { id: i64 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigChange {
    pub competition: i64,
    pub criteria: Vec<Criterion>,
    pub rules: String,
    pub roles: Vec<RoleId>,
    pub judges: Vec<UserId>,
    pub marks: JudgingMarks,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewCompetition {
    pub guild: GuildId,
    pub name: String,
    pub first_day: Date,
    pub last_day: Date,
    pub zone: String,
    pub category: ChannelId,
    pub announcements: ChannelId,
    pub submissions: ChannelId,
    pub criteria: Vec<Criterion>,
    pub rules: String,
    pub roles: Vec<RoleId>,
    pub judges: Vec<UserId>,
    pub marks: JudgingMarks,
    pub judge_log: Option<ChannelId>,
    pub created_by: UserId,
    pub created_at_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Competition {
    pub id: i64,
    pub status: Status,
    pub last_seen: Option<u64>,
    pub guild: GuildId,
    pub name: String,
    pub first_day: Date,
    pub last_day: Date,
    pub zone: String,
    pub category: ChannelId,
    pub announcements: ChannelId,
    pub submissions: ChannelId,
    pub criteria: Vec<Criterion>,
    pub rules: String,
    pub roles: Vec<RoleId>,
    pub judges: Vec<UserId>,
    pub marks: JudgingMarks,
    pub judge_log: Option<ChannelId>,
    pub created_by: UserId,
    pub created_at_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(clippy::large_enum_variant)]
pub enum CreateOutcome {
    Created(Competition),
    AlreadyLive,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdjustOutcome {
    Adjusted { total: i64 },
    BelowZero { total: i64 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AwardOutcome {
    Awarded { players: Vec<UserId>, points: u32 },
    AlreadyAwarded,
    NotASubmission,
    Closed,
    Rejected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RejectOutcome {
    Rejected,
    AlreadyRejected,
    AlreadyAwarded,
}

#[derive(Debug, Clone)]
pub struct IqStore {
    pool: SqlitePool,
}

impl IqStore {
    pub async fn open(path: &Path) -> Result<Self, IqStoreError> {
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

    pub async fn with_pool(pool: SqlitePool) -> Result<Self, IqStoreError> {
        let mut before_marks = false;
        let mut before_players = false;
        for (table, expected) in EXPECTED_COLUMNS {
            let found = columns(&pool, table).await?;
            if found.is_empty() && PLAYERS_TABLES.contains(&table) {
                before_players = true;
                continue;
            }
            if found.is_empty() {
                return Err(IqStoreError::MissingTable { table });
            }
            if found.iter().any(|column| column.name == PRE_MARKS_COLUMN) {
                before_marks = true;
                continue;
            }
            let wanted = expected_columns(expected);
            if table == CRITERIA && found == without_kind(&wanted) {
                before_players = true;
                continue;
            }
            if found != wanted {
                return Err(IqStoreError::UnexpectedColumns { table, found });
            }
        }
        for index in EXPECTED_INDEXES {
            let present: Option<i64> =
                sqlx::query_scalar("SELECT 1 FROM sqlite_master WHERE type = 'index' AND name = ?")
                    .bind(index)
                    .fetch_optional(&pool)
                    .await?;
            if present.is_none() {
                return Err(IqStoreError::MissingIndex { index });
            }
        }
        let unawarded: Option<i64> =
            sqlx::query_scalar("SELECT 1 FROM sqlite_master WHERE type = 'index' AND name = ?")
                .bind(UNAWARDED_INDEX)
                .fetch_optional(&pool)
                .await?;
        if before_marks || unawarded.is_none() {
            return Err(IqStoreError::BeforeMarks);
        }
        if before_players {
            return Err(IqStoreError::BeforePlayers);
        }
        Ok(Self { pool })
    }

    pub async fn create(&self, new: &NewCompetition) -> Result<CreateOutcome, IqStoreError> {
        let guild = to_integer(new.guild.get().into())?;
        let mut transaction = self.pool.begin().await?;
        let live: Option<i64> = sqlx::query_scalar(
            "SELECT id FROM iq_competitions WHERE guild_id = ? AND status <> 'archived'",
        )
        .bind(guild)
        .fetch_optional(&mut *transaction)
        .await?;
        if live.is_some() {
            transaction.rollback().await?;
            return Ok(CreateOutcome::AlreadyLive);
        }
        let id: i64 = sqlx::query_scalar(
            "INSERT INTO iq_competitions (guild_id, name, first_day, last_day, time_zone, status, category_id, announcements_id, submissions_id, judge_log_channel_id, denied_mark, awarded_mark, confirm_mark, rules, last_seen_message, created_by, created_at_ms) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL, ?, ?) RETURNING id",
        )
        .bind(guild)
        .bind(&new.name)
        .bind(new.first_day.to_string())
        .bind(new.last_day.to_string())
        .bind(&new.zone)
        .bind(status_text(Status::Running))
        .bind(to_integer(new.category.get().into())?)
        .bind(to_integer(new.announcements.get().into())?)
        .bind(to_integer(new.submissions.get().into())?)
        .bind(
            new.judge_log
                .map(|channel| to_integer(channel.get().into()))
                .transpose()?,
        )
        .bind(new.marks.denied.text())
        .bind(new.marks.awarded.text())
        .bind(new.marks.confirm.text())
        .bind(&new.rules)
        .bind(to_integer(new.created_by.get().into())?)
        .bind(new.created_at_ms)
        .fetch_one(&mut *transaction)
        .await?;
        insert_config(&mut transaction, id, &new.criteria, &new.roles, &new.judges).await?;
        transaction.commit().await?;
        let created = self
            .competition(id)
            .await?
            .ok_or(IqStoreError::UnknownCompetition { id })?;
        Ok(CreateOutcome::Created(created))
    }

    pub async fn live(&self, guild: GuildId) -> Result<Option<Competition>, IqStoreError> {
        let row = sqlx::query(LIVE_IN_GUILD)
            .bind(to_integer(guild.get().into())?)
            .fetch_optional(&self.pool)
            .await?;
        self.complete(row).await
    }

    pub async fn by_submissions_channel(
        &self,
        channel: ChannelId,
    ) -> Result<Option<Competition>, IqStoreError> {
        let row = sqlx::query(LIVE_BY_SUBMISSIONS)
            .bind(to_integer(channel.get().into())?)
            .fetch_optional(&self.pool)
            .await?;
        self.complete(row).await
    }

    pub async fn competition(&self, id: i64) -> Result<Option<Competition>, IqStoreError> {
        let row = sqlx::query(BY_ID)
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        self.complete(row).await
    }

    pub async fn revert_completed(&self, id: i64, to: Status) -> Result<bool, IqStoreError> {
        let changed = sqlx::query(
            "UPDATE iq_competitions SET status = ? WHERE id = ? AND status = 'completed'",
        )
        .bind(status_text(to))
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(changed.rows_affected() == 1)
    }

    pub async fn set_status(&self, id: i64, status: Status) -> Result<(), IqStoreError> {
        let changed = sqlx::query("UPDATE iq_competitions SET status = ? WHERE id = ?")
            .bind(status_text(status))
            .bind(id)
            .execute(&self.pool)
            .await?;
        expect_one(changed.rows_affected(), id)
    }

    pub async fn resume(&self, id: i64, at_unix: i64) -> Result<(), IqStoreError> {
        let mark = to_integer(snowflake_at(at_unix)?.into())?;
        let changed = sqlx::query(
            "UPDATE iq_competitions SET status = 'running', last_seen_message = max(coalesce(last_seen_message, ?), ?) WHERE id = ? AND status IN ('running', 'stopped')",
        )
        .bind(mark)
        .bind(mark)
        .bind(id)
        .execute(&self.pool)
        .await?;
        if changed.rows_affected() == 1 {
            return Ok(());
        }
        match self.competition(id).await? {
            Some(_) => Err(IqStoreError::NotResumable { id }),
            None => Err(IqStoreError::UnknownCompetition { id }),
        }
    }

    pub async fn mark_seen(&self, id: i64, message: u64) -> Result<(), IqStoreError> {
        let message = to_integer(message.into())?;
        let changed = sqlx::query(
            "UPDATE iq_competitions SET last_seen_message = max(coalesce(last_seen_message, ?), ?) WHERE id = ?",
        )
        .bind(message)
        .bind(message)
        .bind(id)
        .execute(&self.pool)
        .await?;
        expect_one(changed.rows_affected(), id)
    }

    pub async fn last_seen(&self, id: i64) -> Result<Option<u64>, IqStoreError> {
        let row: Option<Option<i64>> =
            sqlx::query_scalar("SELECT last_seen_message FROM iq_competitions WHERE id = ?")
                .bind(id)
                .fetch_optional(&self.pool)
                .await?;
        row.ok_or(IqStoreError::UnknownCompetition { id })?
            .map(to_unsigned)
            .transpose()
    }

    pub async fn set_judge_log(&self, id: i64, channel: ChannelId) -> Result<(), IqStoreError> {
        let changed =
            sqlx::query("UPDATE iq_competitions SET judge_log_channel_id = ? WHERE id = ?")
                .bind(to_integer(channel.get().into())?)
                .bind(id)
                .execute(&self.pool)
                .await?;
        expect_one(changed.rows_affected(), id)
    }

    pub async fn update_config(&self, change: &ConfigChange) -> Result<(), IqStoreError> {
        let ConfigChange {
            competition: id,
            criteria,
            rules,
            roles,
            judges,
            marks,
        } = change;
        let id = *id;
        let mut transaction = self.pool.begin().await?;
        let changed = sqlx::query(
            "UPDATE iq_competitions SET rules = ?, denied_mark = ?, awarded_mark = ?, confirm_mark = ? WHERE id = ?",
        )
        .bind(rules)
        .bind(marks.denied.text())
        .bind(marks.awarded.text())
        .bind(marks.confirm.text())
        .bind(id)
        .execute(&mut *transaction)
        .await?;
        if changed.rows_affected() != 1 {
            transaction.rollback().await?;
            return Err(IqStoreError::UnknownCompetition { id });
        }
        for statement in [
            "DELETE FROM iq_criteria WHERE competition_id = ?",
            "DELETE FROM iq_target_roles WHERE competition_id = ?",
            "DELETE FROM iq_judges WHERE competition_id = ?",
        ] {
            sqlx::query(statement)
                .bind(id)
                .execute(&mut *transaction)
                .await?;
        }
        insert_config(&mut transaction, id, criteria, roles, judges).await?;
        transaction.commit().await?;
        Ok(())
    }

    pub async fn set_announcements(&self, id: i64, messages: &[u64]) -> Result<(), IqStoreError> {
        let messages: Vec<i64> = messages
            .iter()
            .map(|message| to_integer((*message).into()))
            .collect::<Result<_, _>>()?;
        let mut transaction = self.pool.begin().await?;
        let exists: Option<i64> = sqlx::query_scalar("SELECT id FROM iq_competitions WHERE id = ?")
            .bind(id)
            .fetch_optional(&mut *transaction)
            .await?;
        if exists.is_none() {
            transaction.rollback().await?;
            return Err(IqStoreError::UnknownCompetition { id });
        }
        sqlx::query("DELETE FROM iq_announcements WHERE competition_id = ?")
            .bind(id)
            .execute(&mut *transaction)
            .await?;
        for (position, message) in (0_i64..).zip(messages) {
            sqlx::query(
                "INSERT INTO iq_announcements (competition_id, position, message_id) VALUES (?, ?, ?)",
            )
            .bind(id)
            .bind(position)
            .bind(message)
            .execute(&mut *transaction)
            .await?;
        }
        transaction.commit().await?;
        Ok(())
    }

    pub async fn announcements(&self, id: i64) -> Result<Vec<u64>, IqStoreError> {
        let rows: Vec<i64> = sqlx::query_scalar(
            "SELECT message_id FROM iq_announcements WHERE competition_id = ? ORDER BY position",
        )
        .bind(id)
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter().map(to_unsigned).collect()
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn record_submission(
        &self,
        id: i64,
        message: u64,
        user: UserId,
        partners: &[UserId],
        criterion: &str,
        points: u32,
        at_ms: i64,
    ) -> Result<bool, IqStoreError> {
        let message = to_integer(message.into())?;
        let partners = partners
            .iter()
            .map(|partner| to_integer(partner.get().into()))
            .collect::<Result<Vec<i64>, IqStoreError>>()?;
        let mut transaction = self.pool.begin().await?;
        let recorded = sqlx::query(
            "INSERT INTO iq_submissions (message_id, competition_id, user_id, criterion, points, posted_at_ms, awarded_by, awarded_at_ms) VALUES (?, ?, ?, ?, ?, ?, NULL, NULL) ON CONFLICT (message_id) DO NOTHING",
        )
        .bind(message)
        .bind(id)
        .bind(to_integer(user.get().into())?)
        .bind(criterion)
        .bind(i64::from(points))
        .bind(at_ms)
        .execute(&mut *transaction)
        .await?;
        let inserted = recorded.rows_affected() == 1;
        if inserted {
            for partner in partners {
                sqlx::query(
                    "INSERT INTO iq_submission_partners (message_id, user_id) VALUES (?, ?) ON CONFLICT (message_id, user_id) DO NOTHING",
                )
                .bind(message)
                .bind(partner)
                .execute(&mut *transaction)
                .await?;
            }
        }
        transaction.commit().await?;
        Ok(inserted)
    }

    pub async fn award(
        &self,
        message: u64,
        judge: UserId,
        at_ms: i64,
    ) -> Result<AwardOutcome, IqStoreError> {
        let message = to_integer(message.into())?;
        let mut transaction = self.pool.begin().await?;
        let awarded: Option<(i64, i64)> = sqlx::query_as(
            "UPDATE iq_submissions SET awarded_by = ?, awarded_at_ms = ? WHERE message_id = ? AND awarded_by IS NULL AND message_id NOT IN (SELECT message_id FROM iq_rejections) AND competition_id IN (SELECT id FROM iq_competitions WHERE status IN ('running', 'stopped')) RETURNING user_id, points",
        )
        .bind(to_integer(judge.get().into())?)
        .bind(at_ms)
        .bind(message)
        .fetch_optional(&mut *transaction)
        .await?;
        if let Some((user, points)) = awarded {
            let partners: Vec<i64> = sqlx::query_scalar(
                "SELECT user_id FROM iq_submission_partners WHERE message_id = ? ORDER BY rowid",
            )
            .bind(message)
            .fetch_all(&mut *transaction)
            .await?;
            transaction.commit().await?;
            let players = std::iter::once(user)
                .chain(partners)
                .map(to_user)
                .collect::<Result<Vec<UserId>, IqStoreError>>()?;
            return Ok(AwardOutcome::Awarded {
                players,
                points: to_points(points)?,
            });
        }
        transaction.rollback().await?;
        let rejected: Option<i64> =
            sqlx::query_scalar("SELECT 1 FROM iq_rejections WHERE message_id = ?")
                .bind(message)
                .fetch_optional(&self.pool)
                .await?;
        if rejected.is_some() {
            return Ok(AwardOutcome::Rejected);
        }
        let found: Option<String> = sqlx::query_scalar(
            "SELECT c.status FROM iq_submissions s JOIN iq_competitions c ON c.id = s.competition_id WHERE s.message_id = ?",
        )
        .bind(message)
        .fetch_optional(&self.pool)
        .await?;
        let Some(status) = found else {
            return Ok(AwardOutcome::NotASubmission);
        };
        match parse_status(&status)? {
            Status::Completed | Status::Archived => Ok(AwardOutcome::Closed),
            Status::Running | Status::Stopped => Ok(AwardOutcome::AlreadyAwarded),
        }
    }

    pub async fn unawarded(&self, competition: i64, limit: u32) -> Result<Vec<u64>, IqStoreError> {
        let rows: Vec<i64> = sqlx::query_scalar(
            "SELECT message_id FROM iq_submissions WHERE competition_id = ? AND awarded_by IS NULL AND message_id NOT IN (SELECT message_id FROM iq_rejections) ORDER BY message_id DESC LIMIT ?",
        )
        .bind(competition)
        .bind(i64::from(limit))
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter().map(to_unsigned).collect()
    }

    pub async fn criterion(&self, message: u64) -> Result<Option<String>, IqStoreError> {
        Ok(
            sqlx::query_scalar("SELECT criterion FROM iq_submissions WHERE message_id = ?")
                .bind(to_integer(message.into())?)
                .fetch_optional(&self.pool)
                .await?,
        )
    }

    pub async fn recorded(&self, competition: i64, message: u64) -> Result<bool, IqStoreError> {
        let message = to_integer(message.into())?;
        let found: Option<i64> = sqlx::query_scalar(
            "SELECT 1 FROM iq_submissions WHERE competition_id = ? AND message_id = ? UNION ALL SELECT 1 FROM iq_rejections WHERE competition_id = ? AND message_id = ? LIMIT 1",
        )
        .bind(competition)
        .bind(message)
        .bind(competition)
        .bind(message)
        .fetch_optional(&self.pool)
        .await?;
        Ok(found.is_some())
    }

    pub async fn forget(&self, competition: i64, message: u64) -> Result<(), IqStoreError> {
        let message = to_integer(message.into())?;
        let mut transaction = self.pool.begin().await?;
        sqlx::query(
            "DELETE FROM iq_submission_partners WHERE message_id IN (SELECT message_id FROM iq_submissions WHERE competition_id = ? AND message_id = ? AND awarded_by IS NULL)",
        )
        .bind(competition)
        .bind(message)
        .execute(&mut *transaction)
        .await?;
        sqlx::query(
            "DELETE FROM iq_submissions WHERE competition_id = ? AND message_id = ? AND awarded_by IS NULL",
        )
        .bind(competition)
        .bind(message)
        .execute(&mut *transaction)
        .await?;
        transaction.commit().await?;
        Ok(())
    }

    pub async fn adjust(
        &self,
        id: i64,
        user: UserId,
        points: i64,
        reason: &str,
        judge: UserId,
        at_ms: i64,
    ) -> Result<AdjustOutcome, IqStoreError> {
        let points = iq::adjustment(points).ok_or(IqStoreError::BadAdjustment { points })?;
        let reason = checked_reason(reason)?;
        let user = to_integer(user.get().into())?;
        let judge = to_integer(judge.get().into())?;
        let mut transaction = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let current: i64 = sqlx::query_scalar(concat!(
            "SELECT coalesce(sum(points), 0) FROM (",
            player_points!(),
            ") WHERE user_id = ?"
        ))
        .bind(id)
        .bind(id)
        .bind(id)
        .bind(user)
        .fetch_one(&mut *transaction)
        .await?;
        let total = current.saturating_add(points);
        if points < 0 && total < 0 {
            transaction.rollback().await?;
            return Ok(AdjustOutcome::BelowZero { total: current });
        }
        sqlx::query(
            "INSERT INTO iq_adjustments (competition_id, user_id, points, reason, judge_id, made_at_ms) VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(id)
        .bind(user)
        .bind(points)
        .bind(reason)
        .bind(judge)
        .bind(at_ms)
        .execute(&mut *transaction)
        .await?;
        transaction.commit().await?;
        Ok(AdjustOutcome::Adjusted { total })
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn reject(
        &self,
        competition: i64,
        message: u64,
        poster: UserId,
        reason: &str,
        judge: UserId,
        at_ms: i64,
    ) -> Result<RejectOutcome, IqStoreError> {
        let reason = checked_reason(reason)?;
        let message = to_integer(message.into())?;
        let poster = to_integer(poster.get().into())?;
        let judge = to_integer(judge.get().into())?;
        let mut transaction = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let awarded: Option<i64> = sqlx::query_scalar(
            "SELECT 1 FROM iq_submissions WHERE message_id = ? AND awarded_by IS NOT NULL",
        )
        .bind(message)
        .fetch_optional(&mut *transaction)
        .await?;
        if awarded.is_some() {
            transaction.rollback().await?;
            return Ok(RejectOutcome::AlreadyAwarded);
        }
        let inserted = sqlx::query(
            "INSERT INTO iq_rejections (message_id, competition_id, user_id, reason, rejected_by, rejected_at_ms) VALUES (?, ?, ?, ?, ?, ?) ON CONFLICT (message_id) DO NOTHING",
        )
        .bind(message)
        .bind(competition)
        .bind(poster)
        .bind(reason)
        .bind(judge)
        .bind(at_ms)
        .execute(&mut *transaction)
        .await?;
        transaction.commit().await?;
        Ok(match inserted.rows_affected() {
            0 => RejectOutcome::AlreadyRejected,
            _ => RejectOutcome::Rejected,
        })
    }

    pub async fn scores(&self, id: i64) -> Result<Vec<Score>, IqStoreError> {
        let rows: Vec<(i64, i64)> = sqlx::query_as(concat!(
            "SELECT user_id, SUM(points) FROM (",
            player_points!(),
            ") GROUP BY user_id ORDER BY user_id"
        ))
        .bind(id)
        .bind(id)
        .bind(id)
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter()
            .map(|(user, points)| {
                Ok(Score {
                    user: to_user(user)?,
                    points,
                })
            })
            .collect()
    }

    pub async fn archive(&self, id: i64) -> Result<(), IqStoreError> {
        self.set_status(id, Status::Archived).await
    }

    pub async fn last_archived(&self, guild: GuildId) -> Result<Option<Competition>, IqStoreError> {
        let row = sqlx::query(LAST_ARCHIVED)
            .bind(to_integer(guild.get().into())?)
            .fetch_optional(&self.pool)
            .await?;
        self.complete(row).await
    }

    pub async fn templates(&self, guild: GuildId) -> Result<Vec<Competition>, IqStoreError> {
        let rows = sqlx::query(NEWEST_IN_GUILD)
            .bind(to_integer(guild.get().into())?)
            .bind(TEMPLATE_LIMIT)
            .fetch_all(&self.pool)
            .await?;
        let mut templates = Vec::with_capacity(rows.len());
        for row in rows {
            templates.push(self.with_config(CompetitionRow::read(&row)?).await?);
        }
        Ok(templates)
    }

    pub async fn judge_role(&self, guild: GuildId) -> Result<Option<RoleId>, IqStoreError> {
        let role: Option<i64> =
            sqlx::query_scalar("SELECT judge_role_id FROM iq_servers WHERE guild_id = ?")
                .bind(to_integer(guild.get().into())?)
                .fetch_optional(&self.pool)
                .await?;
        role.map(|role| to_unsigned(role).map(RoleId::new))
            .transpose()
    }

    pub async fn set_judge_role(&self, guild: GuildId, role: RoleId) -> Result<(), IqStoreError> {
        sqlx::query(
            "INSERT INTO iq_servers (guild_id, judge_role_id) VALUES (?, ?) ON CONFLICT (guild_id) DO UPDATE SET judge_role_id = excluded.judge_role_id",
        )
        .bind(to_integer(guild.get().into())?)
        .bind(to_integer(role.get().into())?)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn complete(&self, row: Option<SqliteRow>) -> Result<Option<Competition>, IqStoreError> {
        match row {
            Some(row) => Ok(Some(self.with_config(CompetitionRow::read(&row)?).await?)),
            None => Ok(None),
        }
    }

    async fn with_config(&self, row: CompetitionRow) -> Result<Competition, IqStoreError> {
        let id = row.id;
        let criteria: Vec<(String, String, i64, String)> = sqlx::query_as(
            "SELECT name, description, points, kind FROM iq_criteria WHERE competition_id = ? ORDER BY position",
        )
        .bind(id)
        .fetch_all(&self.pool)
        .await?;
        let roles: Vec<i64> = sqlx::query_scalar(
            "SELECT role_id FROM iq_target_roles WHERE competition_id = ? ORDER BY role_id",
        )
        .bind(id)
        .fetch_all(&self.pool)
        .await?;
        let judges: Vec<i64> = sqlx::query_scalar(
            "SELECT user_id FROM iq_judges WHERE competition_id = ? ORDER BY user_id",
        )
        .bind(id)
        .fetch_all(&self.pool)
        .await?;
        to_competition(row, criteria, roles, judges)
    }
}

fn checked_reason(reason: &str) -> Result<&str, IqStoreError> {
    let reason = reason.trim();
    if reason.is_empty() {
        return Err(IqStoreError::ReasonMissing);
    }
    if reason.chars().count() > REASON_LIMIT {
        return Err(IqStoreError::ReasonTooLong);
    }
    Ok(reason)
}

async fn insert_config(
    transaction: &mut Transaction<'_, Sqlite>,
    id: i64,
    criteria: &[Criterion],
    roles: &[RoleId],
    judges: &[UserId],
) -> Result<(), IqStoreError> {
    for (position, criterion) in (0_i64..).zip(criteria) {
        sqlx::query(
            "INSERT INTO iq_criteria (competition_id, position, name, description, points, kind) VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(id)
        .bind(position)
        .bind(&criterion.name)
        .bind(&criterion.description)
        .bind(i64::from(criterion.points))
        .bind(kind_text(criterion.kind))
        .execute(&mut **transaction)
        .await?;
    }
    for role in roles {
        sqlx::query("INSERT INTO iq_target_roles (competition_id, role_id) VALUES (?, ?)")
            .bind(id)
            .bind(to_integer(role.get().into())?)
            .execute(&mut **transaction)
            .await?;
    }
    for judge in judges {
        sqlx::query("INSERT INTO iq_judges (competition_id, user_id) VALUES (?, ?)")
            .bind(id)
            .bind(to_integer(judge.get().into())?)
            .execute(&mut **transaction)
            .await?;
    }
    Ok(())
}

async fn columns(pool: &SqlitePool, table: &str) -> Result<Vec<Column>, IqStoreError> {
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

fn without_kind(columns: &[Column]) -> Vec<Column> {
    columns
        .iter()
        .filter(|column| column.name != KIND_COLUMN)
        .cloned()
        .collect()
}

fn to_competition(
    row: CompetitionRow,
    criteria: Vec<(String, String, i64, String)>,
    roles: Vec<i64>,
    judges: Vec<i64>,
) -> Result<Competition, IqStoreError> {
    let marks = JudgingMarks::new(
        parse_mark(row.denied)?,
        parse_mark(row.awarded)?,
        parse_mark(row.confirm)?,
    )
    .map_err(|_| IqStoreError::RepeatedMarks)?;
    Ok(Competition {
        id: row.id,
        status: parse_status(&row.status)?,
        last_seen: row.last_seen.map(to_unsigned).transpose()?,
        guild: GuildId::new(to_unsigned(row.guild)?),
        name: row.name,
        first_day: parse_date(row.first_day)?,
        last_day: parse_date(row.last_day)?,
        zone: row.zone,
        category: ChannelId::new(to_unsigned(row.category)?),
        announcements: ChannelId::new(to_unsigned(row.announcements)?),
        submissions: ChannelId::new(to_unsigned(row.submissions)?),
        criteria: criteria
            .into_iter()
            .map(|(name, description, points, kind)| {
                Ok(Criterion {
                    name,
                    description,
                    points: to_points(points)?,
                    kind: parse_kind(kind)?,
                })
            })
            .collect::<Result<_, IqStoreError>>()?,
        rules: row.rules,
        roles: roles
            .into_iter()
            .map(|role| to_unsigned(role).map(RoleId::new))
            .collect::<Result<_, _>>()?,
        judges: judges.into_iter().map(to_user).collect::<Result<_, _>>()?,
        marks,
        judge_log: row
            .judge_log
            .map(|channel| to_unsigned(channel).map(ChannelId::new))
            .transpose()?,
        created_by: to_user(row.created_by)?,
        created_at_ms: row.created_at_ms,
    })
}

fn parse_mark(text: String) -> Result<Mark, IqStoreError> {
    Mark::parse(&text).ok_or(IqStoreError::BadMark { value: text })
}

fn kind_text(kind: CriterionKind) -> &'static str {
    match kind {
        CriterionKind::Single => "single",
        CriterionKind::Multi => "multi",
    }
}

fn parse_kind(text: String) -> Result<CriterionKind, IqStoreError> {
    match text.as_str() {
        "single" => Ok(CriterionKind::Single),
        "multi" => Ok(CriterionKind::Multi),
        _ => Err(IqStoreError::BadCriterionKind { value: text }),
    }
}

fn status_text(status: Status) -> &'static str {
    match status {
        Status::Running => "running",
        Status::Stopped => "stopped",
        Status::Completed => "completed",
        Status::Archived => "archived",
    }
}

fn parse_status(text: &str) -> Result<Status, IqStoreError> {
    match text {
        "running" => Ok(Status::Running),
        "stopped" => Ok(Status::Stopped),
        "completed" => Ok(Status::Completed),
        "archived" => Ok(Status::Archived),
        other => Err(IqStoreError::BadStatus {
            value: other.to_owned(),
        }),
    }
}

fn parse_date(text: String) -> Result<Date, IqStoreError> {
    text.parse::<Date>()
        .map_err(|_| IqStoreError::BadDate { value: text })
}

fn snowflake_at(at_unix: i64) -> Result<u64, IqStoreError> {
    at_unix
        .checked_mul(1_000)
        .and_then(|at_ms| at_ms.checked_sub(DISCORD_EPOCH_MS))
        .and_then(|since_epoch| u64::try_from(since_epoch).ok())
        .filter(|since_epoch| *since_epoch < SNOWFLAKE_TIME_LIMIT)
        .map(|since_epoch| since_epoch << SNOWFLAKE_TIME_SHIFT)
        .ok_or(IqStoreError::NotASnowflakeMoment { at_unix })
}

fn expect_one(rows: u64, id: i64) -> Result<(), IqStoreError> {
    match rows {
        1 => Ok(()),
        _ => Err(IqStoreError::UnknownCompetition { id }),
    }
}

fn to_points(value: i64) -> Result<u32, IqStoreError> {
    u32::try_from(value)
        .ok()
        .filter(|points| *points > 0)
        .ok_or(IqStoreError::BadPoints { value })
}

fn to_user(value: i64) -> Result<UserId, IqStoreError> {
    u64::try_from(value)
        .ok()
        .filter(|id| *id > 0)
        .map(UserId::new)
        .ok_or(IqStoreError::InvalidUser { value })
}

fn to_unsigned(value: i64) -> Result<u64, IqStoreError> {
    u64::try_from(value).map_err(|_| IqStoreError::Negative { value })
}

fn to_integer(value: u128) -> Result<i64, IqStoreError> {
    i64::try_from(value).map_err(|_| IqStoreError::OutOfRange { value })
}
