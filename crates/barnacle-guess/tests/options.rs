mod common;

use barnacle_guess::RoundOptions;
use common::tier;

#[test]
fn defaults_are_tiers_six_to_eleven_without_the_historical_filter() {
    let options = RoundOptions::default();
    assert_eq!(
        (options.min_tier(), options.max_tier(), options.historical()),
        (tier(6), tier(11), false)
    );
}

#[test]
fn a_reversed_range_is_swapped() {
    let options = RoundOptions::new(Some(tier(9)), Some(tier(4)), Some(true));
    assert_eq!(
        (options.min_tier(), options.max_tier(), options.historical()),
        (tier(4), tier(9), true)
    );
}

#[test]
fn a_single_bound_keeps_the_other_default_and_is_swapped_when_reversed() {
    let only_max = RoundOptions::new(None, Some(tier(3)), None);
    assert_eq!(
        (only_max.min_tier(), only_max.max_tier()),
        (tier(3), tier(6))
    );
    let only_min = RoundOptions::new(Some(tier(8)), None, None);
    assert_eq!(
        (only_min.min_tier(), only_min.max_tier()),
        (tier(8), tier(11))
    );
}

#[test]
fn allows_only_tiers_inside_the_inclusive_range() {
    let options = RoundOptions::new(Some(tier(5)), Some(tier(7)), None);
    assert!(!options.allows(tier(4), false));
    assert!(options.allows(tier(5), false));
    assert!(options.allows(tier(7), false));
    assert!(!options.allows(tier(8), false));
}

#[test]
fn historical_rejects_paper_ships_and_nothing_else() {
    let historical = RoundOptions::new(None, None, Some(true));
    assert!(historical.allows(tier(8), false));
    assert!(!historical.allows(tier(8), true));
    assert!(RoundOptions::default().allows(tier(8), true));
}

#[test]
fn spans_several_tiers_only_when_the_bounds_differ() {
    assert!(RoundOptions::default().spans_several_tiers());
    assert!(!RoundOptions::new(Some(tier(10)), Some(tier(10)), None).spans_several_tiers());
}
