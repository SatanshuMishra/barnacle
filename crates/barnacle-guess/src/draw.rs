use std::collections::BTreeSet;

use barnacle_catalog::Nation;
use barnacle_catalog::ShipIndex;
use barnacle_catalog::Tier;

use crate::options::RoundOptions;
use crate::reveal::Reveal;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Hint {
    Tier(Tier),
    Nation(Nation),
}

impl Hint {
    pub(crate) fn for_ship(reveal: &Reveal, options: &RoundOptions) -> Self {
        if options.spans_several_tiers() {
            Self::Tier(reveal.tier)
        } else {
            Self::Nation(reveal.nation.clone())
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Draw {
    pub(crate) options: RoundOptions,
    pub(crate) answers: BTreeSet<String>,
    pub(crate) hint: Hint,
    pub(crate) reveal: Reveal,
}

impl Draw {
    pub fn ship(&self) -> &ShipIndex {
        &self.reveal.index
    }

    pub fn options(&self) -> &RoundOptions {
        &self.options
    }

    pub fn answers(&self) -> &BTreeSet<String> {
        &self.answers
    }

    pub fn hint(&self) -> &Hint {
        &self.hint
    }

    pub fn reveal(&self) -> &Reveal {
        &self.reveal
    }
}
