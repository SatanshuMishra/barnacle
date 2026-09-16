mod common;

use barnacle_catalog::Catalog;
use barnacle_catalog::ModelError;
use barnacle_catalog::ShipIndex;
use barnacle_catalog::ShipName;
use barnacle_catalog::Tier;
use common::catalog;
use common::index;
use common::ship;

#[test]
fn ship_index_accepts_wargaming_indices() {
    assert_eq!(ShipIndex::parse("PASB008").unwrap().as_str(), "PASB008");
    assert_eq!(index("PGSC519").to_string(), "PGSC519");
}

#[test]
fn ship_index_rejects_malformed_values() {
    for bad in ["", "PASB08", "pasb008", "PASB0088", "PASB-08"] {
        assert_eq!(
            ShipIndex::parse(bad),
            Err(ModelError::InvalidIndex {
                value: bad.to_owned()
            })
        );
    }
}

#[test]
fn tier_accepts_one_through_eleven_only() {
    assert_eq!(Tier::new(1).unwrap().get(), 1);
    assert_eq!(Tier::new(11).unwrap().get(), 11);
    assert_eq!(Tier::new(0), Err(ModelError::InvalidTier { value: 0 }));
    assert_eq!(Tier::new(12), Err(ModelError::InvalidTier { value: 12 }));
    assert_eq!(Tier::new(300), Err(ModelError::InvalidTier { value: 300 }));
}

#[test]
fn display_name_prefers_the_full_name() {
    let arkansas = ShipName {
        short: "Arkansas B".to_owned(),
        full: Some("Arkansas Beta".to_owned()),
    };
    let goliath = ShipName {
        short: "Goliath".to_owned(),
        full: None,
    };
    assert_eq!(arkansas.display(), "Arkansas Beta");
    assert_eq!(goliath.display(), "Goliath");
}

#[test]
fn catalog_round_trips_through_json() {
    let original = catalog(
        13187581,
        vec![ship("PASB008", "Colorado", "upgradeable", 7, "aa11")],
    );
    let json = original.to_json().unwrap();
    assert!(json.contains("\"index\": \"PASB008\""));
    assert_eq!(Catalog::from_json(&json).unwrap(), original);
}

#[test]
fn catalog_json_with_a_bad_index_or_tier_is_rejected() {
    let json = catalog(
        13187581,
        vec![ship("PASB008", "Colorado", "upgradeable", 7, "aa11")],
    )
    .to_json()
    .unwrap();
    assert!(Catalog::from_json(&json.replace("PASB008", "bad")).is_err());
    assert!(Catalog::from_json(&json.replace("\"tier\": 7", "\"tier\": 12")).is_err());
}

#[test]
fn catalog_finds_ships_by_index() {
    let found = catalog(
        13187581,
        vec![ship("PASB008", "Colorado", "upgradeable", 7, "aa11")],
    );
    assert_eq!(
        found.get(&index("PASB008")).map(|ship| ship.tier.get()),
        Some(7)
    );
    assert!(found.get(&index("PBSC710")).is_none());
}
