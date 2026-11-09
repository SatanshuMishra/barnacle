mod common;

use std::path::Path;

use barnacle_bot::ids::ChannelId;
use barnacle_bot::ids::GuildId;
use barnacle_bot::ids::RoleId;
use barnacle_bot::iq::Criterion;
use barnacle_bot::iq::CriterionKind;
use barnacle_bot::iq::JudgingMarks;
use barnacle_bot::iq::Status;
use barnacle_bot::iq_store::BoardPost;
use barnacle_bot::iq_store::Competition;
use barnacle_bot::iq_store::CreateOutcome;
use barnacle_bot::iq_store::IqStore;
use barnacle_bot::iq_store::NewCompetition;
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

async fn store() -> IqStore {
    let pool = attendance_pool().await;
    for file in MIGRATIONS {
        apply(&pool, file).await;
    }
    IqStore::with_pool(pool).await.unwrap()
}

fn competition_in(guild: u64, channels: u64) -> NewCompetition {
    NewCompetition {
        guild: GuildId::new(guild),
        name: format!("IQ {guild}"),
        first_day: date(2026, 11, 1),
        last_day: date(2026, 11, 30),
        zone: "America/Edmonton".to_owned(),
        category: ChannelId::new(channels),
        announcements: ChannelId::new(channels + 1),
        submissions: ChannelId::new(channels + 2),
        criteria: vec![Criterion {
            name: "Solo".to_owned(),
            description: String::new(),
            points: 5,
            kind: CriterionKind::Single,
        }],
        rules: String::new(),
        roles: vec![RoleId::new(31)],
        judges: vec![UserId::new(21)],
        marks: JudgingMarks::default(),
        judge_log: None,
        created_by: UserId::new(100),
        created_at_ms: 1_793_500_000_000,
    }
}

async fn created(store: &IqStore, new: &NewCompetition) -> Competition {
    match store.create(new).await.unwrap() {
        CreateOutcome::Created(competition) => competition,
        CreateOutcome::AlreadyLive => panic!("{} was refused", new.name),
    }
}

#[tokio::test]
async fn a_mark_is_claimed_once() {
    let store = store().await;
    let id = created(&store, &competition_in(5, 40)).await.id;
    assert!(store.claim_board_mark(id, 1).await.unwrap());
    assert!(!store.claim_board_mark(id, 1).await.unwrap());
    assert!(store.claim_board_mark(id, 2).await.unwrap());
}

#[tokio::test]
async fn a_claim_keeps_the_last_printed_scores() {
    let store = store().await;
    let id = created(&store, &competition_in(5, 40)).await.id;
    assert!(store.claim_board_mark(id, 1).await.unwrap());
    store.record_board_post(id, "11:5", 900).await.unwrap();
    assert!(store.claim_board_mark(id, 2).await.unwrap());
    assert_eq!(
        store.board_post(id).await.unwrap(),
        Some(BoardPost {
            mark: 2,
            digest: "11:5".to_owned(),
            message: Some(900),
        })
    );
}

#[tokio::test]
async fn only_running_competitions_are_printed() {
    let store = store().await;
    let running = created(&store, &competition_in(5, 40)).await;
    let stopped = created(&store, &competition_in(6, 50)).await;
    store.set_status(stopped.id, Status::Stopped).await.unwrap();
    assert_eq!(store.running().await.unwrap(), vec![running]);
}
