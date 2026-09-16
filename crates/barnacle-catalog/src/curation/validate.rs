use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::fmt;

use crate::curation::config::CurationConfig;
use crate::curation::rules::Curated;
use crate::curation::rules::curate;
use crate::model::Catalog;
use crate::model::ShipIndex;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Section {
    Exclude,
    ExcludeBase,
    Keep,
    Lookalikes,
    Aliases,
}

impl fmt::Display for Section {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Exclude => "exclude",
            Self::ExcludeBase => "exclude base",
            Self::Keep => "keep",
            Self::Lookalikes => "lookalikes",
            Self::Aliases => "aliases",
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Problem {
    #[error("{section} names {index}, which is not in the catalog")]
    UnknownIndex { section: Section, index: ShipIndex },
    #[error("{index} is excluded more than once")]
    DuplicateExclude { index: ShipIndex },
    #[error("{index} is both excluded and kept")]
    ExcludedAndKept { index: ShipIndex },
    #[error("{index} is kept, but keeping it changes nothing")]
    KeepHasNoEffect { index: ShipIndex },
    #[error("{index} counts as a copy of {base}, but {base} is not in the pool")]
    BaseNotInPool { index: ShipIndex, base: ShipIndex },
    #[error("lookalike group {position} lists {index} more than once")]
    DuplicateInLookalikeGroup { position: usize, index: ShipIndex },
    #[error("{index} appears in more than one lookalike group")]
    InSeveralLookalikeGroups { index: ShipIndex },
    #[error("{index} is in a lookalike group but is not in the pool")]
    LookalikeNotInPool { index: ShipIndex },
    #[error("lookalike group {position} has {eligible} ship(s) in the pool; it needs at least 2")]
    LookalikeGroupTooSmall { position: usize, eligible: usize },
    #[error("no ship is left in the pool")]
    EmptyPool,
    #[error("curation has never been reviewed; review build {build} and set reviewed_through")]
    NotReviewed { build: u32 },
    #[error(
        "curation was reviewed through build {reviewed_through}, but the catalog is build {build}"
    )]
    ReviewBehind { reviewed_through: u32, build: u32 },
}

pub fn validate(catalog: &Catalog, config: &CurationConfig, curated: &Curated) -> Vec<Problem> {
    [
        unknown_indices(catalog, config),
        duplicate_excludes(config),
        excluded_and_kept(config),
        ineffective_keeps(catalog, config, curated),
        missing_bases(catalog, curated),
        lookalike_problems(config, curated),
        pool_state(curated),
        review_state(catalog, config),
    ]
    .into_iter()
    .flatten()
    .collect()
}

fn unknown_indices(catalog: &Catalog, config: &CurationConfig) -> Vec<Problem> {
    config
        .exclude
        .iter()
        .map(|entry| (Section::Exclude, &entry.index))
        .chain(
            config
                .exclude
                .iter()
                .filter_map(|entry| entry.base.as_ref())
                .map(|base| (Section::ExcludeBase, base)),
        )
        .chain(
            config
                .keep
                .iter()
                .map(|entry| (Section::Keep, &entry.index)),
        )
        .chain(
            config
                .lookalikes
                .iter()
                .flat_map(|group| &group.ships)
                .map(|index| (Section::Lookalikes, index)),
        )
        .chain(
            config
                .aliases
                .iter()
                .map(|entry| (Section::Aliases, &entry.index)),
        )
        .filter(|(_, index)| catalog.get(index).is_none())
        .map(|(section, index)| Problem::UnknownIndex {
            section,
            index: index.clone(),
        })
        .collect()
}

fn repeated<'a>(indices: impl Iterator<Item = &'a ShipIndex>) -> Vec<ShipIndex> {
    let counts = indices.fold(BTreeMap::<&ShipIndex, usize>::new(), |mut counts, index| {
        *counts.entry(index).or_insert(0) += 1;
        counts
    });
    counts
        .into_iter()
        .filter(|(_, count)| *count > 1)
        .map(|(index, _)| index.clone())
        .collect()
}

fn duplicate_excludes(config: &CurationConfig) -> Vec<Problem> {
    repeated(config.exclude.iter().map(|entry| &entry.index))
        .into_iter()
        .map(|index| Problem::DuplicateExclude { index })
        .collect()
}

fn excluded_and_kept(config: &CurationConfig) -> Vec<Problem> {
    let kept: BTreeSet<&ShipIndex> = config.keep.iter().map(|entry| &entry.index).collect();
    config
        .exclude
        .iter()
        .filter(|entry| kept.contains(&entry.index))
        .map(|entry| Problem::ExcludedAndKept {
            index: entry.index.clone(),
        })
        .collect()
}

fn ineffective_keeps(
    catalog: &Catalog,
    config: &CurationConfig,
    curated: &Curated,
) -> Vec<Problem> {
    let excluded: BTreeSet<&ShipIndex> = config.exclude.iter().map(|entry| &entry.index).collect();
    config
        .keep
        .iter()
        .filter(|entry| catalog.get(&entry.index).is_some() && !excluded.contains(&entry.index))
        .filter(|entry| {
            let without_this_keep = curate(
                catalog,
                &CurationConfig {
                    keep: config
                        .keep
                        .iter()
                        .filter(|other| other.index != entry.index)
                        .cloned()
                        .collect(),
                    ..config.clone()
                },
            );
            !(curated.pool.contains(&entry.index)
                && without_this_keep.removed.contains_key(&entry.index))
        })
        .map(|entry| Problem::KeepHasNoEffect {
            index: entry.index.clone(),
        })
        .collect()
}

fn missing_bases(catalog: &Catalog, curated: &Curated) -> Vec<Problem> {
    curated
        .removed
        .iter()
        .filter_map(|(index, removal)| {
            let base = removal.base()?;
            (catalog.get(base).is_some() && !curated.pool.contains(base)).then(|| {
                Problem::BaseNotInPool {
                    index: index.clone(),
                    base: base.clone(),
                }
            })
        })
        .collect()
}

fn lookalike_problems(config: &CurationConfig, curated: &Curated) -> Vec<Problem> {
    let groups: Vec<BTreeSet<&ShipIndex>> = config
        .lookalikes
        .iter()
        .map(|group| group.ships.iter().collect())
        .collect();
    let duplicated = config
        .lookalikes
        .iter()
        .enumerate()
        .flat_map(|(position, group)| {
            repeated(group.ships.iter()).into_iter().map(move |index| {
                Problem::DuplicateInLookalikeGroup {
                    position: position + 1,
                    index,
                }
            })
        });
    let several = repeated(groups.iter().flatten().copied())
        .into_iter()
        .map(|index| Problem::InSeveralLookalikeGroups { index });
    let outside = groups
        .iter()
        .flatten()
        .filter(|index| curated.removed.contains_key(**index))
        .map(|index| Problem::LookalikeNotInPool {
            index: (*index).clone(),
        });
    let small = groups.iter().enumerate().filter_map(|(position, group)| {
        let eligible = group
            .iter()
            .filter(|index| curated.pool.contains(**index))
            .count();
        (eligible < 2).then_some(Problem::LookalikeGroupTooSmall {
            position: position + 1,
            eligible,
        })
    });
    duplicated
        .chain(several)
        .chain(outside)
        .chain(small)
        .collect()
}

fn pool_state(curated: &Curated) -> Vec<Problem> {
    if curated.pool.is_empty() {
        vec![Problem::EmptyPool]
    } else {
        Vec::new()
    }
}

fn review_state(catalog: &Catalog, config: &CurationConfig) -> Vec<Problem> {
    let build = catalog.provenance.build;
    match config.reviewed_through {
        None => vec![Problem::NotReviewed { build }],
        Some(reviewed_through) if reviewed_through < build => vec![Problem::ReviewBehind {
            reviewed_through,
            build,
        }],
        Some(_) => Vec::new(),
    }
}
