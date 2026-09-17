use barnacle_catalog::Tier;
use barnacle_guess::RoundOptions;

const CANCEL_PREFIX: &str = "barnacle-cancel:";
pub const ENDED_BUTTON_ID: &str = "barnacle-cancel:ended";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ChannelAccess {
    pub view_channel: bool,
    pub send_messages: bool,
    pub embed_links: bool,
    pub attach_files: bool,
    pub read_message_history: bool,
}

pub fn missing_permissions(access: ChannelAccess) -> Vec<&'static str> {
    [
        (access.view_channel, "View Channel"),
        (access.send_messages, "Send Messages"),
        (access.embed_links, "Embed Links"),
        (access.attach_files, "Attach Files"),
        (access.read_message_history, "Read Message History"),
    ]
    .into_iter()
    .filter(|(granted, _)| !granted)
    .map(|(_, name)| name)
    .collect()
}

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
