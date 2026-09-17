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
use crate::attendance::RosterRow;
use crate::attendance::SignupView;
use crate::attendance_store::Season;
use crate::schedule::Hour;
use crate::schedule::Night;
use crate::solves::Ranking;
use crate::solves::Standing;

pub const EMBED_COLOUR: u32 = 0x2E6F6B;
pub const ROUND_TITLE: &str = "Name that ship";
pub const ROUND_DESCRIPTION: &str = "First correct answer in chat wins.";
pub const TIERS_FIELD: &str = "Tiers";
pub const PAPER_EXCLUDED: &str = "Paper ships excluded";
pub const CANCEL_LABEL: &str = "Cancel";
pub const ALREADY_RUNNING: &str = "This channel already has a round running.";
pub const CANCEL_REFUSED: &str =
    "Only the player who started this round, or someone who can manage messages, can end it.";
pub const ROUND_OVER: &str = "That round has already ended.";
pub const SOMETHING_WENT_WRONG: &str = "Something went wrong. Nothing was changed.";
pub const NO_SHIP_MATCHES: &str = "No ship matches that.";
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
pub const RUN_IN_TEXT_CHANNEL: &str = "Run this in a text channel.";
pub const DATE_FORMAT: &str = "Dates look like 2026-09-16.";
pub const LAST_DAY_BEFORE_FIRST: &str = "The last day is before the first day.";
pub const NO_NIGHTS_LEFT: &str = "That range has no CB nights left.";
pub const NO_SEASON_HERE: &str = "No CB season is set up here.";
pub const ROSTER_HEADER: &str = "1    2    3    4    ";

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

pub fn empty_pool(options: &RoundOptions) -> String {
    let tiers = if options.spans_several_tiers() {
        format!("tiers {}", tier_range(options))
    } else {
        format!("tier {}", tier_range(options))
    };
    if options.historical() {
        format!("No ships fit {tiers} with paper ships excluded.")
    } else {
        format!("No ships fit {tiers}.")
    }
}

pub fn missing_permissions(names: &[&str]) -> String {
    format!(
        "I need these permissions in this channel to run a round: {}.",
        names.join(", ")
    )
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
        "<#{}> {}, {} to {}. {} ahead. Next sign-up post: {}.",
        season.channel.get(),
        season_name(season.number, season.codename.as_deref()),
        season.range.first_day(),
        season.range.last_day(),
        nights_ahead(nights_left),
        post_time(post_at_unix)
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

pub fn season_number_taken(number: u32) -> String {
    format!("Season {number} already exists. End it first with /cb season end.")
}

pub fn season_overlaps(other: &Season) -> String {
    format!(
        "That range overlaps Season {} ({} to {}).",
        other.number,
        other.range.first_day(),
        other.range.last_day()
    )
}

pub fn season_removed(number: u32) -> String {
    format!("Season {number} was removed. Nothing had been posted.")
}

pub fn season_shortened(number: u32, last_day: Date) -> String {
    format!("Season {number} ends after {last_day}. No more sign-up posts.")
}

pub fn season_not_found(number: u32) -> String {
    format!("No Season {number} is set up here.")
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
    match codename {
        Some(codename) => format!("Season {number}: {codename}"),
        None => format!("Season {number}"),
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
        Cell::None => "-",
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
