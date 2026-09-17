use std::collections::BTreeMap;
use std::collections::BTreeSet;

use barnacle_catalog::Catalog;
use barnacle_catalog::Ship;
use barnacle_catalog::ShipIndex;
use barnacle_catalog::curation::CurationConfig;
use barnacle_catalog::curation::curate;
use barnacle_catalog::names::clean_answer;
use rand::Rng;
use rand::seq::IndexedRandom;

use crate::draw::Draw;
use crate::draw::Hint;
use crate::error::GameError;
use crate::options::RoundOptions;
use crate::recent::RecentShips;
use crate::reveal::Reveal;

#[derive(Debug, Clone, PartialEq, Eq)]
struct Entry {
    reveal: Reveal,
    is_paper: bool,
    answers: BTreeSet<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShipBook {
    entries: BTreeMap<ShipIndex, Entry>,
    lookalikes: BTreeMap<ShipIndex, Vec<ShipIndex>>,
}

impl ShipBook {
    pub fn new(catalog: &Catalog, config: &CurationConfig) -> Self {
        let curated = curate(catalog, config);
        let ships: BTreeMap<&ShipIndex, &Ship> = catalog
            .ships
            .iter()
            .map(|ship| (&ship.index, ship))
            .collect();
        let variants = group_by_key(
            curated
                .removed
                .iter()
                .filter_map(|(index, removal)| removal.base().map(|base| (base, index))),
        );
        let aliases = group_by_key(config.aliases.iter().flat_map(|entry| {
            entry
                .names
                .iter()
                .map(move |name| (&entry.index, name.as_str()))
        }));
        let entries = curated
            .pool
            .iter()
            .filter_map(|index| {
                let ship = ships.get(index)?;
                let name = ship.name.as_ref()?;
                let answers = std::iter::once(index)
                    .chain(variants.get(index).into_iter().flatten().copied())
                    .flat_map(|member| cleaned_names(member, &ships, &aliases))
                    .collect();
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
                        answers,
                    },
                ))
            })
            .collect();
        let lookalikes = group_by_key(config.lookalikes.iter().flat_map(|group| {
            group.ships.iter().flat_map(move |member| {
                group
                    .ships
                    .iter()
                    .filter(move |other| *other != member)
                    .map(move |other| (member.clone(), other.clone()))
            })
        }));
        Self {
            entries,
            lookalikes,
        }
    }

    pub fn pool(&self, options: &RoundOptions) -> Vec<&ShipIndex> {
        self.eligible(options).map(|(index, _)| index).collect()
    }

    pub fn answers(&self, index: &ShipIndex, options: &RoundOptions) -> BTreeSet<String> {
        let Some(own) = self.entries.get(index) else {
            return BTreeSet::new();
        };
        let lookalikes = self
            .lookalikes
            .get(index)
            .into_iter()
            .flatten()
            .filter_map(|other| self.entries.get(other))
            .filter(|entry| options.allows(entry.reveal.tier, entry.is_paper));
        std::iter::once(own)
            .chain(lookalikes)
            .flat_map(|entry| entry.answers.iter().cloned())
            .collect()
    }

    pub fn draw<R: Rng + ?Sized>(
        &self,
        options: &RoundOptions,
        recent: &RecentShips,
        rng: &mut R,
    ) -> Result<Draw, GameError> {
        let eligible: Vec<(&ShipIndex, &Entry)> = self.eligible(options).collect();
        let fresh: Vec<(&ShipIndex, &Entry)> = if eligible.len() > RecentShips::LIMIT {
            eligible
                .into_iter()
                .filter(|(index, _)| !recent.contains(index))
                .collect()
        } else {
            eligible
        };
        let (index, entry) = fresh.choose(rng).copied().ok_or(GameError::EmptyPool {
            min_tier: options.min_tier().get(),
            max_tier: options.max_tier().get(),
            historical: options.historical(),
        })?;
        Ok(Draw {
            options: *options,
            answers: self.answers(index, options),
            hint: Hint::for_ship(&entry.reveal, options),
            reveal: entry.reveal.clone(),
        })
    }

    fn eligible(&self, options: &RoundOptions) -> impl Iterator<Item = (&ShipIndex, &Entry)> {
        let options = *options;
        self.entries
            .iter()
            .filter(move |(_, entry)| options.allows(entry.reveal.tier, entry.is_paper))
    }
}

fn group_by_key<K: Ord, V>(pairs: impl Iterator<Item = (K, V)>) -> BTreeMap<K, Vec<V>> {
    pairs.fold(BTreeMap::new(), |mut grouped, (key, value)| {
        grouped.entry(key).or_insert_with(Vec::new).push(value);
        grouped
    })
}

fn cleaned_names(
    index: &ShipIndex,
    ships: &BTreeMap<&ShipIndex, &Ship>,
    aliases: &BTreeMap<&ShipIndex, Vec<&str>>,
) -> Vec<String> {
    let official = ships
        .get(index)
        .and_then(|ship| ship.name.as_ref())
        .into_iter()
        .flat_map(|name| std::iter::once(name.short.as_str()).chain(name.full.as_deref()));
    let extra = aliases.get(index).into_iter().flatten().copied();
    official
        .chain(extra)
        .map(clean_answer)
        .filter(|answer| !answer.is_empty())
        .collect()
}
