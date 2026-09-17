use barnacle_catalog::Catalog;
use barnacle_catalog::ShipIndex;
use barnacle_catalog::curation::Curated;
use barnacle_guess::ShipBook;

use crate::text;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShipCard {
    pub title: String,
    pub fields: Vec<(String, String)>,
}

pub fn ship_card(
    catalog: &Catalog,
    curated: &Curated,
    book: &ShipBook,
    index: &ShipIndex,
) -> Option<ShipCard> {
    let ship = catalog.get(index)?;
    let title = ship
        .name
        .as_ref()
        .map(|name| name.display().to_owned())
        .unwrap_or_else(|| index.to_string());
    let name_of = |other: &ShipIndex| {
        catalog
            .get(other)
            .and_then(|found| found.name.as_ref())
            .map(|name| name.display().to_owned())
    };
    let in_guess = match curated.removed.get(index) {
        None => "Yes".to_owned(),
        Some(removal) => {
            let base_name = removal
                .base()
                .and_then(name_of)
                .map(|name| format!(" ({name})"))
                .unwrap_or_default();
            format!("No: {removal}{base_name}")
        }
    };
    let answers: Vec<String> = book.names(index).into_iter().collect();
    let lookalikes: Vec<String> = book
        .lookalikes(index)
        .into_iter()
        .map(|other| name_of(other).unwrap_or_else(|| other.to_string()))
        .collect();
    let paper = if ship.is_paper { "Yes" } else { "No" };
    let fields = vec![
        ("ID".to_owned(), index.to_string()),
        ("Tier".to_owned(), text::tier_numeral(ship.tier).to_owned()),
        ("Class".to_owned(), text::class_label(&ship.class)),
        ("Nation".to_owned(), text::nation_label(&ship.nation)),
        ("Group".to_owned(), ship.group.to_string()),
        ("Paper ship".to_owned(), paper.to_owned()),
        (
            "In /guess".to_owned(),
            text::listing(&[in_guess], text::FIELD_LIMIT),
        ),
        (
            "Accepted answers".to_owned(),
            text::listing(&answers, text::FIELD_LIMIT),
        ),
        (
            "Look-alikes".to_owned(),
            text::listing(&lookalikes, text::FIELD_LIMIT),
        ),
    ];
    Some(ShipCard { title, fields })
}
