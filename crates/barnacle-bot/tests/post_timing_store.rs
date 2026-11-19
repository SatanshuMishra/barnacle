mod common;

use std::path::Path;
use std::sync::Arc;

use barnacle_bot::attendance::Signups;
use barnacle_bot::attendance_store::Attendance;
use barnacle_bot::attendance_store::CreateOutcome;
use barnacle_bot::attendance_store::EditOutcome;
use barnacle_bot::attendance_store::Event;
use barnacle_bot::attendance_store::EventChange;
use barnacle_bot::attendance_store::NewEvent;
use barnacle_bot::attendance_store::PostState;
use barnacle_bot::ids::ChannelId;
use barnacle_bot::ids::GuildId;
use barnacle_bot::schedule::CleanUpDelay;
use barnacle_bot::schedule::PostLead;
use barnacle_bot::schedule::Schedule;
use barnacle_bot::schedule::Span;
use barnacle_bot::schedule::parse_day;
use barnacle_bot::startup;
use barnacle_bot::startup::StartupError;
use barnacle_guess::UserId;
use common::fakes::FakeBoard;
use jiff::civil::Date;
use sqlx::sqlite::SqliteConnectOptions;
use sqlx::sqlite::SqlitePool;
use sqlx::sqlite::SqlitePoolOptions;

const GUILD: GuildId = GuildId::new(1);
const CHANNEL: ChannelId = ChannelId::new(10);
const MANAGER: UserId = UserId::new(100);
const S1: i64 = 1_792_020_600;
const S2: i64 = 1_792_107_000;
const CLI_DIRECTIVE: &str = ".bail on\n";
const THROUGH_0013: [&str; 13] = [
    common::MIGRATION,
    common::CB_MIGRATION,
    common::CB_CONTROLS,
    common::CB_HARNESS,
    include_str!("../../../migrations/0005_voice_rooms.sql"),
    common::CB_PING_ROLES,
    include_str!("../../../migrations/0007_bot_updates.sql"),
    common::CB_EVENTS,
    include_str!("../../../migrations/0009_iq_competition.sql"),
    include_str!("../../../migrations/0010_organiser_roles.sql"),
    include_str!("../../../migrations/0011_member_zones.sql"),
    include_str!("../../../migrations/0012_iq_marks_and_judge_log.sql"),
    include_str!("../../../migrations/0013_iq_players_and_rejections.sql"),
];

fn day(text: &str) -> Date {
    parse_day(text).unwrap()
}

fn millis(now_unix: i64) -> u64 {
    u64::try_from(now_unix).unwrap() * 1_000
}

fn lead(days: u8) -> PostLead {
    PostLead::from_days(days).unwrap()
}

fn delay(minutes: u16) -> CleanUpDelay {
    CleanUpDelay::from_minutes(minutes).unwrap()
}

fn proposal(first: &str, last: &str, schedule: Schedule, created_at_unix: i64) -> NewEvent {
    NewEvent {
        guild: GUILD,
        channel: CHANNEL,
        name: "Scrim Nights".to_owned(),
        span: Span::new(day(first), day(last), schedule).unwrap(),
        created_by: MANAGER,
        created_at_ms: millis(created_at_unix),
        ping_roles: Vec::new(),
    }
}

async fn created(store: &Attendance, new: &NewEvent) -> Event {
    match store.create_event(new).await.unwrap() {
        CreateOutcome::Created(event) => event,
        CreateOutcome::NameTaken => panic!("the event was not stored"),
    }
}

async fn stored(store: &Attendance, id: i64) -> Event {
    store.event(id).await.unwrap().unwrap()
}

fn timing(event: &Event) -> (u8, u16) {
    let schedule = event.span.schedule();
    (schedule.post_lead().days(), schedule.clean_up().minutes())
}

async fn five_days_and_two_hours(store: &Attendance) -> Event {
    created(
        store,
        &proposal(
            "2026-10-14",
            "2026-10-18",
            Schedule::clan_battles()
                .with_post_lead(lead(5))
                .with_clean_up(delay(120)),
            S1 - 8 * 86_400,
        ),
    )
    .await
}

async fn edited(store: &Attendance, event: &Event, change: &EventChange) {
    let outcome = store
        .edit_event(
            GUILD,
            event.id,
            event.created_at_ms,
            change,
            day("2026-10-01"),
            S1 - 13 * 86_400,
        )
        .await
        .unwrap();
    assert!(matches!(outcome, EditOutcome::Edited { .. }), "{outcome:?}");
}

struct Beat {
    signups: Arc<Signups<FakeBoard>>,
    board: FakeBoard,
    event: Event,
}

async fn beat_on(schedule: Schedule, last: &str, created_at_unix: i64) -> Beat {
    let store = Attendance::with_pool(common::attendance_pool().await)
        .await
        .unwrap();
    let event = created(
        &store,
        &proposal("2026-10-14", last, schedule, created_at_unix),
    )
    .await;
    let board = FakeBoard::new();
    Beat {
        signups: Signups::new(board.clone(), store),
        board,
        event,
    }
}

async fn tick(beat: &Beat, now_unix: i64) {
    beat.signups.tick(now_unix, millis(now_unix)).await;
}

async fn state(beat: &Beat, date: &str) -> Option<PostState> {
    beat.signups
        .store()
        .post(&beat.event, day(date))
        .await
        .unwrap()
        .map(|post| post.state)
}

const SET_POST_LEAD: &str = "UPDATE cb_seasons SET post_lead_days = ? WHERE id = ?";
const SET_CLEAN_UP: &str = "UPDATE cb_seasons SET clean_up_minutes = ? WHERE id = ?";

async fn set(
    pool: &SqlitePool,
    statement: &'static str,
    id: i64,
    value: i64,
) -> Result<(), sqlx::Error> {
    sqlx::query(statement)
        .bind(value)
        .bind(id)
        .execute(pool)
        .await
        .map(|_| ())
}

async fn column_names(pool: &SqlitePool) -> Vec<String> {
    sqlx::query_scalar("SELECT name FROM pragma_table_info('cb_seasons') ORDER BY cid")
        .fetch_all(pool)
        .await
        .unwrap()
}

async fn migrated(path: &Path, migrations: &[&'static str]) {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            SqliteConnectOptions::new()
                .filename(path)
                .create_if_missing(true),
        )
        .await
        .unwrap();
    for migration in migrations {
        sqlx::raw_sql(migration.trim_start_matches(CLI_DIRECTIVE))
            .execute(&pool)
            .await
            .unwrap();
    }
    pool.close().await;
}

#[tokio::test]
async fn a_created_event_keeps_its_post_lead_and_clean_up() {
    let store = Attendance::with_pool(common::attendance_pool().await)
        .await
        .unwrap();
    let event = five_days_and_two_hours(&store).await;
    assert_eq!(timing(&stored(&store, event.id).await), (5, 120));
    let listed = store.events_in(GUILD).await.unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(timing(&listed[0]), (5, 120));
}

#[tokio::test]
async fn an_edit_changes_only_the_timing_it_names() {
    let store = Attendance::with_pool(common::attendance_pool().await)
        .await
        .unwrap();
    let event = five_days_and_two_hours(&store).await;
    edited(
        &store,
        &event,
        &EventChange {
            post_lead: Some(lead(2)),
            ..EventChange::default()
        },
    )
    .await;
    assert_eq!(timing(&stored(&store, event.id).await), (2, 120));
    edited(
        &store,
        &event,
        &EventChange {
            clean_up: Some(delay(0)),
            ..EventChange::default()
        },
    )
    .await;
    assert_eq!(timing(&stored(&store, event.id).await), (2, 0));
}

#[tokio::test]
async fn an_event_saved_before_post_timing_reads_the_defaults() {
    let pool = common::seasons_pool().await;
    common::run_script(&pool, common::CB_EVENTS).await.unwrap();
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO cb_seasons (guild_id, channel_id, name, first_day, last_day, weekdays, start_minute, time_zone, hours, created_by, created_at_ms) VALUES (1, 10, 'Old Event', '2026-10-14', '2026-10-18', 127, 1410, 'UTC', 4, 100, 1) RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    common::run_script(&pool, common::CB_POST_TIMING)
        .await
        .unwrap();
    let store = Attendance::with_pool(pool).await.unwrap();
    assert_eq!(timing(&stored(&store, id).await), (1, 30));
}

#[tokio::test]
async fn the_database_refuses_timing_off_the_menus() {
    let pool = common::attendance_pool().await;
    let store = Attendance::with_pool(pool.clone()).await.unwrap();
    let event = five_days_and_two_hours(&store).await;
    assert!(set(&pool, SET_POST_LEAD, event.id, 0).await.is_err());
    assert!(set(&pool, SET_POST_LEAD, event.id, 8).await.is_err());
    assert!(set(&pool, SET_POST_LEAD, event.id, 7).await.is_ok());
    assert!(set(&pool, SET_CLEAN_UP, event.id, 45).await.is_err());
    assert!(set(&pool, SET_CLEAN_UP, event.id, 270).await.is_err());
    assert!(set(&pool, SET_CLEAN_UP, event.id, 240).await.is_ok());
}

#[tokio::test]
async fn the_post_timing_migration_rolls_back() {
    let pool = common::attendance_pool().await;
    let store = Attendance::with_pool(pool.clone()).await.unwrap();
    let event = five_days_and_two_hours(&store).await;
    common::run_script(&pool, common::CB_POST_TIMING_DOWN)
        .await
        .unwrap();
    let columns = column_names(&pool).await;
    assert!(!columns.contains(&"post_lead_days".to_owned()));
    assert!(!columns.contains(&"clean_up_minutes".to_owned()));
    let kept: Vec<String> = sqlx::query_scalar("SELECT name FROM cb_seasons WHERE id = ?")
        .bind(event.id)
        .fetch_all(&pool)
        .await
        .unwrap();
    assert_eq!(kept, vec!["Scrim Nights".to_owned()]);
    let ledger: Vec<String> = sqlx::query_scalar("SELECT name FROM schema_migrations")
        .fetch_all(&pool)
        .await
        .unwrap();
    assert!(!ledger.contains(&"0014_event_post_timing".to_owned()));
}

#[tokio::test]
async fn the_beat_posts_each_session_at_its_post_lead() {
    let beat = beat_on(
        Schedule::rehearsal().with_post_lead(lead(2)),
        "2026-10-15",
        S1 - 3 * 86_400,
    )
    .await;
    tick(&beat, S1 - 172_801).await;
    assert_eq!(state(&beat, "2026-10-14").await, None);
    tick(&beat, S1 - 172_800).await;
    assert_eq!(state(&beat, "2026-10-14").await, Some(PostState::Open));
    assert_eq!(state(&beat, "2026-10-15").await, None);
    tick(&beat, S2 - 172_800).await;
    assert_eq!(state(&beat, "2026-10-15").await, Some(PostState::Open));
}

#[tokio::test]
async fn a_long_clean_up_keeps_the_post_up_without_holding_back_the_next() {
    let beat = beat_on(
        Schedule::rehearsal().with_clean_up(delay(240)),
        "2026-10-15",
        S1 - 2 * 86_400,
    )
    .await;
    tick(&beat, S1 - 86_400).await;
    tick(&beat, S1).await;
    assert_eq!(state(&beat, "2026-10-14").await, Some(PostState::Closed));
    assert_eq!(state(&beat, "2026-10-15").await, Some(PostState::Open));
    tick(&beat, S1 + 14_400 + 14_399).await;
    assert_eq!(state(&beat, "2026-10-14").await, Some(PostState::Closed));
    tick(&beat, S1 + 14_400 + 14_400).await;
    assert_eq!(state(&beat, "2026-10-14").await, Some(PostState::Removed));
    assert_eq!(state(&beat, "2026-10-15").await, Some(PostState::Open));
}

#[tokio::test]
async fn a_zero_clean_up_removes_the_post_when_the_session_ends() {
    let beat = beat_on(
        Schedule::rehearsal().with_clean_up(delay(0)),
        "2026-10-14",
        S1 - 2 * 86_400,
    )
    .await;
    tick(&beat, S1 - 86_400).await;
    tick(&beat, S1 + 14_399).await;
    assert_eq!(state(&beat, "2026-10-14").await, Some(PostState::Closed));
    tick(&beat, S1 + 14_400).await;
    assert_eq!(state(&beat, "2026-10-14").await, Some(PostState::Removed));
}

#[tokio::test]
async fn an_event_whose_post_moment_has_passed_posts_at_the_next_beat() {
    let beat = beat_on(
        Schedule::rehearsal().with_post_lead(lead(3)),
        "2026-10-14",
        S1 - 86_400,
    )
    .await;
    tick(&beat, S1 - 86_400).await;
    assert_eq!(state(&beat, "2026-10-14").await, Some(PostState::Open));
    assert_eq!(beat.board.sends().len(), 1);
}

#[tokio::test]
async fn a_database_at_0013_is_told_to_apply_0014() {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("barnacle.sqlite3");
    migrated(&path, &THROUGH_0013).await;
    let error = startup::open_stores(&path).await.err().unwrap();
    assert!(matches!(error, StartupError::PostTiming { .. }));
    assert_eq!(
        error.to_string(),
        format!(
            "the sign-up timing columns are missing from {path}; back the file up, then add them with `sqlite3 {path} < migrations/0014_event_post_timing.sql`",
            path = path.display()
        )
    );
    migrated(&path, &[common::CB_POST_TIMING]).await;
    assert!(startup::open_stores(&path).await.is_ok());
}
