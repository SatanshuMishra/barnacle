mod common;

use std::collections::BTreeSet;

use barnacle_catalog::Nation;
use barnacle_catalog::ShipClass;
use barnacle_catalog::ShipIndex;
use barnacle_guess::GameError;
use barnacle_guess::Hint;
use barnacle_guess::RecentShips;
use barnacle_guess::Reveal;
use barnacle_guess::RoundOptions;
use barnacle_guess::ShipBook;
use common::book;
use common::fleet;
use common::index;
use common::numbered;
use common::set;
use common::ship;
use common::tier;
use common::with_full_name;
use common::with_nation;
use rand::SeedableRng;
use rand::rngs::StdRng;

fn remembered(numbers: std::ops::Range<usize>) -> RecentShips {
    numbers.fold(RecentShips::default(), |recent, number| {
        recent.remember(numbered(number))
    })
}

fn drawn_over_many_rounds(book: &ShipBook, recent: &RecentShips) -> BTreeSet<ShipIndex> {
    let mut rng = StdRng::seed_from_u64(20260916);
    (0..500)
        .map(|_| {
            book.draw(&RoundOptions::default(), recent, &mut rng)
                .unwrap()
                .ship()
                .clone()
        })
        .collect()
}

#[test]
fn a_pool_of_twenty_is_drawn_in_full_even_when_every_ship_is_recent() {
    let book = book(fleet(20), "");
    assert_eq!(
        drawn_over_many_rounds(&book, &remembered(0..20)),
        (0..20).map(numbered).collect()
    );
}

#[test]
fn a_pool_larger_than_twenty_skips_the_recent_ships() {
    let book = book(fleet(21), "");
    assert_eq!(
        drawn_over_many_rounds(&book, &remembered(0..20)),
        BTreeSet::from([numbered(20)])
    );
}

#[test]
fn recent_ships_outside_the_pool_do_not_shrink_it() {
    let book = book(fleet(25), "");
    let recent = (100..120).fold(RecentShips::default(), |recent, number| {
        recent.remember(numbered(number))
    });
    assert_eq!(
        drawn_over_many_rounds(&book, &recent),
        (0..25).map(numbered).collect()
    );
}

#[test]
fn drawing_from_an_empty_pool_is_an_error() {
    let book = book(vec![ship("PASB005", "New York", 5, "new-york")], "");
    let options = RoundOptions::new(Some(tier(9)), Some(tier(8)), Some(true));
    assert_eq!(
        book.draw(
            &options,
            &RecentShips::default(),
            &mut StdRng::seed_from_u64(1)
        ),
        Err(GameError::EmptyPool {
            min_tier: 8,
            max_tier: 9,
            historical: true,
        })
    );
}

#[test]
fn a_draw_carries_its_options_answers_and_reveal() {
    let book = book(
        vec![with_nation(
            with_full_name(ship("PBSC507", "Belfast", 7, "belfast"), "HMS Belfast"),
            "United_Kingdom",
        )],
        "",
    );
    let options = RoundOptions::new(Some(tier(7)), Some(tier(7)), None);
    let draw = book
        .draw(
            &options,
            &RecentShips::default(),
            &mut StdRng::seed_from_u64(1),
        )
        .unwrap();
    assert_eq!(draw.ship(), &index("PBSC507"));
    assert_eq!(draw.options(), &options);
    assert_eq!(draw.answers(), &set(["belfast", "hmsbelfast"]));
    assert_eq!(
        draw.reveal(),
        &Reveal {
            index: index("PBSC507"),
            name: "HMS Belfast".to_owned(),
            tier: tier(7),
            nation: Nation::new("United_Kingdom"),
            class: ShipClass::Cruiser,
        }
    );
}

#[test]
fn the_hint_is_the_nation_when_the_range_is_a_single_tier() {
    let book = book(
        vec![with_nation(
            ship("PBSC507", "Belfast", 7, "belfast"),
            "United_Kingdom",
        )],
        "",
    );
    let options = RoundOptions::new(Some(tier(7)), Some(tier(7)), None);
    let draw = book
        .draw(
            &options,
            &RecentShips::default(),
            &mut StdRng::seed_from_u64(1),
        )
        .unwrap();
    assert_eq!(draw.hint(), &Hint::Nation(Nation::new("United_Kingdom")));
}

#[test]
fn the_hint_is_the_tier_when_the_range_spans_several_tiers() {
    let book = book(vec![ship("PBSC507", "Belfast", 7, "belfast")], "");
    let draw = book
        .draw(
            &RoundOptions::default(),
            &RecentShips::default(),
            &mut StdRng::seed_from_u64(1),
        )
        .unwrap();
    assert_eq!(draw.hint(), &Hint::Tier(tier(7)));
}
