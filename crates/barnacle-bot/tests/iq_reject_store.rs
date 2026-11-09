mod common;

use std::path::Path;

use barnacle_bot::ids::ChannelId;
use barnacle_bot::ids::GuildId;
use barnacle_bot::ids::RoleId;
use barnacle_bot::iq::Criterion;
use barnacle_bot::iq::CriterionKind;
use barnacle_bot::iq::JudgingMarks;
use barnacle_bot::iq::Score;
use barnacle_bot::iq_store::AwardOutcome;
use barnacle_bot::iq_store::CreateOutcome;
use barnacle_bot::iq_store::IqStore;
use barnacle_bot::iq_store::IqStoreError;
use barnacle_bot::iq_store::NewCompetition;
use barnacle_bot::iq_store::RejectOutcome;
use barnacle_guess::UserId;
use common::attendance_pool;
use jiff::civil::date;
use sqlx::sqlite::SqlitePool;

const CLI_DIRECTIVE: &str = ".bail on\n";
const MIGRATIONS: [&str; 3] = [
    "0009_iq_competition.sql",
    "0012_iq_marks_and_judge_log.sql",
    "0013_iq_players_and_rejections.sql",
];
const PLAYER: UserId = UserId::new(11);
const JUDGE: UserId = UserId::new(21);
const AT: i64 = 1_793_600_000_000;
const REASON: &str = "Wrong ship";

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

async fn store_with_competition() -> (IqStore, i64) {
    let pool = attendance_pool().await;
    for file in MIGRATIONS {
        apply(&pool, file).await;
    }
    let store = IqStore::with_pool(pool).await.unwrap();
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
            name: "Solo".to_owned(),
            description: String::new(),
            points: 5,
            kind: CriterionKind::Single,
        }],
        rules: String::new(),
        roles: vec![RoleId::new(31)],
        judges: vec![JUDGE],
        marks: JudgingMarks::default(),
        judge_log: None,
        created_by: UserId::new(100),
        created_at_ms: 1_793_500_000_000,
    };
    let id = match store.create(&new).await.unwrap() {
        CreateOutcome::Created(competition) => competition.id,
        CreateOutcome::AlreadyLive => panic!("the competition was refused"),
    };
    (store, id)
}

async fn record(store: &IqStore, id: i64, message: u64) {
    store
        .record_submission(id, message, PLAYER, &[], "Solo", 5, AT)
        .await
        .unwrap();
}

#[tokio::test]
async fn a_rejected_post_can_never_be_awarded() {
    let (store, id) = store_with_competition().await;
    record(&store, id, 500).await;
    assert_eq!(
        store
            .reject(id, 500, PLAYER, REASON, JUDGE, AT)
            .await
            .unwrap(),
        RejectOutcome::Rejected
    );
    assert_eq!(
        store.award(500, JUDGE, AT).await.unwrap(),
        AwardOutcome::Rejected
    );
    assert_eq!(store.scores(id).await.unwrap(), Vec::<Score>::new());
}

#[tokio::test]
async fn a_post_the_bot_never_recorded_can_be_rejected() {
    let (store, id) = store_with_competition().await;
    assert_eq!(
        store
            .reject(id, 501, PLAYER, REASON, JUDGE, AT)
            .await
            .unwrap(),
        RejectOutcome::Rejected
    );
    assert!(store.recorded(id, 501).await.unwrap());
}

#[tokio::test]
async fn a_rejected_post_leaves_the_offline_recheck() {
    let (store, id) = store_with_competition().await;
    record(&store, id, 500).await;
    record(&store, id, 502).await;
    store
        .reject(id, 500, PLAYER, REASON, JUDGE, AT)
        .await
        .unwrap();
    assert_eq!(store.unawarded(id, 10).await.unwrap(), vec![502]);
}

#[tokio::test]
async fn an_awarded_post_cannot_be_rejected() {
    let (store, id) = store_with_competition().await;
    record(&store, id, 500).await;
    store.award(500, JUDGE, AT).await.unwrap();
    assert_eq!(
        store
            .reject(id, 500, PLAYER, REASON, JUDGE, AT)
            .await
            .unwrap(),
        RejectOutcome::AlreadyAwarded
    );
    assert_eq!(
        store.scores(id).await.unwrap(),
        vec![Score {
            user: PLAYER,
            points: 5
        }]
    );
}

#[tokio::test]
async fn a_post_cannot_be_rejected_twice() {
    let (store, id) = store_with_competition().await;
    assert_eq!(
        store
            .reject(id, 500, PLAYER, REASON, JUDGE, AT)
            .await
            .unwrap(),
        RejectOutcome::Rejected
    );
    assert_eq!(
        store
            .reject(id, 500, PLAYER, REASON, JUDGE, AT)
            .await
            .unwrap(),
        RejectOutcome::AlreadyRejected
    );
}

#[tokio::test]
async fn a_rejection_needs_a_short_reason() {
    let (store, id) = store_with_competition().await;
    let blank = store.reject(id, 500, PLAYER, "   ", JUDGE, AT).await;
    assert!(
        matches!(blank, Err(IqStoreError::ReasonMissing)),
        "{blank:?}"
    );
    let too_long = store
        .reject(id, 500, PLAYER, &"a".repeat(201), JUDGE, AT)
        .await;
    assert!(
        matches!(too_long, Err(IqStoreError::ReasonTooLong)),
        "{too_long:?}"
    );
    let at_limit = store
        .reject(id, 500, PLAYER, &"a".repeat(200), JUDGE, AT)
        .await;
    assert!(
        matches!(at_limit, Ok(RejectOutcome::Rejected)),
        "{at_limit:?}"
    );
}
