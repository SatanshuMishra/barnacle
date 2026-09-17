use std::future::Future;
use std::path::Path;
use std::time::Duration;

use barnacle_catalog::ShipIndex;
use barnacle_guess::UserId;
use sqlx::sqlite::SqliteConnectOptions;
use sqlx::sqlite::SqlitePool;
use sqlx::sqlite::SqlitePoolOptions;

use crate::ids::GuildId;

const BUSY_TIMEOUT: Duration = Duration::from_secs(5);
const EXPECTED_COLUMNS: [(&str, &str, bool, bool); 6] = [
    ("id", "INTEGER", false, true),
    ("guild_id", "INTEGER", true, false),
    ("user_id", "INTEGER", true, false),
    ("ship_index", "TEXT", true, false),
    ("elapsed_ms", "INTEGER", true, false),
    ("solved_at_ms", "INTEGER", true, false),
];

#[derive(Debug, thiserror::Error)]
pub enum SolvesError {
    #[error("the solves database could not be used")]
    Database(#[from] sqlx::Error),
    #[error("the solves database has no guess_solves table")]
    MissingTable,
    #[error("guess_solves does not have the expected columns; found {found:?}")]
    UnexpectedColumns { found: Vec<Column> },
    #[error("{value} does not fit in a SQLite integer")]
    OutOfRange { value: u128 },
    #[error("guess_solves holds a negative value, {value}")]
    Negative { value: i64 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Column {
    pub name: String,
    pub kind: String,
    pub not_null: bool,
    pub primary_key: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SolveRecord {
    pub guild: GuildId,
    pub user: UserId,
    pub ship: ShipIndex,
    pub elapsed: Duration,
    pub solved_at_ms: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Profile {
    pub wins: u64,
    pub best: Option<Duration>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Ranking {
    #[default]
    MostWins,
    FastestTime,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Standing {
    pub user: UserId,
    pub wins: u64,
    pub best: Duration,
}

pub trait SolveStore: Send + Sync + 'static {
    fn record(
        &self,
        record: &SolveRecord,
    ) -> impl Future<Output = Result<bool, SolvesError>> + Send;
}

#[derive(Debug, Clone)]
pub struct Solves {
    pool: SqlitePool,
}

impl Solves {
    pub async fn open(path: &Path) -> Result<Self, SolvesError> {
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

    pub async fn with_pool(pool: SqlitePool) -> Result<Self, SolvesError> {
        let found = columns(&pool).await?;
        let expected: Vec<Column> = EXPECTED_COLUMNS
            .iter()
            .map(|(name, kind, not_null, primary_key)| Column {
                name: (*name).to_owned(),
                kind: (*kind).to_owned(),
                not_null: *not_null,
                primary_key: *primary_key,
            })
            .collect();
        if found.is_empty() {
            Err(SolvesError::MissingTable)
        } else if found != expected {
            Err(SolvesError::UnexpectedColumns { found })
        } else {
            Ok(Self { pool })
        }
    }

    pub async fn profile(&self, guild: GuildId, user: UserId) -> Result<Profile, SolvesError> {
        let (wins, best): (i64, Option<i64>) = sqlx::query_as(
            "SELECT COUNT(*), MIN(elapsed_ms) FROM guess_solves WHERE guild_id = ? AND user_id = ?",
        )
        .bind(to_integer(guild.get().into())?)
        .bind(to_integer(user.get().into())?)
        .fetch_one(&self.pool)
        .await?;
        Ok(Profile {
            wins: u64::try_from(wins).unwrap_or_default(),
            best: best
                .and_then(|millis| u64::try_from(millis).ok())
                .map(Duration::from_millis),
        })
    }

    pub async fn leaderboard(
        &self,
        guild: GuildId,
        ranking: Ranking,
        size: u32,
    ) -> Result<Vec<Standing>, SolvesError> {
        let query = match ranking {
            Ranking::MostWins => sqlx::query_as(
                "SELECT user_id, COUNT(*) AS wins, MIN(elapsed_ms) AS best FROM guess_solves WHERE guild_id = ? GROUP BY user_id ORDER BY wins DESC, best ASC, user_id ASC LIMIT ?",
            ),
            Ranking::FastestTime => sqlx::query_as(
                "SELECT user_id, COUNT(*) AS wins, MIN(elapsed_ms) AS best FROM guess_solves WHERE guild_id = ? GROUP BY user_id ORDER BY best ASC, wins DESC, user_id ASC LIMIT ?",
            ),
        };
        let rows: Vec<(i64, i64, i64)> = query
            .bind(to_integer(guild.get().into())?)
            .bind(i64::from(size))
            .fetch_all(&self.pool)
            .await?;
        rows.into_iter()
            .map(|(user, wins, best)| {
                Ok(Standing {
                    user: UserId::new(to_unsigned(user)?),
                    wins: to_unsigned(wins)?,
                    best: Duration::from_millis(to_unsigned(best)?),
                })
            })
            .collect()
    }
}

impl SolveStore for Solves {
    async fn record(&self, record: &SolveRecord) -> Result<bool, SolvesError> {
        let guild = to_integer(record.guild.get().into())?;
        let user = to_integer(record.user.get().into())?;
        let elapsed = to_integer(record.elapsed.as_millis())?;
        let solved_at = to_integer(record.solved_at_ms.into())?;
        let mut transaction = self.pool.begin().await?;
        let previous: Option<i64> = sqlx::query_scalar(
            "SELECT MIN(elapsed_ms) FROM guess_solves WHERE guild_id = ? AND user_id = ?",
        )
        .bind(guild)
        .bind(user)
        .fetch_one(&mut *transaction)
        .await?;
        sqlx::query(
            "INSERT INTO guess_solves (guild_id, user_id, ship_index, elapsed_ms, solved_at_ms) VALUES (?, ?, ?, ?, ?)",
        )
        .bind(guild)
        .bind(user)
        .bind(record.ship.as_str())
        .bind(elapsed)
        .bind(solved_at)
        .execute(&mut *transaction)
        .await?;
        transaction.commit().await?;
        Ok(previous.is_none_or(|best| elapsed < best))
    }
}

async fn columns(pool: &SqlitePool) -> Result<Vec<Column>, SolvesError> {
    let rows: Vec<(String, String, i64, i64)> = sqlx::query_as(
        "SELECT name, type, \"notnull\", pk FROM pragma_table_info('guess_solves') ORDER BY cid",
    )
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

fn to_unsigned(value: i64) -> Result<u64, SolvesError> {
    u64::try_from(value).map_err(|_| SolvesError::Negative { value })
}

fn to_integer(value: u128) -> Result<i64, SolvesError> {
    i64::try_from(value).map_err(|_| SolvesError::OutOfRange { value })
}
