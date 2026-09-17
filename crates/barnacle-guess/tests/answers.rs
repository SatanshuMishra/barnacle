mod common;

use barnacle_guess::RoundOptions;
use common::book;
use common::index;
use common::paper;
use common::set;
use common::ship;
use common::tier;
use common::with_full_name;

#[test]
fn short_and_full_names_are_accepted_in_cleaned_form() {
    let book = book(
        vec![with_full_name(
            ship("PASB017", "W. Virginia '41", 8, "west-virginia"),
            "West Virginia '41",
        )],
        "",
    );
    assert_eq!(
        book.answers(&index("PASB017"), &RoundOptions::default()),
        set(["wvirginia41", "westvirginia41"])
    );
}

#[test]
fn accented_names_are_accepted_in_ascii() {
    let book = book(vec![ship("PGSC109", "Ägir", 9, "agir")], "");
    assert_eq!(
        book.answers(&index("PGSC109"), &RoundOptions::default()),
        set(["agir"])
    );
}

#[test]
fn variant_and_alias_names_count_for_the_base_ship() {
    let book = book(
        vec![
            ship("PASB010", "Montana", 10, "montana"),
            ship("PASB510", "Montana B", 10, "montana"),
        ],
        r#"
[[aliases]]
index = "PASB010"
names = ["Monty"]

[[aliases]]
index = "PASB510"
names = ["Big Monty"]
"#,
    );
    assert_eq!(
        book.answers(&index("PASB010"), &RoundOptions::default()),
        set(["montana", "monty", "montanab", "bigmonty"])
    );
}

#[test]
fn a_collaboration_reskin_without_a_twin_adds_no_names() {
    let book = book(
        vec![
            ship("PJSC007", "Myoko", 7, "myoko"),
            ship("PJSC707", "ARP Myoko", 7, "arp-myoko"),
        ],
        "",
    );
    assert_eq!(
        book.answers(&index("PJSC007"), &RoundOptions::default()),
        set(["myoko"])
    );
}

#[test]
fn a_lookalike_counts_only_when_its_tier_is_in_the_range() {
    let book = book(
        vec![
            ship("PBSC507", "Belfast", 7, "belfast"),
            ship("PBSC528", "Belfast '43", 8, "belfast-43"),
            ship("PBSC108", "Edinburgh", 8, "edinburgh"),
        ],
        r#"
[[lookalikes]]
ships = ["PBSC507", "PBSC528"]
"#,
    );
    let both_tiers = RoundOptions::new(Some(tier(7)), Some(tier(8)), None);
    let tier_seven = RoundOptions::new(Some(tier(7)), Some(tier(7)), None);
    assert_eq!(
        book.answers(&index("PBSC507"), &both_tiers),
        set(["belfast", "belfast43"])
    );
    assert_eq!(
        book.answers(&index("PBSC528"), &both_tiers),
        set(["belfast", "belfast43"])
    );
    assert_eq!(
        book.answers(&index("PBSC507"), &tier_seven),
        set(["belfast"])
    );
}

#[test]
fn a_paper_lookalike_does_not_count_in_a_historical_round() {
    let book = book(
        vec![
            ship("PASB009", "Iowa", 9, "iowa"),
            paper(ship("PASB109", "Minnesota", 9, "minnesota")),
        ],
        r#"
[[lookalikes]]
ships = ["PASB009", "PASB109"]
"#,
    );
    assert_eq!(
        book.answers(&index("PASB009"), &RoundOptions::default()),
        set(["iowa", "minnesota"])
    );
    assert_eq!(
        book.answers(
            &index("PASB009"),
            &RoundOptions::new(None, None, Some(true))
        ),
        set(["iowa"])
    );
}

#[test]
fn a_lookalike_brings_its_variants_and_aliases() {
    let book = book(
        vec![
            ship("PASB009", "Iowa", 9, "iowa"),
            ship("PASB509", "Missouri", 9, "missouri"),
            ship("PASB709", "Missouri Golden", 9, "missouri-golden"),
        ],
        r#"
[[lookalikes]]
ships = ["PASB009", "PASB509"]

[[aliases]]
index = "PASB509"
names = ["Mighty Mo"]
"#,
    );
    assert_eq!(
        book.pool(&RoundOptions::default()),
        [&index("PASB009"), &index("PASB509")]
    );
    assert_eq!(
        book.answers(&index("PASB009"), &RoundOptions::default()),
        set(["iowa", "missouri", "missourigolden", "mightymo"])
    );
}

#[test]
fn a_ship_in_two_lookalike_groups_accepts_both_groups() {
    let book = book(
        vec![
            ship("PBSC507", "Belfast", 7, "belfast"),
            ship("PBSC528", "Belfast '43", 8, "belfast-43"),
            ship("PBSC108", "Edinburgh", 8, "edinburgh"),
        ],
        r#"
[[lookalikes]]
ships = ["PBSC507", "PBSC528"]

[[lookalikes]]
ships = ["PBSC507", "PBSC108"]
"#,
    );
    assert_eq!(
        book.answers(&index("PBSC507"), &RoundOptions::default()),
        set(["belfast", "belfast43", "edinburgh"])
    );
}

#[test]
fn a_copy_of_a_variant_counts_and_a_baseless_exclusion_does_not() {
    let book = book(
        vec![
            ship("PASB010", "Montana", 10, "montana"),
            ship("PASB510", "Montana B", 10, "montana"),
            ship("PASB610", "Montana Event", 10, "montana-event"),
            ship("PASB611", "Montana Prototype", 10, "montana-prototype"),
        ],
        r#"
[[exclude]]
index = "PASB610"
reason = "carbon_copy"
base = "PASB510"

[[exclude]]
index = "PASB611"
reason = "bad_silhouette"
"#,
    );
    assert_eq!(
        book.answers(&index("PASB010"), &RoundOptions::default()),
        set(["montana", "montanab", "montanaevent"])
    );
}

#[test]
fn ships_outside_the_pool_have_no_answers() {
    let book = book(
        vec![
            ship("PASB010", "Montana", 10, "montana"),
            ship("PASB510", "Montana B", 10, "montana"),
            ship("PASB011", "Ohio", 10, "ohio"),
        ],
        r#"
[[lookalikes]]
ships = ["PASB510", "PASB011"]
"#,
    );
    assert!(
        book.answers(&index("PZSX999"), &RoundOptions::default())
            .is_empty()
    );
    assert!(
        book.answers(&index("PASB510"), &RoundOptions::default())
            .is_empty()
    );
}

#[test]
fn a_name_that_cleans_to_nothing_is_not_an_answer() {
    let book = book(
        vec![ship("PASB009", "Iowa", 9, "iowa")],
        r#"
[[aliases]]
index = "PASB009"
names = ["- . ,"]
"#,
    );
    assert_eq!(
        book.answers(&index("PASB009"), &RoundOptions::default()),
        set(["iowa"])
    );
}

#[test]
fn a_ships_own_names_and_its_lookalikes_can_be_listed() {
    let book = book(
        vec![
            ship("PBSC507", "Belfast", 7, "belfast"),
            ship("PBSC528", "Belfast '43", 8, "belfast-43"),
            ship("PBSC108", "Edinburgh", 8, "edinburgh"),
        ],
        r#"
[[lookalikes]]
ships = ["PBSC507", "PBSC528"]

[[lookalikes]]
ships = ["PBSC507", "PBSC108"]

[[aliases]]
index = "PBSC507"
names = ["Belfast Classic"]
"#,
    );
    assert_eq!(
        book.names(&index("PBSC507")),
        set(["belfast", "belfastclassic"])
    );
    assert_eq!(
        book.lookalikes(&index("PBSC507")),
        [&index("PBSC108"), &index("PBSC528")].into_iter().collect()
    );
    assert!(book.names(&index("PZSX999")).is_empty());
    assert!(
        book.lookalikes(&index("PBSC108"))
            .contains(&index("PBSC507"))
    );
}
