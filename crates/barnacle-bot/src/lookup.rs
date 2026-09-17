use std::collections::BTreeMap;

use barnacle_catalog::Catalog;
use barnacle_catalog::ShipIndex;
use barnacle_catalog::names::clean_answer;

use crate::text;

pub const MAX_SUGGESTIONS: usize = 25;
const MAX_LABEL_CHARS: usize = 100;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Suggestion {
    pub label: String,
    pub index: ShipIndex,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Listing {
    index: ShipIndex,
    label: String,
    forms: Vec<String>,
}

impl Listing {
    fn starts_with(&self, wanted: &str) -> bool {
        self.forms.iter().any(|form| form.starts_with(wanted))
    }

    fn contains(&self, wanted: &str) -> bool {
        self.forms.iter().any(|form| form.contains(wanted))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Directory {
    listings: Vec<Listing>,
}

impl Directory {
    pub fn new(catalog: &Catalog) -> Self {
        let ordered: BTreeMap<(String, ShipIndex), Listing> = catalog
            .ships
            .iter()
            .filter_map(|ship| {
                let name = ship.name.as_ref()?;
                let display = name.display().to_owned();
                let label: String = format!(
                    "{display} ({}, {})",
                    text::tier_numeral(ship.tier),
                    text::nation_label(&ship.nation)
                )
                .chars()
                .take(MAX_LABEL_CHARS)
                .collect();
                let forms = std::iter::once(name.short.as_str())
                    .chain(name.full.as_deref())
                    .map(clean_answer)
                    .filter(|form| !form.is_empty())
                    .collect();
                let listing = Listing {
                    index: ship.index.clone(),
                    label,
                    forms,
                };
                Some(((display, ship.index.clone()), listing))
            })
            .collect();
        Self {
            listings: ordered.into_values().collect(),
        }
    }

    pub fn suggest(&self, typed: &str) -> Vec<Suggestion> {
        let wanted = clean_answer(typed);
        let leading = self
            .listings
            .iter()
            .filter(|listing| listing.starts_with(&wanted));
        let inside = self
            .listings
            .iter()
            .filter(|listing| !listing.starts_with(&wanted) && listing.contains(&wanted));
        leading
            .chain(inside)
            .take(MAX_SUGGESTIONS)
            .map(|listing| Suggestion {
                label: listing.label.clone(),
                index: listing.index.clone(),
            })
            .collect()
    }

    pub fn resolve(&self, typed: &str) -> Option<&ShipIndex> {
        let by_index = ShipIndex::parse(&typed.trim().to_ascii_uppercase())
            .ok()
            .and_then(|index| self.listings.iter().find(|listing| listing.index == index));
        let wanted = clean_answer(typed);
        by_index
            .or_else(|| {
                self.listings
                    .iter()
                    .find(|listing| !wanted.is_empty() && listing.forms.contains(&wanted))
            })
            .map(|listing| &listing.index)
    }
}
