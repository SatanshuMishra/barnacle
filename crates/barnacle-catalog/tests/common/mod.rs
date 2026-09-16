#![allow(dead_code)]

use barnacle_catalog::Catalog;
use barnacle_catalog::Nation;
use barnacle_catalog::ParamId;
use barnacle_catalog::Provenance;
use barnacle_catalog::Ship;
use barnacle_catalog::ShipClass;
use barnacle_catalog::ShipGroup;
use barnacle_catalog::ShipIndex;
use barnacle_catalog::ShipName;
use barnacle_catalog::Silhouette;
use barnacle_catalog::Tier;

pub fn index(value: &str) -> ShipIndex {
    ShipIndex::parse(value).unwrap()
}

pub fn ship(value: &str, name: &str, group: &str, tier: u32, silhouette: &str) -> Ship {
    Ship {
        id: ParamId::new(1),
        index: index(value),
        tier: Tier::new(tier).unwrap(),
        group: ShipGroup::new(group),
        class: ShipClass::Cruiser,
        nation: Nation::new("USA"),
        is_paper: false,
        name: Some(ShipName {
            short: name.to_owned(),
            full: Some(name.to_owned()),
        }),
        silhouette: Some(Silhouette {
            sha256: silhouette.to_owned(),
        }),
    }
}

pub fn catalog(build: u32, ships: Vec<Ship>) -> Catalog {
    Catalog {
        provenance: Provenance {
            game_version: "15.8.0".to_owned(),
            build,
            data_repo_commit: "0000000".to_owned(),
            wowsunpack: "0.45.0".to_owned(),
            wows_data_mgr: "0.21.0".to_owned(),
        },
        ships,
    }
}
