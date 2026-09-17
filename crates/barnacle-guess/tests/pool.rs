mod common;

use barnacle_guess::RoundOptions;
use common::book;
use common::index;
use common::paper;
use common::ship;
use common::tier;
use common::with_group;

#[test]
fn the_pool_holds_eligible_base_ships_inside_the_tier_range() {
    let book = book(
        vec![
            ship("PASB004", "Wyoming", 4, "wyoming"),
            ship("PASB005", "New York", 5, "new-york"),
            ship("PASB006", "New Mexico", 6, "new-mexico"),
            ship("PASB010", "Montana", 10, "montana"),
            ship("PASB510", "Montana B", 10, "montana"),
            with_group(
                ship("PASX006", "Prototype", 6, "prototype"),
                "demoWithoutStats",
            ),
        ],
        "",
    );
    let options = RoundOptions::new(Some(tier(5)), Some(tier(10)), None);
    assert_eq!(
        book.pool(&options),
        [&index("PASB005"), &index("PASB006"), &index("PASB010")]
    );
}

#[test]
fn historical_drops_paper_ships_from_the_pool() {
    let book = book(
        vec![
            ship("PASB009", "Iowa", 9, "iowa"),
            paper(ship("PASB109", "Minnesota", 9, "minnesota")),
        ],
        "",
    );
    assert_eq!(book.pool(&RoundOptions::default()).len(), 2);
    assert_eq!(
        book.pool(&RoundOptions::new(None, None, Some(true))),
        [&index("PASB009")]
    );
}

#[test]
fn ships_the_curation_excludes_never_enter_the_pool() {
    let book = book(
        vec![
            ship("PJSC007", "Myoko", 7, "myoko"),
            ship("PJSC707", "ARP Myoko", 7, "arp-myoko"),
        ],
        "",
    );
    assert_eq!(book.pool(&RoundOptions::default()), [&index("PJSC007")]);
}
