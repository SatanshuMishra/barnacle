mod common;

use std::collections::BTreeSet;
use std::time::Duration;

use barnacle_bot::attendance::Cell;
use barnacle_bot::attendance::Delivery;
use barnacle_bot::attendance::HourTally;
use barnacle_bot::attendance::PostTag;
use barnacle_bot::attendance::PurgeReport;
use barnacle_bot::attendance::RosterRow;
use barnacle_bot::attendance::SignupView;
use barnacle_bot::attendance::TickReport;
use barnacle_bot::attendance_store::Event;
use barnacle_bot::discord;
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
use barnacle_bot::solves::Ranking;
use barnacle_bot::solves::Standing;
use barnacle_bot::text;
use barnacle_bot::text::Refusal;
use barnacle_bot::wiring::PingVerdict;
use barnacle_catalog::Nation;
use barnacle_catalog::ShipClass;
use barnacle_guess::Hint;
use barnacle_guess::Reveal;
use barnacle_guess::RoundOptions;
use barnacle_guess::SeriesLength;
use barnacle_guess::Tally;
use barnacle_guess::Timing;
use barnacle_guess::UserId;
use common::every_refusal;
use common::index;
use common::tier;
use poise::serenity_prelude as serenity;

fn yamato() -> Reveal {
    Reveal {
        index: index("PJSB018"),
        name: "Yamato".to_owned(),
        tier: tier(10),
        nation: Nation::new("Japan"),
        class: ShipClass::Battleship,
    }
}

fn warspite() -> Reveal {
    Reveal {
        index: index("PBSB105"),
        name: "Warspite".to_owned(),
        tier: tier(6),
        nation: Nation::new("United_Kingdom"),
        class: ShipClass::Battleship,
    }
}

#[test]
fn tiers_are_roman_numerals() {
    let numerals: Vec<&str> = (1..=11)
        .map(|value| text::tier_numeral(tier(value)))
        .collect();
    assert_eq!(
        numerals,
        [
            "I", "II", "III", "IV", "V", "VI", "VII", "VIII", "IX", "X", "XI"
        ]
    );
    assert_eq!(text::tier_range(&RoundOptions::default()), "VI-XI");
    assert_eq!(
        text::tier_range(&RoundOptions::new(Some(tier(8)), Some(tier(8)), None)),
        "VIII"
    );
}

#[test]
fn nations_get_barnacles_labels() {
    let labels: Vec<String> = [
        "USA",
        "United_Kingdom",
        "Russia",
        "Pan_Asia",
        "Pan_America",
        "Events",
        "Japan",
        "Commonwealth",
        "Some_New_Nation",
    ]
    .into_iter()
    .map(|nation| text::nation_label(&Nation::new(nation)))
    .collect();
    assert_eq!(
        labels,
        [
            "U.S.A.",
            "U.K.",
            "U.S.S.R.",
            "Pan-Asia",
            "Pan-America",
            "Event",
            "Japan",
            "Commonwealth",
            "Some New Nation"
        ]
    );
}

#[test]
fn classes_get_plain_labels() {
    let labels: Vec<String> = [
        ShipClass::Destroyer,
        ShipClass::Cruiser,
        ShipClass::Battleship,
        ShipClass::AircraftCarrier,
        ShipClass::Submarine,
        ShipClass::Other("Auxiliary".to_owned()),
        ShipClass::Unspecified,
    ]
    .iter()
    .map(text::class_label)
    .collect();
    assert_eq!(
        labels,
        [
            "destroyer",
            "cruiser",
            "battleship",
            "aircraft carrier",
            "submarine",
            "Auxiliary",
            "unknown class"
        ]
    );
}

#[test]
fn times_show_three_decimals() {
    assert_eq!(text::seconds(Duration::from_millis(3_251)), "3.251 s");
    assert_eq!(text::seconds(Duration::from_millis(60_005)), "60.005 s");
    assert_eq!(text::best_time(None), "No rounds won here yet.");
    assert_eq!(text::best_time(Some(Duration::from_millis(900))), "0.900 s");
}

#[test]
fn hints_never_end_with_two_full_stops() {
    assert_eq!(text::hint(&Hint::Tier(tier(8))), "Hint: it's tier VIII.");
    assert_eq!(
        text::hint(&Hint::Nation(Nation::new("Japan"))),
        "Hint: it's from Japan."
    );
    assert_eq!(
        text::hint(&Hint::Nation(Nation::new("United_Kingdom"))),
        "Hint: it's from U.K."
    );
}

#[test]
fn round_endings_describe_the_ship() {
    assert_eq!(
        text::win(&yamato(), Duration::from_millis(3_251), Some(false)),
        "Correct: **Yamato**, tier X battleship, Japan. Solved in 3.251 s."
    );
    assert_eq!(
        text::win(&yamato(), Duration::from_millis(3_251), Some(true)),
        "Correct: **Yamato**, tier X battleship, Japan. Solved in 3.251 s. New personal best."
    );
    assert_eq!(
        text::win(&warspite(), Duration::from_millis(12_000), None),
        "Correct: **Warspite**, tier VI battleship, U.K. Solved in 12.000 s."
    );
    assert_eq!(
        text::timed_out(&yamato()),
        "Nobody named it. It was **Yamato**, tier X battleship, Japan."
    );
    assert_eq!(
        text::cancelled(UserId::new(42), &warspite()),
        "<@42> ended the round. It was **Warspite**, tier VI battleship, U.K."
    );
}

#[test]
fn ship_names_cannot_inject_formatting() {
    assert_eq!(text::escape("*Hood*_[x]"), "\\*Hood\\*\\_\\[x]");
}

#[test]
fn an_empty_pool_names_the_options_and_how_to_widen_them() {
    assert_eq!(
        Refusal::EmptyPool(RoundOptions::new(Some(tier(6)), Some(tier(8)), None)).message(),
        "No ships fit tiers VI-VIII. Try a wider range with min_tier and max_tier."
    );
    assert_eq!(
        Refusal::EmptyPool(RoundOptions::new(Some(tier(6)), Some(tier(8)), Some(true))).message(),
        "No ships fit tiers VI-VIII with paper ships excluded. Try a wider range with min_tier and max_tier, or leave historical off."
    );
    assert_eq!(
        Refusal::EmptyPool(RoundOptions::new(Some(tier(2)), Some(tier(2)), None)).message(),
        "No ships fit tier II. Try a wider range with min_tier and max_tier."
    );
}

#[test]
fn the_footer_follows_the_timing() {
    assert_eq!(text::round_footer(Timing::STANDARD), "Hint in 20 seconds");
}

#[test]
fn long_lists_are_cut_to_the_limit() {
    let names: Vec<String> = ["alpha", "bravo", "charlie", "delta"]
        .map(str::to_owned)
        .to_vec();
    assert_eq!(text::listing(&[], 100), "None");
    assert_eq!(text::listing(&names, 100), "alpha, bravo, charlie, delta");
    assert_eq!(text::listing(&names, 25), "alpha, bravo and 2 more");
    assert_eq!(text::listing(&names, 7), "4 more");
    assert!(text::listing(&names, 25).chars().count() <= 25);
}

#[test]
fn about_names_the_data_and_carries_the_wargaming_notice() {
    let about = text::about(
        &common::catalog(Vec::new()).provenance,
        "15.8.0_13187581_r4",
        "0.1.0",
    );
    assert!(about.contains("World of Warships 15.8.0 (build 13187581), catalog 15.8.0_13187581_r4, data commit 442496e."));
    assert!(about.contains("wowsunpack 0.45.0 and wows-data-mgr 0.21.0"));
    assert!(about.contains("Barnacle 0.1.0"));
    assert!(about.ends_with(text::WARGAMING_NOTICE));
}

fn variant(refusal: &Refusal) -> usize {
    match refusal {
        Refusal::WrongChannelType => 0,
        Refusal::MissingBotPermissions { .. } => 1,
        Refusal::DateFormat => 2,
        Refusal::LastDayBeforeFirst => 3,
        Refusal::DatesTooFar => 4,
        Refusal::NoSessionsLeft => 5,
        Refusal::NameTaken(_) => 6,
        Refusal::NameFormat => 7,
        Refusal::EventNotFound => 8,
        Refusal::NothingToChange => 9,
        Refusal::RehearsalOnly => 10,
        Refusal::RehearsalRunning => 11,
        Refusal::NothingPending => 12,
        Refusal::RoundAlreadyRunning => 13,
        Refusal::EmptyPool(_) => 14,
        Refusal::NoShipMatches => 15,
        Refusal::NameEmpty => 16,
        Refusal::NameTooLong { .. } => 17,
        Refusal::NotAHub => 18,
        Refusal::CategoryAndTopLevel => 19,
        Refusal::HubNothingToChange => 20,
        Refusal::PingBlockedByChannel { .. } => 21,
        Refusal::PingMixesEveryone => 22,
        Refusal::NothingToRepost { .. } => 23,
        Refusal::RepostNothingToPing => 24,
        Refusal::RepostPingNotAllowed => 25,
        Refusal::RepostPingsChanged => 26,
        Refusal::RepostSuperseded => 27,
        Refusal::SeriesRoundsOutOfRange => 28,
        Refusal::TemplateUnknown => 29,
        Refusal::MissingOptions(_) => 30,
        Refusal::StartFormat => 31,
        Refusal::HoursOutOfRange => 32,
        Refusal::ZoneUnknown => 33,
        Refusal::WeekdaysInvalid => 34,
        Refusal::TooManyRoles => 35,
        Refusal::PanelUnknown => 36,
        Refusal::PanelNotYours => 37,
        Refusal::PanelExpired => 38,
        Refusal::ChannelUnknown => 39,
    }
}

const REFUSAL_VARIANTS: usize = 40;

#[test]
fn every_refusal_code_is_unique_and_snake_case() {
    let refusals = every_refusal();
    assert_eq!(
        refusals.iter().map(variant).collect::<BTreeSet<usize>>(),
        (0..REFUSAL_VARIANTS).collect::<BTreeSet<usize>>(),
        "every_refusal must build each variant once"
    );
    let codes: Vec<&str> = refusals.iter().map(Refusal::code).collect();
    assert_eq!(
        codes.iter().collect::<BTreeSet<_>>().len(),
        codes.len(),
        "{codes:?}"
    );
    for code in &codes {
        assert!(
            !code.is_empty()
                && code
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte == b'_')
                && !code.starts_with('_')
                && !code.ends_with('_')
                && !code.contains("__"),
            "{code} is not snake_case"
        );
    }
    assert_eq!(
        Refusal::MissingBotPermissions {
            names: vec!["Embed Links"],
            channel: None
        }
        .code(),
        "missing_bot_permissions"
    );
    assert_eq!(Refusal::DateFormat.code(), "date_format");
    assert_eq!(Refusal::EventNotFound.code(), "event_not_found");
    assert_eq!(Refusal::NoSessionsLeft.code(), "no_sessions_left");
    assert_eq!(
        Refusal::NameTaken("Ranked Sprint".to_owned()).code(),
        "name_taken"
    );
}

#[test]
fn missing_bot_permissions_are_named_with_who_can_grant_them() {
    assert_eq!(
        Refusal::MissingBotPermissions {
            names: vec!["Embed Links"],
            channel: None
        }
        .message(),
        "Barnacle needs Embed Links in this channel. A server admin can grant it to Barnacle's role or in this channel's permissions, then run the command again."
    );
    assert_eq!(
        Refusal::MissingBotPermissions {
            names: vec!["View Channel", "Send Messages", "Embed Links"],
            channel: None
        }
        .message(),
        "Barnacle needs View Channel, Send Messages and Embed Links in this channel. A server admin can grant them to Barnacle's role or in this channel's permissions, then run the command again."
    );
    assert_eq!(
        Refusal::MissingBotPermissions {
            names: vec!["Send Messages", "Read Message History"],
            channel: Some(ChannelId::new(123))
        }
        .message(),
        "Barnacle needs Send Messages and Read Message History in <#123>. A server admin can grant them to Barnacle's role or in that channel's permissions, then run the command again."
    );
}

#[test]
fn every_refusal_says_what_to_do_next() {
    assert_eq!(
        Refusal::WrongChannelType.message(),
        "Sign-up posts go in a text channel. Run this in a text channel."
    );
    assert_eq!(
        Refusal::DateFormat.message(),
        "Dates look like 2026-09-16. Run the command again with the dates written that way."
    );
    assert_eq!(
        Refusal::LastDayBeforeFirst.message(),
        "The last day is before the first day. Give a last day on or after the first day."
    );
    assert_eq!(
        Refusal::DatesTooFar.message(),
        "An event has to sit within six months either side of today. Pick dates closer to today."
    );
    assert_eq!(
        Refusal::NoSessionsLeft.message(),
        "Those dates hold no session still ahead. Pick dates with a session still ahead."
    );
    assert_eq!(
        Refusal::NothingToChange.message(),
        "Nothing to change. Give a new name, dates, start, hours, time zone or channel, or change the weekdays or pings in the panel."
    );
    assert_eq!(
        Refusal::NameFormat.message(),
        "An event's name is 1 to 100 characters. Run the command again with a name that long."
    );
    assert_eq!(
        Refusal::RehearsalOnly.message(),
        "This server is not set up for rehearsals. Whoever runs Barnacle can list it under [rehearsal] in barnacle.toml, then restart Barnacle."
    );
    assert_eq!(
        Refusal::NothingPending.message(),
        "Nothing is waiting to happen in this rehearsal. /rehearse start sets up another rehearsal."
    );
    assert_eq!(
        Refusal::RoundAlreadyRunning.message(),
        "This channel already has a round running. Name the ship or wait for the round to end, then start another."
    );
    assert_eq!(
        Refusal::NoShipMatches.message(),
        "No ship matches that. Pick one of the names suggested as you type."
    );
    assert_eq!(
        Refusal::NameEmpty.message(),
        "A name needs at least one character that is not a space. Run the command again with a name."
    );
    assert_eq!(
        Refusal::NameTooLong { limit: 90 }.message(),
        "That name is too long; the limit is 90 characters. Give a shorter one."
    );
    assert_eq!(
        Refusal::NotAHub.message(),
        "That channel is not a Join to Create channel in this server. `/voice hub list` shows the ones that are."
    );
    assert_eq!(
        Refusal::CategoryAndTopLevel.message(),
        "Pick a category or top_level, not both."
    );
    assert_eq!(
        Refusal::HubNothingToChange.message(),
        "Nothing to change. Give a new name, room_name, category or top_level."
    );
    assert_eq!(
        Refusal::SeriesRoundsOutOfRange.message(),
        "A series is 2 to 20 rounds."
    );
    assert!(
        every_refusal()
            .iter()
            .all(|refusal| !refusal.message().contains("Reference")),
        "a refusal of bad input carries no reference"
    );
}

fn rounds(count: u32) -> SeriesLength {
    SeriesLength::new(count).unwrap()
}

fn aki_and_bo() -> Tally {
    Tally::default()
        .won(UserId::new(1), Duration::from_millis(3_100))
        .won(UserId::new(2), Duration::from_millis(2_500))
        .won(UserId::new(1), Duration::from_millis(4_000))
}

#[test]
fn series_standings_list_wins_then_fastest_times() {
    assert_eq!(
        text::series_standings(&aki_and_bo(), 3, rounds(10)),
        "### Standings after round 3 of 10\n\
         **Most rounds won**\n\
         **1.** <@1> · 2 wins\n\
         **2.** <@2> · 1 win\n\
         **Fastest time**\n\
         **1.** <@2> · 2.500 s\n\
         **2.** <@1> · 3.100 s"
    );
    assert_eq!(
        text::series_standings(&Tally::default(), 3, rounds(10)),
        "### Standings after round 3 of 10\nNobody has won a round in this series yet."
    );
}

#[test]
fn a_full_series_ending_with_twenty_players_fits_one_message() {
    let tally = (0..20).fold(Tally::default(), |tally, player| {
        tally.won(
            UserId::new(1_000_000_000_000_000_000 + player),
            Duration::from_millis(29_999 - player),
        )
    });
    let long_ship = Reveal {
        index: index("PASA997"),
        name: "Almirante Almirante Almirante Almirante Almirante Almirante".to_owned(),
        tier: tier(11),
        nation: Nation::new("Pan_America"),
        class: ShipClass::AircraftCarrier,
    };
    let result = text::win(&long_ship, Duration::from_millis(29_999), Some(true));
    let checked = text::series_message(&[
        &text::series_standings(&tally, 20, rounds(20)),
        text::SERIES_CHECK,
    ]);
    let expired = text::series_message(&[
        text::SERIES_EXPIRED,
        &text::series_overview(&tally, 20, rounds(20)),
    ]);
    for message in [&checked, &expired] {
        assert_eq!(message.matches("<@").count(), 40, "{message}");
    }
    for message in [result, checked, expired] {
        assert!(
            message.encode_utf16().count() < text::MESSAGE_CONTENT_LIMIT,
            "{} characters",
            message.encode_utf16().count()
        );
    }
}

#[test]
fn series_countdown_check_and_overview_read_as_specified() {
    let header = "## Silhouette series\n**10 rounds** | Tiers **VI-XI**\nStarted by <@100>";
    assert_eq!(
        text::series_intro(&RoundOptions::default(), rounds(10), UserId::new(100), 5),
        format!("{header}\n### Round 1 of 10 starts in 5 seconds")
    );
    assert_eq!(
        text::series_intro(&RoundOptions::default(), rounds(10), UserId::new(100), 3),
        format!("{header}\n### Round 1 of 10 starts in 3...")
    );
    assert_eq!(
        text::series_intro(
            &RoundOptions::new(Some(tier(8)), Some(tier(8)), Some(true)),
            rounds(2),
            UserId::new(100),
            5
        ),
        "## Silhouette series\n\
         **2 rounds** | Tier **VIII**\n\
         Ships that were never built are left out.\n\
         Started by <@100>\n\
         ### Round 1 of 2 starts in 5 seconds"
    );
    assert_eq!(
        [5, 3, 2, 1].map(|remaining| text::series_countdown(2, rounds(10), remaining)),
        [
            "### Round 2 of 10 starts in 5 seconds",
            "### Round 2 of 10 starts in 3...",
            "### Round 2 of 10 starts in 2...",
            "### Round 2 of 10 starts in 1...",
        ]
    );
    assert_eq!(text::series_round_label(4, rounds(10)), "Round 4 of 10");
    assert_eq!(text::SERIES_FIELD, "Series");
    assert_eq!(
        text::SERIES_CHECK,
        "### Are you still playing?\nPress **Yes** within 10 seconds to keep the series going."
    );
    assert_eq!(
        text::SERIES_EXPIRED,
        "### Nobody answered, so the series ended."
    );
    assert_eq!(
        text::skipped(UserId::new(42), &warspite()),
        "<@42> skipped this round. It was **Warspite**, tier VI battleship, U.K."
    );
    assert_eq!(
        text::series_ended(UserId::new(42), &warspite()),
        "<@42> ended the series. It was **Warspite**, tier VI battleship, U.K."
    );
    assert_eq!(
        text::series_overview(&Tally::default(), 4, rounds(10)),
        "## Series over\n**4 of 10 rounds played**\nNobody won a round in this series."
    );
    assert_eq!(
        text::series_overview(&aki_and_bo(), 10, rounds(10)),
        "## Series over\n\
         **10 of 10 rounds played**\n\
         ### Most rounds won\n\
         **1.** <@1> · 2 wins\n\
         **2.** <@2> · 1 win\n\
         ### Fastest time\n\
         **1.** <@2> · 2.500 s\n\
         **2.** <@1> · 3.100 s"
    );
    assert_eq!(
        text::series_message(&[&text::timed_out(&yamato()), "", text::SERIES_CHECK]),
        "Nobody named it. It was **Yamato**, tier X battleship, Japan.\n### Are you still playing?\nPress **Yes** within 10 seconds to keep the series going."
    );
}

#[test]
fn series_messages_read_as_styled() {
    let starter = UserId::new(100);
    assert_eq!(
        text::series_header(&RoundOptions::default(), rounds(5), starter),
        "## Silhouette series\n**5 rounds** | Tiers **VI-XI**\nStarted by <@100>"
    );
    assert_eq!(
        text::series_header(
            &RoundOptions::new(Some(tier(10)), Some(tier(10)), None),
            rounds(5),
            starter
        ),
        "## Silhouette series\n**5 rounds** | Tier **X**\nStarted by <@100>"
    );
    assert_eq!(
        text::series_header(
            &RoundOptions::new(None, None, Some(true)),
            rounds(5),
            starter
        ),
        "## Silhouette series\n\
         **5 rounds** | Tiers **VI-XI**\n\
         Ships that were never built are left out.\n\
         Started by <@100>"
    );
    assert_eq!(
        text::series_intro(&RoundOptions::default(), rounds(5), starter, 3),
        "## Silhouette series\n\
         **5 rounds** | Tiers **VI-XI**\n\
         Started by <@100>\n\
         ### Round 1 of 5 starts in 3..."
    );
    assert_eq!(
        text::series_countdown(2, rounds(5), 5),
        "### Round 2 of 5 starts in 5 seconds"
    );
    assert_eq!(
        text::series_countdown(2, rounds(5), 3),
        "### Round 2 of 5 starts in 3..."
    );
    assert_eq!(
        text::series_standings(&aki_and_bo(), 3, rounds(10)),
        "### Standings after round 3 of 10\n\
         **Most rounds won**\n\
         **1.** <@1> · 2 wins\n\
         **2.** <@2> · 1 win\n\
         **Fastest time**\n\
         **1.** <@2> · 2.500 s\n\
         **2.** <@1> · 3.100 s"
    );
    assert_eq!(
        text::series_standings(&Tally::default(), 3, rounds(10)),
        "### Standings after round 3 of 10\nNobody has won a round in this series yet."
    );
    assert_eq!(
        text::series_overview(&aki_and_bo(), 3, rounds(5)),
        "## Series over\n\
         **3 of 5 rounds played**\n\
         ### Most rounds won\n\
         **1.** <@1> · 2 wins\n\
         **2.** <@2> · 1 win\n\
         ### Fastest time\n\
         **1.** <@2> · 2.500 s\n\
         **2.** <@1> · 3.100 s"
    );
    assert_eq!(
        text::series_overview(&Tally::default(), 3, rounds(5)),
        "## Series over\n**3 of 5 rounds played**\nNobody won a round in this series."
    );
    assert_eq!(
        text::SERIES_CHECK,
        "### Are you still playing?\nPress **Yes** within 10 seconds to keep the series going."
    );
    assert_eq!(
        text::SERIES_EXPIRED,
        "### Nobody answered, so the series ended."
    );
}

#[test]
fn the_updates_setup_reply_names_the_channel_ping_and_any_warning() {
    let channel = ChannelId::new(123);
    let role = Some(Ping::Role(RoleId::new(456)));
    assert_eq!(
        text::updates_setup_reply(channel, role, Some(PingVerdict::Sounds)),
        "Barnacle update posts will go to <#123> and ping <@&456>."
    );
    assert_eq!(
        text::updates_setup_reply(channel, None, None),
        "Barnacle update posts will go to <#123> and ping nobody."
    );
    assert_eq!(
        text::updates_setup_reply(channel, Some(Ping::Everyone), Some(PingVerdict::Sounds)),
        "Barnacle update posts will go to <#123> and ping @everyone."
    );
    for verdict in [PingVerdict::Silent, PingVerdict::BlockedByChannel] {
        let reply = text::updates_setup_reply(channel, role, Some(verdict));
        assert!(
            reply.starts_with("Barnacle update posts will go to <#123> and ping <@&456>. "),
            "{reply}"
        );
        assert!(reply.contains("will not be pinged"), "{reply}");
    }
    let unchecked = text::updates_setup_reply(channel, role, Some(PingVerdict::Unchecked));
    assert!(unchecked.contains("could not check"), "{unchecked}");
}

#[test]
fn the_announce_preview_names_the_channel_ping_and_any_warning() {
    let channel = ChannelId::new(123);
    let role = Some(Ping::Role(RoleId::new(456)));
    assert_eq!(
        text::announce_preview_line(channel, role, Some(PingVerdict::Sounds)),
        "This will be posted in <#123> and ping <@&456>."
    );
    assert_eq!(
        text::announce_preview_line(channel, None, None),
        "This will be posted in <#123> and ping nobody."
    );
    let silent = text::announce_preview_line(channel, role, Some(PingVerdict::Silent));
    assert!(
        silent.starts_with("This will be posted in <#123> and ping <@&456>. "),
        "{silent}"
    );
    assert!(silent.contains("will not be pinged"), "{silent}");
    assert_eq!(
        text::announce_preview_line(channel, role, Some(PingVerdict::BlockedByChannel)),
        text::updates_setup_reply(channel, role, Some(PingVerdict::BlockedByChannel)).replacen(
            "Barnacle update posts will go to <#123>",
            "This will be posted in <#123>",
            1
        )
    );
}

#[test]
fn announcement_replies_name_the_version_and_what_to_do() {
    assert_eq!(
        text::UPDATES_NOT_SET_UP,
        "This server has no updates channel yet. Someone with Manage Server can set one with /updates setup."
    );
    assert_eq!(
        text::updates_already_sent("0.2.0"),
        "This server already got the notes for Barnacle 0.2.0."
    );
    assert_eq!(
        text::announce_version_changed("0.2.1"),
        "Barnacle has been updated to 0.2.1 since this preview, so nothing was posted. Run /announce again."
    );
    assert_eq!(
        text::announce_posted(ChannelId::new(123)),
        "Posted in <#123>."
    );
}

fn standing(wins: u64, best_millis: u64) -> Standing {
    Standing {
        user: UserId::new(7),
        wins,
        best: Duration::from_millis(best_millis),
    }
}

#[test]
fn a_standing_line_gives_rank_name_wins_and_best_time() {
    assert_eq!(
        text::standing_line(1, "Dana", &standing(12, 3_412)),
        "**1.** Dana · 12 wins · best 3.412 s"
    );
    assert_eq!(
        text::standing_line(50, "*Lee*", &standing(1, 900)),
        "**50.** \\*Lee\\* · 1 win · best 0.900 s"
    );
}

#[test]
fn a_long_name_is_cut_to_discords_name_length_before_escaping() {
    assert_eq!(
        text::standing_line(3, &"_".repeat(40), &standing(2, 1_000)),
        format!("**3.** {} · 2 wins · best 1.000 s", "\\_".repeat(32))
    );
}

#[test]
fn a_short_board_is_one_page() {
    let lines = ["first".to_owned(), "second".to_owned()];
    assert_eq!(text::leaderboard_pages(&lines), ["first\nsecond"]);
    assert!(text::leaderboard_pages(&[]).is_empty());
}

#[test]
fn a_full_board_of_long_names_is_split_within_discords_limits() {
    let lines: Vec<String> = (1..=50)
        .map(|rank| text::standing_line(rank, &"\u{1D400}*".repeat(16), &standing(999_999, 30_000)))
        .collect();
    let pages = text::leaderboard_pages(&lines);
    let length = |page: &String| page.encode_utf16().count();
    assert!(pages.len() > 1);
    assert!(
        pages
            .iter()
            .all(|page| length(page) <= text::EMBED_DESCRIPTION_LIMIT)
    );
    let title = text::leaderboard_title(Ranking::FastestTime)
        .encode_utf16()
        .count();
    assert!(title + pages.iter().map(length).sum::<usize>() <= text::MESSAGE_EMBEDS_LIMIT);
    assert_eq!(pages.join("\n"), lines.join("\n"));
}

const SESSION_START: i64 = 1_790_206_200;
const ROSTER_CELLS: usize = 20;
const SEASON_35: &str = "Clan Battles · Season 35: Komodo Dragon";

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

fn session_on(schedule: &Schedule, date: &str) -> Session {
    schedule.session_at(parse_day(date).unwrap()).unwrap()
}

fn signup_session() -> Session {
    session_on(&Schedule::clan_battles(), "2026-09-23")
}

fn denver(count: i64) -> Schedule {
    schedule(
        Weekdays::new(&[jiff::civil::Weekday::Wednesday]).unwrap(),
        "17:00",
        "America/Denver",
        count,
    )
}

fn tallies_for(session: Session, counts: &[(u32, u32)]) -> Vec<HourTally> {
    session
        .hours()
        .each()
        .zip(counts.iter().copied().chain(std::iter::repeat((0, 0))))
        .map(|(hour, (attending, nope))| HourTally {
            hour,
            attending,
            nope,
        })
        .collect()
}

fn tallies() -> Vec<HourTally> {
    tallies_for(signup_session(), &[(6, 1), (7, 1), (5, 2), (3, 3)])
}

fn roster() -> Vec<RosterRow> {
    vec![
        RosterRow {
            user: UserId::new(11),
            cells: vec![Cell::In, Cell::In, Cell::In, Cell::Out],
        },
        RosterRow {
            user: UserId::new(22),
            cells: vec![Cell::In, Cell::In, Cell::Out, Cell::Out],
        },
    ]
}

fn view_of(name: &str, session: Session, open: bool) -> SignupView {
    SignupView {
        event: 1,
        name: name.to_owned(),
        session,
        open,
        pings: Vec::new(),
        hours: tallies_for(session, &[]),
        rows: Vec::new(),
        hidden: 0,
    }
}

fn signup_view(open: bool, rows: Vec<RosterRow>, hidden: usize) -> SignupView {
    SignupView {
        hours: tallies(),
        rows,
        hidden,
        ..view_of(SEASON_35, signup_session(), open)
    }
}

fn event(name: &str, first_day: &str, last_day: &str, schedule: Schedule) -> Event {
    Event {
        id: 1,
        guild: GuildId::new(1),
        channel: ChannelId::new(123),
        name: name.to_owned(),
        span: Span::new(
            parse_day(first_day).unwrap(),
            parse_day(last_day).unwrap(),
            schedule,
        )
        .unwrap(),
        created_by: UserId::new(7),
        created_at_ms: 0,
        ping_roles: Vec::new(),
        ended_at_ms: None,
        posts_up: false,
    }
}

fn ranked_sprint() -> Event {
    event(
        "Ranked Sprint",
        "2026-09-16",
        "2026-11-05",
        Schedule::clan_battles(),
    )
}

fn holds_night_or_season(text: &str) -> bool {
    let lower = text.to_lowercase();
    lower.contains("night") || lower.contains("season")
}

#[test]
fn roster_rows_are_aligned_and_mention_the_player() {
    let description = text::signup_description(&signup_view(true, roster(), 0));
    assert_eq!(
        description,
        [
            "<t:1790206200:F> · starts <t:1790206200:R>",
            "Sign-ups close when the session starts.",
            "",
            "**Hour 1** · <t:1790206200:t> – <t:1790209800:t> · 6 in · 1 out",
            "**Hour 2** · <t:1790209800:t> – <t:1790213400:t> · 7 in · 1 out",
            "**Hour 3** · <t:1790213400:t> – <t:1790217000:t> · 5 in · 2 out",
            "**Hour 4** · <t:1790217000:t> – <t:1790220600:t> · 3 in · 3 out",
            "",
            "`1    2    3    4    `",
            "`in   in   in   out  ` <@11>",
            "`in   in   out  out  ` <@22>",
        ]
        .join("\n")
    );
    let spans: Vec<usize> = description
        .lines()
        .filter(|line| line.starts_with('`'))
        .map(|line| line.split('`').nth(1).unwrap().chars().count())
        .collect();
    assert_eq!(spans, [ROSTER_CELLS; 3]);
}

#[test]
fn an_empty_roster_says_so() {
    let description = text::signup_description(&signup_view(true, Vec::new(), 0));
    assert!(description.ends_with(&format!("\n\n{}", text::NO_ANSWERS_YET)));
    assert!(!description.contains('`'));
    assert!(!description.contains("<@"));
}

#[test]
fn a_closed_view_says_closed_and_drops_the_notice() {
    let description = text::signup_description(&signup_view(false, roster(), 0));
    let lines: Vec<&str> = description.lines().collect();
    assert_eq!(lines[0], "<t:1790206200:F> · Sign-ups closed");
    assert_eq!(lines[1], "");
    assert!(!description.contains(text::SIGNUPS_CLOSE_AT_START));
    assert!(!description.contains("starts <t:"));
    assert_eq!(lines.last(), Some(&"`in   in   out  out  ` <@22>"));
}

#[test]
fn hidden_players_are_counted() {
    let description = text::signup_description(&signup_view(true, roster(), 7));
    assert!(description.ends_with("\n`in   in   out  out  ` <@22>\nand 7 more."));
    assert!(!text::signup_description(&signup_view(true, roster(), 0)).contains("more."));
}

#[test]
fn a_sign_up_post_is_titled_with_its_event_name() {
    for (name, title) in [
        ("*Ranked* Sprint", "\\*Ranked\\* Sprint"),
        (
            "Clan Battles · Season 35: Komodo Dragon",
            "Clan Battles · Season 35: Komodo Dragon",
        ),
    ] {
        let view = view_of(name, signup_session(), true);
        let sent = serde_json::to_value(discord::signup_message(&view, Delivery::New)).unwrap();
        assert_eq!(sent["embeds"][0]["title"], title, "{sent}");
        let edited = serde_json::to_value(discord::signup_edit(&view)).unwrap();
        assert_eq!(edited["embeds"][0]["title"], title, "{edited}");
    }
}

#[test]
fn hour_buttons_and_the_closed_notice_name_the_hour_and_the_time() {
    assert_eq!(text::hour_button_label(hour(1), true), "Hour 1: Attending");
    assert_eq!(text::hour_button_label(hour(4), false), "Hour 4: Nope");
    assert_eq!(
        text::signups_closed_at(SESSION_START),
        "Sign-ups for this session closed at <t:1790206200:t>."
    );
}

#[test]
fn event_replies_name_the_event() {
    assert_eq!(
        Refusal::NameTaken("*Ranked* Sprint".to_owned()).message(),
        "An event named \\*Ranked\\* Sprint is still running here. Pick another name, or end that event first with /event end."
    );
    assert_eq!(
        text::event_removed("Ranked Sprint"),
        "Ranked Sprint was removed. Nothing had been posted."
    );
    assert_eq!(
        Refusal::EventNotFound.message(),
        "No running event here matches that. Pick one of the events suggested as you type; /event list shows the events that are set up."
    );
}

#[test]
fn the_description_lists_each_hour_of_the_event() {
    let session = session_on(
        &schedule(Weekdays::EVERY_DAY, "23:30", "UTC", 2),
        "2026-09-23",
    );
    let description = text::signup_description(&view_of("Ranked Sprint", session, true));
    let hour_lines: Vec<&str> = description
        .lines()
        .filter(|line| line.starts_with("**Hour "))
        .collect();
    assert_eq!(
        hour_lines,
        [
            "**Hour 1** · <t:1790206200:t> – <t:1790209800:t> · 0 in · 0 out",
            "**Hour 2** · <t:1790209800:t> – <t:1790213400:t> · 0 in · 0 out",
        ]
    );
}

#[test]
fn a_sign_up_post_writes_times_as_timestamps() {
    let session = session_on(&denver(4), "2026-10-14");
    let description = text::signup_description(&view_of("Ranked Sprint", session, true));
    for wanted in [
        "<t:1792018800:F>",
        "<t:1792018800:R>",
        "<t:1792018800:t> – <t:1792022400:t>",
        "<t:1792022400:t> – <t:1792026000:t>",
        "<t:1792026000:t> – <t:1792029600:t>",
        "<t:1792029600:t> – <t:1792033200:t>",
    ] {
        assert!(description.contains(wanted), "{description}");
    }
    assert!(!description.contains("17:00"), "{description}");
    assert!(!description.contains("23:00"), "{description}");
}

fn long_session(count: i64) -> Session {
    session_on(
        &schedule(Weekdays::EVERY_DAY, "23:30", "UTC", count),
        "2026-10-14",
    )
}

fn cells_for(count: u8, attending: &[u8]) -> Vec<Cell> {
    (1..=count)
        .map(|number| {
            if attending.contains(&number) {
                Cell::In
            } else {
                Cell::Out
            }
        })
        .collect()
}

fn member(user: u64, count: u8, attending: &[u8]) -> RosterRow {
    RosterRow {
        user: UserId::new(user),
        cells: cells_for(count, attending),
    }
}

#[test]
fn a_24_hour_event_lists_24_hour_lines() {
    let description = text::signup_description(&view_of("Ranked Sprint", long_session(24), true));
    let hour_lines: Vec<&str> = description
        .lines()
        .filter(|line| line.starts_with("**Hour "))
        .collect();
    assert_eq!(hour_lines.len(), 24);
    for (number, line) in (1..=24).zip(&hour_lines) {
        assert!(line.starts_with(&format!("**Hour {number}** · ")), "{line}");
    }
}

#[test]
fn no_choose_hours_text_says_night_or_season() {
    let view = view_of("Ranked Sprint", long_session(6), true);
    for text in [
        text::hours_menu_text(&view),
        text::hours_saved(&[hour(2), hour(3), hour(5)]),
        text::hours_saved(&[]),
        text::HOURS_REFUSED.to_owned(),
    ] {
        assert!(!holds_night_or_season(&text), "{text}");
    }
}

#[test]
fn the_choose_hours_replies_read_as_written() {
    assert_eq!(text::CHOOSE_HOURS_LABEL, "Choose hours");
    assert_eq!(
        text::hours_saved(&[hour(2), hour(3), hour(5)]),
        "Saved. You attend hours 2–3, 5 of this session."
    );
    assert_eq!(
        text::hours_saved(&[]),
        "Saved. You attend no hour of this session."
    );
    assert_eq!(
        text::HOURS_REFUSED,
        "Those hours are not part of this session. Nothing was saved."
    );
}

#[test]
fn a_long_event_lists_hours_as_ranges() {
    let view = SignupView {
        rows: vec![
            member(1, 12, &[]),
            member(2, 12, &[4]),
            member(3, 12, &[1, 2, 3]),
            member(4, 12, &[1, 2, 3, 6, 7, 8, 12]),
            member(5, 12, &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]),
        ],
        ..view_of("Ranked Sprint", long_session(12), true)
    };
    let description = text::signup_description(&view);
    let roster: Vec<&str> = description
        .lines()
        .filter(|line| line.starts_with('`'))
        .collect();
    assert_eq!(
        roster,
        [
            "`out` <@1>",
            "`4` <@2>",
            "`1–3` <@3>",
            "`1–3, 6–8, 12` <@4>",
            "`1–12` <@5>",
        ]
    );
}

#[test]
fn a_short_event_keeps_its_grid() {
    let view = SignupView {
        rows: vec![member(11, 2, &[1])],
        ..view_of("Ranked Sprint", long_session(2), true)
    };
    let description = text::signup_description(&view);
    let roster: Vec<&str> = description
        .lines()
        .filter(|line| line.starts_with('`'))
        .collect();
    assert_eq!(roster, ["`1    2    `", "`in   out  ` <@11>"]);
}

#[test]
fn a_description_that_fits_lists_its_members() {
    let view = SignupView {
        rows: (1..=10)
            .map(|user| member(user, 24, &[1, 2, 3, 6, 7, 8, 12]))
            .collect(),
        ..view_of("Ranked Sprint", long_session(24), true)
    };
    let description = text::signup_description(&view);
    assert_eq!(
        description
            .lines()
            .filter(|line| line.starts_with('`'))
            .count(),
        10
    );
    assert!(!description.contains(" more."), "{description}");
    assert!(description.chars().count() <= 4_096);
}

#[test]
fn a_long_roster_gives_way_to_fit() {
    let odd: Vec<u8> = (1..=23).step_by(2).collect();
    let session = long_session(24);
    let view = SignupView {
        hours: tallies_for(
            session,
            &(1..=24)
                .map(|number| if number % 2 == 1 { (60, 0) } else { (0, 60) })
                .collect::<Vec<(u32, u32)>>(),
        ),
        rows: (0..60)
            .map(|index| member(100_000_000_000_000_000 + index, 24, &odd))
            .collect(),
        hidden: 100,
        ..view_of("Ranked Sprint", session, true)
    };
    let description = text::signup_description(&view);
    assert!(
        description.chars().count() <= 4_096,
        "{} characters",
        description.chars().count()
    );
    let lines: Vec<&str> = description.lines().collect();
    assert_eq!(lines[0], "<t:1792020600:F> · starts <t:1792020600:R>");
    assert_eq!(lines[1], "Sign-ups close when the session starts.");
    for number in 1..=24_i64 {
        let start = 1_792_020_600 + (number - 1) * 3_600;
        let counts = if number % 2 == 1 {
            "60 in · 0 out"
        } else {
            "0 in · 60 out"
        };
        let wanted = format!(
            "**Hour {number}** · <t:{start}:t> – <t:{}:t> · {counts}",
            start + 3_600
        );
        assert!(lines.contains(&wanted.as_str()), "{wanted}");
    }
    let shown: Vec<&str> = lines
        .iter()
        .copied()
        .filter(|line| line.starts_with('`'))
        .collect();
    assert!(!shown.is_empty() && shown.len() < 60, "{}", shown.len());
    for (index, line) in shown.iter().enumerate() {
        assert_eq!(
            *line,
            format!(
                "`1, 3, 5, 7, 9, 11, 13, 15, 17, 19, 21, 23` <@{}>",
                100_000_000_000_000_000 + index
            )
        );
    }
    assert_eq!(
        lines.last().copied(),
        Some(format!("and {} more.", 100 + 60 - shown.len()).as_str())
    );
}

#[test]
fn the_rehearsal_running_refusal_names_reset() {
    let message = Refusal::RehearsalRunning.message();
    assert!(message.contains("/rehearse reset"), "{message}");
    assert!(!holds_night_or_season(&message), "{message}");
}

#[test]
fn no_event_text_says_night_or_season() {
    let event = ranked_sprint();
    let session = session_on(&Schedule::clan_battles(), "2026-09-23");
    let rehearsal = event_every_day("Ranked Sprint", "2026-09-19", "2026-09-21");
    let mut texts = vec![
        text::signup_description(&view_of("Ranked Sprint", session, true)),
        text::signup_description(&view_of("Ranked Sprint", session, false)),
        text::signups_closed_at(SESSION_START),
        text::SESSION_GONE.to_owned(),
        text::rehearsal_started(&rehearsal, 3, Some(1_790_811_000)),
        text::rehearsal_started(&rehearsal, 0, None),
        text::stepped(
            1_790_811_000,
            &TickReport {
                posted: tags(1),
                adopted: tags(1),
                closed: tags(1),
                removed: tags(1),
                failures: 1,
            },
        ),
        text::stepped(1_790_811_000, &TickReport::default()),
        text::reset_done(&PurgeReport {
            events: 1,
            messages: 2,
            answers: 8,
            failures: 0,
        }),
        text::reset_done(&PurgeReport::default()),
        text::reset_blocked(&PurgeReport {
            events: 0,
            messages: 1,
            answers: 0,
            failures: 1,
        }),
        text::event_ended("Ranked Sprint", 0),
        text::event_ended("Ranked Sprint", 2),
        text::event_removed("Ranked Sprint"),
        text::reposted(&event, session, false, &[]),
        text::reposted(&event, session, true, &[Ping::Role(RoleId::new(4))]),
        text::CLEAR_FAILED_END.to_owned(),
        text::CLEAR_FAILED_MOVE.to_owned(),
    ];
    texts.extend(
        every_refusal()
            .iter()
            .chain(&[Refusal::NothingToRepost {
                name: "Ranked Sprint".to_owned(),
                next_post_at: None,
                sessions_left: 0,
            }])
            .map(Refusal::message),
    );
    for text in texts {
        assert!(!holds_night_or_season(&text), "{text}");
    }
}

#[test]
fn a_full_roster_fits_discords_description_limit() {
    let crowd: Vec<RosterRow> = (0..barnacle_bot::attendance::ROSTER_LIMIT)
        .map(|number| RosterRow {
            user: UserId::new(u64::MAX - u64::try_from(number).unwrap()),
            cells: vec![Cell::Out, Cell::In, Cell::Out, Cell::Out],
        })
        .collect();
    let description = text::signup_description(&signup_view(true, crowd, 9_999));
    assert!(
        description.encode_utf16().count() <= text::EMBED_DESCRIPTION_LIMIT,
        "a full roster renders {} units, over Discord's {}",
        description.encode_utf16().count(),
        text::EMBED_DESCRIPTION_LIMIT
    );
}

#[test]
fn an_ended_event_says_the_answers_are_kept() {
    assert_eq!(
        text::event_ended("Ranked Sprint", 2),
        "Ranked Sprint has ended. 2 sign-up posts were cleared, and the answers are kept."
    );
    assert_eq!(
        text::event_ended("Ranked Sprint", 1),
        "Ranked Sprint has ended. 1 sign-up post was cleared, and the answers are kept."
    );
    assert_eq!(
        text::event_ended("*Ranked* Sprint", 0),
        "\\*Ranked\\* Sprint has ended. The answers are kept."
    );
}

#[test]
fn a_pings_line_names_its_ping_role() {
    assert_eq!(
        text::pings_line(&[Ping::Role(RoleId::new(123))]),
        " Pings <@&123>."
    );
    assert_eq!(text::pings_line(&[]), "");
}

#[test]
fn a_silent_ping_warning_names_the_ping_the_cause_and_the_fix() {
    let role = Ping::Role(RoleId::new(456));
    let channel = ChannelId::new(123);
    assert_eq!(
        text::silent_ping_warning(&[(role, PingVerdict::Sounds)], channel),
        None
    );
    assert_eq!(
        text::silent_ping_warning(&[(Ping::Everyone, PingVerdict::Sounds)], channel),
        None
    );
    let silent = text::silent_ping_warning(&[(role, PingVerdict::Silent)], channel).unwrap();
    for wanted in [
        "<@&456>",
        "<#123>",
        "mentionable",
        "Mention @everyone, @here, and All Roles",
    ] {
        assert!(silent.contains(wanted), "{silent}");
    }
    let everyone =
        text::silent_ping_warning(&[(Ping::Everyone, PingVerdict::Silent)], channel).unwrap();
    assert!(everyone.contains("@everyone"), "{everyone}");
    assert!(
        everyone.contains("Mention @everyone, @here, and All Roles"),
        "{everyone}"
    );
    assert!(!everyone.contains("mentionable"), "{everyone}");
    assert!(!everyone.contains("<@&"), "{everyone}");
    let blocked =
        text::silent_ping_warning(&[(role, PingVerdict::BlockedByChannel)], channel).unwrap();
    assert!(blocked.contains("<#123>"), "{blocked}");
    assert!(blocked.contains("override"), "{blocked}");
    let unchecked = text::silent_ping_warning(&[(role, PingVerdict::Unchecked)], channel).unwrap();
    assert!(unchecked.contains("could not check"), "{unchecked}");
    assert!(unchecked.contains("<#123>"), "{unchecked}");
}

#[test]
fn a_channel_blocked_ping_refusal_names_the_override() {
    let refusal = Refusal::PingBlockedByChannel {
        pings: vec![Ping::Role(RoleId::new(456))],
        channel: ChannelId::new(123),
    };
    assert_eq!(refusal.code(), "ping_blocked_by_channel");
    let message = refusal.message();
    for wanted in [
        "<#123>",
        "override",
        "Mention @everyone, @here, and All Roles",
    ] {
        assert!(message.contains(wanted), "{message}");
    }
}

#[test]
fn a_pings_line_names_everyone_as_plain_text_and_lists_every_ping() {
    let line = text::pings_line(&[Ping::Everyone]);
    assert_eq!(line, " Pings @everyone.");
    assert!(!line.contains("<@&"), "{line}");
    assert_eq!(
        text::pings_line(&[
            Ping::Role(RoleId::new(4)),
            Ping::Role(RoleId::new(5)),
            Ping::Role(RoleId::new(6))
        ]),
        " Pings <@&4>, <@&5> and <@&6>."
    );
}

#[test]
fn a_warning_names_each_ping_that_cannot_sound() {
    let warning = text::silent_ping_warning(
        &[
            (Ping::Role(RoleId::new(4)), PingVerdict::Sounds),
            (Ping::Role(RoleId::new(5)), PingVerdict::Silent),
        ],
        ChannelId::new(123),
    )
    .unwrap();
    assert!(warning.contains("<@&5>"), "{warning}");
    assert!(!warning.contains("<@&4>"), "{warning}");
}

#[test]
fn a_channel_blocked_refusal_names_every_blocked_ping() {
    let message = Refusal::PingBlockedByChannel {
        pings: vec![Ping::Role(RoleId::new(4)), Ping::Role(RoleId::new(5))],
        channel: ChannelId::new(123),
    }
    .message();
    assert!(message.contains("<@&4>"), "{message}");
    assert!(message.contains("<@&5>"), "{message}");
}

#[test]
fn a_mixed_everyone_refusal_says_to_pick_one_or_the_other() {
    let refusal = Refusal::PingMixesEveryone;
    assert_eq!(refusal.code(), "ping_mixes_everyone");
    let message = refusal.message();
    assert!(message.contains("@everyone"), "{message}");
}

#[test]
fn a_repost_reply_says_what_was_done() {
    let sprint = ranked_sprint();
    let session = signup_session();
    let quiet = text::reposted(&sprint, session, false, &[]);
    for wanted in ["2026-09-23", "<#123>", "Ranked Sprint"] {
        assert!(quiet.contains(wanted), "{quiet}");
    }
    assert!(!quiet.contains("<@&"), "{quiet}");
    assert!(!quiet.contains("ecovered"), "{quiet}");
    let pinged = text::reposted(
        &sprint,
        session,
        false,
        &[Ping::Role(RoleId::new(4)), Ping::Role(RoleId::new(5))],
    );
    assert!(pinged.contains("<@&4>"), "{pinged}");
    assert!(pinged.contains("<@&5>"), "{pinged}");
    let recovered = text::reposted(&sprint, session, true, &[]);
    assert!(recovered.contains("Recovered"), "{recovered}");
    assert!(recovered.contains("<#123>"), "{recovered}");
    assert!(recovered.contains("2026-09-23"), "{recovered}");
}

#[test]
fn repost_refusals_name_their_reason() {
    let at = Refusal::NothingToRepost {
        name: "Ranked Sprint".to_owned(),
        next_post_at: Some(1_790_811_000),
        sessions_left: 3,
    };
    assert_eq!(at.code(), "nothing_to_repost");
    assert!(
        at.message().contains("<t:1790811000:F>"),
        "{}",
        at.message()
    );
    let soon = Refusal::NothingToRepost {
        name: "Ranked Sprint".to_owned(),
        next_post_at: None,
        sessions_left: 3,
    }
    .message();
    assert!(soon.contains("within a minute"), "{soon}");
    let over = Refusal::NothingToRepost {
        name: "Ranked Sprint".to_owned(),
        next_post_at: None,
        sessions_left: 0,
    }
    .message();
    assert!(over.contains("no sign-up posts left"), "{over}");
    assert!(!over.contains("within a minute"), "{over}");
    assert_eq!(
        Refusal::RepostNothingToPing.code(),
        "repost_nothing_to_ping"
    );
    assert_eq!(
        Refusal::RepostPingNotAllowed.code(),
        "repost_ping_not_allowed"
    );
    assert!(
        Refusal::RepostPingNotAllowed
            .message()
            .contains("Mention @everyone, @here, and All Roles")
    );
    assert_eq!(Refusal::RepostPingsChanged.code(), "repost_pings_changed");
    assert!(Refusal::RepostPingsChanged.message().contains("again"));
    assert_eq!(Refusal::RepostSuperseded.code(), "repost_superseded");
    assert!(
        Refusal::RepostSuperseded
            .message()
            .contains("nothing was reposted")
    );
}

fn tags(count: usize) -> Vec<PostTag> {
    (0..count)
        .map(|position| PostTag {
            event: 1,
            session: session_on(
                &Schedule::clan_battles(),
                ["2026-09-23", "2026-09-24", "2026-09-26"][position],
            ),
        })
        .collect()
}

fn event_every_day(name: &str, first_day: &str, last_day: &str) -> Event {
    event(
        name,
        first_day,
        last_day,
        schedule(Weekdays::EVERY_DAY, "23:30", "UTC", 4),
    )
}

#[test]
fn a_rehearsal_start_names_the_event_and_the_next_post() {
    assert_eq!(
        text::rehearsal_started(
            &event_every_day("Drill", "2026-09-19", "2026-09-21"),
            3,
            Some(1_790_811_000)
        ),
        "Rehearsing Drill. 3 sessions, 3 still ahead. Next sign-up post: <t:1790811000:F> (<t:1790811000:R>)."
    );
    assert_eq!(
        text::rehearsal_started(
            &event_every_day("Drill", "2026-09-19", "2026-09-21"),
            3,
            None
        ),
        "Rehearsing Drill. 3 sessions, 3 still ahead. Next sign-up post: within a minute."
    );
    assert_eq!(
        text::rehearsal_started(
            &event_every_day("Drill", "2026-09-19", "2026-09-19"),
            1,
            Some(1_790_811_000)
        ),
        "Rehearsing Drill. 1 session, 1 still ahead. Next sign-up post: <t:1790811000:F> (<t:1790811000:R>)."
    );
    assert_eq!(
        text::rehearsal_started(
            &event_every_day("*Drill*", "2026-09-19", "2026-09-21"),
            3,
            None
        ),
        "Rehearsing \\*Drill\\*. 3 sessions, 3 still ahead. Next sign-up post: within a minute."
    );
}

#[test]
fn a_step_reports_what_the_beat_did() {
    let posted = TickReport {
        posted: tags(1),
        ..TickReport::default()
    };
    assert_eq!(
        text::stepped(1_790_811_000, &posted),
        "Moved the rehearsal clock to <t:1790811000:F>. 1 sign-up post went up."
    );
    let closed_and_posted = TickReport {
        posted: tags(1),
        closed: tags(1),
        ..TickReport::default()
    };
    assert_eq!(
        text::stepped(1_790_897_400, &closed_and_posted),
        "Moved the rehearsal clock to <t:1790897400:F>. 1 sign-up post closed. 1 sign-up post went up."
    );
    let removed = TickReport {
        removed: tags(1),
        ..TickReport::default()
    };
    assert_eq!(
        text::stepped(1_790_913_600, &removed),
        "Moved the rehearsal clock to <t:1790913600:F>. 1 sign-up post was removed."
    );
    assert_eq!(
        text::stepped(1_790_811_000, &TickReport::default()),
        "Moved the rehearsal clock to <t:1790811000:F>. Nothing happened."
    );
    let everything = TickReport {
        posted: tags(1),
        adopted: tags(2),
        closed: tags(3),
        removed: tags(2),
        failures: 2,
    };
    assert_eq!(
        text::stepped(1_790_811_000, &everything),
        "Moved the rehearsal clock to <t:1790811000:F>. 2 sign-up posts were removed. 3 sign-up posts closed. 1 sign-up post went up. 2 sign-up posts were adopted. 2 steps failed."
    );
    let one_failure = TickReport {
        failures: 1,
        ..TickReport::default()
    };
    assert_eq!(
        text::stepped(1_790_811_000, &one_failure),
        "Moved the rehearsal clock to <t:1790811000:F>. 1 step failed."
    );
    assert!(!text::stepped(1_790_811_000, &one_failure).contains("Nothing happened"));
}

#[test]
fn a_reset_reports_what_it_cleared() {
    assert_eq!(
        text::reset_done(&PurgeReport {
            events: 1,
            messages: 2,
            answers: 8,
            failures: 0
        }),
        "Cleared 1 event, 2 sign-up posts and 8 answers."
    );
    assert_eq!(
        text::reset_done(&PurgeReport {
            events: 1,
            messages: 1,
            answers: 0,
            failures: 0
        }),
        "Cleared 1 event and 1 sign-up post."
    );
    assert_eq!(
        text::reset_done(&PurgeReport {
            events: 2,
            messages: 0,
            answers: 1,
            failures: 0
        }),
        "Cleared 2 events and 1 answer."
    );
    assert_eq!(
        text::reset_done(&PurgeReport {
            events: 1,
            messages: 0,
            answers: 0,
            failures: 0
        }),
        "Cleared 1 event."
    );
    assert_eq!(
        text::reset_done(&PurgeReport::default()),
        "Nothing was there to clear."
    );
}

#[test]
fn a_blocked_reset_says_what_it_had_already_cleared() {
    assert_eq!(
        text::reset_blocked(&PurgeReport {
            events: 0,
            messages: 0,
            answers: 0,
            failures: 1
        }),
        "Some sign-up posts could not be removed, so no event or answer was deleted. Check that the bot can manage messages here, then run this again."
    );
    assert_eq!(
        text::reset_blocked(&PurgeReport {
            events: 0,
            messages: 1,
            answers: 0,
            failures: 1
        }),
        "Some sign-up posts could not be removed, so no event or answer was deleted. 1 sign-up post was already cleared. Check that the bot can manage messages here, then run this again."
    );
    assert_eq!(
        text::reset_blocked(&PurgeReport {
            events: 0,
            messages: 8,
            answers: 0,
            failures: 2
        }),
        "Some sign-up posts could not be removed, so no event or answer was deleted. 8 sign-up posts were already cleared. Check that the bot can manage messages here, then run this again."
    );
}

#[test]
fn a_bulk_step_with_failures_points_to_the_log() {
    assert_eq!(
        text::failures_noted(text::CLEAR_FAILED_MOVE, 2),
        "Some sign-up posts could not be removed from the old channel, so nothing moved. Check that the bot can manage messages there, then run this again. Barnacle's log records the reason for each."
    );
    assert_eq!(
        text::failures_noted(text::CLEAR_FAILED_END, 1),
        "Some sign-up posts could not be removed, so the event is still running. Check that the bot can manage messages in its channel, then run this again. Barnacle's log records the reason for each."
    );
    let stepped = text::stepped(
        1_790_811_000,
        &TickReport {
            posted: tags(1),
            failures: 2,
            ..TickReport::default()
        },
    );
    assert_eq!(
        text::failures_noted(&stepped, 2),
        "Moved the rehearsal clock to <t:1790811000:F>. 1 sign-up post went up. 2 steps failed. Barnacle's log records the reason for each."
    );
    let blocked = text::reset_blocked(&PurgeReport {
        events: 0,
        messages: 8,
        answers: 0,
        failures: 2,
    });
    assert!(text::failures_noted(&blocked, 2).starts_with(&blocked));
    assert!(text::failures_noted(&blocked, 2).ends_with(" 8 sign-up posts were already cleared. Check that the bot can manage messages here, then run this again. Barnacle's log records the reason for each."));
    assert_eq!(text::failures_noted(&stepped, 0), stepped);
}

const BEFORE_THE_SPRINT: i64 = 1_791_900_000;
const AFTER_THE_SPRINT: i64 = 1_792_300_000;

fn listed_sprint() -> Event {
    Event {
        channel: ChannelId::new(10),
        ..event(
            "Ranked Sprint",
            "2026-10-14",
            "2026-10-17",
            schedule(
                Weekdays::new(&[
                    jiff::civil::Weekday::Wednesday,
                    jiff::civil::Weekday::Saturday,
                ])
                .unwrap(),
                "17:00",
                "America/Denver",
                4,
            ),
        )
    }
}

#[test]
fn an_event_line_shows_its_schedule() {
    let line = text::event_line(&listed_sprint(), BEFORE_THE_SPRINT);
    assert_eq!(
        line,
        "<#10> **Ranked Sprint**, 2026-10-14 to 2026-10-17. Wednesday and Saturday at <t:1792018800:t>, 4 hours, America/Denver. 2 sessions ahead. Next sign-up post: <t:1791932400:F> (<t:1791932400:R>)."
    );
    for wanted in [
        "<#10>",
        "Ranked Sprint",
        "2026-10-14 to 2026-10-17",
        "Wednesday and Saturday",
        "<t:1792018800:t>",
        "4 hours",
        "America/Denver",
        "2 sessions ahead",
        "<t:1791932400:F>",
    ] {
        assert!(line.contains(wanted), "{wanted} in {line}");
    }
}

#[test]
fn an_event_line_with_nothing_ahead_says_nothing_further_posts() {
    assert_eq!(
        text::event_line(&listed_sprint(), AFTER_THE_SPRINT),
        "<#10> **Ranked Sprint**, 2026-10-14 to 2026-10-17. Wednesday and Saturday at <t:1792278000:t>, 4 hours, America/Denver. 0 sessions ahead. Nothing further will post."
    );
}

#[test]
fn an_event_line_names_every_day_and_singulars() {
    let single = Event {
        span: Span::new(
            parse_day("2026-10-14").unwrap(),
            parse_day("2026-10-14").unwrap(),
            schedule(Weekdays::EVERY_DAY, "23:30", "UTC", 1),
        )
        .unwrap(),
        ..listed_sprint()
    };
    assert_eq!(
        text::event_line(&single, BEFORE_THE_SPRINT),
        "<#10> **Ranked Sprint**, 2026-10-14 to 2026-10-14. every day at <t:1792020600:t>, 1 hour, UTC. 1 session ahead. Next sign-up post: <t:1791934200:F> (<t:1791934200:R>)."
    );
    let four = Event {
        span: Span::new(
            parse_day("2026-10-14").unwrap(),
            parse_day("2026-10-18").unwrap(),
            Schedule::clan_battles(),
        )
        .unwrap(),
        ..listed_sprint()
    };
    assert!(
        text::event_line(&four, BEFORE_THE_SPRINT)
            .contains(". Wednesday, Thursday, Saturday and Sunday at <t:1792020600:t>, 4 hours, UTC. 4 sessions ahead.")
    );
}

#[test]
fn an_event_line_writes_times_as_timestamps() {
    let line = text::event_line(&listed_sprint(), BEFORE_THE_SPRINT);
    assert!(line.contains("<t:1792018800:t>"), "{line}");
    assert!(!line.contains("17:00"), "{line}");
    assert!(!line.contains("23:00"), "{line}");
}

#[test]
fn an_empty_list_says_no_event_is_set_up() {
    let none = serde_json::to_value(
        discord::event_list_reply(&[], 1_791_900_000)
            .to_slash_initial_response(serenity::CreateInteractionResponseMessage::new()),
    )
    .unwrap();
    assert_eq!(none["flags"], 64);
    assert_eq!(
        none["allowed_mentions"],
        serde_json::to_value(serenity::CreateAllowedMentions::new()).unwrap()
    );
    assert_eq!(none["content"], "No event is set up here.");
    let one = serde_json::to_value(
        discord::event_list_reply(&[listed_sprint()], 1_791_900_000)
            .to_slash_initial_response(serenity::CreateInteractionResponseMessage::new()),
    )
    .unwrap();
    assert_eq!(one["flags"], 64);
    assert_eq!(
        one["allowed_mentions"],
        serde_json::to_value(serenity::CreateAllowedMentions::new()).unwrap()
    );
    assert_eq!(
        one["content"],
        "<#10> **Ranked Sprint**, 2026-10-14 to 2026-10-17. Wednesday and Saturday at <t:1792018800:t>, 4 hours, America/Denver. 2 sessions ahead. Next sign-up post: <t:1791932400:F> (<t:1791932400:R>)."
    );
}

#[test]
fn no_event_list_text_says_night_or_season() {
    let ahead = text::event_line(&listed_sprint(), BEFORE_THE_SPRINT);
    let none_ahead = text::event_line(&listed_sprint(), AFTER_THE_SPRINT);
    let list = text::event_list(std::slice::from_ref(&ahead));
    for written in [
        ahead.as_str(),
        none_ahead.as_str(),
        list.as_str(),
        text::NO_EVENT_HERE,
    ] {
        assert!(!holds_night_or_season(written), "{written}");
    }
}

#[test]
fn a_long_event_list_is_capped_and_counted() {
    let short = vec!["one".to_owned(), "two".to_owned()];
    assert_eq!(text::event_list(&short), "one\ntwo");
    let long: Vec<String> = (0..40)
        .map(|number| format!("<#10> **Ranked Sprint {number}**, 2026-10-14 to 2026-10-17. Wednesday and Saturday at <t:1792018800:t>, 4 hours, America/Denver. 2 sessions ahead. Next sign-up post: <t:1791932400:F> (<t:1791932400:R>)."))
        .collect();
    let rendered = text::event_list(&long);
    assert!(rendered.encode_utf16().count() <= text::MESSAGE_CONTENT_LIMIT);
    assert!(rendered.lines().last().unwrap().starts_with("and "));
    assert!(rendered.lines().last().unwrap().ends_with(" more."));
}

#[test]
fn an_event_line_names_its_pings() {
    let one = Event {
        ping_roles: vec![RoleId::new(456)],
        ..listed_sprint()
    };
    assert!(
        text::event_line(&one, BEFORE_THE_SPRINT).ends_with(" Pings <@&456>."),
        "{}",
        text::event_line(&one, BEFORE_THE_SPRINT)
    );
    let everyone = Event {
        ping_roles: vec![RoleId::new(1)],
        ..listed_sprint()
    };
    let line = text::event_line(&everyone, BEFORE_THE_SPRINT);
    assert!(line.ends_with(" Pings @everyone."), "{line}");
    assert!(!line.contains("<@&"), "{line}");
    let three = Event {
        ping_roles: vec![RoleId::new(4), RoleId::new(5), RoleId::new(6)],
        ..listed_sprint()
    };
    let line = text::event_line(&three, BEFORE_THE_SPRINT);
    assert!(line.ends_with(" Pings <@&4>, <@&5> and <@&6>."), "{line}");
    assert!(!text::event_line(&listed_sprint(), BEFORE_THE_SPRINT).contains("Pings"));
}
