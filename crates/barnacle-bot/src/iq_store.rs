use std::path::Path;
use std::time::Duration;

use barnacle_guess::UserId;
use jiff::civil::Date;
use sqlx::Sqlite;
use sqlx::Transaction;
use sqlx::sqlite::SqliteConnectOptions;
use sqlx::sqlite::SqlitePool;
use sqlx::sqlite::SqlitePoolOptions;

use crate::ids::ChannelId;
use crate::ids::GuildId;
use crate::ids::RoleId;
use crate::iq;
use crate::iq::Criterion;
use crate::iq::JudgingReaction;
use crate::iq::Score;
use crate::iq::Status;
use crate::solves::Column;

const BUSY_TIMEOUT: Duration = Duration::from_secs(5);
const DISCORD_EPOCH_MS: i64 = 1_420_070_400_000;
const SNOWFLAKE_TIME_SHIFT: u32 = 22;
const SNOWFLAKE_TIME_LIMIT: u64 = 1 << 41;
const TEMPLATE_LIMIT: i64 = 10;
const REASON_LIMIT: usize = 200;

const COMPETITIONS: &str = "iq_competitions";
const CRITERIA: &str = "iq_criteria";
const TARGET_ROLES: &str = "iq_target_roles";
const JUDGES: &str = "iq_judges";
const ANNOUNCEMENTS: &str = "iq_announcements";
const SUBMISSIONS: &str = "iq_submissions";
const ADJUSTMENTS: &str = "iq_adjustments";
const SERVERS: &str = "iq_servers";
const LIVE_INDEX: &str = "iq_competitions_live";
const CRITERION_NAME_INDEX: &str = "iq_criteria_name";

type ColumnSpec = (&'static str, &'static str, bool, bool);
type TableSpec = (&'static str, &'static [ColumnSpec]);

const EXPECTED_COLUMNS: [TableSpec; 8] = [
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
            ("judging_reaction", "TEXT", true, false),
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
];

const EXPECTED_INDEXES: [&str; 2] = [LIVE_INDEX, CRITERION_NAME_INDEX];

const LIVE_IN_GUILD: &str = "SELECT id, guild_id, name, first_day, last_day, time_zone, status, category_id, announcements_id, submissions_id, judging_reaction, rules, last_seen_message, created_by, created_at_ms FROM iq_competitions WHERE guild_id = ? AND status <> 'archived' ORDER BY id DESC LIMIT 1";
const LIVE_BY_SUBMISSIONS: &str = "SELECT id, guild_id, name, first_day, last_day, time_zone, status, category_id, announcements_id, submissions_id, judging_reaction, rules, last_seen_message, created_by, created_at_ms FROM iq_competitions WHERE submissions_id = ? AND status <> 'archived' ORDER BY id DESC LIMIT 1";
const BY_ID: &str = "SELECT id, guild_id, name, first_day, last_day, time_zone, status, category_id, announcements_id, submissions_id, judging_reaction, rules, last_seen_message, created_by, created_at_ms FROM iq_competitions WHERE id = ?";
const LAST_ARCHIVED: &str = "SELECT id, guild_id, name, first_day, last_day, time_zone, status, category_id, announcements_id, submissions_id, judging_reaction, rules, last_seen_message, created_by, created_at_ms FROM iq_competitions WHERE guild_id = ? AND status = 'archived' ORDER BY created_at_ms DESC, id DESC LIMIT 1";
const NEWEST_IN_GUILD: &str = "SELECT id, guild_id, name, first_day, last_day, time_zone, status, category_id, announcements_id, submissions_id, judging_reaction, rules, last_seen_message, created_by, created_at_ms FROM iq_competitions WHERE guild_id = ? ORDER BY created_at_ms DESC, id DESC LIMIT ?";

type CompetitionRow = (
    i64,
    i64,
    String,
    String,
    String,
    String,
    String,
    i64,
    i64,
    i64,
    String,
    String,
    Option<i64>,
    i64,
    i64,
);

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
    #[error("the IQ Competition database holds {value}, which is not a judging reaction")]
    BadReaction { value: String },
    #[error("the IQ Competition database holds {value}, which is not a criterion's points")]
    BadPoints { value: i64 },
    #[error("{points} is not a points change between -1000 and 1000 other than 0")]
    BadAdjustment { points: i64 },
    #[error("a points change reason is longer than {REASON_LIMIT} characters")]
    ReasonTooLong,
    #[error("{at_unix} is outside the moments a Discord message ID can hold")]
    NotASnowflakeMoment { at_unix: i64 },
    #[error("there is no IQ Competition with ID {id}")]
    UnknownCompetition { id: i64 },
    #[error("IQ Competition {id} is completed or archived, so it cannot resume")]
    NotResumable { id: i64 },
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
    pub reaction: JudgingReaction,
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
    pub reaction: JudgingReaction,
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
pub enum AwardOutcome {
    Awarded { user: UserId, points: u32 },
    AlreadyAwarded,
    NotASubmission,
    Closed,
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
        for (table, expected) in EXPECTED_COLUMNS {
            let found = columns(&pool, table).await?;
            if found.is_empty() {
                return Err(IqStoreError::MissingTable { table });
            }
            if found != expected_columns(expected) {
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
            "INSERT INTO iq_competitions (guild_id, name, first_day, last_day, time_zone, status, category_id, announcements_id, submissions_id, judging_reaction, rules, last_seen_message, created_by, created_at_ms) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL, ?, ?) RETURNING id",
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
        .bind(new.reaction.value())
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
        let row: Option<CompetitionRow> = sqlx::query_as(LIVE_IN_GUILD)
            .bind(to_integer(guild.get().into())?)
            .fetch_optional(&self.pool)
            .await?;
        self.complete(row).await
    }

    pub async fn by_submissions_channel(
        &self,
        channel: ChannelId,
    ) -> Result<Option<Competition>, IqStoreError> {
        let row: Option<CompetitionRow> = sqlx::query_as(LIVE_BY_SUBMISSIONS)
            .bind(to_integer(channel.get().into())?)
            .fetch_optional(&self.pool)
            .await?;
        self.complete(row).await
    }

    pub async fn competition(&self, id: i64) -> Result<Option<Competition>, IqStoreError> {
        let row: Option<CompetitionRow> = sqlx::query_as(BY_ID)
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        self.complete(row).await
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

    pub async fn update_config(
        &self,
        id: i64,
        criteria: &[Criterion],
        rules: &str,
        roles: &[RoleId],
        judges: &[UserId],
        reaction: JudgingReaction,
    ) -> Result<(), IqStoreError> {
        let mut transaction = self.pool.begin().await?;
        let changed =
            sqlx::query("UPDATE iq_competitions SET rules = ?, judging_reaction = ? WHERE id = ?")
                .bind(rules)
                .bind(reaction.value())
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

    pub async fn record_submission(
        &self,
        id: i64,
        message: u64,
        user: UserId,
        criterion: &str,
        points: u32,
        at_ms: i64,
    ) -> Result<bool, IqStoreError> {
        let recorded = sqlx::query(
            "INSERT INTO iq_submissions (message_id, competition_id, user_id, criterion, points, posted_at_ms, awarded_by, awarded_at_ms) VALUES (?, ?, ?, ?, ?, ?, NULL, NULL) ON CONFLICT (message_id) DO NOTHING",
        )
        .bind(to_integer(message.into())?)
        .bind(id)
        .bind(to_integer(user.get().into())?)
        .bind(criterion)
        .bind(i64::from(points))
        .bind(at_ms)
        .execute(&self.pool)
        .await?;
        Ok(recorded.rows_affected() == 1)
    }

    pub async fn award(
        &self,
        message: u64,
        judge: UserId,
        at_ms: i64,
    ) -> Result<AwardOutcome, IqStoreError> {
        let message = to_integer(message.into())?;
        let awarded: Option<(i64, i64)> = sqlx::query_as(
            "UPDATE iq_submissions SET awarded_by = ?, awarded_at_ms = ? WHERE message_id = ? AND awarded_by IS NULL AND competition_id IN (SELECT id FROM iq_competitions WHERE status IN ('running', 'stopped')) RETURNING user_id, points",
        )
        .bind(to_integer(judge.get().into())?)
        .bind(at_ms)
        .bind(message)
        .fetch_optional(&self.pool)
        .await?;
        if let Some((user, points)) = awarded {
            return Ok(AwardOutcome::Awarded {
                user: to_user(user)?,
                points: to_points(points)?,
            });
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

    pub async fn adjust(
        &self,
        id: i64,
        user: UserId,
        points: i64,
        reason: Option<&str>,
        judge: UserId,
        at_ms: i64,
    ) -> Result<(), IqStoreError> {
        let points = iq::adjustment(points).ok_or(IqStoreError::BadAdjustment { points })?;
        if reason.is_some_and(|reason| reason.chars().count() > REASON_LIMIT) {
            return Err(IqStoreError::ReasonTooLong);
        }
        sqlx::query(
            "INSERT INTO iq_adjustments (competition_id, user_id, points, reason, judge_id, made_at_ms) VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(id)
        .bind(to_integer(user.get().into())?)
        .bind(points)
        .bind(reason)
        .bind(to_integer(judge.get().into())?)
        .bind(at_ms)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn scores(&self, id: i64) -> Result<Vec<Score>, IqStoreError> {
        let rows: Vec<(i64, i64)> = sqlx::query_as(
            "SELECT user_id, SUM(points) FROM (SELECT user_id, points FROM iq_submissions WHERE competition_id = ? AND awarded_by IS NOT NULL UNION ALL SELECT user_id, points FROM iq_adjustments WHERE competition_id = ?) GROUP BY user_id ORDER BY user_id",
        )
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
        let row: Option<CompetitionRow> = sqlx::query_as(LAST_ARCHIVED)
            .bind(to_integer(guild.get().into())?)
            .fetch_optional(&self.pool)
            .await?;
        self.complete(row).await
    }

    pub async fn templates(&self, guild: GuildId) -> Result<Vec<Competition>, IqStoreError> {
        let rows: Vec<CompetitionRow> = sqlx::query_as(NEWEST_IN_GUILD)
            .bind(to_integer(guild.get().into())?)
            .bind(TEMPLATE_LIMIT)
            .fetch_all(&self.pool)
            .await?;
        let mut templates = Vec::with_capacity(rows.len());
        for row in rows {
            templates.push(self.with_config(row).await?);
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

    async fn complete(
        &self,
        row: Option<CompetitionRow>,
    ) -> Result<Option<Competition>, IqStoreError> {
        match row {
            Some(row) => Ok(Some(self.with_config(row).await?)),
            None => Ok(None),
        }
    }

    async fn with_config(&self, row: CompetitionRow) -> Result<Competition, IqStoreError> {
        let id = row.0;
        let criteria: Vec<(String, String, i64)> = sqlx::query_as(
            "SELECT name, description, points FROM iq_criteria WHERE competition_id = ? ORDER BY position",
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

async fn insert_config(
    transaction: &mut Transaction<'_, Sqlite>,
    id: i64,
    criteria: &[Criterion],
    roles: &[RoleId],
    judges: &[UserId],
) -> Result<(), IqStoreError> {
    for (position, criterion) in (0_i64..).zip(criteria) {
        sqlx::query(
            "INSERT INTO iq_criteria (competition_id, position, name, description, points) VALUES (?, ?, ?, ?, ?)",
        )
        .bind(id)
        .bind(position)
        .bind(&criterion.name)
        .bind(&criterion.description)
        .bind(i64::from(criterion.points))
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

fn to_competition(
    row: CompetitionRow,
    criteria: Vec<(String, String, i64)>,
    roles: Vec<i64>,
    judges: Vec<i64>,
) -> Result<Competition, IqStoreError> {
    let (
        id,
        guild,
        name,
        first_day,
        last_day,
        zone,
        status,
        category,
        announcements,
        submissions,
        reaction,
        rules,
        last_seen,
        created_by,
        created_at_ms,
    ) = row;
    Ok(Competition {
        id,
        status: parse_status(&status)?,
        last_seen: last_seen.map(to_unsigned).transpose()?,
        guild: GuildId::new(to_unsigned(guild)?),
        name,
        first_day: parse_date(first_day)?,
        last_day: parse_date(last_day)?,
        zone,
        category: ChannelId::new(to_unsigned(category)?),
        announcements: ChannelId::new(to_unsigned(announcements)?),
        submissions: ChannelId::new(to_unsigned(submissions)?),
        criteria: criteria
            .into_iter()
            .map(|(name, description, points)| {
                Ok(Criterion {
                    name,
                    description,
                    points: to_points(points)?,
                })
            })
            .collect::<Result<_, IqStoreError>>()?,
        rules,
        roles: roles
            .into_iter()
            .map(|role| to_unsigned(role).map(RoleId::new))
            .collect::<Result<_, _>>()?,
        judges: judges.into_iter().map(to_user).collect::<Result<_, _>>()?,
        reaction: JudgingReaction::parse(&reaction)
            .ok_or(IqStoreError::BadReaction { value: reaction })?,
        created_by: to_user(created_by)?,
        created_at_ms,
    })
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
