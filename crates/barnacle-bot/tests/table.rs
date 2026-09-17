mod common;

use std::collections::BTreeSet;
use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;

use barnacle_bot::ids::Place;
use barnacle_bot::table::CancelOutcome;
use barnacle_bot::table::Ending;
use barnacle_bot::table::StartOutcome;
use barnacle_bot::table::Table;
use barnacle_guess::Guess;
use barnacle_guess::Hint;
use barnacle_guess::RoundOptions;
use barnacle_guess::UserId;
use common::at;
use common::fakes::FakeDiscord;
use common::fakes::FakeStore;
use common::fakes::OTHER_PLACE;
use common::fakes::PLACE;
use common::fakes::Posted;
use common::fakes::table_with;
use common::fleet;
use common::index;
use common::ship;
use common::tier;

const INVOKER: UserId = UserId::new(100);
const PLAYER: UserId = UserId::new(200);
const OTHER: UserId = UserId::new(300);
const POSTED_AT: u64 = 1_000;

type TestTable = Arc<Table<FakeDiscord, FakeStore>>;

fn yamato_table(discord: FakeDiscord, store: FakeStore) -> TestTable {
    table_with(
        vec![ship("PJSB018", "Yamato", 10, "yamato")],
        discord,
        store,
    )
}

async fn start(table: &TestTable, place: Place) -> StartOutcome<Infallible> {
    table
        .start(
            place,
            RoundOptions::default(),
            INVOKER,
            |_draw, _number| async { Ok(at(POSTED_AT)) },
        )
        .await
}

async fn started(table: &TestTable, place: Place) -> u64 {
    match start(table, place).await {
        StartOutcome::Started { number } => number,
        other => panic!("the round did not start: {other:?}"),
    }
}

fn guess(author: UserId, author_is_bot: bool, sent_at: u64, text: &str) -> Guess<'_> {
    Guess {
        author,
        author_is_bot,
        message: at(sent_at),
        text,
    }
}

fn seconds(value: u64) -> Duration {
    Duration::from_secs(value)
}

async fn wait(value: u64) {
    tokio::time::sleep(seconds(value)).await;
}

#[tokio::test(start_paused = true)]
async fn an_unanswered_round_posts_the_hint_at_twenty_seconds_and_ends_at_thirty() {
    let discord = FakeDiscord::new();
    let table = yamato_table(discord.clone(), FakeStore::default());
    started(&table, PLACE).await;
    wait(60).await;
    let posted = discord.posted();
    assert_eq!(posted.len(), 2);
    assert_eq!(
        posted[0],
        (
            seconds(20),
            Posted::Hint {
                channel: PLACE.channel,
                hint: Hint::Tier(tier(10)),
            }
        )
    );
    assert_eq!(posted[1].0, seconds(30));
    assert!(matches!(
        &posted[1].1,
        Posted::Ending {
            round_post,
            ending: Ending::TimedOut { reveal },
            ..
        } if *round_post == at(POSTED_AT) && reveal.index == index("PJSB018")
    ));
}

#[tokio::test(start_paused = true)]
async fn a_win_before_the_hint_ends_the_round_and_nothing_else_is_posted() {
    let discord = FakeDiscord::new();
    let table = yamato_table(discord.clone(), FakeStore::default());
    started(&table, PLACE).await;
    wait(5).await;
    table
        .hear(PLACE, guess(PLAYER, false, POSTED_AT + 5_250, "Yamato"))
        .await;
    wait(60).await;
    let posted = discord.posted();
    assert_eq!(posted.len(), 1);
    assert_eq!(posted[0].0, seconds(5));
    assert!(matches!(
        &posted[0].1,
        Posted::Ending {
            ending: Ending::Solved { solve, message, personal_best: Some(true), .. },
            ..
        } if solve.winner == PLAYER
            && solve.elapsed == Duration::from_millis(5_250)
            && *message == at(POSTED_AT + 5_250)
    ));
}

#[tokio::test(start_paused = true)]
async fn a_win_after_the_hint_follows_the_hint() {
    let discord = FakeDiscord::new();
    let table = yamato_table(discord.clone(), FakeStore::default());
    started(&table, PLACE).await;
    wait(25).await;
    table
        .hear(PLACE, guess(PLAYER, false, POSTED_AT + 25_000, "yamato"))
        .await;
    wait(60).await;
    let posted = discord.posted();
    assert_eq!(posted.len(), 2);
    assert!(matches!(posted[0].1, Posted::Hint { .. }));
    assert_eq!(posted[1].0, seconds(25));
    assert!(matches!(
        posted[1].1,
        Posted::Ending {
            ending: Ending::Solved { .. },
            ..
        }
    ));
}

#[tokio::test(start_paused = true)]
async fn a_busy_channel_refuses_a_second_round_until_the_first_ends() {
    let table = yamato_table(FakeDiscord::new(), FakeStore::default());
    started(&table, PLACE).await;
    assert!(matches!(start(&table, PLACE).await, StartOutcome::Busy));
    table
        .hear(PLACE, guess(PLAYER, false, POSTED_AT + 1, "yamato"))
        .await;
    assert!(matches!(
        start(&table, PLACE).await,
        StartOutcome::Started { .. }
    ));
}

#[tokio::test(start_paused = true)]
async fn the_starter_may_cancel() {
    let discord = FakeDiscord::new();
    let table = yamato_table(discord.clone(), FakeStore::default());
    let number = started(&table, PLACE).await;
    assert_eq!(
        table.cancel(PLACE, number, INVOKER, false).await,
        CancelOutcome::Cancelled
    );
    assert!(matches!(
        discord.endings().as_slice(),
        [Ending::Cancelled { by, .. }] if *by == INVOKER
    ));
}

#[tokio::test(start_paused = true)]
async fn a_moderator_may_cancel_but_another_player_may_not() {
    let discord = FakeDiscord::new();
    let table = yamato_table(discord.clone(), FakeStore::default());
    let number = started(&table, PLACE).await;
    assert_eq!(
        table.cancel(PLACE, number, OTHER, false).await,
        CancelOutcome::Refused
    );
    assert!(discord.endings().is_empty());
    assert_eq!(
        table.cancel(PLACE, number, OTHER, true).await,
        CancelOutcome::Cancelled
    );
    assert!(matches!(
        discord.endings().as_slice(),
        [Ending::Cancelled { by, .. }] if *by == OTHER
    ));
}

#[tokio::test(start_paused = true)]
async fn a_click_on_a_round_that_is_not_running_is_already_over() {
    let table = yamato_table(FakeDiscord::new(), FakeStore::default());
    assert_eq!(
        table.cancel(PLACE, 1, INVOKER, true).await,
        CancelOutcome::AlreadyOver
    );
    let number = started(&table, PLACE).await;
    assert_eq!(
        table.cancel(PLACE, number + 1, INVOKER, true).await,
        CancelOutcome::AlreadyOver
    );
    assert_eq!(
        table.cancel(PLACE, number, INVOKER, false).await,
        CancelOutcome::Cancelled
    );
    assert_eq!(
        table.cancel(PLACE, number, INVOKER, false).await,
        CancelOutcome::AlreadyOver
    );
}

#[tokio::test(start_paused = true)]
async fn bots_early_messages_and_wrong_guesses_do_not_end_the_round() {
    let discord = FakeDiscord::new();
    let table = yamato_table(discord.clone(), FakeStore::default());
    started(&table, PLACE).await;
    table
        .hear(PLACE, guess(PLAYER, true, POSTED_AT + 1, "yamato"))
        .await;
    table
        .hear(PLACE, guess(PLAYER, false, POSTED_AT - 1, "yamato"))
        .await;
    table
        .hear(PLACE, guess(PLAYER, false, POSTED_AT + 2, "musashi"))
        .await;
    wait(60).await;
    assert!(matches!(
        discord.endings().as_slice(),
        [Ending::TimedOut { .. }]
    ));
}

#[tokio::test(start_paused = true)]
async fn two_channels_run_their_rounds_independently() {
    let discord = FakeDiscord::new();
    let table = yamato_table(discord.clone(), FakeStore::default());
    started(&table, PLACE).await;
    started(&table, OTHER_PLACE).await;
    table
        .hear(PLACE, guess(PLAYER, false, POSTED_AT + 1, "yamato"))
        .await;
    wait(60).await;
    let for_channel = |place: Place| {
        discord
            .posted()
            .into_iter()
            .filter(move |(_, posted)| match posted {
                Posted::Hint { channel, .. } | Posted::Ending { channel, .. } => {
                    *channel == place.channel
                }
            })
            .count()
    };
    assert_eq!(for_channel(PLACE), 1);
    assert_eq!(for_channel(OTHER_PLACE), 2);
}

#[tokio::test(start_paused = true)]
async fn a_win_and_a_cancel_at_the_same_moment_end_the_round_once() {
    let discord = FakeDiscord::new();
    let table = yamato_table(discord.clone(), FakeStore::default());
    let number = started(&table, PLACE).await;
    let (_, cancelled) = tokio::join!(
        table.hear(PLACE, guess(PLAYER, false, POSTED_AT + 1, "yamato")),
        table.cancel(PLACE, number, INVOKER, false)
    );
    wait(60).await;
    assert_eq!(discord.endings().len(), 1);
    assert_eq!(
        cancelled == CancelOutcome::Cancelled,
        matches!(discord.endings()[0], Ending::Cancelled { .. })
    );
}

#[tokio::test(start_paused = true)]
async fn a_failed_round_post_leaves_the_channel_free() {
    let discord = FakeDiscord::new();
    let table = yamato_table(discord.clone(), FakeStore::default());
    let failed = table
        .start(
            PLACE,
            RoundOptions::default(),
            INVOKER,
            |_draw, _number| async { Err::<barnacle_guess::Snowflake, &str>("upload failed") },
        )
        .await;
    assert!(matches!(failed, StartOutcome::PostFailed("upload failed")));
    wait(60).await;
    assert!(discord.posted().is_empty());
    assert!(matches!(
        start(&table, PLACE).await,
        StartOutcome::Started { .. }
    ));
}

#[tokio::test(start_paused = true)]
async fn an_empty_pool_starts_nothing() {
    let table = yamato_table(FakeDiscord::new(), FakeStore::default());
    let options = RoundOptions::new(Some(tier(1)), Some(tier(2)), None);
    let outcome = table
        .start(PLACE, options, INVOKER, |_draw, _number| async {
            Ok::<_, Infallible>(at(POSTED_AT))
        })
        .await;
    assert!(matches!(outcome, StartOutcome::NoShips(_)));
    assert!(matches!(
        start(&table, PLACE).await,
        StartOutcome::Started { .. }
    ));
}

#[tokio::test(start_paused = true)]
async fn a_channel_does_not_repeat_its_recent_ships() {
    let table = table_with(fleet(21), FakeDiscord::new(), FakeStore::default());
    let drawn = std::sync::Mutex::new(Vec::new());
    for _ in 0..21 {
        let outcome = table
            .start(PLACE, RoundOptions::default(), INVOKER, |draw, _number| {
                drawn.lock().unwrap().push(draw.ship().clone());
                async { Ok::<_, Infallible>(at(POSTED_AT)) }
            })
            .await;
        let StartOutcome::Started { number } = outcome else {
            panic!("the round did not start");
        };
        table.cancel(PLACE, number, INVOKER, false).await;
    }
    let drawn = drawn.into_inner().unwrap();
    assert_eq!(drawn.iter().collect::<BTreeSet<_>>().len(), 21);
}

#[tokio::test(start_paused = true)]
async fn a_win_is_recorded_with_its_server_player_ship_and_time() {
    let store = FakeStore::default();
    let table = yamato_table(FakeDiscord::new(), store.clone());
    started(&table, PLACE).await;
    table
        .hear(PLACE, guess(PLAYER, false, POSTED_AT + 4_000, "yamato"))
        .await;
    assert_eq!(
        store.saved(),
        [barnacle_bot::solves::SolveRecord {
            guild: PLACE.guild,
            user: PLAYER,
            ship: index("PJSB018"),
            elapsed: Duration::from_millis(4_000),
            solved_at_ms: at(POSTED_AT + 4_000).unix_millis(),
        }]
    );
}

#[tokio::test(start_paused = true)]
async fn a_failing_discord_does_not_stop_rounds_from_ending_and_being_recorded() {
    let store = FakeStore::default();
    let table = yamato_table(FakeDiscord::failing(), store.clone());
    started(&table, PLACE).await;
    table
        .hear(PLACE, guess(PLAYER, false, POSTED_AT + 1, "yamato"))
        .await;
    assert!(matches!(
        start(&table, PLACE).await,
        StartOutcome::Started { .. }
    ));
    assert_eq!(store.saved().len(), 1);
}

#[tokio::test(start_paused = true)]
async fn a_storage_failure_still_announces_the_win_without_a_best() {
    let discord = FakeDiscord::new();
    let table = yamato_table(discord.clone(), FakeStore::failing());
    started(&table, PLACE).await;
    table
        .hear(PLACE, guess(PLAYER, false, POSTED_AT + 1, "yamato"))
        .await;
    assert!(matches!(
        discord.endings().as_slice(),
        [Ending::Solved {
            personal_best: None,
            ..
        }]
    ));
}

#[tokio::test(start_paused = true)]
async fn an_earlier_rounds_timer_does_not_hint_at_the_next_round() {
    let discord = FakeDiscord::new();
    let table = yamato_table(discord.clone(), FakeStore::default());
    let first = started(&table, PLACE).await;
    wait(5).await;
    table.cancel(PLACE, first, INVOKER, false).await;
    wait(5).await;
    started(&table, PLACE).await;
    wait(60).await;
    let times: Vec<(Duration, bool)> = discord
        .posted()
        .into_iter()
        .map(|(time, posted)| (time, matches!(posted, Posted::Hint { .. })))
        .collect();
    assert_eq!(
        times,
        [
            (seconds(5), false),
            (seconds(30), true),
            (seconds(40), false)
        ]
    );
}

#[tokio::test(start_paused = true)]
async fn an_earlier_rounds_timer_does_not_end_the_next_round() {
    let discord = FakeDiscord::new();
    let table = yamato_table(discord.clone(), FakeStore::default());
    let first = started(&table, PLACE).await;
    wait(25).await;
    table.cancel(PLACE, first, INVOKER, false).await;
    wait(1).await;
    started(&table, PLACE).await;
    wait(9).await;
    assert_eq!(discord.endings().len(), 1);
    wait(60).await;
    let endings: Vec<Duration> = discord
        .posted()
        .into_iter()
        .filter(|(_, posted)| matches!(posted, Posted::Ending { .. }))
        .map(|(time, _)| time)
        .collect();
    assert_eq!(endings, [seconds(25), seconds(56)]);
}
