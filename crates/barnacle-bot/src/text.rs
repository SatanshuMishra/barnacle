use std::time::Duration;

use barnacle_catalog::Nation;
use barnacle_catalog::Provenance;
use barnacle_catalog::ShipClass;
use barnacle_catalog::Tier;
use barnacle_guess::Hint;
use barnacle_guess::Reveal;
use barnacle_guess::RoundOptions;
use barnacle_guess::Timing;
use barnacle_guess::UserId;
use jiff::civil::Date;

use crate::attendance::Cell;
use crate::attendance::HourTally;
use crate::attendance::PostsReport;
use crate::attendance::PurgeReport;
use crate::attendance::RosterRow;
use crate::attendance::SignupView;
use crate::attendance::TickReport;
use crate::attendance_store::Season;
use crate::failure::join_names;
use crate::ids::ChannelId;
use crate::ids::Ping;
use crate::schedule::Hour;
use crate::schedule::Night;
use crate::solves::Ranking;
use crate::solves::Standing;
use crate::voice_text;
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
pub const SIGNUPS_CLOSE_AT_START: &str = "Sign-ups close when the night starts.";
pub const SIGNUPS_CLOSED: &str = "Sign-ups closed";
pub const SEASON_GONE: &str = "This season no longer exists.";
pub const NO_SEASON_HERE: &str = "No CB season is set up here.";
pub const ROSTER_HEADER: &str = "1    2    3    4    ";
pub const CLEAR_FAILED_MOVE: &str = "Some sign-up posts could not be removed from the old channel, so nothing moved. Check that the bot can manage messages there, then run this again.";
pub const CLEAR_FAILED_END: &str = "Some sign-up posts could not be removed, so the season is still running. Check that the bot can manage messages in its channel, then run this again.";
pub const FAILURES_LOGGED: &str = "Barnacle's log records the reason for each.";
const CLEAR_FAILED_EDIT: &str =
    "Some sign-up posts outside the new dates could not be cleared; run this again to retry.";
const REFRESH_FAILED: &str =
    "Its sign-up post could not be updated; run this again once Barnacle can edit it.";
const RESET_BLOCKED_HEAD: &str =
    "Some sign-up posts could not be removed, so no season or answer was deleted.";
const RESET_BLOCKED_TAIL: &str =
    "Check that the bot can manage messages here, then run this again.";

const MENTION_PERMISSION: &str = "Mention @everyone, @here, and All Roles";
const MENTIONABLE_SETTING: &str = "Allow anyone to @mention this role";

const NOTHING_AHEAD: &str = "Nothing further will post.";
const NOTHING_HAPPENED: &str = "Nothing happened.";
const NOTHING_TO_CLEAR: &str = "Nothing was there to clear.";
const NAME_LIMIT: usize = 32;
const SEASON_LIST_TAIL: usize = 32;
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
    format!(
        "Nobody named it. {}",
        sentence(&format!("It was {}", ship_line(reveal)))
    )
}

pub fn cancelled(by: UserId, reveal: &Reveal) -> String {
    format!(
        "<@{}> ended the round. {}",
        by.get(),
        sentence(&format!("It was {}", ship_line(reveal)))
    )
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
    NoNightsLeft,
    SeasonNumberTaken(u32),
    SeasonOverlaps(Season),
    SeasonNotFound(u32),
    SeasonAlreadyHere(u32),
    NothingToChange,
    CodenameBothWays,
    PingBothWays,
    RehearsalOnly,
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
        ping: Ping,
        channel: ChannelId,
    },
}

impl Refusal {
    pub fn code(&self) -> &'static str {
        match self {
            Refusal::WrongChannelType => "wrong_channel_type",
            Refusal::MissingBotPermissions { .. } => "missing_bot_permissions",
            Refusal::DateFormat => "date_format",
            Refusal::LastDayBeforeFirst => "last_day_before_first",
            Refusal::DatesTooFar => "dates_too_far",
            Refusal::NoNightsLeft => "no_nights_left",
            Refusal::SeasonNumberTaken(_) => "season_number_taken",
            Refusal::SeasonOverlaps(_) => "season_overlaps",
            Refusal::SeasonNotFound(_) => "season_not_found",
            Refusal::SeasonAlreadyHere(_) => "season_already_here",
            Refusal::NothingToChange => "nothing_to_change",
            Refusal::CodenameBothWays => "codename_both_ways",
            Refusal::PingBothWays => "ping_both_ways",
            Refusal::RehearsalOnly => "rehearsal_only",
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
                "A season has to sit within six months either side of today. Pick dates closer to today."
                    .to_owned()
            }
            Refusal::NoNightsLeft => {
                "That range has no CB nights left. Pick dates with a CB night still ahead."
                    .to_owned()
            }
            Refusal::SeasonNumberTaken(number) => format!(
                "Season {number} already exists. Use another number, or end that season first with /cb season end."
            ),
            Refusal::SeasonOverlaps(other) => format!(
                "That range overlaps Season {} ({} to {}). Pick dates outside it, or change that season's dates with /cb season edit.",
                other.number,
                other.range.first_day(),
                other.range.last_day()
            ),
            Refusal::SeasonNotFound(number) => format!(
                "No Season {number} is set up here. /cb season show lists the seasons that are."
            ),
            Refusal::SeasonAlreadyHere(number) => format!(
                "Season {number} already posts in this channel. To move it, run this in the channel it should post in."
            ),
            Refusal::NothingToChange => {
                "Nothing to change. Give a new_number, first_day, last_day, codename or ping_role, or use clear_codename or clear_ping_role."
                    .to_owned()
            }
            Refusal::CodenameBothWays => "Pass a codename or clear it, not both.".to_owned(),
            Refusal::PingBothWays => "Pass a ping role or clear it, not both.".to_owned(),
            Refusal::RehearsalOnly => {
                "This server is not set up for rehearsals. Whoever runs Barnacle can list it under [rehearsal] in barnacle.toml, then restart Barnacle."
                    .to_owned()
            }
            Refusal::NothingPending => {
                "Nothing is waiting to happen in this rehearsal. /rehearse start sets up another rehearsal season."
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
            Refusal::PingBlockedByChannel { ping, channel } => {
                ping_blocked_by_channel(*ping, *channel)
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

fn ping_blocked_by_channel(ping: Ping, channel: ChannelId) -> String {
    format!(
        "{} would not be pinged in <#{}>: Barnacle holds {MENTION_PERMISSION} in the server, but a permission override in that channel removes it. A server admin can allow {MENTION_PERMISSION} for Barnacle in that channel's permissions{}, then run the command again.",
        ping_mention(ping),
        channel.get(),
        mentionable_alternative(ping)
    )
}

fn empty_pool(options: &RoundOptions) -> String {
    let tiers = if options.spans_several_tiers() {
        format!("tiers {}", tier_range(options))
    } else {
        format!("tier {}", tier_range(options))
    };
    if options.historical() {
        format!(
            "No ships fit {tiers} with paper ships excluded. Try a wider range with min_tier and max_tier, or leave historical off."
        )
    } else {
        format!("No ships fit {tiers}. Try a wider range with min_tier and max_tier.")
    }
}

fn them(count: usize) -> &'static str {
    if count == 1 { "it" } else { "them" }
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
    let wins = match standing.wins {
        1 => "1 win".to_owned(),
        wins => format!("{wins} wins"),
    };
    format!(
        "**{rank}.** {} · {wins} · best {}",
        escape(&name),
        seconds(standing.best)
    )
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

pub fn signup_title(number: u32, codename: Option<&str>) -> String {
    format!("Clan Battles · {}", season_name(number, codename))
}

pub fn signup_description(view: &SignupView) -> String {
    let start = view.night.start_unix();
    let heading = if view.open {
        vec![
            format!("{} · starts {}", full_time(start), relative_time(start)),
            SIGNUPS_CLOSE_AT_START.to_owned(),
        ]
    } else {
        vec![format!("{} · {SIGNUPS_CLOSED}", full_time(start))]
    };
    heading
        .into_iter()
        .chain(std::iter::once(String::new()))
        .chain(view.hours.iter().map(|tally| hour_line(view.night, tally)))
        .chain(std::iter::once(String::new()))
        .chain(roster_lines(view))
        .collect::<Vec<String>>()
        .join("\n")
}

pub fn hour_button_label(hour: Hour, attending: bool) -> String {
    format!("Hour {}: {}", hour.get(), choice_label(attending))
}

pub fn signups_closed_at(start_unix: i64) -> String {
    format!(
        "Sign-ups for this night closed at {}.",
        clock_time(start_unix)
    )
}

pub fn season_started(season: &Season, nights_left: usize, post_at_unix: Option<i64>) -> String {
    format!(
        "{} will post here. {}, {nights_left} still ahead. First sign-up post: {}.",
        season_name(season.number, season.codename.as_deref()),
        nights(season.range.night_count()),
        post_time(post_at_unix)
    )
}

pub fn season_line(season: &Season, nights_left: usize, post_at_unix: Option<i64>) -> String {
    format!(
        "<#{}> {}, {} to {}. {} ahead. Next sign-up post: {}.{}",
        season.channel.get(),
        season_name(season.number, season.codename.as_deref()),
        season.range.first_day(),
        season.range.last_day(),
        nights_ahead(nights_left),
        post_time(post_at_unix),
        pings_line(season.ping_role.map(|role| Ping::of(season.guild, role)))
    )
}

pub fn season_list(lines: &[String]) -> String {
    let mut kept: Vec<&str> = Vec::new();
    let mut length = 0;
    for line in lines {
        let next = length + discord_length(line) + usize::from(!kept.is_empty());
        if next > MESSAGE_CONTENT_LIMIT - SEASON_LIST_TAIL {
            break;
        }
        length = next;
        kept.push(line);
    }
    let hidden = lines.len() - kept.len();
    let shown = kept.join("\n");
    if hidden == 0 {
        shown
    } else {
        format!("{shown}\nand {hidden} more.")
    }
}

fn nights_ahead(count: usize) -> String {
    match count {
        1 => "1 night".to_owned(),
        count => format!("{count} nights"),
    }
}

fn nights(count: usize) -> String {
    match count {
        1 => "1 CB night".to_owned(),
        count => format!("{count} CB nights"),
    }
}

pub fn season_removed(number: u32) -> String {
    format!("Season {number} was removed. Nothing had been posted.")
}

pub fn season_shortened(number: u32, last_day: Date) -> String {
    format!("Season {number} ends after {last_day}. No more sign-up posts.")
}

pub fn rehearsal_started(season: &Season, nights_left: usize, post_at_unix: Option<i64>) -> String {
    sentences(&[
        format!(
            "Rehearsing season {}.",
            season_label(season.number, season.codename.as_deref())
        ),
        still_ahead(season, nights_left),
        next_post(nights_left, post_at_unix),
        manage_with(season),
    ])
}

pub fn season_edited(
    season: &Season,
    nights_left: usize,
    post_at_unix: Option<i64>,
    cleared: usize,
) -> String {
    sentences(&[
        format!(
            "{} updated.",
            season_name(season.number, season.codename.as_deref())
        ),
        still_ahead(season, nights_left),
        next_post(nights_left, post_at_unix),
        cleared_outside_the_range(cleared),
        manage_with(season),
    ])
}

pub fn season_edit_reached(edited: &str, cleared: &PostsReport, refreshed: &PostsReport) -> String {
    let reply = sentences(&[
        edited.to_owned(),
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

pub fn season_moved(
    season: &Season,
    nights_left: usize,
    cleared: usize,
    post_at_unix: Option<i64>,
) -> String {
    sentences(&[
        format!(
            "{} now posts in this channel.",
            season_name(season.number, season.codename.as_deref())
        ),
        cleared_from_the_old_channel(cleared),
        next_post(nights_left, post_at_unix),
    ])
}

pub fn season_ended(number: u32, cleared: usize) -> String {
    let kept = match cleared {
        0 => "The answers are kept.".to_owned(),
        cleared => format!(
            "{} {}, and the answers are kept.",
            signup_posts(cleared),
            cleared_verb(cleared)
        ),
    };
    format!("Season {number} has ended. {kept}")
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
        (purge.seasons, seasons(purge.seasons)),
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

pub fn pings_line(ping: Option<Ping>) -> String {
    match ping {
        Some(Ping::Everyone) => " Pings @everyone.".to_owned(),
        Some(Ping::Role(role)) => format!(" Pings <@&{role}>."),
        None => String::new(),
    }
}

pub fn silent_ping_warning(ping: Ping, verdict: PingVerdict, channel: ChannelId) -> Option<String> {
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

fn mentionable_alternative(ping: Ping) -> String {
    match ping {
        Ping::Everyone => String::new(),
        Ping::Role(_) => format!(", or turn on the role's {MENTIONABLE_SETTING} setting"),
    }
}

fn still_ahead(season: &Season, nights_left: usize) -> String {
    format!(
        "{}, {nights_left} still ahead.",
        nights(season.range.night_count())
    )
}

fn manage_with(season: &Season) -> String {
    format!("Manage it with number {}.", season.number)
}

fn next_post(nights_left: usize, post_at_unix: Option<i64>) -> String {
    if nights_left == 0 {
        NOTHING_AHEAD.to_owned()
    } else {
        format!("Next sign-up post: {}.", post_time(post_at_unix))
    }
}

fn cleared_outside_the_range(cleared: usize) -> String {
    match cleared {
        0 => String::new(),
        cleared => format!(
            "{} outside the new dates {}.",
            signup_posts(cleared),
            cleared_verb(cleared)
        ),
    }
}

fn cleared_from_the_old_channel(cleared: usize) -> String {
    match cleared {
        0 => String::new(),
        cleared => format!(
            "{} {} from the old channel.",
            signup_posts(cleared),
            cleared_verb(cleared)
        ),
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

fn seasons(count: usize) -> String {
    match count {
        1 => "1 season".to_owned(),
        count => format!("{count} seasons"),
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
    parts
        .iter()
        .filter(|part| !part.is_empty())
        .cloned()
        .collect::<Vec<String>>()
        .join(" ")
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

fn season_name(number: u32, codename: Option<&str>) -> String {
    format!("Season {}", season_label(number, codename))
}

fn season_label(number: u32, codename: Option<&str>) -> String {
    match codename {
        Some(codename) => format!("{number}: {}", escape(codename)),
        None => number.to_string(),
    }
}

fn choice_label(attending: bool) -> &'static str {
    if attending { "Attending" } else { "Nope" }
}

fn hour_line(night: Night, tally: &HourTally) -> String {
    format!(
        "**Hour {}** · {} – {} · {} in · {} out",
        tally.hour.get(),
        clock_time(night.hour_start_unix(tally.hour)),
        clock_time(night.hour_end_unix(tally.hour)),
        tally.attending,
        tally.nope
    )
}

fn roster_lines(view: &SignupView) -> Vec<String> {
    if view.rows.is_empty() {
        return vec![NO_ANSWERS_YET.to_owned()];
    }
    std::iter::once(format!("`{ROSTER_HEADER}`"))
        .chain(view.rows.iter().map(roster_line))
        .chain((view.hidden > 0).then(|| format!("and {} more.", view.hidden)))
        .collect()
}

fn roster_line(row: &RosterRow) -> String {
    let cells: String = row
        .cells
        .iter()
        .map(|cell| format!("{:<CELL_WIDTH$}", cell_text(*cell)))
        .collect();
    format!("`{cells}` <@{}>", row.user.get())
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
