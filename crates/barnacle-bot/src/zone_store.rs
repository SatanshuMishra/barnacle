use barnacle_guess::UserId;
use sqlx::sqlite::SqlitePool;

use crate::schedule::ZONE_NAME_LIMIT;
use crate::solves::Column;

const MEMBER_ZONES: &str = "member_zones";

type ColumnSpec = (&'static str, &'static str, bool, bool);
type TableSpec = (&'static str, &'static [ColumnSpec]);

const EXPECTED_COLUMNS: [TableSpec; 1] = [(
    MEMBER_ZONES,
    &[
        ("user_id", "INTEGER", false, true),
        ("time_zone", "TEXT", true, false),
    ],
)];

#[derive(Debug, thiserror::Error)]
pub enum ZoneStoreError {
    #[error("the member zone database could not be used")]
    Database(#[from] sqlx::Error),
    #[error("the member zone database has no {table} table")]
    MissingTable { table: &'static str },
    #[error("{table} does not have the expected columns; found {found:?}")]
    UnexpectedColumns {
        table: &'static str,
        found: Vec<Column>,
    },
    #[error("{value} does not fit in a SQLite integer")]
    OutOfRange { value: u64 },
    #[error("a zone name must be 1 to {ZONE_NAME_LIMIT} characters, not {length}")]
    ZoneLength { length: usize },
}

#[derive(Debug, Clone)]
pub struct ZoneStore {
    pool: SqlitePool,
}

impl ZoneStore {
    pub async fn with_pool(pool: SqlitePool) -> Result<ZoneStore, ZoneStoreError> {
        for (table, expected) in EXPECTED_COLUMNS {
            let found = columns(&pool, table).await?;
            if found.is_empty() {
                return Err(ZoneStoreError::MissingTable { table });
            }
            if found != expected_columns(expected) {
                return Err(ZoneStoreError::UnexpectedColumns { table, found });
            }
        }
        Ok(ZoneStore { pool })
    }

    pub async fn remember(&self, user: UserId, zone: &str) -> Result<(), ZoneStoreError> {
        let length = zone.chars().count();
        if !(1..=ZONE_NAME_LIMIT).contains(&length) {
            return Err(ZoneStoreError::ZoneLength { length });
        }
        sqlx::query("INSERT OR REPLACE INTO member_zones (user_id, time_zone) VALUES (?, ?)")
            .bind(to_integer(user.get())?)
            .bind(zone)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn remembered(&self, user: UserId) -> Result<Option<String>, ZoneStoreError> {
        let zone: Option<String> =
            sqlx::query_scalar("SELECT time_zone FROM member_zones WHERE user_id = ?")
                .bind(to_integer(user.get())?)
                .fetch_optional(&self.pool)
                .await?;
        Ok(zone)
    }
}

async fn columns(pool: &SqlitePool, table: &str) -> Result<Vec<Column>, ZoneStoreError> {
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

fn to_integer(value: u64) -> Result<i64, ZoneStoreError> {
    i64::try_from(value).map_err(|_| ZoneStoreError::OutOfRange { value })
}
