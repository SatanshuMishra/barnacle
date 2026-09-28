mod common;

use std::time::Duration;

use barnacle_guess::Quiet;
use barnacle_guess::SeriesLength;
use barnacle_guess::SeriesStanding;
use barnacle_guess::Tally;
use barnacle_guess::UserId;

fn user(id: u64) -> UserId {
    UserId::new(id)
}

#[test]
fn a_series_is_ten_rounds_unless_told_otherwise_and_allows_two_to_twenty() {
    assert_eq!(SeriesLength::default().get(), 10);
    assert!(SeriesLength::new(2).is_some());
    assert!(SeriesLength::new(20).is_some());
    assert!(SeriesLength::new(1).is_none());
    assert!(SeriesLength::new(21).is_none());
}

#[test]
fn standings_count_only_wins_in_this_series() {
    let tally = Tally::default()
        .won(user(1), Duration::from_millis(3100))
        .won(user(2), Duration::from_millis(2500))
        .won(user(1), Duration::from_millis(4000));

    assert_eq!(
        tally.by_wins(),
        vec![
            SeriesStanding {
                user: user(1),
                wins: 2,
                best: Duration::from_millis(3100),
            },
            SeriesStanding {
                user: user(2),
                wins: 1,
                best: Duration::from_millis(2500),
            },
        ]
    );
    assert_eq!(
        tally.by_time(),
        vec![
            SeriesStanding {
                user: user(2),
                wins: 1,
                best: Duration::from_millis(2500),
            },
            SeriesStanding {
                user: user(1),
                wins: 2,
                best: Duration::from_millis(3100),
            },
        ]
    );

    let fresh = Tally::default();
    assert!(fresh.is_empty());
    assert!(fresh.by_wins().is_empty());
    assert!(fresh.by_time().is_empty());
}

#[test]
fn standings_order_wins_then_best_time_then_first_win() {
    let equal_wins_different_time = Tally::default()
        .won(user(1), Duration::from_millis(5000))
        .won(user(2), Duration::from_millis(3000));
    assert_eq!(
        equal_wins_different_time
            .by_wins()
            .into_iter()
            .map(|standing| standing.user)
            .collect::<Vec<_>>(),
        vec![user(2), user(1)]
    );

    let equal_wins_equal_time = Tally::default()
        .won(user(1), Duration::from_millis(5000))
        .won(user(2), Duration::from_millis(5000));
    assert_eq!(
        equal_wins_equal_time
            .by_wins()
            .into_iter()
            .map(|standing| standing.user)
            .collect::<Vec<_>>(),
        vec![user(1), user(2)]
    );
    assert_eq!(
        equal_wins_equal_time
            .by_time()
            .into_iter()
            .map(|standing| standing.user)
            .collect::<Vec<_>>(),
        vec![user(1), user(2)]
    );
}

#[test]
fn three_silent_rounds_in_a_row_call_for_a_check_and_activity_resets_the_count() {
    let after_two = Quiet::default().after_round(false).after_round(false);
    assert!(!after_two.check_due());

    let after_three = after_two.after_round(false);
    assert!(after_three.check_due());

    let mixed = Quiet::default()
        .after_round(false)
        .after_round(false)
        .after_round(true)
        .after_round(false)
        .after_round(false);
    assert!(!mixed.check_due());

    let replaced = Quiet::default();
    assert!(!replaced.check_due());
}
