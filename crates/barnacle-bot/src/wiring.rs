use barnacle_catalog::Tier;
use barnacle_guess::RoundOptions;

use crate::attendance::Delivery;
use crate::attendance::Target;
use crate::ids::RoleId;
use crate::schedule::Hour;
use crate::schedule::Night;
use crate::solves::Ranking;

const CANCEL_PREFIX: &str = "barnacle-cancel:";
pub const ENDED_BUTTON_ID: &str = "barnacle-cancel:ended";
pub const DEFAULT_LEADERBOARD_SIZE: u32 = 10;
pub const SIGNUP_PREFIX: &str = "barnacle-cb:";

const ALL_TARGET: &str = "all";
const ATTENDING_CHOICE: &str = "in";
const NOPE_CHOICE: &str = "out";
const SIGNUP_FIELDS: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq, poise::ChoiceParameter)]
pub enum SortChoice {
    #[name = "Most wins"]
    MostWins,
    #[name = "Fastest time"]
    FastestTime,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LeaderboardRequest {
    pub ranking: Ranking,
    pub size: u32,
}

pub fn leaderboard_request(sort: Option<SortChoice>, size: Option<u32>) -> LeaderboardRequest {
    LeaderboardRequest {
        ranking: match sort {
            Some(SortChoice::FastestTime) => Ranking::FastestTime,
            Some(SortChoice::MostWins) | None => Ranking::MostWins,
        },
        size: size.unwrap_or(DEFAULT_LEADERBOARD_SIZE),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ChannelAccess {
    pub view_channel: bool,
    pub send_messages: bool,
    pub embed_links: bool,
    pub attach_files: bool,
    pub read_message_history: bool,
    pub in_thread: bool,
}

pub fn missing_permissions(access: ChannelAccess) -> Vec<&'static str> {
    [
        (access.view_channel, "View Channel"),
        (
            access.send_messages,
            if access.in_thread {
                "Send Messages in Threads"
            } else {
                "Send Messages"
            },
        ),
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SignupClick {
    pub season: i64,
    pub night: Night,
    pub target: Target,
    pub attending: bool,
}

pub fn signup_button_id(season: i64, night: Night, target: Target, attending: bool) -> String {
    format!(
        "{}{}:{}",
        signup_tag_prefix(season, night),
        target_text(target),
        choice_text(attending)
    )
}

pub fn signup_tag_prefix(season: i64, night: Night) -> String {
    format!("{SIGNUP_PREFIX}{season}:{}:", night.label())
}

pub fn signup_click(custom_id: &str) -> Option<SignupClick> {
    let fields: Vec<&str> = custom_id.strip_prefix(SIGNUP_PREFIX)?.split(':').collect();
    let [season, night, target, choice] = <[&str; SIGNUP_FIELDS]>::try_from(fields).ok()?;
    Some(SignupClick {
        season: canonical_season(season)?,
        night: Night::parse(night)?,
        target: parse_target(target)?,
        attending: parse_choice(choice)?,
    })
}

fn canonical_season(text: &str) -> Option<i64> {
    if text.starts_with('0') || !text.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    text.parse().ok().filter(|season| *season > 0)
}

pub fn missing_signup_permissions(access: ChannelAccess) -> Vec<&'static str> {
    [
        (access.view_channel, "View Channel"),
        (access.send_messages, "Send Messages"),
        (access.embed_links, "Embed Links"),
        (access.read_message_history, "Read Message History"),
    ]
    .into_iter()
    .filter(|(granted, _)| !granted)
    .map(|(_, name)| name)
    .collect()
}

fn tier(value: Option<i64>) -> Option<Tier> {
    let value = u32::try_from(value?).ok()?;
    Tier::new(value).ok()
}

fn target_text(target: Target) -> String {
    match target {
        Target::All => ALL_TARGET.to_owned(),
        Target::One(hour) => hour.get().to_string(),
    }
}

fn choice_text(attending: bool) -> &'static str {
    if attending {
        ATTENDING_CHOICE
    } else {
        NOPE_CHOICE
    }
}

fn parse_target(text: &str) -> Option<Target> {
    if text == ALL_TARGET {
        return Some(Target::All);
    }
    Hour::ALL
        .into_iter()
        .find(|hour| hour.get().to_string() == text)
        .map(Target::One)
}

fn parse_choice(text: &str) -> Option<bool> {
    match text {
        ATTENDING_CHOICE => Some(true),
        NOPE_CHOICE => Some(false),
        _ => None,
    }
}

pub fn ping_content(ping: Option<RoleId>) -> String {
    match ping {
        Some(role) => format!("<@&{role}>"),
        None => String::new(),
    }
}

pub fn ping_allowance(delivery: Delivery, ping: Option<RoleId>) -> Vec<RoleId> {
    match (delivery, ping) {
        (Delivery::New, Some(role)) => vec![role],
        (Delivery::New, None) | (Delivery::Redraw, _) => Vec::new(),
    }
}
