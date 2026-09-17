use barnacle_catalog::Tier;
use barnacle_guess::RoundOptions;

const CANCEL_PREFIX: &str = "barnacle-cancel:";
pub const ENDED_BUTTON_ID: &str = "barnacle-cancel:ended";

pub fn round_options(
    min_tier: Option<i64>,
    max_tier: Option<i64>,
    historical: Option<bool>,
) -> RoundOptions {
    RoundOptions::new(tier(min_tier), tier(max_tier), historical)
}

pub fn cancel_button_id(number: u64) -> String {
    format!("{CANCEL_PREFIX}{number}")
}

pub fn round_number(custom_id: &str) -> Option<u64> {
    custom_id.strip_prefix(CANCEL_PREFIX)?.parse().ok()
}

fn tier(value: Option<i64>) -> Option<Tier> {
    let value = u32::try_from(value?).ok()?;
    Tier::new(value).ok()
}
