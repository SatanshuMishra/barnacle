use sqlx::sqlite::SqlitePool;

use crate::ids::ChannelId;
use crate::ids::GuildId;
use crate::ids::Ping;
use crate::ids::RoleId;
use crate::solves::Column;

const BOT_UPDATES: &str = "bot_updates";

type ColumnSpec = (&'static str, &'static str, bool, bool);

const EXPECTED_COLUMNS: [ColumnSpec; 4] = [
    ("guild_id", "INTEGER", false, true),
    ("channel_id", "INTEGER", true, false),
    ("ping_id", "INTEGER", false, false),
    ("last_version", "TEXT", false, false),
];

const SETTINGS: &str =
    "SELECT guild_id, channel_id, ping_id, last_version FROM bot_updates WHERE guild_id = ?";

type SettingsRow = (i64, i64, Option<i64>, Option<String>);

#[derive(Debug, thiserror::Error)]
pub enum UpdatesStoreError {
    #[error("the updates database could not be used")]
    Database(#[from] sqlx::Error),
    #[error("the updates database has no bot_updates table")]
    MissingTable,
    #[error("bot_updates does not have the expected columns; found {found:?}")]
    UnexpectedColumns { found: Vec<Column> },
    #[error("{value} does not fit in a SQLite integer")]
    OutOfRange { value: u128 },
    #[error("the updates database holds a negative value, {value}")]
    Negative { value: i64 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateSettings {
    pub guild: GuildId,
    pub channel: ChannelId,
    pub ping: Option<Ping>,
    pub last_version: Option<String>,
}

#[derive(Debug, Clone)]
pub struct UpdatesStore {
    pool: SqlitePool,
}

impl UpdatesStore {
    pub async fn with_pool(pool: SqlitePool) -> Result<Self, UpdatesStoreError> {
        let found = columns(&pool).await?;
        if found.is_empty() {
            return Err(UpdatesStoreError::MissingTable);
        }
        if found != expected_columns() {
            return Err(UpdatesStoreError::UnexpectedColumns { found });
        }
        Ok(Self { pool })
    }

    pub async fn save(
        &self,
        guild: GuildId,
        channel: ChannelId,
        ping: Option<Ping>,
    ) -> Result<(), UpdatesStoreError> {
        let ping_id = ping.map(|ping| match ping {
            Ping::Everyone => guild.get(),
            Ping::Role(role) => role.get(),
        });
        sqlx::query(
            "INSERT INTO bot_updates (guild_id, channel_id, ping_id) VALUES (?, ?, ?) ON CONFLICT (guild_id) DO UPDATE SET channel_id = excluded.channel_id, ping_id = excluded.ping_id",
        )
        .bind(to_integer(guild.get().into())?)
        .bind(to_integer(channel.get().into())?)
        .bind(ping_id.map(|id| to_integer(id.into())).transpose()?)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn settings(
        &self,
        guild: GuildId,
    ) -> Result<Option<UpdateSettings>, UpdatesStoreError> {
        let row: Option<SettingsRow> = sqlx::query_as(SETTINGS)
            .bind(to_integer(guild.get().into())?)
            .fetch_optional(&self.pool)
            .await?;
        row.map(to_settings).transpose()
    }

    pub async fn mark_sent(&self, guild: GuildId, version: &str) -> Result<(), UpdatesStoreError> {
        sqlx::query("UPDATE bot_updates SET last_version = ? WHERE guild_id = ?")
            .bind(version)
            .bind(to_integer(guild.get().into())?)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

async fn columns(pool: &SqlitePool) -> Result<Vec<Column>, UpdatesStoreError> {
    let rows: Vec<(String, String, i64, i64)> =
        sqlx::query_as("SELECT name, type, \"notnull\", pk FROM pragma_table_info(?) ORDER BY cid")
            .bind(BOT_UPDATES)
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

fn expected_columns() -> Vec<Column> {
    EXPECTED_COLUMNS
        .iter()
        .map(|(name, kind, not_null, primary_key)| Column {
            name: (*name).to_owned(),
            kind: (*kind).to_owned(),
            not_null: *not_null,
            primary_key: *primary_key,
        })
        .collect()
}

fn to_settings(row: SettingsRow) -> Result<UpdateSettings, UpdatesStoreError> {
    let (guild, channel, ping, last_version) = row;
    let guild = GuildId::new(to_unsigned(guild)?);
    Ok(UpdateSettings {
        guild,
        channel: ChannelId::new(to_unsigned(channel)?),
        ping: ping
            .map(|id| to_unsigned(id).map(|role| Ping::of(guild, RoleId::new(role))))
            .transpose()?,
        last_version,
    })
}

fn to_unsigned(value: i64) -> Result<u64, UpdatesStoreError> {
    u64::try_from(value).map_err(|_| UpdatesStoreError::Negative { value })
}

fn to_integer(value: u128) -> Result<i64, UpdatesStoreError> {
    i64::try_from(value).map_err(|_| UpdatesStoreError::OutOfRange { value })
}
