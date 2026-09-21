mod common;

use barnacle_bot::ids::ChannelId;
use barnacle_bot::ids::GuildId;
use barnacle_bot::voice_store::Hub;
use barnacle_bot::voice_store::Room;
use barnacle_bot::voice_store::VoiceStore;
use barnacle_bot::voice_store::VoiceStoreError;
use barnacle_guess::UserId;
use common::attendance_pool;
use common::memory_pool;
use sqlx::sqlite::SqlitePool;

const VOICE_ROOMS: &str = include_str!("../../../migrations/0005_voice_rooms.sql");
const VOICE_ROOMS_DOWN: &str = include_str!("../../../migrations/0005_voice_rooms.down.sql");
const CLI_DIRECTIVE: &str = ".bail on\n";

const GUILD: GuildId = GuildId::new(1);
const OTHER_GUILD: GuildId = GuildId::new(2);
const HUB: ChannelId = ChannelId::new(10);
const SECOND_HUB: ChannelId = ChannelId::new(11);
const OTHER_HUB: ChannelId = ChannelId::new(20);
const ADMIN: UserId = UserId::new(100);
const AKI: UserId = UserId::new(200);
const CREATED_AT: u64 = 1_700_000_000_000;

async fn voice_pool() -> SqlitePool {
    let pool = attendance_pool().await;
    sqlx::raw_sql(VOICE_ROOMS.trim_start_matches(CLI_DIRECTIVE))
        .execute(&pool)
        .await
        .unwrap();
    pool
}

async fn store() -> VoiceStore {
    VoiceStore::with_pool(voice_pool().await).await.unwrap()
}

fn hub(channel: ChannelId, guild: GuildId, room_name: &str, created_at_ms: u64) -> Hub {
    Hub {
        channel,
        guild,
        room_name: room_name.to_owned(),
        created_by: ADMIN,
        created_at_ms,
    }
}

fn room(channel: u64, guild: GuildId, room_name: &str, number: u32) -> Room {
    Room {
        channel: ChannelId::new(channel),
        guild,
        hub: HUB,
        room_name: room_name.to_owned(),
        number,
        owner: AKI,
        created_at_ms: CREATED_AT,
        empty_since_ms: None,
    }
}

#[tokio::test]
async fn a_database_without_the_voice_tables_is_refused() {
    let error = VoiceStore::with_pool(attendance_pool().await)
        .await
        .err()
        .unwrap();
    assert!(matches!(
        error,
        VoiceStoreError::MissingTable { table: "vc_hubs" }
    ));
    assert_eq!(
        error.to_string(),
        "the voice room database has no vc_hubs table"
    );
    let pool = memory_pool().await;
    sqlx::raw_sql(
        "CREATE TABLE vc_hubs (channel_id INTEGER PRIMARY KEY, guild_id INTEGER NOT NULL, room_name TEXT NOT NULL, created_by INTEGER NOT NULL, created_at_ms INTEGER NOT NULL) STRICT",
    )
    .execute(&pool)
    .await
    .unwrap();
    assert!(matches!(
        VoiceStore::with_pool(pool).await,
        Err(VoiceStoreError::MissingTable { table: "vc_rooms" })
    ));
}

#[tokio::test]
async fn unexpected_voice_columns_are_reported() {
    let pool = memory_pool().await;
    sqlx::raw_sql(
        "CREATE TABLE vc_hubs (channel_id INTEGER PRIMARY KEY, name TEXT NOT NULL) STRICT",
    )
    .execute(&pool)
    .await
    .unwrap();
    let error = VoiceStore::with_pool(pool).await.err().unwrap();
    assert!(matches!(
        &error,
        VoiceStoreError::UnexpectedColumns {
            table: "vc_hubs",
            found
        } if found.len() == 2
    ));
    assert!(error.to_string().starts_with(
        "vc_hubs does not have the expected columns; found [Column { name: \"channel_id\""
    ));
}

#[tokio::test]
async fn a_database_without_the_numbered_index_is_refused() {
    let pool = voice_pool().await;
    sqlx::raw_sql("DROP INDEX vc_rooms_numbered")
        .execute(&pool)
        .await
        .unwrap();
    let error = VoiceStore::with_pool(pool).await.err().unwrap();
    assert!(matches!(
        error,
        VoiceStoreError::MissingIndex {
            index: "vc_rooms_numbered"
        }
    ));
    assert_eq!(
        error.to_string(),
        "the voice room database has no vc_rooms_numbered index"
    );
}

#[tokio::test]
async fn the_voice_migration_runs_once_and_rolls_back() {
    let pool = voice_pool().await;
    assert!(
        sqlx::raw_sql(VOICE_ROOMS.trim_start_matches(CLI_DIRECTIVE))
            .execute(&pool)
            .await
            .is_err()
    );
    sqlx::raw_sql("ROLLBACK").execute(&pool).await.unwrap();
    let applied: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM schema_migrations WHERE name = '0005_voice_rooms'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(applied, 1);
    sqlx::raw_sql(VOICE_ROOMS_DOWN.trim_start_matches(CLI_DIRECTIVE))
        .execute(&pool)
        .await
        .unwrap();
    assert!(matches!(
        VoiceStore::with_pool(pool.clone()).await,
        Err(VoiceStoreError::MissingTable { table: "vc_hubs" })
    ));
    let applied: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM schema_migrations WHERE name = '0005_voice_rooms'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(applied, 0);
    sqlx::raw_sql(VOICE_ROOMS.trim_start_matches(CLI_DIRECTIVE))
        .execute(&pool)
        .await
        .unwrap();
    assert!(VoiceStore::with_pool(pool).await.is_ok());
}

#[tokio::test]
async fn a_room_name_outside_one_to_ninety_characters_is_refused_by_the_table() {
    let store = store().await;
    assert!(
        store
            .add_hub(&hub(HUB, GUILD, "", CREATED_AT))
            .await
            .is_err()
    );
    assert!(
        store
            .add_hub(&hub(HUB, GUILD, &"c".repeat(91), CREATED_AT))
            .await
            .is_err()
    );
    store
        .add_hub(&hub(HUB, GUILD, &"c".repeat(90), CREATED_AT))
        .await
        .unwrap();
}

#[tokio::test]
async fn a_hub_is_recorded_renamed_and_removed() {
    let store = store().await;
    let recorded = hub(HUB, GUILD, "cb", CREATED_AT);
    store.add_hub(&recorded).await.unwrap();
    assert_eq!(store.hub(HUB).await.unwrap(), Some(recorded.clone()));
    assert_eq!(store.hub(SECOND_HUB).await.unwrap(), None);
    assert!(store.rename_rooms(HUB, "scrim").await.unwrap());
    assert_eq!(
        store.hub(HUB).await.unwrap(),
        Some(Hub {
            room_name: "scrim".to_owned(),
            ..recorded
        })
    );
    assert!(!store.rename_rooms(SECOND_HUB, "scrim").await.unwrap());
    assert!(store.remove_hub(HUB).await.unwrap());
    assert!(!store.remove_hub(HUB).await.unwrap());
    assert_eq!(store.hub(HUB).await.unwrap(), None);
}

#[tokio::test]
async fn hubs_in_lists_one_servers_hubs_oldest_first() {
    let store = store().await;
    let later = hub(HUB, GUILD, "cb", CREATED_AT + 1);
    let earlier = hub(SECOND_HUB, GUILD, "scrim", CREATED_AT);
    let elsewhere = hub(OTHER_HUB, OTHER_GUILD, "cb", CREATED_AT);
    store.add_hub(&later).await.unwrap();
    store.add_hub(&earlier).await.unwrap();
    store.add_hub(&elsewhere).await.unwrap();
    assert_eq!(
        store.hubs_in(GUILD).await.unwrap(),
        vec![earlier.clone(), later.clone()]
    );
    assert_eq!(
        store.hubs_in(OTHER_GUILD).await.unwrap(),
        vec![elsewhere.clone()]
    );
    assert_eq!(store.hubs().await.unwrap(), vec![earlier, elsewhere, later]);
}

#[tokio::test]
async fn next_number_is_one_above_the_highest_open_room_of_that_server_and_name() {
    let store = store().await;
    assert_eq!(store.next_number(GUILD, "cb").await.unwrap(), 1);
    store.add_room(&room(500, GUILD, "cb", 1)).await.unwrap();
    store.add_room(&room(501, GUILD, "cb", 3)).await.unwrap();
    store.add_room(&room(502, GUILD, "scrim", 7)).await.unwrap();
    store
        .add_room(&room(503, OTHER_GUILD, "cb", 9))
        .await
        .unwrap();
    assert_eq!(store.next_number(GUILD, "cb").await.unwrap(), 4);
    assert_eq!(store.next_number(GUILD, "scrim").await.unwrap(), 8);
    assert_eq!(store.next_number(OTHER_GUILD, "cb").await.unwrap(), 10);
    assert_eq!(store.next_number(OTHER_GUILD, "scrim").await.unwrap(), 1);
}

#[tokio::test]
async fn two_open_rooms_cannot_share_a_name_in_one_server() {
    let store = store().await;
    store.add_room(&room(500, GUILD, "cb", 1)).await.unwrap();
    assert!(store.add_room(&room(501, GUILD, "cb", 1)).await.is_err());
    store
        .add_room(&room(502, OTHER_GUILD, "cb", 1))
        .await
        .unwrap();
}

#[tokio::test]
async fn a_room_is_recorded_counted_and_removed() {
    let store = store().await;
    let first = room(500, GUILD, "cb", 1);
    let second = Room {
        created_at_ms: CREATED_AT + 1,
        ..room(501, GUILD, "cb", 2)
    };
    let foreign = Room {
        hub: SECOND_HUB,
        ..room(502, GUILD, "scrim", 1)
    };
    store.add_room(&second).await.unwrap();
    store.add_room(&first).await.unwrap();
    store.add_room(&foreign).await.unwrap();
    assert_eq!(
        store.room(ChannelId::new(500)).await.unwrap(),
        Some(first.clone())
    );
    assert_eq!(store.room(ChannelId::new(599)).await.unwrap(), None);
    assert_eq!(
        store.rooms().await.unwrap(),
        vec![first, foreign, second.clone()]
    );
    assert_eq!(store.open_rooms_of(HUB).await.unwrap(), 2);
    assert_eq!(store.open_rooms_of(SECOND_HUB).await.unwrap(), 1);
    assert_eq!(store.open_rooms_of(OTHER_HUB).await.unwrap(), 0);
    assert!(store.remove_room(ChannelId::new(500)).await.unwrap());
    assert!(!store.remove_room(ChannelId::new(500)).await.unwrap());
    assert_eq!(store.open_rooms_of(HUB).await.unwrap(), 1);
    assert_eq!(store.room(ChannelId::new(501)).await.unwrap(), Some(second));
}

#[tokio::test]
async fn an_empty_mark_keeps_its_first_moment_until_the_room_is_occupied() {
    let store = store().await;
    let channel = ChannelId::new(500);
    store.add_room(&room(500, GUILD, "cb", 1)).await.unwrap();
    store.mark_empty(channel, CREATED_AT + 10).await.unwrap();
    store.mark_empty(channel, CREATED_AT + 20).await.unwrap();
    assert_eq!(
        store.room(channel).await.unwrap().unwrap().empty_since_ms,
        Some(CREATED_AT + 10)
    );
    store.mark_occupied(channel).await.unwrap();
    assert_eq!(
        store.room(channel).await.unwrap().unwrap().empty_since_ms,
        None
    );
    store.mark_empty(channel, CREATED_AT + 30).await.unwrap();
    assert_eq!(
        store.room(channel).await.unwrap().unwrap().empty_since_ms,
        Some(CREATED_AT + 30)
    );
}

#[tokio::test]
async fn marking_a_channel_that_is_not_a_room_changes_nothing() {
    let store = store().await;
    store.add_room(&room(500, GUILD, "cb", 1)).await.unwrap();
    store.mark_empty(HUB, CREATED_AT).await.unwrap();
    store.mark_occupied(HUB).await.unwrap();
    assert_eq!(
        store.rooms().await.unwrap(),
        vec![room(500, GUILD, "cb", 1)]
    );
}
