mod common;

use barnacle_guess::RecentShips;
use common::numbered;

#[test]
fn starts_empty() {
    assert!(RecentShips::default().is_empty());
}

#[test]
fn remembering_returns_a_new_memory_and_leaves_the_old_one_alone() {
    let empty = RecentShips::default();
    let one = empty.remember(numbered(1));
    assert!(empty.is_empty());
    assert!(one.contains(&numbered(1)));
    assert!(!one.contains(&numbered(2)));
}

#[test]
fn keeps_only_the_last_twenty_ships() {
    let recent = (0..=20).fold(RecentShips::default(), |recent, number| {
        recent.remember(numbered(number))
    });
    assert_eq!(recent.len(), 20);
    assert!(!recent.contains(&numbered(0)));
    assert!(recent.contains(&numbered(1)));
    assert!(recent.contains(&numbered(20)));
}
