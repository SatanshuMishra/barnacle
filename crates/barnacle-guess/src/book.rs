use std::collections::BTreeMap;

use barnacle_catalog::Catalog;
use barnacle_catalog::Ship;
use barnacle_catalog::ShipIndex;
use barnacle_catalog::curation::CurationConfig;
use barnacle_catalog::curation::curate;

use crate::options::RoundOptions;
use crate::reveal::Reveal;

#[derive(Debug, Clone, PartialEq, Eq)]
struct Entry {
    reveal: Reveal,
    is_paper: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShipBook {
    entries: BTreeMap<ShipIndex, Entry>,
}

impl ShipBook {
    pub fn new(catalog: &Catalog, config: &CurationConfig) -> Self {
        let curated = curate(catalog, config);
        let ships: BTreeMap<&ShipIndex, &Ship> = catalog
            .ships
            .iter()
            .map(|ship| (&ship.index, ship))
            .collect();
        let entries = curated
            .pool
            .iter()
            .filter_map(|index| {
                let ship = ships.get(index)?;
                let name = ship.name.as_ref()?;
                let reveal = Reveal {
                    index: index.clone(),
                    name: name.display().to_owned(),
                    tier: ship.tier,
                    nation: ship.nation.clone(),
                    class: ship.class.clone(),
                };
                Some((
                    index.clone(),
                    Entry {
                        reveal,
                        is_paper: ship.is_paper,
                    },
                ))
            })
            .collect();
        Self { entries }
    }

    pub fn pool(&self, options: &RoundOptions) -> Vec<&ShipIndex> {
        self.eligible(options).map(|(index, _)| index).collect()
    }

    fn eligible(&self, options: &RoundOptions) -> impl Iterator<Item = (&ShipIndex, &Entry)> {
        let options = *options;
        self.entries
            .iter()
            .filter(move |(_, entry)| options.allows(entry.reveal.tier, entry.is_paper))
    }
}
