mod common;

use std::path::Path;

use barnacle_bot::ids::ChannelId;
use barnacle_bot::ids::GuildId;
use barnacle_bot::ids::RoleId;
use barnacle_bot::iq::Criterion;
use barnacle_bot::iq::CriterionKind;
use barnacle_bot::iq::JudgingMarks;
use barnacle_bot::iq::Status;
use barnacle_bot::iq::adjustment;
use barnacle_bot::iq_store::Competition;
use barnacle_bot::iq_store::CreateOutcome;
use barnacle_bot::iq_store::IqStore;
use barnacle_bot::iq_store::IqStoreError;
use barnacle_bot::iq_store::NewCompetition;
use barnacle_guess::UserId;
use common::attendance_pool;
use jiff::civil::date;
use sqlx::sqlite::SqlitePool;

const IQ_MIGRATION: &str = "0009_iq_competition.sql";
const MARKS_MIGRATION: &str = "0012_iq_marks_and_judge_log.sql";
const PLAYERS_MIGRATION: &str = "0013_iq_players_and_rejections.sql";
const CLI_DIRECTIVE: &str = ".bail on\n";
const DISCORD_EPOCH_MS: i64 = 1_420_070_400_000;
const LAST_SNOWFLAKE_SECOND: i64 = 3_619_093_655;

async fn apply(pool: &SqlitePool, file: &str) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../migrations")
        .join(file);
    let script = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("{} is readable: {error}", path.display()));
    sqlx::raw_sql(sqlx::AssertSqlSafe(
        script.trim_start_matches(CLI_DIRECTIVE).to_owned(),
    ))
    .execute(pool)
    .await
    .unwrap();
}

async fn iq_pool() -> SqlitePool {
    let pool = attendance_pool().await;
    apply(&pool, IQ_MIGRATION).await;
    apply(&pool, MARKS_MIGRATION).await;
    apply(&pool, PLAYERS_MIGRATION).await;
    pool
}

async fn stopped_competition() -> (IqStore, Competition) {
    let store = IqStore::with_pool(iq_pool().await).await.unwrap();
    let new = NewCompetition {
        guild: GuildId::new(5),
        name: "November IQ".to_owned(),
        first_day: date(2026, 11, 1),
        last_day: date(2026, 11, 30),
        zone: "America/Edmonton".to_owned(),
        category: ChannelId::new(40),
        announcements: ChannelId::new(41),
        submissions: ChannelId::new(42),
        criteria: vec![Criterion {
            name: "Drunken Kraken".to_owned(),
            description: "Five kills in one battle.".to_owned(),
            points: 5,
            kind: CriterionKind::Single,
        }],
        rules: "Post your own battles only.".to_owned(),
        roles: vec![RoleId::new(31)],
        judges: vec![UserId::new(21)],
        marks: JudgingMarks::default(),
        judge_log: None,
        created_by: UserId::new(100),
        created_at_ms: 1_793_500_000_000,
    };
    let competition = match store.create(&new).await.unwrap() {
        CreateOutcome::Created(competition) => competition,
        CreateOutcome::AlreadyLive => {
            panic!("the first competition is refused as a second live one")
        }
    };
    store
        .set_status(competition.id, Status::Stopped)
        .await
        .unwrap();
    (store, competition)
}

#[test]
fn an_adjustment_reaches_a_thousand_points_either_way_and_no_further() {
    assert_eq!(adjustment(1000), Some(1000));
    assert_eq!(adjustment(-1000), Some(-1000));
    assert_eq!(adjustment(1001), None);
    assert_eq!(adjustment(-1001), None);
    assert_eq!(adjustment(0), None);
}

#[tokio::test]
async fn resuming_at_the_last_storable_second_marks_its_snowflake() {
    let (store, competition) = stopped_competition().await;
    store
        .resume(competition.id, LAST_SNOWFLAKE_SECOND)
        .await
        .unwrap();
    let since_epoch = u64::try_from(LAST_SNOWFLAKE_SECOND * 1_000 - DISCORD_EPOCH_MS).unwrap();
    assert_eq!(
        store.last_seen(competition.id).await.unwrap(),
        Some(since_epoch << 22)
    );
}

#[tokio::test]
async fn resuming_past_the_last_storable_second_is_refused() {
    let (store, competition) = stopped_competition().await;
    let refused = store
        .resume(competition.id, LAST_SNOWFLAKE_SECOND + 1)
        .await;
    assert!(
        matches!(refused, Err(IqStoreError::NotASnowflakeMoment { at_unix }) if at_unix == LAST_SNOWFLAKE_SECOND + 1),
        "{refused:?}"
    );
}
