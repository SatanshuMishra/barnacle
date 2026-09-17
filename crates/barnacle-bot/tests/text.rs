mod common;

use std::time::Duration;

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
