use std::path::Path;

use barnacle_catalog::ShipIndex;
use barnacle_catalog::ShipName;
use barnacle_data::extract::translations::EnglishNames;
use barnacle_data::extract::translations::NamesError;

fn names() -> EnglishNames {
    EnglishNames::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/mini_en.mo"))
        .unwrap()
}

fn index(value: &str) -> ShipIndex {
    ShipIndex::parse(value).unwrap()
}

#[test]
fn finds_short_and_full_names() {
    assert_eq!(
        names().ship_name(&index("PASB008")),
        Some(ShipName {
            short: "Colorado".to_owned(),
            full: Some("Colorado".to_owned())
        })
    );
    assert_eq!(
        names().ship_name(&index("PGSC519")).map(|name| name.short),
        Some("Ägir".to_owned())
    );
}

#[test]
fn a_missing_full_name_is_absent_not_the_key() {
    assert_eq!(
        names().ship_name(&index("PBSC210")),
        Some(ShipName {
            short: "Goliath".to_owned(),
            full: None
        })
    );
}

#[test]
fn a_ship_without_any_name_has_none() {
    assert_eq!(names().ship_name(&index("PBSC710")), None);
}

#[test]
fn a_missing_catalog_file_is_an_open_error() {
    let error = EnglishNames::load(Path::new("/nonexistent/global.mo"))
        .err()
        .unwrap();
    assert!(matches!(error, NamesError::Open { .. }));
}
