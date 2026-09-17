use std::collections::BTreeMap;
use std::collections::BTreeSet;

use barnacle_catalog::Catalog;
use barnacle_catalog::Ship;
use barnacle_catalog::ShipIndex;
use barnacle_catalog::curation::CurationConfig;
use barnacle_catalog::curation::curate;
use barnacle_catalog::names::clean_answer;

use crate::options::RoundOptions;
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
        let lookalikes = config
            .lookalikes
            .iter()
            .flat_map(|group| {
                group.ships.iter().map(|member| {
                    let others = group
                        .ships
                        .iter()
                        .filter(|other| *other != member)
                        .cloned()
                        .collect::<Vec<_>>();
                    (member.clone(), others)
                })
            })
            .collect();
        Self {
            entries,
            lookalikes,
        }
    }

    pub fn pool(&self, options: &RoundOptions) -> Vec<&ShipIndex> {
        self.eligible(options).map(|(index, _)| index).collect()
    }

    pub fn answers(&self, index: &ShipIndex, options: &RoundOptions) -> BTreeSet<String> {
        let lookalikes = self
            .lookalikes
            .get(index)
            .into_iter()
            .flatten()
            .filter_map(|other| self.entries.get(other))
            .filter(|entry| options.allows(entry.reveal.tier, entry.is_paper));
        self.entries
            .get(index)
            .into_iter()
            .chain(lookalikes)
            .flat_map(|entry| entry.answers.iter().cloned())
            .collect()
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
