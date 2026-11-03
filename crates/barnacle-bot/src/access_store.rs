use std::collections::BTreeSet;

use sqlx::sqlite::SqlitePool;

use crate::ids::GuildId;
use crate::ids::RoleId;
use crate::solves::Column;

const ORGANISER_ROLES: &str = "organiser_roles";

type ColumnSpec = (&'static str, &'static str, bool, bool);
type TableSpec = (&'static str, &'static [ColumnSpec]);

const EXPECTED_COLUMNS: [TableSpec; 1] = [(
    ORGANISER_ROLES,
    &[
        ("guild_id", "INTEGER", true, true),
        ("role_id", "INTEGER", true, true),
    ],
)];

#[derive(Debug, thiserror::Error)]
pub enum AccessStoreError {
    #[error("the organiser access database could not be used")]
    Database(#[from] sqlx::Error),
    #[error("the organiser access database has no {table} table")]
    MissingTable { table: &'static str },
    #[error("{table} does not have the expected columns; found {found:?}")]
    UnexpectedColumns {
        table: &'static str,
        found: Vec<Column>,
    },
    #[error("{value} does not fit in a SQLite integer")]
    OutOfRange { value: u64 },
    #[error("the organiser access database holds a negative value, {value}")]
    Negative { value: i64 },
}

#[derive(Debug, Clone)]
pub struct AccessStore {
    pool: SqlitePool,
}

impl AccessStore {
    pub async fn with_pool(pool: SqlitePool) -> Result<AccessStore, AccessStoreError> {
        for (table, expected) in EXPECTED_COLUMNS {
            let found = columns(&pool, table).await?;
            if found.is_empty() {
                return Err(AccessStoreError::MissingTable { table });
            }
            if found != expected_columns(expected) {
                return Err(AccessStoreError::UnexpectedColumns { table, found });
            }
        }
        Ok(AccessStore { pool })
    }

    pub async fn set_organiser_roles(
        &self,
        guild: GuildId,
        roles: &[RoleId],
    ) -> Result<(), AccessStoreError> {
        let guild = to_integer(guild.get())?;
        let roles = roles
            .iter()
            .map(|role| to_integer(role.get()))
            .collect::<Result<BTreeSet<i64>, _>>()?;
        let mut transaction = self.pool.begin().await?;
        sqlx::query("DELETE FROM organiser_roles WHERE guild_id = ?")
            .bind(guild)
            .execute(&mut *transaction)
            .await?;
        for role in roles {
            sqlx::query("INSERT INTO organiser_roles (guild_id, role_id) VALUES (?, ?)")
                .bind(guild)
                .bind(role)
                .execute(&mut *transaction)
                .await?;
        }
        transaction.commit().await?;
        Ok(())
    }

    pub async fn organiser_roles(&self, guild: GuildId) -> Result<Vec<RoleId>, AccessStoreError> {
        let roles: Vec<i64> = sqlx::query_scalar(
            "SELECT role_id FROM organiser_roles WHERE guild_id = ? ORDER BY role_id",
        )
        .bind(to_integer(guild.get())?)
        .fetch_all(&self.pool)
        .await?;
        roles
            .into_iter()
            .map(|role| to_unsigned(role).map(RoleId::new))
            .collect()
    }
}

async fn columns(pool: &SqlitePool, table: &str) -> Result<Vec<Column>, AccessStoreError> {
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

fn to_unsigned(value: i64) -> Result<u64, AccessStoreError> {
    u64::try_from(value).map_err(|_| AccessStoreError::Negative { value })
}

fn to_integer(value: u64) -> Result<i64, AccessStoreError> {
    i64::try_from(value).map_err(|_| AccessStoreError::OutOfRange { value })
}
