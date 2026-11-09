use barnacle_bot::ids::ChannelId;
use barnacle_bot::ids::GuildId;
use barnacle_bot::iq::BoardRow;
use barnacle_bot::iq_setup::SetupDraft;
use barnacle_bot::iq_text;
use barnacle_bot::schedule::Zone;
use barnacle_guess::UserId;
use jiff::civil::date;

fn fresh_draft() -> SetupDraft {
    SetupDraft::new(
        GuildId::new(1),
        UserId::new(100),
        vec![],
        Zone::find("Europe/Berlin").unwrap(),
        date(2026, 11, 2),
    )
}

fn one_row() -> Vec<(BoardRow, String)> {
    vec![(
        BoardRow {
            rank: Some(1),
            user: UserId::new(11),
            points: 8,
            requester: true,
        },
        "Malphus".to_owned(),
    )]
}

#[test]
fn the_board_opens_with_the_new_header() {
    let board = iq_text::leaderboard("November IQ", 2000, 1000, &one_row(), 1);
    assert!(
        board.starts_with("IQ COMPETITION LEADERBOARD\nNOVEMBER IQ \u{2022} <t:1940:R>\n\n"),
        "{board}"
    );
}

#[test]
fn an_ended_board_says_it_ended() {
    let board = iq_text::leaderboard("November IQ", 2000, 3000, &one_row(), 1);
    assert_eq!(
        board.lines().nth(1),
        Some("NOVEMBER IQ \u{2022} ended <t:1940:R>"),
        "{board}"
    );
}

#[test]
fn board_rows_are_not_lists_and_the_caller_is_bold() {
    let other = BoardRow {
        rank: Some(4),
        user: UserId::new(12),
        points: 20,
        requester: false,
    };
    let caller = BoardRow {
        rank: Some(3),
        user: UserId::new(11),
        points: 28,
        requester: true,
    };
    let unranked = BoardRow {
        rank: None,
        user: UserId::new(13),
        points: 0,
        requester: true,
    };
    assert_eq!(
        iq_text::board_line(&other, "WoOoOsTeR"),
        "**4.** WoOoOsTeR \u{b7} 20"
    );
    assert_eq!(
        iq_text::board_line(&caller, "Malphus"),
        "**3.** **Malphus \u{b7} 28**"
    );
    assert_eq!(
        iq_text::board_line(&unranked, "Newcomer"),
        "**--.** **Newcomer \u{b7} 0**"
    );
}

#[test]
fn numbered_iq_lines_are_not_lists() {
    assert_eq!(
        iq_text::rule_line(10, "Own replays only"),
        "**10.** Own replays only"
    );
    let draft = fresh_draft().with_criterion("Solo", "", "5", None).unwrap();
    let criterion = draft.criteria()[0].clone();
    assert_eq!(
        iq_text::criterion_line(10, &criterion),
        "**10.** **Solo** \u{b7} 5 points"
    );
    let announcement = iq_text::start_announcement(
        "November IQ",
        2000,
        &[],
        ChannelId::new(42),
        &[criterion],
        "First\nSecond",
        &[],
    );
    let lines: Vec<&str> = announcement.lines().collect();
    assert!(lines.contains(&"**1.** First"), "{announcement}");
    assert!(lines.contains(&"**2.** Second"), "{announcement}");
}
