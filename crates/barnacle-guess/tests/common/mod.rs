#![allow(dead_code)]

use std::collections::BTreeSet;

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
use barnacle_catalog::curation::CurationConfig;
use barnacle_guess::ShipBook;
use barnacle_guess::Snowflake;

pub fn index(value: &str) -> ShipIndex {
    ShipIndex::parse(value).unwrap()
}

pub fn tier(value: u32) -> Tier {
    Tier::new(value).unwrap()
}

pub fn numbered(number: usize) -> ShipIndex {
    index(&format!("PXSX{number:03}"))
}

pub fn at(millis: u64) -> Snowflake {
    Snowflake::new(millis << 22)
}

pub fn set<const N: usize>(answers: [&str; N]) -> BTreeSet<String> {
    answers.iter().map(|answer| (*answer).to_owned()).collect()
}

pub fn ship(value: &str, name: &str, tier_value: u32, silhouette: &str) -> Ship {
    Ship {
        id: ParamId::new(1),
        index: index(value),
        tier: tier(tier_value),
        group: ShipGroup::new("upgradeable"),
        class: ShipClass::Cruiser,
        nation: Nation::new("USA"),
        is_paper: false,
        name: Some(ShipName {
            short: name.to_owned(),
            full: None,
        }),
        silhouette: Some(Silhouette {
            sha256: silhouette.to_owned(),
        }),
        hull_model: None,
    }
}

pub fn with_full_name(ship: Ship, full: &str) -> Ship {
    let short = ship.name.as_ref().map(|name| name.short.clone()).unwrap();
    Ship {
        name: Some(ShipName {
            short,
            full: Some(full.to_owned()),
        }),
        ..ship
    }
}

pub fn with_nation(ship: Ship, nation: &str) -> Ship {
    Ship {
        nation: Nation::new(nation),
        ..ship
    }
}

pub fn with_group(ship: Ship, group: &str) -> Ship {
    Ship {
        group: ShipGroup::new(group),
        ..ship
    }
}

pub fn paper(ship: Ship) -> Ship {
    Ship {
        is_paper: true,
        ..ship
    }
}

pub fn fleet(size: usize) -> Vec<Ship> {
    (0..size)
        .map(|number| {
            ship(
                numbered(number).as_str(),
                &format!("Hull {number}"),
                8,
                &format!("silhouette-{number}"),
            )
        })
        .collect()
}

pub fn book(ships: Vec<Ship>, curation: &str) -> ShipBook {
    let catalog = Catalog {
        provenance: Provenance {
            game_version: "15.8.0".to_owned(),
            build: 13187581,
            data_repo_commit: "0000000".to_owned(),
            wowsunpack: "0.45.0".to_owned(),
            wows_data_mgr: "0.21.0".to_owned(),
        },
        ships,
    };
    let config =
        CurationConfig::from_toml(&format!("groups = [\"upgradeable\"]\n{curation}")).unwrap();
    ShipBook::new(&catalog, &config)
}
