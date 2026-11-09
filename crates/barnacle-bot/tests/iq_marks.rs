mod common;

use std::path::Path;

use barnacle_bot::ids::ChannelId;
use barnacle_bot::ids::GuildId;
use barnacle_bot::ids::RoleId;
use barnacle_bot::iq;
use barnacle_bot::iq::Criterion;
use barnacle_bot::iq::CriterionKind;
use barnacle_bot::iq::JudgingMarks;
use barnacle_bot::iq::Mark;
use barnacle_bot::iq::MarksRefusal;
use barnacle_bot::iq_store::AwardOutcome;
use barnacle_bot::iq_store::Competition;
use barnacle_bot::iq_store::CreateOutcome;
use barnacle_bot::iq_store::IqStore;
use barnacle_bot::iq_store::NewCompetition;
use barnacle_guess::UserId;
use common::attendance_pool;
use jiff::civil::date;
use sqlx::sqlite::SqlitePool;

const IQ_MIGRATION: &str = "0009_iq_competition.sql";
const MARKS_MIGRATION: &str = "0012_iq_marks_and_judge_log.sql";
const PLAYERS_MIGRATION: &str = "0013_iq_players_and_rejections.sql";
const CLI_DIRECTIVE: &str = ".bail on\n";
const RED_X: &str = "\u{274C}";
const TROPHY: &str = "\u{1F3C6}";
const GREEN_CHECK: &str = "\u{2705}";
const STAR: &str = "\u{2B50}";
const KRAKEN: &str = "<:kraken:42>";
const JUDGE: UserId = UserId::new(21);
const AT_MS: i64 = 1_793_600_000_000;

fn mark(text: &str) -> Mark {
    Mark::parse(text).unwrap_or_else(|| panic!("{text} is a mark"))
}

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

async fn marks_pool() -> SqlitePool {
    let pool = attendance_pool().await;
    apply(&pool, IQ_MIGRATION).await;
    apply(&pool, MARKS_MIGRATION).await;
    apply(&pool, PLAYERS_MIGRATION).await;
    pool
}

fn november(marks: JudgingMarks, judge_log: Option<ChannelId>) -> NewCompetition {
    NewCompetition {
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
        marks,
        judge_log,
        created_by: UserId::new(100),
        created_at_ms: 1_793_500_000_000,
    }
}

async fn created(store: &IqStore, new: &NewCompetition) -> Competition {
    match store.create(new).await.unwrap() {
        CreateOutcome::Created(competition) => competition,
        CreateOutcome::AlreadyLive => {
            panic!("the first competition is refused as a second live one")
        }
    }
}

async fn with_submissions() -> (IqStore, i64) {
    let store = IqStore::with_pool(marks_pool().await).await.unwrap();
    let competition = created(&store, &november(JudgingMarks::default(), None)).await;
    for (message, user) in [(10, 30), (11, 31), (12, 32)] {
        let recorded = store
            .record_submission(
                competition.id,
                message,
                UserId::new(user),
                &[],
                "Drunken Kraken",
                5,
                AT_MS,
            )
            .await
            .unwrap();
        assert!(recorded, "submission {message} is recorded");
    }
    store.mark_seen(competition.id, 12).await.unwrap();
    (store, competition.id)
}

#[test]
fn a_custom_emoji_is_read_from_its_discord_form() {
    let kraken = Mark::parse("<:kraken:123456789012345678>");
    assert!(
        matches!(
            &kraken,
            Some(Mark::Custom { id: 123_456_789_012_345_678, name, animated: false }) if name == "kraken"
        ),
        "{kraken:?}"
    );
    let spin = Mark::parse("<a:spin:42>");
    assert!(
        matches!(
            &spin,
            Some(Mark::Custom {
                id: 42,
                animated: true,
                ..
            })
        ),
        "{spin:?}"
    );
    let trophy = Mark::parse("\u{1F3C6}");
    assert!(trophy.is_some());
    assert_eq!(Mark::parse("hello"), None);
    assert_eq!(kraken.unwrap().text(), "<:kraken:123456789012345678>");
    assert_eq!(spin.unwrap().text(), "<a:spin:42>");
    assert_eq!(trophy.unwrap().text(), "\u{1F3C6}");
}

#[test]
fn the_default_marks_are_x_trophy_and_check() {
    let marks = JudgingMarks::default();
    assert_eq!(marks.denied.text(), "\u{274C}");
    assert_eq!(marks.awarded.text(), "\u{1F3C6}");
    assert_eq!(marks.confirm.text(), "\u{2705}");
}

#[test]
fn two_fields_with_the_same_emoji_are_refused() {
    assert_eq!(
        JudgingMarks::new(mark(RED_X), mark(GREEN_CHECK), mark(GREEN_CHECK)).err(),
        Some(MarksRefusal::Repeated)
    );
    assert_eq!(
        JudgingMarks::new(mark(TROPHY), mark(TROPHY), mark(GREEN_CHECK)).err(),
        Some(MarksRefusal::Repeated)
    );
    assert_eq!(
        JudgingMarks::new(mark(RED_X), mark(KRAKEN), mark("<:old:42>")).err(),
        Some(MarksRefusal::Repeated)
    );
    let distinct = JudgingMarks::new(mark(RED_X), mark(KRAKEN), mark(GREEN_CHECK));
    assert!(distinct.is_ok(), "{distinct:?}");
}

#[test]
fn only_a_judges_awarded_emoji_counts() {
    let defaults = JudgingMarks::default();
    assert!(iq::counts_as_award(&mark(TROPHY), &defaults, true));
    assert!(!iq::counts_as_award(&mark(STAR), &defaults, true));
    assert!(!iq::counts_as_award(&mark(TROPHY), &defaults, false));
    let kraken = JudgingMarks::new(mark(RED_X), mark(KRAKEN), mark(GREEN_CHECK)).unwrap();
    assert!(iq::counts_as_award(&mark("<:renamed:42>"), &kraken, true));
}

#[tokio::test]
async fn a_competition_keeps_its_three_marks_and_judge_log() {
    let store = IqStore::with_pool(marks_pool().await).await.unwrap();
    let marks = JudgingMarks::new(mark(RED_X), mark(KRAKEN), mark(GREEN_CHECK)).unwrap();
    let competition = created(&store, &november(marks, Some(ChannelId::new(43)))).await;
    let live = store
        .live(GuildId::new(5))
        .await
        .unwrap()
        .expect("the competition is live");
    assert_eq!(live.marks.denied, mark(RED_X));
    assert_eq!(live.marks.awarded, mark(KRAKEN));
    assert_eq!(live.marks.confirm, mark(GREEN_CHECK));
    assert_eq!(live.judge_log, Some(ChannelId::new(43)));
    store
        .set_judge_log(competition.id, ChannelId::new(44))
        .await
        .unwrap();
    let moved = store
        .live(GuildId::new(5))
        .await
        .unwrap()
        .expect("the competition is still live");
    assert_eq!(moved.judge_log, Some(ChannelId::new(44)));
}

#[tokio::test]
async fn an_existing_competition_keeps_its_judging_emoji() {
    let pool = attendance_pool().await;
    apply(&pool, IQ_MIGRATION).await;
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO iq_competitions (guild_id, name, first_day, last_day, time_zone, status, category_id, announcements_id, submissions_id, judging_reaction, rules, last_seen_message, created_by, created_at_ms) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?) RETURNING id",
    )
    .bind(5_i64)
    .bind("October IQ")
    .bind("2026-10-01")
    .bind("2026-10-31")
    .bind("America/Edmonton")
    .bind("running")
    .bind(40_i64)
    .bind(41_i64)
    .bind(42_i64)
    .bind("star")
    .bind("Post your own battles only.")
    .bind(None::<i64>)
    .bind(100_i64)
    .bind(1_790_000_000_000_i64)
    .fetch_one(&pool)
    .await
    .unwrap();
    apply(&pool, MARKS_MIGRATION).await;
    apply(&pool, PLAYERS_MIGRATION).await;
    let store = IqStore::with_pool(pool).await.unwrap();
    let competition = store
        .competition(id)
        .await
        .unwrap()
        .expect("the competition survives the migration");
    assert_eq!(competition.marks.denied, mark(RED_X));
    assert_eq!(competition.marks.awarded, mark("\u{2B50}"));
    assert_eq!(competition.marks.confirm, mark(GREEN_CHECK));
    assert_eq!(competition.judge_log, None);
}

#[tokio::test]
async fn unawarded_submissions_include_ones_seen_before() {
    let (store, id) = with_submissions().await;
    store.award(11, JUDGE, AT_MS).await.unwrap();
    assert_eq!(store.unawarded(id, 500).await.unwrap(), vec![12, 10]);
}

#[tokio::test]
async fn the_rescan_stops_at_its_limit() {
    let (store, id) = with_submissions().await;
    assert_eq!(store.unawarded(id, 2).await.unwrap(), vec![12, 11]);
}

#[tokio::test]
async fn a_deleted_post_leaves_the_rescan() {
    let (store, id) = with_submissions().await;
    assert_eq!(
        store.award(11, JUDGE, AT_MS).await.unwrap(),
        AwardOutcome::Awarded {
            players: vec![UserId::new(31)],
            points: 5
        }
    );
    store.forget(id, 12).await.unwrap();
    store.forget(id, 11).await.unwrap();
    assert_eq!(store.unawarded(id, 500).await.unwrap(), vec![10]);
    let scores = store.scores(id).await.unwrap();
    let player = scores
        .iter()
        .find(|score| score.user == UserId::new(31))
        .unwrap_or_else(|| panic!("user 31 has a score: {scores:?}"));
    assert_eq!(player.points, 5);
}
