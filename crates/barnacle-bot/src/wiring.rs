use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::fmt;

use barnacle_catalog::Tier;
use barnacle_guess::RoundOptions;
use barnacle_guess::SeriesLength;
use jiff::civil::Date;

use crate::attendance::Delivery;
use crate::attendance::Target;
use crate::attendance::listed_events;
use crate::attendance_store::AttendanceError;
use crate::attendance_store::Event;
use crate::ids::ChannelId;
use crate::ids::GuildId;
use crate::ids::Ping;
use crate::ids::RoleId;
use crate::schedule::Hour;
use crate::schedule::Hours;
use crate::schedule::parse_day;
use crate::solves::Ranking;

const CANCEL_PREFIX: &str = "barnacle-cancel:";
const SKIP_PREFIX: &str = "barnacle-skip:";
const END_SERIES_PREFIX: &str = "barnacle-end:";
const STILL_PLAYING_PREFIX: &str = "barnacle-still:";
const ANNOUNCE_PREFIX: &str = "barnacle-announce:";
pub const ENDED_BUTTON_ID: &str = "barnacle-cancel:ended";
pub const ENDED_SKIP_BUTTON_ID: &str = "barnacle-skip:ended";
pub const ENDED_END_BUTTON_ID: &str = "barnacle-end:ended";
pub const DEFAULT_LEADERBOARD_SIZE: u32 = 10;
pub const SIGNUP_PREFIX: &str = "barnacle-cb:";
pub const CHOOSE_PREFIX: &str = "barnacle-cb-pick:";
pub const HOURS_PREFIX: &str = "barnacle-cb-hours:";
pub const PANEL_PREFIX: &str = "barnacle-event:";
pub const EVENT_NAME_LIMIT: usize = 100;

const ALL_TARGET: &str = "all";
const ATTENDING_CHOICE: &str = "in";
const NOPE_CHOICE: &str = "out";
const SIGNUP_FIELDS: usize = 4;
const SESSION_FIELDS: usize = 2;
const PANEL_FIELDS: usize = 2;
pub const SUGGESTION_LIMIT: usize = 25;
const CHOICE_LABEL_LIMIT: usize = 100;
const CUT_MARK: char = '…';

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
    lacking([
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
    ])
}

pub fn missing_update_permissions(access: ChannelAccess) -> Vec<&'static str> {
    lacking([
        (access.view_channel, "View Channel"),
        (access.send_messages, "Send Messages"),
        (access.embed_links, "Embed Links"),
    ])
}

fn lacking<const N: usize>(checks: [(bool, &'static str); N]) -> Vec<&'static str> {
    checks
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

pub fn series_request(
    min_tier: Option<i64>,
    max_tier: Option<i64>,
    historical: Option<bool>,
    rounds: Option<i64>,
) -> Option<(RoundOptions, SeriesLength)> {
    let length = match rounds {
        None => SeriesLength::default(),
        Some(rounds) => SeriesLength::new(u32::try_from(rounds).ok()?)?,
    };
    Some((round_options(min_tier, max_tier, historical), length))
}

pub fn cancel_button_id(number: u64) -> String {
    tagged_id(CANCEL_PREFIX, number)
}

pub fn round_number(custom_id: &str) -> Option<u64> {
    id_number(custom_id, CANCEL_PREFIX)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeriesClick {
    Skip(u64),
    End(u64),
    StillPlaying(u64),
}

pub fn skip_button_id(number: u64) -> String {
    tagged_id(SKIP_PREFIX, number)
}

pub fn end_series_button_id(number: u64) -> String {
    tagged_id(END_SERIES_PREFIX, number)
}

pub fn still_playing_button_id(series: u64) -> String {
    tagged_id(STILL_PLAYING_PREFIX, series)
}

pub fn series_click(custom_id: &str) -> Option<SeriesClick> {
    id_number(custom_id, SKIP_PREFIX)
        .map(SeriesClick::Skip)
        .or_else(|| id_number(custom_id, END_SERIES_PREFIX).map(SeriesClick::End))
        .or_else(|| id_number(custom_id, STILL_PLAYING_PREFIX).map(SeriesClick::StillPlaying))
}

pub fn announce_button_id(version: &str) -> String {
    tagged_id(ANNOUNCE_PREFIX, version)
}

pub fn announce_version(custom_id: &str) -> Option<String> {
    custom_id
        .strip_prefix(ANNOUNCE_PREFIX)
        .filter(|version| !version.is_empty())
        .map(str::to_owned)
}

fn tagged_id(prefix: &str, tag: impl fmt::Display) -> String {
    format!("{prefix}{tag}")
}

fn id_number(custom_id: &str, prefix: &str) -> Option<u64> {
    custom_id.strip_prefix(prefix)?.parse().ok()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SignupClick {
    pub event: i64,
    pub date: Date,
    pub target: Target,
    pub attending: bool,
}

pub fn signup_button_id(event: i64, date: Date, target: Target, attending: bool) -> String {
    format!(
        "{}{}:{}",
        signup_tag_prefix(event, date),
        target_text(target),
        choice_text(attending)
    )
}

pub fn signup_tag_prefix(event: i64, date: Date) -> String {
    format!("{SIGNUP_PREFIX}{event}:{date}:")
}

pub fn signup_click(custom_id: &str) -> Option<SignupClick> {
    let fields: Vec<&str> = custom_id.strip_prefix(SIGNUP_PREFIX)?.split(':').collect();
    let [event, date, target, choice] = <[&str; SIGNUP_FIELDS]>::try_from(fields).ok()?;
    Some(SignupClick {
        event: canonical_id(event)?,
        date: parse_day(date)?,
        target: parse_target(target)?,
        attending: parse_choice(choice)?,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionRef {
    pub event: i64,
    pub date: Date,
}

pub fn choose_button_id(event: i64, date: Date) -> String {
    format!("{CHOOSE_PREFIX}{event}:{date}")
}

pub fn hours_menu_id(event: i64, date: Date) -> String {
    format!("{HOURS_PREFIX}{event}:{date}")
}

pub fn choose_press(custom_id: &str) -> Option<SessionRef> {
    session_ref(custom_id, CHOOSE_PREFIX)
}

pub fn hours_menu(custom_id: &str) -> Option<SessionRef> {
    session_ref(custom_id, HOURS_PREFIX)
}

fn session_ref(custom_id: &str, prefix: &str) -> Option<SessionRef> {
    let fields: Vec<&str> = custom_id.strip_prefix(prefix)?.split(':').collect();
    let [event, date] = <[&str; SESSION_FIELDS]>::try_from(fields).ok()?;
    Some(SessionRef {
        event: canonical_id(event)?,
        date: parse_day(date)?,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelPart {
    Days,
    Roles,
    Everyone,
    Save,
    Cancel,
}

impl PanelPart {
    const ALL: [PanelPart; 5] = [
        PanelPart::Days,
        PanelPart::Roles,
        PanelPart::Everyone,
        PanelPart::Save,
        PanelPart::Cancel,
    ];

    fn text(self) -> &'static str {
        match self {
            PanelPart::Days => "days",
            PanelPart::Roles => "roles",
            PanelPart::Everyone => "everyone",
            PanelPart::Save => "save",
            PanelPart::Cancel => "cancel",
        }
    }
}

pub fn panel_id(panel: u64, part: PanelPart) -> String {
    format!("{PANEL_PREFIX}{panel}:{}", part.text())
}

pub fn panel_part(custom_id: &str) -> Option<(u64, PanelPart)> {
    let fields: Vec<&str> = custom_id.strip_prefix(PANEL_PREFIX)?.split(':').collect();
    let [panel, part] = <[&str; PANEL_FIELDS]>::try_from(fields).ok()?;
    let panel = canonical_id(panel).and_then(|id| u64::try_from(id).ok())?;
    let part = PanelPart::ALL
        .into_iter()
        .find(|known| known.text() == part)?;
    Some((panel, part))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FoundChannel {
    pub guild: GuildId,
    pub id: ChannelId,
    pub text: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChannelProblem {
    Unknown,
    NotText,
}

pub fn event_channel(
    guild: GuildId,
    found: Option<FoundChannel>,
) -> Result<ChannelId, ChannelProblem> {
    match found {
        Some(found) if found.guild == guild && found.text => Ok(found.id),
        Some(found) if found.guild == guild => Err(ChannelProblem::NotText),
        Some(_) | None => Err(ChannelProblem::Unknown),
    }
}

pub fn hour_choices(values: &[String], hours: Hours) -> Option<Vec<Hour>> {
    let chosen: Vec<Hour> = values
        .iter()
        .map(|value| parse_hour(value).filter(|hour| hours.holds(*hour)))
        .collect::<Option<Vec<Hour>>>()?;
    let distinct: BTreeSet<Hour> = chosen.iter().copied().collect();
    (distinct.len() == chosen.len()).then(|| distinct.into_iter().collect())
}

fn canonical_id(text: &str) -> Option<i64> {
    if text.starts_with('0') || !text.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    text.parse().ok().filter(|id| *id > 0)
}

pub fn event_name(text: &str) -> Option<String> {
    let name = text.trim();
    (1..=EVENT_NAME_LIMIT)
        .contains(&name.chars().count())
        .then(|| name.to_owned())
}

pub fn same_name(a: &str, b: &str) -> bool {
    a.to_lowercase() == b.to_lowercase()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventChoice {
    pub label: String,
    pub value: String,
}

pub fn event_choices(events: &[Event], typed: &str, now_unix: i64) -> Vec<EventChoice> {
    if typed.chars().count() > EVENT_NAME_LIMIT {
        return Vec::new();
    }
    let wanted = typed.to_lowercase();
    listed_events(events, now_unix)
        .into_iter()
        .filter(|event| event.name.to_lowercase().contains(&wanted))
        .map(|event| ((event.span.first_day(), event.id), event))
        .collect::<BTreeMap<(Date, i64), &Event>>()
        .into_values()
        .take(SUGGESTION_LIMIT)
        .map(event_choice)
        .collect()
}

fn event_choice(event: &Event) -> EventChoice {
    let dates = format!(" · {} to {}", event.span.first_day(), event.span.last_day());
    let room = CHOICE_LABEL_LIMIT - dates.chars().count();
    let name = if event.name.chars().count() > room {
        event
            .name
            .chars()
            .take(room - 1)
            .chain(std::iter::once(CUT_MARK))
            .collect()
    } else {
        event.name.clone()
    };
    EventChoice {
        label: format!("{name}{dates}"),
        value: event.id.to_string(),
    }
}

pub fn event_suggestions(
    read: Result<Vec<Event>, AttendanceError>,
    typed: &str,
    now_unix: i64,
) -> Vec<EventChoice> {
    read.map(|events| event_choices(&events, typed, now_unix))
        .unwrap_or_default()
}

pub fn chosen_event<'a>(events: &'a [Event], text: &str, now_unix: i64) -> Option<&'a Event> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    let running = listed_events(events, now_unix);
    canonical_id(text)
        .and_then(|id| running.iter().copied().find(|event| event.id == id))
        .or_else(|| {
            running
                .into_iter()
                .find(|event| same_name(&event.name, text))
        })
}

pub fn missing_signup_permissions(access: ChannelAccess) -> Vec<&'static str> {
    lacking([
        (access.view_channel, "View Channel"),
        (access.send_messages, "Send Messages"),
        (access.embed_links, "Embed Links"),
        (access.read_message_history, "Read Message History"),
    ])
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
    parse_hour(text).map(Target::One)
}

fn parse_hour(text: &str) -> Option<Hour> {
    text.parse()
        .ok()
        .and_then(Hour::new)
        .filter(|hour| hour.get().to_string() == text)
}

fn parse_choice(text: &str) -> Option<bool> {
    match text {
        ATTENDING_CHOICE => Some(true),
        NOPE_CHOICE => Some(false),
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PingChoiceProblem {
    MixedEveryone,
}

pub fn ping_choice(
    guild: GuildId,
    given: [Option<RoleId>; 5],
) -> Result<Vec<RoleId>, PingChoiceProblem> {
    let roles = given
        .into_iter()
        .flatten()
        .fold(Vec::new(), |kept: Vec<RoleId>, role| {
            if kept.contains(&role) {
                kept
            } else {
                kept.into_iter().chain(std::iter::once(role)).collect()
            }
        });
    let everyone = RoleId::new(guild.get());
    if roles.len() > 1 && roles.contains(&everyone) {
        Err(PingChoiceProblem::MixedEveryone)
    } else {
        Ok(roles)
    }
}

pub fn ping_content(pings: &[Ping]) -> String {
    pings
        .iter()
        .map(|ping| match ping {
            Ping::Everyone => "@everyone".to_owned(),
            Ping::Role(role) => format!("<@&{role}>"),
        })
        .collect::<Vec<String>>()
        .join(" ")
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PingAllowance {
    pub everyone: bool,
    pub roles: Vec<RoleId>,
}

pub fn ping_allowance(delivery: Delivery, pings: &[Ping]) -> PingAllowance {
    match delivery {
        Delivery::New => PingAllowance {
            everyone: pings.contains(&Ping::Everyone),
            roles: pings
                .iter()
                .filter_map(|ping| match ping {
                    Ping::Role(role) => Some(*role),
                    Ping::Everyone => None,
                })
                .collect(),
        },
        Delivery::Redraw => PingAllowance::default(),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PingReach {
    pub role_mentionable: bool,
    pub server_grant: bool,
    pub channel_grant: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PingVerdict {
    Sounds,
    BlockedByChannel,
    Silent,
    Unchecked,
}

pub fn ping_verdict(ping: Ping, reach: Option<PingReach>) -> PingVerdict {
    let Some(reach) = reach else {
        return PingVerdict::Unchecked;
    };
    let mentionable = matches!(ping, Ping::Role(_)) && reach.role_mentionable;
    if mentionable || reach.channel_grant {
        PingVerdict::Sounds
    } else if reach.server_grant {
        PingVerdict::BlockedByChannel
    } else {
        PingVerdict::Silent
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SetupPing {
    Proceed,
    Warn,
    Refuse,
}

pub fn setup_ping(creating: bool, verdict: PingVerdict) -> SetupPing {
    match verdict {
        PingVerdict::Sounds => SetupPing::Proceed,
        PingVerdict::BlockedByChannel if creating => SetupPing::Refuse,
        PingVerdict::BlockedByChannel | PingVerdict::Silent | PingVerdict::Unchecked => {
            SetupPing::Warn
        }
    }
}

pub fn unheard_pings(
    pings: &[Ping],
    mention_roles: &[RoleId],
    mention_everyone: bool,
) -> Vec<Ping> {
    pings
        .iter()
        .copied()
        .filter(|ping| match ping {
            Ping::Role(role) => !mention_roles.contains(role),
            Ping::Everyone => !mention_everyone,
        })
        .collect()
}

pub fn may_ping_again(
    pings: &[Ping],
    person_mentions_everyone: Option<bool>,
    mentionable: &[bool],
) -> bool {
    match person_mentions_everyone {
        None => false,
        Some(true) => true,
        Some(false) => {
            pings.len() == mentionable.len()
                && pings.iter().all(|ping| matches!(ping, Ping::Role(_)))
                && mentionable.iter().all(|mentionable| *mentionable)
        }
    }
}
