use std::path::Path;
use std::time::Duration;

use barnacle_guess::UserId;
use sqlx::sqlite::SqliteConnectOptions;
use sqlx::sqlite::SqlitePool;
use sqlx::sqlite::SqlitePoolOptions;

use crate::ids::ChannelId;
use crate::ids::GuildId;
use crate::solves::Column;

const BUSY_TIMEOUT: Duration = Duration::from_secs(5);

const VC_HUBS: &str = "vc_hubs";
const VC_ROOMS: &str = "vc_rooms";
const NUMBERED_INDEX: &str = "vc_rooms_numbered";

type ColumnSpec = (&'static str, &'static str, bool, bool);
type TableSpec = (&'static str, &'static [ColumnSpec]);

const EXPECTED_COLUMNS: [TableSpec; 2] = [
    (
        VC_HUBS,
        &[
            ("channel_id", "INTEGER", false, true),
            ("guild_id", "INTEGER", true, false),
            ("room_name", "TEXT", true, false),
            ("created_by", "INTEGER", true, false),
            ("created_at_ms", "INTEGER", true, false),
        ],
    ),
    (
        VC_ROOMS,
        &[
            ("channel_id", "INTEGER", false, true),
            ("guild_id", "INTEGER", true, false),
            ("hub_id", "INTEGER", true, false),
            ("room_name", "TEXT", true, false),
            ("number", "INTEGER", true, false),
            ("owner_id", "INTEGER", true, false),
            ("created_at_ms", "INTEGER", true, false),
            ("empty_since_ms", "INTEGER", false, false),
        ],
    ),
];

const HUB_BY_CHANNEL: &str = "SELECT channel_id, guild_id, room_name, created_by, created_at_ms FROM vc_hubs WHERE channel_id = ?";
const HUBS: &str = "SELECT channel_id, guild_id, room_name, created_by, created_at_ms FROM vc_hubs ORDER BY created_at_ms, channel_id";
const HUBS_IN_GUILD: &str = "SELECT channel_id, guild_id, room_name, created_by, created_at_ms FROM vc_hubs WHERE guild_id = ? ORDER BY created_at_ms, channel_id";
const ROOM_BY_CHANNEL: &str = "SELECT channel_id, guild_id, hub_id, room_name, number, owner_id, created_at_ms, empty_since_ms FROM vc_rooms WHERE channel_id = ?";
const ROOMS: &str = "SELECT channel_id, guild_id, hub_id, room_name, number, owner_id, created_at_ms, empty_since_ms FROM vc_rooms ORDER BY created_at_ms, channel_id";

type HubRow = (i64, i64, String, i64, i64);

type RoomRow = (i64, i64, i64, String, i64, i64, i64, Option<i64>);

#[derive(Debug, thiserror::Error)]
pub enum VoiceStoreError {
    #[error("the voice room database could not be used")]
    Database(#[from] sqlx::Error),
    #[error("the voice room database has no {table} table")]
    MissingTable { table: &'static str },
    #[error("{table} does not have the expected columns; found {found:?}")]
    UnexpectedColumns {
        table: &'static str,
        found: Vec<Column>,
    },
    #[error("the voice room database has no {index} index")]
    MissingIndex { index: &'static str },
    #[error("{value} does not fit in a SQLite integer")]
    OutOfRange { value: u128 },
    #[error("the voice room database holds a negative value, {value}")]
    Negative { value: i64 },
    #[error("the voice room database holds a user ID below 1, {value}")]
    InvalidUser { value: i64 },
    #[error("the voice room database holds {value}, which is not a room number")]
    BadNumber { value: i64 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hub {
    pub channel: ChannelId,
    pub guild: GuildId,
    pub room_name: String,
    pub created_by: UserId,
    pub created_at_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Room {
    pub channel: ChannelId,
    pub guild: GuildId,
    pub hub: ChannelId,
    pub room_name: String,
    pub number: u32,
    pub owner: UserId,
    pub created_at_ms: u64,
    pub empty_since_ms: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct VoiceStore {
    pool: SqlitePool,
}

impl VoiceStore {
    pub async fn open(path: &Path) -> Result<Self, VoiceStoreError> {
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

    pub async fn with_pool(pool: SqlitePool) -> Result<Self, VoiceStoreError> {
        for (table, expected) in EXPECTED_COLUMNS {
            let found = columns(&pool, table).await?;
            if found.is_empty() {
                return Err(VoiceStoreError::MissingTable { table });
            }
            if found != expected_columns(expected) {
                return Err(VoiceStoreError::UnexpectedColumns { table, found });
            }
        }
        let index: Option<i64> =
            sqlx::query_scalar("SELECT 1 FROM sqlite_master WHERE type = 'index' AND name = ?")
                .bind(NUMBERED_INDEX)
                .fetch_optional(&pool)
                .await?;
        if index.is_none() {
            return Err(VoiceStoreError::MissingIndex {
                index: NUMBERED_INDEX,
            });
        }
        Ok(Self { pool })
    }

    pub async fn add_hub(&self, hub: &Hub) -> Result<(), VoiceStoreError> {
        sqlx::query(
            "INSERT INTO vc_hubs (channel_id, guild_id, room_name, created_by, created_at_ms) VALUES (?, ?, ?, ?, ?)",
        )
        .bind(to_integer(hub.channel.get().into())?)
        .bind(to_integer(hub.guild.get().into())?)
        .bind(&hub.room_name)
        .bind(to_integer(hub.created_by.get().into())?)
        .bind(to_integer(hub.created_at_ms.into())?)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn hub(&self, channel: ChannelId) -> Result<Option<Hub>, VoiceStoreError> {
        let row: Option<HubRow> = sqlx::query_as(HUB_BY_CHANNEL)
            .bind(to_integer(channel.get().into())?)
            .fetch_optional(&self.pool)
            .await?;
        row.map(to_hub).transpose()
    }

    pub async fn hubs(&self) -> Result<Vec<Hub>, VoiceStoreError> {
        let rows: Vec<HubRow> = sqlx::query_as(HUBS).fetch_all(&self.pool).await?;
        rows.into_iter().map(to_hub).collect()
    }

    pub async fn hubs_in(&self, guild: GuildId) -> Result<Vec<Hub>, VoiceStoreError> {
        let rows: Vec<HubRow> = sqlx::query_as(HUBS_IN_GUILD)
            .bind(to_integer(guild.get().into())?)
            .fetch_all(&self.pool)
            .await?;
        rows.into_iter().map(to_hub).collect()
    }

    pub async fn rename_rooms(
        &self,
        hub: ChannelId,
        room_name: &str,
    ) -> Result<bool, VoiceStoreError> {
        let renamed = sqlx::query("UPDATE vc_hubs SET room_name = ? WHERE channel_id = ?")
            .bind(room_name)
            .bind(to_integer(hub.get().into())?)
            .execute(&self.pool)
            .await?;
        Ok(renamed.rows_affected() > 0)
    }

    pub async fn remove_hub(&self, hub: ChannelId) -> Result<bool, VoiceStoreError> {
        let removed = sqlx::query("DELETE FROM vc_hubs WHERE channel_id = ?")
            .bind(to_integer(hub.get().into())?)
            .execute(&self.pool)
            .await?;
        Ok(removed.rows_affected() > 0)
    }

    pub async fn next_number(
        &self,
        guild: GuildId,
        room_name: &str,
    ) -> Result<u32, VoiceStoreError> {
        let next: i64 = sqlx::query_scalar(
            "SELECT COALESCE(MAX(number), 0) + 1 FROM vc_rooms WHERE guild_id = ? AND room_name = ?",
        )
        .bind(to_integer(guild.get().into())?)
        .bind(room_name)
        .fetch_one(&self.pool)
        .await?;
        to_number(next)
    }

    pub async fn add_room(&self, room: &Room) -> Result<(), VoiceStoreError> {
        sqlx::query(
            "INSERT INTO vc_rooms (channel_id, guild_id, hub_id, room_name, number, owner_id, created_at_ms, empty_since_ms) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(to_integer(room.channel.get().into())?)
        .bind(to_integer(room.guild.get().into())?)
        .bind(to_integer(room.hub.get().into())?)
        .bind(&room.room_name)
        .bind(i64::from(room.number))
        .bind(to_integer(room.owner.get().into())?)
        .bind(to_integer(room.created_at_ms.into())?)
        .bind(
            room.empty_since_ms
                .map(|since| to_integer(since.into()))
                .transpose()?,
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn room(&self, channel: ChannelId) -> Result<Option<Room>, VoiceStoreError> {
        let row: Option<RoomRow> = sqlx::query_as(ROOM_BY_CHANNEL)
            .bind(to_integer(channel.get().into())?)
            .fetch_optional(&self.pool)
            .await?;
        row.map(to_room).transpose()
    }

    pub async fn rooms(&self) -> Result<Vec<Room>, VoiceStoreError> {
        let rows: Vec<RoomRow> = sqlx::query_as(ROOMS).fetch_all(&self.pool).await?;
        rows.into_iter().map(to_room).collect()
    }

    pub async fn open_rooms_of(&self, hub: ChannelId) -> Result<usize, VoiceStoreError> {
        let open: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM vc_rooms WHERE hub_id = ?")
            .bind(to_integer(hub.get().into())?)
            .fetch_one(&self.pool)
            .await?;
        usize::try_from(open).map_err(|_| VoiceStoreError::Negative { value: open })
    }

    pub async fn mark_empty(&self, room: ChannelId, at_ms: u64) -> Result<(), VoiceStoreError> {
        sqlx::query(
            "UPDATE vc_rooms SET empty_since_ms = ? WHERE channel_id = ? AND empty_since_ms IS NULL",
        )
        .bind(to_integer(at_ms.into())?)
        .bind(to_integer(room.get().into())?)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn mark_occupied(&self, room: ChannelId) -> Result<(), VoiceStoreError> {
        sqlx::query("UPDATE vc_rooms SET empty_since_ms = NULL WHERE channel_id = ?")
            .bind(to_integer(room.get().into())?)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn remove_room(&self, room: ChannelId) -> Result<bool, VoiceStoreError> {
        let removed = sqlx::query("DELETE FROM vc_rooms WHERE channel_id = ?")
            .bind(to_integer(room.get().into())?)
            .execute(&self.pool)
            .await?;
        Ok(removed.rows_affected() > 0)
    }
}

async fn columns(pool: &SqlitePool, table: &str) -> Result<Vec<Column>, VoiceStoreError> {
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

fn to_hub(row: HubRow) -> Result<Hub, VoiceStoreError> {
    let (channel, guild, room_name, created_by, created_at_ms) = row;
    Ok(Hub {
        channel: ChannelId::new(to_unsigned(channel)?),
        guild: GuildId::new(to_unsigned(guild)?),
        room_name,
        created_by: to_user(created_by)?,
        created_at_ms: to_unsigned(created_at_ms)?,
    })
}

fn to_room(row: RoomRow) -> Result<Room, VoiceStoreError> {
    let (channel, guild, hub, room_name, number, owner, created_at_ms, empty_since_ms) = row;
    Ok(Room {
        channel: ChannelId::new(to_unsigned(channel)?),
        guild: GuildId::new(to_unsigned(guild)?),
        hub: ChannelId::new(to_unsigned(hub)?),
        room_name,
        number: to_number(number)?,
        owner: to_user(owner)?,
        created_at_ms: to_unsigned(created_at_ms)?,
        empty_since_ms: empty_since_ms.map(to_unsigned).transpose()?,
    })
}

fn to_user(value: i64) -> Result<UserId, VoiceStoreError> {
    u64::try_from(value)
        .ok()
        .filter(|id| *id > 0)
        .map(UserId::new)
        .ok_or(VoiceStoreError::InvalidUser { value })
}

fn to_number(value: i64) -> Result<u32, VoiceStoreError> {
    u32::try_from(value)
        .ok()
        .filter(|number| *number > 0)
        .ok_or(VoiceStoreError::BadNumber { value })
}

fn to_unsigned(value: i64) -> Result<u64, VoiceStoreError> {
    u64::try_from(value).map_err(|_| VoiceStoreError::Negative { value })
}

fn to_integer(value: u128) -> Result<i64, VoiceStoreError> {
    i64::try_from(value).map_err(|_| VoiceStoreError::OutOfRange { value })
}
