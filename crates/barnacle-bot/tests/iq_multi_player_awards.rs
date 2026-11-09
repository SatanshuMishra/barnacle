mod common;

use std::path::Path;

use barnacle_bot::ids::ChannelId;
use barnacle_bot::ids::GuildId;
use barnacle_bot::ids::RoleId;
use barnacle_bot::iq::Criterion;
use barnacle_bot::iq::CriterionKind;
use barnacle_bot::iq::JudgingMarks;
use barnacle_bot::iq::Score;
use barnacle_bot::iq_store::AdjustOutcome;
use barnacle_bot::iq_store::AwardOutcome;
use barnacle_bot::iq_store::CreateOutcome;
use barnacle_bot::iq_store::IqStore;
use barnacle_bot::iq_store::NewCompetition;
use barnacle_bot::iq_text;
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
const MESSAGE: u64 = 500;
const POSTER: UserId = UserId::new(11);
const SECOND_PLAYER: UserId = UserId::new(12);
const THIRD_PLAYER: UserId = UserId::new(13);
const JUDGE: UserId = UserId::new(21);
const AT: i64 = 1_793_600_000_000;

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
        criteria: vec![
            Criterion {
                name: "Lockout".to_owned(),
                description: String::new(),
                points: 8,
                kind: CriterionKind::Multi,
            },
            Criterion {
                name: "Solo".to_owned(),
                description: String::new(),
                points: 5,
                kind: CriterionKind::Single,
            },
        ],
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

async fn awarded_lockout() -> (IqStore, i64, AwardOutcome) {
    let (store, id) = store_with_competition().await;
    store
        .record_submission(
            id,
            MESSAGE,
            POSTER,
            &[SECOND_PLAYER, THIRD_PLAYER],
            "Lockout",
            8,
            AT,
        )
        .await
        .unwrap();
    let outcome = store.award(MESSAGE, JUDGE, AT).await.unwrap();
    (store, id, outcome)
}

#[tokio::test]
async fn an_award_gives_every_player_the_full_points() {
    let (store, id, outcome) = awarded_lockout().await;
    assert_eq!(
        outcome,
        AwardOutcome::Awarded {
            players: vec![POSTER, SECOND_PLAYER, THIRD_PLAYER],
            points: 8,
        }
    );
    assert_eq!(
        store.scores(id).await.unwrap(),
        vec![
            Score {
                user: POSTER,
                points: 8
            },
            Score {
                user: SECOND_PLAYER,
                points: 8
            },
            Score {
                user: THIRD_PLAYER,
                points: 8
            },
        ]
    );
}

#[tokio::test]
async fn a_partners_points_count_toward_their_total() {
    let (store, id, _) = awarded_lockout().await;
    assert_eq!(
        store
            .adjust(id, SECOND_PLAYER, -9, "fix", JUDGE, AT)
            .await
            .unwrap(),
        AdjustOutcome::BelowZero { total: 8 }
    );
    assert_eq!(
        store
            .adjust(id, SECOND_PLAYER, -8, "fix", JUDGE, AT)
            .await
            .unwrap(),
        AdjustOutcome::Adjusted { total: 0 }
    );
}

#[tokio::test]
async fn forgetting_a_post_drops_its_players() {
    let (store, id) = store_with_competition().await;
    store
        .record_submission(
            id,
            MESSAGE,
            POSTER,
            &[SECOND_PLAYER, THIRD_PLAYER],
            "Lockout",
            8,
            AT,
        )
        .await
        .unwrap();
    let forgot = store.forget(id, MESSAGE).await;
    assert!(forgot.is_ok(), "{forgot:?}");
    store
        .record_submission(id, MESSAGE, POSTER, &[], "Lockout", 8, AT)
        .await
        .unwrap();
    assert_eq!(
        store.award(MESSAGE, JUDGE, AT).await.unwrap(),
        AwardOutcome::Awarded {
            players: vec![POSTER],
            points: 8,
        }
    );
}

#[test]
fn the_judge_log_names_every_player() {
    assert_eq!(
        iq_text::judge_log_award(JUDGE, &[POSTER], "Solo", 5, "link"),
        "<@21> awarded <@11> 5 points for Solo: link"
    );
    assert_eq!(
        iq_text::judge_log_award(
            JUDGE,
            &[POSTER, SECOND_PLAYER, THIRD_PLAYER],
            "Lockout",
            8,
            "link"
        ),
        "<@21> awarded <@11>, <@12> and <@13> 8 points each for Lockout: link"
    );
}
