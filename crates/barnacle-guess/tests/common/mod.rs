#![allow(dead_code)]

use barnacle_catalog::ShipIndex;
use barnacle_catalog::Tier;

pub fn index(value: &str) -> ShipIndex {
    ShipIndex::parse(value).unwrap()
}

pub fn tier(value: u32) -> Tier {
    Tier::new(value).unwrap()
}
