#![allow(dead_code)]

pub mod fakes;

use barnacle_bot::solves::Solves;
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
use barnacle_guess::Snowflake;
use sqlx::sqlite::SqlitePool;
use sqlx::sqlite::SqlitePoolOptions;

pub const MIGRATION: &str = include_str!("../../../../migrations/0001_guess_solves.sql");
pub const BUILD: u32 = 13187581;

pub fn index(value: &str) -> ShipIndex {
    ShipIndex::parse(value).unwrap()
}

pub fn tier(value: u32) -> Tier {
    Tier::new(value).unwrap()
}

pub fn at(millis: u64) -> Snowflake {
    Snowflake::new(millis << 22)
}

pub fn ship(value: &str, name: &str, tier_value: u32, silhouette: &str) -> Ship {
    Ship {
        id: ParamId::new(1),
        index: index(value),
        tier: tier(tier_value),
        group: ShipGroup::new("upgradeable"),
        class: ShipClass::Battleship,
        nation: Nation::new("Japan"),
        is_paper: false,
        name: Some(ShipName {
            short: name.to_owned(),
            full: None,
        }),
        silhouette: Some(Silhouette {
            sha256: silhouette.to_owned(),
        }),
    }
}

pub fn fleet(size: usize) -> Vec<Ship> {
    (0..size)
        .map(|number| {
            ship(
                &format!("PXSX{number:03}"),
                &format!("Hull {number}"),
                8,
                &format!("silhouette-{number}"),
            )
        })
        .collect()
}

pub fn catalog(ships: Vec<Ship>) -> Catalog {
    Catalog {
        provenance: Provenance {
            game_version: "15.8.0".to_owned(),
            build: BUILD,
            data_repo_commit: "442496ea2f27517507a562f6eb3ceee06003d3da".to_owned(),
            wowsunpack: "0.45.0".to_owned(),
            wows_data_mgr: "0.21.0".to_owned(),
        },
        ships,
    }
}

pub fn curation_text(tables: &str) -> String {
    format!("reviewed_through = {BUILD}\ngroups = [\"upgradeable\"]\n{tables}")
}

pub fn curation(tables: &str) -> CurationConfig {
    CurationConfig::from_toml(&curation_text(tables)).unwrap()
}

pub async fn memory_pool() -> SqlitePool {
    SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap()
}

pub async fn migrated_pool() -> SqlitePool {
    let pool = memory_pool().await;
    sqlx::raw_sql(MIGRATION).execute(&pool).await.unwrap();
    pool
}

pub async fn solves() -> Solves {
    Solves::with_pool(migrated_pool().await).await.unwrap()
}
