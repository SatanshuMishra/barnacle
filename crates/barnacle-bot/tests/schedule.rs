use barnacle_bot::schedule::Hour;
use barnacle_bot::schedule::Night;
use barnacle_bot::schedule::Range;
use barnacle_bot::schedule::parse_day;
use jiff::civil::Date;

fn day(text: &str) -> Date {
    parse_day(text).unwrap()
}

fn season_35() -> Range {
    Range::new(day("2026-09-16"), day("2026-11-05")).unwrap()
}

fn night(text: &str) -> Night {
    Night::parse(text).unwrap()
}

#[test]
fn season_35_has_thirty_nights() {
    let range = season_35();
    let nights: Vec<String> = range.nights().map(Night::label).collect();
    assert_eq!(nights.len(), 30);
    assert_eq!(range.night_count(), 30);
    assert_eq!(nights.first().unwrap(), "2026-09-16");
    assert_eq!(nights.last().unwrap(), "2026-11-05");
}

#[test]
fn night_moments_match_the_spec() {
    let night = night("2026-11-01");
    assert_eq!(night.start_unix(), 1_793_575_800);
    assert_eq!(night.end_unix(), 1_793_590_200);
    assert_eq!(night.post_at_unix(), 1_793_489_400);
    assert_eq!(night.remove_at_unix(), 1_793_592_000);
    assert_eq!(night.hour_start_unix(Hour::new(3).unwrap()), 1_793_583_000);
    assert_eq!(night.hour_end_unix(Hour::new(3).unwrap()), 1_793_586_600);
    assert_eq!(night.hour_start_unix(Hour::new(1).unwrap()), 1_793_575_800);
    assert_eq!(night.hour_end_unix(Hour::new(4).unwrap()), night.end_unix());
    assert_eq!(Hour::new(0), None);
    assert_eq!(Hour::new(5), None);
    assert_eq!(Hour::ALL.map(Hour::get), [1, 2, 3, 4]);
}

#[test]
fn only_cb_weekdays_are_nights() {
    assert_eq!(Night::parse("2026-09-14"), None);
    assert_eq!(Night::parse("2026-09-15"), None);
    assert_eq!(Night::parse("2026-09-18"), None);
    assert!(Night::parse("2026-09-16").is_some());
    assert!(Night::parse("2026-09-17").is_some());
    assert!(Night::parse("2026-09-19").is_some());
    assert!(Night::parse("2026-09-20").is_some());
}

#[test]
fn parse_day_is_strict() {
    assert_eq!(parse_day("2026-9-16"), None);
    assert_eq!(parse_day("2026/09/16"), None);
    assert_eq!(parse_day("2026-09-31"), None);
    assert_eq!(parse_day("tomorrow"), None);
    assert_eq!(parse_day(""), None);
    assert_eq!(parse_day("2026-09-16"), Some(day("2026-09-16")));
}

#[test]
fn due_night_walks_the_boundaries() {
    let range = season_35();
    let rows = [
        (1_790_078_400_i64, None),
        (1_790_121_600, Some("2026-09-23")),
        (1_790_206_140, Some("2026-09-23")),
        (1_790_206_200, Some("2026-09-24")),
        (1_793_921_400, None),
    ];
    for (now, expected) in rows {
        assert_eq!(
            range.due_night(now).map(Night::label),
            expected.map(str::to_owned),
            "due night at {now}"
        );
    }
}

#[test]
fn a_season_set_up_late_starts_from_the_next_night() {
    let range = season_35();
    let late = 1_790_078_400;
    assert_eq!(range.due_night(late), None);
    assert_eq!(
        range.next_night(late).map(Night::label).as_deref(),
        Some("2026-09-23")
    );
    assert_eq!(range.nights_left(late), 26);
    assert_eq!(
        range.due_night(1_790_119_800).map(Night::label).as_deref(),
        Some("2026-09-23")
    );
    assert_eq!(
        range.last_moment_unix(),
        Some(night("2026-11-05").remove_at_unix())
    );
    assert!(range.holds(night("2026-09-23")));
    assert!(!range.holds(night("2026-11-08")));
}

#[test]
fn range_rejects_a_backwards_range() {
    assert_eq!(Range::new(day("2026-11-05"), day("2026-09-16")), None);
    let single = Range::new(day("2026-09-16"), day("2026-09-16")).unwrap();
    assert_eq!(single.night_count(), 1);
    let empty = Range::new(day("2026-09-14"), day("2026-09-15")).unwrap();
    assert_eq!(empty.night_count(), 0);
    assert_eq!(empty.last_moment_unix(), None);
    assert_eq!(empty.next_night(0), None);
}

#[test]
fn overlaps_is_symmetric() {
    let season_34 = Range::new(day("2026-06-10"), day("2026-08-02")).unwrap();
    let season_35 = season_35();
    let straddling = Range::new(day("2026-08-02"), day("2026-09-16")).unwrap();
    assert!(!season_34.overlaps(season_35));
    assert!(!season_35.overlaps(season_34));
    assert!(season_34.overlaps(straddling));
    assert!(straddling.overlaps(season_34));
    assert!(season_35.overlaps(straddling));
    assert!(straddling.overlaps(season_35));
    assert!(season_35.overlaps(season_35));
}
