use barnacle_bot::failure::Kind;
use barnacle_bot::ids::GuildId;
use barnacle_bot::iq::Attachment;
use barnacle_bot::iq::BoardRow;
use barnacle_bot::iq::CaughtUp;
use barnacle_bot::iq::CheckPath;
use barnacle_bot::iq::Checked;
use barnacle_bot::iq::CleanupRefusal;
use barnacle_bot::iq::CompleteRefusal;
use barnacle_bot::iq::Criterion;
use barnacle_bot::iq::Problem;
use barnacle_bot::iq::Score;
use barnacle_bot::iq::StartChoice;
use barnacle_bot::iq::Status;
use barnacle_bot::iq::board_rows;
use barnacle_bot::iq::check_submission;
use barnacle_bot::iq::end_unix;
use barnacle_bot::iq::may_clean_up;
use barnacle_bot::iq::may_complete;
use barnacle_bot::iq::message_chunks;
use barnacle_bot::iq::moves_last_seen;
use barnacle_bot::iq::role_already_off;
use barnacle_bot::iq::should_check;
use barnacle_bot::iq::standings;
use barnacle_bot::iq::start_choice;
use barnacle_bot::schedule::Zone;
use barnacle_guess::UserId;
use jiff::civil::date;

const END: i64 = 1_796_104_800;
const REQUESTER: UserId = UserId::new(999);

fn criterion(name: &str, points: u32) -> Criterion {
    Criterion {
        name: name.to_owned(),
        description: String::new(),
        points,
    }
}

fn criteria() -> Vec<Criterion> {
    vec![criterion("Double Down", 2), criterion("Drunken Kraken", 5)]
}

fn attachment(content_type: &str) -> Attachment {
    Attachment {
        content_type: Some(content_type.to_owned()),
    }
}

fn two_images() -> Vec<Attachment> {
    vec![attachment("image/png"), attachment("image/jpeg")]
}

fn check(text: &str, attachments: &[Attachment]) -> Checked {
    check_submission(text, attachments, &criteria())
}

fn player(number: u64) -> UserId {
    UserId::new(number)
}

fn points_at(rank: usize) -> i64 {
    19 - i64::try_from(rank).unwrap()
}

fn sixteen_players() -> Vec<Score> {
    (1..=16)
        .map(|rank| Score {
            user: player(u64::try_from(rank).unwrap()),
            points: points_at(rank),
        })
        .collect()
}

fn row(rank: usize) -> BoardRow {
    BoardRow {
        rank: Some(rank),
        user: player(u64::try_from(rank).unwrap()),
        points: points_at(rank),
        requester: false,
    }
}

fn top_nine() -> Vec<BoardRow> {
    (1..=9).map(row).collect()
}

fn no_points_row() -> BoardRow {
    BoardRow {
        rank: None,
        user: REQUESTER,
        points: 0,
        requester: true,
    }
}

#[test]
fn a_name_in_any_capitals_with_two_images_is_valid() {
    assert_eq!(
        check("dRunken KRAken", &two_images()),
        Checked::Valid {
            criterion: "Drunken Kraken".to_owned(),
            points: 5,
        }
    );
}

#[test]
fn a_misspelt_name_names_the_closest_criterion() {
    assert_eq!(
        check("drunken krakn", &two_images()),
        Checked::Invalid(vec![Problem::UnknownName {
            typed: "drunken krakn".to_owned(),
            closest: "Drunken Kraken".to_owned(),
        }])
    );
}

#[test]
fn an_empty_message_asks_for_a_name() {
    assert_eq!(
        check("  ", &two_images()),
        Checked::Invalid(vec![Problem::NoName {
            example: "Double Down".to_owned(),
        }])
    );
}

#[test]
fn one_screenshot_is_refused() {
    assert_eq!(
        check("Double Down", &[attachment("image/png")]),
        Checked::Invalid(vec![Problem::ScreenshotCount(1)])
    );
}

#[test]
fn three_screenshots_are_refused() {
    assert_eq!(
        check(
            "Double Down",
            &[
                attachment("image/png"),
                attachment("image/jpeg"),
                attachment("image/webp"),
            ]
        ),
        Checked::Invalid(vec![Problem::ScreenshotCount(3)])
    );
}

#[test]
fn no_screenshot_is_refused() {
    assert_eq!(
        check("Double Down", &[]),
        Checked::Invalid(vec![Problem::ScreenshotCount(0)])
    );
}

#[test]
fn a_non_image_file_is_refused() {
    assert_eq!(
        check(
            "Double Down",
            &[
                attachment("image/png"),
                attachment("image/png"),
                attachment("text/plain"),
            ]
        ),
        Checked::Invalid(vec![Problem::NotAnImage])
    );
}

#[test]
fn an_attachment_without_a_type_is_not_an_image() {
    assert_eq!(
        check(
            "Double Down",
            &[attachment("image/png"), Attachment { content_type: None }]
        ),
        Checked::Invalid(vec![Problem::ScreenshotCount(1), Problem::NotAnImage])
    );
}

#[test]
fn extra_words_around_the_name_are_an_unknown_name() {
    assert_eq!(
        check("Double Down please", &two_images()),
        Checked::Invalid(vec![Problem::UnknownName {
            typed: "Double Down please".to_owned(),
            closest: "Double Down".to_owned(),
        }])
    );
}

#[test]
fn a_long_unknown_text_is_cut_in_the_problem() {
    let tied_criteria = vec![criterion("Double Down", 2), criterion("Torpedo Soup", 3)];
    assert_eq!(
        check_submission(&"a".repeat(300), &two_images(), &tied_criteria),
        Checked::Invalid(vec![Problem::UnknownName {
            typed: format!("{}\u{2026}", "a".repeat(100)),
            closest: "Double Down".to_owned(),
        }])
    );
}

#[test]
fn every_problem_is_listed_in_order() {
    assert_eq!(
        check(
            "krakn",
            &[attachment("image/png"), attachment("text/plain")]
        ),
        Checked::Invalid(vec![
            Problem::UnknownName {
                typed: "krakn".to_owned(),
                closest: "Drunken Kraken".to_owned(),
            },
            Problem::ScreenshotCount(1),
            Problem::NotAnImage,
        ])
    );
}

#[test]
fn tied_players_share_a_rank_and_the_next_is_skipped() {
    let scores: Vec<Score> = [27, 22, 19, 18, 18, 15]
        .into_iter()
        .zip(1..)
        .map(|(points, number)| Score {
            user: player(number),
            points,
        })
        .collect();
    let ranks: Vec<usize> = standings(&scores)
        .into_iter()
        .map(|standing| standing.rank)
        .collect();
    assert_eq!(ranks, [1, 2, 3, 4, 4, 6]);
}

#[test]
fn a_requester_in_the_top_ten_sees_the_top_ten() {
    let rows = board_rows(&standings(&sixteen_players()), player(1));
    assert_eq!(rows.len(), 10);
    assert_eq!(
        rows[0],
        BoardRow {
            rank: Some(1),
            user: player(1),
            points: 18,
            requester: true,
        }
    );
}

#[test]
fn a_requester_outside_the_top_ten_takes_the_tenth_line() {
    let rows = board_rows(&standings(&sixteen_players()), player(14));
    let expected: Vec<BoardRow> = top_nine()
        .into_iter()
        .chain([BoardRow {
            rank: Some(14),
            user: player(14),
            points: 5,
            requester: true,
        }])
        .collect();
    assert_eq!(rows, expected);
}

#[test]
fn a_requester_with_no_points_shows_dashes() {
    let rows = board_rows(&standings(&sixteen_players()), REQUESTER);
    let expected: Vec<BoardRow> = top_nine().into_iter().chain([no_points_row()]).collect();
    assert_eq!(rows, expected);
}

#[test]
fn a_requester_with_no_points_on_a_short_board_still_gets_a_line() {
    let four: Vec<Score> = sixteen_players().into_iter().take(4).collect();
    let rows = board_rows(&standings(&four), REQUESTER);
    let expected: Vec<BoardRow> = (1..=4).map(row).chain([no_points_row()]).collect();
    assert_eq!(rows, expected);
}

#[test]
fn start_chooses_by_the_live_competition() {
    assert_eq!(start_choice(None), StartChoice::Setup);
    assert_eq!(start_choice(Some(Status::Stopped)), StartChoice::Resume);
    assert_eq!(
        start_choice(Some(Status::Running)),
        StartChoice::AlreadyRunning
    );
    assert_eq!(
        start_choice(Some(Status::Completed)),
        StartChoice::NeedsCleanup
    );
}

#[test]
fn the_end_is_midnight_after_the_last_day_in_its_zone() {
    let edmonton = Zone::find("America/Edmonton").unwrap();
    assert_eq!(end_unix(date(2026, 11, 30), &edmonton), 1_796_104_800);
}

#[test]
fn complete_waits_for_the_end() {
    assert_eq!(
        may_complete(Status::Running, END, END - 1),
        Err(CompleteRefusal::TooEarly)
    );
    assert_eq!(may_complete(Status::Running, END, END), Ok(()));
    assert_eq!(may_complete(Status::Stopped, END, END + 60), Ok(()));
}

#[test]
fn cleanup_is_refused_while_running() {
    assert_eq!(may_clean_up(Status::Running), Err(CleanupRefusal::Running));
    assert_eq!(may_clean_up(Status::Stopped), Ok(()));
    assert_eq!(may_clean_up(Status::Completed), Ok(()));
}

#[test]
fn only_a_running_competition_before_its_end_checks_messages() {
    assert!(should_check(Status::Running, END, END - 1));
    assert!(!should_check(Status::Running, END, END));
    assert!(!should_check(Status::Stopped, END, END - 100));
}

#[test]
fn a_long_announcement_is_split_between_lines() {
    let lines: Vec<String> = (0..45)
        .map(|number| format!("{number:02}{}", "x".repeat(97)))
        .collect();
    let text = lines.join("\n");
    assert_eq!(text.chars().count(), 4499);
    let chunks = message_chunks(&text);
    assert_eq!(chunks.len(), 3);
    for chunk in &chunks {
        assert!(chunk.chars().count() <= 2000, "{}", chunk.chars().count());
        assert!(
            chunk
                .split('\n')
                .all(|line| lines.contains(&line.to_owned())),
            "{chunk}"
        );
    }
    assert_eq!(chunks.join("\n"), text);
}

#[test]
fn a_single_overlong_line_is_cut() {
    let text = "\u{e9}".repeat(4500);
    let chunks = message_chunks(&text);
    let sizes: Vec<usize> = chunks.iter().map(|chunk| chunk.chars().count()).collect();
    assert_eq!(sizes, [2000, 2000, 500]);
    assert_eq!(chunks.concat(), text);
}

#[test]
fn a_live_check_moves_the_mark_only_after_its_server_caught_up() {
    assert!(!moves_last_seen(CheckPath::Live, false));
    assert!(moves_last_seen(CheckPath::Live, true));
    assert!(moves_last_seen(CheckPath::CatchUp, false));
}

#[test]
fn a_new_connection_forgets_every_caught_up_server() {
    let caught_up = CaughtUp::default();
    assert!(!caught_up.holds(GuildId::new(5)));
    caught_up.finish(GuildId::new(5));
    assert!(caught_up.holds(GuildId::new(5)));
    assert!(!caught_up.holds(GuildId::new(6)));
    caught_up.forget_all();
    assert!(!caught_up.holds(GuildId::new(5)));
}

#[test]
fn a_left_member_or_deleted_role_counts_as_taken_away() {
    assert!(role_already_off(Kind::UnknownMember));
    assert!(role_already_off(Kind::UnknownRole));
    assert!(!role_already_off(Kind::MissingPermissions));
    assert!(!role_already_off(Kind::RateLimited));
}
