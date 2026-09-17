use barnacle_catalog::Tier;

const DEFAULT_MIN_TIER: u32 = 6;
const DEFAULT_MAX_TIER: u32 = 11;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RoundOptions {
    min_tier: Tier,
    max_tier: Tier,
    historical: bool,
}

impl RoundOptions {
    pub fn new(min_tier: Option<Tier>, max_tier: Option<Tier>, historical: Option<bool>) -> Self {
        let first = min_tier.unwrap_or_else(|| default_tier(DEFAULT_MIN_TIER));
        let second = max_tier.unwrap_or_else(|| default_tier(DEFAULT_MAX_TIER));
        Self {
            min_tier: first.min(second),
            max_tier: first.max(second),
            historical: historical.unwrap_or(false),
        }
    }

    pub fn min_tier(&self) -> Tier {
        self.min_tier
    }

    pub fn max_tier(&self) -> Tier {
        self.max_tier
    }

    pub fn historical(&self) -> bool {
        self.historical
    }

    pub fn spans_several_tiers(&self) -> bool {
        self.min_tier != self.max_tier
    }

    pub fn allows(&self, tier: Tier, is_paper: bool) -> bool {
        (self.min_tier..=self.max_tier).contains(&tier) && !(self.historical && is_paper)
    }
}

impl Default for RoundOptions {
    fn default() -> Self {
        Self::new(None, None, None)
    }
}

fn default_tier(value: u32) -> Tier {
    Tier::new(value).expect("default tiers are within 1-11")
}
