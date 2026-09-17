mod common;

use std::sync::Arc;

use barnacle_bot::attendance::Cell;
use barnacle_bot::attendance::Click;
use barnacle_bot::attendance::ClickOutcome;
use barnacle_bot::attendance::PostTag;
use barnacle_bot::attendance::Signups;
use barnacle_bot::attendance::Target;
use barnacle_bot::attendance_store::Attendance;
use barnacle_bot::attendance_store::CreateOutcome;
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
use common::attendance_pool;
use common::fakes::FakeBoard;

const GUILD: GuildId = GuildId::new(1);
const CHANNEL: ChannelId = ChannelId::new(10);
const MANAGER: UserId = UserId::new(100);
const AKI: UserId = UserId::new(200);
const BOREALIS: UserId = UserId::new(300);

const FIRST_POST_AT: i64 = 1_789_515_000;
const FIRST_START: i64 = 1_789_601_400;
const FIRST_REMOVE_AT: i64 = 1_789_617_600;
const SECOND_START: i64 = 1_789_687_800;

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
    }
}

async fn ready(board: FakeBoard) -> (Arc<Signups<FakeBoard>>, Season, Attendance) {
    let store = Attendance::with_pool(attendance_pool().await)
        .await
        .unwrap();
    let season = match store.create_season(&proposal()).await.unwrap() {
        CreateOutcome::Created(season) => season,
        other => panic!("expected a created season, got {other:?}"),
    };
    (Signups::new(board, store.clone()), season, store)
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
    while store.roster(season.id, first_night()).await.unwrap().len() < 21 {
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
        [Cell::In, Cell::None, Cell::None, Cell::None]
    );
    assert_eq!(drawn.rows[1].user, BOREALIS);
    assert_eq!(
        drawn.rows[1].cells,
        [Cell::None, Cell::Out, Cell::None, Cell::None]
    );
    assert_eq!(drawn.hours.map(|tally| tally.attending), [1, 0, 0, 0]);
    assert_eq!(drawn.hours.map(|tally| tally.nope), [0, 1, 0, 0]);
    assert_eq!(drawn.season, season.id);
    assert_eq!(drawn.number, 35);
    assert_eq!(drawn.codename.as_deref(), Some("Komodo Dragon"));
    assert_eq!(SECOND_START, second_night().start_unix());
}
