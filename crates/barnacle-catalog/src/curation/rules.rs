use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::fmt;

use crate::curation::config::CurationConfig;
use crate::curation::config::ExcludeReason;
use crate::model::Catalog;
use crate::model::Ship;
use crate::model::ShipIndex;
use crate::names::name_tokens;

pub const COLLABORATION_PREFIXES: [&str; 5] = ["arp", "al", "hsf", "ba", "star"];
pub const VARIANT_SUFFIXES: [&str; 4] = ["b", "golden", "clr", "beta"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Removal {
    GroupNotAllowed,
    NoEnglishName,
    NoSilhouette,
    IdenticalSilhouette {
        base: ShipIndex,
    },
    SharesBadSilhouette {
        with: ShipIndex,
    },
    CollaborationPrefix,
    VariantSuffix {
        base: ShipIndex,
    },
    SharedHull {
        base: ShipIndex,
    },
    CopyOf {
        via: ShipIndex,
        base: ShipIndex,
    },
    Manual {
        reason: ExcludeReason,
        base: Option<ShipIndex>,
    },
}

impl Removal {
    pub fn base(&self) -> Option<&ShipIndex> {
        match self {
            Self::IdenticalSilhouette { base }
            | Self::VariantSuffix { base }
            | Self::SharedHull { base }
            | Self::CopyOf { base, .. } => Some(base),
            Self::Manual { base, .. } => base.as_ref(),
            Self::GroupNotAllowed
            | Self::NoEnglishName
            | Self::NoSilhouette
            | Self::SharesBadSilhouette { .. }
            | Self::CollaborationPrefix => None,
        }
    }
}

impl fmt::Display for Removal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::GroupNotAllowed => f.write_str("group not allowed"),
            Self::NoEnglishName => f.write_str("no English name"),
            Self::NoSilhouette => f.write_str("no silhouette"),
            Self::IdenticalSilhouette { base } => write!(f, "same silhouette as {base}"),
            Self::SharesBadSilhouette { with } => write!(
                f,
                "same silhouette as {with}, which is excluded for bad silhouette art"
            ),
            Self::CollaborationPrefix => f.write_str("collaboration reskin"),
            Self::VariantSuffix { base } => write!(f, "variant of {base}"),
            Self::SharedHull { base } => write!(f, "same hull model as {base}"),
            Self::CopyOf { via, base } => {
                write!(f, "copy of {via}, which is a copy of {base}")
            }
            Self::Manual {
                reason,
                base: Some(base),
            } => write!(f, "excluded by curation: {reason} of {base}"),
            Self::Manual { reason, base: None } => write!(f, "excluded by curation: {reason}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Curated {
    pub pool: BTreeSet<ShipIndex>,
    pub removed: BTreeMap<ShipIndex, Removal>,
}

impl Curated {
    pub fn variants_of(&self, base: &ShipIndex) -> Vec<&ShipIndex> {
        self.removed
            .iter()
            .filter(|(_, removal)| removal.base() == Some(base))
            .map(|(index, _)| index)
            .collect()
    }
}

#[derive(Clone)]
struct Candidate<'a> {
    ship: &'a Ship,
    tokens: Vec<String>,
    silhouette: &'a str,
}

impl<'a> Candidate<'a> {
    fn from_ship(ship: &'a Ship) -> Option<Self> {
        let name = ship.name.as_ref()?;
        let silhouette = ship.silhouette.as_ref()?;
        Some(Self {
            ship,
            tokens: name_tokens(name.display()),
            silhouette: &silhouette.sha256,
        })
    }

    fn index(&self) -> &'a ShipIndex {
        &self.ship.index
    }

    fn collaboration_prefix(&self) -> bool {
        self.tokens.len() > 1
            && self
                .tokens
                .first()
                .is_some_and(|token| COLLABORATION_PREFIXES.contains(&token.as_str()))
    }

    fn variant_stem(&self) -> Option<&[String]> {
        match self.tokens.split_last() {
            Some((last, stem)) if !stem.is_empty() && VARIANT_SUFFIXES.contains(&last.as_str()) => {
                Some(stem)
            }
            _ => None,
        }
    }

    fn base_rank(&self) -> (bool, u8, &'a ShipIndex) {
        let marked = self.collaboration_prefix() || self.variant_stem().is_some();
        let group = match self.ship.group.as_str() {
            "upgradeable" => 0,
            "start" => 1,
            "special" | "premium" => 2,
            _ => 3,
        };
        (marked, group, self.index())
    }
}

fn best_base<'a, 'c>(members: impl Iterator<Item = &'c Candidate<'a>>) -> Option<&'a ShipIndex>
where
    'a: 'c,
{
    members
        .min_by_key(|member| member.base_rank())
        .map(Candidate::index)
}

fn group_by<'a, 'c, K: Ord>(
    candidates: &'c [Candidate<'a>],
    key: impl Fn(&'c Candidate<'a>) -> K,
) -> BTreeMap<K, Vec<&'c Candidate<'a>>> {
    let mut groups: BTreeMap<K, Vec<&'c Candidate<'a>>> = BTreeMap::new();
    for candidate in candidates {
        groups.entry(key(candidate)).or_default().push(candidate);
    }
    groups
}

fn without<'a>(
    candidates: &[Candidate<'a>],
    removed: &BTreeMap<ShipIndex, Removal>,
) -> Vec<Candidate<'a>> {
    candidates
        .iter()
        .filter(|candidate| !removed.contains_key(candidate.index()))
        .cloned()
        .collect()
}

fn basic_removal(ship: &Ship, config: &CurationConfig) -> Option<Removal> {
    if !config.groups.contains(&ship.group) {
        Some(Removal::GroupNotAllowed)
    } else if ship.name.is_none() {
        Some(Removal::NoEnglishName)
    } else if ship.silhouette.is_none() {
        Some(Removal::NoSilhouette)
    } else {
        None
    }
}

fn shares_bad_silhouette(
    candidates: &[Candidate<'_>],
    bad: &BTreeMap<&str, &ShipIndex>,
    kept: &BTreeSet<&ShipIndex>,
    excluded: &BTreeSet<&ShipIndex>,
) -> BTreeMap<ShipIndex, Removal> {
    candidates
        .iter()
        .filter(|candidate| {
            !kept.contains(candidate.index()) && !excluded.contains(candidate.index())
        })
        .filter_map(|candidate| {
            let with = bad.get(candidate.silhouette)?;
            Some((
                candidate.index().clone(),
                Removal::SharesBadSilhouette {
                    with: (*with).clone(),
                },
            ))
        })
        .collect()
}

fn final_base(
    removed: &BTreeMap<ShipIndex, Removal>,
    pool: &BTreeSet<ShipIndex>,
    start: &ShipIndex,
) -> Option<ShipIndex> {
    std::iter::successors(Some(start), |current| {
        removed.get(*current).and_then(Removal::base)
    })
    .take(removed.len() + 1)
    .find(|candidate| pool.contains(*candidate))
    .cloned()
}

fn resolve_chains(
    removed: &BTreeMap<ShipIndex, Removal>,
    pool: &BTreeSet<ShipIndex>,
) -> BTreeMap<ShipIndex, Removal> {
    removed
        .iter()
        .map(|(index, removal)| {
            let resolved = removal
                .base()
                .filter(|via| !pool.contains(*via))
                .and_then(|via| {
                    final_base(removed, pool, via).map(|base| Removal::CopyOf {
                        via: via.clone(),
                        base,
                    })
                });
            (index.clone(), resolved.unwrap_or_else(|| removal.clone()))
        })
        .collect()
}

fn identical_silhouettes(
    candidates: &[Candidate<'_>],
    kept: &BTreeSet<&ShipIndex>,
    excluded: &BTreeSet<&ShipIndex>,
) -> BTreeMap<ShipIndex, Removal> {
    group_by(candidates, |candidate| candidate.silhouette)
        .into_values()
        .filter(|members| members.len() > 1)
        .flat_map(|members| {
            let base = best_base(members.iter().copied().filter(|member| {
                !member.collaboration_prefix() && !excluded.contains(member.index())
            }));
            members
                .into_iter()
                .filter_map(|member| {
                    let base = base?;
                    (member.index() != base && !kept.contains(member.index())).then(|| {
                        (
                            member.index().clone(),
                            Removal::IdenticalSilhouette { base: base.clone() },
                        )
                    })
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

fn collaboration_prefixes(
    candidates: &[Candidate<'_>],
    kept: &BTreeSet<&ShipIndex>,
) -> BTreeMap<ShipIndex, Removal> {
    candidates
        .iter()
        .filter(|candidate| candidate.collaboration_prefix() && !kept.contains(candidate.index()))
        .map(|candidate| (candidate.index().clone(), Removal::CollaborationPrefix))
        .collect()
}

fn variant_suffixes(
    candidates: &[Candidate<'_>],
    kept: &BTreeSet<&ShipIndex>,
    excluded: &BTreeSet<&ShipIndex>,
) -> BTreeMap<ShipIndex, Removal> {
    let by_name = group_by(candidates, |candidate| candidate.tokens.as_slice());
    candidates
        .iter()
        .filter(|candidate| !kept.contains(candidate.index()))
        .filter_map(|candidate| {
            let stem = candidate.variant_stem()?;
            let others = by_name.get(stem)?.iter().copied().filter(|other| {
                other.index() != candidate.index() && !excluded.contains(other.index())
            });
            let base = best_base(others)?;
            Some((
                candidate.index().clone(),
                Removal::VariantSuffix { base: base.clone() },
            ))
        })
        .collect()
}

fn shared_hulls(
    candidates: &[Candidate<'_>],
    kept: &BTreeSet<&ShipIndex>,
    excluded: &BTreeSet<&ShipIndex>,
) -> BTreeMap<ShipIndex, Removal> {
    group_by(candidates, |candidate| candidate.ship.hull_model.as_deref())
        .into_iter()
        .filter(|(model, members)| model.is_some() && members.len() > 1)
        .flat_map(|(_, members)| {
            let base = best_base(
                members
                    .iter()
                    .copied()
                    .filter(|member| !excluded.contains(member.index())),
            );
            members
                .into_iter()
                .filter_map(|member| {
                    let base = base?;
                    (member.index() != base && !kept.contains(member.index())).then(|| {
                        (
                            member.index().clone(),
                            Removal::SharedHull { base: base.clone() },
                        )
                    })
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

fn manual_exclusions(
    catalog: &Catalog,
    config: &CurationConfig,
    basic: &BTreeMap<ShipIndex, Removal>,
) -> BTreeMap<ShipIndex, Removal> {
    config
        .exclude
        .iter()
        .filter(|entry| catalog.get(&entry.index).is_some() && !basic.contains_key(&entry.index))
        .map(|entry| {
            (
                entry.index.clone(),
                Removal::Manual {
                    reason: entry.reason,
                    base: entry.base.clone(),
                },
            )
        })
        .collect()
}

pub fn curate(catalog: &Catalog, config: &CurationConfig) -> Curated {
    let kept: BTreeSet<&ShipIndex> = config.keep.iter().map(|entry| &entry.index).collect();
    let excluded: BTreeSet<&ShipIndex> = config.exclude.iter().map(|entry| &entry.index).collect();
    let basic: BTreeMap<ShipIndex, Removal> = catalog
        .ships
        .iter()
        .filter_map(|ship| basic_removal(ship, config).map(|removal| (ship.index.clone(), removal)))
        .collect();
    let candidates: Vec<Candidate<'_>> = catalog
        .ships
        .iter()
        .filter(|ship| !basic.contains_key(&ship.index))
        .filter_map(Candidate::from_ship)
        .collect();

    let bad: BTreeMap<&str, &ShipIndex> = config
        .exclude
        .iter()
        .filter(|entry| entry.reason == ExcludeReason::BadSilhouette)
        .filter_map(|entry| {
            let silhouette = catalog.get(&entry.index)?.silhouette.as_ref()?;
            Some((silhouette.sha256.as_str(), &entry.index))
        })
        .collect();

    let shared_bad = shares_bad_silhouette(&candidates, &bad, &kept, &excluded);
    let after_bad = without(&candidates, &shared_bad);
    let identical = identical_silhouettes(&after_bad, &kept, &excluded);
    let after_identical = without(&after_bad, &identical);
    let collaboration = collaboration_prefixes(&after_identical, &kept);
    let after_collaboration = without(&after_identical, &collaboration);
    let suffixes = variant_suffixes(&after_collaboration, &kept, &excluded);
    let after_suffixes = without(&after_collaboration, &suffixes);
    let hulls = shared_hulls(&after_suffixes, &kept, &excluded);
    let manual = manual_exclusions(catalog, config, &basic);

    let removed: BTreeMap<ShipIndex, Removal> = basic
        .into_iter()
        .chain(shared_bad)
        .chain(identical)
        .chain(collaboration)
        .chain(suffixes)
        .chain(hulls)
        .chain(manual)
        .collect();
    let pool: BTreeSet<ShipIndex> = candidates
        .iter()
        .map(|candidate| candidate.index().clone())
        .filter(|index| !removed.contains_key(index))
        .collect();
    let removed = resolve_chains(&removed, &pool);
    Curated { pool, removed }
}
