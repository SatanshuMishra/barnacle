mod common;

use barnacle_bot::attendance_store::Attendance;
use barnacle_bot::attendance_store::AttendanceError;
use barnacle_bot::attendance_store::CreateOutcome;
use barnacle_bot::attendance_store::EditOutcome;
use barnacle_bot::attendance_store::EndOutcome;
use barnacle_bot::attendance_store::Mark;
use barnacle_bot::attendance_store::NewSeason;
use barnacle_bot::attendance_store::PostState;
use barnacle_bot::attendance_store::Season;
use barnacle_bot::attendance_store::SeasonChange;
use barnacle_bot::ids::ChannelId;
use barnacle_bot::ids::GuildId;
use barnacle_bot::ids::RoleId;
use barnacle_bot::schedule::Hour;
use barnacle_bot::schedule::Night;
use barnacle_bot::schedule::Range;
use barnacle_bot::schedule::parse_day;
use barnacle_guess::Snowflake;
use barnacle_guess::UserId;
use common::attendance_pool;
use common::memory_pool;
use jiff::civil::Date;
use sqlx::sqlite::SqlitePool;

const TODAY: &str = "2026-09-16";

const GUILD: GuildId = GuildId::new(1);
const OTHER_GUILD: GuildId = GuildId::new(2);
const CHANNEL: ChannelId = ChannelId::new(10);
const MANAGER: UserId = UserId::new(100);
const AKI: UserId = UserId::new(200);
const BOREALIS: UserId = UserId::new(300);
const CREWMATES: RoleId = RoleId::new(4242);
const RESERVES: RoleId = RoleId::new(4343);
const CREATED_AT: u64 = 1_700_000_000_000;
const ENDED_AT: u64 = 1_700_000_900_000;

const POSTS_OF_SEASON: &str = "SELECT COUNT(*) FROM cb_posts WHERE season_id = ?";
const MARKS_OF_SEASON: &str = "SELECT COUNT(*) FROM cb_marks WHERE season_id = ?";

async fn attendance() -> Attendance {
    Attendance::with_pool(attendance_pool().await)
        .await
        .unwrap()
}

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
        ping_role: None,
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

async fn pinging_season_35(store: &Attendance) -> Season {
    let new = NewSeason {
        ping_role: Some(CREWMATES),
        ..proposal(35, "2026-09-16", "2026-11-05")
    };
    created(store, &new).await
}

async fn edited(store: &Attendance, number: u32, change: &SeasonChange) -> (Season, Season) {
    match store
        .edit_season(GUILD, number, change, day(TODAY))
        .await
        .unwrap()
    {
        EditOutcome::Edited { before, after } => (before, after),
        other => panic!("expected an edited season, got {other:?}"),
    }
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
async fn a_season_stores_and_returns_its_ping_role() {
    let store = attendance().await;
    let season = pinging_season_35(&store).await;
    assert_eq!(season.ping_role, Some(CREWMATES));
    assert_eq!(season.ended_at_ms, None);
    assert_eq!(store.season(season.id).await.unwrap(), Some(season.clone()));
    assert_eq!(store.live_season(GUILD, 35).await.unwrap(), Some(season));
    assert_eq!(store.live_season(GUILD, 36).await.unwrap(), None);
    assert_eq!(store.live_season(OTHER_GUILD, 35).await.unwrap(), None);
}

#[tokio::test]
async fn end_season_removes_a_season_with_no_posts() {
    let store = attendance().await;
    let season = season_35(&store).await;
    assert_eq!(
        store.end_season(GUILD, 35, ENDED_AT).await.unwrap(),
        EndOutcome::Removed
    );
    assert_eq!(store.season(season.id).await.unwrap(), None);
    assert_eq!(
        store.end_season(GUILD, 35, ENDED_AT).await.unwrap(),
        EndOutcome::NotFound
    );
}

#[tokio::test]
async fn end_season_marks_a_season_that_has_posted() {
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
    assert_eq!(
        store.end_season(GUILD, 35, ENDED_AT).await.unwrap(),
        EndOutcome::Ended {
            season: season.clone()
        }
    );
    let ended = store.season(season.id).await.unwrap().unwrap();
    assert_eq!(ended.ended_at_ms, Some(ENDED_AT));
    assert_eq!(ended.range, season.range);
    assert_eq!(
        store.end_season(GUILD, 35, ENDED_AT).await.unwrap(),
        EndOutcome::NotFound
    );
}

#[tokio::test]
async fn an_ended_season_frees_its_number() {
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
    assert_eq!(
        store.end_season(GUILD, 35, ENDED_AT).await.unwrap(),
        EndOutcome::Ended {
            season: season.clone()
        }
    );
    let again = season_35(&store).await;
    assert_eq!(again.number, 35);
    assert_ne!(again.id, season.id);
    assert_eq!(store.live_season(GUILD, 35).await.unwrap(), Some(again));
}

#[tokio::test]
async fn an_ended_season_is_not_live_and_not_listed() {
    let store = attendance().await;
    let season = season_35(&store).await;
    let last_moment = season.range.last_moment_unix().unwrap();
    store
        .record_post(
            season.id,
            night("2026-09-16"),
            Snowflake::new(7),
            CREATED_AT,
        )
        .await
        .unwrap();
    assert_eq!(
        store.live_seasons(last_moment - 1).await.unwrap(),
        vec![season.clone()]
    );
    assert_eq!(store.seasons_in(GUILD).await.unwrap(), vec![season.clone()]);
    store.end_season(GUILD, 35, ENDED_AT).await.unwrap();
    assert!(
        store
            .live_seasons(last_moment - 1)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(store.seasons_in(GUILD).await.unwrap().is_empty());
    assert_eq!(store.live_season(GUILD, 35).await.unwrap(), None);
}

#[tokio::test]
async fn a_click_can_still_resolve_an_ended_season() {
    let store = attendance().await;
    let season = pinging_season_35(&store).await;
    store
        .record_post(
            season.id,
            night("2026-09-16"),
            Snowflake::new(7),
            CREATED_AT,
        )
        .await
        .unwrap();
    store.end_season(GUILD, 35, ENDED_AT).await.unwrap();
    let found = store.season(season.id).await.unwrap().unwrap();
    assert_eq!(found.number, 35);
    assert_eq!(found.guild, GUILD);
    assert_eq!(found.channel, CHANNEL);
    assert_eq!(found.ping_role, Some(CREWMATES));
    assert_eq!(found.ended_at_ms, Some(ENDED_AT));
}

#[tokio::test]
async fn edit_season_rejects_a_number_another_live_season_holds() {
    let store = attendance().await;
    let season_34 = created(&store, &proposal(34, "2026-06-10", "2026-08-02")).await;
    let season = season_35(&store).await;
    let take_34 = SeasonChange {
        number: Some(34),
        ..SeasonChange::default()
    };
    assert_eq!(
        store
            .edit_season(GUILD, 35, &take_34, day(TODAY))
            .await
            .unwrap(),
        EditOutcome::NumberTaken
    );
    assert_eq!(
        store.live_season(GUILD, 35).await.unwrap(),
        Some(season.clone())
    );
    let keep_35 = SeasonChange {
        number: Some(35),
        ..SeasonChange::default()
    };
    let (before, after) = edited(&store, 35, &keep_35).await;
    assert_eq!(before, season);
    assert_eq!(after, season);
    store.end_season(GUILD, 34, ENDED_AT).await.unwrap();
    assert_eq!(store.season(season_34.id).await.unwrap(), None);
    let (_, renumbered) = edited(&store, 35, &take_34).await;
    assert_eq!(renumbered.number, 34);
}

#[tokio::test]
async fn edit_season_ignores_an_ended_season_when_checking_overlap() {
    let store = attendance().await;
    let season_34 = created(&store, &proposal(34, "2026-06-10", "2026-08-02")).await;
    store
        .record_post(
            season_34.id,
            night("2026-06-10"),
            Snowflake::new(7),
            CREATED_AT,
        )
        .await
        .unwrap();
    let season = season_35(&store).await;
    let reach_back = SeasonChange {
        first_day: Some(day("2026-07-01")),
        ..SeasonChange::default()
    };
    assert_eq!(
        store
            .edit_season(GUILD, 35, &reach_back, day(TODAY))
            .await
            .unwrap(),
        EditOutcome::Overlaps(season_34)
    );
    store.end_season(GUILD, 34, ENDED_AT).await.unwrap();
    let (before, after) = edited(&store, 35, &reach_back).await;
    assert_eq!(before, season);
    assert_eq!(after.range.first_day(), day("2026-07-01"));
    assert_eq!(after.range.last_day(), day("2026-11-05"));
}

#[tokio::test]
async fn edit_season_rejects_a_backwards_range() {
    let store = attendance().await;
    let season = season_35(&store).await;
    let backwards = SeasonChange {
        last_day: Some(day("2026-09-15")),
        ..SeasonChange::default()
    };
    assert_eq!(
        store
            .edit_season(GUILD, 35, &backwards, day(TODAY))
            .await
            .unwrap(),
        EditOutcome::BadRange
    );
    assert_eq!(store.live_season(GUILD, 35).await.unwrap(), Some(season));
    assert_eq!(
        store
            .edit_season(GUILD, 36, &backwards, day(TODAY))
            .await
            .unwrap(),
        EditOutcome::NotFound
    );
}

#[tokio::test]
async fn edit_season_changes_only_what_it_is_given() {
    let store = attendance().await;
    let season = pinging_season_35(&store).await;
    let renumber = SeasonChange {
        number: Some(36),
        ..SeasonChange::default()
    };
    let (before, after) = edited(&store, 35, &renumber).await;
    assert_eq!(before, season);
    assert_eq!(after.number, 36);
    assert_eq!(after.id, season.id);
    assert_eq!(after.guild, season.guild);
    assert_eq!(after.channel, season.channel);
    assert_eq!(after.codename, season.codename);
    assert_eq!(after.range, season.range);
    assert_eq!(after.created_by, season.created_by);
    assert_eq!(after.created_at_ms, season.created_at_ms);
    assert_eq!(after.ping_role, season.ping_role);
    assert_eq!(after.ended_at_ms, None);
    assert_eq!(store.live_season(GUILD, 35).await.unwrap(), None);
    assert_eq!(store.live_season(GUILD, 36).await.unwrap(), Some(after));
}

#[tokio::test]
async fn edit_season_clears_a_codename_and_a_ping_role() {
    let store = attendance().await;
    let season = pinging_season_35(&store).await;
    let clear = SeasonChange {
        codename: Some(None),
        ping_role: Some(None),
        ..SeasonChange::default()
    };
    let (before, after) = edited(&store, 35, &clear).await;
    assert_eq!(before, season);
    assert_eq!(after.codename, None);
    assert_eq!(after.ping_role, None);
    assert_eq!(
        store.live_season(GUILD, 35).await.unwrap(),
        Some(after.clone())
    );
    let set = SeasonChange {
        codename: Some(Some("Basilisk".to_owned())),
        ping_role: Some(Some(RESERVES)),
        ..SeasonChange::default()
    };
    let (was, now) = edited(&store, 35, &set).await;
    assert_eq!(was, after);
    assert_eq!(now.codename.as_deref(), Some("Basilisk"));
    assert_eq!(now.ping_role, Some(RESERVES));
    assert_eq!(store.live_season(GUILD, 35).await.unwrap(), Some(now));
}

#[tokio::test]
async fn move_season_changes_the_channel_and_nothing_else() {
    let store = attendance().await;
    let season = pinging_season_35(&store).await;
    let elsewhere = ChannelId::new(11);
    assert!(
        store
            .move_season(GUILD, season.id, elsewhere)
            .await
            .unwrap()
    );
    let moved = store.season(season.id).await.unwrap().unwrap();
    assert_eq!(moved.channel, elsewhere);
    assert_eq!(
        moved,
        Season {
            channel: elsewhere,
            ..season
        }
    );
}

#[tokio::test]
async fn end_season_keeps_the_marks() {
    let store = attendance().await;
    let season = season_35(&store).await;
    let first = night("2026-09-16");
    store
        .record_post(season.id, first, Snowflake::new(7), CREATED_AT)
        .await
        .unwrap();
    store
        .mark(season.id, first, AKI, &[hour(1)], true, 1_000)
        .await
        .unwrap();
    assert_eq!(
        store.end_season(GUILD, 35, ENDED_AT).await.unwrap(),
        EndOutcome::Ended {
            season: season.clone()
        }
    );
    assert_eq!(
        store.roster(season.id, first).await.unwrap(),
        vec![Mark {
            user: AKI,
            hour: hour(1),
            attending: true,
            answered_at_ms: 1_000,
        }]
    );
    assert_eq!(store.posts(season.id).await.unwrap().len(), 1);
}

#[tokio::test]
async fn a_bad_role_id_is_named() {
    let pool = attendance_pool().await;
    sqlx::raw_sql("PRAGMA ignore_check_constraints = on")
        .execute(&pool)
        .await
        .unwrap();
    let store = Attendance::with_pool(pool.clone()).await.unwrap();
    sqlx::query("INSERT INTO cb_seasons (id, guild_id, channel_id, number, codename, first_day, last_day, created_by, created_at_ms, ping_role_id) VALUES (1, 1, 10, 35, NULL, '2026-09-16', '2026-11-05', 100, 0, 0)")
        .execute(&pool)
        .await
        .unwrap();
    let error = store.season(1).await.err().unwrap();
    assert!(matches!(error, AttendanceError::InvalidRole { value: 0 }));
    assert_eq!(
        error.to_string(),
        "the attendance database holds 0, which is not a role ID"
    );
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
        "CREATE TABLE cb_seasons (id INTEGER PRIMARY KEY, guild_id INTEGER NOT NULL, channel_id INTEGER NOT NULL, number INTEGER NOT NULL, codename TEXT, first_day TEXT NOT NULL, last_day TEXT NOT NULL, created_by INTEGER NOT NULL, created_at_ms INTEGER NOT NULL, ping_role_id INTEGER, ended_at_ms INTEGER) STRICT",
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
    store
        .record_post(season.id, first, Snowflake::new(9), CREATED_AT + 1)
        .await
        .unwrap();
    let reposted = store.post(season.id, first).await.unwrap().unwrap();
    assert_eq!(reposted.message, Snowflake::new(9));
    assert_eq!(reposted.state, PostState::Open);
    assert_eq!(reposted.posted_at_ms, CREATED_AT + 1);
    assert_eq!(store.posts(season.id).await.unwrap().len(), 1);
    assert_eq!(
        store.post(season.id, night("2026-09-17")).await.unwrap(),
        None
    );
}

#[tokio::test]
async fn a_database_without_the_live_number_index_is_named() {
    let pool = attendance_pool().await;
    sqlx::raw_sql("DROP INDEX cb_seasons_live_number")
        .execute(&pool)
        .await
        .unwrap();
    let error = Attendance::with_pool(pool).await.err().unwrap();
    assert!(matches!(
        error,
        AttendanceError::MissingIndex {
            index: "cb_seasons_live_number"
        }
    ));
    assert_eq!(
        error.to_string(),
        "the attendance database has no cb_seasons_live_number index"
    );
}

#[tokio::test]
async fn edit_season_refuses_a_range_far_from_today() {
    let store = attendance().await;
    season_35(&store).await;
    let far = SeasonChange {
        last_day: Some(day("2027-09-16")),
        ..SeasonChange::default()
    };
    assert_eq!(
        store
            .edit_season(GUILD, 35, &far, day(TODAY))
            .await
            .unwrap(),
        EditOutcome::TooFar
    );
    let back = SeasonChange {
        first_day: Some(day("2025-09-16")),
        ..SeasonChange::default()
    };
    assert_eq!(
        store
            .edit_season(GUILD, 35, &back, day(TODAY))
            .await
            .unwrap(),
        EditOutcome::TooFar
    );
}

#[tokio::test]
async fn move_season_only_moves_a_live_season_of_its_own_guild() {
    let store = attendance().await;
    let season = season_35(&store).await;
    let elsewhere = ChannelId::new(99);
    assert!(
        !store
            .move_season(OTHER_GUILD, season.id, elsewhere)
            .await
            .unwrap()
    );
    store
        .end_season(GUILD, season.number, ENDED_AT)
        .await
        .unwrap();
    assert!(
        !store
            .move_season(GUILD, season.id, elsewhere)
            .await
            .unwrap()
    );
}

async fn marked_season(store: &Attendance, new: &NewSeason) -> Season {
    let season = created(store, new).await;
    let first = season.range.nights().next().unwrap();
    store
        .record_post(season.id, first, Snowflake::new(7), CREATED_AT)
        .await
        .unwrap();
    store
        .mark(season.id, first, AKI, &[hour(1)], true, 1_000)
        .await
        .unwrap();
    season
}

async fn counted(pool: &SqlitePool, query: &'static str, season: i64) -> i64 {
    sqlx::query_scalar(query)
        .bind(season)
        .fetch_one(pool)
        .await
        .unwrap()
}

#[tokio::test]
async fn purge_season_deletes_its_marks_posts_and_season() {
    let pool = attendance_pool().await;
    let (store, season) = posted_season(pool.clone()).await;
    let first = night("2026-09-16");
    let second = night("2026-09-17");
    store
        .record_post(season.id, second, Snowflake::new(8), CREATED_AT)
        .await
        .unwrap();
    store
        .mark(season.id, first, AKI, &[hour(1), hour(2)], true, 1_000)
        .await
        .unwrap();
    store
        .mark(season.id, second, BOREALIS, &[hour(3)], false, 2_000)
        .await
        .unwrap();
    store.end_season(GUILD, 35, ENDED_AT).await.unwrap();
    assert_eq!(
        store
            .purge_season(GUILD, season.id, season.number)
            .await
            .unwrap(),
        Some(3)
    );
    assert_eq!(store.season(season.id).await.unwrap(), None);
    assert_eq!(counted(&pool, POSTS_OF_SEASON, season.id).await, 0);
    assert_eq!(counted(&pool, MARKS_OF_SEASON, season.id).await, 0);
    assert!(store.seasons_in_any_state(GUILD).await.unwrap().is_empty());
    assert_eq!(
        store
            .purge_season(GUILD, season.id, season.number)
            .await
            .unwrap(),
        None
    );
}

#[tokio::test]
async fn purge_season_leaves_another_guilds_season_alone() {
    let pool = attendance_pool().await;
    let store = Attendance::with_pool(pool.clone()).await.unwrap();
    let ours = marked_season(&store, &proposal(35, "2026-09-16", "2026-11-05")).await;
    let theirs = NewSeason {
        guild: OTHER_GUILD,
        ..proposal(35, "2026-09-16", "2026-11-05")
    };
    let theirs = marked_season(&store, &theirs).await;
    assert_eq!(
        store
            .purge_season(OTHER_GUILD, ours.id, ours.number)
            .await
            .unwrap(),
        None
    );
    assert_eq!(store.season(ours.id).await.unwrap(), Some(ours.clone()));
    assert_eq!(counted(&pool, POSTS_OF_SEASON, ours.id).await, 1);
    assert_eq!(counted(&pool, MARKS_OF_SEASON, ours.id).await, 1);
    assert_eq!(
        store
            .purge_season(GUILD, ours.id, ours.number)
            .await
            .unwrap(),
        Some(1)
    );
    assert_eq!(store.season(ours.id).await.unwrap(), None);
    assert_eq!(store.season(theirs.id).await.unwrap(), Some(theirs.clone()));
    assert_eq!(counted(&pool, POSTS_OF_SEASON, theirs.id).await, 1);
    assert_eq!(counted(&pool, MARKS_OF_SEASON, theirs.id).await, 1);
    assert_eq!(
        store.seasons_in_any_state(OTHER_GUILD).await.unwrap(),
        vec![theirs]
    );
}

#[tokio::test]
async fn seasons_in_any_state_lists_an_ended_season() {
    let store = attendance().await;
    let season_34 = created(&store, &proposal(34, "2026-06-10", "2026-08-02")).await;
    store
        .record_post(
            season_34.id,
            night("2026-06-10"),
            Snowflake::new(7),
            CREATED_AT,
        )
        .await
        .unwrap();
    let season = season_35(&store).await;
    store.end_season(GUILD, 34, ENDED_AT).await.unwrap();
    let ended = Season {
        ended_at_ms: Some(ENDED_AT),
        ..season_34
    };
    assert_eq!(store.seasons_in(GUILD).await.unwrap(), vec![season.clone()]);
    assert_eq!(
        store.seasons_in_any_state(GUILD).await.unwrap(),
        vec![ended, season]
    );
    assert!(
        store
            .seasons_in_any_state(OTHER_GUILD)
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn purge_season_ignores_a_number_that_is_not_the_one_it_read() {
    let store = attendance().await;
    let season = season_35(&store).await;
    assert_eq!(
        store
            .purge_season(GUILD, season.id, season.number + 1)
            .await
            .unwrap(),
        None
    );
    assert!(store.season(season.id).await.unwrap().is_some());
}

#[tokio::test]
async fn purge_season_reports_a_deleted_season_that_held_no_answers() {
    let store = attendance().await;
    let season = season_35(&store).await;
    assert_eq!(
        store
            .purge_season(GUILD, season.id, season.number)
            .await
            .unwrap(),
        Some(0)
    );
    assert!(store.season(season.id).await.unwrap().is_none());
}
