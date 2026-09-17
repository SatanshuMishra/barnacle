mod common;

use barnacle_bot::attendance_store::Attendance;
use barnacle_bot::attendance_store::AttendanceError;
use barnacle_bot::attendance_store::CreateOutcome;
use barnacle_bot::attendance_store::EndOutcome;
use barnacle_bot::attendance_store::Mark;
use barnacle_bot::attendance_store::NewSeason;
use barnacle_bot::attendance_store::PostState;
use barnacle_bot::attendance_store::Season;
use barnacle_bot::ids::ChannelId;
use barnacle_bot::ids::GuildId;
use barnacle_bot::schedule::Hour;
use barnacle_bot::schedule::Night;
use barnacle_bot::schedule::Range;
use barnacle_bot::schedule::parse_day;
use barnacle_guess::Snowflake;
use barnacle_guess::UserId;
use common::attendance;
use common::attendance_pool;
use common::memory_pool;
use jiff::civil::Date;
use sqlx::sqlite::SqlitePool;

const GUILD: GuildId = GuildId::new(1);
const OTHER_GUILD: GuildId = GuildId::new(2);
const CHANNEL: ChannelId = ChannelId::new(10);
const MANAGER: UserId = UserId::new(100);
const AKI: UserId = UserId::new(200);
const BOREALIS: UserId = UserId::new(300);
const CREATED_AT: u64 = 1_700_000_000_000;

fn day(text: &str) -> Date {
    parse_day(text).unwrap()
}

fn night(text: &str) -> Night {
    Night::parse(text).unwrap()
}

fn hour(value: u8) -> Hour {
    Hour::new(value).unwrap()
}

fn proposal(number: u32, first: &str, last: &str) -> NewSeason {
    NewSeason {
        guild: GUILD,
        channel: CHANNEL,
        number,
        codename: Some("Komodo Dragon".to_owned()),
        range: Range::new(day(first), day(last)).unwrap(),
        created_by: MANAGER,
        created_at_ms: CREATED_AT,
    }
}

async fn created(store: &Attendance, new: &NewSeason) -> Season {
    match store.create_season(new).await.unwrap() {
        CreateOutcome::Created(season) => season,
        other => panic!("expected a created season, got {other:?}"),
    }
}

async fn season_35(store: &Attendance) -> Season {
    created(store, &proposal(35, "2026-09-16", "2026-11-05")).await
}

#[tokio::test]
async fn create_season_rejects_a_duplicate_number() {
    let store = attendance().await;
    let season = season_35(&store).await;
    assert_eq!(season.number, 35);
    assert_eq!(season.guild, GUILD);
    assert_eq!(season.codename.as_deref(), Some("Komodo Dragon"));
    assert_eq!(
        store
            .create_season(&proposal(35, "2027-01-06", "2027-02-24"))
            .await
            .unwrap(),
        CreateOutcome::NumberTaken
    );
    let elsewhere = NewSeason {
        guild: OTHER_GUILD,
        ..proposal(35, "2026-09-16", "2026-11-05")
    };
    assert!(matches!(
        store.create_season(&elsewhere).await.unwrap(),
        CreateOutcome::Created(_)
    ));
    assert_eq!(store.seasons_in(GUILD).await.unwrap(), vec![season]);
}

#[tokio::test]
async fn create_season_rejects_an_overlapping_range() {
    let store = attendance().await;
    let season_34 = created(&store, &proposal(34, "2026-06-10", "2026-08-02")).await;
    let outcome = store
        .create_season(&proposal(36, "2026-08-02", "2026-09-20"))
        .await
        .unwrap();
    assert_eq!(outcome, CreateOutcome::Overlaps(season_34));
    assert!(matches!(
        store
            .create_season(&proposal(36, "2026-08-03", "2026-09-20"))
            .await
            .unwrap(),
        CreateOutcome::Created(_)
    ));
}

#[tokio::test]
async fn end_season_removes_a_season_with_no_posts() {
    let store = attendance().await;
    let season = season_35(&store).await;
    assert_eq!(
        store.end_season(GUILD, 35).await.unwrap(),
        EndOutcome::Removed
    );
    assert_eq!(store.season(season.id).await.unwrap(), None);
    assert_eq!(
        store.end_season(GUILD, 35).await.unwrap(),
        EndOutcome::NotFound
    );
}

#[tokio::test]
async fn end_season_shortens_a_season_that_has_posted() {
    let store = attendance().await;
    let season = season_35(&store).await;
    store
        .record_post(
            season.id,
            night("2026-09-16"),
            Snowflake::new(7),
            CREATED_AT,
        )
        .await
        .unwrap();
    store
        .record_post(
            season.id,
            night("2026-10-01"),
            Snowflake::new(8),
            CREATED_AT,
        )
        .await
        .unwrap();
    assert_eq!(
        store.end_season(GUILD, 35).await.unwrap(),
        EndOutcome::Shortened {
            last_day: day("2026-10-01")
        }
    );
    let shortened = store.season(season.id).await.unwrap().unwrap();
    assert_eq!(shortened.range.last_day(), day("2026-10-01"));
    assert_eq!(shortened.range.first_day(), day("2026-09-16"));
}

async fn posted_season(pool: SqlitePool) -> (Attendance, Season) {
    let store = Attendance::with_pool(pool).await.unwrap();
    let season = season_35(&store).await;
    store
        .record_post(
            season.id,
            night("2026-09-16"),
            Snowflake::new(7),
            CREATED_AT,
        )
        .await
        .unwrap();
    (store, season)
}

#[tokio::test]
async fn marks_upsert_and_keep_the_first_answer_time() {
    let pool = attendance_pool().await;
    let (store, season) = posted_season(pool.clone()).await;
    let night = night("2026-09-16");
    store
        .mark(season.id, night, AKI, &[hour(1)], true, 1_000)
        .await
        .unwrap();
    store
        .mark(season.id, night, AKI, &[hour(1)], false, 5_000)
        .await
        .unwrap();
    assert_eq!(
        store.roster(season.id, night).await.unwrap(),
        vec![Mark {
            user: AKI,
            hour: hour(1),
            attending: false,
            answered_at_ms: 1_000,
        }]
    );
    let changed: i64 = sqlx::query_scalar("SELECT changed_at_ms FROM cb_marks")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(changed, 5_000);
}

#[tokio::test]
async fn roster_is_ordered_by_first_answer() {
    let (store, season) = posted_season(attendance_pool().await).await;
    let night = night("2026-09-16");
    store
        .mark(season.id, night, BOREALIS, &[hour(2), hour(1)], true, 2_000)
        .await
        .unwrap();
    store
        .mark(season.id, night, AKI, &Hour::ALL, false, 3_000)
        .await
        .unwrap();
    store
        .mark(season.id, night, BOREALIS, &[hour(3)], false, 9_000)
        .await
        .unwrap();
    let roster = store.roster(season.id, night).await.unwrap();
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
            (AKI, 1, false, 3_000),
            (AKI, 2, false, 3_000),
            (AKI, 3, false, 3_000),
            (AKI, 4, false, 3_000),
            (BOREALIS, 3, false, 9_000),
        ]
    );
}

#[tokio::test]
async fn live_seasons_drops_a_season_whose_posts_are_all_removed() {
    let store = attendance().await;
    let season = season_35(&store).await;
    let last_moment = season.range.last_moment_unix().unwrap();
    assert_eq!(
        store.live_seasons(last_moment - 1).await.unwrap(),
        vec![season.clone()]
    );
    assert!(store.live_seasons(last_moment).await.unwrap().is_empty());
    let next = created(&store, &proposal(36, "2026-11-11", "2026-12-30")).await;
    assert_eq!(
        store.live_seasons(last_moment).await.unwrap(),
        vec![next.clone()]
    );
    assert_eq!(
        store.live_seasons(last_moment - 1).await.unwrap(),
        vec![season, next]
    );
}

#[tokio::test]
async fn live_seasons_keeps_a_season_until_its_last_post_is_removed() {
    let store = attendance().await;
    let season = season_35(&store).await;
    let last = season.range.nights().last().unwrap();
    let last_moment = season.range.last_moment_unix().unwrap();
    store
        .record_post(season.id, last, Snowflake::new(500), CREATED_AT)
        .await
        .unwrap();
    assert_eq!(
        store.live_seasons(last_moment).await.unwrap(),
        vec![season.clone()]
    );
    assert_eq!(
        store.live_seasons(last_moment + 86_400).await.unwrap(),
        vec![season.clone()]
    );
    store
        .set_post_state(season.id, last, PostState::Removed)
        .await
        .unwrap();
    assert!(store.live_seasons(last_moment).await.unwrap().is_empty());
}

#[tokio::test]
async fn a_season_number_above_the_ceiling_is_named_as_one() {
    let pool = attendance_pool().await;
    let store = Attendance::with_pool(pool.clone()).await.unwrap();
    sqlx::query("INSERT INTO cb_seasons (id, guild_id, channel_id, number, codename, first_day, last_day, created_by, created_at_ms) VALUES (1, 1, 10, 5000000000, NULL, '2026-09-16', '2026-11-05', 100, 0)")
        .execute(&pool)
        .await
        .unwrap();
    let error = store.season(1).await.err().unwrap();
    assert!(matches!(
        error,
        AttendanceError::BadSeasonNumber {
            value: 5_000_000_000
        }
    ));
    assert_eq!(
        error.to_string(),
        "the attendance database holds 5000000000, which is not a season number"
    );
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
        "CREATE TABLE cb_seasons (id INTEGER PRIMARY KEY, guild_id INTEGER NOT NULL, channel_id INTEGER NOT NULL, number INTEGER NOT NULL, codename TEXT, first_day TEXT NOT NULL, last_day TEXT NOT NULL, created_by INTEGER NOT NULL, created_at_ms INTEGER NOT NULL) STRICT",
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
async fn a_post_records_its_state_and_message() {
    let (store, season) = posted_season(attendance_pool().await).await;
    let first = night("2026-09-16");
    let post = store.post(season.id, first).await.unwrap().unwrap();
    assert_eq!(post.night, first);
    assert_eq!(post.message, Snowflake::new(7));
    assert_eq!(post.state, PostState::Open);
    assert_eq!(post.posted_at_ms, CREATED_AT);
    store
        .set_post_state(season.id, first, PostState::Closed)
        .await
        .unwrap();
    assert_eq!(
        store.posts(season.id).await.unwrap()[0].state,
        PostState::Closed
    );
    assert!(
        store
            .record_post(season.id, first, Snowflake::new(9), CREATED_AT)
            .await
            .is_err()
    );
    assert_eq!(
        store.post(season.id, night("2026-09-17")).await.unwrap(),
        None
    );
}
