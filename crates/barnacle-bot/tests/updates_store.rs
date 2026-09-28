mod common;

use barnacle_bot::ids::ChannelId;
use barnacle_bot::ids::GuildId;
use barnacle_bot::ids::Ping;
use barnacle_bot::ids::RoleId;
use barnacle_bot::updates_store::UpdateSettings;
use barnacle_bot::updates_store::UpdatesStore;
use barnacle_bot::updates_store::UpdatesStoreError;
use common::attendance_pool;
use sqlx::sqlite::SqlitePool;

const VOICE_ROOMS: &str = include_str!("../../../migrations/0005_voice_rooms.sql");
const BOT_UPDATES: &str = include_str!("../../../migrations/0007_bot_updates.sql");
const BOT_UPDATES_DOWN: &str = include_str!("../../../migrations/0007_bot_updates.down.sql");
const CLI_DIRECTIVE: &str = ".bail on\n";

const GUILD: GuildId = GuildId::new(1);
const OTHER_GUILD: GuildId = GuildId::new(2);
const CHANNEL: ChannelId = ChannelId::new(10);
const OTHER_CHANNEL: ChannelId = ChannelId::new(11);
const ROLE: RoleId = RoleId::new(123);

async fn updates_pool() -> SqlitePool {
    let pool = attendance_pool().await;
    for migration in [VOICE_ROOMS, BOT_UPDATES] {
        sqlx::raw_sql(migration.trim_start_matches(CLI_DIRECTIVE))
            .execute(&pool)
            .await
            .unwrap();
    }
    pool
}

async fn store() -> UpdatesStore {
    UpdatesStore::with_pool(updates_pool().await).await.unwrap()
}

async fn ledger_rows(pool: &SqlitePool) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM schema_migrations WHERE name = '0007_bot_updates'")
        .fetch_one(pool)
        .await
        .unwrap()
}

fn settings(
    guild: GuildId,
    channel: ChannelId,
    ping: Option<Ping>,
    last_version: Option<&str>,
) -> Option<UpdateSettings> {
    Some(UpdateSettings {
        guild,
        channel,
        ping,
        last_version: last_version.map(str::to_owned),
    })
}

#[tokio::test]
async fn a_servers_update_settings_round_trip_and_replace() {
    let pool = updates_pool().await;
    let store = UpdatesStore::with_pool(pool.clone()).await.unwrap();
    store
        .save(GUILD, CHANNEL, Some(Ping::Role(ROLE)))
        .await
        .unwrap();
    assert_eq!(
        store.settings(GUILD).await.unwrap(),
        settings(GUILD, CHANNEL, Some(Ping::Role(ROLE)), None)
    );
    store
        .save(GUILD, OTHER_CHANNEL, Some(Ping::Everyone))
        .await
        .unwrap();
    assert_eq!(
        store.settings(GUILD).await.unwrap(),
        settings(GUILD, OTHER_CHANNEL, Some(Ping::Everyone), None)
    );
    let stored: Option<i64> =
        sqlx::query_scalar("SELECT ping_id FROM bot_updates WHERE guild_id = 1")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(stored, Some(1));
    store.save(GUILD, CHANNEL, None).await.unwrap();
    assert_eq!(
        store.settings(GUILD).await.unwrap(),
        settings(GUILD, CHANNEL, None, None)
    );
    assert_eq!(store.settings(OTHER_GUILD).await.unwrap(), None);
}

#[tokio::test]
async fn the_last_sent_version_is_kept_per_server() {
    let store = store().await;
    store
        .save(GUILD, CHANNEL, Some(Ping::Role(ROLE)))
        .await
        .unwrap();
    store.save(OTHER_GUILD, OTHER_CHANNEL, None).await.unwrap();
    store.mark_sent(GUILD, "0.2.0").await.unwrap();
    assert_eq!(
        store.settings(GUILD).await.unwrap(),
        settings(GUILD, CHANNEL, Some(Ping::Role(ROLE)), Some("0.2.0"))
    );
    assert_eq!(
        store.settings(OTHER_GUILD).await.unwrap(),
        settings(OTHER_GUILD, OTHER_CHANNEL, None, None)
    );
    store.save(GUILD, OTHER_CHANNEL, None).await.unwrap();
    assert_eq!(
        store.settings(GUILD).await.unwrap(),
        settings(GUILD, OTHER_CHANNEL, None, Some("0.2.0"))
    );
}

#[tokio::test]
async fn the_updates_migration_applies_once_and_its_down_file_reverses_it() {
    let pool = updates_pool().await;
    let store = UpdatesStore::with_pool(pool.clone()).await.unwrap();
    assert_eq!(ledger_rows(&pool).await, 1);
    store.save(GUILD, CHANNEL, None).await.unwrap();
    assert!(
        sqlx::raw_sql(BOT_UPDATES.trim_start_matches(CLI_DIRECTIVE))
            .execute(&pool)
            .await
            .is_err()
    );
    sqlx::raw_sql("ROLLBACK").execute(&pool).await.unwrap();
    assert_eq!(ledger_rows(&pool).await, 1);
    assert_eq!(
        store.settings(GUILD).await.unwrap(),
        settings(GUILD, CHANNEL, None, None)
    );
    sqlx::raw_sql(BOT_UPDATES_DOWN.trim_start_matches(CLI_DIRECTIVE))
        .execute(&pool)
        .await
        .unwrap();
    let error = UpdatesStore::with_pool(pool.clone()).await.err().unwrap();
    assert!(matches!(error, UpdatesStoreError::MissingTable));
    assert_eq!(
        error.to_string(),
        "the updates database has no bot_updates table"
    );
    assert_eq!(ledger_rows(&pool).await, 0);
    sqlx::raw_sql(BOT_UPDATES.trim_start_matches(CLI_DIRECTIVE))
        .execute(&pool)
        .await
        .unwrap();
    assert!(UpdatesStore::with_pool(pool).await.is_ok());
}

#[tokio::test]
async fn unexpected_update_columns_are_reported() {
    let pool = common::memory_pool().await;
    sqlx::raw_sql(
        "CREATE TABLE bot_updates (guild_id INTEGER PRIMARY KEY, channel_id INTEGER NOT NULL) STRICT",
    )
    .execute(&pool)
    .await
    .unwrap();
    assert!(matches!(
        UpdatesStore::with_pool(pool).await,
        Err(UpdatesStoreError::UnexpectedColumns { .. })
    ));
}

#[tokio::test]
async fn a_version_outside_five_to_thirty_two_characters_is_refused_by_the_table() {
    let store = store().await;
    store.save(GUILD, CHANNEL, None).await.unwrap();
    assert!(store.mark_sent(GUILD, "0.2").await.is_err());
    assert!(store.mark_sent(GUILD, &"1".repeat(33)).await.is_err());
    store.mark_sent(GUILD, &"1".repeat(32)).await.unwrap();
}
