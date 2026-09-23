mod common;

use std::sync::Arc;

use barnacle_bot::attendance::Cell;
use barnacle_bot::attendance::ClearScope;
use barnacle_bot::attendance::Click;
use barnacle_bot::attendance::ClickOutcome;
use barnacle_bot::attendance::PostTag;
use barnacle_bot::attendance::PostsReport;
use barnacle_bot::attendance::PurgeReport;
use barnacle_bot::attendance::ROSTER_LIMIT;
use barnacle_bot::attendance::Signups;
use barnacle_bot::attendance::Target;
use barnacle_bot::attendance::TickReport;
use barnacle_bot::attendance::next_moment;
use barnacle_bot::attendance_store::Attendance;
use barnacle_bot::attendance_store::CreateOutcome;
use barnacle_bot::attendance_store::EditOutcome;
use barnacle_bot::attendance_store::EndOutcome;
use barnacle_bot::attendance_store::NewSeason;
use barnacle_bot::attendance_store::Post;
use barnacle_bot::attendance_store::PostState;
use barnacle_bot::attendance_store::Season;
use barnacle_bot::attendance_store::SeasonChange;
use barnacle_bot::failure::Kind;
use barnacle_bot::ids::ChannelId;
use barnacle_bot::ids::GuildId;
use barnacle_bot::ids::Ping;
use barnacle_bot::ids::RoleId;
use barnacle_bot::schedule::Hour;
use barnacle_bot::schedule::Night;
use barnacle_bot::schedule::Range;
use barnacle_bot::schedule::parse_day;
use barnacle_guess::Snowflake;
use barnacle_guess::UserId;
use common::attendance_pool;
use common::fakes::FakeBoard;
use sqlx::sqlite::SqlitePool;

const TODAY: &str = "2026-09-16";

const GUILD: GuildId = GuildId::new(1);
const REHEARSAL: GuildId = GuildId::new(2);
const CHANNEL: ChannelId = ChannelId::new(10);
const OTHER_CHANNEL: ChannelId = ChannelId::new(11);
const REHEARSAL_CHANNEL: ChannelId = ChannelId::new(20);
const MANAGER: UserId = UserId::new(100);
const AKI: UserId = UserId::new(200);
const BOREALIS: UserId = UserId::new(300);
const CREWMATES: RoleId = RoleId::new(4242);

const REAL_NOW: i64 = 1_789_473_600;
const FIRST_POST_AT: i64 = 1_789_515_000;
const FIRST_START: i64 = 1_789_601_400;
const FIRST_REMOVE_AT: i64 = 1_789_617_600;
const SECOND_START: i64 = 1_789_687_800;
const SECOND_REMOVE_AT: i64 = 1_789_704_000;
const THIRD_START: i64 = 1_789_774_200;
const THIRD_REMOVE_AT: i64 = 1_789_790_400;

fn millis(now_unix: i64) -> u64 {
    u64::try_from(now_unix).unwrap() * 1_000
}

fn night(text: &str) -> Night {
    Night::parse(text).unwrap()
}

fn first_night() -> Night {
    night("2026-09-16")
}

fn second_night() -> Night {
    night("2026-09-17")
}

fn third_night() -> Night {
    night("2026-09-18")
}

fn proposal() -> NewSeason {
    NewSeason {
        guild: GUILD,
        channel: CHANNEL,
        number: 35,
        codename: Some("Komodo Dragon".to_owned()),
        range: Range::new(
            parse_day("2026-09-16").unwrap(),
            parse_day("2026-11-05").unwrap(),
        )
        .unwrap(),
        created_by: MANAGER,
        created_at_ms: millis(FIRST_POST_AT),
        ping_role: None,
    }
}

async fn created(store: &Attendance, new: &NewSeason) -> Season {
    match store.create_season(new).await.unwrap() {
        CreateOutcome::Created(season) => season,
        other => panic!("expected a created season, got {other:?}"),
    }
}

async fn ready_from(
    board: FakeBoard,
    new: &NewSeason,
) -> (Arc<Signups<FakeBoard>>, Season, Attendance) {
    let store = Attendance::with_pool(attendance_pool().await)
        .await
        .unwrap();
    let season = created(&store, new).await;
    (Signups::new(board, store.clone()), season, store)
}

fn rehearsal_proposal() -> NewSeason {
    NewSeason {
        guild: REHEARSAL,
        channel: REHEARSAL_CHANNEL,
        range: Range::every_day(
            parse_day("2026-09-16").unwrap(),
            parse_day("2026-09-18").unwrap(),
        )
        .unwrap(),
        created_at_ms: millis(REAL_NOW),
        ..proposal()
    }
}

async fn two_servers() -> (Attendance, Season, Season) {
    let store = Attendance::with_pool(attendance_pool().await)
        .await
        .unwrap();
    let clan = created(&store, &proposal()).await;
    let elsewhere = created(&store, &rehearsal_proposal()).await;
    (store, clan, elsewhere)
}

async fn ready(board: FakeBoard) -> (Arc<Signups<FakeBoard>>, Season, Attendance) {
    ready_from(board, &proposal()).await
}

async fn edited(store: &Attendance, change: &SeasonChange) -> Season {
    match store
        .edit_season(GUILD, 35, change, parse_day(TODAY).unwrap())
        .await
        .unwrap()
    {
        EditOutcome::Edited { after, .. } => after,
        other => panic!("expected an edited season, got {other:?}"),
    }
}

async fn both_nights(
    board: FakeBoard,
) -> (
    Arc<Signups<FakeBoard>>,
    Season,
    Attendance,
    Snowflake,
    Snowflake,
) {
    let (signups, season, store) = ready(board).await;
    signups.tick(FIRST_POST_AT, millis(FIRST_POST_AT)).await;
    signups.tick(FIRST_START, millis(FIRST_START)).await;
    let first = message_for(&store, &season, first_night()).await;
    let second = message_for(&store, &season, second_night()).await;
    (signups, season, store, first, second)
}

async fn message_for(store: &Attendance, season: &Season, night: Night) -> Snowflake {
    store
        .post(season.id, night)
        .await
        .unwrap()
        .expect("a post was recorded")
        .message
}

async fn state_of(store: &Attendance, season: &Season, night: Night) -> PostState {
    store
        .post(season.id, night)
        .await
        .unwrap()
        .expect("a post was recorded")
        .state
}

fn tag(season: &Season, night: Night) -> PostTag {
    PostTag {
        season: season.id,
        night,
    }
}

fn press(season: &Season, message: Snowflake, user: UserId, target: Target) -> Click {
    Click {
        guild: GUILD,
        channel: CHANNEL,
        message,
        user,
        season: season.id,
        night: first_night(),
        target,
        attending: true,
    }
}

#[tokio::test]
async fn posts_the_due_night_once_across_repeated_ticks() {
    let board = FakeBoard::new();
    let (signups, season, store) = ready(board.clone()).await;
    let report = signups.tick(FIRST_POST_AT, millis(FIRST_POST_AT)).await;
    assert_eq!(report.posted, vec![tag(&season, first_night())]);
    assert_eq!(report.failures, 0);
    for step in 1..4 {
        let again = signups
            .tick(FIRST_POST_AT + step * 60, millis(FIRST_POST_AT))
            .await;
        assert!(again.posted.is_empty());
        assert_eq!(again.failures, 0);
    }
    assert_eq!(board.sends().len(), 1);
    assert_eq!(
        state_of(&store, &season, first_night()).await,
        PostState::Open
    );
}

#[tokio::test]
async fn a_rebuilt_signups_does_not_post_twice() {
    let board = FakeBoard::new();
    let (signups, season, store) = ready(board.clone()).await;
    signups.tick(FIRST_POST_AT, millis(FIRST_POST_AT)).await;
    drop(signups);
    let rebuilt = Signups::new(board.clone(), store.clone());
    let report = rebuilt
        .tick(FIRST_POST_AT + 60, millis(FIRST_POST_AT))
        .await;
    assert!(report.posted.is_empty());
    assert!(report.adopted.is_empty());
    assert_eq!(report.failures, 0);
    assert_eq!(board.sends().len(), 1);
    assert_eq!(
        message_for(&store, &season, first_night()).await,
        Snowflake::new(1_000)
    );
}

#[tokio::test]
async fn a_post_found_in_the_channel_is_adopted_not_resent() {
    let board = FakeBoard::new();
    let (signups, season, store) = ready(board.clone()).await;
    board.plant(tag(&season, first_night()), Snowflake::new(4_242));
    let report = signups.tick(FIRST_POST_AT, millis(FIRST_POST_AT)).await;
    assert_eq!(report.adopted, vec![tag(&season, first_night())]);
    assert!(report.posted.is_empty());
    assert!(board.sends().is_empty());
    assert_eq!(
        message_for(&store, &season, first_night()).await,
        Snowflake::new(4_242)
    );
}

#[tokio::test]
async fn a_failed_send_is_retried_next_tick() {
    let board = FakeBoard::new();
    let (signups, season, store) = ready(board.clone()).await;
    board.fail_sends(true);
    let failed = signups.tick(FIRST_POST_AT, millis(FIRST_POST_AT)).await;
    assert!(failed.posted.is_empty());
    assert_eq!(failed.failures, 1);
    assert_eq!(store.post(season.id, first_night()).await.unwrap(), None);
    board.fail_sends(false);
    let retried = signups
        .tick(FIRST_POST_AT + 60, millis(FIRST_POST_AT + 60))
        .await;
    assert_eq!(retried.posted, vec![tag(&season, first_night())]);
    assert_eq!(retried.failures, 0);
    assert_eq!(board.sends().len(), 2);
}

#[tokio::test]
async fn a_night_that_started_while_offline_is_never_posted() {
    let board = FakeBoard::new();
    let (signups, season, store) = ready(board.clone()).await;
    let report = signups.tick(FIRST_START, millis(FIRST_START)).await;
    assert_eq!(report.posted, vec![tag(&season, second_night())]);
    assert_eq!(store.post(season.id, first_night()).await.unwrap(), None);
    let later = signups
        .tick(FIRST_START + 3_600, millis(FIRST_START + 3_600))
        .await;
    assert!(later.posted.is_empty());
    assert_eq!(store.post(season.id, first_night()).await.unwrap(), None);
}

#[tokio::test]
async fn closes_at_the_start() {
    let board = FakeBoard::new();
    let (signups, season, store) = ready(board.clone()).await;
    signups.tick(FIRST_POST_AT, millis(FIRST_POST_AT)).await;
    let report = signups.tick(FIRST_START, millis(FIRST_START)).await;
    assert_eq!(report.closed, vec![tag(&season, first_night())]);
    assert_eq!(report.failures, 0);
    assert_eq!(
        state_of(&store, &season, first_night()).await,
        PostState::Closed
    );
    let closing = board.edits().pop().unwrap();
    assert!(!closing.open);
    assert_eq!(closing.night, first_night());
    let again = signups
        .tick(FIRST_START + 60, millis(FIRST_START + 60))
        .await;
    assert!(again.closed.is_empty());
    assert_eq!(board.edits().len(), 1);
}

#[tokio::test]
async fn marks_the_post_closed_only_after_the_edit_succeeds() {
    let board = FakeBoard::new();
    let (signups, season, store) = ready(board.clone()).await;
    signups.tick(FIRST_POST_AT, millis(FIRST_POST_AT)).await;
    board.fail_edits(true);
    let refused = signups.tick(FIRST_START, millis(FIRST_START)).await;
    assert!(refused.closed.is_empty());
    assert_eq!(refused.failures, 1);
    assert_eq!(
        state_of(&store, &season, first_night()).await,
        PostState::Open
    );
    board.fail_edits(false);
    let closed = signups
        .tick(FIRST_START + 60, millis(FIRST_START + 60))
        .await;
    assert_eq!(closed.closed, vec![tag(&season, first_night())]);
    assert_eq!(
        state_of(&store, &season, first_night()).await,
        PostState::Closed
    );
}

#[tokio::test]
async fn removes_thirty_minutes_after_the_end() {
    let board = FakeBoard::new();
    let (signups, season, store) = ready(board.clone()).await;
    signups.tick(FIRST_POST_AT, millis(FIRST_POST_AT)).await;
    signups.tick(FIRST_START, millis(FIRST_START)).await;
    let early = signups
        .tick(FIRST_REMOVE_AT - 1, millis(FIRST_REMOVE_AT - 1))
        .await;
    assert!(early.removed.is_empty());
    assert!(board.deletes().is_empty());
    let report = signups.tick(FIRST_REMOVE_AT, millis(FIRST_REMOVE_AT)).await;
    assert_eq!(report.removed, vec![tag(&season, first_night())]);
    assert_eq!(board.deletes(), vec![Snowflake::new(1_000)]);
    assert_eq!(
        state_of(&store, &season, first_night()).await,
        PostState::Removed
    );
    let again = signups
        .tick(FIRST_REMOVE_AT + 60, millis(FIRST_REMOVE_AT + 60))
        .await;
    assert!(again.removed.is_empty());
    assert_eq!(board.deletes().len(), 1);
}

#[tokio::test]
async fn a_gone_message_counts_as_removed() {
    let board = FakeBoard::new();
    let (signups, season, store) = ready(board.clone()).await;
    signups.tick(FIRST_POST_AT, millis(FIRST_POST_AT)).await;
    board.report_gone(true);
    let report = signups.tick(FIRST_REMOVE_AT, millis(FIRST_REMOVE_AT)).await;
    assert_eq!(report.removed, vec![tag(&season, first_night())]);
    assert_eq!(report.failures, 0);
    assert_eq!(
        state_of(&store, &season, first_night()).await,
        PostState::Removed
    );
}

#[tokio::test]
async fn a_long_gap_removes_and_posts_in_one_tick() {
    let board = FakeBoard::new();
    let (signups, season, store) = ready(board.clone()).await;
    signups.tick(FIRST_POST_AT, millis(FIRST_POST_AT)).await;
    let report = signups.tick(FIRST_REMOVE_AT, millis(FIRST_REMOVE_AT)).await;
    assert_eq!(report.removed, vec![tag(&season, first_night())]);
    assert_eq!(report.posted, vec![tag(&season, second_night())]);
    assert_eq!(report.failures, 0);
    assert_eq!(
        state_of(&store, &season, first_night()).await,
        PostState::Removed
    );
    assert_eq!(
        state_of(&store, &season, second_night()).await,
        PostState::Open
    );
}

#[tokio::test]
async fn a_click_after_the_start_writes_nothing() {
    let board = FakeBoard::new();
    let (signups, season, store) = ready(board.clone()).await;
    signups.tick(FIRST_POST_AT, millis(FIRST_POST_AT)).await;
    let message = message_for(&store, &season, first_night()).await;
    let outcome = signups
        .click(
            press(&season, message, AKI, Target::All),
            FIRST_START,
            millis(FIRST_START),
        )
        .await;
    assert_eq!(
        outcome,
        ClickOutcome::Closed {
            start_unix: FIRST_START
        }
    );
    assert!(
        store
            .roster(season.id, first_night())
            .await
            .unwrap()
            .is_empty()
    );
    assert!(board.edits().is_empty());
}

#[tokio::test]
async fn a_click_from_another_guild_is_refused() {
    let board = FakeBoard::new();
    let (signups, season, store) = ready(board.clone()).await;
    signups.tick(FIRST_POST_AT, millis(FIRST_POST_AT)).await;
    let message = message_for(&store, &season, first_night()).await;
    let elsewhere = Click {
        guild: GuildId::new(999),
        ..press(&season, message, AKI, Target::All)
    };
    assert_eq!(
        signups
            .click(elsewhere, FIRST_POST_AT, millis(FIRST_POST_AT))
            .await,
        ClickOutcome::UnknownSeason
    );
    let unknown = Click {
        season: season.id + 1,
        ..press(&season, message, AKI, Target::All)
    };
    assert_eq!(
        signups
            .click(unknown, FIRST_POST_AT, millis(FIRST_POST_AT))
            .await,
        ClickOutcome::UnknownSeason
    );
    let outside = Click {
        night: night("2026-11-08"),
        ..press(&season, message, AKI, Target::All)
    };
    assert_eq!(
        signups
            .click(outside, FIRST_POST_AT, millis(FIRST_POST_AT))
            .await,
        ClickOutcome::UnknownSeason
    );
    let not_a_cb_day = Click {
        night: night("2026-09-25"),
        ..press(&season, message, AKI, Target::All)
    };
    assert_eq!(
        signups
            .click(not_a_cb_day, FIRST_POST_AT, millis(FIRST_POST_AT))
            .await,
        ClickOutcome::UnknownSeason
    );
    assert!(
        store
            .roster(season.id, first_night())
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn attend_all_writes_four_marks() {
    let board = FakeBoard::new();
    let (signups, season, store) = ready(board.clone()).await;
    signups.tick(FIRST_POST_AT, millis(FIRST_POST_AT)).await;
    let message = message_for(&store, &season, first_night()).await;
    let outcome = signups
        .click(
            press(&season, message, AKI, Target::All),
            FIRST_POST_AT,
            millis(FIRST_POST_AT),
        )
        .await;
    assert_eq!(outcome, ClickOutcome::Recorded);
    let roster = store.roster(season.id, first_night()).await.unwrap();
    assert_eq!(roster.len(), 4);
    assert_eq!(
        roster.iter().map(|mark| mark.hour).collect::<Vec<Hour>>(),
        Hour::ALL.to_vec()
    );
    assert!(roster.iter().all(|mark| mark.attending));
    let drawn = board.edits().pop().unwrap();
    assert_eq!(drawn.rows.len(), 1);
    assert_eq!(drawn.rows[0].user, AKI);
    assert_eq!(drawn.rows[0].cells, [Cell::In; 4]);
    assert_eq!(drawn.hours.map(|tally| tally.attending), [1, 1, 1, 1]);
    assert_eq!(drawn.hours.map(|tally| tally.nope), [0, 0, 0, 0]);
    assert_eq!(drawn.hidden, 0);
    assert!(drawn.open);
}

#[tokio::test]
async fn twenty_clicks_during_one_slow_edit_make_at_most_three_edits() {
    let board = FakeBoard::gated();
    let (signups, season, store) = ready(board.clone()).await;
    signups.tick(FIRST_POST_AT, millis(FIRST_POST_AT)).await;
    let message = message_for(&store, &season, first_night()).await;
    let clicking: Vec<_> = (0..21)
        .map(|offset| {
            let signups = Arc::clone(&signups);
            let season = season.clone();
            tokio::spawn(async move {
                signups
                    .click(
                        press(
                            &season,
                            message,
                            UserId::new(500 + offset),
                            Target::One(Hour::new(1).unwrap()),
                        ),
                        FIRST_POST_AT,
                        millis(FIRST_POST_AT),
                    )
                    .await
            })
        })
        .collect();
    while store.roster(season.id, first_night()).await.unwrap().len() < 21 * Hour::ALL.len() {
        tokio::task::yield_now().await;
    }
    board.open_edits();
    for click in clicking {
        assert_eq!(click.await.unwrap(), ClickOutcome::Recorded);
    }
    assert!(!board.edits().is_empty());
    assert!(
        board.edits().len() <= 3,
        "{} edits for twenty-one clicks",
        board.edits().len()
    );
    assert_eq!(board.edits().pop().unwrap().rows.len(), 21);
}

#[tokio::test]
async fn the_last_edit_shows_the_last_mark() {
    let board = FakeBoard::new();
    let (signups, season, store) = ready(board.clone()).await;
    signups.tick(FIRST_POST_AT, millis(FIRST_POST_AT)).await;
    let message = message_for(&store, &season, first_night()).await;
    signups
        .click(
            press(&season, message, AKI, Target::One(Hour::new(1).unwrap())),
            FIRST_POST_AT,
            millis(FIRST_POST_AT),
        )
        .await;
    let last = Click {
        attending: false,
        ..press(
            &season,
            message,
            BOREALIS,
            Target::One(Hour::new(2).unwrap()),
        )
    };
    signups
        .click(last, FIRST_POST_AT + 1, millis(FIRST_POST_AT + 1))
        .await;
    let drawn = board.edits().pop().unwrap();
    assert_eq!(drawn.rows.len(), 2);
    assert_eq!(drawn.rows[0].user, AKI);
    assert_eq!(
        drawn.rows[0].cells,
        [Cell::In, Cell::Out, Cell::Out, Cell::Out]
    );
    assert_eq!(drawn.rows[1].user, BOREALIS);
    assert_eq!(drawn.rows[1].cells, [Cell::Out; 4]);
    assert_eq!(drawn.hours.map(|tally| tally.attending), [1, 0, 0, 0]);
    assert_eq!(drawn.hours.map(|tally| tally.nope), [1, 2, 2, 2]);
    assert_eq!(drawn.season, season.id);
    assert_eq!(drawn.number, 35);
    assert_eq!(drawn.codename.as_deref(), Some("Komodo Dragon"));
    assert_eq!(SECOND_START, second_night().start_unix());
}

#[tokio::test]
async fn a_single_hour_answer_marks_every_other_hour_nope() {
    let board = FakeBoard::new();
    let (signups, season, store) = ready(board.clone()).await;
    signups.tick(FIRST_POST_AT, millis(FIRST_POST_AT)).await;
    let message = message_for(&store, &season, first_night()).await;
    let outcome = signups
        .click(
            press(&season, message, AKI, Target::One(Hour::new(2).unwrap())),
            FIRST_POST_AT,
            millis(FIRST_POST_AT),
        )
        .await;
    assert_eq!(outcome, ClickOutcome::Recorded);
    let drawn = board.edits().pop().unwrap();
    assert_eq!(drawn.rows.len(), 1);
    assert_eq!(drawn.rows[0].user, AKI);
    assert_eq!(
        drawn.rows[0].cells,
        [Cell::Out, Cell::In, Cell::Out, Cell::Out]
    );
    assert_eq!(drawn.hours.map(|tally| tally.attending), [0, 1, 0, 0]);
    assert_eq!(drawn.hours.map(|tally| tally.nope), [1, 0, 1, 1]);
}

#[tokio::test]
async fn a_later_single_hour_answer_keeps_the_earlier_answers() {
    let board = FakeBoard::new();
    let (signups, season, store) = ready(board.clone()).await;
    signups.tick(FIRST_POST_AT, millis(FIRST_POST_AT)).await;
    let message = message_for(&store, &season, first_night()).await;
    for (hour, offset) in [(1, 0), (3, 1)] {
        let outcome = signups
            .click(
                press(&season, message, AKI, Target::One(Hour::new(hour).unwrap())),
                FIRST_POST_AT + offset,
                millis(FIRST_POST_AT + offset),
            )
            .await;
        assert_eq!(outcome, ClickOutcome::Recorded);
    }
    let drawn = board.edits().pop().unwrap();
    assert_eq!(drawn.rows.len(), 1);
    assert_eq!(
        drawn.rows[0].cells,
        [Cell::In, Cell::Out, Cell::In, Cell::Out]
    );
    assert_eq!(drawn.hours.map(|tally| tally.attending), [1, 0, 1, 0]);
    assert_eq!(drawn.hours.map(|tally| tally.nope), [0, 1, 0, 1]);
}

#[tokio::test]
async fn a_partial_answer_saved_before_the_change_shows_nope() {
    let board = FakeBoard::new();
    let pool = attendance_pool().await;
    let store = Attendance::with_pool(pool.clone()).await.unwrap();
    let season = created(&store, &proposal()).await;
    let signups = Signups::new(board, store.clone());
    signups.tick(FIRST_POST_AT, millis(FIRST_POST_AT)).await;
    sqlx::query("INSERT INTO cb_marks (season_id, night, user_id, hour, attending, answered_at_ms, changed_at_ms) VALUES (?, ?, ?, 2, 1, ?, ?)")
        .bind(season.id)
        .bind(first_night().label())
        .bind(i64::try_from(AKI.get()).unwrap())
        .bind(i64::try_from(millis(FIRST_POST_AT)).unwrap())
        .bind(i64::try_from(millis(FIRST_POST_AT)).unwrap())
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        store.roster(season.id, first_night()).await.unwrap().len(),
        1
    );
    let view = signups
        .view(&season, first_night(), FIRST_POST_AT)
        .await
        .unwrap();
    assert_eq!(view.rows.len(), 1);
    assert_eq!(view.rows[0].user, AKI);
    assert_eq!(
        view.rows[0].cells,
        [Cell::Out, Cell::In, Cell::Out, Cell::Out]
    );
    assert_eq!(view.hours.map(|tally| tally.attending), [0, 1, 0, 0]);
    assert_eq!(view.hours.map(|tally| tally.nope), [1, 0, 1, 1]);
}

#[tokio::test]
async fn removes_the_last_night_of_a_season() {
    let board = FakeBoard::new();
    let store = Attendance::with_pool(attendance_pool().await)
        .await
        .unwrap();
    let one_night = NewSeason {
        range: Range::new(
            parse_day("2026-09-16").unwrap(),
            parse_day("2026-09-16").unwrap(),
        )
        .unwrap(),
        ..proposal()
    };
    let season = match store.create_season(&one_night).await.unwrap() {
        CreateOutcome::Created(season) => season,
        other => panic!("expected a created season, got {other:?}"),
    };
    let signups = Signups::new(board.clone(), store.clone());
    signups.tick(FIRST_POST_AT, millis(FIRST_POST_AT)).await;
    signups.tick(FIRST_START, millis(FIRST_START)).await;
    let report = signups.tick(FIRST_REMOVE_AT, millis(FIRST_REMOVE_AT)).await;
    assert_eq!(report.removed, vec![tag(&season, first_night())]);
    assert_eq!(board.deletes().len(), 1);
    assert_eq!(
        state_of(&store, &season, first_night()).await,
        PostState::Removed
    );
}

#[tokio::test]
async fn the_roster_stops_at_the_limit_and_counts_the_rest() {
    let board = FakeBoard::new();
    let (signups, season, store) = ready(board.clone()).await;
    signups.tick(FIRST_POST_AT, millis(FIRST_POST_AT)).await;
    let message = message_for(&store, &season, first_night()).await;
    let crowd = u64::try_from(ROSTER_LIMIT + 3).unwrap();
    for number in 0..crowd {
        signups
            .click(
                press(&season, message, UserId::new(1_000 + number), Target::All),
                FIRST_POST_AT,
                millis(FIRST_POST_AT) + number,
            )
            .await;
    }
    let view = signups
        .view(&season, first_night(), FIRST_POST_AT)
        .await
        .unwrap();
    assert_eq!(view.rows.len(), ROSTER_LIMIT);
    assert_eq!(view.hidden, 3);
    assert_eq!(view.rows.first().unwrap().user, UserId::new(1_000));
    assert_eq!(
        view.rows.last().unwrap().user,
        UserId::new(1_000 + crowd - 4)
    );
    assert_eq!(view.hours[0].attending, u32::try_from(crowd).unwrap());
    assert_eq!(view.rows.first().unwrap().cells, [Cell::In; 4]);
}

#[tokio::test]
async fn a_moved_season_reposts_the_same_night_with_its_roster() {
    let board = FakeBoard::new();
    let (signups, season, store) = ready(board.clone()).await;
    signups.tick(FIRST_POST_AT, millis(FIRST_POST_AT)).await;
    let message = message_for(&store, &season, first_night()).await;
    signups
        .click(
            press(&season, message, AKI, Target::All),
            FIRST_POST_AT,
            millis(FIRST_POST_AT),
        )
        .await;
    let cleared = signups
        .clear_posts(&season, ClearScope::All, FIRST_POST_AT)
        .await;
    assert_eq!(
        cleared,
        PostsReport {
            touched: 1,
            failures: 0
        }
    );
    assert_eq!(board.deletes(), vec![message]);
    assert_eq!(
        state_of(&store, &season, first_night()).await,
        PostState::Removed
    );
    assert!(
        store
            .move_season(GUILD, season.id, OTHER_CHANNEL)
            .await
            .unwrap()
    );
    let report = signups
        .tick(FIRST_POST_AT + 60, millis(FIRST_POST_AT + 60))
        .await;
    assert_eq!(report.posted, vec![tag(&season, first_night())]);
    assert_eq!(report.failures, 0);
    let moved = board.sends_to(OTHER_CHANNEL);
    assert_eq!(moved.len(), 1);
    assert_eq!(moved[0].night, first_night());
    assert_eq!(moved[0].rows.len(), 1);
    assert_eq!(moved[0].rows[0].user, AKI);
    assert_eq!(moved[0].rows[0].cells, [Cell::In; 4]);
    assert_eq!(moved[0].hours.map(|tally| tally.attending), [1, 1, 1, 1]);
    assert!(moved[0].open);
    assert_eq!(board.sends_to(CHANNEL).len(), 1);
    assert_eq!(
        state_of(&store, &season, first_night()).await,
        PostState::Open
    );
    assert_ne!(message_for(&store, &season, first_night()).await, message);
    assert_eq!(
        store.roster(season.id, first_night()).await.unwrap().len(),
        4
    );
}

#[tokio::test]
async fn a_night_swept_on_schedule_is_never_reposted() {
    let board = FakeBoard::new();
    let (signups, season, store) = ready(board.clone()).await;
    signups.tick(FIRST_POST_AT, millis(FIRST_POST_AT)).await;
    signups.tick(FIRST_START, millis(FIRST_START)).await;
    let swept = signups.tick(FIRST_REMOVE_AT, millis(FIRST_REMOVE_AT)).await;
    assert_eq!(swept.removed, vec![tag(&season, first_night())]);
    for step in 1..4 {
        let again = signups
            .tick(
                FIRST_REMOVE_AT + step * 60,
                millis(FIRST_REMOVE_AT + step * 60),
            )
            .await;
        assert!(again.posted.is_empty());
        assert_eq!(again.failures, 0);
    }
    assert_eq!(
        state_of(&store, &season, first_night()).await,
        PostState::Removed
    );
    assert_eq!(
        board
            .sends()
            .iter()
            .filter(|view| view.night == first_night())
            .count(),
        1
    );
}

#[tokio::test]
async fn clearing_all_posts_deletes_every_live_message() {
    let board = FakeBoard::new();
    let (signups, season, store, first, second) = both_nights(board.clone()).await;
    assert_eq!(
        state_of(&store, &season, first_night()).await,
        PostState::Closed
    );
    assert_eq!(
        state_of(&store, &season, second_night()).await,
        PostState::Open
    );
    let report = signups
        .clear_posts(&season, ClearScope::All, FIRST_START)
        .await;
    assert_eq!(
        report,
        PostsReport {
            touched: 2,
            failures: 0
        }
    );
    assert_eq!(board.deletes(), vec![first, second]);
    assert_eq!(
        state_of(&store, &season, first_night()).await,
        PostState::Removed
    );
    assert_eq!(
        state_of(&store, &season, second_night()).await,
        PostState::Removed
    );
    let again = signups
        .clear_posts(&season, ClearScope::All, FIRST_START)
        .await;
    assert_eq!(again, PostsReport::default());
    assert_eq!(board.deletes().len(), 2);
}

#[tokio::test]
async fn clearing_outside_the_range_leaves_the_nights_that_remain() {
    let board = FakeBoard::new();
    let (signups, season, store, _first, second) = both_nights(board.clone()).await;
    let after = edited(
        &store,
        &SeasonChange {
            last_day: Some(parse_day("2026-09-16").unwrap()),
            ..SeasonChange::default()
        },
    )
    .await;
    let report = signups
        .clear_posts(&after, ClearScope::OutsideRange, FIRST_START)
        .await;
    assert_eq!(
        report,
        PostsReport {
            touched: 1,
            failures: 0
        }
    );
    assert_eq!(board.deletes(), vec![second]);
    assert_eq!(
        state_of(&store, &season, first_night()).await,
        PostState::Closed
    );
    assert_eq!(
        state_of(&store, &season, second_night()).await,
        PostState::Removed
    );
}

#[tokio::test]
async fn a_cleared_post_keeps_its_marks() {
    let board = FakeBoard::new();
    let (signups, season, store) = ready(board.clone()).await;
    signups.tick(FIRST_POST_AT, millis(FIRST_POST_AT)).await;
    let message = message_for(&store, &season, first_night()).await;
    signups
        .click(
            press(&season, message, AKI, Target::All),
            FIRST_POST_AT,
            millis(FIRST_POST_AT),
        )
        .await;
    let report = signups
        .clear_posts(&season, ClearScope::All, FIRST_POST_AT)
        .await;
    assert_eq!(
        report,
        PostsReport {
            touched: 1,
            failures: 0
        }
    );
    let roster = store.roster(season.id, first_night()).await.unwrap();
    assert_eq!(roster.len(), 4);
    assert!(roster.iter().all(|mark| mark.user == AKI && mark.attending));
    assert_eq!(
        state_of(&store, &season, first_night()).await,
        PostState::Removed
    );
}

#[tokio::test]
async fn refresh_posts_edits_and_never_sends() {
    let board = FakeBoard::new();
    let (signups, season, store) = ready(board.clone()).await;
    signups.tick(FIRST_POST_AT, millis(FIRST_POST_AT)).await;
    assert_eq!(board.sends().len(), 1);
    assert!(board.edits().is_empty());
    let after = edited(
        &store,
        &SeasonChange {
            codename: Some(Some("Blue Whale".to_owned())),
            ping_role: Some(Some(CREWMATES)),
            ..SeasonChange::default()
        },
    )
    .await;
    let report = signups.refresh_posts(&after, FIRST_POST_AT).await;
    assert_eq!(
        report,
        PostsReport {
            touched: 1,
            failures: 0
        }
    );
    let drawn = board.edits();
    assert_eq!(drawn.len(), 1);
    assert_eq!(drawn[0].codename.as_deref(), Some("Blue Whale"));
    assert_eq!(drawn[0].ping, Some(Ping::Role(CREWMATES)));
    assert_eq!(board.sends().len(), 1);
    signups
        .clear_posts(&after, ClearScope::All, FIRST_POST_AT)
        .await;
    assert_eq!(
        signups.refresh_posts(&after, FIRST_POST_AT).await,
        PostsReport::default()
    );
    assert_eq!(board.edits().len(), 1);
    assert_eq!(board.sends().len(), 1);
    assert_eq!(
        state_of(&store, &season, first_night()).await,
        PostState::Removed
    );
}

#[tokio::test]
async fn a_failed_delete_is_counted_and_retried_next_tick() {
    let board = FakeBoard::new();
    let (signups, season, store) = ready(board.clone()).await;
    signups.tick(FIRST_POST_AT, millis(FIRST_POST_AT)).await;
    let message = message_for(&store, &season, first_night()).await;
    board.fail_deletes(true);
    let refused = signups
        .clear_posts(&season, ClearScope::All, FIRST_POST_AT)
        .await;
    assert_eq!(
        refused,
        PostsReport {
            touched: 0,
            failures: 1
        }
    );
    assert_eq!(board.deletes(), vec![message]);
    assert_eq!(
        state_of(&store, &season, first_night()).await,
        PostState::Open
    );
    board.fail_deletes(false);
    let swept = signups.tick(FIRST_REMOVE_AT, millis(FIRST_REMOVE_AT)).await;
    assert_eq!(swept.removed, vec![tag(&season, first_night())]);
    assert_eq!(board.deletes(), vec![message, message]);
    assert_eq!(
        state_of(&store, &season, first_night()).await,
        PostState::Removed
    );
}

#[tokio::test]
async fn an_ended_season_is_never_posted_again() {
    let board = FakeBoard::new();
    let (signups, season, store) = ready(board.clone()).await;
    signups.tick(FIRST_POST_AT, millis(FIRST_POST_AT)).await;
    assert_eq!(
        store
            .end_season(GUILD, 35, millis(FIRST_POST_AT))
            .await
            .unwrap(),
        EndOutcome::Ended {
            season: season.clone()
        }
    );
    let cleared = signups
        .clear_posts(&season, ClearScope::All, FIRST_POST_AT)
        .await;
    assert_eq!(
        cleared,
        PostsReport {
            touched: 1,
            failures: 0
        }
    );
    assert_eq!(
        signups.tick(FIRST_START, millis(FIRST_START)).await,
        TickReport::default()
    );
    assert_eq!(board.sends().len(), 1);
    assert_eq!(store.post(season.id, second_night()).await.unwrap(), None);
}

#[tokio::test]
async fn the_view_carries_the_ping_role() {
    let board = FakeBoard::new();
    let pinging = NewSeason {
        ping_role: Some(CREWMATES),
        ..proposal()
    };
    let (signups, season, _store) = ready_from(board.clone(), &pinging).await;
    signups.tick(FIRST_POST_AT, millis(FIRST_POST_AT)).await;
    assert_eq!(
        board.sends().first().unwrap().ping,
        Some(Ping::Role(CREWMATES))
    );
    assert_eq!(
        signups
            .view(&season, first_night(), FIRST_POST_AT)
            .await
            .unwrap()
            .ping,
        Some(Ping::Role(CREWMATES))
    );
    let plain = FakeBoard::new();
    let (quiet, season, _store) = ready(plain.clone()).await;
    quiet.tick(FIRST_POST_AT, millis(FIRST_POST_AT)).await;
    assert_eq!(plain.sends().first().unwrap().ping, None);
    assert_eq!(
        quiet
            .view(&season, first_night(), FIRST_POST_AT)
            .await
            .unwrap()
            .ping,
        None
    );
}

fn pinging_proposal() -> NewSeason {
    NewSeason {
        ping_role: Some(CREWMATES),
        ..proposal()
    }
}

#[tokio::test]
async fn only_the_first_post_of_a_night_pings() {
    let board = FakeBoard::new();
    let (signups, season, store) = ready_from(board.clone(), &pinging_proposal()).await;
    signups.tick(FIRST_POST_AT, millis(FIRST_POST_AT)).await;
    assert_eq!(board.pinging_sends().len(), 1);
    let cleared = signups
        .clear_posts(&season, ClearScope::All, FIRST_POST_AT)
        .await;
    assert_eq!(cleared.failures, 0);
    assert!(
        store
            .move_season(GUILD, season.id, OTHER_CHANNEL)
            .await
            .unwrap()
    );
    let report = signups
        .tick(FIRST_POST_AT + 60, millis(FIRST_POST_AT + 60))
        .await;
    assert_eq!(report.posted, vec![tag(&season, first_night())]);
    assert_eq!(board.sends_to(OTHER_CHANNEL).len(), 1);
    assert_eq!(board.pinging_sends().len(), 1);
}

#[tokio::test]
async fn a_cleared_post_is_deleted_from_the_channel_it_lives_in() {
    let board = FakeBoard::new();
    let (signups, season, store) = ready(board.clone()).await;
    signups.tick(FIRST_POST_AT, millis(FIRST_POST_AT)).await;
    let message = message_for(&store, &season, first_night()).await;
    signups
        .clear_posts(&season, ClearScope::All, FIRST_POST_AT)
        .await;
    assert_eq!(board.deletes_in(CHANNEL), vec![message]);
    assert!(board.deletes_in(OTHER_CHANNEL).is_empty());
}

#[tokio::test]
async fn a_click_on_an_ended_season_is_refused() {
    let board = FakeBoard::new();
    let (signups, season, store) = ready(board.clone()).await;
    signups.tick(FIRST_POST_AT, millis(FIRST_POST_AT)).await;
    let message = message_for(&store, &season, first_night()).await;
    store
        .end_season(GUILD, season.number, millis(FIRST_POST_AT))
        .await
        .unwrap();
    let outcome = signups
        .click(
            press(&season, message, AKI, Target::All),
            FIRST_POST_AT,
            millis(FIRST_POST_AT),
        )
        .await;
    assert_eq!(outcome, ClickOutcome::UnknownSeason);
    assert!(
        store
            .roster(season.id, first_night())
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn a_redraw_renders_the_season_as_it_is_now() {
    let board = FakeBoard::new();
    let (signups, season, store) = ready(board.clone()).await;
    signups.tick(FIRST_POST_AT, millis(FIRST_POST_AT)).await;
    let change = SeasonChange {
        codename: Some(Some("Blue Whale".to_owned())),
        ..SeasonChange::default()
    };
    edited(&store, &change).await;
    let report = signups.refresh_posts(&season, FIRST_POST_AT).await;
    assert_eq!(report.touched, 1);
    assert_eq!(report.failures, 0);
    let edits = board.edits();
    assert_eq!(
        edits.last().unwrap().codename.as_deref(),
        Some("Blue Whale")
    );
}

#[tokio::test]
async fn a_purge_stops_at_the_first_season_whose_delete_fails() {
    let board = FakeBoard::new();
    let store = Attendance::with_pool(attendance_pool().await)
        .await
        .unwrap();
    let first = created(&store, &proposal()).await;
    let second = created(
        &store,
        &NewSeason {
            number: 36,
            channel: OTHER_CHANNEL,
            range: Range::new(
                parse_day("2026-11-11").unwrap(),
                parse_day("2026-12-20").unwrap(),
            )
            .unwrap(),
            ..proposal()
        },
    )
    .await;
    let signups = Signups::new(board.clone(), store.clone());
    signups.tick(FIRST_POST_AT, millis(FIRST_POST_AT)).await;
    let message = message_for(&store, &first, first_night()).await;
    store
        .record_post(
            second.id,
            night("2026-11-11"),
            Snowflake::new(9_999),
            millis(FIRST_POST_AT),
        )
        .await
        .unwrap();
    board.fail_deletes_in(OTHER_CHANNEL);
    let refused = signups.purge(GUILD, FIRST_POST_AT).await;
    assert_eq!(refused.seasons, 0);
    assert_eq!(refused.answers, 0);
    assert_eq!(refused.failures, 1);
    assert_eq!(refused.messages, 1);
    assert_eq!(board.deletes_in(CHANNEL), vec![message]);
    assert_eq!(
        store.seasons_in_any_state(GUILD).await.unwrap(),
        vec![first, second]
    );
}

#[tokio::test]
async fn tick_in_runs_only_the_guild_it_is_given() {
    let board = FakeBoard::new();
    let (store, clan, elsewhere) = two_servers().await;
    let signups = Signups::new(board.clone(), store.clone());
    let report = signups
        .tick_in(GUILD, FIRST_POST_AT, millis(FIRST_POST_AT))
        .await;
    assert_eq!(report.posted, vec![tag(&clan, first_night())]);
    assert_eq!(report.failures, 0);
    assert_eq!(store.post(elsewhere.id, first_night()).await.unwrap(), None);
    assert_eq!(board.sends_to(CHANNEL).len(), 1);
    assert!(board.sends_to(REHEARSAL_CHANNEL).is_empty());
    let scoped = signups
        .tick_in(REHEARSAL, FIRST_POST_AT, millis(FIRST_POST_AT))
        .await;
    assert_eq!(scoped.posted, vec![tag(&elsewhere, first_night())]);
    assert_eq!(scoped.failures, 0);
    assert_eq!(board.sends_to(REHEARSAL_CHANNEL).len(), 1);
    assert_eq!(board.sends_to(CHANNEL).len(), 1);
}

#[tokio::test]
async fn a_purge_deletes_every_message_row_and_answer() {
    let board = FakeBoard::new();
    let (signups, season, store, first, second) = both_nights(board.clone()).await;
    let attending = Click {
        night: second_night(),
        ..press(&season, second, AKI, Target::All)
    };
    assert_eq!(
        signups
            .click(attending, FIRST_START, millis(FIRST_START))
            .await,
        ClickOutcome::Recorded
    );
    let nope = Click {
        night: second_night(),
        attending: false,
        ..press(
            &season,
            second,
            BOREALIS,
            Target::One(Hour::new(1).unwrap()),
        )
    };
    assert_eq!(
        signups.click(nope, FIRST_START, millis(FIRST_START)).await,
        ClickOutcome::Recorded
    );
    let report = signups.purge(GUILD, FIRST_START).await;
    assert_eq!(
        report,
        PurgeReport {
            seasons: 1,
            messages: 2,
            answers: 8,
            failures: 0
        }
    );
    assert_eq!(board.deletes(), vec![first, second]);
    assert_eq!(store.season(season.id).await.unwrap(), None);
    assert!(store.posts(season.id).await.unwrap().is_empty());
    assert!(
        store
            .roster(season.id, second_night())
            .await
            .unwrap()
            .is_empty()
    );
    assert!(store.seasons_in_any_state(GUILD).await.unwrap().is_empty());
    assert_eq!(
        signups.purge(GUILD, FIRST_START).await,
        PurgeReport::default()
    );
    assert_eq!(board.deletes().len(), 2);
}

#[tokio::test]
async fn a_purge_whose_delete_fails_deletes_no_rows() {
    let board = FakeBoard::new();
    let (signups, season, store) = ready(board.clone()).await;
    signups.tick(FIRST_POST_AT, millis(FIRST_POST_AT)).await;
    let message = message_for(&store, &season, first_night()).await;
    signups
        .click(
            press(&season, message, AKI, Target::All),
            FIRST_POST_AT,
            millis(FIRST_POST_AT),
        )
        .await;
    board.fail_deletes(true);
    let refused = signups.purge(GUILD, FIRST_POST_AT).await;
    assert_eq!(refused.failures, 1);
    assert_eq!(refused.seasons, 0);
    assert_eq!(refused.answers, 0);
    assert_eq!(refused.messages, 0);
    assert_eq!(board.deletes_in(CHANNEL), vec![message]);
    assert_eq!(store.season(season.id).await.unwrap(), Some(season.clone()));
    assert_eq!(
        store.seasons_in_any_state(GUILD).await.unwrap(),
        vec![season.clone()]
    );
    assert_eq!(message_for(&store, &season, first_night()).await, message);
    assert_eq!(
        state_of(&store, &season, first_night()).await,
        PostState::Open
    );
    assert_eq!(
        store.roster(season.id, first_night()).await.unwrap().len(),
        4
    );
    board.fail_deletes(false);
    let cleared = signups.purge(GUILD, FIRST_POST_AT).await;
    assert_eq!(
        cleared,
        PurgeReport {
            seasons: 1,
            messages: 1,
            answers: 4,
            failures: 0
        }
    );
    assert_eq!(store.season(season.id).await.unwrap(), None);
    assert!(
        store
            .roster(season.id, first_night())
            .await
            .unwrap()
            .is_empty()
    );
}

async fn posts_of(store: &Attendance, seasons: &[Season]) -> Vec<(i64, Vec<Post>)> {
    let mut posts = Vec::new();
    for season in seasons {
        posts.push((season.id, store.posts(season.id).await.unwrap()));
    }
    posts
}

async fn offered(store: &Attendance, guild: GuildId, now_unix: i64) -> Option<i64> {
    let seasons = store.seasons_in(guild).await.unwrap();
    let posts = posts_of(store, &seasons).await;
    next_moment(&seasons, &posts, now_unix)
}

async fn step(signups: &Arc<Signups<FakeBoard>>, guild: GuildId) -> Option<(i64, TickReport)> {
    let store = signups.store();
    let offset = store.rehearsal_clock(guild).await.unwrap();
    let moment = offered(store, guild, REAL_NOW + offset).await?;
    store
        .set_rehearsal_clock(guild, moment - REAL_NOW)
        .await
        .unwrap();
    let report = signups.tick_in(guild, moment, millis(moment)).await;
    Some((moment, report))
}

fn acted(report: &TickReport) -> bool {
    !(report.posted.is_empty()
        && report.adopted.is_empty()
        && report.closed.is_empty()
        && report.removed.is_empty())
}

fn post_in(night: Night, state: PostState) -> Post {
    Post {
        night,
        message: Snowflake::new(9_000),
        state,
        posted_at_ms: millis(FIRST_POST_AT),
    }
}

async fn clock_rows(pool: &SqlitePool) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM cb_rehearsal_clock")
        .fetch_one(pool)
        .await
        .unwrap()
}

#[tokio::test]
async fn the_beat_uses_a_rehearsal_servers_own_clock() {
    let board = FakeBoard::new();
    let (store, clan, elsewhere) = two_servers().await;
    let signups = Signups::rehearsing(board.clone(), store.clone(), vec![REHEARSAL]);
    assert_eq!(
        signups.tick(REAL_NOW, millis(REAL_NOW)).await,
        TickReport::default()
    );
    store
        .set_rehearsal_clock(REHEARSAL, FIRST_POST_AT - REAL_NOW)
        .await
        .unwrap();
    let report = signups.tick(REAL_NOW, millis(REAL_NOW)).await;
    assert_eq!(report.posted, vec![tag(&elsewhere, first_night())]);
    assert_eq!(report.failures, 0);
    assert_eq!(board.sends_to(REHEARSAL_CHANNEL).len(), 1);
    assert!(board.sends_to(CHANNEL).is_empty());
    assert_eq!(store.post(clan.id, first_night()).await.unwrap(), None);
    assert_eq!(
        store
            .post(elsewhere.id, first_night())
            .await
            .unwrap()
            .unwrap()
            .posted_at_ms,
        millis(FIRST_POST_AT)
    );
    store
        .set_rehearsal_clock(REHEARSAL, FIRST_START - REAL_NOW)
        .await
        .unwrap();
    let stepped = signups.tick(REAL_NOW, millis(REAL_NOW)).await;
    assert_eq!(stepped.closed, vec![tag(&elsewhere, first_night())]);
    assert_eq!(stepped.posted, vec![tag(&elsewhere, second_night())]);
    assert_eq!(stepped.failures, 0);
    assert!(board.sends_to(CHANNEL).is_empty());
    assert_eq!(store.post(clan.id, first_night()).await.unwrap(), None);
    assert_eq!(
        state_of(&store, &elsewhere, first_night()).await,
        PostState::Closed
    );
}

#[tokio::test]
async fn an_ordinary_server_is_beaten_at_the_real_instant() {
    let board = FakeBoard::new();
    let (store, clan, elsewhere) = two_servers().await;
    let signups = Signups::rehearsing(board.clone(), store.clone(), vec![REHEARSAL]);
    store
        .set_rehearsal_clock(REHEARSAL, FIRST_START - FIRST_POST_AT)
        .await
        .unwrap();
    let report = signups.tick(FIRST_POST_AT, millis(FIRST_POST_AT)).await;
    assert_eq!(report.posted.len(), 2);
    assert!(report.posted.contains(&tag(&clan, first_night())));
    assert!(report.posted.contains(&tag(&elsewhere, second_night())));
    assert_eq!(report.failures, 0);
    assert_eq!(
        store
            .post(clan.id, first_night())
            .await
            .unwrap()
            .unwrap()
            .posted_at_ms,
        millis(FIRST_POST_AT)
    );
    assert_eq!(store.post(elsewhere.id, first_night()).await.unwrap(), None);
    assert_eq!(
        store
            .post(elsewhere.id, second_night())
            .await
            .unwrap()
            .unwrap()
            .posted_at_ms,
        millis(FIRST_START)
    );
    let plain = FakeBoard::new();
    let (quiet_store, quiet_clan, quiet_elsewhere) = two_servers().await;
    let quiet = Signups::rehearsing(plain.clone(), quiet_store.clone(), vec![REHEARSAL]);
    let both = quiet.tick(FIRST_POST_AT, millis(FIRST_POST_AT)).await;
    assert_eq!(both.posted.len(), 2);
    assert!(both.posted.contains(&tag(&quiet_clan, first_night())));
    assert!(both.posted.contains(&tag(&quiet_elsewhere, first_night())));
    assert_eq!(both.failures, 0);
    for season in [&quiet_clan, &quiet_elsewhere] {
        assert_eq!(
            quiet_store
                .post(season.id, first_night())
                .await
                .unwrap()
                .unwrap()
                .posted_at_ms,
            millis(FIRST_POST_AT)
        );
    }
}

#[tokio::test]
async fn next_moment_never_returns_an_instant_that_has_passed() {
    let (_, clan, elsewhere) = two_servers().await;
    let seasons = vec![clan.clone(), elsewhere.clone()];
    assert_eq!(next_moment(&seasons, &[], REAL_NOW), Some(FIRST_POST_AT));
    assert_eq!(
        next_moment(&seasons, &[], FIRST_POST_AT - 1),
        Some(FIRST_POST_AT)
    );
    assert_eq!(next_moment(&seasons, &[], FIRST_POST_AT), Some(FIRST_START));
    assert_eq!(next_moment(&seasons, &[], FIRST_START), Some(SECOND_START));
    let clan_only = vec![clan.clone()];
    let open = vec![(clan.id, vec![post_in(first_night(), PostState::Open)])];
    assert_eq!(
        next_moment(&clan_only, &open, FIRST_START - 1),
        Some(FIRST_START)
    );
    assert_eq!(
        next_moment(&clan_only, &open, FIRST_START),
        Some(FIRST_REMOVE_AT)
    );
    let closed = vec![(clan.id, vec![post_in(first_night(), PostState::Closed)])];
    assert_eq!(
        next_moment(&clan_only, &closed, FIRST_REMOVE_AT - 1),
        Some(FIRST_REMOVE_AT)
    );
    assert_eq!(
        next_moment(&clan_only, &closed, FIRST_REMOVE_AT),
        Some(THIRD_START)
    );
    let removed = vec![(clan.id, vec![post_in(first_night(), PostState::Removed)])];
    assert_eq!(
        next_moment(&clan_only, &removed, FIRST_REMOVE_AT),
        Some(THIRD_START)
    );
    let spent = vec![(
        elsewhere.id,
        vec![
            post_in(first_night(), PostState::Removed),
            post_in(second_night(), PostState::Removed),
            post_in(third_night(), PostState::Removed),
        ],
    )];
    let rehearsal_only = vec![elsewhere.clone()];
    assert_eq!(
        next_moment(&rehearsal_only, &spent, THIRD_REMOVE_AT - 1),
        None
    );
    assert_eq!(next_moment(&rehearsal_only, &spent, THIRD_REMOVE_AT), None);
    for now in [
        REAL_NOW,
        FIRST_POST_AT,
        FIRST_START,
        FIRST_REMOVE_AT,
        SECOND_START,
        SECOND_REMOVE_AT,
        THIRD_START,
        THIRD_REMOVE_AT,
    ] {
        for posts in [&open, &closed, &removed] {
            if let Some(moment) = next_moment(&seasons, posts, now) {
                assert!(moment > now, "{moment} is not after {now}");
            }
        }
    }
    let board = FakeBoard::new();
    let thursday_store = Attendance::with_pool(attendance_pool().await)
        .await
        .unwrap();
    let thursday = created(
        &thursday_store,
        &NewSeason {
            range: Range::new(
                parse_day("2026-09-17").unwrap(),
                parse_day("2026-09-17").unwrap(),
            )
            .unwrap(),
            ..proposal()
        },
    )
    .await;
    thursday_store
        .record_post(
            thursday.id,
            second_night(),
            Snowflake::new(9_000),
            millis(REAL_NOW),
        )
        .await
        .unwrap();
    thursday_store
        .set_post_state(thursday.id, second_night(), PostState::Removed)
        .await
        .unwrap();
    assert_eq!(
        offered(&thursday_store, GUILD, FIRST_POST_AT).await,
        Some(FIRST_START)
    );
    let signups = Signups::new(board.clone(), thursday_store.clone());
    let again = signups
        .tick_in(GUILD, FIRST_START, millis(FIRST_START))
        .await;
    assert_eq!(again.posted, vec![tag(&thursday, second_night())]);
    assert_eq!(
        state_of(&thursday_store, &thursday, second_night()).await,
        PostState::Open
    );
}

#[tokio::test]
async fn stepping_walks_post_then_close_and_successor_then_removal() {
    let board = FakeBoard::new();
    let store = Attendance::with_pool(attendance_pool().await)
        .await
        .unwrap();
    let season = created(&store, &rehearsal_proposal()).await;
    let signups = Signups::rehearsing(board.clone(), store.clone(), vec![REHEARSAL]);
    let (moment, report) = step(&signups, REHEARSAL).await.unwrap();
    assert_eq!(moment, FIRST_POST_AT);
    assert_eq!(
        report,
        TickReport {
            posted: vec![tag(&season, first_night())],
            ..TickReport::default()
        }
    );
    assert_eq!(
        store.rehearsal_clock(REHEARSAL).await.unwrap(),
        FIRST_POST_AT - REAL_NOW
    );
    let first = message_for(&store, &season, first_night()).await;
    let (moment, report) = step(&signups, REHEARSAL).await.unwrap();
    assert_eq!(moment, FIRST_START);
    assert_eq!(
        report,
        TickReport {
            posted: vec![tag(&season, second_night())],
            closed: vec![tag(&season, first_night())],
            ..TickReport::default()
        }
    );
    assert_eq!(
        state_of(&store, &season, first_night()).await,
        PostState::Closed
    );
    assert_eq!(
        state_of(&store, &season, second_night()).await,
        PostState::Open
    );
    let (moment, report) = step(&signups, REHEARSAL).await.unwrap();
    assert_eq!(moment, FIRST_REMOVE_AT);
    assert_eq!(
        report,
        TickReport {
            removed: vec![tag(&season, first_night())],
            ..TickReport::default()
        }
    );
    assert_eq!(
        state_of(&store, &season, first_night()).await,
        PostState::Removed
    );
    assert_eq!(board.deletes_in(REHEARSAL_CHANNEL), vec![first]);
    assert_eq!(board.sends_to(REHEARSAL_CHANNEL).len(), 2);
    assert_eq!(
        store.rehearsal_clock(REHEARSAL).await.unwrap(),
        FIRST_REMOVE_AT - REAL_NOW
    );
    assert_eq!(
        signups.tick(REAL_NOW, millis(REAL_NOW)).await,
        TickReport::default()
    );
}

#[tokio::test]
async fn every_step_the_beat_takes_was_offered_by_next_moment() {
    let board = FakeBoard::new();
    let store = Attendance::with_pool(attendance_pool().await)
        .await
        .unwrap();
    let season = created(&store, &rehearsal_proposal()).await;
    let signups = Signups::rehearsing(board.clone(), store.clone(), vec![REHEARSAL]);
    let mut now = REAL_NOW;
    let mut walked = Vec::new();
    while let Some(moment) = offered(&store, REHEARSAL, now).await {
        assert!(moment > now, "{moment} is not after {now}");
        let early = signups
            .tick_in(REHEARSAL, moment - 1, millis(moment - 1))
            .await;
        assert_eq!(
            early,
            TickReport::default(),
            "the beat acted at {}, which next_moment did not offer",
            moment - 1
        );
        let report = signups.tick_in(REHEARSAL, moment, millis(moment)).await;
        assert!(
            acted(&report),
            "next_moment offered {moment} but the beat did nothing there"
        );
        assert_eq!(report.failures, 0);
        walked.push(moment);
        now = moment;
    }
    assert_eq!(
        walked,
        vec![
            FIRST_POST_AT,
            FIRST_START,
            FIRST_REMOVE_AT,
            SECOND_START,
            SECOND_REMOVE_AT,
            THIRD_START,
            THIRD_REMOVE_AT,
        ]
    );
    for night in season.range.nights() {
        assert_eq!(state_of(&store, &season, night).await, PostState::Removed);
    }
    assert_eq!(board.sends_to(REHEARSAL_CHANNEL).len(), 3);
    assert_eq!(board.deletes_in(REHEARSAL_CHANNEL).len(), 3);
    let far = THIRD_REMOVE_AT + 30 * 86_400;
    assert_eq!(
        signups.tick_in(REHEARSAL, far, millis(far)).await,
        TickReport::default()
    );
}

#[tokio::test]
async fn a_purge_clears_the_rehearsal_clock() {
    let board = FakeBoard::new();
    let pool = attendance_pool().await;
    let store = Attendance::with_pool(pool.clone()).await.unwrap();
    let season = created(&store, &rehearsal_proposal()).await;
    let signups = Signups::rehearsing(board.clone(), store.clone(), vec![REHEARSAL]);
    let (moment, _) = step(&signups, REHEARSAL).await.unwrap();
    store.set_rehearsal_clock(GUILD, 5).await.unwrap();
    assert_eq!(
        store.rehearsal_clock(REHEARSAL).await.unwrap(),
        moment - REAL_NOW
    );
    let message = message_for(&store, &season, first_night()).await;
    board.fail_deletes(true);
    let refused = signups.purge(REHEARSAL, REAL_NOW).await;
    assert_eq!(refused.failures, 1);
    assert_eq!(refused.seasons, 0);
    assert_eq!(
        store.rehearsal_clock(REHEARSAL).await.unwrap(),
        moment - REAL_NOW
    );
    assert_eq!(clock_rows(&pool).await, 2);
    board.fail_deletes(false);
    let cleared = signups.purge(REHEARSAL, REAL_NOW).await;
    assert_eq!(
        cleared,
        PurgeReport {
            seasons: 1,
            messages: 1,
            answers: 0,
            failures: 0
        }
    );
    assert_eq!(board.deletes_in(REHEARSAL_CHANNEL), vec![message, message]);
    assert!(
        store
            .seasons_in_any_state(REHEARSAL)
            .await
            .unwrap()
            .is_empty()
    );
    assert_eq!(store.rehearsal_clock(REHEARSAL).await.unwrap(), 0);
    assert_eq!(store.rehearsal_clock(GUILD).await.unwrap(), 5);
    assert_eq!(clock_rows(&pool).await, 1);
    assert_eq!(signups.purge(GUILD, REAL_NOW).await, PurgeReport::default());
    assert_eq!(clock_rows(&pool).await, 0);
}

#[tokio::test]
async fn a_click_uses_the_rehearsal_clock() {
    let board = FakeBoard::new();
    let (store, _clan, elsewhere) = two_servers().await;
    let signups = Signups::rehearsing(board.clone(), store.clone(), vec![REHEARSAL]);
    store
        .set_rehearsal_clock(REHEARSAL, FIRST_POST_AT - REAL_NOW)
        .await
        .unwrap();
    signups.tick(REAL_NOW, millis(REAL_NOW)).await;
    let message = message_for(&store, &elsewhere, first_night()).await;
    let press = Click {
        guild: REHEARSAL,
        channel: REHEARSAL_CHANNEL,
        message,
        user: AKI,
        season: elsewhere.id,
        night: first_night(),
        target: Target::All,
        attending: true,
    };
    assert_eq!(
        signups.click(press, REAL_NOW, millis(REAL_NOW)).await,
        ClickOutcome::Recorded
    );
    store
        .set_rehearsal_clock(REHEARSAL, FIRST_START - REAL_NOW)
        .await
        .unwrap();
    assert_eq!(
        signups.click(press, REAL_NOW, millis(REAL_NOW)).await,
        ClickOutcome::Closed {
            start_unix: FIRST_START
        }
    );
}

fn only(events: Vec<serde_json::Value>) -> serde_json::Value {
    assert_eq!(events.len(), 1, "{events:?}");
    events.into_iter().next().unwrap()
}

#[tokio::test]
async fn a_click_that_cannot_be_saved_returns_its_failure() {
    let logs = common::logs::capture();
    let pool = attendance_pool().await;
    let store = Attendance::with_pool(pool.clone()).await.unwrap();
    let season = created(&store, &proposal()).await;
    let signups = Signups::new(FakeBoard::new(), store.clone());
    signups.tick(FIRST_POST_AT, millis(FIRST_POST_AT)).await;
    let message = message_for(&store, &season, first_night()).await;
    sqlx::query("DROP TABLE cb_marks")
        .execute(&pool)
        .await
        .unwrap();
    let outcome = signups
        .click(
            press(&season, message, AKI, Target::All),
            FIRST_POST_AT,
            millis(FIRST_POST_AT),
        )
        .await;
    let failure = match outcome {
        ClickOutcome::Failed(failure) => failure,
        other => panic!("expected a failed click, got {other:?}"),
    };
    assert_eq!(failure.kind, Kind::Database);
    assert!(
        logs.events()
            .iter()
            .all(|event| event["barnacle.reference"] != failure.reference.as_str())
    );
}

#[tokio::test]
async fn a_post_barnacle_cannot_send_is_logged_with_its_reason() {
    let logs = common::logs::capture();
    let board = FakeBoard::new();
    board.fail_sends(true);
    let (signups, season, _) = ready(board.clone()).await;
    let report = signups.tick(FIRST_POST_AT, millis(FIRST_POST_AT)).await;
    assert_eq!(report.failures, 1);
    let event = only(logs.named("signup.post.failed"));
    assert_eq!(event["event.outcome"], "failure");
    assert_eq!(event["message"], "a sign-up post could not be published");
    assert!(
        event["exception.message"]
            .as_str()
            .unwrap()
            .contains("the fake board refuses every send")
    );
    assert_eq!(event["error.type"], "internal");
    assert_eq!(event["barnacle.reference"].as_str().unwrap().len(), 8);
    assert_eq!(event["barnacle.season.id"], season.id.to_string());
    assert_eq!(event["discord.channel.id"], CHANNEL.get().to_string());
    assert_eq!(event["discord.guild.id"], GUILD.get().to_string());
    assert_eq!(event["barnacle.night"], "2026-09-16");
    assert!(logs.named("signup.post.published").is_empty());
}

#[tokio::test]
async fn each_sign_up_step_is_recorded_with_its_post() {
    let logs = common::logs::capture();
    let board = FakeBoard::new();
    let (signups, season, store) = ready(board.clone()).await;
    signups.tick(FIRST_POST_AT, millis(FIRST_POST_AT)).await;
    let message = message_for(&store, &season, first_night()).await;
    signups.tick(FIRST_START, millis(FIRST_START)).await;
    signups.tick(FIRST_REMOVE_AT, millis(FIRST_REMOVE_AT)).await;
    for name in [
        "signup.post.published",
        "signup.post.closed",
        "signup.post.removed",
    ] {
        let first = logs
            .named(name)
            .into_iter()
            .find(|event| event["barnacle.night"] == "2026-09-16")
            .unwrap_or_else(|| panic!("no {name} event for the first night"));
        assert_eq!(first["level"], "INFO");
        assert_eq!(first["event.outcome"], "success");
        assert_eq!(first["discord.message.id"], message.get().to_string());
        assert_eq!(first["barnacle.season.id"], season.id.to_string());
        assert_eq!(first["discord.channel.id"], CHANNEL.get().to_string());
    }
    assert!(logs.named("signup.post.failed").is_empty());
}

#[tokio::test]
async fn a_post_that_cannot_be_redrawn_is_logged_with_its_message() {
    let logs = common::logs::capture();
    let board = FakeBoard::new();
    let (signups, season, store) = ready(board.clone()).await;
    signups.tick(FIRST_POST_AT, millis(FIRST_POST_AT)).await;
    let message = message_for(&store, &season, first_night()).await;
    board.fail_edits(true);
    let report = signups.tick(FIRST_START, millis(FIRST_START)).await;
    assert_eq!(report.failures, 1);
    let event = only(logs.named("signup.post.failed"));
    assert_eq!(event["message"], "a sign-up post could not be redrawn");
    assert!(
        event["exception.message"]
            .as_str()
            .unwrap()
            .contains("the fake board refuses every edit")
    );
    assert_eq!(event["discord.message.id"], message.get().to_string());
    assert_eq!(event["barnacle.night"], "2026-09-16");
    assert!(logs.named("signup.post.closed").is_empty());
}

#[tokio::test]
async fn a_post_that_cannot_be_removed_is_logged_once() {
    let logs = common::logs::capture();
    let board = FakeBoard::new();
    let (signups, season, store) = ready(board.clone()).await;
    signups.tick(FIRST_POST_AT, millis(FIRST_POST_AT)).await;
    let message = message_for(&store, &season, first_night()).await;
    board.fail_deletes(true);
    let report = signups
        .clear_posts(&season, ClearScope::All, FIRST_POST_AT)
        .await;
    assert_eq!(report.failures, 1);
    let event = only(logs.named("signup.post.failed"));
    assert_eq!(event["message"], "a sign-up post could not be removed");
    assert_eq!(event["discord.message.id"], message.get().to_string());
    assert_eq!(event["barnacle.season.id"], season.id.to_string());
}

#[tokio::test]
async fn an_everyone_ping_is_written_as_everyone() {
    let board = FakeBoard::new();
    let everyone = NewSeason {
        ping_role: Some(RoleId::new(GUILD.get())),
        ..proposal()
    };
    let (signups, _season, _store) = ready_from(board.clone(), &everyone).await;
    signups.tick(FIRST_POST_AT, millis(FIRST_POST_AT)).await;
    let sent = board.sends();
    let view = sent.first().expect("the first night's post was sent");
    assert_eq!(
        barnacle_bot::wiring::ping_content(view.ping),
        "@everyone",
        "a season pinging its server's own @everyone role must write the literal @everyone, never <@&server id>"
    );
}

#[tokio::test]
async fn a_ping_discord_did_not_register_logs_a_warning() {
    let logs = common::logs::capture();
    let board = FakeBoard::new();
    board.unheard_pings(true);
    let (signups, season, store) = ready_from(board.clone(), &pinging_proposal()).await;
    let report = signups.tick(FIRST_POST_AT, millis(FIRST_POST_AT)).await;
    assert_eq!(report.posted, vec![tag(&season, first_night())]);
    let message = message_for(&store, &season, first_night()).await;
    let silent = only(logs.named("signup.ping.silent"));
    assert_eq!(silent["level"], "WARN");
    assert_eq!(silent["event.outcome"], "failure");
    assert_eq!(silent["barnacle.season.id"], season.id.to_string());
    assert_eq!(silent["barnacle.night"], "2026-09-16");
    assert_eq!(silent["discord.message.id"], message.get().to_string());
    assert_eq!(silent["barnacle.ping"], CREWMATES.get().to_string());
    let published = only(logs.named("signup.post.published"));
    assert_eq!(published["barnacle.ping.heard"], false);
    assert_eq!(published["discord.message.id"], message.get().to_string());
}

#[tokio::test]
async fn a_published_post_names_its_ping_and_that_it_was_heard() {
    let logs = common::logs::capture();
    let board = FakeBoard::new();
    let (signups, _season, _store) = ready_from(board.clone(), &pinging_proposal()).await;
    signups.tick(FIRST_POST_AT, millis(FIRST_POST_AT)).await;
    let published = only(logs.named("signup.post.published"));
    assert_eq!(published["barnacle.ping"], CREWMATES.get().to_string());
    assert_eq!(published["barnacle.ping.heard"], true);
    assert!(logs.named("signup.ping.silent").is_empty());
}

#[tokio::test]
async fn a_re_sent_post_never_logs_a_silent_ping() {
    let logs = common::logs::capture();
    let board = FakeBoard::new();
    board.unheard_pings(true);
    let (signups, season, _store) = ready_from(board.clone(), &pinging_proposal()).await;
    signups.tick(FIRST_POST_AT, millis(FIRST_POST_AT)).await;
    assert_eq!(logs.named("signup.ping.silent").len(), 1);
    let cleared = signups
        .clear_posts(&season, ClearScope::All, FIRST_POST_AT)
        .await;
    assert_eq!(cleared.failures, 0);
    let logs = common::logs::capture();
    let report = signups
        .tick(FIRST_POST_AT + 60, millis(FIRST_POST_AT + 60))
        .await;
    assert_eq!(report.posted, vec![tag(&season, first_night())]);
    assert!(logs.named("signup.ping.silent").is_empty());
    let published = only(logs.named("signup.post.published"));
    assert!(published.get("barnacle.ping").is_none());
    assert!(published.get("barnacle.ping.heard").is_none());
}

#[tokio::test]
async fn a_recorded_click_logs_who_chose_what() {
    let board = FakeBoard::new();
    let (signups, season, store) = ready(board.clone()).await;
    signups.tick(FIRST_POST_AT, millis(FIRST_POST_AT)).await;
    let message = message_for(&store, &season, first_night()).await;
    let logs = common::logs::capture();
    let outcome = signups
        .click(
            Click {
                attending: false,
                ..press(&season, message, AKI, Target::One(Hour::new(2).unwrap()))
            },
            FIRST_POST_AT,
            millis(FIRST_POST_AT),
        )
        .await;
    assert_eq!(outcome, ClickOutcome::Recorded);
    let event = only(logs.named("signup.click.recorded"));
    assert_eq!(event["level"], "INFO");
    assert_eq!(event["event.outcome"], "success");
    assert_eq!(event["discord.user.id"], AKI.get().to_string());
    assert_eq!(event["discord.guild.id"], GUILD.get().to_string());
    assert_eq!(event["discord.channel.id"], CHANNEL.get().to_string());
    assert_eq!(event["barnacle.season.id"], season.id.to_string());
    assert_eq!(event["barnacle.night"], "2026-09-16");
    assert_eq!(event["discord.message.id"], message.get().to_string());
    assert_eq!(event["barnacle.signup.target"], "2");
    assert_eq!(event["barnacle.signup.choice"], "nope");
}
