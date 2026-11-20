use barnacle_bot::schedule::CleanUpDelay;
use barnacle_bot::schedule::PostLead;
use barnacle_bot::schedule::Schedule;
use barnacle_bot::schedule::Session;
use barnacle_bot::schedule::Span;
use barnacle_bot::schedule::parse_day;
use jiff::civil::Date;

fn day(text: &str) -> Date {
    parse_day(text).unwrap()
}

fn lead(days: u8) -> PostLead {
    PostLead::from_days(days).unwrap()
}

fn delay(minutes: u16) -> CleanUpDelay {
    CleanUpDelay::from_minutes(minutes).unwrap()
}

fn first_session(schedule: &Schedule) -> Session {
    schedule.session_at(day("2026-10-14")).unwrap()
}

fn clan_battles_week(schedule: Schedule) -> Span {
    Span::new(day("2026-10-14"), day("2026-10-18"), schedule).unwrap()
}

fn due(span: &Span, now_unix: i64) -> Vec<String> {
    span.due_sessions(now_unix).map(Session::label).collect()
}

#[test]
fn post_lead_takes_one_to_seven_days() {
    assert_eq!(PostLead::from_days(0), None);
    assert_eq!(PostLead::from_days(8), None);
    for days in 1..=7 {
        assert_eq!(PostLead::from_days(days).map(PostLead::days), Some(days));
    }
    let choices: Vec<u8> = PostLead::choices()
        .into_iter()
        .map(PostLead::days)
        .collect();
    assert_eq!(choices, vec![1, 2, 3, 4, 5, 6, 7]);
    assert_eq!(PostLead::default().days(), 1);
}

#[test]
fn a_three_day_post_lead_posts_three_days_before_the_start() {
    let schedule = Schedule::clan_battles().with_post_lead(lead(3));
    let session = first_session(&schedule);
    assert_eq!(session.post_at_unix(), session.start_unix() - 259_200);
    assert_eq!(session.close_unix(), session.start_unix());
    assert_eq!(schedule.post_lead().days(), 3);
}

#[test]
fn a_seven_day_post_lead_posts_a_week_before_the_start() {
    let schedule = Schedule::clan_battles().with_post_lead(lead(7));
    let session = first_session(&schedule);
    assert_eq!(session.post_at_unix(), session.start_unix() - 604_800);
}

#[test]
fn a_schedule_without_a_post_lead_posts_one_day_before() {
    let schedule = Schedule::clan_battles();
    let session = first_session(&schedule);
    assert_eq!(schedule.post_lead().days(), 1);
    assert_eq!(session.post_at_unix(), session.start_unix() - 86_400);
}

#[test]
fn clean_up_delay_takes_half_hours_up_to_four_hours() {
    for minutes in [0, 30, 240] {
        assert_eq!(
            CleanUpDelay::from_minutes(minutes).map(CleanUpDelay::minutes),
            Some(minutes)
        );
    }
    for minutes in [15, 45, 239, 241, 270] {
        assert_eq!(CleanUpDelay::from_minutes(minutes), None);
    }
    let choices: Vec<u16> = CleanUpDelay::choices()
        .into_iter()
        .map(CleanUpDelay::minutes)
        .collect();
    assert_eq!(choices, vec![0, 30, 60, 90, 120, 150, 180, 210, 240]);
    assert_eq!(CleanUpDelay::default().minutes(), 30);
}

#[test]
fn a_four_hour_clean_up_removes_four_hours_after_the_end() {
    let schedule = Schedule::clan_battles().with_clean_up(delay(240));
    let session = first_session(&schedule);
    assert_eq!(session.remove_at_unix(), session.end_unix() + 14_400);
    assert_eq!(schedule.clean_up().minutes(), 240);
}

#[test]
fn a_zero_clean_up_removes_at_the_end() {
    let schedule = Schedule::clan_battles().with_clean_up(delay(0));
    let session = first_session(&schedule);
    assert_eq!(session.remove_at_unix(), session.end_unix());
}

#[test]
fn a_schedule_without_a_clean_up_delay_removes_thirty_minutes_after() {
    let schedule = Schedule::clan_battles();
    let session = first_session(&schedule);
    assert_eq!(schedule.clean_up().minutes(), 30);
    assert_eq!(session.remove_at_unix(), session.end_unix() + 1_800);
}

#[test]
fn an_event_runs_until_its_last_post_is_removed() {
    let schedule = Schedule::rehearsal().with_clean_up(delay(240));
    let span = Span::new(day("2026-10-14"), day("2026-10-14"), schedule.clone()).unwrap();
    let session = first_session(&schedule);
    assert_eq!(span.last_moment_unix(), Some(session.end_unix() + 14_400));
}

#[test]
fn a_passed_post_moment_makes_a_session_still_ahead_due() {
    let span = clan_battles_week(Schedule::clan_battles().with_post_lead(lead(3)));
    let now = first_session(span.schedule()).start_unix() - 86_400;
    assert_eq!(due(&span, now), vec!["2026-10-14", "2026-10-15"]);
    let default_span = clan_battles_week(Schedule::clan_battles());
    assert_eq!(due(&default_span, now), vec!["2026-10-14"]);
}

#[test]
fn a_started_session_is_never_due() {
    let span = clan_battles_week(Schedule::clan_battles().with_post_lead(lead(7)));
    let now = first_session(span.schedule()).start_unix() + 1;
    assert_eq!(
        due(&span, now),
        vec!["2026-10-15", "2026-10-17", "2026-10-18"]
    );
}
