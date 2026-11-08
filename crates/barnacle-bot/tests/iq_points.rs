mod common;

use std::path::Path;

use barnacle_bot::ids::ChannelId;
use barnacle_bot::ids::GuildId;
use barnacle_bot::ids::RoleId;
use barnacle_bot::iq::Criterion;
use barnacle_bot::iq::CriterionKind;
use barnacle_bot::iq::JudgingMarks;
use barnacle_bot::iq_store::AdjustOutcome;
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
const PLAYER: UserId = UserId::new(31);
const JUDGE: UserId = UserId::new(21);
const AT_MS: i64 = 1_793_600_000_000;

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

async fn running_competition() -> (IqStore, i64) {
    let pool = attendance_pool().await;
    apply(&pool, IQ_MIGRATION).await;
    apply(&pool, MARKS_MIGRATION).await;
    apply(&pool, PLAYERS_MIGRATION).await;
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
            name: "Drunken Kraken".to_owned(),
            description: "Five kills in one battle.".to_owned(),
            points: 5,
            kind: CriterionKind::Single,
        }],
        rules: "Post your own battles only.".to_owned(),
        roles: vec![RoleId::new(31)],
        judges: vec![JUDGE],
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
    (store, competition.id)
}

async fn player_points(store: &IqStore, id: i64) -> i64 {
    let scores = store.scores(id).await.unwrap();
    scores
        .iter()
        .find(|score| score.user == PLAYER)
        .unwrap_or_else(|| panic!("user 31 has a score: {scores:?}"))
        .points
}

async fn seeded_at_fifty() -> (IqStore, i64) {
    let (store, id) = running_competition().await;
    let seeded = store.adjust(id, PLAYER, 50, "Seed", JUDGE, AT_MS).await;
    assert!(
        matches!(seeded, Ok(AdjustOutcome::Adjusted { total: 50 })),
        "{seeded:?}"
    );
    (store, id)
}

#[tokio::test]
async fn a_removal_past_zero_is_refused() {
    let (store, id) = running_competition().await;
    store
        .adjust(id, UserId::new(31), 50, "Seed", UserId::new(21), AT_MS)
        .await
        .unwrap();
    let refused = store
        .adjust(
            id,
            UserId::new(31),
            -100,
            "Wrong screenshot",
            UserId::new(21),
            AT_MS,
        )
        .await;
    assert!(
        matches!(refused, Ok(AdjustOutcome::BelowZero { total: 50 })),
        "{refused:?}"
    );
    assert_eq!(player_points(&store, id).await, 50);
}

#[tokio::test]
async fn a_removal_down_to_zero_is_allowed() {
    let (store, id) = seeded_at_fifty().await;
    let emptied = store.adjust(id, PLAYER, -50, "Reset", JUDGE, AT_MS).await;
    assert!(
        matches!(emptied, Ok(AdjustOutcome::Adjusted { total: 0 })),
        "{emptied:?}"
    );
    let refused = store.adjust(id, PLAYER, -1, "One more", JUDGE, AT_MS).await;
    assert!(
        matches!(refused, Ok(AdjustOutcome::BelowZero { total: 0 })),
        "{refused:?}"
    );
}

#[tokio::test]
async fn awarded_points_count_toward_the_total() {
    let (store, id) = running_competition().await;
    let recorded = store
        .record_submission(id, 1000, PLAYER, "Drunken Kraken", 5, AT_MS)
        .await
        .unwrap();
    assert!(recorded);
    store.award(1000, JUDGE, AT_MS).await.unwrap();
    let too_many = store.adjust(id, PLAYER, -6, "Too many", JUDGE, AT_MS).await;
    assert!(
        matches!(too_many, Ok(AdjustOutcome::BelowZero { total: 5 })),
        "{too_many:?}"
    );
    let all = store
        .adjust(id, PLAYER, -5, "All of it", JUDGE, AT_MS)
        .await;
    assert!(
        matches!(all, Ok(AdjustOutcome::Adjusted { total: 0 })),
        "{all:?}"
    );
    let added = store.adjust(id, PLAYER, 3, "Bonus", JUDGE, AT_MS).await;
    assert!(
        matches!(added, Ok(AdjustOutcome::Adjusted { total: 3 })),
        "{added:?}"
    );
}

#[tokio::test]
async fn two_removals_at_once_cannot_both_pass() {
    let (store, id) = seeded_at_fifty().await;
    let (first, second) = tokio::join!(
        store.adjust(id, PLAYER, -30, "Twice", JUDGE, AT_MS),
        store.adjust(id, PLAYER, -30, "Twice", JUDGE, AT_MS)
    );
    let one_each = matches!(
        (&first, &second),
        (
            Ok(AdjustOutcome::Adjusted { total: 20 }),
            Ok(AdjustOutcome::BelowZero { total: 20 })
        ) | (
            Ok(AdjustOutcome::BelowZero { total: 20 }),
            Ok(AdjustOutcome::Adjusted { total: 20 })
        )
    );
    assert!(one_each, "{first:?} and {second:?}");
    assert_eq!(player_points(&store, id).await, 20);
}

#[tokio::test]
async fn a_blank_reason_is_refused() {
    let (store, id) = running_competition().await;
    let spaces = store.adjust(id, PLAYER, 5, "   ", JUDGE, AT_MS).await;
    assert!(
        matches!(spaces, Err(IqStoreError::ReasonMissing)),
        "{spaces:?}"
    );
    let empty = store.adjust(id, PLAYER, 5, "", JUDGE, AT_MS).await;
    assert!(
        matches!(empty, Err(IqStoreError::ReasonMissing)),
        "{empty:?}"
    );
    let long = "x".repeat(201);
    let too_long = store.adjust(id, PLAYER, 5, &long, JUDGE, AT_MS).await;
    assert!(
        matches!(too_long, Err(IqStoreError::ReasonTooLong)),
        "{too_long:?}"
    );
}
