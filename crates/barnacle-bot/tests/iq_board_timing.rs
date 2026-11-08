use barnacle_bot::iq;
use barnacle_bot::iq::BoardDue;
use barnacle_bot::iq::Standing;
use barnacle_guess::UserId;

const CREATED: i64 = 1_000_000;

#[test]
fn marks_count_whole_twelve_hours_since_creation() {
    assert_eq!(iq::board_mark(CREATED, CREATED), 0);
    assert_eq!(iq::board_mark(CREATED, CREATED + 43_199_999), 0);
    assert_eq!(iq::board_mark(CREATED, CREATED + 43_200_000), 1);
    assert_eq!(iq::board_mark(CREATED, CREATED + 129_600_000), 3);
    assert_eq!(iq::board_mark(CREATED, CREATED - 1), 0);
}

#[test]
fn a_new_mark_with_changed_scores_prints() {
    assert_eq!(
        iq::board_due(2, Some(1), Some("11:5"), "11:8"),
        BoardDue::Print
    );
    assert_eq!(iq::board_due(1, None, None, "11:5"), BoardDue::Print);
}

#[test]
fn unchanged_scores_are_skipped_until_the_next_mark() {
    assert_eq!(
        iq::board_due(2, Some(1), Some("11:5"), "11:5"),
        BoardDue::Unchanged
    );
    assert_eq!(iq::board_due(1, None, None, ""), BoardDue::Unchanged);
}

#[test]
fn no_new_mark_means_no_print() {
    assert_eq!(iq::board_due(0, None, None, "11:5"), BoardDue::NotYet);
    assert_eq!(
        iq::board_due(3, Some(3), Some(""), "11:5"),
        BoardDue::NotYet
    );
}

#[test]
fn the_digest_follows_the_shown_scores() {
    let standings = [
        Standing {
            rank: 1,
            user: UserId::new(11),
            points: 8,
        },
        Standing {
            rank: 2,
            user: UserId::new(12),
            points: 5,
        },
    ];
    assert_eq!(iq::board_digest(&standings), "11:8,12:5");
    assert_eq!(iq::board_digest(&[]), "");
}
