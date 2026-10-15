use std::collections::BTreeSet;
use std::time::Duration;

use barnacle_catalog::Nation;
use barnacle_catalog::Provenance;
use barnacle_catalog::ShipClass;
use barnacle_catalog::Tier;
use barnacle_guess::Hint;
use barnacle_guess::Reveal;
use barnacle_guess::RoundOptions;
use barnacle_guess::SeriesLength;
use barnacle_guess::SeriesStanding;
use barnacle_guess::Tally;
use barnacle_guess::Timing;
use barnacle_guess::UserId;
use jiff::civil::Date;
use jiff::civil::Weekday;

use crate::attendance::Cell;
use crate::attendance::HourTally;
use crate::attendance::PostsReport;
use crate::attendance::PurgeReport;
use crate::attendance::RosterRow;
use crate::attendance::SignupView;
use crate::attendance::TickReport;
use crate::attendance::next_post_at;
use crate::attendance_store::Event;
use crate::event_draft::PANEL_LIFETIME_MINUTES;
use crate::event_draft::ROLE_LIMIT;
use crate::event_draft::Template;
use crate::failure::join_names;
use crate::ids::ChannelId;
use crate::ids::Ping;
use crate::schedule::Hour;
use crate::schedule::Hours;
use crate::schedule::MAX_HOURS;
use crate::schedule::Session;
use crate::schedule::Span;
use crate::schedule::Weekdays;
use crate::solves::Ranking;
use crate::solves::Standing;
use crate::table::COUNTDOWN_SECONDS;
use crate::table::Ending;
use crate::voice_text;
use crate::wiring::EVENT_NAME_LIMIT;
use crate::wiring::PingVerdict;

pub const EMBED_COLOUR: u32 = 0x2E6F6B;
pub const ROUND_TITLE: &str = "Name that ship";
pub const ROUND_DESCRIPTION: &str = "First correct answer in chat wins.";
pub const TIERS_FIELD: &str = "Tiers";
pub const PAPER_EXCLUDED: &str = "Paper ships excluded";
pub const CANCEL_LABEL: &str = "Cancel";
pub const CANCEL_REFUSED: &str =
    "Only the player who started this round, or someone who can manage messages, can end it.";
pub const ROUND_OVER: &str = "That round has already ended.";
pub const SERIES_FIELD: &str = "Series";
pub const SERIES_CHECK: &str =
    "### Are you still playing?\nPress **Yes** within 10 seconds to keep the series going.";
pub const SERIES_EXPIRED: &str = "### Nobody answered, so the series ended.";
pub const SKIP_LABEL: &str = "Skip";
pub const END_SERIES_LABEL: &str = "End series";
pub const STILL_PLAYING_LABEL: &str = "Yes";
pub const SERIES_REFUSED: &str =
    "Only the player who started this series, or someone who can manage messages, can do that.";
pub const UPDATES_NOT_SET_UP: &str = "This server has no updates channel yet. Someone with Manage Server can set one with /updates setup.";
pub const ANNOUNCE_OWNERS_ONLY: &str = "Only the owner of Barnacle can post its update notes.";
pub const SEND_LABEL: &str = "Send";
pub const SOMETHING_WENT_WRONG: &str = "Something went wrong. Nothing was changed.";
pub const ROUNDS_WON: &str = "Rounds won here";
pub const BEST_TIME: &str = "Best time here";
pub const NO_WINS_HERE: &str = "No rounds won here yet.";
pub const ABOUT_TITLE: &str = "About Barnacle";
pub const ABOUT_SUMMARY: &str = "Barnacle is a Discord bot for World of Warships players. Its first game asks you to name a ship from its silhouette.";
pub const SOURCE_URL: &str = "https://github.com/SatanshuMishra/barnacle";
pub const WARGAMING_NOTICE: &str = "Barnacle is an unofficial fan project. It is not affiliated with, endorsed by, or supported by Wargaming. World of Warships, its ship names and its ship silhouettes are trademarks or copyrighted works of Wargaming.";
pub const FIELD_LIMIT: usize = 1024;
pub const EMBED_DESCRIPTION_LIMIT: usize = 4096;
pub const MESSAGE_EMBEDS_LIMIT: usize = 6000;
pub const MESSAGE_CONTENT_LIMIT: usize = 2000;
pub const FORMER_MEMBER: &str = "Former member";
pub const ATTEND_ALL_LABEL: &str = "Attend all";
pub const NOPE_ALL_LABEL: &str = "Nope all";
pub const NO_ANSWERS_YET: &str = "No one has answered yet.";
pub const SIGNUPS_CLOSE_AT_START: &str = "Sign-ups close when the session starts.";
pub const SIGNUPS_CLOSED: &str = "Sign-ups closed";
pub const SESSION_GONE: &str = "This session no longer exists.";
pub const NO_EVENT_HERE: &str = "No event is set up here.";
pub const CHOOSE_HOURS_LABEL: &str = "Choose hours";
pub const HOURS_REFUSED: &str = "Those hours are not part of this session. Nothing was saved.";
pub const SHORT_EVENT_HOURS: u8 = 4;
pub const CLEAR_FAILED_MOVE: &str = "Some sign-up posts could not be removed from the old channel, so nothing moved. Check that the bot can manage messages there, then run this again.";
pub const CLEAR_FAILED_END: &str = "Some sign-up posts could not be removed, so the event is still running. Check that the bot can manage messages in its channel, then run this again.";
pub const REHEARSAL_RUNNING: &str = "A rehearsal is still running here. /rehearse reset ends it, and then /rehearse start sets up the next one.";
pub const FAILURES_LOGGED: &str = "Barnacle's log records the reason for each.";
pub const PANEL_CANCELLED: &str = "Cancelled. Nothing was saved.";
pub const CREATE_LABEL: &str = "Create";
pub const SAVE_LABEL: &str = "Save";
pub const WEEKDAYS_PLACEHOLDER: &str = "Weekdays";
pub const ROLES_PLACEHOLDER: &str = "Roles to ping";
const CLEAR_FAILED_EDIT: &str = "Some sign-up posts outside the new schedule could not be cleared; edit the event again to retry.";
const REFRESH_FAILED: &str =
    "Some sign-up posts could not be redrawn; edit the event again once Barnacle can edit them.";
const NO_SESSION_IN_SPAN: &str = "No session falls within those dates.";
const RESET_BLOCKED_HEAD: &str =
    "Some sign-up posts could not be removed, so no event or answer was deleted.";
const RESET_BLOCKED_TAIL: &str =
    "Check that the bot can manage messages here, then run this again.";

const MENTION_PERMISSION: &str = "Mention @everyone, @here, and All Roles";
const MENTIONABLE_SETTING: &str = "Allow anyone to @mention this role";

const SERIES_TITLE: &str = "## Silhouette series";
const SERIES_OVER: &str = "## Series over";
const HISTORICAL_SERIES: &str = "Ships that were never built are left out.";
const MOST_ROUNDS_WON: &str = "Most rounds won";
const FASTEST_TIME: &str = "Fastest time";
const NO_SERIES_WINS_YET: &str = "Nobody has won a round in this series yet.";
const NO_SERIES_WINS: &str = "Nobody won a round in this series.";

const NOTHING_AHEAD: &str = "Nothing further will post.";
const EVERY_DAY: &str = "every day";
const EVENT_LIST_TAIL: usize = 32;
const NOTHING_HAPPENED: &str = "Nothing happened.";
const NOTHING_TO_CLEAR: &str = "Nothing was there to clear.";
const NAME_LIMIT: usize = 32;
const CELL_WIDTH: usize = 5;
const NUMERALS: [&str; 11] = [
    "I", "II", "III", "IV", "V", "VI", "VII", "VIII", "IX", "X", "XI",
];
const MARKDOWN: [char; 7] = ['\\', '*', '_', '~', '`', '|', '['];

pub fn tier_numeral(tier: Tier) -> &'static str {
    NUMERALS[tier.get() as usize - 1]
}

pub fn tier_range(options: &RoundOptions) -> String {
    let low = tier_numeral(options.min_tier());
    if options.spans_several_tiers() {
        format!("{low}-{}", tier_numeral(options.max_tier()))
    } else {
        low.to_owned()
    }
}

pub fn class_label(class: &ShipClass) -> String {
    match class {
        ShipClass::Destroyer => "destroyer".to_owned(),
        ShipClass::Cruiser => "cruiser".to_owned(),
        ShipClass::Battleship => "battleship".to_owned(),
        ShipClass::AircraftCarrier => "aircraft carrier".to_owned(),
        ShipClass::Submarine => "submarine".to_owned(),
        ShipClass::Other(name) => name.clone(),
        ShipClass::Unspecified => "unknown class".to_owned(),
    }
}

pub fn nation_label(nation: &Nation) -> String {
    match nation.as_str() {
        "USA" => "U.S.A.".to_owned(),
        "United_Kingdom" => "U.K.".to_owned(),
        "Russia" => "U.S.S.R.".to_owned(),
        "Pan_Asia" => "Pan-Asia".to_owned(),
        "Pan_America" => "Pan-America".to_owned(),
        "Events" => "Event".to_owned(),
        other => other.replace('_', " "),
    }
}

pub fn seconds(elapsed: Duration) -> String {
    format!("{}.{:03} s", elapsed.as_secs(), elapsed.subsec_millis())
}

pub fn escape(text: &str) -> String {
    text.chars()
        .flat_map(|c| {
            let slash = MARKDOWN.contains(&c).then_some('\\');
            slash.into_iter().chain(std::iter::once(c))
        })
        .collect()
}

pub fn round_footer(timing: Timing) -> String {
    format!("Hint in {} seconds", timing.before_hint.as_secs())
}

pub fn ship_line(reveal: &Reveal) -> String {
    format!(
        "**{}**, tier {} {}, {}",
        escape(&reveal.name),
        tier_numeral(reveal.tier),
        class_label(&reveal.class),
        nation_label(&reveal.nation)
    )
}

pub fn hint(hint: &Hint) -> String {
    match hint {
        Hint::Tier(tier) => sentence(&format!("Hint: it's tier {}", tier_numeral(*tier))),
        Hint::Nation(nation) => sentence(&format!("Hint: it's from {}", nation_label(nation))),
    }
}

pub fn win(reveal: &Reveal, elapsed: Duration, personal_best: Option<bool>) -> String {
    let result = format!(
        "{} Solved in {}.",
        sentence(&format!("Correct: {}", ship_line(reveal))),
        seconds(elapsed)
    );
    if personal_best == Some(true) {
        format!("{result} New personal best.")
    } else {
        result
    }
}

pub fn timed_out(reveal: &Reveal) -> String {
    format!("Nobody named it. {}", it_was(reveal))
}

pub fn cancelled(by: UserId, reveal: &Reveal) -> String {
    acted(by, "ended the round", reveal)
}

pub fn skipped(by: UserId, reveal: &Reveal) -> String {
    acted(by, "skipped this round", reveal)
}

pub fn series_ended(by: UserId, reveal: &Reveal) -> String {
    acted(by, "ended the series", reveal)
}

pub fn ending_result(ending: &Ending) -> String {
    match ending {
        Ending::Solved {
            solve,
            reveal,
            personal_best,
            ..
        } => win(reveal, solve.elapsed, *personal_best),
        Ending::TimedOut { reveal } => timed_out(reveal),
        Ending::Cancelled { by, reveal } => cancelled(*by, reveal),
    }
}

fn acted(by: UserId, action: &str, reveal: &Reveal) -> String {
    format!("<@{}> {action}. {}", by.get(), it_was(reveal))
}

fn it_was(reveal: &Reveal) -> String {
    sentence(&format!("It was {}", ship_line(reveal)))
}

pub fn series_header(options: &RoundOptions, length: SeriesLength, starter: UserId) -> String {
    let tiers = if options.spans_several_tiers() {
        "Tiers"
    } else {
        "Tier"
    };
    let historical = if options.historical() {
        HISTORICAL_SERIES
    } else {
        ""
    };
    join_filled(
        &[
            SERIES_TITLE,
            &format!(
                "**{} rounds** | {tiers} **{}**",
                length.get(),
                tier_range(options)
            ),
            historical,
            &format!("Started by <@{}>", starter.get()),
        ],
        "\n",
    )
}

pub fn series_intro(
    options: &RoundOptions,
    length: SeriesLength,
    starter: UserId,
    remaining: u32,
) -> String {
    format!(
        "{}\n{}",
        series_header(options, length, starter),
        series_countdown(1, length, remaining)
    )
}

pub fn series_countdown(round: u32, length: SeriesLength, remaining: u32) -> String {
    let label = series_round_label(round, length);
    match remaining {
        COUNTDOWN_SECONDS => format!("### {label} starts in {remaining} seconds"),
        remaining => format!("### {label} starts in {remaining}..."),
    }
}

pub fn series_round_label(round: u32, length: SeriesLength) -> String {
    format!("Round {round} of {}", length.get())
}

pub fn series_standings(tally: &Tally, round: u32, length: SeriesLength) -> String {
    series_summary(
        format!("### Standings after round {round} of {}", length.get()),
        tally,
        NO_SERIES_WINS_YET,
        |label| format!("**{label}**"),
    )
}

pub fn series_overview(tally: &Tally, played: u32, length: SeriesLength) -> String {
    series_summary(
        format!(
            "{SERIES_OVER}\n**{played} of {} rounds played**",
            length.get()
        ),
        tally,
        NO_SERIES_WINS,
        |label| format!("### {label}"),
    )
}

pub fn series_message(parts: &[&str]) -> String {
    join_filled(parts, "\n")
}

fn series_summary(
    heading: String,
    tally: &Tally,
    nobody: &str,
    label: impl Fn(&str) -> String,
) -> String {
    let lists = if tally.is_empty() {
        vec![nobody.to_owned()]
    } else {
        std::iter::once(label(MOST_ROUNDS_WON))
            .chain(series_list(&tally.by_wins(), |standing| {
                wins(u64::from(standing.wins))
            }))
            .chain(std::iter::once(label(FASTEST_TIME)))
            .chain(series_list(&tally.by_time(), |standing| {
                seconds(standing.best)
            }))
            .collect()
    };
    std::iter::once(heading)
        .chain(lists)
        .collect::<Vec<String>>()
        .join("\n")
}

fn series_list(
    standings: &[SeriesStanding],
    value: impl Fn(&SeriesStanding) -> String,
) -> Vec<String> {
    standings
        .iter()
        .enumerate()
        .map(|(rank, standing)| {
            format!(
                "**{}.** <@{}> · {}",
                rank + 1,
                standing.user.get(),
                value(standing)
            )
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    WrongChannelType,
    MissingBotPermissions {
        names: Vec<&'static str>,
        channel: Option<ChannelId>,
    },
    DateFormat,
    LastDayBeforeFirst,
    DatesTooFar,
    NoSessionsLeft,
    NameTaken(String),
    NameFormat,
    EventNotFound,
    NothingToChange,
    RehearsalOnly,
    RehearsalRunning,
    NothingPending,
    RoundAlreadyRunning,
    EmptyPool(RoundOptions),
    NoShipMatches,
    NameEmpty,
    NameTooLong {
        limit: usize,
    },
    NotAHub,
    CategoryAndTopLevel,
    HubNothingToChange,
    PingBlockedByChannel {
        pings: Vec<Ping>,
        channel: ChannelId,
    },
    PingMixesEveryone,
    NothingToRepost {
        name: String,
        next_post_at: Option<i64>,
        sessions_left: usize,
    },
    RepostNothingToPing,
    RepostPingNotAllowed,
    RepostPingsChanged,
    RepostSuperseded,
    SeriesRoundsOutOfRange,
    TemplateUnknown,
    MissingOptions(Vec<&'static str>),
    StartFormat,
    HoursOutOfRange,
    ZoneUnknown,
    WeekdaysInvalid,
    TooManyRoles,
    PanelUnknown,
    PanelNotYours,
    PanelExpired,
    ChannelUnknown,
}

impl Refusal {
    pub fn code(&self) -> &'static str {
        match self {
            Refusal::WrongChannelType => "wrong_channel_type",
            Refusal::MissingBotPermissions { .. } => "missing_bot_permissions",
            Refusal::DateFormat => "date_format",
            Refusal::LastDayBeforeFirst => "last_day_before_first",
            Refusal::DatesTooFar => "dates_too_far",
            Refusal::NoSessionsLeft => "no_sessions_left",
            Refusal::NameTaken(_) => "name_taken",
            Refusal::NameFormat => "name_format",
            Refusal::EventNotFound => "event_not_found",
            Refusal::NothingToChange => "nothing_to_change",
            Refusal::RehearsalOnly => "rehearsal_only",
            Refusal::RehearsalRunning => "rehearsal_running",
            Refusal::NothingPending => "nothing_pending",
            Refusal::RoundAlreadyRunning => "round_already_running",
            Refusal::EmptyPool(_) => "empty_pool",
            Refusal::NoShipMatches => "no_ship_matches",
            Refusal::NameEmpty => "name_empty",
            Refusal::NameTooLong { .. } => "name_too_long",
            Refusal::NotAHub => "not_a_hub",
            Refusal::CategoryAndTopLevel => "category_and_top_level",
            Refusal::HubNothingToChange => "hub_nothing_to_change",
            Refusal::PingBlockedByChannel { .. } => "ping_blocked_by_channel",
            Refusal::PingMixesEveryone => "ping_mixes_everyone",
            Refusal::NothingToRepost { .. } => "nothing_to_repost",
            Refusal::RepostNothingToPing => "repost_nothing_to_ping",
            Refusal::RepostPingNotAllowed => "repost_ping_not_allowed",
            Refusal::RepostPingsChanged => "repost_pings_changed",
            Refusal::RepostSuperseded => "repost_superseded",
            Refusal::SeriesRoundsOutOfRange => "series_rounds_out_of_range",
            Refusal::TemplateUnknown => "template_unknown",
            Refusal::MissingOptions(_) => "missing_options",
            Refusal::StartFormat => "start_format",
            Refusal::HoursOutOfRange => "hours_out_of_range",
            Refusal::ZoneUnknown => "zone_unknown",
            Refusal::WeekdaysInvalid => "weekdays_invalid",
            Refusal::TooManyRoles => "too_many_roles",
            Refusal::PanelUnknown => "panel_unknown",
            Refusal::PanelNotYours => "panel_not_yours",
            Refusal::PanelExpired => "panel_expired",
            Refusal::ChannelUnknown => "channel_unknown",
        }
    }

    pub fn message(&self) -> String {
        match self {
            Refusal::WrongChannelType => {
                "Sign-up posts go in a text channel. Run this in a text channel.".to_owned()
            }
            Refusal::MissingBotPermissions { names, channel } => {
                missing_bot_permissions(names, *channel)
            }
            Refusal::DateFormat => {
                "Dates look like 2026-09-16. Run the command again with the dates written that way."
                    .to_owned()
            }
            Refusal::LastDayBeforeFirst => {
                "The last day is before the first day. Give a last day on or after the first day."
                    .to_owned()
            }
            Refusal::DatesTooFar => {
                "An event has to sit within six months either side of today. Pick dates closer to today."
                    .to_owned()
            }
            Refusal::NoSessionsLeft => {
                "Those dates hold no session still ahead. Pick dates with a session still ahead."
                    .to_owned()
            }
            Refusal::NameTaken(name) => format!(
                "An event named {} is still running here. Pick another name, or end that event first with /event end.",
                escape(name)
            ),
            Refusal::NameFormat => format!(
                "An event's name is 1 to {EVENT_NAME_LIMIT} characters. Run the command again with a name that long."
            ),
            Refusal::EventNotFound => {
                "No running event here matches that. Pick one of the events suggested as you type; /event list shows the events that are set up."
                    .to_owned()
            }
            Refusal::NothingToChange => {
                "Nothing to change. Give a new name, dates, start, hours, time zone or channel, or change the weekdays or pings in the panel."
                    .to_owned()
            }
            Refusal::RehearsalOnly => {
                "This server is not set up for rehearsals. Whoever runs Barnacle can list it under [rehearsal] in barnacle.toml, then restart Barnacle."
                    .to_owned()
            }
            Refusal::RehearsalRunning => REHEARSAL_RUNNING.to_owned(),
            Refusal::NothingPending => {
                "Nothing is waiting to happen in this rehearsal. /rehearse start sets up another rehearsal."
                    .to_owned()
            }
            Refusal::RoundAlreadyRunning => {
                "This channel already has a round running. Name the ship or wait for the round to end, then start another."
                    .to_owned()
            }
            Refusal::EmptyPool(options) => empty_pool(options),
            Refusal::NoShipMatches => {
                "No ship matches that. Pick one of the names suggested as you type.".to_owned()
            }
            Refusal::NameEmpty => {
                "A name needs at least one character that is not a space. Run the command again with a name."
                    .to_owned()
            }
            Refusal::NameTooLong { limit } => {
                format!("{} Give a shorter one.", voice_text::name_too_long(*limit))
            }
            Refusal::NotAHub => {
                "That channel is not a Join to Create channel in this server. `/voice hub list` shows the ones that are."
                    .to_owned()
            }
            Refusal::CategoryAndTopLevel => {
                "Pick a category or top_level, not both.".to_owned()
            }
            Refusal::HubNothingToChange => {
                "Nothing to change. Give a new name, room_name, category or top_level.".to_owned()
            }
            Refusal::PingBlockedByChannel { pings, channel } => {
                ping_blocked_by_channel(pings, *channel)
            }
            Refusal::PingMixesEveryone => {
                "@everyone already pings every member, so it cannot be combined with other roles. Pick @everyone alone, or name roles without it."
                    .to_owned()
            }
            Refusal::NothingToRepost {
                name,
                next_post_at,
                sessions_left,
            } => nothing_to_repost(name, *next_post_at, *sessions_left),
            Refusal::RepostNothingToPing => {
                "This event pings no role, so ping_again has nothing to ping. Pick roles with /event edit, or repost without ping_again."
                    .to_owned()
            }
            Refusal::RepostPingNotAllowed => format!(
                "ping_again pings the event's roles in your name, so you need to be able to ping them yourself: hold {MENTION_PERMISSION} in the event's channel, or every role has to allow anyone to mention it. You can still repost without ping_again."
            ),
            Refusal::RepostPingsChanged => {
                "The event's ping roles or channel changed while reposting, so nothing was reposted. Run the command again."
                    .to_owned()
            }
            Refusal::RepostSuperseded => {
                "The sign-up post changed while it was being reposted, so nothing was reposted. Check the event with /event list, then run the command again if it is still needed."
                    .to_owned()
            }
            Refusal::SeriesRoundsOutOfRange => format!(
                "A series is {} to {} rounds.",
                SeriesLength::MIN,
                SeriesLength::MAX
            ),
            Refusal::TemplateUnknown => format!(
                "Barnacle offers one template, {}. Pick it from the list, or leave template out and give start, hours and time_zone.",
                Template::ClanBattles.name()
            ),
            Refusal::MissingOptions(names) => format!(
                "Without a template, an event needs {}. Run the command again with {}, or pick the {} template.",
                join_names(names),
                them(names.len()),
                Template::ClanBattles.name()
            ),
            Refusal::StartFormat => {
                "A start time is written on a 24-hour clock, such as 17:00 or 5:30. Run the command again with the start written that way."
                    .to_owned()
            }
            Refusal::HoursOutOfRange => format!(
                "An event runs 1 to {MAX_HOURS} hours. Run the command again with hours from 1 to {MAX_HOURS}."
            ),
            Refusal::ZoneUnknown => {
                "Barnacle knows a time zone by its place, such as America/Denver or Europe/Berlin, or as UTC. Pick one of the zones suggested as you type."
                    .to_owned()
            }
            Refusal::WeekdaysInvalid => {
                "Pick at least one weekday, from Monday to Sunday, in the menu.".to_owned()
            }
            Refusal::TooManyRoles => format!(
                "An event pings at most {ROLE_LIMIT} roles. Pick {ROLE_LIMIT} or fewer."
            ),
            Refusal::PanelUnknown => {
                "This panel is no longer open, so nothing was saved. Run the command again to open a new one."
                    .to_owned()
            }
            Refusal::PanelNotYours => {
                "Only the admin who opened this panel can use it. Run the command yourself to open your own."
                    .to_owned()
            }
            Refusal::PanelExpired => format!(
                "This panel was opened more than {PANEL_LIFETIME_MINUTES} minutes ago, so nothing was saved. Run the command again to open a new one."
            ),
            Refusal::ChannelUnknown => {
                "Barnacle cannot find that channel in this server. Pick a text channel here that Barnacle can see."
                    .to_owned()
            }
        }
    }
}

fn missing_bot_permissions(names: &[&str], channel: Option<ChannelId>) -> String {
    let (place, whose) = match channel {
        Some(channel) => (format!("<#{}>", channel.get()), "that"),
        None => ("this channel".to_owned(), "this"),
    };
    format!(
        "Barnacle needs {} in {place}. A server admin can grant {} to Barnacle's role or in {whose} channel's permissions, then run the command again.",
        join_names(names),
        them(names.len())
    )
}

fn ping_blocked_by_channel(pings: &[Ping], channel: ChannelId) -> String {
    format!(
        "{} would not be pinged in <#{}>: Barnacle holds {MENTION_PERMISSION} in the server, but a permission override in that channel removes it. A server admin can allow {MENTION_PERMISSION} for Barnacle in that channel's permissions{}, then run the command again.",
        ping_mentions(pings),
        channel.get(),
        mentionable_alternatives(pings)
    )
}

fn nothing_to_repost(name: &str, next_post_at: Option<i64>, sessions_left: usize) -> String {
    let head = format!("{} has no open sign-up post to repost.", escape(name));
    let tail = match (sessions_left, next_post_at) {
        (0, _) => "The event has no sign-up posts left.".to_owned(),
        (_, Some(at_unix)) => format!("The next one goes up at {}.", full_time(at_unix)),
        (_, None) => "The next one goes up within a minute.".to_owned(),
    };
    format!("{head} {tail}")
}

fn empty_pool(options: &RoundOptions) -> String {
    let tiers = tiers_phrase(options);
    if options.historical() {
        format!(
            "No ships fit {tiers} with paper ships excluded. Try a wider range with min_tier and max_tier, or leave historical off."
        )
    } else {
        format!("No ships fit {tiers}. Try a wider range with min_tier and max_tier.")
    }
}

fn tiers_phrase(options: &RoundOptions) -> String {
    if options.spans_several_tiers() {
        format!("tiers {}", tier_range(options))
    } else {
        format!("tier {}", tier_range(options))
    }
}

fn them(count: usize) -> &'static str {
    if count == 1 { "it" } else { "them" }
}

pub fn warned(warning: Option<String>, reply: String) -> String {
    match warning {
        Some(warning) => format!("{warning}\n\n{reply}"),
        None => reply,
    }
}

pub fn failures_noted(reply: &str, failures: usize) -> String {
    if failures == 0 {
        reply.to_owned()
    } else {
        format!("{reply} {FAILURES_LOGGED}")
    }
}

pub fn best_time(best: Option<Duration>) -> String {
    best.map(seconds).unwrap_or_else(|| NO_WINS_HERE.to_owned())
}

pub fn leaderboard_title(ranking: Ranking) -> &'static str {
    match ranking {
        Ranking::MostWins => "Most wins in this server",
        Ranking::FastestTime => "Fastest times in this server",
    }
}

pub fn standing_line(rank: usize, name: &str, standing: &Standing) -> String {
    let name: String = name.chars().take(NAME_LIMIT).collect();
    format!(
        "**{rank}.** {} · {} · best {}",
        escape(&name),
        wins(standing.wins),
        seconds(standing.best)
    )
}

fn wins(count: u64) -> String {
    match count {
        1 => "1 win".to_owned(),
        count => format!("{count} wins"),
    }
}

pub fn leaderboard_pages(lines: &[String]) -> Vec<String> {
    lines.iter().fold(Vec::new(), |pages: Vec<String>, line| {
        match pages.split_last() {
            Some((last, earlier))
                if discord_length(last) + 1 + discord_length(line) <= EMBED_DESCRIPTION_LIMIT =>
            {
                earlier
                    .iter()
                    .cloned()
                    .chain(std::iter::once(format!("{last}\n{line}")))
                    .collect()
            }
            _ => pages
                .into_iter()
                .chain(std::iter::once(line.clone()))
                .collect(),
        }
    })
}

pub fn about(provenance: &Provenance, catalog_name: &str, bot_version: &str) -> String {
    let commit: String = provenance.data_repo_commit.chars().take(7).collect();
    [
        ABOUT_SUMMARY.to_owned(),
        format!(
            "Ship data: World of Warships {} (build {}), catalog {catalog_name}, data commit {commit}.",
            provenance.game_version, provenance.build
        ),
        format!(
            "Built with wowsunpack {} and wows-data-mgr {}. Barnacle {bot_version}, source at {SOURCE_URL}.",
            provenance.wowsunpack, provenance.wows_data_mgr
        ),
        WARGAMING_NOTICE.to_owned(),
    ]
    .join("\n\n")
}

pub fn listing(items: &[String], limit: usize) -> String {
    if items.is_empty() {
        return "None".to_owned();
    }
    (0..=items.len())
        .rev()
        .map(|shown| {
            let hidden = items.len() - shown;
            let head = items[..shown].join(", ");
            match (shown, hidden) {
                (_, 0) => head,
                (0, _) => format!("{hidden} more"),
                _ => format!("{head} and {hidden} more"),
            }
        })
        .find(|text| text.chars().count() <= limit)
        .unwrap_or_default()
}

pub fn signup_title(name: &str) -> String {
    escape(name)
}

pub fn short_event(hours: Hours) -> bool {
    hours.get() <= SHORT_EVENT_HOURS
}

pub fn signup_description(view: &SignupView) -> String {
    let start = view.session.start_unix();
    let heading = if view.open {
        vec![
            format!("{} · starts {}", full_time(start), relative_time(start)),
            SIGNUPS_CLOSE_AT_START.to_owned(),
        ]
    } else {
        vec![format!("{} · {SIGNUPS_CLOSED}", full_time(start))]
    };
    let head: Vec<String> = heading
        .into_iter()
        .chain(std::iter::once(String::new()))
        .chain(
            view.hours
                .iter()
                .map(|tally| hour_line(view.session, tally)),
        )
        .chain(std::iter::once(String::new()))
        .collect();
    let described = |kept: usize| {
        head.iter()
            .cloned()
            .chain(roster_lines(view, kept))
            .collect::<Vec<String>>()
            .join("\n")
    };
    (0..=view.rows.len())
        .rev()
        .map(described)
        .find(|text| text.chars().count() <= EMBED_DESCRIPTION_LIMIT)
        .unwrap_or_else(|| described(0))
}

pub fn hour_button_label(hour: Hour, attending: bool) -> String {
    format!("Hour {}: {}", hour.get(), choice_label(attending))
}

pub fn signups_closed_at(start_unix: i64) -> String {
    format!(
        "Sign-ups for this session closed at {}.",
        clock_time(start_unix)
    )
}

pub fn event_removed(name: &str) -> String {
    format!("{} was removed. Nothing had been posted.", escape(name))
}

pub fn rehearsal_started(event: &Event, sessions_left: usize, post_at_unix: Option<i64>) -> String {
    sentences(&[
        format!("Rehearsing {}.", escape(&event.name)),
        still_ahead(event, sessions_left),
        next_post(sessions_left, post_at_unix),
    ])
}

pub fn event_ended(name: &str, cleared: usize) -> String {
    let kept = match cleared {
        0 => "The answers are kept.".to_owned(),
        cleared => format!(
            "{} {}, and the answers are kept.",
            signup_posts(cleared),
            cleared_verb(cleared)
        ),
    };
    format!("{} has ended. {kept}", escape(name))
}

pub fn stepped(moment_unix: i64, report: &TickReport) -> String {
    let steps = sentences(&[
        removed(report.removed.len()),
        closed(report.closed.len()),
        went_up(report.posted.len()),
        adopted(report.adopted.len()),
        failed(report.failures),
    ]);
    sentences(&[
        format!("Moved the rehearsal clock to {}.", full_time(moment_unix)),
        if steps.is_empty() {
            NOTHING_HAPPENED.to_owned()
        } else {
            steps
        },
    ])
}

pub fn reset_blocked(purge: &PurgeReport) -> String {
    sentences(&[
        RESET_BLOCKED_HEAD.to_owned(),
        already_cleared(purge.messages),
        RESET_BLOCKED_TAIL.to_owned(),
    ])
}

fn already_cleared(count: usize) -> String {
    match count {
        0 => String::new(),
        count => format!(
            "{} {} already cleared.",
            signup_posts(count),
            was_verb(count)
        ),
    }
}

pub fn reset_done(purge: &PurgeReport) -> String {
    let counts: Vec<String> = [
        (purge.events, events(purge.events)),
        (purge.messages, signup_posts(purge.messages)),
        (purge.answers, answers(purge.answers)),
    ]
    .into_iter()
    .filter(|(count, _)| *count > 0)
    .map(|(_, part)| part)
    .collect();
    sentences(&[if counts.is_empty() {
        NOTHING_TO_CLEAR.to_owned()
    } else {
        format!("Cleared {}.", joined(&counts))
    }])
}

pub fn reposted(event: &Event, session: Session, adopted: bool, pinged: &[Ping]) -> String {
    let name = escape(&event.name);
    let place = format!("<#{}>", event.channel.get());
    let label = session.label();
    let done = if adopted {
        format!(
            "Recovered {name}'s sign-up post for {label} in {place}, left there by an earlier repost that did not finish. It was not pinged again."
        )
    } else {
        format!(
            "Reposted {name}'s sign-up post for {label} at the bottom of {place}. Every answer is kept."
        )
    };
    if pinged.is_empty() {
        done
    } else {
        format!("{done} It pinged {}.", ping_mentions(pinged))
    }
}

pub fn event_line(event: &Event, now_unix: i64) -> String {
    let span = &event.span;
    let schedule = span.schedule();
    let sessions_left = span.sessions_left(now_unix);
    let start = span
        .next_session(now_unix)
        .or_else(|| span.sessions().last())
        .map(|session| format!(" at {}", clock_time(session.start_unix())))
        .unwrap_or_default();
    format!(
        "<#{}> **{}**, {} to {}. {}{start}, {}, {}. {} ahead. {}{}",
        event.channel.get(),
        escape(&event.name),
        span.first_day(),
        span.last_day(),
        weekdays_phrase(schedule.weekdays()),
        hours_phrase(schedule.hours()),
        schedule.zone().name(),
        sessions(sessions_left),
        next_post(sessions_left, next_post_at(span, now_unix)),
        pings_line(
            &event
                .ping_roles
                .iter()
                .map(|role| Ping::of(event.guild, *role))
                .collect::<Vec<Ping>>()
        )
    )
}

pub fn event_list(lines: &[String]) -> String {
    let kept: Vec<&str> = lines
        .iter()
        .scan(None::<usize>, |length, line| {
            let next = length.map_or(0, |length| length + 1) + discord_length(line);
            (next <= MESSAGE_CONTENT_LIMIT - EVENT_LIST_TAIL).then(|| {
                *length = Some(next);
                line.as_str()
            })
        })
        .collect();
    let hidden = lines.len() - kept.len();
    let shown = kept.join("\n");
    if hidden == 0 {
        shown
    } else {
        format!("{shown}\nand {hidden} more.")
    }
}

pub fn event_created(event: &Event, sessions_left: usize, post_at_unix: Option<i64>) -> String {
    sentences(&[
        format!(
            "{} will post in <#{}>.",
            escape(&event.name),
            event.channel.get()
        ),
        still_ahead(event, sessions_left),
        next_post(sessions_left, post_at_unix),
    ])
}

pub fn event_edited(after: &Event, cleared: &PostsReport, refreshed: &PostsReport) -> String {
    let reply = sentences(&[
        format!("{} was updated.", escape(&after.name)),
        format!(
            "It posts in <#{}>, {} from {} to {}.",
            after.channel.get(),
            sessions(after.span.session_count()),
            after.span.first_day(),
            after.span.last_day()
        ),
        posts_cleared(cleared.touched),
        posts_redrawn(refreshed.touched),
        if cleared.failures == 0 {
            String::new()
        } else {
            CLEAR_FAILED_EDIT.to_owned()
        },
        if refreshed.failures == 0 {
            String::new()
        } else {
            REFRESH_FAILED.to_owned()
        },
    ]);
    failures_noted(&reply, cleared.failures + refreshed.failures)
}

pub fn panel_preview(
    name: &str,
    channel: ChannelId,
    span: &Span,
    pings: &[Ping],
    now_unix: i64,
) -> String {
    let schedule = span.schedule();
    let first = span.sessions().next();
    let start = first
        .map(|session| format!(" at {}", clock_time(session.start_unix())))
        .unwrap_or_default();
    let count = match first {
        Some(session) => format!(
            "First session: {}. {}, {} still ahead.",
            full_time(session.start_unix()),
            sessions(span.session_count()),
            span.sessions_left(now_unix)
        ),
        None => NO_SESSION_IN_SPAN.to_owned(),
    };
    let pinged = if pings.is_empty() {
        "nobody".to_owned()
    } else {
        ping_mentions(pings)
    };
    [
        format!("**{}** in <#{}>", escape(name), channel.get()),
        format!(
            "{} to {}. {}{start}, {}, {}.",
            span.first_day(),
            span.last_day(),
            weekdays_phrase(schedule.weekdays()),
            hours_phrase(schedule.hours()),
            schedule.zone().name()
        ),
        count,
        format!("Pings {pinged}."),
    ]
    .join("\n")
}

pub fn everyone_label(on: bool) -> &'static str {
    if on {
        "Ping @everyone: on"
    } else {
        "Ping @everyone: off"
    }
}

pub fn date_label(date: Date) -> String {
    format!("{date} ({})", weekday_name(date.weekday()))
}

fn weekdays_phrase(weekdays: Weekdays) -> String {
    if weekdays == Weekdays::EVERY_DAY {
        return EVERY_DAY.to_owned();
    }
    let names: Vec<&str> = weekdays.days().into_iter().map(weekday_name).collect();
    join_names(&names)
}

pub fn weekday_name(day: Weekday) -> &'static str {
    match day {
        Weekday::Monday => "Monday",
        Weekday::Tuesday => "Tuesday",
        Weekday::Wednesday => "Wednesday",
        Weekday::Thursday => "Thursday",
        Weekday::Friday => "Friday",
        Weekday::Saturday => "Saturday",
        Weekday::Sunday => "Sunday",
    }
}

fn hours_phrase(hours: Hours) -> String {
    match hours.get() {
        1 => "1 hour".to_owned(),
        count => format!("{count} hours"),
    }
}

pub fn pings_line(pings: &[Ping]) -> String {
    if pings.is_empty() {
        String::new()
    } else {
        format!(" Pings {}.", ping_mentions(pings))
    }
}

pub fn updates_setup_reply(
    channel: ChannelId,
    ping: Option<Ping>,
    verdict: Option<PingVerdict>,
) -> String {
    with_ping_warning(
        format!(
            "Barnacle update posts will go to <#{}> and ping {}.",
            channel.get(),
            ping_target(ping)
        ),
        channel,
        ping,
        verdict,
    )
}

pub fn announce_preview_line(
    channel: ChannelId,
    ping: Option<Ping>,
    verdict: Option<PingVerdict>,
) -> String {
    with_ping_warning(
        format!(
            "This will be posted in <#{}> and ping {}.",
            channel.get(),
            ping_target(ping)
        ),
        channel,
        ping,
        verdict,
    )
}

pub fn updates_already_sent(version: &str) -> String {
    format!("This server already got the notes for Barnacle {version}.")
}

pub fn announce_version_changed(running: &str) -> String {
    format!(
        "Barnacle has been updated to {running} since this preview, so nothing was posted. Run /announce again."
    )
}

pub fn announce_posted(channel: ChannelId) -> String {
    format!("Posted in <#{}>.", channel.get())
}

fn ping_target(ping: Option<Ping>) -> String {
    ping.map_or_else(|| "nobody".to_owned(), ping_mention)
}

fn with_ping_warning(
    line: String,
    channel: ChannelId,
    ping: Option<Ping>,
    verdict: Option<PingVerdict>,
) -> String {
    let warning = ping
        .zip(verdict)
        .and_then(|(ping, verdict)| ping_warning(ping, verdict, channel));
    sentences(&[line, warning.unwrap_or_default()])
}

pub fn silent_ping_warning(checked: &[(Ping, PingVerdict)], channel: ChannelId) -> Option<String> {
    let warnings: Vec<String> = checked
        .iter()
        .filter_map(|(ping, verdict)| ping_warning(*ping, *verdict, channel))
        .collect();
    (!warnings.is_empty()).then(|| warnings.join(" "))
}

fn ping_warning(ping: Ping, verdict: PingVerdict, channel: ChannelId) -> Option<String> {
    let mention = ping_mention(ping);
    let place = format!("<#{}>", channel.get());
    match verdict {
        PingVerdict::Sounds => None,
        PingVerdict::Silent => Some(match ping {
            Ping::Everyone => format!(
                "{mention} will not be pinged in {place}: Barnacle lacks {MENTION_PERMISSION} there. A server admin can grant {MENTION_PERMISSION} to Barnacle in that channel."
            ),
            Ping::Role(_) => format!(
                "{mention} will not be pinged in {place}: the role is not mentionable and Barnacle lacks {MENTION_PERMISSION} there. A server admin can turn on the role's {MENTIONABLE_SETTING} setting, or grant {MENTION_PERMISSION} to Barnacle in that channel."
            ),
        }),
        PingVerdict::BlockedByChannel => Some(format!(
            "{mention} will not be pinged in {place}: Barnacle holds {MENTION_PERMISSION} in the server, but a permission override in that channel removes it. A server admin can allow {MENTION_PERMISSION} for Barnacle in that channel's permissions{}.",
            mentionable_alternative(ping)
        )),
        PingVerdict::Unchecked => Some(match ping {
            Ping::Everyone => format!(
                "Barnacle could not check whether {mention} will be pinged in {place}. Check that Barnacle holds {MENTION_PERMISSION} in that channel."
            ),
            Ping::Role(_) => format!(
                "Barnacle could not check whether {mention} will be pinged in {place}. Check that the role's {MENTIONABLE_SETTING} setting is on, or that Barnacle holds {MENTION_PERMISSION} in that channel."
            ),
        }),
    }
}

fn ping_mention(ping: Ping) -> String {
    match ping {
        Ping::Everyone => "@everyone".to_owned(),
        Ping::Role(role) => format!("<@&{role}>"),
    }
}

fn ping_mentions(pings: &[Ping]) -> String {
    joined(
        &pings
            .iter()
            .copied()
            .map(ping_mention)
            .collect::<Vec<String>>(),
    )
}

fn mentionable_alternative(ping: Ping) -> String {
    mentionable_alternatives(&[ping])
}

fn mentionable_alternatives(pings: &[Ping]) -> String {
    match pings
        .iter()
        .filter(|ping| matches!(ping, Ping::Role(_)))
        .count()
    {
        0 => String::new(),
        1 => format!(", or turn on the role's {MENTIONABLE_SETTING} setting"),
        _ => format!(", or turn on each role's {MENTIONABLE_SETTING} setting"),
    }
}

fn still_ahead(event: &Event, sessions_left: usize) -> String {
    format!(
        "{}, {sessions_left} still ahead.",
        sessions(event.span.session_count())
    )
}

fn next_post(sessions_left: usize, post_at_unix: Option<i64>) -> String {
    if sessions_left == 0 {
        NOTHING_AHEAD.to_owned()
    } else {
        format!("Next sign-up post: {}.", post_time(post_at_unix))
    }
}

fn posts_cleared(count: usize) -> String {
    match count {
        0 => String::new(),
        count => format!("{} {}.", signup_posts(count), cleared_verb(count)),
    }
}

fn posts_redrawn(count: usize) -> String {
    match count {
        0 => String::new(),
        count => format!("{} {} redrawn.", signup_posts(count), was_verb(count)),
    }
}

fn went_up(count: usize) -> String {
    match count {
        0 => String::new(),
        count => format!("{} went up.", signup_posts(count)),
    }
}

fn adopted(count: usize) -> String {
    match count {
        0 => String::new(),
        count => format!("{} {} adopted.", signup_posts(count), was_verb(count)),
    }
}

fn closed(count: usize) -> String {
    match count {
        0 => String::new(),
        count => format!("{} closed.", signup_posts(count)),
    }
}

fn removed(count: usize) -> String {
    match count {
        0 => String::new(),
        count => format!("{} {} removed.", signup_posts(count), was_verb(count)),
    }
}

fn failed(count: usize) -> String {
    match count {
        0 => String::new(),
        count => format!("{} failed.", steps(count)),
    }
}

fn joined(parts: &[String]) -> String {
    match parts.split_last() {
        Some((last, [])) => last.clone(),
        Some((last, head)) => format!("{} and {last}", head.join(", ")),
        None => String::new(),
    }
}

fn signup_posts(count: usize) -> String {
    match count {
        1 => "1 sign-up post".to_owned(),
        count => format!("{count} sign-up posts"),
    }
}

fn events(count: usize) -> String {
    match count {
        1 => "1 event".to_owned(),
        count => format!("{count} events"),
    }
}

fn sessions(count: usize) -> String {
    match count {
        1 => "1 session".to_owned(),
        count => format!("{count} sessions"),
    }
}

fn answers(count: usize) -> String {
    match count {
        1 => "1 answer".to_owned(),
        count => format!("{count} answers"),
    }
}

fn steps(count: usize) -> String {
    match count {
        1 => "1 step".to_owned(),
        count => format!("{count} steps"),
    }
}

fn cleared_verb(count: usize) -> &'static str {
    if count == 1 {
        "was cleared"
    } else {
        "were cleared"
    }
}

fn was_verb(count: usize) -> &'static str {
    if count == 1 { "was" } else { "were" }
}

fn sentences(parts: &[String]) -> String {
    join_filled(parts, " ")
}

fn join_filled(parts: &[impl AsRef<str>], separator: &str) -> String {
    parts
        .iter()
        .map(AsRef::as_ref)
        .filter(|part| !part.is_empty())
        .collect::<Vec<&str>>()
        .join(separator)
}

fn discord_length(text: &str) -> usize {
    text.encode_utf16().count()
}

fn sentence(text: &str) -> String {
    if text.ends_with('.') {
        text.to_owned()
    } else {
        format!("{text}.")
    }
}

fn choice_label(attending: bool) -> &'static str {
    if attending { "Attending" } else { "Nope" }
}

fn hour_line(session: Session, tally: &HourTally) -> String {
    format!(
        "**Hour {}** · {} – {} · {} in · {} out",
        tally.hour.get(),
        clock_time(session.hour_start_unix(tally.hour)),
        clock_time(session.hour_end_unix(tally.hour)),
        tally.attending,
        tally.nope
    )
}

fn roster_header(hours: Hours) -> String {
    hours
        .each()
        .map(|hour| format!("{:<CELL_WIDTH$}", hour.get()))
        .collect()
}

fn roster_lines(view: &SignupView, kept: usize) -> Vec<String> {
    if view.rows.is_empty() {
        return vec![NO_ANSWERS_YET.to_owned()];
    }
    let hours = view.session.hours();
    let hidden = view.hidden + view.rows.len() - kept;
    short_event(hours)
        .then(|| format!("`{}`", roster_header(hours)))
        .into_iter()
        .chain(
            view.rows
                .iter()
                .take(kept)
                .map(|row| roster_line(hours, row)),
        )
        .chain((hidden > 0).then(|| format!("and {hidden} more.")))
        .collect()
}

fn roster_line(hours: Hours, row: &RosterRow) -> String {
    let answer = if short_event(hours) {
        row.cells
            .iter()
            .map(|cell| format!("{:<CELL_WIDTH$}", cell_text(*cell)))
            .collect()
    } else {
        let attending: Vec<Hour> = hours
            .each()
            .zip(&row.cells)
            .filter(|(_, cell)| **cell == Cell::In)
            .map(|(hour, _)| hour)
            .collect();
        if attending.is_empty() {
            cell_text(Cell::Out).to_owned()
        } else {
            hour_ranges(&attending)
        }
    };
    format!("`{answer}` <@{}>", row.user.get())
}

pub fn hour_ranges(hours: &[Hour]) -> String {
    let numbers: BTreeSet<u8> = hours.iter().map(|hour| hour.get()).collect();
    numbers
        .iter()
        .fold(Vec::<(u8, u8)>::new(), |runs, &number| {
            match runs.split_last() {
                Some((&(first, last), earlier)) if last + 1 == number => earlier
                    .iter()
                    .copied()
                    .chain(std::iter::once((first, number)))
                    .collect(),
                _ => runs
                    .into_iter()
                    .chain(std::iter::once((number, number)))
                    .collect(),
            }
        })
        .into_iter()
        .map(|(first, last)| {
            if first == last {
                first.to_string()
            } else {
                format!("{first}–{last}")
            }
        })
        .collect::<Vec<String>>()
        .join(", ")
}

pub fn hours_menu_text(view: &SignupView) -> String {
    let session = view.session;
    std::iter::once(format!(
        "Pick the hours you attend for {}, {}.",
        escape(&view.name),
        full_time(session.start_unix())
    ))
    .chain(session.hours().each().map(|hour| {
        format!(
            "**Hour {}** · {} – {}",
            hour.get(),
            clock_time(session.hour_start_unix(hour)),
            clock_time(session.hour_end_unix(hour))
        )
    }))
    .collect::<Vec<String>>()
    .join("\n")
}

pub fn hours_saved(attending: &[Hour]) -> String {
    match attending.len() {
        0 => "Saved. You attend no hour of this session.".to_owned(),
        1 => format!(
            "Saved. You attend hour {} of this session.",
            hour_ranges(attending)
        ),
        _ => format!(
            "Saved. You attend hours {} of this session.",
            hour_ranges(attending)
        ),
    }
}

fn cell_text(cell: Cell) -> &'static str {
    match cell {
        Cell::In => "in",
        Cell::Out => "out",
    }
}

fn post_time(at_unix: Option<i64>) -> String {
    match at_unix {
        Some(at_unix) => format!("{} ({})", full_time(at_unix), relative_time(at_unix)),
        None => "within a minute".to_owned(),
    }
}

fn clock_time(unix: i64) -> String {
    format!("<t:{unix}:t>")
}

fn full_time(unix: i64) -> String {
    format!("<t:{unix}:F>")
}

fn relative_time(unix: i64) -> String {
    format!("<t:{unix}:R>")
}
