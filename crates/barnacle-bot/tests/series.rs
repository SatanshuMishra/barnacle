mod common;

use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;

use barnacle_bot::discord::ending_mentions;
use barnacle_bot::discord::ending_message;
use barnacle_bot::discord::series_edit;
use barnacle_bot::ids::ChannelId;
use barnacle_bot::ids::Place;
use barnacle_bot::solves::SolveRecord;
use barnacle_bot::table::CancelOutcome;
use barnacle_bot::table::StartOutcome;
use barnacle_bot::table::Table;
use barnacle_bot::text;
use barnacle_catalog::Nation;
use barnacle_catalog::ShipClass;
use barnacle_guess::Guess;
use barnacle_guess::Reveal;
use barnacle_guess::RoundOptions;
use barnacle_guess::SeriesLength;
use barnacle_guess::Snowflake;
use barnacle_guess::Tally;
use barnacle_guess::UserId;
use common::at;
use common::fakes::FakeDiscord;
use common::fakes::FakeStore;
use common::fakes::OTHER_PLACE;
use common::fakes::PLACE;
use common::fakes::SeriesPosted;
use common::fakes::table_with;
use common::index;
use common::ship;
use common::tier;
use poise::serenity_prelude as serenity;
use tokio::time::Instant;

const STARTER: UserId = UserId::new(100);
const PLAYER: UserId = UserId::new(200);
const OTHER: UserId = UserId::new(300);
const NOBODY_YET: &str = "Nobody has won a round in this series yet.";
const NOBODY: &str = "Nobody won a round in this series.";

type TestTable = Arc<Table<FakeDiscord, FakeStore>>;

fn yamato_table(discord: FakeDiscord, store: FakeStore) -> TestTable {
    table_with(
        vec![ship("PJSB018", "Yamato", 10, "yamato")],
        discord,
        store,
    )
}

fn yamato() -> Reveal {
    Reveal {
        index: index("PJSB018"),
        name: "Yamato".to_owned(),
        tier: tier(10),
        nation: Nation::new("Japan"),
        class: ShipClass::Battleship,
    }
}

fn rounds(count: u32) -> SeriesLength {
    SeriesLength::new(count).unwrap()
}

fn millis(value: u64) -> Duration {
    Duration::from_millis(value)
}

fn seconds(value: u64) -> Duration {
    Duration::from_secs(value)
}

async fn until(origin: Instant, elapsed_millis: u64) {
    tokio::time::sleep_until(origin + millis(elapsed_millis)).await;
}

async fn begin(table: &TestTable, place: Place, length: u32) -> StartOutcome<Infallible> {
    table
        .start_series(
            place,
            RoundOptions::default(),
            rounds(length),
            STARTER,
            || async { Ok(at(0)) },
        )
        .await
}

async fn begun(table: &TestTable, place: Place, length: u32) -> u64 {
    match begin(table, place, length).await {
        StartOutcome::Started { number } => number,
        other => panic!("the series did not start: {other:?}"),
    }
}

async fn start(table: &TestTable, place: Place) -> StartOutcome<Infallible> {
    table
        .start(
            place,
            RoundOptions::default(),
            STARTER,
            |_draw, _number| async { Ok(at(1)) },
        )
        .await
}

fn guess(author: UserId, author_is_bot: bool, sent_at: u64, text: &str) -> Guess<'_> {
    Guess {
        author,
        author_is_bot,
        message: at(sent_at),
        text,
    }
}

fn round_posts(discord: &FakeDiscord, place: Place) -> Vec<(Duration, u32, u64)> {
    discord
        .series_posted()
        .into_iter()
        .filter_map(|(time, posted)| match posted {
            SeriesPosted::Round {
                channel,
                round,
                number,
                ..
            } if channel == place.channel => Some((time, round, number)),
            SeriesPosted::Round { .. }
            | SeriesPosted::Ending { .. }
            | SeriesPosted::Message { .. }
            | SeriesPosted::Edited { .. }
            | SeriesPosted::Deleted { .. } => None,
        })
        .collect()
}

fn round_number(discord: &FakeDiscord, place: Place, round: u32) -> u64 {
    round_posts(discord, place)
        .into_iter()
        .find(|(_, posted, _)| *posted == round)
        .map(|(_, _, number)| number)
        .unwrap()
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Ended {
    time: Duration,
    round_post: Snowflake,
    content: String,
    reply_to: Option<Snowflake>,
}

fn endings(discord: &FakeDiscord, place: Place) -> Vec<Ended> {
    discord
        .series_posted()
        .into_iter()
        .filter_map(|(time, posted)| match posted {
            SeriesPosted::Ending {
                channel,
                round_post,
                content,
                reply_to,
            } if channel == place.channel => Some(Ended {
                time,
                round_post,
                content,
                reply_to,
            }),
            SeriesPosted::Round { .. }
            | SeriesPosted::Ending { .. }
            | SeriesPosted::Message { .. }
            | SeriesPosted::Edited { .. }
            | SeriesPosted::Deleted { .. } => None,
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Sent {
    time: Duration,
    message: Snowflake,
    content: String,
    still_playing: Option<u64>,
}

fn messages(discord: &FakeDiscord, place: Place) -> Vec<Sent> {
    discord
        .series_posted()
        .into_iter()
        .filter_map(|(time, posted)| match posted {
            SeriesPosted::Message {
                channel,
                message,
                content,
                still_playing,
            } if channel == place.channel => Some(Sent {
                time,
                message,
                content,
                still_playing,
            }),
            SeriesPosted::Round { .. }
            | SeriesPosted::Ending { .. }
            | SeriesPosted::Message { .. }
            | SeriesPosted::Edited { .. }
            | SeriesPosted::Deleted { .. } => None,
        })
        .collect()
}

fn contents(sent: Vec<Sent>) -> Vec<String> {
    sent.into_iter().map(|sent| sent.content).collect()
}

fn deletions(discord: &FakeDiscord, place: Place) -> Vec<(usize, Snowflake)> {
    discord
        .series_posted()
        .into_iter()
        .enumerate()
        .filter_map(|(position, (_, posted))| match posted {
            SeriesPosted::Deleted { channel, message } if channel == place.channel => {
                Some((position, message))
            }
            SeriesPosted::Round { .. }
            | SeriesPosted::Ending { .. }
            | SeriesPosted::Message { .. }
            | SeriesPosted::Edited { .. }
            | SeriesPosted::Deleted { .. } => None,
        })
        .collect()
}

fn round_position(discord: &FakeDiscord, place: Place, wanted: u32) -> usize {
    discord
        .series_posted()
        .into_iter()
        .position(|(_, posted)| {
            matches!(
                posted,
                SeriesPosted::Round { channel, round, .. }
                    if channel == place.channel && round == wanted
            )
        })
        .unwrap()
}

fn edits_of(discord: &FakeDiscord, place: Place, wanted: Snowflake) -> Vec<(Duration, String)> {
    discord
        .series_posted()
        .into_iter()
        .filter_map(|(time, posted)| match posted {
            SeriesPosted::Edited {
                channel,
                message,
                content,
            } if channel == place.channel && message == wanted => Some((time, content)),
            SeriesPosted::Round { .. }
            | SeriesPosted::Ending { .. }
            | SeriesPosted::Message { .. }
            | SeriesPosted::Edited { .. }
            | SeriesPosted::Deleted { .. } => None,
        })
        .collect()
}

fn standings_without_winners(round: u32, length: u32) -> String {
    format!("### Standings after round {round} of {length}\n{NOBODY_YET}")
}

fn overview_without_winners(played: u32, length: u32) -> String {
    format!("## Series over\n**{played} of {length} rounds played**\n{NOBODY}")
}

fn json(value: impl serde::Serialize) -> serde_json::Value {
    serde_json::to_value(value).unwrap()
}

fn only(events: Vec<serde_json::Value>) -> serde_json::Value {
    assert_eq!(events.len(), 1, "{events:?}");
    events.into_iter().next().unwrap()
}

#[tokio::test(start_paused = true)]
async fn a_series_counts_down_five_seconds_before_its_first_round() {
    let discord = FakeDiscord::new();
    let table = yamato_table(discord.clone(), FakeStore::default());
    let origin = Instant::now();
    begun(&table, PLACE, 5).await;
    until(origin, 6_000).await;
    let settings = "## Silhouette series\n**5 rounds** | Tiers **VI-XI**\nStarted by <@100>";
    assert_eq!(
        edits_of(&discord, PLACE, at(0)),
        [
            (
                seconds(2),
                format!("{settings}\n### Round 1 of 5 starts in 3...")
            ),
            (
                seconds(3),
                format!("{settings}\n### Round 1 of 5 starts in 2...")
            ),
            (
                seconds(4),
                format!("{settings}\n### Round 1 of 5 starts in 1...")
            ),
            (seconds(5), settings.to_owned()),
        ]
    );
    let posted = discord.series_posted();
    assert_eq!(posted.len(), 5);
    assert!(matches!(
        &posted[3],
        (time, SeriesPosted::Round { channel, round: 1, length, ship, .. })
            if *time == seconds(5)
                && *channel == PLACE.channel
                && *length == rounds(5)
                && *ship == index("PJSB018")
    ));
}

#[tokio::test(start_paused = true)]
async fn a_won_round_ends_with_standings_and_a_countdown_to_the_next() {
    let discord = FakeDiscord::new();
    let table = yamato_table(discord.clone(), FakeStore::default());
    let origin = Instant::now();
    begun(&table, PLACE, 3).await;
    until(origin, 9_000).await;
    table
        .hear(PLACE, guess(PLAYER, false, 9_000, "yamato"))
        .await;
    until(origin, 15_000).await;
    let standings = |line: &str| {
        format!(
            "### Standings after round 1 of 3\n\
             **Most rounds won**\n\
             **1.** <@200> · 1 win\n\
             **Fastest time**\n\
             **1.** <@200> · 4.000 s\n\
             {line}"
        )
    };
    assert_eq!(
        endings(&discord, PLACE),
        [Ended {
            time: millis(9_250),
            round_post: at(5_000),
            content: "Correct: **Yamato**, tier X battleship, Japan. Solved in 4.000 s. New personal best."
                .to_owned(),
            reply_to: Some(at(9_000)),
        }]
    );
    let sent = messages(&discord, PLACE);
    assert_eq!(
        sent.iter()
            .map(|sent| (sent.time, sent.content.clone(), sent.still_playing))
            .collect::<Vec<_>>(),
        [(
            millis(9_250),
            standings("### Round 2 of 3 starts in 5 seconds"),
            None
        )]
    );
    assert_eq!(
        edits_of(&discord, PLACE, sent[0].message),
        [
            (millis(11_250), standings("### Round 2 of 3 starts in 3...")),
            (millis(12_250), standings("### Round 2 of 3 starts in 2...")),
            (millis(13_250), standings("### Round 2 of 3 starts in 1...")),
        ]
    );
    let posts: Vec<(Duration, u32)> = round_posts(&discord, PLACE)
        .into_iter()
        .map(|(time, round, _)| (time, round))
        .collect();
    assert_eq!(posts, [(seconds(5), 1), (millis(14_250), 2)]);
}

#[tokio::test(start_paused = true)]
async fn the_last_round_ends_with_the_overview_and_frees_the_channel() {
    let logs = common::logs::capture();
    let discord = FakeDiscord::new();
    let table = yamato_table(discord.clone(), FakeStore::default());
    let origin = Instant::now();
    let series = begun(&table, PLACE, 2).await;
    until(origin, 43_000).await;
    table
        .hear(PLACE, guess(PLAYER, false, 43_000, "yamato"))
        .await;
    until(origin, 120_000).await;
    let ended = endings(&discord, PLACE);
    assert_eq!(ended.len(), 2);
    assert_eq!(
        ended[1],
        Ended {
            time: millis(43_250),
            round_post: at(40_000),
            content: text::win(&yamato(), seconds(3), Some(true)),
            reply_to: Some(at(43_000)),
        }
    );
    let last = messages(&discord, PLACE).pop().unwrap();
    assert_eq!(
        (last.time, last.content.as_str(), last.still_playing),
        (
            millis(43_250),
            text::series_overview(&Tally::default().won(PLAYER, seconds(3)), 2, rounds(2)).as_str(),
            None
        )
    );
    assert_eq!(
        last.content,
        "## Series over\n**2 of 2 rounds played**\n### Most rounds won\n**1.** <@200> · 1 win\n### Fastest time\n**1.** <@200> · 3.000 s"
    );
    assert_eq!(
        discord.series_posted().last().map(|(time, _)| *time),
        Some(millis(43_250))
    );
    assert_eq!(round_posts(&discord, PLACE).len(), 2);
    assert!(matches!(
        start(&table, PLACE).await,
        StartOutcome::Started { .. }
    ));
    let started = only(logs.named("guess.series.started"));
    assert_eq!(started["barnacle.round.number"], series);
    assert_eq!(
        started["discord.channel.id"],
        PLACE.channel.get().to_string()
    );
    let ended = only(logs.named("guess.series.ended"));
    assert_eq!(ended["message"], "a silhouette series completed");
    assert_eq!(ended["event.outcome"], "success");
    assert_eq!(ended["barnacle.round.number"], series);
}

#[tokio::test(start_paused = true)]
async fn every_won_series_round_is_saved_like_a_single_round() {
    let store = FakeStore::default();
    let table = yamato_table(FakeDiscord::new(), store.clone());
    let origin = Instant::now();
    begun(&table, PLACE, 2).await;
    until(origin, 7_000).await;
    table
        .hear(PLACE, guess(PLAYER, false, 7_000, "yamato"))
        .await;
    until(origin, 15_250).await;
    table
        .hear(PLACE, guess(OTHER, false, 15_250, "Yamato"))
        .await;
    until(origin, 30_000).await;
    assert_eq!(
        store.saved(),
        [
            SolveRecord {
                guild: PLACE.guild,
                user: PLAYER,
                ship: index("PJSB018"),
                elapsed: millis(2_000),
                solved_at_ms: at(7_000).unix_millis(),
            },
            SolveRecord {
                guild: PLACE.guild,
                user: OTHER,
                ship: index("PJSB018"),
                elapsed: millis(3_000),
                solved_at_ms: at(15_250).unix_millis(),
            },
        ]
    );
}

#[tokio::test(start_paused = true)]
async fn skip_ends_only_the_current_round() {
    let discord = FakeDiscord::new();
    let table = yamato_table(discord.clone(), FakeStore::default());
    let origin = Instant::now();
    begun(&table, PLACE, 3).await;
    until(origin, 6_000).await;
    let first = round_number(&discord, PLACE, 1);
    assert_eq!(
        table.skip(PLACE, first, OTHER, false).await,
        CancelOutcome::Refused
    );
    assert!(endings(&discord, PLACE).is_empty());
    assert_eq!(
        table.skip(PLACE, first, STARTER, false).await,
        CancelOutcome::Cancelled
    );
    assert_eq!(
        endings(&discord, PLACE),
        [Ended {
            time: seconds(6),
            round_post: at(5_000),
            content: "<@100> skipped this round. It was **Yamato**, tier X battleship, Japan."
                .to_owned(),
            reply_to: None,
        }]
    );
    assert_eq!(
        contents(messages(&discord, PLACE)),
        [text::series_message(&[
            &standings_without_winners(1, 3),
            "### Round 2 of 3 starts in 5 seconds",
        ])]
    );
    until(origin, 12_000).await;
    let second = round_number(&discord, PLACE, 2);
    assert_eq!(
        table.skip(PLACE, first, STARTER, false).await,
        CancelOutcome::AlreadyOver
    );
    assert_eq!(
        table.skip(PLACE, second, OTHER, true).await,
        CancelOutcome::Cancelled
    );
    until(origin, 18_000).await;
    let third = round_number(&discord, PLACE, 3);
    assert_eq!(
        table.skip(PLACE, third, STARTER, false).await,
        CancelOutcome::Cancelled
    );
    until(origin, 60_000).await;
    let posts: Vec<(Duration, u32)> = round_posts(&discord, PLACE)
        .into_iter()
        .map(|(time, round, _)| (time, round))
        .collect();
    assert_eq!(posts, [(seconds(5), 1), (seconds(11), 2), (seconds(17), 3)]);
    let ended = endings(&discord, PLACE);
    assert_eq!(ended.len(), 3);
    assert_eq!(ended[2].content, text::skipped(STARTER, &yamato()));
    assert_eq!(
        contents(messages(&discord, PLACE)).last(),
        Some(&overview_without_winners(3, 3))
    );
}

#[tokio::test(start_paused = true)]
async fn end_series_stops_the_series_and_posts_the_overview() {
    let logs = common::logs::capture();
    let discord = FakeDiscord::new();
    let table = yamato_table(discord.clone(), FakeStore::default());
    let origin = Instant::now();
    begun(&table, PLACE, 5).await;
    begun(&table, OTHER_PLACE, 5).await;
    until(origin, 6_000).await;
    let running = round_number(&discord, PLACE, 1);
    assert_eq!(
        table.end_series(PLACE, running, OTHER, false).await,
        CancelOutcome::Refused
    );
    assert!(endings(&discord, PLACE).is_empty());
    assert_eq!(
        table.end_series(PLACE, running, STARTER, false).await,
        CancelOutcome::Cancelled
    );
    assert_eq!(
        table
            .end_series(
                OTHER_PLACE,
                round_number(&discord, OTHER_PLACE, 1),
                OTHER,
                true
            )
            .await,
        CancelOutcome::Cancelled
    );
    until(origin, 120_000).await;
    assert_eq!(
        endings(&discord, PLACE),
        [Ended {
            time: seconds(6),
            round_post: at(5_000),
            content: "<@100> ended the series. It was **Yamato**, tier X battleship, Japan."
                .to_owned(),
            reply_to: None,
        }]
    );
    assert_eq!(
        contents(messages(&discord, PLACE)),
        [overview_without_winners(1, 5)]
    );
    assert_eq!(
        endings(&discord, OTHER_PLACE)
            .into_iter()
            .map(|ended| ended.content)
            .collect::<Vec<String>>(),
        [text::series_ended(OTHER, &yamato())]
    );
    assert_eq!(
        contents(messages(&discord, OTHER_PLACE)),
        [overview_without_winners(1, 5)]
    );
    assert_eq!(round_posts(&discord, PLACE).len(), 1);
    assert_eq!(round_posts(&discord, OTHER_PLACE).len(), 1);
    assert_eq!(
        discord.series_posted().last().map(|(time, _)| *time),
        Some(seconds(6))
    );
    assert!(matches!(
        start(&table, PLACE).await,
        StartOutcome::Started { .. }
    ));
    let summaries: Vec<String> = logs
        .named("guess.series.ended")
        .iter()
        .map(|event| event["message"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(
        summaries,
        [
            "a silhouette series was ended by a player",
            "a silhouette series was ended by a player"
        ]
    );
}

#[tokio::test(start_paused = true)]
async fn three_silent_rounds_ask_and_silence_ends_the_series() {
    let logs = common::logs::capture();
    let discord = FakeDiscord::new();
    let table = yamato_table(discord.clone(), FakeStore::default());
    let origin = Instant::now();
    let series = begun(&table, PLACE, 10).await;
    until(origin, 106_000).await;
    let ended = endings(&discord, PLACE);
    let sent = messages(&discord, PLACE);
    assert_eq!(
        sent.iter()
            .map(|sent| (sent.time, sent.still_playing))
            .collect::<Vec<_>>(),
        [
            (seconds(35), None),
            (seconds(70), None),
            (seconds(105), Some(series))
        ]
    );
    assert_eq!(ended[2].content, text::timed_out(&yamato()));
    assert_eq!(
        sent[2].content,
        text::series_message(&[&standings_without_winners(3, 10), text::SERIES_CHECK])
    );
    assert!(!sent[2].content.contains("starts in"));
    until(origin, 300_000).await;
    assert_eq!(
        edits_of(&discord, PLACE, sent[2].message),
        [(
            seconds(115),
            text::series_message(&[
                "### Nobody answered, so the series ended.",
                &overview_without_winners(3, 10),
            ])
        )]
    );
    assert_eq!(round_posts(&discord, PLACE).len(), 3);
    assert_eq!(
        table.still_playing(PLACE, series).await,
        CancelOutcome::AlreadyOver
    );
    assert!(matches!(
        start(&table, PLACE).await,
        StartOutcome::Started { .. }
    ));
    assert_eq!(
        only(logs.named("guess.series.ended"))["message"],
        "a silhouette series ended after nobody answered"
    );
}

#[tokio::test(start_paused = true)]
async fn yes_keeps_the_series_going_and_restarts_the_silent_count() {
    let discord = FakeDiscord::new();
    let table = yamato_table(discord.clone(), FakeStore::default());
    let origin = Instant::now();
    let series = begun(&table, PLACE, 10).await;
    until(origin, 107_000).await;
    assert_eq!(
        table.still_playing(PLACE, series + 1).await,
        CancelOutcome::AlreadyOver
    );
    until(origin, 109_000).await;
    assert_eq!(
        table.still_playing(PLACE, series).await,
        CancelOutcome::Cancelled
    );
    assert_eq!(
        table.still_playing(PLACE, series).await,
        CancelOutcome::AlreadyOver
    );
    until(origin, 250_000).await;
    let countdown = |remaining: u32| {
        text::series_message(&[
            &standings_without_winners(3, 10),
            &text::series_countdown(4, rounds(10), remaining),
        ])
    };
    let sent = messages(&discord, PLACE);
    assert_eq!(
        edits_of(&discord, PLACE, sent[2].message),
        [
            (seconds(109), countdown(5)),
            (seconds(111), countdown(3)),
            (seconds(112), countdown(2)),
            (seconds(113), countdown(1)),
        ]
    );
    let posts: Vec<(Duration, u32)> = round_posts(&discord, PLACE)
        .into_iter()
        .map(|(time, round, _)| (time, round))
        .collect();
    assert_eq!(
        posts,
        [
            (seconds(5), 1),
            (seconds(40), 2),
            (seconds(75), 3),
            (seconds(114), 4),
            (seconds(149), 5),
            (seconds(184), 6),
        ]
    );
    assert_eq!(
        sent.into_iter()
            .map(|sent| sent.still_playing)
            .collect::<Vec<_>>(),
        [None, None, Some(series), None, None, Some(series)]
    );
}

#[tokio::test(start_paused = true)]
async fn a_wrong_answer_keeps_a_round_from_being_silent() {
    let discord = FakeDiscord::new();
    let table = yamato_table(discord.clone(), FakeStore::default());
    let origin = Instant::now();
    begun(&table, PLACE, 10).await;
    let bots_only = begun(&table, OTHER_PLACE, 10).await;
    until(origin, 80_000).await;
    table
        .hear(PLACE, guess(PLAYER, false, 80_000, "musashi"))
        .await;
    table
        .hear(OTHER_PLACE, guess(PLAYER, true, 80_000, "musashi"))
        .await;
    until(origin, 106_000).await;
    let heard = messages(&discord, PLACE);
    assert_eq!(heard.len(), 3);
    assert_eq!(heard[2].still_playing, None);
    assert!(
        heard[2]
            .content
            .ends_with("### Round 4 of 10 starts in 5 seconds"),
        "{}",
        heard[2].content
    );
    assert_eq!(
        messages(&discord, OTHER_PLACE)
            .into_iter()
            .map(|sent| sent.still_playing)
            .collect::<Vec<_>>(),
        [None, None, Some(bots_only)]
    );
    until(origin, 115_000).await;
    assert_eq!(round_posts(&discord, PLACE).len(), 4);
}

#[tokio::test(start_paused = true)]
async fn the_last_round_never_asks_the_check() {
    let discord = FakeDiscord::new();
    let table = yamato_table(discord.clone(), FakeStore::default());
    let origin = Instant::now();
    begun(&table, PLACE, 3).await;
    until(origin, 200_000).await;
    let ended = endings(&discord, PLACE);
    assert_eq!(ended.len(), 3);
    assert_eq!(
        ended[2],
        Ended {
            time: seconds(105),
            round_post: at(75_000),
            content: text::timed_out(&yamato()),
            reply_to: None,
        }
    );
    let last = messages(&discord, PLACE).pop().unwrap();
    assert_eq!(
        (last.time, last.content.as_str(), last.still_playing),
        (seconds(105), overview_without_winners(3, 3).as_str(), None)
    );
    assert!(edits_of(&discord, PLACE, last.message).is_empty());
    assert_eq!(round_posts(&discord, PLACE).len(), 3);
}

#[tokio::test(start_paused = true)]
async fn a_running_series_keeps_its_channel_busy() {
    let discord = FakeDiscord::new();
    let table = yamato_table(discord.clone(), FakeStore::default());
    let origin = Instant::now();
    let series = begun(&table, PLACE, 10).await;
    for moment in [1_000, 10_000, 37_000, 108_000] {
        until(origin, moment).await;
        assert!(
            matches!(start(&table, PLACE).await, StartOutcome::Busy),
            "{moment}"
        );
        assert!(
            matches!(begin(&table, PLACE, 5).await, StartOutcome::Busy),
            "{moment}"
        );
    }
    assert_eq!(
        messages(&discord, PLACE)
            .last()
            .and_then(|sent| sent.still_playing),
        Some(series)
    );
    let third = Place {
        guild: PLACE.guild,
        channel: ChannelId::new(12),
    };
    assert!(matches!(
        begin(&table, OTHER_PLACE, 5).await,
        StartOutcome::Started { .. }
    ));
    assert!(matches!(
        start(&table, third).await,
        StartOutcome::Started { .. }
    ));
}

#[tokio::test(start_paused = true)]
async fn a_failed_round_post_ends_the_series() {
    let logs = common::logs::capture();
    let failing = FakeDiscord::failing();
    let table = yamato_table(failing.clone(), FakeStore::default());
    let origin = Instant::now();
    begun(&table, PLACE, 5).await;
    until(origin, 5_001).await;
    assert!(matches!(
        start(&table, PLACE).await,
        StartOutcome::Started { .. }
    ));
    until(origin, 120_000).await;
    let attempts: Vec<Duration> = failing
        .series_posted()
        .into_iter()
        .map(|(time, _)| time)
        .collect();
    assert_eq!(attempts, [seconds(2), seconds(3), seconds(4), seconds(5)]);
    assert_eq!(round_posts(&failing, PLACE).len(), 1);

    let hanging = FakeDiscord::hanging();
    let table = yamato_table(hanging.clone(), FakeStore::default());
    let origin = Instant::now();
    begun(&table, PLACE, 5).await;
    until(origin, 6_000).await;
    assert!(matches!(
        start(&table, PLACE).await,
        StartOutcome::Started { .. }
    ));
    assert_eq!(origin.elapsed(), seconds(10));
    until(origin, 120_000).await;
    assert_eq!(
        hanging
            .series_posted()
            .into_iter()
            .map(|(time, _)| time)
            .collect::<Vec<Duration>>(),
        [seconds(2), seconds(3), seconds(4), seconds(5)]
    );

    let failed: Vec<String> = logs
        .named("guess.post.failed")
        .iter()
        .map(|event| event["message"].as_str().unwrap().to_owned())
        .collect();
    assert!(failed.contains(&"a series round could not be posted".to_owned()));
    assert!(failed.contains(&"posting a series round timed out".to_owned()));
    let ended: Vec<String> = logs
        .named("guess.series.ended")
        .iter()
        .map(|event| event["message"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(
        ended,
        [
            "a silhouette series ended after a round could not be posted",
            "a silhouette series ended after a round could not be posted"
        ]
    );
}

#[tokio::test(start_paused = true)]
async fn a_series_pings_only_the_winner() {
    assert_eq!(
        ending_mentions(true),
        serenity::CreateAllowedMentions::new().replied_user(true)
    );
    assert_eq!(
        ending_mentions(false),
        serenity::CreateAllowedMentions::new()
    );
    let channel = serenity::ChannelId::new(PLACE.channel.get());
    let standings = "**1.** <@200> · 1 win\n**2.** <@300> · 1 win";
    let winner_reply = json(ending_message(channel, standings, Some(at(8_000))));
    assert_eq!(
        winner_reply["allowed_mentions"],
        json(serenity::CreateAllowedMentions::new().replied_user(true))
    );
    assert_eq!(
        winner_reply["message_reference"]["message_id"],
        at(8_000).get().to_string()
    );
    let unanswered = json(ending_message(channel, standings, None));
    assert_eq!(
        unanswered["allowed_mentions"],
        json(serenity::CreateAllowedMentions::new())
    );
    assert!(unanswered["message_reference"].is_null());
    let edited = json(series_edit(standings));
    assert_eq!(
        edited["allowed_mentions"],
        json(serenity::CreateAllowedMentions::new())
    );
    assert_eq!(edited["components"], serde_json::json!([]));
    let discord = FakeDiscord::new();
    let table = yamato_table(discord.clone(), FakeStore::default());
    let origin = Instant::now();
    begun(&table, PLACE, 3).await;
    until(origin, 8_000).await;
    table
        .hear(PLACE, guess(PLAYER, false, 8_000, "yamato"))
        .await;
    until(origin, 50_000).await;
    assert_eq!(
        endings(&discord, PLACE)
            .into_iter()
            .map(|ended| ended.reply_to)
            .collect::<Vec<_>>(),
        [Some(at(8_000)), None]
    );
}

#[tokio::test(start_paused = true)]
async fn a_rounds_kept_result_carries_no_standings_or_countdown() {
    let discord = FakeDiscord::new();
    let table = yamato_table(discord.clone(), FakeStore::default());
    let origin = Instant::now();
    begun(&table, PLACE, 3).await;
    until(origin, 36_000).await;
    let first_ending = discord
        .series_posted()
        .into_iter()
        .find_map(|(_, posted)| match posted {
            SeriesPosted::Ending { content, .. } => Some(content),
            _ => None,
        })
        .expect("round 1 ended");
    assert_eq!(first_ending, text::timed_out(&yamato()));
}

#[tokio::test(start_paused = true)]
async fn a_rounds_standings_are_deleted_once_the_next_round_appears() {
    let discord = FakeDiscord::new();
    let table = yamato_table(discord.clone(), FakeStore::default());
    let origin = Instant::now();
    begun(&table, PLACE, 3).await;
    until(origin, 120_000).await;
    let standings: Vec<Sent> = messages(&discord, PLACE)
        .into_iter()
        .filter(|sent| sent.content.contains("Standings after round 1 of 3"))
        .collect();
    assert_eq!(standings.len(), 1, "{standings:?}");
    let deleted: Vec<usize> = deletions(&discord, PLACE)
        .into_iter()
        .filter(|(_, message)| *message == standings[0].message)
        .map(|(position, _)| position)
        .collect();
    assert_eq!(deleted.len(), 1);
    assert!(deleted[0] > round_position(&discord, PLACE, 2));
}

#[tokio::test(start_paused = true)]
async fn the_opening_keeps_its_header_once_round_one_appears() {
    let discord = FakeDiscord::new();
    let table = yamato_table(discord.clone(), FakeStore::default());
    let origin = Instant::now();
    begun(&table, PLACE, 3).await;
    until(origin, 120_000).await;
    assert_eq!(round_posts(&discord, PLACE)[0].0, seconds(5));
    assert_eq!(
        edits_of(&discord, PLACE, at(0)).pop(),
        Some((
            seconds(5),
            text::series_header(&RoundOptions::default(), rounds(3), STARTER)
        ))
    );
}

#[tokio::test(start_paused = true)]
async fn a_check_answered_yes_is_deleted_after_the_next_round() {
    let discord = FakeDiscord::new();
    let table = yamato_table(discord.clone(), FakeStore::default());
    let origin = Instant::now();
    let series = begun(&table, PLACE, 5).await;
    until(origin, 106_000).await;
    let check = messages(&discord, PLACE).pop().unwrap();
    assert_eq!(
        (check.time, check.content.as_str(), check.still_playing),
        (
            seconds(105),
            text::series_message(&[&standings_without_winners(3, 5), text::SERIES_CHECK]).as_str(),
            Some(series)
        )
    );
    until(origin, 109_000).await;
    assert_eq!(
        table.still_playing(PLACE, series).await,
        CancelOutcome::Cancelled
    );
    until(origin, 200_000).await;
    let countdown = |remaining: u32| {
        text::series_message(&[
            &standings_without_winners(3, 5),
            &text::series_countdown(4, rounds(5), remaining),
        ])
    };
    assert_eq!(
        edits_of(&discord, PLACE, check.message),
        [
            (seconds(109), countdown(5)),
            (seconds(111), countdown(3)),
            (seconds(112), countdown(2)),
            (seconds(113), countdown(1)),
        ]
    );
    let deleted: Vec<usize> = deletions(&discord, PLACE)
        .into_iter()
        .filter(|(_, message)| *message == check.message)
        .map(|(position, _)| position)
        .collect();
    assert_eq!(deleted.len(), 1);
    assert!(deleted[0] > round_position(&discord, PLACE, 4));
}

#[tokio::test(start_paused = true)]
async fn an_expired_check_stays_as_the_final_overview() {
    let discord = FakeDiscord::new();
    let table = yamato_table(discord.clone(), FakeStore::default());
    let origin = Instant::now();
    let series = begun(&table, PLACE, 5).await;
    until(origin, 106_000).await;
    let check = messages(&discord, PLACE).pop().unwrap();
    assert_eq!(check.still_playing, Some(series));
    until(origin, 300_000).await;
    assert_eq!(
        edits_of(&discord, PLACE, check.message),
        [(
            seconds(115),
            text::series_message(&[text::SERIES_EXPIRED, &overview_without_winners(3, 5)])
        )]
    );
    assert_eq!(round_posts(&discord, PLACE).len(), 3);
    assert!(
        deletions(&discord, PLACE)
            .iter()
            .all(|(_, message)| *message != check.message)
    );
}

#[tokio::test(start_paused = true)]
async fn the_last_round_keeps_its_result_and_a_final_overview() {
    let discord = FakeDiscord::new();
    let table = yamato_table(discord.clone(), FakeStore::default());
    let origin = Instant::now();
    begun(&table, PLACE, 2).await;
    until(origin, 120_000).await;
    let ended = endings(&discord, PLACE);
    assert_eq!(ended.len(), 2);
    assert_eq!(
        (ended[1].time, ended[1].content.as_str()),
        (seconds(70), text::timed_out(&yamato()).as_str())
    );
    let sent = messages(&discord, PLACE);
    assert_eq!(
        sent.iter()
            .map(|sent| (sent.time, sent.content.clone()))
            .collect::<Vec<_>>(),
        [
            (
                seconds(35),
                text::series_message(&[
                    &standings_without_winners(1, 2),
                    &text::series_countdown(2, rounds(2), 5),
                ])
            ),
            (
                seconds(70),
                text::series_overview(&Tally::default(), 2, rounds(2))
            ),
        ]
    );
    let deleted: Vec<Snowflake> = deletions(&discord, PLACE)
        .into_iter()
        .map(|(_, message)| message)
        .collect();
    assert_eq!(deleted, [sent[0].message]);
}
