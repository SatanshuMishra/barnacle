use barnacle_bot::ids::ChannelId;
use barnacle_bot::ids::RoleId;
use barnacle_bot::iq::BoardRow;
use barnacle_bot::iq::Completion;
use barnacle_bot::iq::Criterion;
use barnacle_bot::iq::Done;
use barnacle_bot::iq::Problem;
use barnacle_bot::iq::Standing;
use barnacle_bot::iq_setup::MarkField;
use barnacle_bot::iq_text;
use barnacle_bot::iq_text::board_line;
use barnacle_bot::iq_text::submission_reply;
use barnacle_bot::iq_text::unfinished;
use barnacle_bot::iq_text::winner_announcement;
use barnacle_guess::UserId;

const COMPETITION: &str = "November IQ";
const TARGET_ROLE: RoleId = RoleId::new(31);

fn standing(rank: usize, user: u64, points: i64) -> Standing {
    Standing {
        rank,
        user: UserId::new(user),
        points,
    }
}

fn names(pairs: &[(u64, &str)]) -> Vec<(UserId, String)> {
    pairs
        .iter()
        .map(|(user, name)| (UserId::new(*user), (*name).to_owned()))
        .collect()
}

fn announcement(standings: &[Standing], pairs: &[(u64, &str)]) -> String {
    winner_announcement(COMPETITION, &[TARGET_ROLE], standings, &names(pairs))
}

#[test]
fn one_problem_gets_the_one_thing_reply() {
    assert_eq!(
        submission_reply(&[Problem::ScreenshotCount(1)]),
        "**Not counted.** One thing to fix:\n- It has 1 screenshot; it needs exactly 2.\nPost it again as a new message. Edits are not re-checked."
    );
}

#[test]
fn several_problems_are_counted_and_listed() {
    let reply = submission_reply(&[
        Problem::UnknownName {
            typed: "krakn".to_owned(),
            closest: "Drunken Kraken".to_owned(),
        },
        Problem::ScreenshotCount(3),
        Problem::NotAnImage,
    ]);
    assert!(
        reply.starts_with("**Not counted.** 3 things to fix:"),
        "{reply}"
    );
    let lines: Vec<&str> = reply.lines().collect();
    assert_eq!(
        lines[1..4],
        [
            "- No criterion is called \"krakn\". The closest is Drunken Kraken.",
            "- It has 3 screenshots; it needs exactly 2.",
            "- Only images count as screenshots. Leave other files out.",
        ]
    );
}

#[test]
fn a_leaderboard_row_is_written_with_a_two_digit_rank() {
    let halyard = BoardRow {
        rank: Some(1),
        user: UserId::new(1),
        points: 27,
        requester: false,
    };
    let captain = BoardRow {
        rank: Some(14),
        user: UserId::new(14),
        points: 5,
        requester: true,
    };
    assert_eq!(board_line(&halyard, "Halyard"), "01. Halyard \u{b7} 27");
    assert_eq!(
        board_line(&captain, "Captain_Ranger"),
        "**14. Captain_Ranger \u{b7} 5**"
    );
}

#[test]
fn a_no_points_row_shows_dashes() {
    let newcomer = BoardRow {
        rank: None,
        user: UserId::new(99),
        points: 0,
        requester: true,
    };
    assert_eq!(
        board_line(&newcomer, "Newcomer"),
        "**--. Newcomer \u{b7} 0**"
    );
}

#[test]
fn a_single_winner_is_named() {
    let text = announcement(
        &[standing(1, 1, 27), standing(2, 2, 22)],
        &[(1, "Halyard"), (2, "Tidewright")],
    );
    assert!(
        text.contains("The winner is <@1> with 27 points."),
        "{text}"
    );
}

#[test]
fn a_tie_names_every_tied_player() {
    let text = announcement(
        &[
            standing(1, 1, 27),
            standing(1, 2, 27),
            standing(1, 3, 27),
            standing(4, 4, 12),
        ],
        &[
            (1, "Halyard"),
            (2, "Tidewright"),
            (3, "Bosun"),
            (4, "Lookout"),
        ],
    );
    assert!(
        text.contains("It's a tie: <@1>, <@2> and <@3> share the win with 27 points each."),
        "{text}"
    );
}

#[test]
fn nobody_scoring_means_no_winner() {
    let text = announcement(&[], &[]);
    assert!(
        text.contains("Nobody scored, so there is no winner."),
        "{text}"
    );
}

#[test]
fn an_unfinished_change_names_what_was_done() {
    let untouched = unfinished(&[]);
    assert!(untouched.contains("was not changed"), "{untouched}");
    let judged = unfinished(&[Done::JudgesChanged]);
    assert!(!judged.contains("was not changed"), "{judged}");
    assert!(
        judged.contains("the IQ-Judge role was handed out and taken away"),
        "{judged}"
    );
    assert!(judged.contains("Press it again"), "{judged}");
}

#[test]
fn a_completed_competition_is_not_told_to_press_again() {
    let completed = unfinished(&[Done::Completed]);
    assert!(!completed.contains("Press it again"), "{completed}");
    assert!(completed.contains("/iq cleanup"), "{completed}");
}

#[test]
fn a_typed_name_is_quoted_as_plain_text() {
    let reply = submission_reply(&[Problem::UnknownName {
        typed: "x\n# Judges: [everyone](https://example.com) gets *1000*".to_owned(),
        closest: "Drunken Kraken".to_owned(),
    }]);
    let lines: Vec<&str> = reply.lines().collect();
    assert_eq!(
        lines[1],
        "- No criterion is called \"x # Judges: \\[everyone](https://example.com) gets \\*1000\\*\". The closest is Drunken Kraken."
    );
}

#[test]
fn the_mark_prompt_names_the_owner_and_the_field() {
    assert_eq!(
        iq_text::mark_prompt(UserId::new(100), MarkField::Confirm),
        "<@100> react to this message with the emoji for Confirm awarded."
    );
    assert_eq!(
        iq_text::mark_repeated(MarkField::Denied),
        "That emoji is already the Denied mark. Pick a different one."
    );
    assert_eq!(
        iq_text::NO_EMOJI_PICKED,
        "No emoji picked. Press the button to try again."
    );
}

#[test]
fn a_points_change_is_confirmed_with_the_new_total() {
    assert_eq!(
        iq_text::points_done(UserId::new(31), 20),
        "Done. <@31> now has 20 points."
    );
}

#[test]
fn a_negative_total_allows_no_removal() {
    assert_eq!(
        iq_text::below_zero(UserId::new(31), -40),
        "<@31> has -40 points, so at most 0 can be removed."
    );
}

#[test]
fn an_early_complete_says_it_ends_before_the_last_day() {
    let question = iq_text::complete_question(
        "November IQ",
        1_796_104_800,
        Completion::Early,
        ChannelId::new(30),
        &[],
        &["Shrimpy"],
        21,
    );
    assert_eq!(
        question,
        "**November IQ** runs until <t:1796104740:F>. End it now and post the result in <#30>? Posts after this are not judged.\nWinner: **Shrimpy** with 21 points."
    );
}

#[test]
fn the_start_announcement_quotes_each_criterion_apart_and_numbers_the_rules() {
    let criteria = [
        Criterion {
            name: "Drunken Kraken".to_owned(),
            description: "Get 5 kills. Survive the battle.".to_owned(),
            points: 10,
        },
        Criterion {
            name: "Huh? What happened?".to_owned(),
            description: "Earn 3 million potential damage.".to_owned(),
            points: 11,
        },
    ];
    let rules = "1. Only submit your own replays.\n\n2) No subterfuge.\n- Be kind.";
    let text = iq_text::start_announcement(
        COMPETITION,
        1_796_104_800,
        &[TARGET_ROLE],
        ChannelId::new(40),
        &criteria,
        rules,
        &[UserId::new(7)],
    );
    let expected = [
        "**November IQ** has started. It runs until <t:1796104740:F>.",
        "",
        "To score, post in <#40>: just the criterion name, with exactly 2 screenshots (The main rewards page and team leaderboard page) attached. A judge's reaction awards the points. Check where you stand with **/iq leaderboard**.",
        "",
        "<@&31>",
        "",
        "## Criteria",
        "> **Drunken Kraken** \u{b7} 10 points",
        "> Get 5 kills. Survive the battle.",
        "",
        "> **Huh? What happened?** \u{b7} 11 points",
        "> Earn 3 million potential damage.",
        "## Rules",
        "1. Only submit your own replays.",
        "2. No subterfuge.",
        "3. Be kind.",
        "Judges: <@7>",
    ]
    .join("\n");
    assert_eq!(text, expected);
}
