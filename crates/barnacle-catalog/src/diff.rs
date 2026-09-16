use std::collections::BTreeMap;
use std::collections::BTreeSet;

use crate::curation::Curated;
use crate::curation::CurationConfig;
use crate::model::Catalog;
use crate::model::Ship;
use crate::model::ShipGroup;
use crate::model::ShipIndex;
use crate::names::name_tokens;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Regrouped {
    pub index: ShipIndex,
    pub from: ShipGroup,
    pub to: ShipGroup,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogDiff {
    pub added: Vec<ShipIndex>,
    pub removed: Vec<ShipIndex>,
    pub regrouped: Vec<Regrouped>,
    pub new_groups: Vec<ShipGroup>,
}

pub fn diff(old: Option<&Catalog>, new: &Catalog) -> CatalogDiff {
    let old_ships: BTreeMap<&ShipIndex, &Ship> = old
        .into_iter()
        .flat_map(|catalog| &catalog.ships)
        .map(|ship| (&ship.index, ship))
        .collect();
    let new_ships: BTreeMap<&ShipIndex, &Ship> =
        new.ships.iter().map(|ship| (&ship.index, ship)).collect();
    let old_groups: BTreeSet<&ShipGroup> = old_ships.values().map(|ship| &ship.group).collect();
    let new_groups: BTreeSet<&ShipGroup> = new_ships.values().map(|ship| &ship.group).collect();
    CatalogDiff {
        added: new_ships
            .keys()
            .filter(|index| !old_ships.contains_key(**index))
            .map(|index| (*index).clone())
            .collect(),
        removed: old_ships
            .keys()
            .filter(|index| !new_ships.contains_key(**index))
            .map(|index| (*index).clone())
            .collect(),
        regrouped: new_ships
            .values()
            .filter_map(|ship| {
                let before = old_ships.get(&ship.index)?;
                (before.group != ship.group).then(|| Regrouped {
                    index: ship.index.clone(),
                    from: before.group.clone(),
                    to: ship.group.clone(),
                })
            })
            .collect(),
        new_groups: new_groups
            .difference(&old_groups)
            .map(|group| (*group).clone())
            .collect(),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LookalikeCandidate {
    pub original: ShipIndex,
    pub refit: ShipIndex,
}

pub fn year_refit_candidates(
    catalog: &Catalog,
    config: &CurationConfig,
    curated: &Curated,
) -> Vec<LookalikeCandidate> {
    let pool: Vec<(&Ship, Vec<String>)> = catalog
        .ships
        .iter()
        .filter(|ship| curated.pool.contains(&ship.index))
        .filter_map(|ship| {
            ship.name
                .as_ref()
                .map(|name| (ship, name_tokens(name.display())))
        })
        .collect();
    let by_name: BTreeMap<&[String], &Ship> = pool
        .iter()
        .map(|(ship, tokens)| (tokens.as_slice(), *ship))
        .collect();
    pool.iter()
        .filter_map(|(refit, tokens)| {
            let (year, stem) = tokens.split_last()?;
            let is_year = year.len() == 2 && year.bytes().all(|byte| byte.is_ascii_digit());
            let original = by_name.get(stem).filter(|_| is_year && !stem.is_empty())?;
            let grouped = config.lookalikes.iter().any(|group| {
                group.ships.contains(&original.index) && group.ships.contains(&refit.index)
            });
            (!grouped).then(|| LookalikeCandidate {
                original: original.index.clone(),
                refit: refit.index.clone(),
            })
        })
        .collect()
}
