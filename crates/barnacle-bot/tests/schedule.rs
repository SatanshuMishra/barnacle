use barnacle_bot::schedule::Hour;
use barnacle_bot::schedule::Hours;
use barnacle_bot::schedule::Schedule;
use barnacle_bot::schedule::Session;
use barnacle_bot::schedule::Span;
use barnacle_bot::schedule::StartTime;
use barnacle_bot::schedule::Weekdays;
use barnacle_bot::schedule::Zone;
use barnacle_bot::schedule::parse_day;
use barnacle_bot::schedule::rehearsal_span;
use jiff::Timestamp;
use jiff::civil::Date;
use jiff::civil::Weekday;
use jiff::tz::TimeZone;

fn day(text: &str) -> Date {
    parse_day(text).unwrap()
}

fn schedule(weekdays: Weekdays, start: &str, zone: &str, hours: i64) -> Schedule {
    Schedule::new(
        weekdays,
        StartTime::parse(start).unwrap(),
        Zone::find(zone).unwrap(),
        Hours::new(hours).unwrap(),
    )
}

fn placed(date: &str, start: &str, hours: i64, zone: &str) -> Session {
    schedule(Weekdays::EVERY_DAY, start, zone, hours)
        .session_at(day(date))
        .unwrap()
}

fn span(first: &str, last: &str, weekdays: &[Weekday], start: &str, zone: &str) -> Span {
    Span::new(
        day(first),
        day(last),
        schedule(Weekdays::new(weekdays).unwrap(), start, zone, 4),
    )
    .unwrap()
}

fn clan_battles_week() -> Span {
    Span::new(
        day("2026-10-14"),
        day("2026-10-18"),
        Schedule::clan_battles(),
    )
    .unwrap()
}

fn labels(span: &Span) -> Vec<String> {
    span.sessions().map(Session::label).collect()
}

fn local(unix: i64, zone: &str) -> String {
    Timestamp::from_second(unix)
        .unwrap()
        .to_zoned(TimeZone::get(zone).unwrap())
        .to_string()
}

const EVERY_WEEKDAY: [Weekday; 7] = [
    Weekday::Monday,
    Weekday::Tuesday,
    Weekday::Wednesday,
    Weekday::Thursday,
    Weekday::Friday,
    Weekday::Saturday,
    Weekday::Sunday,
];

#[test]
fn a_denver_session_in_daylight_time_starts_at_its_local_time() {
    let session = placed("2026-10-14", "17:00", 4, "America/Denver");
    assert_eq!(session.start_unix(), 1_792_018_800);
    assert_eq!(session.end_unix(), 1_792_033_200);
    assert_eq!(session.post_at_unix(), 1_791_932_400);
    assert_eq!(session.start_unix(), 1_792_018_800, "sign-ups close");
    assert_eq!(session.remove_at_unix(), 1_792_035_000);
}

#[test]
fn a_denver_session_in_standard_time_starts_at_its_local_time() {
    let session = placed("2026-11-02", "17:00", 1, "America/Denver");
    assert_eq!(session.start_unix(), 1_793_664_000);
    assert_eq!(session.end_unix(), 1_793_667_600);
    assert_eq!(session.post_at_unix(), 1_793_577_600);
    assert_eq!(session.remove_at_unix(), 1_793_669_400);
}

#[test]
fn a_utc_session_runs_past_midnight() {
    let session = placed("2026-10-14", "23:30", 4, "UTC");
    assert_eq!(session.start_unix(), 1_792_020_600);
    assert_eq!(session.end_unix(), 1_792_035_000);
    assert_eq!(session.remove_at_unix(), 1_792_036_800);
}

#[test]
fn a_start_skipped_by_spring_forward_moves_an_hour_later() {
    let session = placed("2027-03-14", "02:30", 4, "America/Denver");
    assert_eq!(session.start_unix(), 1_805_016_600);
    assert_eq!(
        local(session.start_unix(), "America/Denver"),
        "2027-03-14T03:30:00-06:00[America/Denver]"
    );
}

#[test]
fn a_start_repeated_by_fall_back_takes_the_earlier_instant() {
    let session = placed("2026-11-01", "01:30", 4, "America/Denver");
    assert_eq!(session.start_unix(), 1_793_518_200);
    assert_eq!(
        local(session.start_unix(), "America/Denver"),
        "2026-11-01T01:30:00-06:00[America/Denver]"
    );
}

#[test]
fn a_session_across_fall_back_lasts_its_hours_of_elapsed_time() {
    let session = placed("2026-11-01", "00:30", 4, "America/Denver");
    assert_eq!(session.start_unix(), 1_793_514_600);
    assert_eq!(session.end_unix(), 1_793_529_000);
    assert_eq!(session.end_unix() - session.start_unix(), 14_400);
}

#[test]
fn a_24_hour_session_ends_a_day_later() {
    let session = placed("2026-11-02", "17:00", 24, "America/Denver");
    assert_eq!(session.start_unix(), 1_793_664_000);
    assert_eq!(session.end_unix(), 1_793_750_400);
    assert_eq!(session.remove_at_unix(), 1_793_752_200);
}

#[test]
fn clan_battle_weekdays_give_four_sessions_a_week() {
    let week = span(
        "2026-10-12",
        "2026-10-18",
        &[
            Weekday::Wednesday,
            Weekday::Thursday,
            Weekday::Saturday,
            Weekday::Sunday,
        ],
        "23:30",
        "UTC",
    );
    assert_eq!(
        labels(&week),
        ["2026-10-14", "2026-10-15", "2026-10-17", "2026-10-18"]
    );
}

#[test]
fn one_weekday_on_a_single_date_gives_one_session() {
    let single = span(
        "2026-10-14",
        "2026-10-14",
        &[Weekday::Wednesday],
        "23:30",
        "UTC",
    );
    assert_eq!(labels(&single), ["2026-10-14"]);
    assert_eq!(single.session_count(), 1);
}

#[test]
fn all_seven_weekdays_give_a_session_every_day() {
    let days = span("2026-10-14", "2026-10-16", &EVERY_WEEKDAY, "23:30", "UTC");
    assert_eq!(labels(&days), ["2026-10-14", "2026-10-15", "2026-10-16"]);
}

#[test]
fn a_range_without_its_weekday_gives_no_session() {
    let mondays = span(
        "2026-10-14",
        "2026-10-18",
        &[Weekday::Monday],
        "23:30",
        "UTC",
    );
    assert_eq!(labels(&mondays), Vec::<String>::new());
    assert_eq!(mondays.session_count(), 0);
}

#[test]
fn an_empty_weekday_set_is_refused() {
    assert_eq!(Weekdays::new(&[]), None);
}

#[test]
fn the_weekday_is_read_in_the_event_zone() {
    let kiritimati = span(
        "2026-10-14",
        "2026-10-14",
        &[Weekday::Wednesday],
        "01:00",
        "Pacific/Kiritimati",
    );
    let sessions: Vec<Session> = kiritimati.sessions().collect();
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].label(), "2026-10-14");
    assert_eq!(sessions[0].start_unix(), 1_791_889_200);
    let utc = Timestamp::from_second(1_791_889_200)
        .unwrap()
        .to_zoned(TimeZone::UTC);
    assert_eq!(utc.date(), day("2026-10-13"));
    assert_eq!(utc.weekday(), Weekday::Tuesday);
    assert_eq!((utc.hour(), utc.minute()), (11, 0));
}

#[test]
fn the_first_and_last_days_are_held_and_their_neighbours_are_not() {
    let days = span("2026-10-14", "2026-10-16", &EVERY_WEEKDAY, "23:30", "UTC");
    assert!(!days.holds(day("2026-10-13")));
    assert!(days.holds(day("2026-10-14")));
    assert!(days.holds(day("2026-10-16")));
    assert!(!days.holds(day("2026-10-17")));
    assert_eq!(days.session_on(day("2026-10-13")), None);
    assert_eq!(
        days.session_on(day("2026-10-14")).map(Session::start_unix),
        Some(1_792_020_600)
    );
    assert_eq!(
        days.session_on(day("2026-10-16")).map(Session::start_unix),
        Some(1_792_193_400)
    );
    assert_eq!(days.session_on(day("2026-10-17")), None);
}

#[test]
fn before_the_first_post_nothing_is_due() {
    let week = clan_battles_week();
    let now = 1_791_934_199;
    assert_eq!(week.sessions_left(now), 4);
    assert_eq!(
        week.next_session(now).map(Session::label).as_deref(),
        Some("2026-10-14")
    );
    assert_eq!(week.due_session(now), None);
}

#[test]
fn at_the_first_post_time_the_first_session_is_due() {
    let week = clan_battles_week();
    let now = 1_791_934_200;
    assert_eq!(week.sessions_left(now), 4);
    assert_eq!(
        week.next_session(now).map(Session::label).as_deref(),
        Some("2026-10-14")
    );
    assert_eq!(
        week.due_session(now).map(Session::label).as_deref(),
        Some("2026-10-14")
    );
}

#[test]
fn after_a_start_the_following_session_is_next() {
    let week = clan_battles_week();
    let now = 1_792_107_001;
    assert_eq!(week.sessions_left(now), 2);
    assert_eq!(
        week.next_session(now).map(Session::label).as_deref(),
        Some("2026-10-17")
    );
    assert_eq!(week.due_session(now), None);
}

#[test]
fn after_the_last_start_nothing_is_left() {
    let week = clan_battles_week();
    let now = 1_792_366_201;
    assert_eq!(week.sessions_left(now), 0);
    assert_eq!(week.next_session(now), None);
    assert_eq!(week.due_session(now), None);
    assert_eq!(week.last_moment_unix(), Some(1_792_382_400));
}

#[test]
fn after_the_last_deletion_the_last_moment_has_passed() {
    let week = clan_battles_week();
    let now = 1_792_382_401;
    assert_eq!(week.sessions_left(now), 0);
    assert_eq!(week.last_moment_unix(), Some(1_792_382_400));
    assert!(week.last_moment_unix().unwrap() < now);
}

#[test]
fn an_hour_count_from_1_to_24_is_accepted() {
    assert_eq!(Hours::new(1).map(Hours::get), Some(1));
    assert_eq!(Hours::new(12).map(Hours::get), Some(12));
    assert_eq!(Hours::new(24).map(Hours::get), Some(24));
}

#[test]
fn an_hour_count_outside_1_to_24_is_refused() {
    assert_eq!(Hours::new(0), None);
    assert_eq!(Hours::new(25), None);
    assert_eq!(Hours::new(-1), None);
}

#[test]
fn a_24_hour_start_time_is_read() {
    let read = |text: &str| StartTime::parse(text).map(|start| start.to_string());
    assert_eq!(read("17:00").as_deref(), Some("17:00"));
    assert_eq!(read("5:00").as_deref(), Some("05:00"));
    assert_eq!(read("00:00").as_deref(), Some("00:00"));
    assert_eq!(read("23:59").as_deref(), Some("23:59"));
    assert_eq!(read(" 17:00 ").as_deref(), Some("17:00"));
}

#[test]
fn a_start_time_in_another_form_is_refused() {
    for text in [
        "24:00", "17:60", "5pm", "5:00 PM", "17:00:00", "1700", "noon", "",
    ] {
        assert_eq!(StartTime::parse(text), None, "{text:?}");
    }
}

#[test]
fn a_place_name_is_kept_in_the_database_spelling() {
    let found = |text: &str| Zone::find(text).map(|zone| zone.name().to_owned());
    assert_eq!(found("America/Denver").as_deref(), Some("America/Denver"));
    assert_eq!(found("AMERICA/DENVER").as_deref(), Some("America/Denver"));
    assert_eq!(found("US/Mountain").as_deref(), Some("US/Mountain"));
    assert_eq!(found("UTC").as_deref(), Some("UTC"));
    assert_eq!(found("Etc/GMT+12").as_deref(), Some("Etc/GMT+12"));
    assert_eq!(found(" America/Denver ").as_deref(), Some("America/Denver"));
}

#[test]
fn a_zone_that_is_not_a_place_name_is_refused() {
    let long = "a".repeat(65);
    for text in [
        "MDT",
        "EST",
        "MST7MDT",
        "UTC-6",
        "-06:00",
        "Nowhere/City",
        "",
        long.as_str(),
    ] {
        assert_eq!(Zone::find(text), None, "{text:?}");
    }
}

fn season_35() -> Span {
    Span::new(
        day("2026-09-16"),
        day("2026-11-05"),
        Schedule::clan_battles(),
    )
    .unwrap()
}

fn clan_battles_session(text: &str) -> Session {
    Schedule::clan_battles().session_at(day(text)).unwrap()
}

fn rehearsal_session(text: &str) -> Session {
    Schedule::rehearsal().session_at(day(text)).unwrap()
}

fn clan_battles_span(first: &str, last: &str) -> Span {
    Span::new(day(first), day(last), Schedule::clan_battles()).unwrap()
}

fn rehearsal_week(first: &str, last: &str) -> Span {
    Span::new(day(first), day(last), Schedule::rehearsal()).unwrap()
}

#[test]
fn season_35_has_thirty_sessions() {
    let span = season_35();
    let sessions = labels(&span);
    assert_eq!(sessions.len(), 30);
    assert_eq!(span.session_count(), 30);
    assert_eq!(sessions.first().unwrap(), "2026-09-16");
    assert_eq!(sessions.last().unwrap(), "2026-11-05");
}

#[test]
fn session_moments_match_the_spec() {
    let session = clan_battles_session("2026-11-01");
    assert_eq!(session.start_unix(), 1_793_575_800);
    assert_eq!(session.end_unix(), 1_793_590_200);
    assert_eq!(session.post_at_unix(), 1_793_489_400);
    assert_eq!(session.remove_at_unix(), 1_793_592_000);
    assert_eq!(
        session.hour_start_unix(Hour::new(3).unwrap()),
        1_793_583_000
    );
    assert_eq!(session.hour_end_unix(Hour::new(3).unwrap()), 1_793_586_600);
    assert_eq!(
        session.hour_start_unix(Hour::new(1).unwrap()),
        1_793_575_800
    );
    assert_eq!(
        session.hour_end_unix(Hour::new(4).unwrap()),
        session.end_unix()
    );
    assert_eq!(Hour::new(0), None);
    assert_eq!(Hour::new(25), None);
    assert_eq!(
        session.hours().each().map(Hour::get).collect::<Vec<u8>>(),
        [1, 2, 3, 4]
    );
}

#[test]
fn only_cb_weekdays_hold_clan_battles_sessions() {
    let week = clan_battles_span("2026-09-14", "2026-09-20");
    assert_eq!(week.session_on(day("2026-09-14")), None);
    assert_eq!(week.session_on(day("2026-09-15")), None);
    assert_eq!(week.session_on(day("2026-09-18")), None);
    assert!(week.session_on(day("2026-09-16")).is_some());
    assert!(week.session_on(day("2026-09-17")).is_some());
    assert!(week.session_on(day("2026-09-19")).is_some());
    assert!(week.session_on(day("2026-09-20")).is_some());
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
fn due_session_walks_the_boundaries() {
    let span = season_35();
    let rows = [
        (1_790_078_400_i64, None),
        (1_790_121_600, Some("2026-09-23")),
        (1_790_206_140, Some("2026-09-23")),
        (1_790_206_200, Some("2026-09-24")),
        (1_793_921_400, None),
    ];
    for (now, expected) in rows {
        assert_eq!(
            span.due_session(now).map(Session::label),
            expected.map(str::to_owned),
            "due session at {now}"
        );
    }
}

#[test]
fn an_event_set_up_late_starts_from_the_next_session() {
    let span = season_35();
    let late = 1_790_078_400;
    assert_eq!(span.due_session(late), None);
    assert_eq!(
        span.next_session(late).map(Session::label).as_deref(),
        Some("2026-09-23")
    );
    assert_eq!(span.sessions_left(late), 26);
    assert_eq!(
        span.due_session(1_790_119_800)
            .map(Session::label)
            .as_deref(),
        Some("2026-09-23")
    );
    assert_eq!(
        span.last_moment_unix(),
        Some(clan_battles_session("2026-11-05").remove_at_unix())
    );
    assert!(span.holds(day("2026-09-23")));
    assert!(!span.holds(day("2026-11-08")));
}

#[test]
fn a_span_refuses_a_backwards_range() {
    assert_eq!(
        Span::new(
            day("2026-11-05"),
            day("2026-09-16"),
            Schedule::clan_battles()
        ),
        None
    );
    let single = clan_battles_span("2026-09-16", "2026-09-16");
    assert_eq!(single.session_count(), 1);
    let empty = clan_battles_span("2026-09-14", "2026-09-15");
    assert_eq!(empty.session_count(), 0);
    assert_eq!(empty.last_moment_unix(), None);
    assert_eq!(empty.next_session(0), None);
}

#[test]
fn a_span_more_than_six_months_from_today_is_not_near() {
    let today = parse_day("2026-09-17").unwrap();
    assert!(clan_battles_span("2026-09-16", "2026-11-05").near(today));
    assert!(clan_battles_span("2026-03-17", "2027-03-17").near(today));
    assert!(!clan_battles_span("2026-09-16", "2027-03-18").near(today));
    assert!(!clan_battles_span("2026-03-16", "2026-11-05").near(today));
}

#[test]
fn a_session_can_fall_on_any_day_when_the_schedule_says_so() {
    let week = rehearsal_week("2026-09-14", "2026-09-20");
    assert_eq!(week.schedule().weekdays(), Weekdays::EVERY_DAY);
    assert_eq!(
        labels(&week),
        [
            "2026-09-14",
            "2026-09-15",
            "2026-09-16",
            "2026-09-17",
            "2026-09-18",
            "2026-09-19",
            "2026-09-20",
        ]
    );
    assert_eq!(week.session_count(), 7);
    let friday = rehearsal_session("2026-09-18");
    assert_eq!(friday.start_unix(), 1_789_774_200);
    assert_eq!(friday.end_unix(), friday.start_unix() + 4 * 3600);
    assert_eq!(
        friday.post_at_unix(),
        clan_battles_session("2026-09-17").start_unix()
    );
    assert_eq!(friday.remove_at_unix(), friday.end_unix() + 30 * 60);
    assert!(week.holds(friday.date()));
    let thursday_start = clan_battles_session("2026-09-17").start_unix();
    assert_eq!(
        week.next_session(thursday_start)
            .map(Session::label)
            .as_deref(),
        Some("2026-09-18")
    );
    assert_eq!(
        week.due_session(thursday_start)
            .map(Session::label)
            .as_deref(),
        Some("2026-09-18")
    );
    assert_eq!(week.sessions_left(thursday_start), 3);
    assert_eq!(
        week.last_moment_unix(),
        Some(rehearsal_session("2026-09-20").remove_at_unix())
    );
    assert_eq!(
        Span::new(day("2026-09-20"), day("2026-09-14"), Schedule::rehearsal()),
        None
    );
}

#[test]
fn a_clan_battles_span_still_holds_only_clan_battle_sessions() {
    let week = clan_battles_span("2026-09-14", "2026-09-20");
    assert_eq!(week.schedule().weekdays(), Weekdays::CLAN_BATTLES);
    assert_eq!(
        labels(&week),
        ["2026-09-16", "2026-09-17", "2026-09-19", "2026-09-20"]
    );
    assert_eq!(week.session_count(), 4);
    assert!(!week.holds(day("2026-09-18")));
    assert!(week.holds(day("2026-09-17")));
    assert!(week.holds(day("2026-09-20")));
    assert!(!week.holds(day("2026-09-23")));
    let thursday_start = clan_battles_session("2026-09-17").start_unix();
    assert_eq!(
        week.next_session(thursday_start)
            .map(Session::label)
            .as_deref(),
        Some("2026-09-19")
    );
    assert_eq!(week.due_session(thursday_start), None);
    assert!(season_35().holds(day("2026-09-16")));
    assert_eq!(season_35().sessions_left(0), 30);
    assert_eq!(
        rehearsal_week("2026-09-16", "2026-11-05").sessions_left(0),
        51
    );
}

#[test]
fn a_session_label_round_trips_on_a_day_that_is_not_a_cb_day() {
    let friday = rehearsal_session("2026-09-18");
    assert_eq!(friday.label(), "2026-09-18");
    assert_eq!(
        Schedule::rehearsal().session_at(parse_day(&friday.label()).unwrap()),
        Some(friday)
    );
    let monday = rehearsal_session("2026-09-14");
    assert_eq!(
        Schedule::rehearsal().session_at(parse_day(&monday.label()).unwrap()),
        Some(monday)
    );
    assert_eq!(monday.start_unix(), 1_789_428_600);
    assert_eq!(
        rehearsal_session("2026-09-17"),
        clan_battles_session("2026-09-17")
    );
    assert_eq!(parse_day("2026-9-18"), None);
    assert_eq!(parse_day("2026-09-31"), None);
}

#[test]
fn a_rehearsal_runs_from_tomorrow_for_its_sessions() {
    let span = rehearsal_span(day("2026-10-14"), 3, 1792000000).unwrap();
    assert_eq!(span.first_day(), day("2026-10-15"));
    assert_eq!(span.last_day(), day("2026-10-17"));
    assert_eq!(
        span.schedule().weekdays(),
        Weekdays::new(&EVERY_WEEKDAY).unwrap()
    );
    assert_eq!(span.schedule().start(), StartTime::parse("23:30").unwrap());
    assert_eq!(span.schedule().zone(), &Zone::find("UTC").unwrap());
    assert_eq!(span.schedule().hours(), Hours::new(4).unwrap());
    assert_eq!(span.session_count(), 3);
}

#[test]
fn a_rehearsal_starts_a_day_later_when_tomorrows_post_has_come() {
    let span = rehearsal_span(day("2026-10-14"), 1, 1792020600).unwrap();
    assert_eq!(span.first_day(), day("2026-10-16"));
    assert_eq!(span.last_day(), day("2026-10-16"));
}

#[test]
fn a_rehearsal_session_count_is_clamped() {
    for (sessions, expected) in [(0, 1), (7, 7), (8, 7)] {
        let span = rehearsal_span(day("2026-10-14"), sessions, 1792000000).unwrap();
        assert_eq!(span.session_count(), expected, "{sessions} sessions");
    }
}
