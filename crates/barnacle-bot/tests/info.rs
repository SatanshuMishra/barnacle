mod common;

use barnacle_bot::info::ship_card;
use barnacle_catalog::curation::curate;
use barnacle_guess::ShipBook;
use common::catalog;
use common::curation;
use common::index;
use common::ship;

fn field<'a>(card: &'a barnacle_bot::info::ShipCard, name: &str) -> &'a str {
    card.fields
        .iter()
        .find(|(field, _)| field == name)
        .map(|(_, value)| value.as_str())
        .unwrap()
}

#[test]
fn a_pool_ship_shows_its_details_answers_and_lookalikes() {
    let catalog = catalog(vec![
        ship("PJSB018", "Yamato", 10, "yamato"),
        ship("PJSB518", "Yamato B", 10, "yamato"),
        ship("PJSB019", "Musashi", 9, "musashi"),
    ]);
    let config = curation("[[lookalikes]]\nships = [\"PJSB018\", \"PJSB019\"]\n");
    let curated = curate(&catalog, &config);
    let book = ShipBook::new(&catalog, &config);
    let card = ship_card(&catalog, &curated, &book, &index("PJSB018")).unwrap();
    assert_eq!(card.title, "Yamato");
    assert_eq!(
        card.fields
            .iter()
            .map(|(name, _)| name.as_str())
            .collect::<Vec<_>>(),
        [
            "ID",
            "Tier",
            "Class",
            "Nation",
            "Group",
            "Paper ship",
            "In /guess",
            "Accepted answers",
            "Look-alikes"
        ]
    );
    assert_eq!(field(&card, "ID"), "PJSB018");
    assert_eq!(field(&card, "Tier"), "X");
    assert_eq!(field(&card, "Class"), "battleship");
    assert_eq!(field(&card, "Nation"), "Japan");
    assert_eq!(field(&card, "Group"), "upgradeable");
    assert_eq!(field(&card, "Paper ship"), "No");
    assert_eq!(field(&card, "In /guess"), "Yes");
    assert_eq!(field(&card, "Accepted answers"), "yamato, yamatob");
    assert_eq!(field(&card, "Look-alikes"), "Musashi");
}

#[test]
fn an_excluded_ship_says_why_and_names_its_base() {
    let catalog = catalog(vec![
        ship("PJSB018", "Yamato", 10, "yamato"),
        ship("PJSB518", "Yamato B", 10, "yamato"),
    ]);
    let config = curation("");
    let curated = curate(&catalog, &config);
    let book = ShipBook::new(&catalog, &config);
    let card = ship_card(&catalog, &curated, &book, &index("PJSB518")).unwrap();
    assert_eq!(
        field(&card, "In /guess"),
        "No: same silhouette as PJSB018 (Yamato)"
    );
    assert_eq!(field(&card, "Accepted answers"), "None");
    assert_eq!(field(&card, "Look-alikes"), "None");
    assert!(ship_card(&catalog, &curated, &book, &index("PZSX999")).is_none());
}
