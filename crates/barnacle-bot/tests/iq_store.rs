mod common;

use std::path::Path;

use barnacle_bot::ids::ChannelId;
use barnacle_bot::ids::GuildId;
use barnacle_bot::ids::RoleId;
use barnacle_bot::iq::Criterion;
use barnacle_bot::iq::JudgingReaction;
use barnacle_bot::iq::Score;
use barnacle_bot::iq::Status;
use barnacle_bot::iq_store::AwardOutcome;
use barnacle_bot::iq_store::Competition;
use barnacle_bot::iq_store::ConfigChange;
use barnacle_bot::iq_store::CreateOutcome;
use barnacle_bot::iq_store::IqStore;
use barnacle_bot::iq_store::NewCompetition;
use barnacle_guess::UserId;
use common::attendance_pool;
use jiff::civil::date;
use sqlx::sqlite::SqlitePool;

const IQ_MIGRATION: &str = "0009_iq_competition.sql";
const CLI_DIRECTIVE: &str = ".bail on\n";
const GUILD: GuildId = GuildId::new(5);
const ADMIN: UserId = UserId::new(100);
const PLAYER: UserId = UserId::new(11);
const SECOND_PLAYER: UserId = UserId::new(12);
const THIRD_PLAYER: UserId = UserId::new(13);
const JUDGE: UserId = UserId::new(21);
const SECOND_JUDGE: UserId = UserId::new(22);
const CREATED_AT_MS: i64 = 1_793_500_000_000;
const POSTED_AT_MS: i64 = 1_793_600_000_000;
const AWARDED_AT_MS: i64 = 1_793_600_060_000;
const RULES: &str = "Post your own battles only.\nOne post per battle.";

async fn iq_pool() -> SqlitePool {
    let pool = attendance_pool().await;
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../migrations")
        .join(IQ_MIGRATION);
    let script = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("{} is readable: {error}", path.display()));
    sqlx::raw_sql(sqlx::AssertSqlSafe(
        script.trim_start_matches(CLI_DIRECTIVE).to_owned(),
    ))
    .execute(&pool)
    .await
    .unwrap();
    pool
}

async fn store() -> IqStore {
    IqStore::with_pool(iq_pool().await).await.unwrap()
}

fn criterion(name: &str, description: &str, points: u32) -> Criterion {
    Criterion {
        name: name.to_owned(),
        description: description.to_owned(),
        points,
    }
}

fn criteria() -> Vec<Criterion> {
    vec![
        criterion("Double Down", "Two kills with one salvo.", 2),
        criterion("Drunken Kraken", "Five kills in one battle.", 5),
    ]
}

fn competition(name: &str, created_at_ms: i64) -> NewCompetition {
    NewCompetition {
        guild: GUILD,
        name: name.to_owned(),
        first_day: date(2026, 11, 1),
        last_day: date(2026, 11, 30),
        zone: "America/Edmonton".to_owned(),
        category: ChannelId::new(40),
        announcements: ChannelId::new(41),
        submissions: ChannelId::new(42),
        criteria: criteria(),
        rules: RULES.to_owned(),
        roles: vec![RoleId::new(31)],
        judges: vec![JUDGE, SECOND_JUDGE],
        reaction: JudgingReaction::Trophy,
        created_by: ADMIN,
        created_at_ms,
    }
}

fn reaction_for(number: u32) -> JudgingReaction {
    match number % 4 {
        0 => JudgingReaction::Trophy,
        1 => JudgingReaction::Star,
        2 => JudgingReaction::Hundred,
        _ => JudgingReaction::Crown,
    }
}

fn numbered(number: u32) -> NewCompetition {
    NewCompetition {
        criteria: vec![criterion(
            &format!("Criterion {number}"),
            &format!("Description {number}"),
            number,
        )],
        rules: format!("Rules {number}"),
        roles: vec![RoleId::new(300 + u64::from(number))],
        judges: vec![UserId::new(200 + u64::from(number))],
        reaction: reaction_for(number),
        ..competition(
            &format!("IQ {number}"),
            CREATED_AT_MS + i64::from(number) * 1_000,
        )
    }
}

async fn created(store: &IqStore, new: &NewCompetition) -> Competition {
    match store.create(new).await.unwrap() {
        CreateOutcome::Created(competition) => competition,
        CreateOutcome::AlreadyLive => panic!("{} is refused as a second live one", new.name),
    }
}

async fn running(store: &IqStore) -> Competition {
    let november = created(store, &competition("November IQ", CREATED_AT_MS)).await;
    store
        .set_status(november.id, Status::Running)
        .await
        .unwrap();
    november
}

async fn kraken_posted(store: &IqStore, id: i64, message: u64, user: UserId) {
    store
        .record_submission(id, message, user, "Drunken Kraken", 5, POSTED_AT_MS)
        .await
        .unwrap();
}

fn points_of(scores: &[Score], user: UserId) -> i64 {
    scores
        .iter()
        .filter(|score| score.user == user)
        .map(|score| score.points)
        .sum()
}

#[tokio::test]
async fn the_first_judge_reaction_awards_the_points() {
    let store = store().await;
    let november = running(&store).await;
    kraken_posted(&store, november.id, 900, PLAYER).await;
    assert_eq!(
        store.award(900, JUDGE, AWARDED_AT_MS).await.unwrap(),
        AwardOutcome::Awarded {
            user: PLAYER,
            points: 5,
        }
    );
    assert_eq!(
        points_of(&store.scores(november.id).await.unwrap(), PLAYER),
        5
    );
}

#[tokio::test]
async fn a_second_judge_reaction_awards_nothing_more() {
    let store = store().await;
    let november = running(&store).await;
    kraken_posted(&store, november.id, 900, PLAYER).await;
    store.award(900, JUDGE, AWARDED_AT_MS).await.unwrap();
    assert_eq!(
        store
            .award(900, SECOND_JUDGE, AWARDED_AT_MS + 1_000)
            .await
            .unwrap(),
        AwardOutcome::AlreadyAwarded
    );
    assert_eq!(
        points_of(&store.scores(november.id).await.unwrap(), PLAYER),
        5
    );
}

#[tokio::test]
async fn a_judge_may_award_their_own_post() {
    let store = store().await;
    let november = running(&store).await;
    store
        .record_submission(november.id, 901, JUDGE, "Double Down", 2, POSTED_AT_MS)
        .await
        .unwrap();
    assert_eq!(
        store.award(901, JUDGE, AWARDED_AT_MS).await.unwrap(),
        AwardOutcome::Awarded {
            user: JUDGE,
            points: 2,
        }
    );
}

#[tokio::test]
async fn a_reaction_on_a_message_that_is_not_a_valid_submission_awards_nothing() {
    let store = store().await;
    let november = running(&store).await;
    kraken_posted(&store, november.id, 900, PLAYER).await;
    store.award(900, JUDGE, AWARDED_AT_MS).await.unwrap();
    let before = store.scores(november.id).await.unwrap();
    assert_eq!(
        store.award(555, JUDGE, AWARDED_AT_MS).await.unwrap(),
        AwardOutcome::NotASubmission
    );
    assert_eq!(store.scores(november.id).await.unwrap(), before);
}

#[tokio::test]
async fn a_reaction_after_completion_awards_nothing() {
    let store = store().await;
    let november = running(&store).await;
    kraken_posted(&store, november.id, 902, SECOND_PLAYER).await;
    store
        .set_status(november.id, Status::Completed)
        .await
        .unwrap();
    assert_eq!(
        store.award(902, JUDGE, AWARDED_AT_MS).await.unwrap(),
        AwardOutcome::Closed
    );
    assert_eq!(
        points_of(&store.scores(november.id).await.unwrap(), SECOND_PLAYER),
        0
    );
}

#[tokio::test]
async fn recording_a_submission_twice_keeps_the_first() {
    let store = store().await;
    let november = running(&store).await;
    store
        .record_submission(
            november.id,
            903,
            THIRD_PLAYER,
            "Double Down",
            2,
            POSTED_AT_MS,
        )
        .await
        .unwrap();
    store
        .record_submission(
            november.id,
            903,
            THIRD_PLAYER,
            "Drunken Kraken",
            5,
            POSTED_AT_MS + 1_000,
        )
        .await
        .unwrap();
    assert_eq!(
        store.award(903, JUDGE, AWARDED_AT_MS).await.unwrap(),
        AwardOutcome::Awarded {
            user: THIRD_PLAYER,
            points: 2,
        }
    );
}

#[tokio::test]
async fn points_are_kept_as_they_were_when_posted() {
    let store = store().await;
    let november = running(&store).await;
    kraken_posted(&store, november.id, 900, PLAYER).await;
    let edited = vec![
        criterion("Double Down", "Two kills with one salvo.", 2),
        criterion("Drunken Kraken", "Five kills in one battle.", 7),
    ];
    store
        .update_config(&ConfigChange {
            competition: november.id,
            criteria: edited,
            rules: RULES.to_owned(),
            roles: vec![RoleId::new(31)],
            judges: vec![JUDGE, SECOND_JUDGE],
            reaction: JudgingReaction::Trophy,
        })
        .await
        .unwrap();
    assert_eq!(
        store.award(900, JUDGE, AWARDED_AT_MS).await.unwrap(),
        AwardOutcome::Awarded {
            user: PLAYER,
            points: 5,
        }
    );
}

#[tokio::test]
async fn the_last_seen_mark_only_rises() {
    let store = store().await;
    let november = running(&store).await;
    store.mark_seen(november.id, 1000).await.unwrap();
    store.mark_seen(november.id, 900).await.unwrap();
    assert_eq!(store.last_seen(november.id).await.unwrap(), Some(1000));
}

#[tokio::test]
async fn resuming_marks_the_pause_as_seen() {
    let resume_snowflake: u64 = (1_767_225_600_000 - 1_420_070_400_000) << 22;
    let store = store().await;
    let november = running(&store).await;
    store
        .set_status(november.id, Status::Stopped)
        .await
        .unwrap();
    store.mark_seen(november.id, 1000).await.unwrap();
    store.resume(november.id, 1_767_225_600).await.unwrap();
    let live = store
        .live(GUILD)
        .await
        .unwrap()
        .expect("a live competition");
    assert_eq!(live.status, Status::Running);
    assert_eq!(live.last_seen, Some(resume_snowflake));
    assert_eq!(
        store.last_seen(november.id).await.unwrap(),
        Some(resume_snowflake)
    );
}

#[tokio::test]
async fn a_second_live_competition_is_refused() {
    let store = store().await;
    created(&store, &competition("November IQ", CREATED_AT_MS)).await;
    let second = store
        .create(&competition("December IQ", CREATED_AT_MS + 1_000))
        .await
        .unwrap();
    assert!(matches!(second, CreateOutcome::AlreadyLive));
}

#[tokio::test]
async fn clean_up_archives_and_a_new_start_begins_at_zero() {
    let store = store().await;
    let november = running(&store).await;
    kraken_posted(&store, november.id, 900, PLAYER).await;
    store.award(900, JUDGE, AWARDED_AT_MS).await.unwrap();
    store.archive(november.id).await.unwrap();
    let december = store
        .create(&competition("December IQ", CREATED_AT_MS + 1_000))
        .await
        .unwrap();
    let CreateOutcome::Created(december) = december else {
        panic!("the next competition is created once the last is archived");
    };
    assert!(store.scores(december.id).await.unwrap().is_empty());
    assert_eq!(
        points_of(&store.scores(november.id).await.unwrap(), PLAYER),
        5
    );
}

#[tokio::test]
async fn templates_are_the_ten_newest_competitions() {
    let store = store().await;
    for number in 1..=12 {
        let made = created(&store, &numbered(number)).await;
        store.archive(made.id).await.unwrap();
    }
    let templates = store.templates(GUILD).await.unwrap();
    let names: Vec<String> = templates
        .iter()
        .map(|template| template.name.clone())
        .collect();
    let newest_first: Vec<String> = (3..=12)
        .rev()
        .map(|number| format!("IQ {number}"))
        .collect();
    assert_eq!(names, newest_first);
    for (template, number) in templates.iter().zip((3..=12).rev()) {
        let expected = numbered(number);
        assert_eq!(template.criteria, expected.criteria, "{}", template.name);
        assert_eq!(template.rules, expected.rules, "{}", template.name);
        assert_eq!(template.roles, expected.roles, "{}", template.name);
        assert_eq!(template.judges, expected.judges, "{}", template.name);
        assert_eq!(template.reaction, expected.reaction, "{}", template.name);
    }
}

#[tokio::test]
async fn a_first_competition_has_no_templates() {
    let store = store().await;
    assert!(store.templates(GUILD).await.unwrap().is_empty());
}

#[tokio::test]
async fn an_adjustment_counts_in_the_scores() {
    let store = store().await;
    let november = running(&store).await;
    kraken_posted(&store, november.id, 900, PLAYER).await;
    store.award(900, JUDGE, AWARDED_AT_MS).await.unwrap();
    store
        .adjust(
            november.id,
            PLAYER,
            -5,
            Some("The same battle was posted twice"),
            JUDGE,
            AWARDED_AT_MS + 1_000,
        )
        .await
        .unwrap();
    store
        .adjust(
            november.id,
            SECOND_PLAYER,
            3,
            None,
            JUDGE,
            AWARDED_AT_MS + 2_000,
        )
        .await
        .unwrap();
    let scores = store.scores(november.id).await.unwrap();
    assert_eq!(points_of(&scores, PLAYER), 0);
    assert_eq!(points_of(&scores, SECOND_PLAYER), 3);
}

#[tokio::test]
async fn reverting_a_completion_returns_it_to_its_old_status() {
    let store = store().await;
    let november = running(&store).await;
    store
        .set_status(november.id, Status::Completed)
        .await
        .unwrap();
    assert!(
        store
            .revert_completed(november.id, Status::Running)
            .await
            .unwrap()
    );
    let live = store
        .live(GUILD)
        .await
        .unwrap()
        .expect("a live competition");
    assert_eq!(live.status, Status::Running);
}

#[tokio::test]
async fn reverting_a_completion_leaves_an_archived_competition_alone() {
    let store = store().await;
    let november = running(&store).await;
    store
        .set_status(november.id, Status::Completed)
        .await
        .unwrap();
    store.archive(november.id).await.unwrap();
    assert!(
        !store
            .revert_completed(november.id, Status::Running)
            .await
            .unwrap()
    );
    assert!(store.live(GUILD).await.unwrap().is_none());
}
