mod common;

use std::time::Duration;

use barnacle_bot::attendance::Cell;
use barnacle_bot::attendance::HourTally;
use barnacle_bot::attendance::PostTag;
use barnacle_bot::attendance::PurgeReport;
use barnacle_bot::attendance::RosterRow;
use barnacle_bot::attendance::SignupView;
use barnacle_bot::attendance::TickReport;
use barnacle_bot::attendance_store::Season;
use barnacle_bot::ids::ChannelId;
use barnacle_bot::ids::GuildId;
use barnacle_bot::ids::RoleId;
use barnacle_bot::schedule::Hour;
use barnacle_bot::schedule::Night;
use barnacle_bot::schedule::Range;
use barnacle_bot::schedule::parse_day;
use barnacle_bot::solves::Ranking;
use barnacle_bot::solves::Standing;
use barnacle_bot::text;
use barnacle_catalog::Nation;
use barnacle_catalog::ShipClass;
use barnacle_guess::Hint;
use barnacle_guess::Reveal;
use barnacle_guess::RoundOptions;
use barnacle_guess::Timing;
use barnacle_guess::UserId;
use common::index;
use common::tier;

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
fn an_empty_pool_names_the_options() {
    assert_eq!(
        text::empty_pool(&RoundOptions::new(Some(tier(6)), Some(tier(8)), None)),
        "No ships fit tiers VI-VIII."
    );
    assert_eq!(
        text::empty_pool(&RoundOptions::new(Some(tier(6)), Some(tier(8)), Some(true))),
        "No ships fit tiers VI-VIII with paper ships excluded."
    );
    assert_eq!(
        text::empty_pool(&RoundOptions::new(Some(tier(2)), Some(tier(2)), None)),
        "No ships fit tier II."
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

#[test]
fn missing_permissions_are_listed_in_one_sentence() {
    assert_eq!(
        text::missing_permissions(&["Attach Files", "Read Message History"]),
        "I need these permissions in this channel to run a round: Attach Files, Read Message History."
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

const NIGHT_START: i64 = 1_790_206_200;
const ROSTER_CELLS: usize = 20;

fn signup_night() -> Night {
    Night::parse("2026-09-23").unwrap()
}

fn tallies() -> [HourTally; 4] {
    let counts = [(6, 1), (7, 1), (5, 2), (3, 3)];
    std::array::from_fn(|index| HourTally {
        hour: Hour::ALL[index],
        attending: counts[index].0,
        nope: counts[index].1,
    })
}

fn roster() -> Vec<RosterRow> {
    vec![
        RosterRow {
            user: UserId::new(11),
            cells: [Cell::In, Cell::In, Cell::In, Cell::None],
        },
        RosterRow {
            user: UserId::new(22),
            cells: [Cell::In, Cell::In, Cell::Out, Cell::Out],
        },
    ]
}

fn signup_view(open: bool, rows: Vec<RosterRow>, hidden: usize) -> SignupView {
    SignupView {
        season: 1,
        number: 35,
        codename: Some("Komodo Dragon".to_owned()),
        night: signup_night(),
        open,
        ping: None,
        hours: tallies(),
        rows,
        hidden,
    }
}

fn season(number: u32, codename: Option<&str>, first_day: &str, last_day: &str) -> Season {
    Season {
        id: 1,
        guild: GuildId::new(1),
        channel: ChannelId::new(123),
        number,
        codename: codename.map(str::to_owned),
        range: Range::new(parse_day(first_day).unwrap(), parse_day(last_day).unwrap()).unwrap(),
        created_by: UserId::new(7),
        created_at_ms: 0,
        ping_role: None,
        ended_at_ms: None,
    }
}

fn pinging(season: &Season, role: u64) -> Season {
    Season {
        ping_role: Some(RoleId::new(role)),
        ..season.clone()
    }
}

#[test]
fn roster_rows_are_aligned_and_mention_the_player() {
    let description = text::signup_description(&signup_view(true, roster(), 0));
    assert_eq!(
        description,
        [
            "<t:1790206200:F> · starts <t:1790206200:R>",
            "Sign-ups close when the night starts.",
            "",
            "**Hour 1** · <t:1790206200:t> – <t:1790209800:t> · 6 in · 1 out",
            "**Hour 2** · <t:1790209800:t> – <t:1790213400:t> · 7 in · 1 out",
            "**Hour 3** · <t:1790213400:t> – <t:1790217000:t> · 5 in · 2 out",
            "**Hour 4** · <t:1790217000:t> – <t:1790220600:t> · 3 in · 3 out",
            "",
            "`1    2    3    4    `",
            "`in   in   in   -    ` <@11>",
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
    assert_eq!(text::ROSTER_HEADER.chars().count(), ROSTER_CELLS);
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
fn a_title_without_a_codename_omits_the_colon() {
    assert_eq!(text::signup_title(35, None), "Clan Battles · Season 35");
    assert!(!text::signup_title(35, None).contains(':'));
    assert_eq!(
        text::signup_title(35, Some("Komodo Dragon")),
        "Clan Battles · Season 35: Komodo Dragon"
    );
}

#[test]
fn hour_buttons_and_the_closed_notice_name_the_hour_and_the_time() {
    assert_eq!(
        text::hour_button_label(Hour::ALL[0], true),
        "Hour 1: Attending"
    );
    assert_eq!(text::hour_button_label(Hour::ALL[3], false), "Hour 4: Nope");
    assert_eq!(
        text::signups_closed_at(NIGHT_START),
        "Sign-ups for this night closed at <t:1790206200:t>."
    );
}

#[test]
fn a_started_season_names_its_nights_and_its_first_post() {
    let komodo = season(35, Some("Komodo Dragon"), "2026-09-16", "2026-11-05");
    assert_eq!(
        text::season_started(&komodo, 23, Some(1_790_811_000)),
        "Season 35: Komodo Dragon will post here. 30 CB nights, 23 still ahead. First sign-up post: <t:1790811000:F> (<t:1790811000:R>)."
    );
    assert_eq!(
        text::season_started(&season(35, None, "2026-09-16", "2026-11-05"), 23, None),
        "Season 35 will post here. 30 CB nights, 23 still ahead. First sign-up post: within a minute."
    );
    assert_eq!(
        text::season_line(&komodo, 23, Some(1_790_811_000)),
        "<#123> Season 35: Komodo Dragon, 2026-09-16 to 2026-11-05. 23 nights ahead. Next sign-up post: <t:1790811000:F> (<t:1790811000:R>)."
    );
}

#[test]
fn season_replies_name_the_season_and_its_dates() {
    assert_eq!(
        text::season_number_taken(35),
        "Season 35 already exists. End it first with /cb season end."
    );
    assert_eq!(
        text::season_overlaps(&season(34, None, "2026-06-10", "2026-08-02")),
        "That range overlaps Season 34 (2026-06-10 to 2026-08-02)."
    );
    assert_eq!(
        text::season_removed(35),
        "Season 35 was removed. Nothing had been posted."
    );
    assert_eq!(
        text::season_shortened(35, parse_day("2026-10-01").unwrap()),
        "Season 35 ends after 2026-10-01. No more sign-up posts."
    );
    assert_eq!(text::season_not_found(35), "No Season 35 is set up here.");
}

#[test]
fn one_night_reads_as_singular() {
    let single = season(35, None, "2026-09-16", "2026-09-16");
    assert_eq!(
        text::season_started(&single, 1, None),
        "Season 35 will post here. 1 CB night, 1 still ahead. First sign-up post: within a minute."
    );
    assert_eq!(
        text::season_line(&single, 1, None),
        "<#123> Season 35, 2026-09-16 to 2026-09-16. 1 night ahead. Next sign-up post: within a minute."
    );
}

#[test]
fn a_full_roster_fits_discords_description_limit() {
    let crowd: Vec<RosterRow> = (0..barnacle_bot::attendance::ROSTER_LIMIT)
        .map(|number| RosterRow {
            user: UserId::new(u64::MAX - u64::try_from(number).unwrap()),
            cells: [Cell::Out, Cell::In, Cell::None, Cell::Out],
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
fn a_long_season_list_is_capped_and_counted() {
    let short = vec!["one".to_owned(), "two".to_owned()];
    assert_eq!(text::season_list(&short), "one\ntwo");
    let long: Vec<String> = (0..40)
        .map(|number| format!("<#123> Season {number}, 2026-09-16 to 2026-11-05. 23 nights ahead. Next sign-up post: <t:1790811000:F> (<t:1790811000:R>)."))
        .collect();
    let rendered = text::season_list(&long);
    assert!(rendered.encode_utf16().count() <= text::MESSAGE_CONTENT_LIMIT);
    assert!(rendered.lines().last().unwrap().starts_with("and "));
    assert!(rendered.lines().last().unwrap().ends_with(" more."));
}

#[test]
fn an_edited_season_reports_what_is_left_and_what_was_cleared() {
    let komodo = season(35, Some("Komodo Dragon"), "2026-09-16", "2026-10-21");
    assert_eq!(
        text::season_edited(&komodo, 18, Some(1_790_811_000), 0),
        "Season 35: Komodo Dragon updated. 21 CB nights, 18 still ahead. Next sign-up post: <t:1790811000:F> (<t:1790811000:R>). Manage it with number 35."
    );
    assert_eq!(
        text::season_edited(&komodo, 18, Some(1_790_811_000), 2),
        "Season 35: Komodo Dragon updated. 21 CB nights, 18 still ahead. Next sign-up post: <t:1790811000:F> (<t:1790811000:R>). 2 sign-up posts outside the new dates were cleared. Manage it with number 35."
    );
    assert_eq!(
        text::season_edited(&komodo, 18, None, 1),
        "Season 35: Komodo Dragon updated. 21 CB nights, 18 still ahead. Next sign-up post: within a minute. 1 sign-up post outside the new dates was cleared. Manage it with number 35."
    );
    let single = season(35, None, "2026-09-16", "2026-09-16");
    assert_eq!(
        text::season_edited(&single, 1, None, 0),
        "Season 35 updated. 1 CB night, 1 still ahead. Next sign-up post: within a minute. Manage it with number 35."
    );
    assert_eq!(
        text::season_edited(&single, 0, None, 1),
        "Season 35 updated. 1 CB night, 0 still ahead. Nothing further will post. 1 sign-up post outside the new dates was cleared. Manage it with number 35."
    );
}

#[test]
fn a_moved_season_names_the_new_channel_and_the_next_post() {
    let komodo = season(35, Some("Komodo Dragon"), "2026-09-16", "2026-11-05");
    assert_eq!(
        text::season_moved(&komodo, 4, 1, Some(1_790_811_000)),
        "Season 35: Komodo Dragon now posts in this channel. 1 sign-up post was cleared from the old channel. Next sign-up post: <t:1790811000:F> (<t:1790811000:R>)."
    );
    assert_eq!(
        text::season_moved(&komodo, 4, 0, Some(1_790_811_000)),
        "Season 35: Komodo Dragon now posts in this channel. Next sign-up post: <t:1790811000:F> (<t:1790811000:R>)."
    );
    assert_eq!(
        text::season_moved(&season(35, None, "2026-09-16", "2026-11-05"), 2, 2, None),
        "Season 35 now posts in this channel. 2 sign-up posts were cleared from the old channel. Next sign-up post: within a minute."
    );
    assert_eq!(
        text::season_moved(&komodo, 0, 1, None),
        "Season 35: Komodo Dragon now posts in this channel. 1 sign-up post was cleared from the old channel. Nothing further will post."
    );
    assert_eq!(
        text::season_already_here(35),
        "Season 35 already posts in this channel."
    );
}

#[test]
fn an_ended_season_says_the_answers_are_kept() {
    assert_eq!(
        text::season_ended(35, 2),
        "Season 35 has ended. 2 sign-up posts were cleared, and the answers are kept."
    );
    assert_eq!(
        text::season_ended(35, 1),
        "Season 35 has ended. 1 sign-up post was cleared, and the answers are kept."
    );
    assert_eq!(
        text::season_ended(35, 0),
        "Season 35 has ended. The answers are kept."
    );
}

#[test]
fn a_season_line_names_its_ping_role() {
    assert_eq!(text::pings_line(Some(RoleId::new(123))), " Pings <@&123>.");
    assert_eq!(text::pings_line(None), "");
    let komodo = season(35, Some("Komodo Dragon"), "2026-09-16", "2026-11-05");
    assert_eq!(
        text::season_line(&pinging(&komodo, 456), 23, Some(1_790_811_000)),
        "<#123> Season 35: Komodo Dragon, 2026-09-16 to 2026-11-05. 23 nights ahead. Next sign-up post: <t:1790811000:F> (<t:1790811000:R>). Pings <@&456>."
    );
    assert!(!text::season_line(&komodo, 23, Some(1_790_811_000)).contains("Pings"));
}

fn tags(count: usize) -> Vec<PostTag> {
    (0..count)
        .map(|position| PostTag {
            season: 1,
            night: Night::parse(["2026-09-23", "2026-09-24", "2026-09-26"][position]).unwrap(),
        })
        .collect()
}

fn rehearsal(number: u32, codename: Option<&str>) -> Season {
    Season {
        range: Range::every_day(
            parse_day("2026-09-19").unwrap(),
            parse_day("2026-09-21").unwrap(),
        )
        .unwrap(),
        ..season(number, codename, "2026-09-19", "2026-09-21")
    }
}

#[test]
fn a_rehearsal_start_names_the_season_and_the_next_post() {
    assert_eq!(
        text::rehearsal_started(
            &rehearsal(35, Some("Komodo Dragon")),
            3,
            Some(1_790_811_000)
        ),
        "Rehearsing season 35: Komodo Dragon. 3 CB nights, 3 still ahead. Next sign-up post: <t:1790811000:F> (<t:1790811000:R>). Manage it with number 35."
    );
    assert_eq!(
        text::rehearsal_started(&rehearsal(99, None), 3, None),
        "Rehearsing season 99. 3 CB nights, 3 still ahead. Next sign-up post: within a minute. Manage it with number 99."
    );
    let single = Season {
        range: Range::every_day(
            parse_day("2026-09-19").unwrap(),
            parse_day("2026-09-19").unwrap(),
        )
        .unwrap(),
        ..rehearsal(99, None)
    };
    assert_eq!(
        text::rehearsal_started(&single, 1, Some(1_790_811_000)),
        "Rehearsing season 99. 1 CB night, 1 still ahead. Next sign-up post: <t:1790811000:F> (<t:1790811000:R>). Manage it with number 99."
    );
    assert_eq!(
        text::rehearsal_started(&rehearsal(35, Some("*Komodo*")), 3, None),
        "Rehearsing season 35: \\*Komodo\\*. 3 CB nights, 3 still ahead. Next sign-up post: within a minute. Manage it with number 35."
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
    assert_eq!(
        text::NOTHING_PENDING,
        "Nothing is waiting to happen in this rehearsal."
    );
}

#[test]
fn a_reset_reports_what_it_cleared() {
    assert_eq!(
        text::reset_done(&PurgeReport {
            seasons: 1,
            messages: 2,
            answers: 8,
            failures: 0
        }),
        "Cleared 1 season, 2 sign-up posts and 8 answers."
    );
    assert_eq!(
        text::reset_done(&PurgeReport {
            seasons: 1,
            messages: 1,
            answers: 0,
            failures: 0
        }),
        "Cleared 1 season and 1 sign-up post."
    );
    assert_eq!(
        text::reset_done(&PurgeReport {
            seasons: 2,
            messages: 0,
            answers: 1,
            failures: 0
        }),
        "Cleared 2 seasons and 1 answer."
    );
    assert_eq!(
        text::reset_done(&PurgeReport {
            seasons: 1,
            messages: 0,
            answers: 0,
            failures: 0
        }),
        "Cleared 1 season."
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
            seasons: 0,
            messages: 0,
            answers: 0,
            failures: 1
        }),
        "Some sign-up posts could not be removed, so no season or answer was deleted. Check that the bot can manage messages here, then run this again."
    );
    assert_eq!(
        text::reset_blocked(&PurgeReport {
            seasons: 0,
            messages: 1,
            answers: 0,
            failures: 1
        }),
        "Some sign-up posts could not be removed, so no season or answer was deleted. 1 sign-up post was already cleared. Check that the bot can manage messages here, then run this again."
    );
    assert_eq!(
        text::reset_blocked(&PurgeReport {
            seasons: 0,
            messages: 8,
            answers: 0,
            failures: 2
        }),
        "Some sign-up posts could not be removed, so no season or answer was deleted. 8 sign-up posts were already cleared. Check that the bot can manage messages here, then run this again."
    );
}
