use barnacle_catalog::Nation;
use barnacle_catalog::ShipClass;
use barnacle_catalog::ShipIndex;
use barnacle_catalog::Tier;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reveal {
    pub index: ShipIndex,
    pub name: String,
    pub tier: Tier,
    pub nation: Nation,
    pub class: ShipClass,
}
