mod common;

use barnacle_bot::attendance_store::Attendance;
use barnacle_bot::attendance_store::AttendanceError;
use barnacle_bot::attendance_store::CreateOutcome;
use barnacle_bot::attendance_store::EditOutcome;
use barnacle_bot::attendance_store::EndOutcome;
use barnacle_bot::attendance_store::Event;
use barnacle_bot::attendance_store::EventChange;
use barnacle_bot::attendance_store::Mark;
use barnacle_bot::attendance_store::NewEvent;
use barnacle_bot::attendance_store::PostState;
use barnacle_bot::ids::ChannelId;
use barnacle_bot::ids::GuildId;
use barnacle_bot::ids::RoleId;
use barnacle_bot::schedule::Hour;
use barnacle_bot::schedule::Hours;
use barnacle_bot::schedule::Schedule;
use barnacle_bot::schedule::Session;
use barnacle_bot::schedule::Span;
use barnacle_bot::schedule::StartTime;
use barnacle_bot::schedule::Weekdays;
use barnacle_bot::schedule::Zone;
use barnacle_bot::schedule::parse_day;
use barnacle_guess::Snowflake;
use barnacle_guess::UserId;
use common::attendance_pool;
use common::event_state;
use common::every_answer;
use common::memory_pool;
use common::posts_and_answers;
use common::run_script;
use common::seasons_pool;
use jiff::civil::Date;
use jiff::civil::Weekday;
use sqlx::sqlite::SqlitePool;

const GUILD: GuildId = GuildId::new(1);
const OTHER_GUILD: GuildId = GuildId::new(2);
const CHANNEL: ChannelId = ChannelId::new(10);
const MANAGER: UserId = UserId::new(100);
const AKI: UserId = UserId::new(200);
const BOREALIS: UserId = UserId::new(300);
const CREWMATES: RoleId = RoleId::new(4242);
const RESERVES: RoleId = RoleId::new(4343);
const TRIALS: RoleId = RoleId::new(4444);
const CREATED_AT: u64 = 1_700_000_000_000;
const ENDED_AT: u64 = 1_700_000_900_000;

const POSTS_OF_EVENT: &str = "SELECT COUNT(*) FROM cb_posts WHERE season_id = ?";
const MARKS_OF_EVENT: &str = "SELECT COUNT(*) FROM cb_marks WHERE season_id = ?";
const EVENTS_OF_GUILD: &str = "SELECT COUNT(*) FROM cb_seasons WHERE guild_id = ?";
const SEASONS_AT_0007: [&str; 11] = [
    "id",
    "guild_id",
    "channel_id",
    "number",
    "codename",
    "first_day",
    "last_day",
    "created_by",
    "created_at_ms",
    "ended_at_ms",
    "every_day",
];

async fn attendance() -> Attendance {
    Attendance::with_pool(attendance_pool().await)
        .await
        .unwrap()
}

fn day(text: &str) -> Date {
    parse_day(text).unwrap()
}

fn hour(value: u8) -> Hour {
    Hour::new(value).unwrap()
}

fn hours(count: i64) -> Hours {
    Hours::new(count).unwrap()
}

fn clan_battles(first: &str, last: &str) -> Span {
    Span::new(day(first), day(last), Schedule::clan_battles()).unwrap()
}

fn every_day(first: &str, last: &str) -> Span {
    Span::new(
        day(first),
        day(last),
        Schedule::new(
            Weekdays::EVERY_DAY,
            StartTime::parse("23:30").unwrap(),
            Zone::find("UTC").unwrap(),
            hours(4),
        ),
    )
    .unwrap()
}

fn proposal(name: &str, first: &str, last: &str) -> NewEvent {
    NewEvent {
        guild: GUILD,
        channel: CHANNEL,
        name: name.to_owned(),
        span: clan_battles(first, last),
        created_by: MANAGER,
        created_at_ms: CREATED_AT,
        ping_roles: Vec::new(),
    }
}

fn session(event: &Event, date: &str) -> Session {
    event.span.schedule().session_at(day(date)).unwrap()
}

async fn created(store: &Attendance, new: &NewEvent) -> Event {
    match store.create_event(new).await.unwrap() {
        CreateOutcome::Created(event) => event,
        other => panic!("expected a created event, got {other:?}"),
    }
}

async fn season_35(store: &Attendance) -> Event {
    created(store, &proposal("Season 35", "2026-09-16", "2026-11-05")).await
}

async fn pinging_season_35(store: &Attendance) -> Event {
    let new = NewEvent {
        ping_roles: vec![CREWMATES],
        ..proposal("Season 35", "2026-09-16", "2026-11-05")
    };
    created(store, &new).await
}

fn with_posts(event: &Event) -> Event {
    Event {
        posts_up: true,
        ..event.clone()
    }
}

async fn counted(pool: &SqlitePool, query: &'static str, id: i64) -> i64 {
    sqlx::query_scalar(query)
        .bind(id)
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn events_of(pool: &SqlitePool, guild: GuildId) -> i64 {
    counted(pool, EVENTS_OF_GUILD, i64::try_from(guild.get()).unwrap()).await
}

async fn ended_at(pool: &SqlitePool, id: i64) -> Option<i64> {
    sqlx::query_scalar("SELECT ended_at_ms FROM cb_seasons WHERE id = ?")
        .bind(id)
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn rolled_back(pool: &SqlitePool) {
    sqlx::raw_sql("ROLLBACK").execute(pool).await.unwrap();
}

fn answered(user: UserId, attending: &[bool], at_ms: u64) -> Vec<Mark> {
    attending
        .iter()
        .zip(1..)
        .map(|(attending, value)| Mark {
            user,
            hour: hour(value),
            attending: *attending,
            answered_at_ms: at_ms,
        })
        .collect()
}

#[tokio::test]
async fn a_new_name_is_created() {
    let store = attendance().await;
    let event = created(
        &store,
        &proposal("Ranked Sprint", "2026-10-14", "2026-10-18"),
    )
    .await;
    assert_eq!(event.name, "Ranked Sprint");
    assert_eq!(
        store.event(event.id).await.unwrap().unwrap().name,
        "Ranked Sprint"
    );
}

#[tokio::test]
async fn a_running_name_in_the_server_is_taken_in_either_case() {
    let pool = attendance_pool().await;
    let store = Attendance::with_pool(pool.clone()).await.unwrap();
    let at_ms = 1_791_900_000_000;
    created(
        &store,
        &NewEvent {
            created_at_ms: at_ms,
            ..proposal("Ranked Sprint", "2026-10-14", "2026-10-18")
        },
    )
    .await;
    for name in ["RANKED SPRINT", "Ranked Sprint"] {
        assert_eq!(
            store
                .create_event(&NewEvent {
                    created_at_ms: at_ms,
                    ..proposal(name, "2026-10-14", "2026-10-18")
                })
                .await
                .unwrap(),
            CreateOutcome::NameTaken,
            "{name}"
        );
    }
    assert_eq!(events_of(&pool, GUILD).await, 1);
}

#[tokio::test]
async fn a_name_in_other_accented_case_is_taken() {
    let pool = attendance_pool().await;
    let store = Attendance::with_pool(pool.clone()).await.unwrap();
    let at_ms = 1_791_900_000_000;
    created(
        &store,
        &NewEvent {
            created_at_ms: at_ms,
            ..proposal("Été", "2026-10-14", "2026-10-18")
        },
    )
    .await;
    assert_eq!(
        store
            .create_event(&NewEvent {
                created_at_ms: at_ms,
                ..proposal("été", "2026-10-14", "2026-10-18")
            })
            .await
            .unwrap(),
        CreateOutcome::NameTaken
    );
    assert_eq!(events_of(&pool, GUILD).await, 1);
}

#[tokio::test]
async fn a_name_held_by_an_ended_or_foreign_event_is_free() {
    let store = attendance().await;
    let ended = created(
        &store,
        &proposal("Ranked Sprint", "2026-10-14", "2026-10-18"),
    )
    .await;
    store
        .record_post(ended.id, day("2026-10-14"), Snowflake::new(7), CREATED_AT)
        .await
        .unwrap();
    store.end_event(GUILD, ended.id, ENDED_AT).await.unwrap();
    created(
        &store,
        &NewEvent {
            guild: OTHER_GUILD,
            ..proposal("Ranked Sprint", "2026-10-14", "2026-10-18")
        },
    )
    .await;
    assert!(matches!(
        store
            .create_event(&proposal("Ranked Sprint", "2026-10-14", "2026-10-18"))
            .await
            .unwrap(),
        CreateOutcome::Created(_)
    ));
}

#[tokio::test]
async fn a_finished_event_gives_up_its_name() {
    let pool = attendance_pool().await;
    let store = Attendance::with_pool(pool.clone()).await.unwrap();
    let finished = created(
        &store,
        &NewEvent {
            created_at_ms: 1_791_900_000_000,
            ..proposal("Ranked Sprint", "2026-10-14", "2026-10-14")
        },
    )
    .await;
    assert_eq!(finished.span.last_moment_unix(), Some(1_792_036_800));
    let fresh = created(
        &store,
        &NewEvent {
            created_at_ms: 1_792_400_000_000,
            ..proposal("Ranked Sprint", "2026-10-14", "2026-10-14")
        },
    )
    .await;
    assert_eq!(
        store.event(finished.id).await.unwrap().unwrap().ended_at_ms,
        Some(1_792_400_000_000)
    );
    assert_eq!(ended_at(&pool, fresh.id).await, None);
    assert_eq!(
        store.event(fresh.id).await.unwrap().unwrap().ended_at_ms,
        None
    );
}

#[tokio::test]
async fn a_finished_event_with_a_post_still_up_keeps_its_name() {
    let store = attendance().await;
    let finished = created(
        &store,
        &NewEvent {
            created_at_ms: 1_791_900_000_000,
            ..proposal("Ranked Sprint", "2026-10-14", "2026-10-14")
        },
    )
    .await;
    store
        .record_post(
            finished.id,
            day("2026-10-14"),
            Snowflake::new(7),
            1_791_934_200_000,
        )
        .await
        .unwrap();
    store
        .set_post_state(finished.id, day("2026-10-14"), PostState::Closed)
        .await
        .unwrap();
    let post = store
        .post(&finished, day("2026-10-14"))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        store
            .create_event(&NewEvent {
                created_at_ms: 1_792_400_000_000,
                ..proposal("Ranked Sprint", "2026-10-14", "2026-10-14")
            })
            .await
            .unwrap(),
        CreateOutcome::NameTaken
    );
    let after = store.event(finished.id).await.unwrap().unwrap();
    assert_eq!(after.ended_at_ms, None);
    assert_eq!(
        store.post(&after, day("2026-10-14")).await.unwrap(),
        Some(post)
    );
}

#[tokio::test]
async fn an_unreadable_event_with_the_name_fails_the_save() {
    let pool = attendance_pool().await;
    let store = Attendance::with_pool(pool.clone()).await.unwrap();
    sqlx::query("INSERT INTO cb_seasons (id, guild_id, channel_id, name, first_day, last_day, weekdays, start_minute, time_zone, hours, created_by, created_at_ms) VALUES (1, 1, 10, 'Ranked Sprint', '2026-10-14', '2026-10-18', 108, 1410, 'Nowhere/City', 4, 100, 0)")
        .execute(&pool)
        .await
        .unwrap();
    let error = store
        .create_event(&proposal("Ranked Sprint", "2026-10-14", "2026-10-18"))
        .await
        .err()
        .unwrap();
    assert!(matches!(
        &error,
        AttendanceError::BadZone { value } if value == "Nowhere/City"
    ));
    assert_eq!(events_of(&pool, GUILD).await, 1);
}

#[tokio::test]
async fn creating_beside_a_running_event_keeps_the_rehearsal_clock() {
    let store = attendance().await;
    season_35(&store).await;
    store.set_rehearsal_clock(GUILD, 86_400).await.unwrap();
    assert!(matches!(
        store
            .create_event(&proposal("Drill", "2026-09-16", "2026-11-05"))
            .await
            .unwrap(),
        CreateOutcome::Created(_)
    ));
    assert_eq!(store.rehearsal_clock(GUILD).await.unwrap(), 86_400);
}

#[tokio::test]
async fn creating_the_only_running_event_resets_the_rehearsal_clock() {
    let pool = attendance_pool().await;
    let store = Attendance::with_pool(pool.clone()).await.unwrap();
    let ended = season_35(&store).await;
    store
        .record_post(ended.id, day("2026-09-16"), Snowflake::new(7), CREATED_AT)
        .await
        .unwrap();
    store.end_event(GUILD, ended.id, ENDED_AT).await.unwrap();
    store.set_rehearsal_clock(GUILD, 86_400).await.unwrap();
    assert!(matches!(
        store
            .create_event(&proposal("Drill", "2026-09-16", "2026-11-05"))
            .await
            .unwrap(),
        CreateOutcome::Created(_)
    ));
    assert_eq!(store.rehearsal_clock(GUILD).await.unwrap(), 0);
    assert_eq!(clock_rows(&pool).await, 0);
}

#[tokio::test]
async fn a_refused_name_leaves_the_rehearsal_clock_alone() {
    let store = attendance().await;
    season_35(&store).await;
    store.set_rehearsal_clock(GUILD, 86_400).await.unwrap();
    assert_eq!(
        store
            .create_event(&proposal("Season 35", "2027-01-06", "2027-02-24"))
            .await
            .unwrap(),
        CreateOutcome::NameTaken
    );
    assert_eq!(store.rehearsal_clock(GUILD).await.unwrap(), 86_400);
}

#[tokio::test]
async fn an_event_stores_and_returns_its_ping_role() {
    let store = attendance().await;
    let event = pinging_season_35(&store).await;
    assert_eq!(event.ping_roles, vec![CREWMATES]);
    assert_eq!(event.ended_at_ms, None);
    assert!(!event.posts_up);
    assert_eq!(store.event(event.id).await.unwrap(), Some(event.clone()));
    assert_eq!(
        store.live_event(GUILD, event.id).await.unwrap(),
        Some(event.clone())
    );
    assert_eq!(store.live_event(GUILD, event.id + 1).await.unwrap(), None);
    assert_eq!(store.live_event(OTHER_GUILD, event.id).await.unwrap(), None);
}

#[tokio::test]
async fn ending_an_event_with_nothing_posted_deletes_it() {
    let store = attendance().await;
    let event = season_35(&store).await;
    assert_eq!(
        store.end_event(GUILD, event.id, ENDED_AT).await.unwrap(),
        EndOutcome::Removed
    );
    assert_eq!(store.event(event.id).await.unwrap(), None);
    assert_eq!(
        store.end_event(GUILD, event.id, ENDED_AT).await.unwrap(),
        EndOutcome::NotFound
    );
}

#[tokio::test]
async fn ending_an_event_with_posts_marks_it_ended() {
    let store = attendance().await;
    let event = season_35(&store).await;
    store
        .record_post(event.id, day("2026-09-16"), Snowflake::new(7), CREATED_AT)
        .await
        .unwrap();
    assert_eq!(
        store
            .end_event(GUILD, event.id, 1_792_000_000_000)
            .await
            .unwrap(),
        EndOutcome::Ended {
            event: with_posts(&event)
        }
    );
    let ended = store.event(event.id).await.unwrap().unwrap();
    assert_eq!(ended.ended_at_ms, Some(1_792_000_000_000));
    assert_eq!(ended.span, event.span);
    assert_eq!(
        store
            .end_event(GUILD, event.id, 1_792_000_000_000)
            .await
            .unwrap(),
        EndOutcome::NotFound
    );
}

#[tokio::test]
async fn an_ended_event_frees_its_name() {
    let store = attendance().await;
    let event = season_35(&store).await;
    store
        .record_post(event.id, day("2026-09-16"), Snowflake::new(7), CREATED_AT)
        .await
        .unwrap();
    store.end_event(GUILD, event.id, ENDED_AT).await.unwrap();
    let again = season_35(&store).await;
    assert_eq!(again.name, "Season 35");
    assert_ne!(again.id, event.id);
    assert_eq!(
        store.live_event(GUILD, again.id).await.unwrap(),
        Some(again)
    );
}

#[tokio::test]
async fn an_ended_event_is_not_live_and_not_listed() {
    let store = attendance().await;
    let event = season_35(&store).await;
    let last_moment = event.span.last_moment_unix().unwrap();
    store
        .record_post(event.id, day("2026-09-16"), Snowflake::new(7), CREATED_AT)
        .await
        .unwrap();
    assert_eq!(
        store.live_events(last_moment - 1).await.unwrap().events,
        vec![with_posts(&event)]
    );
    assert_eq!(
        store.events_in(GUILD).await.unwrap(),
        vec![with_posts(&event)]
    );
    store.end_event(GUILD, event.id, ENDED_AT).await.unwrap();
    assert!(
        store
            .live_events(last_moment - 1)
            .await
            .unwrap()
            .events
            .is_empty()
    );
    assert!(store.events_in(GUILD).await.unwrap().is_empty());
    assert_eq!(store.live_event(GUILD, event.id).await.unwrap(), None);
}

#[tokio::test]
async fn a_click_can_still_resolve_an_ended_event() {
    let store = attendance().await;
    let event = pinging_season_35(&store).await;
    store
        .record_post(event.id, day("2026-09-16"), Snowflake::new(7), CREATED_AT)
        .await
        .unwrap();
    store.end_event(GUILD, event.id, ENDED_AT).await.unwrap();
    let found = store.event(event.id).await.unwrap().unwrap();
    assert_eq!(found.name, "Season 35");
    assert_eq!(found.guild, GUILD);
    assert_eq!(found.channel, CHANNEL);
    assert_eq!(found.ping_roles, vec![CREWMATES]);
    assert_eq!(found.ended_at_ms, Some(ENDED_AT));
}

#[tokio::test]
async fn ending_an_event_keeps_the_marks() {
    let store = attendance().await;
    let event = season_35(&store).await;
    let first = day("2026-09-16");
    store
        .record_post(event.id, first, Snowflake::new(7), CREATED_AT)
        .await
        .unwrap();
    store
        .mark(event.id, first, AKI, &[hour(1)], hours(4), true, 1_000)
        .await
        .unwrap();
    assert_eq!(
        store.end_event(GUILD, event.id, ENDED_AT).await.unwrap(),
        EndOutcome::Ended {
            event: with_posts(&event)
        }
    );
    assert_eq!(
        store.roster(event.id, first).await.unwrap(),
        answered(AKI, &[true, false, false, false], 1_000)
    );
    assert_eq!(store.posts(&event).await.unwrap().len(), 1);
}

#[tokio::test]
async fn a_bad_role_id_is_named() {
    let pool = attendance_pool().await;
    sqlx::raw_sql("PRAGMA ignore_check_constraints = on")
        .execute(&pool)
        .await
        .unwrap();
    let store = Attendance::with_pool(pool.clone()).await.unwrap();
    sqlx::query("INSERT INTO cb_seasons (id, guild_id, channel_id, name, first_day, last_day, weekdays, start_minute, time_zone, hours, created_by, created_at_ms) VALUES (1, 1, 10, 'Season 35', '2026-09-16', '2026-11-05', 108, 1410, 'UTC', 4, 100, 0)")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO cb_season_ping_roles (season_id, position, role_id) VALUES (1, 1, 0)")
        .execute(&pool)
        .await
        .unwrap();
    let error = store.event(1).await.err().unwrap();
    assert!(matches!(error, AttendanceError::InvalidRole { value: 0 }));
    assert_eq!(
        error.to_string(),
        "the attendance database holds 0, which is not a role ID"
    );
}

#[tokio::test]
async fn a_bad_schedule_column_is_named() {
    let pool = attendance_pool().await;
    sqlx::raw_sql("PRAGMA ignore_check_constraints = on")
        .execute(&pool)
        .await
        .unwrap();
    let store = Attendance::with_pool(pool.clone()).await.unwrap();
    sqlx::query("INSERT INTO cb_seasons (id, guild_id, channel_id, name, first_day, last_day, weekdays, start_minute, time_zone, hours, created_by, created_at_ms) VALUES (1, 1, 10, 'No days', '2026-09-16', '2026-11-05', 0, 1410, 'UTC', 4, 100, 0), (2, 1, 10, 'No start', '2026-09-16', '2026-11-05', 108, 1440, 'UTC', 4, 100, 0)")
        .execute(&pool)
        .await
        .unwrap();
    assert!(matches!(
        store.event(1).await.err().unwrap(),
        AttendanceError::BadWeekdays { value: 0 }
    ));
    assert!(matches!(
        store.event(2).await.err().unwrap(),
        AttendanceError::BadStart { value: 1440 }
    ));
}

#[tokio::test]
async fn an_hour_count_outside_1_to_24_is_not_read() {
    let pool = attendance_pool().await;
    sqlx::raw_sql("PRAGMA ignore_check_constraints = on")
        .execute(&pool)
        .await
        .unwrap();
    let store = Attendance::with_pool(pool.clone()).await.unwrap();
    sqlx::query("INSERT INTO cb_seasons (id, guild_id, channel_id, name, first_day, last_day, weekdays, start_minute, time_zone, hours, created_by, created_at_ms) VALUES (1, 1, 10, 'Ranked Sprint', '2026-10-14', '2026-10-18', 108, 1410, 'UTC', 0, 100, 0)")
        .execute(&pool)
        .await
        .unwrap();
    let error = store.event(1).await.err().unwrap();
    assert!(matches!(error, AttendanceError::BadHours { value: 0 }));
}

#[tokio::test]
async fn an_unreadable_event_fails_the_listing() {
    let pool = attendance_pool().await;
    let store = Attendance::with_pool(pool.clone()).await.unwrap();
    sqlx::query("INSERT INTO cb_seasons (id, guild_id, channel_id, name, first_day, last_day, weekdays, start_minute, time_zone, hours, created_by, created_at_ms) VALUES (1, 1, 10, 'Ranked Sprint', '2026-10-14', '2026-10-18', 108, 1410, 'Nowhere/City', 4, 100, 0)")
        .execute(&pool)
        .await
        .unwrap();
    let error = store.events_in(GUILD).await.err().unwrap();
    assert!(matches!(
        &error,
        AttendanceError::BadZone { value } if value == "Nowhere/City"
    ));
}

async fn posted_event(pool: SqlitePool) -> (Attendance, Event) {
    let store = Attendance::with_pool(pool).await.unwrap();
    let event = season_35(&store).await;
    store
        .record_post(event.id, day("2026-09-16"), Snowflake::new(7), CREATED_AT)
        .await
        .unwrap();
    (store, with_posts(&event))
}

#[tokio::test]
async fn marks_upsert_and_keep_the_first_answer_time() {
    let pool = attendance_pool().await;
    let (store, event) = posted_event(pool.clone()).await;
    let date = day("2026-09-16");
    store
        .mark(event.id, date, AKI, &[hour(1)], hours(4), true, 1_000)
        .await
        .unwrap();
    store
        .mark(event.id, date, AKI, &[hour(1)], hours(4), false, 5_000)
        .await
        .unwrap();
    assert_eq!(
        store.roster(event.id, date).await.unwrap(),
        answered(AKI, &[false, false, false, false], 1_000)
    );
    let changed: Vec<(i64, i64)> =
        sqlx::query_as("SELECT hour, changed_at_ms FROM cb_marks ORDER BY hour")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(
        changed,
        vec![(1, 5_000), (2, 1_000), (3, 1_000), (4, 1_000)]
    );
}

#[tokio::test]
async fn a_single_hour_mark_fills_the_other_hours_with_nope() {
    let pool = attendance_pool().await;
    let (store, event) = posted_event(pool.clone()).await;
    let date = day("2026-09-16");
    store
        .mark(event.id, date, AKI, &[hour(1)], hours(4), true, 1_000)
        .await
        .unwrap();
    assert_eq!(
        store.roster(event.id, date).await.unwrap(),
        answered(AKI, &[true, false, false, false], 1_000)
    );
    store
        .mark(event.id, date, AKI, &[hour(2)], hours(4), true, 5_000)
        .await
        .unwrap();
    assert_eq!(
        store.roster(event.id, date).await.unwrap(),
        answered(AKI, &[true, true, false, false], 1_000)
    );
    let changed: Vec<(i64, i64, i64)> =
        sqlx::query_as("SELECT hour, answered_at_ms, changed_at_ms FROM cb_marks ORDER BY hour")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(
        changed,
        vec![
            (1, 1_000, 1_000),
            (2, 1_000, 5_000),
            (3, 1_000, 1_000),
            (4, 1_000, 1_000),
        ]
    );
}

#[tokio::test]
async fn a_mark_fills_every_hour_of_a_long_event() {
    let pool = attendance_pool().await;
    let (store, event) = posted_event(pool.clone()).await;
    let date = day("2026-09-16");
    store
        .mark(event.id, date, AKI, &[hour(7)], hours(12), true, 1_000)
        .await
        .unwrap();
    let roster = store.roster(event.id, date).await.unwrap();
    assert_eq!(roster.len(), 12);
    assert_eq!(
        roster
            .iter()
            .filter(|mark| mark.attending)
            .map(|mark| mark.hour.get())
            .collect::<Vec<u8>>(),
        vec![7]
    );
}

#[tokio::test]
async fn roster_is_ordered_by_first_answer() {
    let (store, event) = posted_event(attendance_pool().await).await;
    let date = day("2026-09-16");
    store
        .mark(
            event.id,
            date,
            BOREALIS,
            &[hour(2), hour(1)],
            hours(4),
            true,
            2_000,
        )
        .await
        .unwrap();
    store
        .mark(
            event.id,
            date,
            AKI,
            &[hour(1), hour(2), hour(3), hour(4)],
            hours(4),
            false,
            3_000,
        )
        .await
        .unwrap();
    store
        .mark(event.id, date, BOREALIS, &[hour(3)], hours(4), false, 9_000)
        .await
        .unwrap();
    let roster = store.roster(event.id, date).await.unwrap();
    let order: Vec<(UserId, u8, bool, u64)> = roster
        .iter()
        .map(|mark| {
            (
                mark.user,
                mark.hour.get(),
                mark.attending,
                mark.answered_at_ms,
            )
        })
        .collect();
    assert_eq!(
        order,
        vec![
            (BOREALIS, 1, true, 2_000),
            (BOREALIS, 2, true, 2_000),
            (BOREALIS, 3, false, 2_000),
            (BOREALIS, 4, false, 2_000),
            (AKI, 1, false, 3_000),
            (AKI, 2, false, 3_000),
            (AKI, 3, false, 3_000),
            (AKI, 4, false, 3_000),
        ]
    );
}

#[tokio::test]
async fn live_events_drops_an_event_whose_posts_are_all_removed() {
    let store = attendance().await;
    let event = season_35(&store).await;
    let last_moment = event.span.last_moment_unix().unwrap();
    assert_eq!(
        store.live_events(last_moment - 1).await.unwrap().events,
        vec![event.clone()]
    );
    assert!(
        store
            .live_events(last_moment)
            .await
            .unwrap()
            .events
            .is_empty()
    );
    let next = created(&store, &proposal("Season 36", "2026-11-11", "2026-12-30")).await;
    assert_eq!(
        store.live_events(last_moment).await.unwrap().events,
        vec![next.clone()]
    );
    assert_eq!(
        store.live_events(last_moment - 1).await.unwrap().events,
        vec![event, next]
    );
}

#[tokio::test]
async fn live_events_keeps_an_event_until_its_last_post_is_removed() {
    let store = attendance().await;
    let event = season_35(&store).await;
    let last = event.span.sessions().last().unwrap().date();
    let last_moment = event.span.last_moment_unix().unwrap();
    store
        .record_post(event.id, last, Snowflake::new(500), CREATED_AT)
        .await
        .unwrap();
    assert_eq!(
        store.live_events(last_moment).await.unwrap().events,
        vec![with_posts(&event)]
    );
    assert_eq!(
        store
            .live_events(last_moment + 86_400)
            .await
            .unwrap()
            .events,
        vec![with_posts(&event)]
    );
    store
        .set_post_state(event.id, last, PostState::Removed)
        .await
        .unwrap();
    assert!(
        store
            .live_events(last_moment)
            .await
            .unwrap()
            .events
            .is_empty()
    );
}

#[tokio::test]
async fn live_events_sets_an_unreadable_row_aside() {
    let pool = attendance_pool().await;
    let store = Attendance::with_pool(pool.clone()).await.unwrap();
    let readable = created(
        &store,
        &NewEvent {
            guild: OTHER_GUILD,
            ..proposal("Season 35", "2026-10-14", "2026-10-14")
        },
    )
    .await;
    sqlx::query("INSERT INTO cb_seasons (guild_id, channel_id, name, first_day, last_day, weekdays, start_minute, time_zone, hours, created_by, created_at_ms) VALUES (1, 10, 'Ranked Sprint', '2026-10-14', '2026-10-14', 108, 1410, 'Nowhere/City', 4, 100, 0)")
        .execute(&pool)
        .await
        .unwrap();
    let live = store.live_events(1_791_934_200).await.unwrap();
    assert_eq!(live.events, vec![readable]);
    assert_eq!(live.unreadable.len(), 1);
    assert_eq!(live.unreadable[0].guild, GUILD);
    assert!(matches!(
        &live.unreadable[0].error,
        AttendanceError::BadZone { value } if value == "Nowhere/City"
    ));
}

#[tokio::test]
async fn a_missing_table_is_named() {
    assert!(matches!(
        Attendance::with_pool(memory_pool().await).await,
        Err(AttendanceError::MissingTable {
            table: "cb_seasons"
        })
    ));
    let pool = memory_pool().await;
    sqlx::raw_sql(
        "CREATE TABLE cb_seasons (id INTEGER PRIMARY KEY, guild_id INTEGER NOT NULL, channel_id INTEGER NOT NULL, name TEXT NOT NULL, first_day TEXT NOT NULL, last_day TEXT NOT NULL, weekdays INTEGER NOT NULL, start_minute INTEGER NOT NULL, time_zone TEXT NOT NULL, hours INTEGER NOT NULL, created_by INTEGER NOT NULL, created_at_ms INTEGER NOT NULL, ended_at_ms INTEGER) STRICT; CREATE TABLE cb_season_ping_roles (season_id INTEGER NOT NULL, position INTEGER NOT NULL, role_id INTEGER NOT NULL, PRIMARY KEY (season_id, position)) STRICT",
    )
    .execute(&pool)
    .await
    .unwrap();
    assert!(matches!(
        Attendance::with_pool(pool).await,
        Err(AttendanceError::MissingTable { table: "cb_posts" })
    ));
}

#[tokio::test]
async fn unexpected_columns_are_reported() {
    let pool = memory_pool().await;
    sqlx::raw_sql("CREATE TABLE cb_seasons (id INTEGER PRIMARY KEY, number TEXT NOT NULL) STRICT")
        .execute(&pool)
        .await
        .unwrap();
    let error = Attendance::with_pool(pool).await.err().unwrap();
    assert!(matches!(
        &error,
        AttendanceError::UnexpectedColumns {
            table: "cb_seasons",
            found
        } if found.len() == 2
    ));
    assert!(error.to_string().starts_with(
        "cb_seasons does not have the expected columns; found [Column { name: \"id\""
    ));
}

#[tokio::test]
async fn a_database_at_0007_is_named_as_before_events() {
    let error = Attendance::with_pool(seasons_pool().await)
        .await
        .err()
        .unwrap();
    assert!(matches!(error, AttendanceError::BeforeEvents));
}

#[tokio::test]
async fn a_post_records_its_state_and_message() {
    let (store, event) = posted_event(attendance_pool().await).await;
    let first = day("2026-09-16");
    let post = store.post(&event, first).await.unwrap().unwrap();
    assert_eq!(post.session, session(&event, "2026-09-16"));
    assert_eq!(post.message, Snowflake::new(7));
    assert_eq!(post.state, PostState::Open);
    assert_eq!(post.posted_at_ms, CREATED_AT);
    store
        .set_post_state(event.id, first, PostState::Closed)
        .await
        .unwrap();
    assert_eq!(
        store.posts(&event).await.unwrap()[0].state,
        PostState::Closed
    );
    store
        .record_post(event.id, first, Snowflake::new(9), CREATED_AT + 1)
        .await
        .unwrap();
    let reposted = store.post(&event, first).await.unwrap().unwrap();
    assert_eq!(reposted.message, Snowflake::new(9));
    assert_eq!(reposted.state, PostState::Open);
    assert_eq!(reposted.posted_at_ms, CREATED_AT + 1);
    assert_eq!(store.posts(&event).await.unwrap().len(), 1);
    assert_eq!(store.post(&event, day("2026-09-17")).await.unwrap(), None);
}

#[tokio::test]
async fn a_post_on_a_date_the_schedule_no_longer_holds_is_still_read() {
    let (store, event) = posted_event(attendance_pool().await).await;
    let friday = day("2026-09-18");
    store
        .record_post(event.id, friday, Snowflake::new(8), CREATED_AT)
        .await
        .unwrap();
    let post = store.post(&event, friday).await.unwrap().unwrap();
    assert_eq!(post.session.date(), friday);
    assert_eq!(post.session.start_unix(), 1_789_774_200);
}

#[tokio::test]
async fn a_database_without_the_live_name_index_is_named() {
    let pool = attendance_pool().await;
    sqlx::raw_sql("DROP INDEX cb_seasons_live_name")
        .execute(&pool)
        .await
        .unwrap();
    let error = Attendance::with_pool(pool).await.err().unwrap();
    assert!(matches!(
        error,
        AttendanceError::MissingIndex {
            index: "cb_seasons_live_name"
        }
    ));
    assert_eq!(
        error.to_string(),
        "the attendance database has no cb_seasons_live_name index"
    );
}

async fn marked_event(store: &Attendance, new: &NewEvent) -> Event {
    let event = created(store, new).await;
    let first = event.span.sessions().next().unwrap().date();
    store
        .record_post(event.id, first, Snowflake::new(7), CREATED_AT)
        .await
        .unwrap();
    store
        .mark(event.id, first, AKI, &[hour(1)], hours(4), true, 1_000)
        .await
        .unwrap();
    with_posts(&event)
}

#[tokio::test]
async fn purge_event_deletes_its_marks_posts_and_event() {
    let pool = attendance_pool().await;
    let (store, event) = posted_event(pool.clone()).await;
    let first = day("2026-09-16");
    let second = day("2026-09-17");
    store
        .record_post(event.id, second, Snowflake::new(8), CREATED_AT)
        .await
        .unwrap();
    store
        .mark(
            event.id,
            first,
            AKI,
            &[hour(1), hour(2)],
            hours(4),
            true,
            1_000,
        )
        .await
        .unwrap();
    store
        .mark(
            event.id,
            second,
            BOREALIS,
            &[hour(3)],
            hours(4),
            false,
            2_000,
        )
        .await
        .unwrap();
    store.end_event(GUILD, event.id, ENDED_AT).await.unwrap();
    assert_eq!(
        store
            .purge_event(GUILD, event.id, event.created_at_ms)
            .await
            .unwrap(),
        Some(8)
    );
    assert_eq!(store.event(event.id).await.unwrap(), None);
    assert_eq!(counted(&pool, POSTS_OF_EVENT, event.id).await, 0);
    assert_eq!(counted(&pool, MARKS_OF_EVENT, event.id).await, 0);
    assert!(store.events_in_any_state(GUILD).await.unwrap().is_empty());
    assert_eq!(
        store
            .purge_event(GUILD, event.id, event.created_at_ms)
            .await
            .unwrap(),
        None
    );
}

#[tokio::test]
async fn purge_event_leaves_another_guilds_event_alone() {
    let pool = attendance_pool().await;
    let store = Attendance::with_pool(pool.clone()).await.unwrap();
    let ours = marked_event(&store, &proposal("Season 35", "2026-09-16", "2026-11-05")).await;
    let theirs = NewEvent {
        guild: OTHER_GUILD,
        ..proposal("Season 35", "2026-09-16", "2026-11-05")
    };
    let theirs = marked_event(&store, &theirs).await;
    assert_eq!(
        store
            .purge_event(OTHER_GUILD, ours.id, ours.created_at_ms)
            .await
            .unwrap(),
        None
    );
    assert_eq!(store.event(ours.id).await.unwrap(), Some(ours.clone()));
    assert_eq!(counted(&pool, POSTS_OF_EVENT, ours.id).await, 1);
    assert_eq!(counted(&pool, MARKS_OF_EVENT, ours.id).await, 4);
    assert_eq!(
        store
            .purge_event(GUILD, ours.id, ours.created_at_ms)
            .await
            .unwrap(),
        Some(4)
    );
    assert_eq!(store.event(ours.id).await.unwrap(), None);
    assert_eq!(store.event(theirs.id).await.unwrap(), Some(theirs.clone()));
    assert_eq!(counted(&pool, POSTS_OF_EVENT, theirs.id).await, 1);
    assert_eq!(counted(&pool, MARKS_OF_EVENT, theirs.id).await, 4);
    assert_eq!(
        store.events_in_any_state(OTHER_GUILD).await.unwrap(),
        vec![theirs]
    );
}

#[tokio::test]
async fn events_in_any_state_lists_an_ended_event() {
    let store = attendance().await;
    let season_34 = created(&store, &proposal("Season 34", "2026-06-10", "2026-08-02")).await;
    store
        .record_post(
            season_34.id,
            day("2026-06-10"),
            Snowflake::new(7),
            CREATED_AT,
        )
        .await
        .unwrap();
    let event = season_35(&store).await;
    store
        .end_event(GUILD, season_34.id, ENDED_AT)
        .await
        .unwrap();
    let ended = Event {
        ended_at_ms: Some(ENDED_AT),
        posts_up: true,
        ..season_34
    };
    assert_eq!(store.events_in(GUILD).await.unwrap(), vec![event.clone()]);
    assert_eq!(
        store.events_in_any_state(GUILD).await.unwrap(),
        vec![ended, event]
    );
    assert!(
        store
            .events_in_any_state(OTHER_GUILD)
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn purge_event_ignores_a_creation_time_that_is_not_the_one_it_read() {
    let store = attendance().await;
    let event = season_35(&store).await;
    assert_eq!(
        store
            .purge_event(GUILD, event.id, event.created_at_ms + 1)
            .await
            .unwrap(),
        None
    );
    assert!(store.event(event.id).await.unwrap().is_some());
}

#[tokio::test]
async fn purge_event_reports_a_deleted_event_that_held_no_answers() {
    let store = attendance().await;
    let event = season_35(&store).await;
    assert_eq!(
        store
            .purge_event(GUILD, event.id, event.created_at_ms)
            .await
            .unwrap(),
        Some(0)
    );
    assert!(store.event(event.id).await.unwrap().is_none());
}

#[tokio::test]
async fn an_every_day_event_keeps_its_schedule_when_it_is_read_back() {
    let store = attendance().await;
    let event = created(
        &store,
        &NewEvent {
            span: every_day("2026-09-18", "2026-09-20"),
            ..proposal("Drill", "2026-09-18", "2026-09-20")
        },
    )
    .await;
    assert_eq!(event.span.schedule().weekdays(), Weekdays::EVERY_DAY);
    let labels: Vec<String> = event.span.sessions().map(Session::label).collect();
    assert_eq!(labels, vec!["2026-09-18", "2026-09-19", "2026-09-20"]);
    assert_eq!(store.event(event.id).await.unwrap(), Some(event.clone()));
    assert_eq!(
        store.live_event(GUILD, event.id).await.unwrap(),
        Some(event.clone())
    );
    assert_eq!(store.events_in(GUILD).await.unwrap(), vec![event.clone()]);
    assert_eq!(
        store.events_in_any_state(GUILD).await.unwrap(),
        vec![event.clone()]
    );
    let first_post_at = event.span.sessions().next().unwrap().post_at_unix();
    assert_eq!(
        store.live_events(first_post_at).await.unwrap().events,
        vec![event.clone()]
    );
    let beside = created(&store, &proposal("Season 35", "2026-09-19", "2026-09-23")).await;
    assert_eq!(
        store
            .event(beside.id)
            .await
            .unwrap()
            .unwrap()
            .span
            .schedule()
            .weekdays(),
        Weekdays::CLAN_BATTLES
    );
}

#[tokio::test]
async fn a_denver_event_keeps_its_zone_and_start_when_it_is_read_back() {
    let store = attendance().await;
    let denver = Schedule::new(
        Weekdays::new(&[Weekday::Wednesday]).unwrap(),
        StartTime::parse("17:00").unwrap(),
        Zone::find("America/Denver").unwrap(),
        hours(6),
    );
    let event = created(
        &store,
        &NewEvent {
            span: Span::new(day("2026-10-14"), day("2026-10-28"), denver).unwrap(),
            ..proposal("Ranked Sprint", "2026-10-14", "2026-10-28")
        },
    )
    .await;
    let read = store.event(event.id).await.unwrap().unwrap();
    assert_eq!(read, event);
    assert_eq!(read.span.schedule().zone().name(), "America/Denver");
    assert_eq!(read.span.schedule().start().minutes(), 1020);
    assert_eq!(read.span.schedule().hours().get(), 6);
    assert_eq!(session(&read, "2026-10-14").start_unix(), 1_792_018_800);
}

#[tokio::test]
async fn the_rehearsal_clock_starts_at_zero_and_upserts() {
    let pool = attendance_pool().await;
    let store = Attendance::with_pool(pool.clone()).await.unwrap();
    assert_eq!(store.rehearsal_clock(GUILD).await.unwrap(), 0);
    store.set_rehearsal_clock(GUILD, 86_400).await.unwrap();
    assert_eq!(store.rehearsal_clock(GUILD).await.unwrap(), 86_400);
    store.set_rehearsal_clock(GUILD, 172_800).await.unwrap();
    assert_eq!(store.rehearsal_clock(GUILD).await.unwrap(), 172_800);
    assert_eq!(store.rehearsal_clock(OTHER_GUILD).await.unwrap(), 0);
    store.set_rehearsal_clock(OTHER_GUILD, 10).await.unwrap();
    assert_eq!(clock_rows(&pool).await, 2);
    store.clear_rehearsal_clock(GUILD).await.unwrap();
    assert_eq!(store.rehearsal_clock(GUILD).await.unwrap(), 0);
    assert_eq!(store.rehearsal_clock(OTHER_GUILD).await.unwrap(), 10);
    store.clear_rehearsal_clock(GUILD).await.unwrap();
    store.clear_rehearsal_clock(OTHER_GUILD).await.unwrap();
    assert_eq!(clock_rows(&pool).await, 0);
}

async fn clock_rows(pool: &SqlitePool) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM cb_rehearsal_clock")
        .fetch_one(pool)
        .await
        .unwrap()
}

#[tokio::test]
async fn a_database_without_the_rehearsal_clock_is_named() {
    let pool = memory_pool().await;
    sqlx::raw_sql(common::CB_MIGRATION)
        .execute(&pool)
        .await
        .unwrap();
    run_script(&pool, common::CB_CONTROLS).await.unwrap();
    sqlx::raw_sql("ALTER TABLE cb_seasons ADD COLUMN every_day INTEGER NOT NULL DEFAULT 0")
        .execute(&pool)
        .await
        .unwrap();
    run_script(&pool, common::CB_PING_ROLES).await.unwrap();
    run_script(&pool, common::CB_EVENTS).await.unwrap();
    let error = Attendance::with_pool(pool).await.err().unwrap();
    assert!(matches!(
        error,
        AttendanceError::MissingTable {
            table: "cb_rehearsal_clock"
        }
    ));
}

async fn ping_role_rows(pool: &SqlitePool, event: i64) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM cb_season_ping_roles WHERE season_id = ?")
        .bind(event)
        .fetch_one(pool)
        .await
        .unwrap()
}

#[tokio::test]
async fn an_event_stores_and_returns_its_ping_roles_in_order() {
    let store = attendance().await;
    let new = NewEvent {
        ping_roles: vec![CREWMATES, RESERVES, TRIALS],
        ..proposal("Season 35", "2026-09-16", "2026-11-05")
    };
    let event = created(&store, &new).await;
    assert_eq!(event.ping_roles, vec![CREWMATES, RESERVES, TRIALS]);
    assert_eq!(
        store.event(event.id).await.unwrap().unwrap().ping_roles,
        vec![CREWMATES, RESERVES, TRIALS]
    );
    assert_eq!(
        store
            .live_event(GUILD, event.id)
            .await
            .unwrap()
            .unwrap()
            .ping_roles,
        vec![CREWMATES, RESERVES, TRIALS]
    );
}

#[tokio::test]
async fn removing_an_event_removes_its_ping_roles() {
    let pool = attendance_pool().await;
    let store = Attendance::with_pool(pool.clone()).await.unwrap();
    let ended = pinging_season_35(&store).await;
    assert_eq!(ping_role_rows(&pool, ended.id).await, 1);
    assert_eq!(
        store.end_event(GUILD, ended.id, ENDED_AT).await.unwrap(),
        EndOutcome::Removed
    );
    assert_eq!(ping_role_rows(&pool, ended.id).await, 0);
    let purged = pinging_season_35(&store).await;
    assert_eq!(ping_role_rows(&pool, purged.id).await, 1);
    assert_eq!(
        store
            .purge_event(GUILD, purged.id, purged.created_at_ms)
            .await
            .unwrap(),
        Some(0)
    );
    assert_eq!(ping_role_rows(&pool, purged.id).await, 0);
    let left: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM cb_season_ping_roles")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(left, 0);
}

#[tokio::test]
async fn migration_0006_moves_each_ping_role_to_position_one() {
    let pool = memory_pool().await;
    for migration in [
        common::CB_MIGRATION,
        common::CB_CONTROLS,
        common::CB_HARNESS,
    ] {
        run_script(&pool, migration).await.unwrap();
    }
    sqlx::raw_sql("INSERT INTO cb_seasons (id, guild_id, channel_id, number, codename, first_day, last_day, created_by, created_at_ms, ping_role_id) VALUES (1, 1, 10, 35, NULL, '2026-09-16', '2026-11-05', 100, 0, 4242), (2, 1, 10, 36, NULL, '2026-12-02', '2026-12-20', 100, 0, NULL)")
        .execute(&pool)
        .await
        .unwrap();
    run_script(&pool, common::CB_PING_ROLES).await.unwrap();
    run_script(&pool, common::CB_EVENTS).await.unwrap();
    let store = Attendance::with_pool(pool).await.unwrap();
    assert_eq!(
        store.event(1).await.unwrap().unwrap().ping_roles,
        vec![CREWMATES]
    );
    assert_eq!(
        store.event(2).await.unwrap().unwrap().ping_roles,
        Vec::new()
    );
}

#[tokio::test]
async fn the_0006_rollback_restores_the_first_ping_role() {
    let pool = seasons_pool().await;
    sqlx::raw_sql("INSERT INTO cb_seasons (id, guild_id, channel_id, number, codename, first_day, last_day, created_by, created_at_ms) VALUES (1, 1, 10, 35, NULL, '2026-09-16', '2026-11-05', 100, 0); INSERT INTO cb_season_ping_roles (season_id, position, role_id) VALUES (1, 1, 4242), (1, 2, 4343)")
        .execute(&pool)
        .await
        .unwrap();
    run_script(&pool, common::CB_PING_ROLES_DOWN).await.unwrap();
    let names: Vec<String> =
        sqlx::query_scalar("SELECT name FROM pragma_table_info('cb_seasons') ORDER BY cid")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(
        names,
        [
            "id",
            "guild_id",
            "channel_id",
            "number",
            "codename",
            "first_day",
            "last_day",
            "created_by",
            "created_at_ms",
            "ping_role_id",
            "ended_at_ms",
            "every_day",
        ]
    );
    let restored: Option<i64> =
        sqlx::query_scalar("SELECT ping_role_id FROM cb_seasons WHERE id = 1")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(restored, Some(4242));
}

#[tokio::test]
async fn replacing_a_post_message_keeps_its_state_and_needs_the_old_message() {
    let (store, event) = posted_event(attendance_pool().await).await;
    let first = day("2026-09-16");
    let old = Snowflake::new(7);
    let new = Snowflake::new(8);
    store
        .set_post_state(event.id, first, PostState::Closed)
        .await
        .unwrap();
    let closed = store.post(&event, first).await.unwrap();
    assert!(
        !store
            .replace_post_message(event.id, first, old, new)
            .await
            .unwrap()
    );
    assert_eq!(store.post(&event, first).await.unwrap(), closed);
    store
        .set_post_state(event.id, first, PostState::Open)
        .await
        .unwrap();
    assert!(
        !store
            .replace_post_message(event.id, first, Snowflake::new(99), new)
            .await
            .unwrap()
    );
    assert_eq!(
        store.post(&event, first).await.unwrap().unwrap().message,
        old
    );
    assert!(
        store
            .replace_post_message(event.id, first, old, new)
            .await
            .unwrap()
    );
    let replaced = store.post(&event, first).await.unwrap().unwrap();
    assert_eq!(replaced.message, new);
    assert_eq!(replaced.state, PostState::Open);
    assert_eq!(replaced.posted_at_ms, CREATED_AT);
}

fn clan_battle_days() -> Weekdays {
    Weekdays::new(&[
        Weekday::Wednesday,
        Weekday::Thursday,
        Weekday::Saturday,
        Weekday::Sunday,
    ])
    .unwrap()
}

fn assert_clan_battle_time(event: &Event) {
    let schedule = event.span.schedule();
    assert_eq!(schedule.start(), StartTime::new(23, 30).unwrap());
    assert_eq!(schedule.zone(), &Zone::find("UTC").unwrap());
    assert_eq!(schedule.hours(), hours(4));
}

#[tokio::test]
async fn migration_0008_names_each_season_after_its_title() {
    let pool = seasons_pool().await;
    sqlx::raw_sql("INSERT INTO cb_seasons (id, guild_id, channel_id, number, codename, first_day, last_day, created_by, created_at_ms) VALUES (1, 1, 10, 35, NULL, '2026-09-16', '2026-11-05', 100, 0), (2, 1, 10, 36, 'Komodo Dragon', '2026-11-11', '2026-12-30', 100, 0)")
        .execute(&pool)
        .await
        .unwrap();
    run_script(&pool, common::CB_EVENTS).await.unwrap();
    let store = Attendance::with_pool(pool).await.unwrap();
    let season_35 = store.event(1).await.unwrap().unwrap();
    assert_eq!(season_35.name, "Clan Battles · Season 35");
    assert_eq!(season_35.span.first_day(), day("2026-09-16"));
    assert_eq!(season_35.span.last_day(), day("2026-11-05"));
    assert_eq!(season_35.span.schedule().weekdays(), clan_battle_days());
    assert_clan_battle_time(&season_35);
    let season_36 = store.event(2).await.unwrap().unwrap();
    assert_eq!(season_36.name, "Clan Battles · Season 36: Komodo Dragon");
    assert_eq!(season_36.span.first_day(), day("2026-11-11"));
    assert_eq!(season_36.span.last_day(), day("2026-12-30"));
    assert_eq!(season_36.span.schedule().weekdays(), clan_battle_days());
    assert_clan_battle_time(&season_36);
}

#[tokio::test]
async fn migration_0008_keeps_ended_and_every_day_seasons() {
    let pool = seasons_pool().await;
    sqlx::raw_sql("INSERT INTO cb_seasons (id, guild_id, channel_id, number, codename, first_day, last_day, created_by, created_at_ms, ended_at_ms, every_day) VALUES (1, 1, 10, 34, NULL, '2026-06-10', '2026-08-02', 100, 0, 1700000900000, 0), (2, 1, 10, 99, NULL, '2026-09-18', '2026-09-20', 100, 0, NULL, 1)")
        .execute(&pool)
        .await
        .unwrap();
    run_script(&pool, common::CB_EVENTS).await.unwrap();
    let store = Attendance::with_pool(pool).await.unwrap();
    let ended = store.event(1).await.unwrap().unwrap();
    assert_eq!(ended.ended_at_ms, Some(1_700_000_900_000));
    assert_clan_battle_time(&ended);
    let rehearsal = store.event(2).await.unwrap().unwrap();
    assert_eq!(
        rehearsal.span.schedule().weekdays(),
        Weekdays::new(&[
            Weekday::Monday,
            Weekday::Tuesday,
            Weekday::Wednesday,
            Weekday::Thursday,
            Weekday::Friday,
            Weekday::Saturday,
            Weekday::Sunday,
        ])
        .unwrap()
    );
    assert_eq!(rehearsal.ended_at_ms, None);
    assert_clan_battle_time(&rehearsal);
}

#[tokio::test]
async fn migration_0008_keeps_ping_roles_posts_and_answers() {
    let pool = seasons_pool().await;
    sqlx::raw_sql("INSERT INTO cb_seasons (id, guild_id, channel_id, number, codename, first_day, last_day, created_by, created_at_ms) VALUES (1, 1, 10, 35, NULL, '2026-10-14', '2026-10-18', 100, 0); INSERT INTO cb_season_ping_roles (season_id, position, role_id) VALUES (1, 1, 4343), (1, 2, 4242); INSERT INTO cb_posts (season_id, night, message_id, state, posted_at_ms) VALUES (1, '2026-10-14', 7, 'open', 1791934200000); INSERT INTO cb_marks (season_id, night, user_id, hour, attending, answered_at_ms, changed_at_ms) VALUES (1, '2026-10-14', 200, 1, 1, 1000, 1000), (1, '2026-10-14', 200, 2, 0, 1000, 1000), (1, '2026-10-14', 200, 3, 1, 1000, 1000), (1, '2026-10-14', 200, 4, 0, 1000, 1000)")
        .execute(&pool)
        .await
        .unwrap();
    run_script(&pool, common::CB_EVENTS).await.unwrap();
    let store = Attendance::with_pool(pool).await.unwrap();
    let event = store.event(1).await.unwrap().unwrap();
    assert_eq!(event.ping_roles, vec![RESERVES, CREWMATES]);
    let posts = store.posts(&event).await.unwrap();
    assert_eq!(posts.len(), 1);
    assert_eq!(posts[0].session.date(), day("2026-10-14"));
    assert_eq!(posts[0].session.start_unix(), 1_792_020_600);
    assert_eq!(posts[0].message, Snowflake::new(7));
    assert_eq!(posts[0].state, PostState::Open);
    assert_eq!(posts[0].posted_at_ms, 1_791_934_200_000);
    assert_eq!(
        store.roster(1, day("2026-10-14")).await.unwrap(),
        answered(AKI, &[true, false, true, false], 1_000)
    );
}

#[tokio::test]
async fn migration_0008_runs_once() {
    let pool = attendance_pool().await;
    let store = Attendance::with_pool(pool.clone()).await.unwrap();
    let event = season_35(&store).await;
    let error = run_script(&pool, common::CB_EVENTS).await.err().unwrap();
    assert!(
        error
            .to_string()
            .contains("UNIQUE constraint failed: schema_migrations.name"),
        "{error}"
    );
    rolled_back(&pool).await;
    assert_eq!(store.events_in(GUILD).await.unwrap(), vec![event]);
}

async fn table_columns(pool: &SqlitePool, table: &str) -> Vec<String> {
    sqlx::query_scalar("SELECT name FROM pragma_table_info(?) ORDER BY cid")
        .bind(table)
        .fetch_all(pool)
        .await
        .unwrap()
}

#[tokio::test]
async fn the_0008_rollback_restores_seasons() {
    let pool = seasons_pool().await;
    sqlx::raw_sql("INSERT INTO cb_seasons (id, guild_id, channel_id, number, codename, first_day, last_day, created_by, created_at_ms) VALUES (1, 1, 10, 35, NULL, '2026-10-14', '2026-10-18', 100, 0); INSERT INTO cb_season_ping_roles (season_id, position, role_id) VALUES (1, 1, 4242); INSERT INTO cb_posts (season_id, night, message_id, state, posted_at_ms) VALUES (1, '2026-10-14', 7, 'open', 0); INSERT INTO cb_marks (season_id, night, user_id, hour, attending, answered_at_ms, changed_at_ms) VALUES (1, '2026-10-14', 200, 1, 1, 1000, 1000), (1, '2026-10-14', 200, 2, 0, 1000, 1000), (1, '2026-10-14', 200, 3, 0, 1000, 1000), (1, '2026-10-14', 200, 4, 0, 1000, 1000)")
        .execute(&pool)
        .await
        .unwrap();
    run_script(&pool, common::CB_EVENTS).await.unwrap();
    let store = Attendance::with_pool(pool.clone()).await.unwrap();
    let seven_days = created(
        &store,
        &NewEvent {
            span: every_day("2026-10-14", "2026-10-18"),
            ping_roles: vec![RESERVES],
            ..proposal("Drill", "2026-10-14", "2026-10-18")
        },
    )
    .await;
    store
        .record_post(seven_days.id, day("2026-10-15"), Snowflake::new(8), 0)
        .await
        .unwrap();
    store
        .mark(
            seven_days.id,
            day("2026-10-15"),
            BOREALIS,
            &[hour(2)],
            hours(4),
            true,
            2_000,
        )
        .await
        .unwrap();
    let season_before = posts_and_answers(&pool, 1).await;
    let seven_days_before = posts_and_answers(&pool, seven_days.id).await;
    run_script(&pool, common::CB_EVENTS_DOWN).await.unwrap();
    assert_eq!(table_columns(&pool, "cb_seasons").await, SEASONS_AT_0007);
    let rows: Vec<(i64, i64, Option<String>, i64)> =
        sqlx::query_as("SELECT id, number, codename, every_day FROM cb_seasons ORDER BY id")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(
        rows,
        vec![
            (1, 1, Some("Clan Battles · Season 35".to_owned()), 0),
            (seven_days.id, seven_days.id, Some("Drill".to_owned()), 1),
        ]
    );
    let roles: Vec<(i64, i64)> =
        sqlx::query_as("SELECT season_id, role_id FROM cb_season_ping_roles ORDER BY season_id")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(roles, vec![(1, 4242), (seven_days.id, 4343)]);
    assert_eq!(posts_and_answers(&pool, 1).await, season_before);
    assert_eq!(
        posts_and_answers(&pool, seven_days.id).await,
        seven_days_before
    );
    let ledger: Vec<String> = sqlx::query_scalar("SELECT name FROM schema_migrations")
        .fetch_all(&pool)
        .await
        .unwrap();
    assert!(!ledger.contains(&"0008_events".to_owned()));
}

#[tokio::test]
async fn the_0008_rollback_refuses_what_0007_cannot_hold() {
    let pool = attendance_pool().await;
    let store = Attendance::with_pool(pool.clone()).await.unwrap();
    let denver = created(
        &store,
        &NewEvent {
            span: Span::new(
                day("2026-10-14"),
                day("2026-10-28"),
                Schedule::new(
                    clan_battle_days(),
                    StartTime::parse("17:00").unwrap(),
                    Zone::find("America/Denver").unwrap(),
                    hours(4),
                ),
            )
            .unwrap(),
            ..proposal("Ranked Sprint", "2026-10-14", "2026-10-28")
        },
    )
    .await;
    let denver_before = event_state(&pool, denver.id).await;
    let answers_before = every_answer(&pool).await;
    assert!(run_script(&pool, common::CB_EVENTS_DOWN).await.is_err());
    rolled_back(&pool).await;
    assert_eq!(event_state(&pool, denver.id).await, denver_before);
    assert_eq!(every_answer(&pool).await, answers_before);
    assert_eq!(store.event(denver.id).await.unwrap(), Some(denver));
    assert!(
        table_columns(&pool, "cb_seasons")
            .await
            .contains(&"time_zone".to_owned())
    );

    let pool = attendance_pool().await;
    let store = Attendance::with_pool(pool.clone()).await.unwrap();
    let long = created(
        &store,
        &NewEvent {
            span: Span::new(
                day("2026-10-14"),
                day("2026-10-18"),
                Schedule::new(
                    clan_battle_days(),
                    StartTime::parse("23:30").unwrap(),
                    Zone::find("UTC").unwrap(),
                    hours(6),
                ),
            )
            .unwrap(),
            ..proposal("Long Drill", "2026-10-14", "2026-10-18")
        },
    )
    .await;
    store
        .record_post(long.id, day("2026-10-14"), Snowflake::new(7), 0)
        .await
        .unwrap();
    store
        .mark(
            long.id,
            day("2026-10-14"),
            AKI,
            &[hour(5)],
            hours(6),
            true,
            1_000,
        )
        .await
        .unwrap();
    let long_before = event_state(&pool, long.id).await;
    let answers_before = every_answer(&pool).await;
    assert!(run_script(&pool, common::CB_EVENTS_DOWN).await.is_err());
    rolled_back(&pool).await;
    assert_eq!(event_state(&pool, long.id).await, long_before);
    assert_eq!(every_answer(&pool).await, answers_before);
    assert_eq!(store.event(long.id).await.unwrap(), Some(with_posts(&long)));
}

#[tokio::test]
async fn the_0008_rollback_refuses_an_answer_above_hour_4_on_a_4_hour_event() {
    let pool = attendance_pool().await;
    let store = Attendance::with_pool(pool.clone()).await.unwrap();
    let event = season_35(&store).await;
    store
        .record_post(event.id, day("2026-09-16"), Snowflake::new(7), 0)
        .await
        .unwrap();
    store
        .mark(
            event.id,
            day("2026-09-16"),
            AKI,
            &[hour(5)],
            hours(5),
            true,
            1_000,
        )
        .await
        .unwrap();
    assert!(run_script(&pool, common::CB_EVENTS_DOWN).await.is_err());
    rolled_back(&pool).await;
    assert_eq!(counted(&pool, MARKS_OF_EVENT, event.id).await, 5);
    assert!(
        table_columns(&pool, "cb_seasons")
            .await
            .contains(&"name".to_owned())
    );
}

const EDITED_TODAY: &str = "2026-10-13";
const EDITED_AT: i64 = 1_791_900_000;

fn denver_wednesdays(first: &str, last: &str, count: i64) -> Span {
    Span::new(
        day(first),
        day(last),
        Schedule::new(
            Weekdays::new(&[Weekday::Wednesday]).unwrap(),
            StartTime::parse("17:00").unwrap(),
            Zone::find("America/Denver").unwrap(),
            hours(count),
        ),
    )
    .unwrap()
}

fn sprint(name: &str, count: i64, created_at_ms: u64) -> NewEvent {
    NewEvent {
        span: denver_wednesdays("2026-10-14", "2026-10-28", count),
        created_at_ms,
        ..proposal(name, "2026-10-14", "2026-10-28")
    }
}

async fn edited(
    store: &Attendance,
    id: i64,
    created_at_ms: u64,
    change: &EventChange,
) -> EditOutcome {
    store
        .edit_event(
            GUILD,
            id,
            created_at_ms,
            change,
            day(EDITED_TODAY),
            EDITED_AT,
        )
        .await
        .unwrap()
}

async fn name_of(pool: &SqlitePool, id: i64) -> String {
    sqlx::query_scalar("SELECT name FROM cb_seasons WHERE id = ?")
        .bind(id)
        .fetch_one(pool)
        .await
        .unwrap()
}

#[tokio::test]
async fn an_edit_changes_only_the_fields_given() {
    let store = attendance().await;
    let event = created(&store, &sprint("Ranked Sprint", 4, CREATED_AT)).await;
    let renamed = EventChange {
        name: Some("Ranked Finals".to_owned()),
        ..EventChange::default()
    };
    assert!(matches!(
        edited(&store, event.id, CREATED_AT, &renamed).await,
        EditOutcome::Edited { .. }
    ));
    let later = EventChange {
        start: Some(StartTime::parse("18:00").unwrap()),
        ..EventChange::default()
    };
    let EditOutcome::Edited { after, .. } = edited(&store, event.id, CREATED_AT, &later).await
    else {
        panic!("the start time was not saved");
    };
    assert_eq!(after.name, "Ranked Finals");
    assert_eq!(
        after.span.schedule().start(),
        StartTime::parse("18:00").unwrap()
    );
    let stored = store.live_event(GUILD, event.id).await.unwrap().unwrap();
    assert_eq!(stored.name, "Ranked Finals");
    assert_eq!(
        stored.span.schedule().start(),
        StartTime::parse("18:00").unwrap()
    );
    assert_eq!(stored.span.schedule().hours(), hours(4));
    assert_eq!(
        stored.span.schedule().zone(),
        &Zone::find("America/Denver").unwrap()
    );
    assert_eq!(stored.span.first_day(), day("2026-10-14"));
    assert_eq!(stored.span.last_day(), day("2026-10-28"));
    assert_eq!(stored.channel, CHANNEL);
}

#[tokio::test]
async fn lowering_the_hours_drops_answers_above_them() {
    let pool = attendance_pool().await;
    let store = Attendance::with_pool(pool.clone()).await.unwrap();
    let event = created(&store, &sprint("Ranked Sprint", 6, CREATED_AT)).await;
    store
        .record_post(event.id, day("2026-10-14"), Snowflake::new(7), CREATED_AT)
        .await
        .unwrap();
    store
        .mark(
            event.id,
            day("2026-10-14"),
            AKI,
            &[hour(1), hour(2), hour(3), hour(4), hour(5), hour(6)],
            hours(6),
            true,
            1_000,
        )
        .await
        .unwrap();
    let fewer = EventChange {
        hours: Some(hours(4)),
        ..EventChange::default()
    };
    assert!(matches!(
        edited(&store, event.id, CREATED_AT, &fewer).await,
        EditOutcome::Edited { .. }
    ));
    let kept: Vec<i64> =
        sqlx::query_scalar("SELECT hour FROM cb_marks WHERE season_id = ? ORDER BY hour")
            .bind(event.id)
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(kept, vec![1, 2, 3, 4]);
}

#[tokio::test]
async fn saving_an_edit_of_an_ended_event_is_refused() {
    let pool = attendance_pool().await;
    let store = Attendance::with_pool(pool.clone()).await.unwrap();
    let event = created(&store, &sprint("Ranked Sprint", 4, CREATED_AT)).await;
    store
        .record_post(event.id, day("2026-10-14"), Snowflake::new(7), CREATED_AT)
        .await
        .unwrap();
    store
        .mark(
            event.id,
            day("2026-10-14"),
            AKI,
            &[hour(1)],
            hours(4),
            true,
            CREATED_AT,
        )
        .await
        .unwrap();
    store.end_event(GUILD, event.id, ENDED_AT).await.unwrap();
    let before = event_state(&pool, event.id).await;
    let renamed = EventChange {
        name: Some("Ranked Finals".to_owned()),
        ..EventChange::default()
    };
    assert_eq!(
        edited(&store, event.id, CREATED_AT, &renamed).await,
        EditOutcome::NotFound
    );
    assert_eq!(event_state(&pool, event.id).await, before);
    assert_eq!(name_of(&pool, event.id).await, "Ranked Sprint");
    assert_eq!(
        ended_at(&pool, event.id).await,
        Some(i64::try_from(ENDED_AT).unwrap())
    );
}

#[tokio::test]
async fn saving_an_edit_after_the_id_was_reused_is_refused() {
    let pool = attendance_pool().await;
    let store = Attendance::with_pool(pool.clone()).await.unwrap();
    created(&store, &sprint("Ranked Warmup", 4, CREATED_AT)).await;
    created(&store, &sprint("Ranked Qualifier", 4, CREATED_AT)).await;
    let opened = created(&store, &sprint("Ranked Sprint", 4, 1_791_800_000_000)).await;
    assert_eq!(opened.id, 3);
    assert_eq!(
        store.end_event(GUILD, 3, ENDED_AT).await.unwrap(),
        EndOutcome::Removed
    );
    let taken = created(&store, &sprint("Ranked Relay", 4, 1_791_900_000_000)).await;
    assert_eq!(taken.id, 3);
    let renamed = EventChange {
        name: Some("Ranked Finals".to_owned()),
        ..EventChange::default()
    };
    assert_eq!(
        edited(&store, 3, 1_791_800_000_000, &renamed).await,
        EditOutcome::NotFound
    );
    assert_eq!(name_of(&pool, 3).await, "Ranked Relay");
}
