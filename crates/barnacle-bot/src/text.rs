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
pub const ROUNDS_WON: &str = "Rounds won";
pub const BEST_TIME: &str = "Best time";
pub const NO_WINS_HERE: &str = "No rounds won here yet.";
pub const ABOUT_TITLE: &str = "About Barnacle";
pub const ABOUT_SUMMARY: &str = "Barnacle is a Discord bot for World of Warships players. Its first game asks you to name a ship from its silhouette.";
pub const SOURCE_URL: &str = "https://github.com/SatanshuMishra/barnacle";
pub const WARGAMING_NOTICE: &str = "Barnacle is an unofficial fan project. It is not affiliated with, endorsed by, or supported by Wargaming. World of Warships, its ship names and its ship silhouettes are trademarks or copyrighted works of Wargaming.";
pub const FIELD_LIMIT: usize = 1024;

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

fn sentence(text: &str) -> String {
    if text.ends_with('.') {
        text.to_owned()
    } else {
        format!("{text}.")
    }
}
