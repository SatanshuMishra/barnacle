mod common;

use barnacle_catalog::Catalog;
use barnacle_catalog::ModelError;
use barnacle_catalog::Ship;
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

#[test]
fn a_catalog_without_hull_models_still_loads() {
    let json = r#"{
  "provenance": {
    "game_version": "15.8.0",
    "build": 13187581,
    "data_repo_commit": "0000000",
    "wowsunpack": "0.45.0",
    "wows_data_mgr": "0.21.0"
  },
  "ships": [
    {
      "id": 1,
      "index": "PASB008",
      "tier": 7,
      "group": "upgradeable",
      "class": "cruiser",
      "nation": "USA",
      "is_paper": false,
      "name": { "short": "Colorado", "full": "Colorado" },
      "silhouette": { "sha256": "aa11" }
    }
  ]
}"#;
    let loaded = Catalog::from_json(json).unwrap();
    assert_eq!(loaded.ships[0].hull_model, None);
    assert_eq!(
        loaded,
        catalog(
            13187581,
            vec![ship("PASB008", "Colorado", "upgradeable", 7, "aa11")]
        )
    );

    let modelled = catalog(
        13187581,
        vec![Ship {
            hull_model: Some(
                "content/gameplay/usa/ship/battleship/ASB008_Colorado_1945/ASB008_Colorado_1945.model"
                    .to_owned(),
            ),
            ..ship("PASB008", "Colorado", "upgradeable", 7, "aa11")
        }],
    );
    let written = modelled.to_json().unwrap();
    assert!(written.contains(
        "\"hull_model\": \"content/gameplay/usa/ship/battleship/ASB008_Colorado_1945/ASB008_Colorado_1945.model\""
    ));
    assert_eq!(Catalog::from_json(&written).unwrap(), modelled);
}
