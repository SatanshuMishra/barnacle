mod common;

use std::sync::Arc;
use std::time::Duration;

use barnacle_bot::attendance::Board;
use barnacle_bot::attendance::BoardError;
use barnacle_bot::attendance::Cell;
use barnacle_bot::attendance::ChooseOutcome;
use barnacle_bot::attendance::ClearScope;
use barnacle_bot::attendance::Click;
use barnacle_bot::attendance::ClickOutcome;
use barnacle_bot::attendance::Delivery;
use barnacle_bot::attendance::EditReport;
use barnacle_bot::attendance::HoursPick;
use barnacle_bot::attendance::MenuOutcome;
use barnacle_bot::attendance::PostTag;
use barnacle_bot::attendance::PostsReport;
use barnacle_bot::attendance::PurgeReport;
use barnacle_bot::attendance::ROSTER_LIMIT;
use barnacle_bot::attendance::RehearsalOutcome;
use barnacle_bot::attendance::Removal;
use barnacle_bot::attendance::RepostOutcome;
use barnacle_bot::attendance::RepostPing;
use barnacle_bot::attendance::Sent;
use barnacle_bot::attendance::SignupView;
use barnacle_bot::attendance::Signups;
use barnacle_bot::attendance::Target;
use barnacle_bot::attendance::TickReport;
use barnacle_bot::attendance::listed_events;
use barnacle_bot::attendance::next_moment;
use barnacle_bot::attendance_store::Attendance;
use barnacle_bot::attendance_store::CreateOutcome;
use barnacle_bot::attendance_store::EndOutcome;
use barnacle_bot::attendance_store::Event;
use barnacle_bot::attendance_store::EventChange;
use barnacle_bot::attendance_store::NewEvent;
use barnacle_bot::attendance_store::Post;
use barnacle_bot::attendance_store::PostState;
use barnacle_bot::event_draft::Applied;
use barnacle_bot::event_draft::Ready;
use barnacle_bot::failure::Kind;
use barnacle_bot::ids::ChannelId;
use barnacle_bot::ids::GuildId;
use barnacle_bot::ids::Ping;
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
use barnacle_bot::text;
use barnacle_bot::wiring;
use barnacle_guess::Snowflake;
use barnacle_guess::UserId;
use common::attendance_pool;
use common::fakes::BoardCall;
use common::fakes::FakeBoard;
use jiff::civil::Date;
use jiff::civil::Weekday;
use sqlx::sqlite::SqlitePool;
use tokio::sync::Semaphore;

const GUILD: GuildId = GuildId::new(1);
const REHEARSAL: GuildId = GuildId::new(2);
const CHANNEL: ChannelId = ChannelId::new(10);
const OTHER_CHANNEL: ChannelId = ChannelId::new(11);
const REHEARSAL_CHANNEL: ChannelId = ChannelId::new(20);
const MANAGER: UserId = UserId::new(100);
const AKI: UserId = UserId::new(200);
const BOREALIS: UserId = UserId::new(300);
const CREWMATES: RoleId = RoleId::new(4242);
const RESERVES: RoleId = RoleId::new(4343);
const TRIALS: RoleId = RoleId::new(4444);
const SEASON_35: &str = "Clan Battles · Season 35: Komodo Dragon";

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

fn day(text: &str) -> Date {
    parse_day(text).unwrap()
}

fn first_date() -> Date {
    day("2026-09-16")
}

fn second_date() -> Date {
    day("2026-09-17")
}

fn third_date() -> Date {
    day("2026-09-18")
}

fn hour(value: u8) -> Hour {
    Hour::new(value).unwrap()
}

fn hours(count: i64) -> Hours {
    Hours::new(count).unwrap()
}

fn schedule(weekdays: Weekdays, start: &str, zone: &str, count: i64) -> Schedule {
    Schedule::new(
        weekdays,
        StartTime::parse(start).unwrap(),
        Zone::find(zone).unwrap(),
        hours(count),
    )
}

fn span(first: &str, last: &str, schedule: Schedule) -> Span {
    Span::new(day(first), day(last), schedule).unwrap()
}

fn every_day_at_2330() -> Schedule {
    schedule(Weekdays::EVERY_DAY, "23:30", "UTC", 4)
}

fn proposal() -> NewEvent {
    NewEvent {
        guild: GUILD,
        channel: CHANNEL,
        name: SEASON_35.to_owned(),
        span: span("2026-09-16", "2026-11-05", Schedule::clan_battles()),
        created_by: MANAGER,
        created_at_ms: millis(FIRST_POST_AT),
        ping_roles: Vec::new(),
    }
}

fn named(name: &str, first: &str, last: &str, schedule: Schedule) -> NewEvent {
    NewEvent {
        name: name.to_owned(),
        span: span(first, last, schedule),
        ..proposal()
    }
}

async fn created(store: &Attendance, new: &NewEvent) -> Event {
    match store.create_event(new).await.unwrap() {
        CreateOutcome::Created(event) => event,
        other => panic!("expected a created event, got {other:?}"),
    }
}

async fn ready_on(
    board: FakeBoard,
    new: &NewEvent,
) -> (Arc<Signups<FakeBoard>>, Event, Attendance, SqlitePool) {
    let pool = attendance_pool().await;
    let store = Attendance::with_pool(pool.clone()).await.unwrap();
    let event = created(&store, new).await;
    (Signups::new(board, store.clone()), event, store, pool)
}

async fn ready_from(
    board: FakeBoard,
    new: &NewEvent,
) -> (Arc<Signups<FakeBoard>>, Event, Attendance) {
    let (signups, event, store, _pool) = ready_on(board, new).await;
    (signups, event, store)
}

fn rehearsal_proposal() -> NewEvent {
    NewEvent {
        guild: REHEARSAL,
        channel: REHEARSAL_CHANNEL,
        name: "Drill".to_owned(),
        span: span("2026-09-16", "2026-09-18", every_day_at_2330()),
        created_at_ms: millis(REAL_NOW),
        ..proposal()
    }
}

async fn two_servers() -> (Attendance, Event, Event) {
    let store = Attendance::with_pool(attendance_pool().await)
        .await
        .unwrap();
    let clan = created(&store, &proposal()).await;
    let elsewhere = created(&store, &rehearsal_proposal()).await;
    (store, clan, elsewhere)
}

async fn ready(board: FakeBoard) -> (Arc<Signups<FakeBoard>>, Event, Attendance) {
    ready_from(board, &proposal()).await
}

async fn both_sessions(
    board: FakeBoard,
) -> (
    Arc<Signups<FakeBoard>>,
    Event,
    Attendance,
    Snowflake,
    Snowflake,
) {
    let (signups, event, store) = ready(board).await;
    signups.tick(FIRST_POST_AT, millis(FIRST_POST_AT)).await;
    signups.tick(FIRST_START, millis(FIRST_START)).await;
    let first = message_for(&store, &event, first_date()).await;
    let second = message_for(&store, &event, second_date()).await;
    (signups, event, store, first, second)
}

async fn message_for(store: &Attendance, event: &Event, date: Date) -> Snowflake {
    store
        .post(event, date)
        .await
        .unwrap()
        .expect("a post was recorded")
        .message
}

async fn state_of(store: &Attendance, event: &Event, date: Date) -> PostState {
    store
        .post(event, date)
        .await
        .unwrap()
        .expect("a post was recorded")
        .state
}

fn session(event: &Event, date: Date) -> Session {
    event.span.schedule().session_at(date).unwrap()
}

fn tag(event: &Event, date: Date) -> PostTag {
    PostTag {
        event: event.id,
        session: session(event, date),
    }
}

fn press(event: &Event, message: Snowflake, user: UserId, target: Target) -> Click {
    Click {
        guild: event.guild,
        channel: event.channel,
        message,
        user,
        event: event.id,
        date: first_date(),
        target,
        attending: true,
    }
}

fn with_posts(event: &Event) -> Event {
    Event {
        posts_up: true,
        ..event.clone()
    }
}

fn attending(view: &SignupView) -> Vec<u32> {
    view.hours.iter().map(|tally| tally.attending).collect()
}

fn nope(view: &SignupView) -> Vec<u32> {
    view.hours.iter().map(|tally| tally.nope).collect()
}

async fn moved(pool: &SqlitePool, event: &Event, channel: ChannelId) {
    sqlx::query("UPDATE cb_seasons SET channel_id = ? WHERE id = ?")
        .bind(i64::try_from(channel.get()).unwrap())
        .bind(event.id)
        .execute(pool)
        .await
        .unwrap();
}

async fn unreadable_row(pool: &SqlitePool, guild: GuildId, first: &str, last: &str) {
    sqlx::query("INSERT INTO cb_seasons (guild_id, channel_id, name, first_day, last_day, weekdays, start_minute, time_zone, hours, created_by, created_at_ms) VALUES (?, 10, 'Ranked Sprint', ?, ?, 108, 1410, 'Nowhere/City', 4, 100, 0)")
        .bind(i64::try_from(guild.get()).unwrap())
        .bind(first)
        .bind(last)
        .execute(pool)
        .await
        .unwrap();
}

#[tokio::test]
async fn posts_the_due_session_once_across_repeated_ticks() {
    let board = FakeBoard::new();
    let (signups, event, store) = ready(board.clone()).await;
    let report = signups.tick(FIRST_POST_AT, millis(FIRST_POST_AT)).await;
    assert_eq!(report.posted, vec![tag(&event, first_date())]);
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
        state_of(&store, &event, first_date()).await,
        PostState::Open
    );
}

#[tokio::test]
async fn a_rebuilt_signups_does_not_post_twice() {
    let board = FakeBoard::new();
    let (signups, event, store) = ready(board.clone()).await;
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
        message_for(&store, &event, first_date()).await,
        Snowflake::new(1_000)
    );
}

#[tokio::test]
async fn a_post_found_in_the_channel_is_adopted_not_resent() {
    let board = FakeBoard::new();
    let (signups, event, store) = ready(board.clone()).await;
    board.plant(tag(&event, first_date()), Snowflake::new(4_242));
    let report = signups.tick(FIRST_POST_AT, millis(FIRST_POST_AT)).await;
    assert_eq!(report.adopted, vec![tag(&event, first_date())]);
    assert!(report.posted.is_empty());
    assert!(board.sends().is_empty());
    assert_eq!(
        message_for(&store, &event, first_date()).await,
        Snowflake::new(4_242)
    );
}

#[tokio::test]
async fn a_failed_send_is_retried_next_tick() {
    let board = FakeBoard::new();
    let (signups, event, store) = ready(board.clone()).await;
    board.fail_sends(true);
    let failed = signups.tick(FIRST_POST_AT, millis(FIRST_POST_AT)).await;
    assert!(failed.posted.is_empty());
    assert_eq!(failed.failures, 1);
    assert_eq!(store.post(&event, first_date()).await.unwrap(), None);
    board.fail_sends(false);
    let retried = signups
        .tick(FIRST_POST_AT + 60, millis(FIRST_POST_AT + 60))
        .await;
    assert_eq!(retried.posted, vec![tag(&event, first_date())]);
    assert_eq!(retried.failures, 0);
    assert_eq!(board.sends().len(), 2);
}

#[tokio::test]
async fn a_session_that_started_while_offline_is_never_posted() {
    let board = FakeBoard::new();
    let (signups, event, store) = ready(board.clone()).await;
    let report = signups.tick(FIRST_START, millis(FIRST_START)).await;
    assert_eq!(report.posted, vec![tag(&event, second_date())]);
    assert_eq!(store.post(&event, first_date()).await.unwrap(), None);
    let later = signups
        .tick(FIRST_START + 3_600, millis(FIRST_START + 3_600))
        .await;
    assert!(later.posted.is_empty());
    assert_eq!(store.post(&event, first_date()).await.unwrap(), None);
}

#[tokio::test]
async fn closes_at_the_start() {
    let board = FakeBoard::new();
    let (signups, event, store) = ready(board.clone()).await;
    signups.tick(FIRST_POST_AT, millis(FIRST_POST_AT)).await;
    let report = signups.tick(FIRST_START, millis(FIRST_START)).await;
    assert_eq!(report.closed, vec![tag(&event, first_date())]);
    assert_eq!(report.failures, 0);
    assert_eq!(
        state_of(&store, &event, first_date()).await,
        PostState::Closed
    );
    let closing = board.edits().pop().unwrap();
    assert!(!closing.open);
    assert_eq!(closing.session.date(), first_date());
    let again = signups
        .tick(FIRST_START + 60, millis(FIRST_START + 60))
        .await;
    assert!(again.closed.is_empty());
    assert_eq!(board.edits().len(), 1);
}

#[tokio::test]
async fn marks_the_post_closed_only_after_the_edit_succeeds() {
    let board = FakeBoard::new();
    let (signups, event, store) = ready(board.clone()).await;
    signups.tick(FIRST_POST_AT, millis(FIRST_POST_AT)).await;
    board.fail_edits(true);
    let refused = signups.tick(FIRST_START, millis(FIRST_START)).await;
    assert!(refused.closed.is_empty());
    assert_eq!(refused.failures, 1);
    assert_eq!(
        state_of(&store, &event, first_date()).await,
        PostState::Open
    );
    board.fail_edits(false);
    let closed = signups
        .tick(FIRST_START + 60, millis(FIRST_START + 60))
        .await;
    assert_eq!(closed.closed, vec![tag(&event, first_date())]);
    assert_eq!(
        state_of(&store, &event, first_date()).await,
        PostState::Closed
    );
}

#[tokio::test]
async fn removes_thirty_minutes_after_the_end() {
    let board = FakeBoard::new();
    let (signups, event, store) = ready(board.clone()).await;
    signups.tick(FIRST_POST_AT, millis(FIRST_POST_AT)).await;
    signups.tick(FIRST_START, millis(FIRST_START)).await;
    let early = signups
        .tick(FIRST_REMOVE_AT - 1, millis(FIRST_REMOVE_AT - 1))
        .await;
    assert!(early.removed.is_empty());
    assert!(board.deletes().is_empty());
    let report = signups.tick(FIRST_REMOVE_AT, millis(FIRST_REMOVE_AT)).await;
    assert_eq!(report.removed, vec![tag(&event, first_date())]);
    assert_eq!(board.deletes(), vec![Snowflake::new(1_000)]);
    assert_eq!(
        state_of(&store, &event, first_date()).await,
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
    let (signups, event, store) = ready(board.clone()).await;
    signups.tick(FIRST_POST_AT, millis(FIRST_POST_AT)).await;
    board.report_gone(true);
    let report = signups.tick(FIRST_REMOVE_AT, millis(FIRST_REMOVE_AT)).await;
    assert_eq!(report.removed, vec![tag(&event, first_date())]);
    assert_eq!(report.failures, 0);
    assert_eq!(
        state_of(&store, &event, first_date()).await,
        PostState::Removed
    );
}

#[tokio::test]
async fn a_long_gap_removes_and_posts_in_one_tick() {
    let board = FakeBoard::new();
    let (signups, event, store) = ready(board.clone()).await;
    signups.tick(FIRST_POST_AT, millis(FIRST_POST_AT)).await;
    let report = signups.tick(FIRST_REMOVE_AT, millis(FIRST_REMOVE_AT)).await;
    assert_eq!(report.removed, vec![tag(&event, first_date())]);
    assert_eq!(report.posted, vec![tag(&event, second_date())]);
    assert_eq!(report.failures, 0);
    assert_eq!(
        state_of(&store, &event, first_date()).await,
        PostState::Removed
    );
    assert_eq!(
        state_of(&store, &event, second_date()).await,
        PostState::Open
    );
}

#[tokio::test]
async fn a_click_after_the_start_writes_nothing() {
    let board = FakeBoard::new();
    let (signups, event, store) = ready(board.clone()).await;
    signups.tick(FIRST_POST_AT, millis(FIRST_POST_AT)).await;
    let message = message_for(&store, &event, first_date()).await;
    let outcome = signups
        .click(
            press(&event, message, AKI, Target::All),
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
            .roster(event.id, first_date())
            .await
            .unwrap()
            .is_empty()
    );
    assert!(board.edits().is_empty());
}

#[tokio::test]
async fn a_click_from_another_guild_is_refused() {
    let board = FakeBoard::new();
    let (signups, event, store) = ready(board.clone()).await;
    signups.tick(FIRST_POST_AT, millis(FIRST_POST_AT)).await;
    let message = message_for(&store, &event, first_date()).await;
    let elsewhere = Click {
        guild: GuildId::new(999),
        ..press(&event, message, AKI, Target::All)
    };
    assert_eq!(
        signups
            .click(elsewhere, FIRST_POST_AT, millis(FIRST_POST_AT))
            .await,
        ClickOutcome::UnknownEvent
    );
    let unknown = Click {
        event: event.id + 1,
        ..press(&event, message, AKI, Target::All)
    };
    assert_eq!(
        signups
            .click(unknown, FIRST_POST_AT, millis(FIRST_POST_AT))
            .await,
        ClickOutcome::UnknownEvent
    );
    let outside = Click {
        date: day("2026-11-08"),
        ..press(&event, message, AKI, Target::All)
    };
    assert_eq!(
        signups
            .click(outside, FIRST_POST_AT, millis(FIRST_POST_AT))
            .await,
        ClickOutcome::UnknownEvent
    );
    let not_a_cb_day = Click {
        date: day("2026-09-25"),
        ..press(&event, message, AKI, Target::All)
    };
    assert_eq!(
        signups
            .click(not_a_cb_day, FIRST_POST_AT, millis(FIRST_POST_AT))
            .await,
        ClickOutcome::UnknownEvent
    );
    assert!(
        store
            .roster(event.id, first_date())
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn attend_all_writes_four_marks() {
    let board = FakeBoard::new();
    let (signups, event, store) = ready(board.clone()).await;
    signups.tick(FIRST_POST_AT, millis(FIRST_POST_AT)).await;
    let message = message_for(&store, &event, first_date()).await;
    let outcome = signups
        .click(
            press(&event, message, AKI, Target::All),
            FIRST_POST_AT,
            millis(FIRST_POST_AT),
        )
        .await;
    assert_eq!(outcome, ClickOutcome::Recorded);
    let roster = store.roster(event.id, first_date()).await.unwrap();
    assert_eq!(roster.len(), 4);
    assert_eq!(
        roster.iter().map(|mark| mark.hour).collect::<Vec<Hour>>(),
        vec![hour(1), hour(2), hour(3), hour(4)]
    );
    assert!(roster.iter().all(|mark| mark.attending));
    let drawn = board.edits().pop().unwrap();
    assert_eq!(drawn.rows.len(), 1);
    assert_eq!(drawn.rows[0].user, AKI);
    assert_eq!(drawn.rows[0].cells, [Cell::In; 4]);
    assert_eq!(attending(&drawn), [1, 1, 1, 1]);
    assert_eq!(nope(&drawn), [0, 0, 0, 0]);
    assert_eq!(drawn.hidden, 0);
    assert!(drawn.open);
}

#[tokio::test]
async fn twenty_clicks_during_one_slow_edit_make_at_most_three_edits() {
    let board = FakeBoard::gated();
    let (signups, event, store) = ready(board.clone()).await;
    signups.tick(FIRST_POST_AT, millis(FIRST_POST_AT)).await;
    let message = message_for(&store, &event, first_date()).await;
    let clicking: Vec<_> = (0..21)
        .map(|offset| {
            let signups = Arc::clone(&signups);
            let event = event.clone();
            tokio::spawn(async move {
                signups
                    .click(
                        press(
                            &event,
                            message,
                            UserId::new(500 + offset),
                            Target::One(hour(1)),
                        ),
                        FIRST_POST_AT,
                        millis(FIRST_POST_AT),
                    )
                    .await
            })
        })
        .collect();
    while store.roster(event.id, first_date()).await.unwrap().len() < 21 * 4 {
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
    let (signups, event, store) = ready(board.clone()).await;
    signups.tick(FIRST_POST_AT, millis(FIRST_POST_AT)).await;
    let message = message_for(&store, &event, first_date()).await;
    signups
        .click(
            press(&event, message, AKI, Target::One(hour(1))),
            FIRST_POST_AT,
            millis(FIRST_POST_AT),
        )
        .await;
    let last = Click {
        attending: false,
        ..press(&event, message, BOREALIS, Target::One(hour(2)))
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
    assert_eq!(attending(&drawn), [1, 0, 0, 0]);
    assert_eq!(nope(&drawn), [1, 2, 2, 2]);
    assert_eq!(drawn.event, event.id);
    assert_eq!(drawn.name, SEASON_35);
    assert_eq!(SECOND_START, session(&event, second_date()).start_unix());
}

#[tokio::test]
async fn a_single_hour_answer_marks_every_other_hour_nope() {
    let board = FakeBoard::new();
    let (signups, event, store) = ready(board.clone()).await;
    signups.tick(FIRST_POST_AT, millis(FIRST_POST_AT)).await;
    let message = message_for(&store, &event, first_date()).await;
    let outcome = signups
        .click(
            press(&event, message, AKI, Target::One(hour(2))),
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
    assert_eq!(attending(&drawn), [0, 1, 0, 0]);
    assert_eq!(nope(&drawn), [1, 0, 1, 1]);
}

#[tokio::test]
async fn a_later_single_hour_answer_keeps_the_earlier_answers() {
    let board = FakeBoard::new();
    let (signups, event, store) = ready(board.clone()).await;
    signups.tick(FIRST_POST_AT, millis(FIRST_POST_AT)).await;
    let message = message_for(&store, &event, first_date()).await;
    for (value, offset) in [(1, 0), (3, 1)] {
        let outcome = signups
            .click(
                press(&event, message, AKI, Target::One(hour(value))),
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
    assert_eq!(attending(&drawn), [1, 0, 1, 0]);
    assert_eq!(nope(&drawn), [0, 1, 0, 1]);
}

#[tokio::test]
async fn a_partial_answer_saved_before_the_change_shows_nope() {
    let board = FakeBoard::new();
    let (signups, event, store, pool) = ready_on(board, &proposal()).await;
    signups.tick(FIRST_POST_AT, millis(FIRST_POST_AT)).await;
    sqlx::query("INSERT INTO cb_marks (season_id, night, user_id, hour, attending, answered_at_ms, changed_at_ms) VALUES (?, ?, ?, 2, 1, ?, ?)")
        .bind(event.id)
        .bind(first_date().to_string())
        .bind(i64::try_from(AKI.get()).unwrap())
        .bind(i64::try_from(millis(FIRST_POST_AT)).unwrap())
        .bind(i64::try_from(millis(FIRST_POST_AT)).unwrap())
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(store.roster(event.id, first_date()).await.unwrap().len(), 1);
    let view = signups
        .view(&event, session(&event, first_date()), FIRST_POST_AT)
        .await
        .unwrap();
    assert_eq!(view.rows.len(), 1);
    assert_eq!(view.rows[0].user, AKI);
    assert_eq!(
        view.rows[0].cells,
        [Cell::Out, Cell::In, Cell::Out, Cell::Out]
    );
    assert_eq!(attending(&view), [0, 1, 0, 0]);
    assert_eq!(nope(&view), [1, 0, 1, 1]);
}

#[tokio::test]
async fn an_answer_above_the_events_hours_is_left_out_of_the_view() {
    let board = FakeBoard::new();
    let (signups, event, store, pool) = ready_on(board, &proposal()).await;
    signups.tick(FIRST_POST_AT, millis(FIRST_POST_AT)).await;
    store
        .mark(
            event.id,
            first_date(),
            AKI,
            &[hour(1)],
            hours(4),
            true,
            millis(FIRST_POST_AT),
        )
        .await
        .unwrap();
    sqlx::query("INSERT INTO cb_marks (season_id, night, user_id, hour, attending, answered_at_ms, changed_at_ms) VALUES (?, ?, ?, 6, 1, 0, 0)")
        .bind(event.id)
        .bind(first_date().to_string())
        .bind(i64::try_from(BOREALIS.get()).unwrap())
        .execute(&pool)
        .await
        .unwrap();
    let view = signups
        .view(&event, session(&event, first_date()), FIRST_POST_AT)
        .await
        .unwrap();
    assert_eq!(view.hours.len(), 4);
    assert_eq!(view.rows.len(), 1);
    assert_eq!(view.rows[0].user, AKI);
    assert_eq!(view.rows[0].cells.len(), 4);
    assert_eq!(attending(&view), [1, 0, 0, 0]);
}

#[tokio::test]
async fn removes_the_last_session_of_an_event() {
    let board = FakeBoard::new();
    let one_session = NewEvent {
        span: span("2026-09-16", "2026-09-16", Schedule::clan_battles()),
        ..proposal()
    };
    let (signups, event, store) = ready_from(board.clone(), &one_session).await;
    signups.tick(FIRST_POST_AT, millis(FIRST_POST_AT)).await;
    signups.tick(FIRST_START, millis(FIRST_START)).await;
    let report = signups.tick(FIRST_REMOVE_AT, millis(FIRST_REMOVE_AT)).await;
    assert_eq!(report.removed, vec![tag(&event, first_date())]);
    assert_eq!(board.deletes().len(), 1);
    assert_eq!(
        state_of(&store, &event, first_date()).await,
        PostState::Removed
    );
}

#[tokio::test]
async fn the_roster_stops_at_the_limit_and_counts_the_rest() {
    let board = FakeBoard::new();
    let (signups, event, store) = ready(board.clone()).await;
    signups.tick(FIRST_POST_AT, millis(FIRST_POST_AT)).await;
    let message = message_for(&store, &event, first_date()).await;
    let crowd = u64::try_from(ROSTER_LIMIT + 3).unwrap();
    for number in 0..crowd {
        signups
            .click(
                press(&event, message, UserId::new(1_000 + number), Target::All),
                FIRST_POST_AT,
                millis(FIRST_POST_AT) + number,
            )
            .await;
    }
    let view = signups
        .view(&event, session(&event, first_date()), FIRST_POST_AT)
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
async fn a_moved_event_reposts_the_same_session_with_its_roster() {
    let board = FakeBoard::new();
    let (signups, event, store, pool) = ready_on(board.clone(), &proposal()).await;
    signups.tick(FIRST_POST_AT, millis(FIRST_POST_AT)).await;
    let message = message_for(&store, &event, first_date()).await;
    signups
        .click(
            press(&event, message, AKI, Target::All),
            FIRST_POST_AT,
            millis(FIRST_POST_AT),
        )
        .await;
    let cleared = signups
        .clear_posts(&event, ClearScope::All, FIRST_POST_AT)
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
        state_of(&store, &event, first_date()).await,
        PostState::Removed
    );
    moved(&pool, &event, OTHER_CHANNEL).await;
    let report = signups
        .tick(FIRST_POST_AT + 60, millis(FIRST_POST_AT + 60))
        .await;
    assert_eq!(report.posted, vec![tag(&event, first_date())]);
    assert_eq!(report.failures, 0);
    let moved = board.sends_to(OTHER_CHANNEL);
    assert_eq!(moved.len(), 1);
    assert_eq!(moved[0].session.date(), first_date());
    assert_eq!(moved[0].rows.len(), 1);
    assert_eq!(moved[0].rows[0].user, AKI);
    assert_eq!(moved[0].rows[0].cells, [Cell::In; 4]);
    assert_eq!(attending(&moved[0]), [1, 1, 1, 1]);
    assert!(moved[0].open);
    assert_eq!(board.sends_to(CHANNEL).len(), 1);
    assert_eq!(
        state_of(&store, &event, first_date()).await,
        PostState::Open
    );
    assert_ne!(message_for(&store, &event, first_date()).await, message);
    assert_eq!(store.roster(event.id, first_date()).await.unwrap().len(), 4);
}

#[tokio::test]
async fn a_session_swept_on_schedule_is_never_reposted() {
    let board = FakeBoard::new();
    let (signups, event, store) = ready(board.clone()).await;
    signups.tick(FIRST_POST_AT, millis(FIRST_POST_AT)).await;
    signups.tick(FIRST_START, millis(FIRST_START)).await;
    let swept = signups.tick(FIRST_REMOVE_AT, millis(FIRST_REMOVE_AT)).await;
    assert_eq!(swept.removed, vec![tag(&event, first_date())]);
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
        state_of(&store, &event, first_date()).await,
        PostState::Removed
    );
    assert_eq!(
        board
            .sends()
            .iter()
            .filter(|view| view.session.date() == first_date())
            .count(),
        1
    );
}

#[tokio::test]
async fn clearing_all_posts_deletes_every_live_message() {
    let board = FakeBoard::new();
    let (signups, event, store, first, second) = both_sessions(board.clone()).await;
    assert_eq!(
        state_of(&store, &event, first_date()).await,
        PostState::Closed
    );
    assert_eq!(
        state_of(&store, &event, second_date()).await,
        PostState::Open
    );
    let report = signups
        .clear_posts(&event, ClearScope::All, FIRST_START)
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
        state_of(&store, &event, first_date()).await,
        PostState::Removed
    );
    assert_eq!(
        state_of(&store, &event, second_date()).await,
        PostState::Removed
    );
    let again = signups
        .clear_posts(&event, ClearScope::All, FIRST_START)
        .await;
    assert_eq!(again, PostsReport::default());
    assert_eq!(board.deletes().len(), 2);
}

#[tokio::test]
async fn clearing_outside_the_schedule_leaves_the_sessions_that_remain() {
    let board = FakeBoard::new();
    let (signups, event, store, _first, second) = both_sessions(board.clone()).await;
    let shortened = Event {
        span: span("2026-09-16", "2026-09-16", Schedule::clan_battles()),
        ..event.clone()
    };
    let report = signups
        .clear_posts(&shortened, ClearScope::OutsideSchedule, FIRST_START)
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
        state_of(&store, &event, first_date()).await,
        PostState::Closed
    );
    assert_eq!(
        state_of(&store, &event, second_date()).await,
        PostState::Removed
    );
}

#[tokio::test]
async fn clearing_outside_new_weekdays_removes_a_session_the_schedule_dropped() {
    let board = FakeBoard::new();
    let (signups, event, store, first, _second) = both_sessions(board.clone()).await;
    let thursdays_only = Event {
        span: span(
            "2026-09-16",
            "2026-11-05",
            schedule(
                Weekdays::new(&[Weekday::Thursday]).unwrap(),
                "23:30",
                "UTC",
                4,
            ),
        ),
        ..event.clone()
    };
    let report = signups
        .clear_posts(&thursdays_only, ClearScope::OutsideSchedule, FIRST_START)
        .await;
    assert_eq!(report.touched, 1);
    assert_eq!(board.deletes(), vec![first]);
    assert_eq!(
        state_of(&store, &event, first_date()).await,
        PostState::Removed
    );
    assert_eq!(
        state_of(&store, &event, second_date()).await,
        PostState::Open
    );
}

#[tokio::test]
async fn a_cleared_post_keeps_its_marks() {
    let board = FakeBoard::new();
    let (signups, event, store) = ready(board.clone()).await;
    signups.tick(FIRST_POST_AT, millis(FIRST_POST_AT)).await;
    let message = message_for(&store, &event, first_date()).await;
    signups
        .click(
            press(&event, message, AKI, Target::All),
            FIRST_POST_AT,
            millis(FIRST_POST_AT),
        )
        .await;
    let report = signups
        .clear_posts(&event, ClearScope::All, FIRST_POST_AT)
        .await;
    assert_eq!(
        report,
        PostsReport {
            touched: 1,
            failures: 0
        }
    );
    let roster = store.roster(event.id, first_date()).await.unwrap();
    assert_eq!(roster.len(), 4);
    assert!(roster.iter().all(|mark| mark.user == AKI && mark.attending));
    assert_eq!(
        state_of(&store, &event, first_date()).await,
        PostState::Removed
    );
}

#[tokio::test]
async fn refresh_posts_edits_and_never_sends() {
    let board = FakeBoard::new();
    let (signups, event, store, pool) = ready_on(board.clone(), &proposal()).await;
    signups.tick(FIRST_POST_AT, millis(FIRST_POST_AT)).await;
    assert_eq!(board.sends().len(), 1);
    assert!(board.edits().is_empty());
    sqlx::raw_sql("UPDATE cb_seasons SET name = 'Blue Whale'; INSERT INTO cb_season_ping_roles (season_id, position, role_id) VALUES (1, 1, 4242)")
        .execute(&pool)
        .await
        .unwrap();
    let report = signups.refresh_posts(&event, FIRST_POST_AT).await;
    assert_eq!(
        report,
        PostsReport {
            touched: 1,
            failures: 0
        }
    );
    let drawn = board.edits();
    assert_eq!(drawn.len(), 1);
    assert_eq!(drawn[0].name, "Blue Whale");
    assert_eq!(drawn[0].pings, vec![Ping::Role(CREWMATES)]);
    assert_eq!(board.sends().len(), 1);
    signups
        .clear_posts(&event, ClearScope::All, FIRST_POST_AT)
        .await;
    assert_eq!(
        signups.refresh_posts(&event, FIRST_POST_AT).await,
        PostsReport::default()
    );
    assert_eq!(board.edits().len(), 1);
    assert_eq!(board.sends().len(), 1);
    assert_eq!(
        state_of(&store, &event, first_date()).await,
        PostState::Removed
    );
}

#[tokio::test]
async fn a_failed_delete_is_counted_and_retried_next_tick() {
    let board = FakeBoard::new();
    let (signups, event, store) = ready(board.clone()).await;
    signups.tick(FIRST_POST_AT, millis(FIRST_POST_AT)).await;
    let message = message_for(&store, &event, first_date()).await;
    board.fail_deletes(true);
    let refused = signups
        .clear_posts(&event, ClearScope::All, FIRST_POST_AT)
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
        state_of(&store, &event, first_date()).await,
        PostState::Open
    );
    board.fail_deletes(false);
    let swept = signups.tick(FIRST_REMOVE_AT, millis(FIRST_REMOVE_AT)).await;
    assert_eq!(swept.removed, vec![tag(&event, first_date())]);
    assert_eq!(board.deletes(), vec![message, message]);
    assert_eq!(
        state_of(&store, &event, first_date()).await,
        PostState::Removed
    );
}

#[tokio::test]
async fn an_ended_event_is_never_posted_again() {
    let board = FakeBoard::new();
    let (signups, event, store) = ready(board.clone()).await;
    signups.tick(FIRST_POST_AT, millis(FIRST_POST_AT)).await;
    assert_eq!(
        store
            .end_event(GUILD, event.id, millis(FIRST_POST_AT))
            .await
            .unwrap(),
        EndOutcome::Ended {
            event: with_posts(&event)
        }
    );
    let cleared = signups
        .clear_posts(&event, ClearScope::All, FIRST_POST_AT)
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
    assert_eq!(store.post(&event, second_date()).await.unwrap(), None);
}

fn pinging_proposal() -> NewEvent {
    NewEvent {
        ping_roles: vec![CREWMATES],
        ..proposal()
    }
}

#[tokio::test]
async fn the_view_carries_the_ping_role() {
    let board = FakeBoard::new();
    let (signups, event, _store) = ready_from(board.clone(), &pinging_proposal()).await;
    signups.tick(FIRST_POST_AT, millis(FIRST_POST_AT)).await;
    assert_eq!(
        board.sends().first().unwrap().pings,
        vec![Ping::Role(CREWMATES)]
    );
    assert_eq!(
        signups
            .view(&event, session(&event, first_date()), FIRST_POST_AT)
            .await
            .unwrap()
            .pings,
        vec![Ping::Role(CREWMATES)]
    );
    let plain = FakeBoard::new();
    let (quiet, event, _store) = ready(plain.clone()).await;
    quiet.tick(FIRST_POST_AT, millis(FIRST_POST_AT)).await;
    assert_eq!(plain.sends().first().unwrap().pings, Vec::new());
    assert_eq!(
        quiet
            .view(&event, session(&event, first_date()), FIRST_POST_AT)
            .await
            .unwrap()
            .pings,
        Vec::new()
    );
}

#[tokio::test]
async fn only_the_first_post_of_a_session_pings() {
    let board = FakeBoard::new();
    let (signups, event, _store, pool) = ready_on(board.clone(), &pinging_proposal()).await;
    signups.tick(FIRST_POST_AT, millis(FIRST_POST_AT)).await;
    assert_eq!(board.pinging_sends().len(), 1);
    let cleared = signups
        .clear_posts(&event, ClearScope::All, FIRST_POST_AT)
        .await;
    assert_eq!(cleared.failures, 0);
    moved(&pool, &event, OTHER_CHANNEL).await;
    let report = signups
        .tick(FIRST_POST_AT + 60, millis(FIRST_POST_AT + 60))
        .await;
    assert_eq!(report.posted, vec![tag(&event, first_date())]);
    assert_eq!(board.sends_to(OTHER_CHANNEL).len(), 1);
    assert_eq!(board.pinging_sends().len(), 1);
}

#[tokio::test]
async fn a_cleared_post_is_deleted_from_the_channel_it_lives_in() {
    let board = FakeBoard::new();
    let (signups, event, store) = ready(board.clone()).await;
    signups.tick(FIRST_POST_AT, millis(FIRST_POST_AT)).await;
    let message = message_for(&store, &event, first_date()).await;
    signups
        .clear_posts(&event, ClearScope::All, FIRST_POST_AT)
        .await;
    assert_eq!(board.deletes_in(CHANNEL), vec![message]);
    assert!(board.deletes_in(OTHER_CHANNEL).is_empty());
}

#[tokio::test]
async fn a_click_on_an_ended_event_is_refused() {
    let board = FakeBoard::new();
    let (signups, event, store) = ready(board.clone()).await;
    signups.tick(FIRST_POST_AT, millis(FIRST_POST_AT)).await;
    let message = message_for(&store, &event, first_date()).await;
    store
        .end_event(GUILD, event.id, millis(FIRST_POST_AT))
        .await
        .unwrap();
    let outcome = signups
        .click(
            press(&event, message, AKI, Target::All),
            FIRST_POST_AT,
            millis(FIRST_POST_AT),
        )
        .await;
    assert_eq!(outcome, ClickOutcome::UnknownEvent);
    assert!(
        store
            .roster(event.id, first_date())
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn a_redraw_renders_the_event_as_it_is_now() {
    let board = FakeBoard::new();
    let (signups, event, _store, pool) = ready_on(board.clone(), &proposal()).await;
    signups.tick(FIRST_POST_AT, millis(FIRST_POST_AT)).await;
    sqlx::query("UPDATE cb_seasons SET name = 'Blue Whale' WHERE id = ?")
        .bind(event.id)
        .execute(&pool)
        .await
        .unwrap();
    let report = signups.refresh_posts(&event, FIRST_POST_AT).await;
    assert_eq!(report.touched, 1);
    assert_eq!(report.failures, 0);
    let edits = board.edits();
    assert_eq!(edits.last().unwrap().name, "Blue Whale");
}

#[tokio::test]
async fn a_purge_stops_at_the_first_event_whose_delete_fails() {
    let board = FakeBoard::new();
    let store = Attendance::with_pool(attendance_pool().await)
        .await
        .unwrap();
    let first = created(&store, &proposal()).await;
    let second = created(
        &store,
        &NewEvent {
            channel: OTHER_CHANNEL,
            ..named(
                "Clan Battles · Season 36",
                "2026-11-11",
                "2026-12-20",
                Schedule::clan_battles(),
            )
        },
    )
    .await;
    let signups = Signups::new(board.clone(), store.clone());
    signups.tick(FIRST_POST_AT, millis(FIRST_POST_AT)).await;
    let message = message_for(&store, &first, first_date()).await;
    store
        .record_post(
            second.id,
            day("2026-11-11"),
            Snowflake::new(9_999),
            millis(FIRST_POST_AT),
        )
        .await
        .unwrap();
    board.fail_deletes_in(OTHER_CHANNEL);
    let refused = signups.purge(GUILD, FIRST_POST_AT).await;
    assert_eq!(refused.events, 0);
    assert_eq!(refused.answers, 0);
    assert_eq!(refused.failures, 1);
    assert_eq!(refused.messages, 1);
    assert_eq!(board.deletes_in(CHANNEL), vec![message]);
    assert_eq!(
        store.events_in_any_state(GUILD).await.unwrap(),
        vec![first.clone(), with_posts(&second)]
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
    assert_eq!(report.posted, vec![tag(&clan, first_date())]);
    assert_eq!(report.failures, 0);
    assert_eq!(store.post(&elsewhere, first_date()).await.unwrap(), None);
    assert_eq!(board.sends_to(CHANNEL).len(), 1);
    assert!(board.sends_to(REHEARSAL_CHANNEL).is_empty());
    let scoped = signups
        .tick_in(REHEARSAL, FIRST_POST_AT, millis(FIRST_POST_AT))
        .await;
    assert_eq!(scoped.posted, vec![tag(&elsewhere, first_date())]);
    assert_eq!(scoped.failures, 0);
    assert_eq!(board.sends_to(REHEARSAL_CHANNEL).len(), 1);
    assert_eq!(board.sends_to(CHANNEL).len(), 1);
}

#[tokio::test]
async fn a_purge_deletes_every_message_row_and_answer() {
    let board = FakeBoard::new();
    let (signups, event, store, first, second) = both_sessions(board.clone()).await;
    let joining = Click {
        date: second_date(),
        ..press(&event, second, AKI, Target::All)
    };
    assert_eq!(
        signups
            .click(joining, FIRST_START, millis(FIRST_START))
            .await,
        ClickOutcome::Recorded
    );
    let declining = Click {
        date: second_date(),
        attending: false,
        ..press(&event, second, BOREALIS, Target::One(hour(1)))
    };
    assert_eq!(
        signups
            .click(declining, FIRST_START, millis(FIRST_START))
            .await,
        ClickOutcome::Recorded
    );
    let report = signups.purge(GUILD, FIRST_START).await;
    assert_eq!(
        report,
        PurgeReport {
            events: 1,
            messages: 2,
            answers: 8,
            failures: 0
        }
    );
    assert_eq!(board.deletes(), vec![first, second]);
    assert_eq!(store.event(event.id).await.unwrap(), None);
    assert!(store.posts(&event).await.unwrap().is_empty());
    assert!(
        store
            .roster(event.id, second_date())
            .await
            .unwrap()
            .is_empty()
    );
    assert!(store.events_in_any_state(GUILD).await.unwrap().is_empty());
    assert_eq!(
        signups.purge(GUILD, FIRST_START).await,
        PurgeReport::default()
    );
    assert_eq!(board.deletes().len(), 2);
}

#[tokio::test]
async fn a_purge_whose_delete_fails_deletes_no_rows() {
    let board = FakeBoard::new();
    let (signups, event, store) = ready(board.clone()).await;
    signups.tick(FIRST_POST_AT, millis(FIRST_POST_AT)).await;
    let message = message_for(&store, &event, first_date()).await;
    signups
        .click(
            press(&event, message, AKI, Target::All),
            FIRST_POST_AT,
            millis(FIRST_POST_AT),
        )
        .await;
    board.fail_deletes(true);
    let refused = signups.purge(GUILD, FIRST_POST_AT).await;
    assert_eq!(refused.failures, 1);
    assert_eq!(refused.events, 0);
    assert_eq!(refused.answers, 0);
    assert_eq!(refused.messages, 0);
    assert_eq!(board.deletes_in(CHANNEL), vec![message]);
    assert_eq!(
        store.event(event.id).await.unwrap(),
        Some(with_posts(&event))
    );
    assert_eq!(
        store.events_in_any_state(GUILD).await.unwrap(),
        vec![with_posts(&event)]
    );
    assert_eq!(message_for(&store, &event, first_date()).await, message);
    assert_eq!(
        state_of(&store, &event, first_date()).await,
        PostState::Open
    );
    assert_eq!(store.roster(event.id, first_date()).await.unwrap().len(), 4);
    board.fail_deletes(false);
    let cleared = signups.purge(GUILD, FIRST_POST_AT).await;
    assert_eq!(
        cleared,
        PurgeReport {
            events: 1,
            messages: 1,
            answers: 4,
            failures: 0
        }
    );
    assert_eq!(store.event(event.id).await.unwrap(), None);
    assert!(
        store
            .roster(event.id, first_date())
            .await
            .unwrap()
            .is_empty()
    );
}

async fn posts_of(store: &Attendance, events: &[Event]) -> Vec<(i64, Vec<Post>)> {
    let mut posts = Vec::new();
    for event in events {
        posts.push((event.id, store.posts(event).await.unwrap()));
    }
    posts
}

async fn offered(store: &Attendance, guild: GuildId, now_unix: i64) -> Option<i64> {
    let events = store.events_in(guild).await.unwrap();
    let posts = posts_of(store, &events).await;
    next_moment(&events, &posts, now_unix)
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

fn post_in(session: Session, state: PostState) -> Post {
    Post {
        session,
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
    assert_eq!(report.posted, vec![tag(&elsewhere, first_date())]);
    assert_eq!(report.failures, 0);
    assert_eq!(board.sends_to(REHEARSAL_CHANNEL).len(), 1);
    assert!(board.sends_to(CHANNEL).is_empty());
    assert_eq!(store.post(&clan, first_date()).await.unwrap(), None);
    assert_eq!(
        store
            .post(&elsewhere, first_date())
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
    assert_eq!(stepped.closed, vec![tag(&elsewhere, first_date())]);
    assert_eq!(stepped.posted, vec![tag(&elsewhere, second_date())]);
    assert_eq!(stepped.failures, 0);
    assert!(board.sends_to(CHANNEL).is_empty());
    assert_eq!(store.post(&clan, first_date()).await.unwrap(), None);
    assert_eq!(
        state_of(&store, &elsewhere, first_date()).await,
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
    assert!(report.posted.contains(&tag(&clan, first_date())));
    assert!(report.posted.contains(&tag(&elsewhere, second_date())));
    assert_eq!(report.failures, 0);
    assert_eq!(
        store
            .post(&clan, first_date())
            .await
            .unwrap()
            .unwrap()
            .posted_at_ms,
        millis(FIRST_POST_AT)
    );
    assert_eq!(store.post(&elsewhere, first_date()).await.unwrap(), None);
    assert_eq!(
        store
            .post(&elsewhere, second_date())
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
    assert!(both.posted.contains(&tag(&quiet_clan, first_date())));
    assert!(both.posted.contains(&tag(&quiet_elsewhere, first_date())));
    assert_eq!(both.failures, 0);
    for event in [&quiet_clan, &quiet_elsewhere] {
        assert_eq!(
            quiet_store
                .post(event, first_date())
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
    let events = vec![clan.clone(), elsewhere.clone()];
    assert_eq!(next_moment(&events, &[], REAL_NOW), Some(FIRST_POST_AT));
    assert_eq!(
        next_moment(&events, &[], FIRST_POST_AT - 1),
        Some(FIRST_POST_AT)
    );
    assert_eq!(next_moment(&events, &[], FIRST_POST_AT), Some(FIRST_START));
    assert_eq!(next_moment(&events, &[], FIRST_START), Some(SECOND_START));
    let clan_only = vec![clan.clone()];
    let first_session = session(&clan, first_date());
    let open = vec![(clan.id, vec![post_in(first_session, PostState::Open)])];
    assert_eq!(
        next_moment(&clan_only, &open, FIRST_START - 1),
        Some(FIRST_START)
    );
    assert_eq!(
        next_moment(&clan_only, &open, FIRST_START),
        Some(FIRST_REMOVE_AT)
    );
    let closed = vec![(clan.id, vec![post_in(first_session, PostState::Closed)])];
    assert_eq!(
        next_moment(&clan_only, &closed, FIRST_REMOVE_AT - 1),
        Some(FIRST_REMOVE_AT)
    );
    assert_eq!(
        next_moment(&clan_only, &closed, FIRST_REMOVE_AT),
        Some(THIRD_START)
    );
    let removed = vec![(clan.id, vec![post_in(first_session, PostState::Removed)])];
    assert_eq!(
        next_moment(&clan_only, &removed, FIRST_REMOVE_AT),
        Some(THIRD_START)
    );
    let spent = vec![(
        elsewhere.id,
        vec![
            post_in(session(&elsewhere, first_date()), PostState::Removed),
            post_in(session(&elsewhere, second_date()), PostState::Removed),
            post_in(session(&elsewhere, third_date()), PostState::Removed),
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
            if let Some(moment) = next_moment(&events, posts, now) {
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
        &NewEvent {
            span: span("2026-09-17", "2026-09-17", Schedule::clan_battles()),
            ..proposal()
        },
    )
    .await;
    thursday_store
        .record_post(
            thursday.id,
            second_date(),
            Snowflake::new(9_000),
            millis(REAL_NOW),
        )
        .await
        .unwrap();
    thursday_store
        .set_post_state(thursday.id, second_date(), PostState::Removed)
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
    assert_eq!(again.posted, vec![tag(&thursday, second_date())]);
    assert_eq!(
        state_of(&thursday_store, &thursday, second_date()).await,
        PostState::Open
    );
}

#[tokio::test]
async fn stepping_walks_post_then_close_and_successor_then_removal() {
    let board = FakeBoard::new();
    let store = Attendance::with_pool(attendance_pool().await)
        .await
        .unwrap();
    let event = created(&store, &rehearsal_proposal()).await;
    let signups = Signups::rehearsing(board.clone(), store.clone(), vec![REHEARSAL]);
    let (moment, report) = step(&signups, REHEARSAL).await.unwrap();
    assert_eq!(moment, FIRST_POST_AT);
    assert_eq!(
        report,
        TickReport {
            posted: vec![tag(&event, first_date())],
            ..TickReport::default()
        }
    );
    assert_eq!(
        store.rehearsal_clock(REHEARSAL).await.unwrap(),
        FIRST_POST_AT - REAL_NOW
    );
    let first = message_for(&store, &event, first_date()).await;
    let (moment, report) = step(&signups, REHEARSAL).await.unwrap();
    assert_eq!(moment, FIRST_START);
    assert_eq!(
        report,
        TickReport {
            posted: vec![tag(&event, second_date())],
            closed: vec![tag(&event, first_date())],
            ..TickReport::default()
        }
    );
    assert_eq!(
        state_of(&store, &event, first_date()).await,
        PostState::Closed
    );
    assert_eq!(
        state_of(&store, &event, second_date()).await,
        PostState::Open
    );
    let (moment, report) = step(&signups, REHEARSAL).await.unwrap();
    assert_eq!(moment, FIRST_REMOVE_AT);
    assert_eq!(
        report,
        TickReport {
            removed: vec![tag(&event, first_date())],
            ..TickReport::default()
        }
    );
    assert_eq!(
        state_of(&store, &event, first_date()).await,
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
    let event = created(&store, &rehearsal_proposal()).await;
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
    for session in event.span.sessions() {
        assert_eq!(
            state_of(&store, &event, session.date()).await,
            PostState::Removed
        );
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
    let event = created(&store, &rehearsal_proposal()).await;
    let signups = Signups::rehearsing(board.clone(), store.clone(), vec![REHEARSAL]);
    let (moment, _) = step(&signups, REHEARSAL).await.unwrap();
    store.set_rehearsal_clock(GUILD, 5).await.unwrap();
    assert_eq!(
        store.rehearsal_clock(REHEARSAL).await.unwrap(),
        moment - REAL_NOW
    );
    let message = message_for(&store, &event, first_date()).await;
    board.fail_deletes(true);
    let refused = signups.purge(REHEARSAL, REAL_NOW).await;
    assert_eq!(refused.failures, 1);
    assert_eq!(refused.events, 0);
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
            events: 1,
            messages: 1,
            answers: 0,
            failures: 0
        }
    );
    assert_eq!(board.deletes_in(REHEARSAL_CHANNEL), vec![message, message]);
    assert!(
        store
            .events_in_any_state(REHEARSAL)
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
    let message = message_for(&store, &elsewhere, first_date()).await;
    let press = Click {
        guild: REHEARSAL,
        channel: REHEARSAL_CHANNEL,
        message,
        user: AKI,
        event: elsewhere.id,
        date: first_date(),
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
    let (signups, event, store, pool) = ready_on(FakeBoard::new(), &proposal()).await;
    signups.tick(FIRST_POST_AT, millis(FIRST_POST_AT)).await;
    let message = message_for(&store, &event, first_date()).await;
    sqlx::query("DROP TABLE cb_marks")
        .execute(&pool)
        .await
        .unwrap();
    let outcome = signups
        .click(
            press(&event, message, AKI, Target::All),
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
    let (signups, event, _) = ready(board.clone()).await;
    let report = signups.tick(FIRST_POST_AT, millis(FIRST_POST_AT)).await;
    assert_eq!(report.failures, 1);
    let logged = only(logs.named("signup.post.failed"));
    assert_eq!(logged["event.outcome"], "failure");
    assert_eq!(logged["message"], "a sign-up post could not be published");
    assert!(
        logged["exception.message"]
            .as_str()
            .unwrap()
            .contains("the fake board refuses every send")
    );
    assert_eq!(logged["error.type"], "internal");
    assert_eq!(logged["barnacle.reference"].as_str().unwrap().len(), 8);
    assert_eq!(logged["barnacle.season.id"], event.id.to_string());
    assert_eq!(logged["discord.channel.id"], CHANNEL.get().to_string());
    assert_eq!(logged["discord.guild.id"], GUILD.get().to_string());
    assert_eq!(logged["barnacle.night"], "2026-09-16");
    assert!(logs.named("signup.post.published").is_empty());
}

#[tokio::test]
async fn each_sign_up_step_is_recorded_with_its_post() {
    let logs = common::logs::capture();
    let board = FakeBoard::new();
    let (signups, event, store) = ready(board.clone()).await;
    signups.tick(FIRST_POST_AT, millis(FIRST_POST_AT)).await;
    let message = message_for(&store, &event, first_date()).await;
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
            .find(|logged| logged["barnacle.night"] == "2026-09-16")
            .unwrap_or_else(|| panic!("no {name} event for the first session"));
        assert_eq!(first["level"], "INFO");
        assert_eq!(first["event.outcome"], "success");
        assert_eq!(first["discord.message.id"], message.get().to_string());
        assert_eq!(first["barnacle.season.id"], event.id.to_string());
        assert_eq!(first["discord.channel.id"], CHANNEL.get().to_string());
    }
    assert!(logs.named("signup.post.failed").is_empty());
}

#[tokio::test]
async fn a_post_that_cannot_be_redrawn_is_logged_with_its_message() {
    let logs = common::logs::capture();
    let board = FakeBoard::new();
    let (signups, event, store) = ready(board.clone()).await;
    signups.tick(FIRST_POST_AT, millis(FIRST_POST_AT)).await;
    let message = message_for(&store, &event, first_date()).await;
    board.fail_edits(true);
    let report = signups.tick(FIRST_START, millis(FIRST_START)).await;
    assert_eq!(report.failures, 1);
    let logged = only(logs.named("signup.post.failed"));
    assert_eq!(logged["message"], "a sign-up post could not be redrawn");
    assert!(
        logged["exception.message"]
            .as_str()
            .unwrap()
            .contains("the fake board refuses every edit")
    );
    assert_eq!(logged["discord.message.id"], message.get().to_string());
    assert_eq!(logged["barnacle.night"], "2026-09-16");
    assert!(logs.named("signup.post.closed").is_empty());
}

#[tokio::test]
async fn a_post_that_cannot_be_removed_is_logged_once() {
    let logs = common::logs::capture();
    let board = FakeBoard::new();
    let (signups, event, store) = ready(board.clone()).await;
    signups.tick(FIRST_POST_AT, millis(FIRST_POST_AT)).await;
    let message = message_for(&store, &event, first_date()).await;
    board.fail_deletes(true);
    let report = signups
        .clear_posts(&event, ClearScope::All, FIRST_POST_AT)
        .await;
    assert_eq!(report.failures, 1);
    let logged = only(logs.named("signup.post.failed"));
    assert_eq!(logged["message"], "a sign-up post could not be removed");
    assert_eq!(logged["discord.message.id"], message.get().to_string());
    assert_eq!(logged["barnacle.season.id"], event.id.to_string());
}

#[tokio::test]
async fn an_everyone_ping_is_written_as_everyone() {
    let board = FakeBoard::new();
    let everyone = NewEvent {
        ping_roles: vec![RoleId::new(GUILD.get())],
        ..proposal()
    };
    let (signups, _event, _store) = ready_from(board.clone(), &everyone).await;
    signups.tick(FIRST_POST_AT, millis(FIRST_POST_AT)).await;
    let sent = board.sends();
    let view = sent.first().expect("the first session's post was sent");
    assert_eq!(
        wiring::ping_content(&view.pings),
        "@everyone",
        "an event pinging its server's own @everyone role must write the literal @everyone, never <@&server id>"
    );
}

#[tokio::test]
async fn a_ping_discord_did_not_register_logs_a_warning() {
    let logs = common::logs::capture();
    let board = FakeBoard::new();
    board.unheard_pings(true);
    let (signups, event, store) = ready_from(board.clone(), &pinging_proposal()).await;
    let report = signups.tick(FIRST_POST_AT, millis(FIRST_POST_AT)).await;
    assert_eq!(report.posted, vec![tag(&event, first_date())]);
    let message = message_for(&store, &event, first_date()).await;
    let silent = only(logs.named("signup.ping.silent"));
    assert_eq!(silent["level"], "WARN");
    assert_eq!(silent["event.outcome"], "failure");
    assert_eq!(silent["barnacle.season.id"], event.id.to_string());
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
    let (signups, _event, _store) = ready_from(board.clone(), &pinging_proposal()).await;
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
    let (signups, event, _store) = ready_from(board.clone(), &pinging_proposal()).await;
    signups.tick(FIRST_POST_AT, millis(FIRST_POST_AT)).await;
    assert_eq!(logs.named("signup.ping.silent").len(), 1);
    let cleared = signups
        .clear_posts(&event, ClearScope::All, FIRST_POST_AT)
        .await;
    assert_eq!(cleared.failures, 0);
    let logs = common::logs::capture();
    let report = signups
        .tick(FIRST_POST_AT + 60, millis(FIRST_POST_AT + 60))
        .await;
    assert_eq!(report.posted, vec![tag(&event, first_date())]);
    assert!(logs.named("signup.ping.silent").is_empty());
    let published = only(logs.named("signup.post.published"));
    assert!(published.get("barnacle.ping").is_none());
    assert!(published.get("barnacle.ping.heard").is_none());
}

#[tokio::test]
async fn a_first_post_pings_every_role_once() {
    let board = FakeBoard::new();
    let three = NewEvent {
        ping_roles: vec![CREWMATES, RESERVES, TRIALS],
        ..proposal()
    };
    let (signups, _event, _store) = ready_from(board.clone(), &three).await;
    signups.tick(FIRST_POST_AT, millis(FIRST_POST_AT)).await;
    let pinging = board.pinging_sends();
    assert_eq!(pinging.len(), 1);
    assert_eq!(
        pinging[0].pings,
        vec![
            Ping::Role(CREWMATES),
            Ping::Role(RESERVES),
            Ping::Role(TRIALS)
        ]
    );
    signups
        .tick(FIRST_POST_AT + 60, millis(FIRST_POST_AT + 60))
        .await;
    assert_eq!(board.sends().len(), 1);
}

#[tokio::test]
async fn a_partly_heard_ping_logs_only_the_missed_roles() {
    let logs = common::logs::capture();
    let board = FakeBoard::new();
    board.unheard_only(vec![Ping::Role(RESERVES)]);
    let two = NewEvent {
        ping_roles: vec![CREWMATES, RESERVES],
        ..proposal()
    };
    let (signups, _event, _store) = ready_from(board.clone(), &two).await;
    signups.tick(FIRST_POST_AT, millis(FIRST_POST_AT)).await;
    let silent = only(logs.named("signup.ping.silent"));
    assert_eq!(silent["barnacle.ping"], "4242,4343");
    assert_eq!(silent["barnacle.ping.unheard"], "4343");
    let published = only(logs.named("signup.post.published"));
    assert_eq!(published["barnacle.ping"], "4242,4343");
    assert_eq!(published["barnacle.ping.heard"], false);
}

#[tokio::test]
async fn a_recorded_click_logs_who_chose_what() {
    let board = FakeBoard::new();
    let (signups, event, store) = ready(board.clone()).await;
    signups.tick(FIRST_POST_AT, millis(FIRST_POST_AT)).await;
    let message = message_for(&store, &event, first_date()).await;
    let logs = common::logs::capture();
    let outcome = signups
        .click(
            Click {
                attending: false,
                ..press(&event, message, AKI, Target::One(hour(2)))
            },
            FIRST_POST_AT,
            millis(FIRST_POST_AT),
        )
        .await;
    assert_eq!(outcome, ClickOutcome::Recorded);
    let logged = only(logs.named("signup.click.recorded"));
    assert_eq!(logged["level"], "INFO");
    assert_eq!(logged["event.outcome"], "success");
    assert_eq!(logged["discord.user.id"], AKI.get().to_string());
    assert_eq!(logged["discord.guild.id"], GUILD.get().to_string());
    assert_eq!(logged["discord.channel.id"], CHANNEL.get().to_string());
    assert_eq!(logged["barnacle.season.id"], event.id.to_string());
    assert_eq!(logged["barnacle.night"], "2026-09-16");
    assert_eq!(logged["discord.message.id"], message.get().to_string());
    assert_eq!(logged["barnacle.signup.target"], "2");
    assert_eq!(logged["barnacle.signup.choice"], "nope");
}

const REPOST_AT: i64 = FIRST_POST_AT + 600;

async fn posted(
    board: FakeBoard,
    new: &NewEvent,
) -> (Arc<Signups<FakeBoard>>, Event, Attendance, Snowflake) {
    let (signups, event, store) = ready_from(board, new).await;
    signups.tick(FIRST_POST_AT, millis(FIRST_POST_AT)).await;
    let message = message_for(&store, &event, first_date()).await;
    (signups, event, store, message)
}

async fn repost(
    signups: &Arc<Signups<FakeBoard>>,
    event: &Event,
    ping: RepostPing,
) -> RepostOutcome {
    signups
        .repost(GUILD, event.id, ping, REPOST_AT, millis(REPOST_AT))
        .await
}

fn reposted(outcome: RepostOutcome) -> (Snowflake, Snowflake, bool) {
    match outcome {
        RepostOutcome::Reposted {
            previous,
            message,
            adopted,
            ..
        } => (previous, message, adopted),
        other => panic!("expected a repost, got {other:?}"),
    }
}

fn last_delivery(board: &FakeBoard) -> Delivery {
    board
        .calls()
        .into_iter()
        .rev()
        .find_map(|call| match call {
            BoardCall::Sent { delivery, .. } => Some(delivery),
            _ => None,
        })
        .expect("a post was sent")
}

#[tokio::test]
async fn a_repost_keeps_the_answers_and_the_open_state() {
    let board = FakeBoard::new();
    let (signups, event, store, message) = posted(board.clone(), &proposal()).await;
    for user in [AKI, BOREALIS] {
        assert_eq!(
            signups
                .click(
                    press(&event, message, user, Target::All),
                    FIRST_POST_AT,
                    millis(FIRST_POST_AT)
                )
                .await,
            ClickOutcome::Recorded
        );
    }
    let before = signups
        .view(&event, session(&event, first_date()), REPOST_AT)
        .await
        .unwrap();
    assert_eq!(before.rows.len(), 2);
    reposted(repost(&signups, &event, RepostPing::Quiet).await);
    let sent = board.sends();
    assert_eq!(sent.len(), 2);
    let after = sent.last().unwrap();
    assert_eq!(after.rows, before.rows);
    assert_eq!(after.hours, before.hours);
    assert_eq!(
        state_of(&store, &event, first_date()).await,
        PostState::Open
    );
}

#[tokio::test]
async fn a_repost_deletes_the_old_post_and_records_the_new_one() {
    let board = FakeBoard::new();
    let (signups, event, store, old) = posted(board.clone(), &proposal()).await;
    let (previous, message, adopted) = reposted(repost(&signups, &event, RepostPing::Quiet).await);
    assert_eq!(previous, old);
    assert_ne!(message, old);
    assert!(!adopted);
    assert_eq!(board.deletes(), vec![old]);
    assert_eq!(message_for(&store, &event, first_date()).await, message);
}

#[tokio::test]
async fn a_repost_of_a_vanished_post_sends_a_new_one() {
    let board = FakeBoard::new();
    let (signups, event, store, _old) = posted(board.clone(), &proposal()).await;
    board.report_gone(true);
    let (_, message, _) = reposted(repost(&signups, &event, RepostPing::Quiet).await);
    assert_eq!(board.sends().len(), 2);
    assert_eq!(message_for(&store, &event, first_date()).await, message);
}

#[tokio::test]
async fn a_repost_pings_only_when_asked() {
    let board = FakeBoard::new();
    let (signups, event, _store, _old) = posted(board.clone(), &pinging_proposal()).await;
    reposted(repost(&signups, &event, RepostPing::Quiet).await);
    assert_eq!(last_delivery(&board), Delivery::Redraw);
    assert_eq!(board.pinging_sends().len(), 1);
    reposted(
        repost(
            &signups,
            &event,
            RepostPing::Again {
                checked: vec![CREWMATES],
                channel: CHANNEL,
            },
        )
        .await,
    );
    assert_eq!(last_delivery(&board), Delivery::New);
    assert_eq!(board.pinging_sends().len(), 2);
}

#[tokio::test]
async fn only_an_open_post_is_reposted() {
    let board = FakeBoard::new();
    let (signups, event, store) = ready(board.clone()).await;
    assert_eq!(
        signups
            .repost(
                GUILD,
                event.id,
                RepostPing::Quiet,
                REAL_NOW,
                millis(REAL_NOW)
            )
            .await,
        RepostOutcome::NothingOpen {
            next_post_at: Some(FIRST_POST_AT),
            sessions_left: 30,
        }
    );
    assert!(board.calls().is_empty());
    signups.tick(FIRST_POST_AT, millis(FIRST_POST_AT)).await;
    signups.tick(FIRST_START, millis(FIRST_START)).await;
    let first = message_for(&store, &event, first_date()).await;
    let second = message_for(&store, &event, second_date()).await;
    assert_eq!(
        state_of(&store, &event, first_date()).await,
        PostState::Closed
    );
    let outcome = signups
        .repost(
            GUILD,
            event.id,
            RepostPing::Quiet,
            FIRST_START,
            millis(FIRST_START),
        )
        .await;
    let RepostOutcome::Reposted {
        session: reposted_session,
        previous,
        ..
    } = outcome
    else {
        panic!("expected a repost, got {outcome:?}");
    };
    assert_eq!(reposted_session, session(&event, second_date()));
    assert_eq!(previous, second);
    assert!(!board.deletes().contains(&first));
    assert_eq!(message_for(&store, &event, first_date()).await, first);
    assert_eq!(
        state_of(&store, &event, first_date()).await,
        PostState::Closed
    );
}

#[tokio::test]
async fn a_tick_after_a_repost_sends_nothing_new() {
    let board = FakeBoard::new();
    let (signups, event, _store, _old) = posted(board.clone(), &proposal()).await;
    reposted(repost(&signups, &event, RepostPing::Quiet).await);
    assert_eq!(board.sends().len(), 2);
    let report = signups.tick(REPOST_AT + 60, millis(REPOST_AT + 60)).await;
    assert!(report.posted.is_empty());
    assert!(report.adopted.is_empty());
    assert_eq!(board.sends().len(), 2);
}

#[tokio::test]
async fn a_click_on_the_old_post_lands_on_the_new_one() {
    let board = FakeBoard::new();
    let (signups, event, _store, old) = posted(board.clone(), &proposal()).await;
    let (_, message, _) = reposted(repost(&signups, &event, RepostPing::Quiet).await);
    assert_eq!(
        signups
            .click(
                press(&event, old, AKI, Target::All),
                REPOST_AT,
                millis(REPOST_AT)
            )
            .await,
        ClickOutcome::Recorded
    );
    let edited: Vec<(Snowflake, usize)> = board
        .calls()
        .into_iter()
        .filter_map(|call| match call {
            BoardCall::Edited { message, view, .. } => Some((message, view.rows.len())),
            _ => None,
        })
        .collect();
    assert_eq!(edited.last(), Some(&(message, 1)));
    assert!(edited.iter().all(|(edited, _)| *edited != old));
}

#[tokio::test]
async fn a_repost_adopts_a_post_left_by_a_failed_attempt() {
    let board = FakeBoard::new();
    let (signups, event, store, _old) = posted(board.clone(), &proposal()).await;
    let left = Snowflake::new(777);
    board.plant(tag(&event, first_date()), left);
    board.report_gone(true);
    let (_, message, adopted) = reposted(repost(&signups, &event, RepostPing::Quiet).await);
    assert!(adopted);
    assert_eq!(message, left);
    assert_eq!(board.sends().len(), 1);
    assert_eq!(message_for(&store, &event, first_date()).await, left);
}

#[tokio::test]
async fn a_failed_send_after_the_delete_can_be_retried() {
    let board = FakeBoard::new();
    let (signups, event, store, old) = posted(board.clone(), &proposal()).await;
    board.fail_sends(true);
    assert!(matches!(
        repost(&signups, &event, RepostPing::Quiet).await,
        RepostOutcome::Failed(_)
    ));
    assert_eq!(message_for(&store, &event, first_date()).await, old);
    board.fail_sends(false);
    board.report_gone(true);
    let (previous, message, _) = reposted(repost(&signups, &event, RepostPing::Quiet).await);
    assert_eq!(previous, old);
    assert_eq!(message_for(&store, &event, first_date()).await, message);
}

#[tokio::test]
async fn a_repost_whose_post_was_cleared_meanwhile_deletes_its_new_post() {
    let board = FakeBoard::new();
    let (signups, event, store, old) = posted(board.clone(), &proposal()).await;
    board.gate_sends();
    let reposting = {
        let signups = Arc::clone(&signups);
        let event = event.clone();
        tokio::spawn(async move { repost(&signups, &event, RepostPing::Quiet).await })
    };
    while board.sends().len() < 2 {
        tokio::task::yield_now().await;
    }
    store
        .set_post_state(event.id, first_date(), PostState::Removed)
        .await
        .unwrap();
    board.open_sends();
    assert_eq!(reposting.await.unwrap(), RepostOutcome::Superseded);
    let deleted = board.deletes();
    let discarded = *deleted.last().unwrap();
    assert_ne!(discarded, old);
    assert_eq!(
        deleted
            .iter()
            .filter(|message| **message == discarded)
            .count(),
        1
    );
    let post = store.post(&event, first_date()).await.unwrap().unwrap();
    assert_eq!(post.message, old);
    assert_eq!(post.state, PostState::Removed);
}

#[tokio::test]
async fn a_clear_racing_a_repost_leaves_no_post_behind() {
    let board = FakeBoard::new();
    let (signups, event, store, old) = posted(board.clone(), &proposal()).await;
    board.gate_sends();
    let reposting = {
        let signups = Arc::clone(&signups);
        let event = event.clone();
        tokio::spawn(async move { repost(&signups, &event, RepostPing::Quiet).await })
    };
    while board.sends().len() < 2 {
        tokio::task::yield_now().await;
    }
    let clearing = {
        let signups = Arc::clone(&signups);
        let event = event.clone();
        tokio::spawn(async move {
            signups
                .clear_posts(&event, ClearScope::All, REPOST_AT)
                .await
        })
    };
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    assert!(
        !clearing.is_finished(),
        "a clear must wait for a repost that is still sending"
    );
    board.open_sends();
    let RepostOutcome::Reposted { message, .. } = reposting.await.unwrap() else {
        panic!("the repost should finish before the clear starts");
    };
    assert_eq!(clearing.await.unwrap().failures, 0);
    assert_ne!(message, old);
    assert!(board.deletes().contains(&message));
    let post = store.post(&event, first_date()).await.unwrap().unwrap();
    assert_eq!(post.message, message);
    assert_eq!(post.state, PostState::Removed);
}

#[tokio::test]
async fn a_repost_with_changed_pings_is_refused() {
    let board = FakeBoard::new();
    let (signups, event, _store, _old) = posted(board.clone(), &pinging_proposal()).await;
    assert_eq!(
        repost(
            &signups,
            &event,
            RepostPing::Again {
                checked: vec![RoleId::new(999)],
                channel: CHANNEL,
            },
        )
        .await,
        RepostOutcome::PingsChanged
    );
    assert!(board.deletes().is_empty());
    assert_eq!(board.sends().len(), 1);
}

#[tokio::test]
async fn a_repost_whose_event_moved_after_its_check_is_refused() {
    let board = FakeBoard::new();
    let (signups, event, _store, _old) = posted(board.clone(), &pinging_proposal()).await;
    let sends = board.sends().len();
    assert_eq!(
        repost(
            &signups,
            &event,
            RepostPing::Again {
                checked: vec![CREWMATES],
                channel: OTHER_CHANNEL,
            },
        )
        .await,
        RepostOutcome::PingsChanged
    );
    assert!(board.deletes().is_empty());
    assert_eq!(board.sends().len(), sends);
}

#[tokio::test]
async fn a_repost_logs_both_messages() {
    let board = FakeBoard::new();
    let (signups, event, _store, old) = posted(board.clone(), &proposal()).await;
    let logs = common::logs::capture();
    let (_, message, _) = reposted(repost(&signups, &event, RepostPing::Quiet).await);
    let logged = only(logs.named("signup.post.reposted"));
    assert_eq!(logged["level"], "INFO");
    assert_eq!(logged["event.outcome"], "success");
    assert_eq!(logged["discord.message.id"], message.get().to_string());
    assert_eq!(logged["barnacle.message.previous"], old.get().to_string());
    assert_eq!(logged["barnacle.season.id"], event.id.to_string());
    assert_eq!(logged["barnacle.night"], "2026-09-16");
    assert_eq!(logged["barnacle.ping.again"], false);
    assert_eq!(logged["barnacle.post.adopted"], false);
    assert!(logged.get("barnacle.ping").is_none());
    assert!(logs.named("signup.post.published").is_empty());
}

#[tokio::test]
async fn two_events_with_one_schedule_in_one_channel_both_post() {
    let board = FakeBoard::new();
    let store = Attendance::with_pool(attendance_pool().await)
        .await
        .unwrap();
    let first = created(
        &store,
        &named(
            "Ranked Sprint",
            "2026-10-14",
            "2026-10-18",
            Schedule::clan_battles(),
        ),
    )
    .await;
    let second = created(
        &store,
        &named(
            "Training Room",
            "2026-10-14",
            "2026-10-18",
            Schedule::clan_battles(),
        ),
    )
    .await;
    let signups = Signups::new(board.clone(), store.clone());
    let report = signups.tick(1_791_934_200, millis(1_791_934_200)).await;
    assert_eq!(report.failures, 0);
    let sent = board.sends_to(ChannelId::new(10));
    assert_eq!(sent.len(), 2);
    assert!(
        sent.iter()
            .all(|view| view.session.date() == day("2026-10-14"))
    );
    let mut events: Vec<i64> = sent.iter().map(|view| view.event).collect();
    events.sort_unstable();
    assert_eq!(events, vec![first.id, second.id]);
}

#[tokio::test]
async fn answers_to_one_overlapping_event_leave_the_other_alone() {
    let board = FakeBoard::new();
    let store = Attendance::with_pool(attendance_pool().await)
        .await
        .unwrap();
    let first = created(
        &store,
        &named(
            "Ranked Sprint",
            "2026-10-14",
            "2026-10-18",
            Schedule::clan_battles(),
        ),
    )
    .await;
    let second = created(
        &store,
        &named(
            "Training Room",
            "2026-10-14",
            "2026-10-18",
            Schedule::clan_battles(),
        ),
    )
    .await;
    let signups = Signups::new(board.clone(), store.clone());
    signups.tick(1_791_934_200, millis(1_791_934_200)).await;
    let message = message_for(&store, &first, day("2026-10-14")).await;
    let outcome = signups
        .click(
            Click {
                date: day("2026-10-14"),
                ..press(&first, message, AKI, Target::All)
            },
            1_791_934_300,
            millis(1_791_934_300),
        )
        .await;
    assert_eq!(outcome, ClickOutcome::Recorded);
    let edits = board.edits();
    assert!(!edits.is_empty());
    assert!(edits.iter().all(|view| view.event == first.id));
    assert_eq!(attending(edits.last().unwrap()), [1, 1, 1, 1]);
    assert!(
        store
            .roster(second.id, day("2026-10-14"))
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn an_event_inside_another_events_hours_posts_on_its_own() {
    let board = FakeBoard::new();
    let store = Attendance::with_pool(attendance_pool().await)
        .await
        .unwrap();
    let a = created(
        &store,
        &named(
            "Ranked Sprint",
            "2026-10-14",
            "2026-10-18",
            Schedule::clan_battles(),
        ),
    )
    .await;
    let signups = Signups::new(board.clone(), store.clone());
    signups.tick(1_791_934_200, millis(1_791_934_200)).await;
    assert_eq!(
        state_of(&store, &a, day("2026-10-14")).await,
        PostState::Open
    );
    let b = created(
        &store,
        &NewEvent {
            channel: ChannelId::new(11),
            ..named(
                "Night Owls",
                "2026-10-14",
                "2026-10-18",
                schedule(Weekdays::EVERY_DAY, "00:30", "UTC", 2),
            )
        },
    )
    .await;
    let report = signups.tick(1_792_020_600, millis(1_792_020_600)).await;
    assert_eq!(report.failures, 0);
    assert_eq!(report.closed, vec![tag(&a, day("2026-10-14"))]);
    assert_eq!(report.posted.len(), 2);
    assert!(report.posted.contains(&tag(&a, day("2026-10-15"))));
    assert!(report.posted.contains(&tag(&b, day("2026-10-15"))));
    assert_eq!(
        state_of(&store, &a, day("2026-10-14")).await,
        PostState::Closed
    );
    let in_ten: Vec<Date> = board
        .sends_to(ChannelId::new(10))
        .iter()
        .map(|view| view.session.date())
        .collect();
    assert_eq!(in_ten, vec![day("2026-10-14"), day("2026-10-15")]);
    let in_eleven: Vec<(i64, Date)> = board
        .sends_to(ChannelId::new(11))
        .iter()
        .map(|view| (view.event, view.session.date()))
        .collect();
    assert_eq!(in_eleven, vec![(b.id, day("2026-10-15"))]);
}

#[tokio::test]
async fn events_on_different_dates_post_at_their_own_times() {
    let board = FakeBoard::new();
    let store = Attendance::with_pool(attendance_pool().await)
        .await
        .unwrap();
    let a = created(
        &store,
        &named(
            "Ranked Sprint",
            "2026-10-14",
            "2026-10-14",
            Schedule::clan_battles(),
        ),
    )
    .await;
    let b = created(
        &store,
        &named(
            "Training Room",
            "2026-10-17",
            "2026-10-17",
            Schedule::clan_battles(),
        ),
    )
    .await;
    let signups = Signups::new(board.clone(), store.clone());
    let first = signups.tick(1_791_934_200, millis(1_791_934_200)).await;
    assert_eq!(first.posted, vec![tag(&a, day("2026-10-14"))]);
    let second = signups.tick(1_792_193_400, millis(1_792_193_400)).await;
    assert_eq!(second.posted, vec![tag(&b, day("2026-10-17"))]);
}

async fn single_event(
    board: FakeBoard,
    new: &NewEvent,
) -> (Arc<Signups<FakeBoard>>, Event, Attendance) {
    ready_from(board, new).await
}

#[tokio::test]
async fn a_denver_event_posts_closes_and_deletes_on_its_own_times() {
    let board = FakeBoard::new();
    let denver = named(
        "Ranked Sprint",
        "2026-10-14",
        "2026-10-14",
        schedule(
            Weekdays::new(&[Weekday::Wednesday]).unwrap(),
            "17:00",
            "America/Denver",
            4,
        ),
    );
    let (signups, event, store) = single_event(board.clone(), &denver).await;
    let sent = signups.tick(1_791_932_400, millis(1_791_932_400)).await;
    assert_eq!(sent.posted, vec![tag(&event, day("2026-10-14"))]);
    assert_eq!(board.sends().len(), 1);
    let closed = signups.tick(1_792_018_800, millis(1_792_018_800)).await;
    assert_eq!(closed.closed, vec![tag(&event, day("2026-10-14"))]);
    assert_eq!(
        state_of(&store, &event, day("2026-10-14")).await,
        PostState::Closed
    );
    let deleted = signups.tick(1_792_035_000, millis(1_792_035_000)).await;
    assert_eq!(deleted.removed, vec![tag(&event, day("2026-10-14"))]);
    assert_eq!(board.deletes().len(), 1);
    assert_eq!(
        state_of(&store, &event, day("2026-10-14")).await,
        PostState::Removed
    );
}

#[tokio::test]
async fn a_clan_battles_event_keeps_its_times() {
    let board = FakeBoard::new();
    let clan = named(
        "Clan Battles",
        "2026-10-14",
        "2026-10-14",
        Schedule::clan_battles(),
    );
    let (signups, event, store) = single_event(board.clone(), &clan).await;
    let sent = signups.tick(1_791_934_200, millis(1_791_934_200)).await;
    assert_eq!(sent.posted, vec![tag(&event, day("2026-10-14"))]);
    let closed = signups.tick(1_792_020_600, millis(1_792_020_600)).await;
    assert_eq!(closed.closed, vec![tag(&event, day("2026-10-14"))]);
    let deleted = signups.tick(1_792_036_800, millis(1_792_036_800)).await;
    assert_eq!(deleted.removed, vec![tag(&event, day("2026-10-14"))]);
    assert_eq!(
        state_of(&store, &event, day("2026-10-14")).await,
        PostState::Removed
    );
}

#[tokio::test]
async fn a_far_west_event_stays_live_until_its_last_post_is_deleted() {
    let board = FakeBoard::new();
    let far_west = named(
        "Long Haul",
        "2026-10-18",
        "2026-10-18",
        schedule(
            Weekdays::new(&[Weekday::Sunday]).unwrap(),
            "23:00",
            "Etc/GMT+12",
            24,
        ),
    );
    let (signups, event, store) = single_event(board.clone(), &far_west).await;
    let sent = signups.tick(1_792_321_200, millis(1_792_321_200)).await;
    assert_eq!(sent.posted, vec![tag(&event, day("2026-10-18"))]);
    assert_eq!(
        store
            .live_events(1_792_495_799)
            .await
            .unwrap()
            .events
            .iter()
            .map(|live| live.id)
            .collect::<Vec<i64>>(),
        vec![event.id]
    );
    let deleted = signups.tick(1_792_495_800, millis(1_792_495_800)).await;
    assert_eq!(deleted.removed, vec![tag(&event, day("2026-10-18"))]);
    assert_eq!(board.deletes().len(), 1);
    assert!(
        store
            .live_events(1_792_495_800)
            .await
            .unwrap()
            .events
            .is_empty()
    );
}

#[tokio::test]
async fn an_unreadable_time_zone_is_reported() {
    let board = FakeBoard::new();
    let pool = attendance_pool().await;
    let store = Attendance::with_pool(pool.clone()).await.unwrap();
    unreadable_row(&pool, GUILD, "2026-10-14", "2026-10-18").await;
    let signups = Signups::new(board.clone(), store);
    let report = signups.tick(1_791_934_200, millis(1_791_934_200)).await;
    assert_eq!(report.failures, 1);
    assert!(board.sends().is_empty());
    assert!(board.edits().is_empty());
    assert!(board.deletes().is_empty());
}

#[tokio::test]
async fn an_unreadable_event_does_not_stop_the_others() {
    let board = FakeBoard::new();
    let pool = attendance_pool().await;
    let store = Attendance::with_pool(pool.clone()).await.unwrap();
    unreadable_row(&pool, GuildId::new(1), "2026-10-14", "2026-10-18").await;
    let readable = created(
        &store,
        &NewEvent {
            guild: GuildId::new(2),
            ..named(
                "Clan Battles",
                "2026-10-14",
                "2026-10-14",
                Schedule::clan_battles(),
            )
        },
    )
    .await;
    let signups = Signups::new(board.clone(), store);
    let report = signups.tick(1_791_934_200, millis(1_791_934_200)).await;
    assert_eq!(report.failures, 1);
    assert_eq!(report.posted, vec![tag(&readable, day("2026-10-14"))]);
    assert_eq!(board.sends().len(), 1);
}

#[tokio::test]
async fn a_spring_forward_session_posts_a_full_day_ahead() {
    let board = FakeBoard::new();
    let weekend = named(
        "Ranked Sprint",
        "2027-03-13",
        "2027-03-14",
        schedule(
            Weekdays::new(&[Weekday::Saturday, Weekday::Sunday]).unwrap(),
            "17:00",
            "America/Denver",
            4,
        ),
    );
    let (signups, event, store) = single_event(board.clone(), &weekend).await;
    let first = signups.tick(1_804_896_000, millis(1_804_896_000)).await;
    assert_eq!(first.posted, vec![tag(&event, day("2027-03-13"))]);
    let second = signups.tick(1_804_978_800, millis(1_804_978_800)).await;
    assert_eq!(second.posted, vec![tag(&event, day("2027-03-14"))]);
    let sent: Vec<(Date, i64)> = board
        .sends()
        .iter()
        .map(|view| (view.session.date(), view.session.start_unix()))
        .collect();
    assert_eq!(
        sent,
        vec![
            (day("2027-03-13"), 1_804_982_400),
            (day("2027-03-14"), 1_805_065_200)
        ]
    );
    assert_eq!(
        state_of(&store, &event, day("2027-03-14")).await,
        PostState::Open
    );
    let posted_at_ms = store
        .posts(&event)
        .await
        .unwrap()
        .into_iter()
        .find(|post| post.session.date() == day("2027-03-14"))
        .expect("the 2027-03-14 post was recorded")
        .posted_at_ms;
    assert_eq!(posted_at_ms, 1_804_978_800_000);
}

#[tokio::test]
async fn a_button_from_before_0008_still_records() {
    let pool = common::seasons_pool().await;
    sqlx::raw_sql("INSERT INTO cb_seasons (id, guild_id, channel_id, number, codename, first_day, last_day, created_by, created_at_ms) VALUES (1, 1, 10, 35, NULL, '2026-10-14', '2026-10-18', 100, 0); INSERT INTO cb_posts (season_id, night, message_id, state, posted_at_ms) VALUES (1, '2026-10-14', 7, 'open', 1791934200000)")
        .execute(&pool)
        .await
        .unwrap();
    common::run_script(&pool, common::CB_EVENTS).await.unwrap();
    let store = Attendance::with_pool(pool).await.unwrap();
    let signups = Signups::new(FakeBoard::new(), store.clone());
    let pressed = wiring::signup_click("barnacle-cb:1:2026-10-14:all:in").unwrap();
    let outcome = signups
        .click(
            Click {
                guild: GUILD,
                channel: CHANNEL,
                message: Snowflake::new(7),
                user: AKI,
                event: pressed.event,
                date: pressed.date,
                target: pressed.target,
                attending: pressed.attending,
            },
            1_792_020_599,
            millis(1_792_020_599),
        )
        .await;
    assert_eq!(outcome, ClickOutcome::Recorded);
    let roster = store.roster(1, day("2026-10-14")).await.unwrap();
    assert_eq!(
        roster
            .iter()
            .map(|mark| (mark.hour.get(), mark.attending))
            .collect::<Vec<(u8, bool)>>(),
        vec![(1, true), (2, true), (3, true), (4, true)]
    );
}

#[tokio::test]
async fn a_click_at_the_session_start_is_closed() {
    let board = FakeBoard::new();
    let denver = named(
        "Ranked Sprint",
        "2026-10-14",
        "2026-10-14",
        schedule(
            Weekdays::new(&[Weekday::Wednesday]).unwrap(),
            "17:00",
            "America/Denver",
            4,
        ),
    );
    let (signups, event, store) = single_event(board, &denver).await;
    let outcome = signups
        .click(
            Click {
                date: day("2026-10-14"),
                ..press(&event, Snowflake::new(7), AKI, Target::All)
            },
            1_792_018_800,
            millis(1_792_018_800),
        )
        .await;
    assert_eq!(
        outcome,
        ClickOutcome::Closed {
            start_unix: 1_792_018_800
        }
    );
    assert!(
        store
            .roster(event.id, day("2026-10-14"))
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn a_click_for_an_hour_beyond_the_event_is_refused() {
    let board = FakeBoard::new();
    let (signups, event, store) = ready(board.clone()).await;
    signups.tick(FIRST_POST_AT, millis(FIRST_POST_AT)).await;
    let message = message_for(&store, &event, first_date()).await;
    let outcome = signups
        .click(
            press(&event, message, AKI, Target::One(hour(5))),
            FIRST_POST_AT,
            millis(FIRST_POST_AT),
        )
        .await;
    assert_eq!(outcome, ClickOutcome::UnknownEvent);
    assert!(
        store
            .roster(event.id, first_date())
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn a_click_on_a_dropped_date_ended_or_foreign_event_is_gone() {
    let board = FakeBoard::new();
    let store = Attendance::with_pool(attendance_pool().await)
        .await
        .unwrap();
    let live = created(
        &store,
        &named(
            "Ranked Sprint",
            "2026-10-14",
            "2026-10-18",
            Schedule::clan_battles(),
        ),
    )
    .await;
    let ended = created(
        &store,
        &named(
            "Training Room",
            "2026-10-14",
            "2026-10-18",
            Schedule::clan_battles(),
        ),
    )
    .await;
    let signups = Signups::new(board.clone(), store.clone());
    signups.tick(1_791_934_200, millis(1_791_934_200)).await;
    store
        .end_event(GUILD, ended.id, millis(1_791_934_200))
        .await
        .unwrap();
    let on = |event: &Event, date: &str| Click {
        date: day(date),
        ..press(event, Snowflake::new(1_000), AKI, Target::All)
    };
    let clicks = [
        on(&live, "2026-10-16"),
        on(&ended, "2026-10-14"),
        Click {
            guild: GuildId::new(999),
            ..on(&live, "2026-10-14")
        },
        Click {
            event: ended.id + 100,
            ..on(&live, "2026-10-14")
        },
    ];
    for click in clicks {
        assert_eq!(
            signups
                .click(click, 1_791_934_300, millis(1_791_934_300))
                .await,
            ClickOutcome::UnknownEvent,
            "{click:?}"
        );
    }
    for event in [&live, &ended] {
        for date in ["2026-10-14", "2026-10-16"] {
            assert!(store.roster(event.id, day(date)).await.unwrap().is_empty());
        }
    }
}

#[tokio::test]
async fn a_click_on_the_first_session_records() {
    let board = FakeBoard::new();
    let daily = named(
        "Daily Drill",
        "2026-10-14",
        "2026-10-16",
        every_day_at_2330(),
    );
    let (signups, event, store) = single_event(board, &daily).await;
    signups.tick(1_791_934_200, millis(1_791_934_200)).await;
    let message = message_for(&store, &event, day("2026-10-14")).await;
    let first = signups
        .click(
            Click {
                date: day("2026-10-14"),
                ..press(&event, message, AKI, Target::All)
            },
            1_792_020_599,
            millis(1_792_020_599),
        )
        .await;
    assert_eq!(first, ClickOutcome::Recorded);
    assert_eq!(
        store
            .roster(event.id, day("2026-10-14"))
            .await
            .unwrap()
            .iter()
            .map(|mark| (mark.hour.get(), mark.attending))
            .collect::<Vec<(u8, bool)>>(),
        vec![(1, true), (2, true), (3, true), (4, true)]
    );
    let before = signups
        .click(
            Click {
                date: day("2026-10-13"),
                ..press(&event, message, AKI, Target::All)
            },
            1_791_934_199,
            millis(1_791_934_199),
        )
        .await;
    assert_eq!(before, ClickOutcome::UnknownEvent);
    assert!(
        store
            .roster(event.id, day("2026-10-13"))
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn the_list_keeps_events_until_their_last_post_is_deleted() {
    let store = Attendance::with_pool(attendance_pool().await)
        .await
        .unwrap();
    let late = schedule(Weekdays::EVERY_DAY, "16:16", "UTC", 1);
    let ahead = created(
        &store,
        &named(
            "Sessions Ahead",
            "2026-10-14",
            "2026-10-18",
            Schedule::clan_battles(),
        ),
    )
    .await;
    let closing = created(
        &store,
        &named(
            "Closing Up",
            "2026-10-14",
            "2026-10-14",
            schedule(
                Weekdays::new(&[Weekday::Wednesday]).unwrap(),
                "17:00",
                "America/Denver",
                4,
            ),
        ),
    )
    .await;
    assert_eq!(closing.span.last_moment_unix(), Some(1_792_035_000));
    let swept = created(
        &store,
        &named("Swept", "2026-10-14", "2026-10-14", late.clone()),
    )
    .await;
    let stuck = created(&store, &named("Stuck", "2026-10-14", "2026-10-14", late)).await;
    assert!(swept.span.last_moment_unix().unwrap() < 1_792_000_000);
    for event in [&swept, &stuck] {
        store
            .record_post(
                event.id,
                day("2026-10-14"),
                Snowflake::new(7),
                1_791_908_160_000,
            )
            .await
            .unwrap();
    }
    store
        .set_post_state(swept.id, day("2026-10-14"), PostState::Removed)
        .await
        .unwrap();
    store
        .set_post_state(stuck.id, day("2026-10-14"), PostState::Closed)
        .await
        .unwrap();
    let events = store.events_in(GUILD).await.unwrap();
    let listed: Vec<&str> = listed_events(&events, 1_792_030_000)
        .into_iter()
        .map(|event| event.name.as_str())
        .collect();
    assert_eq!(listed, vec![ahead.name.as_str(), "Closing Up", "Stuck"]);
}

#[tokio::test]
async fn reposting_an_event_by_id_keeps_its_answers() {
    let board = FakeBoard::new();
    let (signups, event, store, old) = posted(board.clone(), &pinging_proposal()).await;
    for user in [AKI, BOREALIS] {
        assert_eq!(
            signups
                .click(
                    press(&event, old, user, Target::All),
                    FIRST_POST_AT,
                    millis(FIRST_POST_AT)
                )
                .await,
            ClickOutcome::Recorded
        );
    }
    let (previous, quiet, _) = reposted(repost(&signups, &event, RepostPing::Quiet).await);
    assert_eq!(previous, old);
    assert_eq!(board.deletes(), vec![old]);
    let first = board.sends().pop().unwrap();
    assert_eq!(first.rows.len(), 2);
    assert_eq!(last_delivery(&board), Delivery::Redraw);
    assert_eq!(board.pinging_sends().len(), 1);
    let (previous, loud, _) = reposted(
        repost(
            &signups,
            &event,
            RepostPing::Again {
                checked: vec![CREWMATES],
                channel: CHANNEL,
            },
        )
        .await,
    );
    assert_eq!(previous, quiet);
    assert_eq!(board.deletes(), vec![old, quiet]);
    let second = board.sends().pop().unwrap();
    assert_eq!(second.rows.len(), 2);
    assert_eq!(last_delivery(&board), Delivery::New);
    assert_eq!(board.pinging_sends().len(), 2);
    assert_eq!(second.pings, vec![Ping::Role(CREWMATES)]);
    assert_eq!(message_for(&store, &event, first_date()).await, loud);
}

#[tokio::test]
async fn reposting_with_ping_again_and_no_role_is_refused() {
    let board = FakeBoard::new();
    let (signups, event, store, old) = posted(board.clone(), &proposal()).await;
    assert!(event.ping_roles.is_empty());
    let sent = board.sends().len();
    assert_eq!(
        repost(
            &signups,
            &event,
            RepostPing::Again {
                checked: vec![],
                channel: CHANNEL,
            },
        )
        .await,
        RepostOutcome::NothingToPing
    );
    assert_eq!(board.deletes(), Vec::new());
    assert_eq!(board.sends().len(), sent);
    assert_eq!(message_for(&store, &event, first_date()).await, old);
}

fn drill(guild: GuildId, now_unix: i64) -> NewEvent {
    NewEvent {
        guild,
        channel: CHANNEL,
        name: "Drill".to_owned(),
        span: span("2026-10-15", "2026-10-17", every_day_at_2330()),
        created_by: MANAGER,
        created_at_ms: millis(now_unix),
        ping_roles: Vec::new(),
    }
}

async fn events_held(store: &Attendance) -> usize {
    store.events_in_any_state(GUILD).await.unwrap().len()
}

#[tokio::test]
async fn a_rehearsal_waits_for_running_events() {
    let now = 1_792_030_000;
    let store = Attendance::with_pool(attendance_pool().await)
        .await
        .unwrap();
    let signups = Signups::new(FakeBoard::new(), store.clone());
    assert!(matches!(
        signups.start_rehearsal(drill(GUILD, now), now).await,
        RehearsalOutcome::Started(_)
    ));

    let store = Attendance::with_pool(attendance_pool().await)
        .await
        .unwrap();
    let ended = created(
        &store,
        &named(
            "Old Season",
            "2026-09-16",
            "2026-09-20",
            Schedule::clan_battles(),
        ),
    )
    .await;
    store
        .record_post(ended.id, day("2026-09-16"), Snowflake::new(7), 0)
        .await
        .unwrap();
    store.end_event(GUILD, ended.id, 1_000).await.unwrap();
    let finished = created(
        &store,
        &named("Yesterday", "2026-10-13", "2026-10-13", every_day_at_2330()),
    )
    .await;
    assert_eq!(finished.span.last_moment_unix(), Some(1_791_950_400));
    let signups = Signups::new(FakeBoard::new(), store.clone());
    assert!(matches!(
        signups.start_rehearsal(drill(GUILD, now), now).await,
        RehearsalOutcome::Started(_)
    ));

    let store = Attendance::with_pool(attendance_pool().await)
        .await
        .unwrap();
    created(
        &store,
        &named("Yesterday", "2026-10-13", "2026-10-13", every_day_at_2330()),
    )
    .await;
    created(
        &store,
        &named(
            "Tonight",
            "2026-10-14",
            "2026-10-14",
            Schedule::clan_battles(),
        ),
    )
    .await;
    let signups = Signups::new(FakeBoard::new(), store.clone());
    assert_eq!(
        signups.start_rehearsal(drill(GUILD, now), now).await,
        RehearsalOutcome::Running
    );
    assert_eq!(events_held(&store).await, 2);

    let pool = attendance_pool().await;
    let store = Attendance::with_pool(pool.clone()).await.unwrap();
    unreadable_row(&pool, GUILD, "2026-10-14", "2026-10-18").await;
    let signups = Signups::new(FakeBoard::new(), store.clone());
    assert!(matches!(
        signups.start_rehearsal(drill(GUILD, now), now).await,
        RehearsalOutcome::Failed(_)
    ));
    let rows: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM cb_seasons")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(rows, 1);
}

fn six_hour_proposal() -> NewEvent {
    named(
        "Ranked Sprint",
        "2026-09-16",
        "2026-09-18",
        schedule(Weekdays::EVERY_DAY, "23:30", "UTC", 6),
    )
}

async fn six_hours_open(
    board: FakeBoard,
) -> (
    Arc<Signups<FakeBoard>>,
    Event,
    Attendance,
    SqlitePool,
    Snowflake,
) {
    let (signups, event, store, pool) = ready_on(board, &six_hour_proposal()).await;
    signups.tick(FIRST_POST_AT, millis(FIRST_POST_AT)).await;
    let message = message_for(&store, &event, first_date()).await;
    (signups, event, store, pool, message)
}

fn pick(event: &Event, values: &[&str]) -> HoursPick {
    HoursPick {
        guild: event.guild,
        channel: event.channel,
        user: AKI,
        event: event.id,
        date: first_date(),
        values: values.iter().map(|value| (*value).to_owned()).collect(),
    }
}

async fn answers_of(store: &Attendance, event: &Event, user: UserId) -> Vec<(u8, bool)> {
    store
        .roster(event.id, first_date())
        .await
        .unwrap()
        .iter()
        .filter(|mark| mark.user == user)
        .map(|mark| (mark.hour.get(), mark.attending))
        .collect()
}

fn hours_of(numbers: &[u8]) -> Vec<Hour> {
    numbers.iter().map(|number| hour(*number)).collect()
}

#[tokio::test]
async fn choosing_hours_records_exactly_the_chosen_ones() {
    let board = FakeBoard::new();
    let (signups, event, store, _pool, message) = six_hours_open(board.clone()).await;
    assert_eq!(
        signups
            .click(
                press(&event, message, AKI, Target::All),
                FIRST_POST_AT,
                millis(FIRST_POST_AT)
            )
            .await,
        ClickOutcome::Recorded
    );
    assert_eq!(
        answers_of(&store, &event, AKI).await,
        [
            (1, true),
            (2, true),
            (3, true),
            (4, true),
            (5, true),
            (6, true)
        ]
    );
    let edits = board.edits().len();
    let outcome = signups
        .choose_hours(
            pick(&event, &["2", "3", "5"]),
            FIRST_POST_AT + 60,
            millis(FIRST_POST_AT + 60),
        )
        .await;
    assert_eq!(
        outcome,
        ChooseOutcome::Saved {
            attending: hours_of(&[2, 3, 5])
        }
    );
    assert_eq!(
        answers_of(&store, &event, AKI).await,
        [
            (1, false),
            (2, true),
            (3, true),
            (4, false),
            (5, true),
            (6, false)
        ]
    );
    assert_eq!(board.edits().len(), edits + 1);
    let drawn = board.edits().pop().unwrap();
    assert_eq!(
        drawn.rows[0].cells,
        [
            Cell::Out,
            Cell::In,
            Cell::In,
            Cell::Out,
            Cell::In,
            Cell::Out
        ]
    );
}

#[tokio::test]
async fn choosing_every_hour_attends_the_whole_session() {
    let board = FakeBoard::new();
    let (signups, event, store, _pool, _message) = six_hours_open(board.clone()).await;
    assert!(answers_of(&store, &event, AKI).await.is_empty());
    let outcome = signups
        .choose_hours(
            pick(&event, &["1", "2", "3", "4", "5", "6"]),
            FIRST_POST_AT + 60,
            millis(FIRST_POST_AT + 60),
        )
        .await;
    assert_eq!(
        outcome,
        ChooseOutcome::Saved {
            attending: hours_of(&[1, 2, 3, 4, 5, 6])
        }
    );
    assert_eq!(
        answers_of(&store, &event, AKI).await,
        [
            (1, true),
            (2, true),
            (3, true),
            (4, true),
            (5, true),
            (6, true)
        ]
    );
}

#[tokio::test]
async fn an_empty_choice_marks_each_hour_out() {
    let board = FakeBoard::new();
    let (signups, event, store, _pool, _message) = six_hours_open(board.clone()).await;
    let outcome = signups
        .choose_hours(
            pick(&event, &[]),
            FIRST_POST_AT + 60,
            millis(FIRST_POST_AT + 60),
        )
        .await;
    assert_eq!(
        outcome,
        ChooseOutcome::Saved {
            attending: Vec::new()
        }
    );
    assert_eq!(
        answers_of(&store, &event, AKI).await,
        [
            (1, false),
            (2, false),
            (3, false),
            (4, false),
            (5, false),
            (6, false)
        ]
    );
    let drawn = board.edits().pop().expect("the post was redrawn");
    assert_eq!(drawn.rows.len(), 1);
    assert_eq!(drawn.rows[0].cells, [Cell::Out; 6]);
}

#[tokio::test]
async fn a_bad_choice_is_refused() {
    let board = FakeBoard::new();
    let (signups, event, store, _pool, _message) = six_hours_open(board.clone()).await;
    for values in [&["7"][..], &["x"], &["2", "2"]] {
        assert_eq!(
            signups
                .choose_hours(
                    pick(&event, values),
                    FIRST_POST_AT + 60,
                    millis(FIRST_POST_AT + 60)
                )
                .await,
            ChooseOutcome::BadChoice,
            "{values:?}"
        );
    }
    assert!(answers_of(&store, &event, AKI).await.is_empty());
    assert!(board.edits().is_empty());
}

#[tokio::test]
async fn a_late_or_stale_choice_is_refused() {
    let (signups, event, store, _pool, _message) = six_hours_open(FakeBoard::new()).await;
    assert_eq!(
        signups
            .choose_hours(pick(&event, &["2"]), FIRST_START, millis(FIRST_START))
            .await,
        ChooseOutcome::Closed {
            start_unix: FIRST_START
        }
    );
    assert!(answers_of(&store, &event, AKI).await.is_empty());

    let (signups, event, store, _pool, _message) = six_hours_open(FakeBoard::new()).await;
    store
        .end_event(GUILD, event.id, millis(FIRST_POST_AT))
        .await
        .unwrap();
    assert_eq!(
        signups
            .choose_hours(
                pick(&event, &["2"]),
                FIRST_POST_AT + 60,
                millis(FIRST_POST_AT + 60)
            )
            .await,
        ChooseOutcome::UnknownEvent
    );
    assert!(answers_of(&store, &event, AKI).await.is_empty());

    let (signups, event, store, pool, _message) = six_hours_open(FakeBoard::new()).await;
    sqlx::query("UPDATE cb_seasons SET first_day = '2026-09-17' WHERE id = ?")
        .bind(event.id)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        signups
            .choose_hours(
                pick(&event, &["2"]),
                FIRST_POST_AT + 60,
                millis(FIRST_POST_AT + 60)
            )
            .await,
        ChooseOutcome::UnknownEvent
    );
    assert!(answers_of(&store, &event, AKI).await.is_empty());
}

#[tokio::test]
async fn a_chosen_hours_submission_logs_hours_and_its_choice() {
    let (signups, event, _store, _pool, message) = six_hours_open(FakeBoard::new()).await;
    let logs = common::logs::capture();
    signups
        .choose_hours(
            pick(&event, &["2"]),
            FIRST_POST_AT + 60,
            millis(FIRST_POST_AT + 60),
        )
        .await;
    signups
        .choose_hours(
            pick(&event, &[]),
            FIRST_POST_AT + 120,
            millis(FIRST_POST_AT + 120),
        )
        .await;
    let logged = logs.named("signup.click.recorded");
    assert_eq!(logged.len(), 2, "{logged:?}");
    for (entry, choice) in logged.iter().zip(["attend", "nope"]) {
        assert_eq!(entry["barnacle.signup.target"], "hours");
        assert_eq!(entry["barnacle.signup.choice"], choice);
        assert_eq!(entry["discord.user.id"], AKI.get().to_string());
        assert_eq!(entry["barnacle.season.id"], event.id.to_string());
        assert_eq!(entry["barnacle.night"], "2026-09-16");
        assert_eq!(entry["discord.message.id"], message.get().to_string());
    }
}

#[tokio::test]
async fn the_hours_menu_opens_with_the_members_attending_hours() {
    let (signups, event, _store, _pool, _message) = six_hours_open(FakeBoard::new()).await;
    signups
        .choose_hours(
            pick(&event, &["2", "3"]),
            FIRST_POST_AT + 60,
            millis(FIRST_POST_AT + 60),
        )
        .await;
    let MenuOutcome::Open { view, attending } = signups
        .hours_menu(
            GUILD,
            event.id,
            first_date(),
            AKI,
            FIRST_POST_AT + 120,
            millis(FIRST_POST_AT + 120),
        )
        .await
    else {
        panic!("the hours menu should open");
    };
    assert_eq!(attending, hours_of(&[2, 3]));
    assert_eq!(view.event, event.id);
    assert_eq!(view.hours.len(), 6);
    assert_eq!(
        signups
            .hours_menu(
                GUILD,
                event.id,
                first_date(),
                AKI,
                FIRST_START,
                millis(FIRST_START)
            )
            .await,
        MenuOutcome::Closed {
            start_unix: FIRST_START
        }
    );
}

const SPRINT_POST_AT: i64 = 1_791_932_400;
const SPRINT_TODAY: &str = "2026-10-13";

fn sprint_weekends() -> NewEvent {
    NewEvent {
        ping_roles: vec![CREWMATES],
        ..named(
            "Ranked Sprint",
            "2026-10-14",
            "2026-10-17",
            schedule(
                Weekdays::new(&[Weekday::Wednesday, Weekday::Saturday]).unwrap(),
                "17:00",
                "America/Denver",
                4,
            ),
        )
    }
}

async fn sprint_posted(
    board: FakeBoard,
) -> (
    Arc<Signups<FakeBoard>>,
    Event,
    Attendance,
    Snowflake,
    Snowflake,
) {
    let (signups, event, store) = ready_from(board, &sprint_weekends()).await;
    signups.tick(SPRINT_POST_AT, millis(SPRINT_POST_AT)).await;
    let wednesday = message_for(&store, &event, day("2026-10-14")).await;
    let saturday = Snowflake::new(8);
    store
        .record_post(
            event.id,
            day("2026-10-17"),
            saturday,
            millis(SPRINT_POST_AT),
        )
        .await
        .unwrap();
    (signups, with_posts(&event), store, wednesday, saturday)
}

async fn sprint_edit(
    signups: &Arc<Signups<FakeBoard>>,
    event: &Event,
    change: &EventChange,
) -> EditReport {
    signups
        .edit_event(
            GUILD,
            event.id,
            event.created_at_ms,
            change,
            day(SPRINT_TODAY),
            SPRINT_POST_AT,
        )
        .await
}

#[tokio::test]
async fn an_edit_deletes_posts_the_schedule_no_longer_holds() {
    let board = FakeBoard::new();
    let (signups, event, store, wednesday, saturday) = sprint_posted(board.clone()).await;
    assert_eq!(board.pinging_sends().len(), 1);
    let change = EventChange {
        weekdays: Some(Weekdays::new(&[Weekday::Wednesday]).unwrap()),
        ..EventChange::default()
    };
    let EditReport::Edited {
        cleared, refreshed, ..
    } = sprint_edit(&signups, &event, &change).await
    else {
        panic!("the weekdays were not saved");
    };
    assert_eq!(
        cleared,
        PostsReport {
            touched: 1,
            failures: 0
        }
    );
    assert_eq!(
        refreshed,
        PostsReport {
            touched: 1,
            failures: 0
        }
    );
    assert_eq!(board.deletes(), vec![saturday]);
    assert_eq!(
        state_of(&store, &event, day("2026-10-17")).await,
        PostState::Removed
    );
    let redrawn: Vec<(Snowflake, Date)> = board
        .calls()
        .into_iter()
        .filter_map(|call| match call {
            BoardCall::Edited { message, view, .. } => Some((message, view.session.date())),
            _ => None,
        })
        .collect();
    assert_eq!(redrawn, vec![(wednesday, day("2026-10-14"))]);
    assert_eq!(board.sends().len(), 1);
    assert_eq!(board.pinging_sends().len(), 1);
}

#[tokio::test]
async fn an_edit_of_dates_or_zone_redraws_with_new_times() {
    let board = FakeBoard::new();
    let (signups, event, _store, wednesday, saturday) = sprint_posted(board.clone()).await;
    let eastern = EventChange {
        zone: Some(Zone::find("America/New_York").unwrap()),
        ..EventChange::default()
    };
    let EditReport::Edited { after, .. } = sprint_edit(&signups, &event, &eastern).await else {
        panic!("the time zone was not saved");
    };
    let wednesday_view = board
        .calls()
        .into_iter()
        .rev()
        .find_map(|call| match call {
            BoardCall::Edited { message, view, .. } if message == wednesday => Some(view),
            _ => None,
        })
        .expect("Wednesday's post was redrawn");
    assert_eq!(wednesday_view.session.start_unix(), 1_792_011_600);
    assert!(text::signup_description(&wednesday_view).contains("<t:1792011600:F>"));
    assert!(board.deletes().is_empty());
    let shorter = EventChange {
        last_day: Some(day("2026-10-14")),
        ..EventChange::default()
    };
    assert!(matches!(
        sprint_edit(&signups, &after, &shorter).await,
        EditReport::Edited { .. }
    ));
    assert_eq!(board.deletes(), vec![saturday]);
    assert_eq!(board.sends().len(), 1);
}

#[tokio::test]
async fn an_edit_of_name_or_roles_redraws_without_a_ping() {
    let board = FakeBoard::new();
    let (signups, event, store, wednesday, _saturday) = sprint_posted(board.clone()).await;
    let renamed = EventChange {
        name: Some("Ranked Finals".to_owned()),
        ..EventChange::default()
    };
    assert!(matches!(
        sprint_edit(&signups, &event, &renamed).await,
        EditReport::Edited { .. }
    ));
    let titled = board
        .calls()
        .into_iter()
        .rev()
        .find_map(|call| match call {
            BoardCall::Edited { message, view, .. } if message == wednesday => Some(view),
            _ => None,
        })
        .expect("Wednesday's post was redrawn");
    assert_eq!(titled.name, "Ranked Finals");
    let repinged = EventChange {
        ping_roles: Some(vec![RoleId::new(12)]),
        ..EventChange::default()
    };
    let EditReport::Edited { after, .. } = sprint_edit(&signups, &event, &repinged).await else {
        panic!("the roles were not saved");
    };
    assert_eq!(after.ping_roles, vec![RoleId::new(12)]);
    assert_eq!(
        store
            .live_event(GUILD, event.id)
            .await
            .unwrap()
            .unwrap()
            .ping_roles,
        vec![RoleId::new(12)]
    );
    let roled = board
        .calls()
        .into_iter()
        .rev()
        .find_map(|call| match call {
            BoardCall::Edited { message, view, .. } if message == wednesday => Some(view),
            _ => None,
        })
        .expect("Wednesday's post was redrawn");
    assert_eq!(roled.pings, vec![Ping::Role(RoleId::new(12))]);
    assert_eq!(board.sends().len(), 1);
    assert_eq!(board.pinging_sends().len(), 1);
}

#[tokio::test]
async fn moving_an_event_clears_the_old_channel_first() {
    let board = FakeBoard::new();
    let (signups, event, store) = ready_from(board.clone(), &sprint_weekends()).await;
    signups.tick(SPRINT_POST_AT, millis(SPRINT_POST_AT)).await;
    let old = message_for(&store, &event, day("2026-10-14")).await;
    let moved_out = EventChange {
        channel: Some(OTHER_CHANNEL),
        ..EventChange::default()
    };
    board.fail_deletes(true);
    assert_eq!(
        sprint_edit(&signups, &event, &moved_out).await,
        EditReport::ClearFailed { failures: 1 }
    );
    assert_eq!(
        store
            .live_event(GUILD, event.id)
            .await
            .unwrap()
            .unwrap()
            .channel,
        CHANNEL
    );
    assert_eq!(
        state_of(&store, &event, day("2026-10-14")).await,
        PostState::Open
    );
    board.fail_deletes(false);
    let EditReport::Edited { after, .. } = sprint_edit(&signups, &event, &moved_out).await else {
        panic!("the channel was not saved");
    };
    assert_eq!(after.channel, OTHER_CHANNEL);
    assert_eq!(board.deletes_in(CHANNEL), vec![old, old]);
    assert_eq!(
        state_of(&store, &event, day("2026-10-14")).await,
        PostState::Removed
    );
    assert_eq!(
        store
            .live_event(GUILD, event.id)
            .await
            .unwrap()
            .unwrap()
            .channel,
        OTHER_CHANNEL
    );
}

fn sprint_on(weekdays: &[Weekday], last: &str) -> NewEvent {
    named(
        "Ranked Sprint",
        "2026-10-14",
        last,
        schedule(
            Weekdays::new(weekdays).unwrap(),
            "17:00",
            "America/Denver",
            4,
        ),
    )
}

fn last_drawn(board: &FakeBoard, wanted: Snowflake) -> SignupView {
    board
        .calls()
        .into_iter()
        .rev()
        .find_map(|call| match call {
            BoardCall::Edited { message, view, .. } if message == wanted => Some(view),
            _ => None,
        })
        .expect("the post was redrawn")
}

#[tokio::test]
async fn a_reopened_session_closes_at_its_new_start() {
    let board = FakeBoard::new();
    let new = sprint_on(&[Weekday::Wednesday], "2026-10-14");
    let (signups, event, store) = ready_from(board.clone(), &new).await;
    signups.tick(1_791_932_400, millis(1_791_932_400)).await;
    signups.tick(1_792_018_800, millis(1_792_018_800)).await;
    let message = message_for(&store, &event, day("2026-10-14")).await;
    assert_eq!(
        state_of(&store, &event, day("2026-10-14")).await,
        PostState::Closed
    );
    let later = EventChange {
        start: Some(StartTime::parse("19:00").unwrap()),
        ..EventChange::default()
    };
    let EditReport::Edited { after, .. } = signups
        .edit_event(
            GUILD,
            event.id,
            event.created_at_ms,
            &later,
            day("2026-10-14"),
            1_792_020_600,
        )
        .await
    else {
        panic!("the start was not saved");
    };
    let reopened = last_drawn(&board, message);
    assert!(reopened.open);
    assert_eq!(reopened.session.start_unix(), 1_792_026_000);
    assert_eq!(
        state_of(&store, &after, day("2026-10-14")).await,
        PostState::Open
    );
    let closed = signups.tick(1_792_026_000, millis(1_792_026_000)).await;
    assert_eq!(closed.closed, vec![tag(&after, day("2026-10-14"))]);
    let redrawn = last_drawn(&board, message);
    assert!(!redrawn.open);
    assert_eq!(redrawn.session.start_unix(), 1_792_026_000);
    assert_eq!(
        state_of(&store, &after, day("2026-10-14")).await,
        PostState::Closed
    );
}

#[tokio::test]
async fn an_edit_closes_a_session_moved_before_now() {
    let board = FakeBoard::new();
    let new = sprint_on(&[Weekday::Wednesday, Weekday::Saturday], "2026-10-17");
    let (signups, event, store) = ready_from(board.clone(), &new).await;
    signups.tick(1_791_932_400, millis(1_791_932_400)).await;
    let message = message_for(&store, &event, day("2026-10-14")).await;
    assert_eq!(
        state_of(&store, &event, day("2026-10-14")).await,
        PostState::Open
    );
    let earlier = EventChange {
        start: Some(StartTime::parse("15:00").unwrap()),
        ..EventChange::default()
    };
    let EditReport::Edited { after, .. } = signups
        .edit_event(
            GUILD,
            event.id,
            event.created_at_ms,
            &earlier,
            day("2026-10-14"),
            1_792_015_200,
        )
        .await
    else {
        panic!("the start was not saved");
    };
    let redrawn = last_drawn(&board, message);
    assert!(!redrawn.open);
    assert_eq!(redrawn.session.start_unix(), 1_792_011_600);
    assert_eq!(
        state_of(&store, &after, day("2026-10-14")).await,
        PostState::Closed
    );
}

#[derive(Clone)]
struct HeldDeletes {
    board: FakeBoard,
    reached: Arc<Semaphore>,
    gate: Arc<Semaphore>,
}

impl HeldDeletes {
    fn new(board: FakeBoard) -> Self {
        Self {
            board,
            reached: Arc::new(Semaphore::new(0)),
            gate: Arc::new(Semaphore::new(0)),
        }
    }

    async fn reached(&self) {
        self.reached.acquire().await.unwrap().forget();
    }

    fn open(&self) {
        self.gate.close();
    }
}

impl Board for HeldDeletes {
    async fn send_post(
        &self,
        channel: ChannelId,
        view: &SignupView,
        delivery: Delivery,
    ) -> Result<Sent, BoardError> {
        self.board.send_post(channel, view, delivery).await
    }

    async fn find_post(
        &self,
        channel: ChannelId,
        tag: PostTag,
    ) -> Result<Option<Snowflake>, BoardError> {
        self.board.find_post(channel, tag).await
    }

    async fn edit_post(
        &self,
        channel: ChannelId,
        message: Snowflake,
        view: &SignupView,
    ) -> Result<(), BoardError> {
        self.board.edit_post(channel, message, view).await
    }

    async fn delete_post(
        &self,
        channel: ChannelId,
        message: Snowflake,
    ) -> Result<Removal, BoardError> {
        self.reached.add_permits(1);
        let _opened = self.gate.acquire().await;
        self.board.delete_post(channel, message).await
    }
}

async fn lands_within_a_second(landed: impl AsyncFn() -> bool) -> bool {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(1);
    while tokio::time::Instant::now() < deadline {
        if landed().await {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    landed().await
}

async fn store_of(events: &[NewEvent]) -> (Attendance, Vec<Event>) {
    let store = Attendance::with_pool(attendance_pool().await)
        .await
        .unwrap();
    let mut held = Vec::new();
    for new in events {
        held.push(created(&store, new).await);
    }
    (store, held)
}

fn saving(event: &Event, change: EventChange) -> Ready {
    Ready::Edit {
        event: event.id,
        created_at_ms: event.created_at_ms,
        change,
        warning: None,
    }
}

fn to_finals_in(channel: Option<ChannelId>) -> EventChange {
    EventChange {
        name: Some("Ranked Finals".to_owned()),
        channel,
        ..EventChange::default()
    }
}

async fn edit_report<B: Board>(
    ready: Ready,
    signups: &Arc<Signups<B>>,
    now_unix: i64,
) -> EditReport {
    match ready.apply(signups, GUILD, now_unix).await {
        Applied::Edit { report, .. } => report,
        other => panic!("expected an edit, got {other:?}"),
    }
}

#[tokio::test]
async fn a_beat_during_a_save_acts_on_the_saved_event() {
    let fake = FakeBoard::new();
    let board = HeldDeletes::new(fake.clone());
    let (store, held) = store_of(&[sprint_on(&[Weekday::Wednesday], "2026-10-14")]).await;
    let event = held[0].clone();
    let signups = Signups::new(board.clone(), store.clone());
    signups.tick(1_791_932_400, millis(1_791_932_400)).await;
    signups.tick(1_792_018_800, millis(1_792_018_800)).await;
    assert_eq!(
        state_of(&store, &event, day("2026-10-14")).await,
        PostState::Closed
    );
    let earlier = fake.calls().len();
    let save = {
        let signups = Arc::clone(&signups);
        let ready = saving(
            &event,
            EventChange {
                start: Some(StartTime::parse("19:00").unwrap()),
                channel: Some(OTHER_CHANNEL),
                ..EventChange::default()
            },
        );
        tokio::spawn(async move { edit_report(ready, &signups, 1_792_020_600).await })
    };
    board.reached().await;
    let beat = {
        let signups = Arc::clone(&signups);
        tokio::spawn(async move { signups.tick(1_792_020_600, millis(1_792_020_600)).await })
    };
    assert!(
        !lands_within_a_second(async || fake.sends().len() > 1).await,
        "a beat must wait for a Save in progress"
    );
    board.open();
    let EditReport::Edited { after, .. } = save.await.unwrap() else {
        panic!("the Save was not saved");
    };
    let beaten = beat.await.unwrap();
    assert_eq!(
        after.span.schedule().start(),
        StartTime::parse("19:00").unwrap()
    );
    assert_eq!(after.channel, OTHER_CHANNEL);
    let saved = store.live_event(GUILD, event.id).await.unwrap().unwrap();
    assert_eq!(
        saved.span.schedule().start(),
        StartTime::parse("19:00").unwrap()
    );
    assert_eq!(saved.channel, OTHER_CHANNEL);
    assert_eq!(beaten.posted, vec![tag(&saved, day("2026-10-14"))]);
    let sent: Vec<(ChannelId, Delivery, Date, bool, i64)> = fake.calls()[earlier..]
        .iter()
        .filter_map(|call| match call {
            BoardCall::Sent {
                channel,
                view,
                delivery,
            } => Some((
                *channel,
                *delivery,
                view.session.date(),
                view.open,
                view.session.start_unix(),
            )),
            _ => None,
        })
        .collect();
    assert_eq!(
        sent,
        vec![(
            OTHER_CHANNEL,
            Delivery::Redraw,
            day("2026-10-14"),
            true,
            1_792_026_000
        )]
    );
}

#[tokio::test]
async fn a_move_to_a_taken_name_deletes_nothing() {
    let board = FakeBoard::new();
    let (store, held) = store_of(&[
        sprint_on(&[Weekday::Wednesday], "2026-10-14"),
        NewEvent {
            name: "Ranked Finals".to_owned(),
            ..sprint_on(&[Weekday::Wednesday], "2026-10-14")
        },
    ])
    .await;
    let (sprint, finals) = (held[0].clone(), held[1].clone());
    let signups = Signups::new(board.clone(), store.clone());
    signups.tick(1_791_932_400, millis(1_791_932_400)).await;
    let ready = saving(&sprint, to_finals_in(Some(OTHER_CHANNEL)));
    assert_eq!(
        edit_report(ready, &signups, 1_791_932_460).await,
        EditReport::NameTaken
    );
    assert!(board.deletes().is_empty());
    assert_eq!(
        state_of(&store, &sprint, day("2026-10-14")).await,
        PostState::Open
    );
    assert_eq!(
        state_of(&store, &finals, day("2026-10-14")).await,
        PostState::Open
    );
    let kept = store.live_event(GUILD, sprint.id).await.unwrap().unwrap();
    assert_eq!(kept.name, "Ranked Sprint");
    assert_eq!(kept.channel, CHANNEL);
}

#[tokio::test]
async fn a_move_with_a_last_day_before_the_first_deletes_nothing() {
    let board = FakeBoard::new();
    let (store, held) = store_of(&[sprint_on(&[Weekday::Wednesday], "2026-10-21")]).await;
    let sprint = held[0].clone();
    let signups = Signups::new(board.clone(), store.clone());
    signups.tick(1_791_932_400, millis(1_791_932_400)).await;
    let ready = saving(
        &sprint,
        EventChange {
            last_day: Some(day("2026-10-13")),
            channel: Some(OTHER_CHANNEL),
            ..EventChange::default()
        },
    );
    assert_eq!(
        edit_report(ready, &signups, 1_791_932_460).await,
        EditReport::BadRange
    );
    assert!(board.deletes().is_empty());
    assert_eq!(
        state_of(&store, &sprint, day("2026-10-14")).await,
        PostState::Open
    );
    let kept = store.live_event(GUILD, sprint.id).await.unwrap().unwrap();
    assert_eq!(kept.span.last_day(), day("2026-10-21"));
    assert_eq!(kept.channel, CHANNEL);
}

#[tokio::test]
async fn a_move_with_dates_too_far_deletes_nothing() {
    let board = FakeBoard::new();
    let (store, held) = store_of(&[sprint_on(&[Weekday::Wednesday], "2026-10-21")]).await;
    let sprint = held[0].clone();
    let signups = Signups::new(board.clone(), store.clone());
    signups.tick(1_791_932_400, millis(1_791_932_400)).await;
    let ready = saving(
        &sprint,
        EventChange {
            last_day: Some(day("2027-04-15")),
            channel: Some(OTHER_CHANNEL),
            ..EventChange::default()
        },
    );
    assert_eq!(
        edit_report(ready, &signups, 1_791_932_460).await,
        EditReport::TooFar
    );
    assert!(board.deletes().is_empty());
    assert_eq!(
        state_of(&store, &sprint, day("2026-10-14")).await,
        PostState::Open
    );
    let kept = store.live_event(GUILD, sprint.id).await.unwrap().unwrap();
    assert_eq!(kept.span.last_day(), day("2026-10-21"));
    assert_eq!(kept.channel, CHANNEL);
}

#[tokio::test]
async fn a_namesake_created_during_a_move_waits_for_it() {
    let fake = FakeBoard::new();
    let board = HeldDeletes::new(fake.clone());
    let (store, held) = store_of(&[sprint_on(&[Weekday::Wednesday], "2026-10-14")]).await;
    let sprint = held[0].clone();
    let signups = Signups::new(board.clone(), store.clone());
    signups.tick(1_791_932_400, millis(1_791_932_400)).await;
    let old = message_for(&store, &sprint, day("2026-10-14")).await;
    let moving = {
        let signups = Arc::clone(&signups);
        let ready = saving(&sprint, to_finals_in(Some(OTHER_CHANNEL)));
        tokio::spawn(async move { edit_report(ready, &signups, 1_791_932_460).await })
    };
    board.reached().await;
    let creating = {
        let signups = Arc::clone(&signups);
        let ready = Ready::Create {
            new: NewEvent {
                name: "Ranked Finals".to_owned(),
                created_at_ms: millis(1_791_932_460),
                ..sprint_on(&[Weekday::Wednesday], "2026-10-14")
            },
            warning: None,
        };
        tokio::spawn(async move { ready.apply(&signups, GUILD, 1_791_932_460).await })
    };
    assert!(
        !lands_within_a_second(async || store.events_in(GUILD).await.unwrap().len() > 1).await,
        "a create must wait for a moving Save"
    );
    board.open();
    let EditReport::Edited { after, .. } = moving.await.unwrap() else {
        panic!("the moving Save was not saved");
    };
    assert_eq!(after.name, "Ranked Finals");
    assert_eq!(after.channel, OTHER_CHANNEL);
    let saved = store.live_event(GUILD, sprint.id).await.unwrap().unwrap();
    assert_eq!(saved.name, "Ranked Finals");
    assert_eq!(saved.channel, OTHER_CHANNEL);
    assert_eq!(fake.deletes_in(CHANNEL), vec![old]);
    let Applied::Create { outcome, .. } = creating.await.unwrap() else {
        panic!("the create was not applied as a create");
    };
    assert_eq!(outcome.unwrap(), CreateOutcome::NameTaken);
    assert_eq!(store.events_in(GUILD).await.unwrap().len(), 1);
}

#[tokio::test]
async fn a_rename_saved_during_a_move_waits_for_it() {
    let fake = FakeBoard::new();
    let board = HeldDeletes::new(fake.clone());
    let (store, held) = store_of(&[
        sprint_on(&[Weekday::Wednesday], "2026-10-14"),
        named(
            "Drill",
            "2026-10-17",
            "2026-10-17",
            schedule(
                Weekdays::new(&[Weekday::Saturday]).unwrap(),
                "17:00",
                "America/Denver",
                4,
            ),
        ),
    ])
    .await;
    let (sprint, drill) = (held[0].clone(), held[1].clone());
    let signups = Signups::new(board.clone(), store.clone());
    signups.tick(1_791_932_400, millis(1_791_932_400)).await;
    let moving = {
        let signups = Arc::clone(&signups);
        let ready = saving(&sprint, to_finals_in(Some(OTHER_CHANNEL)));
        tokio::spawn(async move { edit_report(ready, &signups, 1_791_932_460).await })
    };
    board.reached().await;
    let renaming = {
        let signups = Arc::clone(&signups);
        let ready = saving(&drill, to_finals_in(None));
        tokio::spawn(async move { edit_report(ready, &signups, 1_791_932_460).await })
    };
    assert!(
        !lands_within_a_second(async || {
            store.event(drill.id).await.unwrap().unwrap().name == "Ranked Finals"
        })
        .await,
        "another Save must wait for a moving Save"
    );
    board.open();
    let EditReport::Edited { after, .. } = moving.await.unwrap() else {
        panic!("the moving Save was not saved");
    };
    assert_eq!(after.name, "Ranked Finals");
    assert_eq!(after.channel, OTHER_CHANNEL);
    let saved = store.live_event(GUILD, sprint.id).await.unwrap().unwrap();
    assert_eq!(saved.name, "Ranked Finals");
    assert_eq!(saved.channel, OTHER_CHANNEL);
    assert_eq!(renaming.await.unwrap(), EditReport::NameTaken);
    assert_eq!(
        store
            .live_event(GUILD, drill.id)
            .await
            .unwrap()
            .unwrap()
            .name,
        "Drill"
    );
}
