mod common;

use barnacle_bot::attendance::PostsReport;
use barnacle_bot::attendance_store::Attendance;
use barnacle_bot::attendance_store::CreateOutcome;
use barnacle_bot::attendance_store::Event;
use barnacle_bot::attendance_store::EventChange;
use barnacle_bot::event_draft;
use barnacle_bot::event_draft::CreateOptions;
use barnacle_bot::event_draft::Draft;
use barnacle_bot::event_draft::Drafts;
use barnacle_bot::event_draft::EditOptions;
use barnacle_bot::event_draft::PanelView;
use barnacle_bot::event_draft::Ready;
use barnacle_bot::ids::ChannelId;
use barnacle_bot::ids::GuildId;
use barnacle_bot::ids::Ping;
use barnacle_bot::ids::RoleId;
use barnacle_bot::schedule;
use barnacle_bot::schedule::Hours;
use barnacle_bot::schedule::Schedule;
use barnacle_bot::schedule::Span;
use barnacle_bot::schedule::StartTime;
use barnacle_bot::schedule::Weekdays;
use barnacle_bot::schedule::Zone;
use barnacle_bot::schedule::parse_day;
use barnacle_bot::text;
use barnacle_bot::text::Refusal;
use barnacle_bot::wiring::PingVerdict;
use barnacle_guess::UserId;
use common::attendance_pool;
use jiff::civil::Date;
use jiff::civil::Weekday;

const SERVER: GuildId = GuildId::new(1);
const CHANNEL: ChannelId = ChannelId::new(10);
const ADMIN: UserId = UserId::new(100);
const OTHER_ADMIN: UserId = UserId::new(200);
const PANEL: u64 = 7;
const BEFORE_THE_SESSIONS: i64 = 1_791_900_000;
const AFTER_THE_ONE_START: i64 = 1_792_030_000;

fn day(text: &str) -> Date {
    parse_day(text).unwrap()
}

fn create_options(name: &str, first: &str, last: &str) -> CreateOptions {
    CreateOptions {
        name: name.to_owned(),
        first_day: first.to_owned(),
        last_day: last.to_owned(),
        ..CreateOptions::default()
    }
}

fn templated(name: &str, first: &str, last: &str) -> CreateOptions {
    CreateOptions {
        template: Some("Clan Battles".to_owned()),
        ..create_options(name, first, last)
    }
}

fn denver_options(first: &str, last: &str) -> CreateOptions {
    CreateOptions {
        start: Some("17:00".to_owned()),
        hours: Some(4),
        time_zone: Some("America/Denver".to_owned()),
        ..create_options("Ranked Sprint", first, last)
    }
}

fn create(options: &CreateOptions) -> Result<Draft, Refusal> {
    Draft::create(options, SERVER, CHANNEL, ADMIN, BEFORE_THE_SESSIONS)
}

fn season_36() -> Draft {
    create(&templated("Season 36", "2026-10-14", "2026-10-18")).unwrap()
}

fn wednesdays(first: &str, last: &str, zone: &str) -> Span {
    Span::new(
        day(first),
        day(last),
        Schedule::new(
            Weekdays::new(&[Weekday::Wednesday]).unwrap(),
            StartTime::parse("17:00").unwrap(),
            Zone::find(zone).unwrap(),
            Hours::new(4).unwrap(),
        ),
    )
    .unwrap()
}

fn event(id: i64, name: &str, span: Span, created_at_ms: u64) -> Event {
    Event {
        id,
        guild: SERVER,
        channel: CHANNEL,
        name: name.to_owned(),
        span,
        created_by: ADMIN,
        created_at_ms,
        ping_roles: Vec::new(),
        ended_at_ms: None,
        posts_up: false,
    }
}

fn one_clan_battle() -> Event {
    event(
        3,
        "Ranked Sprint",
        Span::new(
            day("2026-10-14"),
            day("2026-10-14"),
            Schedule::clan_battles(),
        )
        .unwrap(),
        1_791_800_000_000,
    )
}

fn denver_sprint() -> Event {
    Event {
        ping_roles: vec![RoleId::new(11)],
        ..event(
            4,
            "Ranked Sprint",
            wednesdays("2026-10-14", "2026-10-28", "America/Denver"),
            1_791_800_000_000,
        )
    }
}

fn rows(view: &PanelView) -> Vec<serde_json::Value> {
    view.components
        .iter()
        .map(|row| serde_json::to_value(row).unwrap())
        .collect()
}

fn picked_days(view: &PanelView) -> Vec<String> {
    rows(view)[0]["components"][0]["options"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|option| option["default"] == true)
        .map(|option| option["value"].as_str().unwrap().to_owned())
        .collect()
}

fn picked_roles(view: &PanelView) -> Vec<u64> {
    rows(view)[1]["components"][0]["default_values"]
        .as_array()
        .map(|values| {
            values
                .iter()
                .map(|value| value["id"].as_u64().unwrap())
                .collect()
        })
        .unwrap_or_default()
}

fn button_labels(view: &PanelView) -> Vec<String> {
    rows(view)[2]["components"]
        .as_array()
        .unwrap()
        .iter()
        .map(|button| button["label"].as_str().unwrap().to_owned())
        .collect()
}

fn accepted(name: &str) -> bool {
    name.contains('/') || name == "UTC"
}

fn in_order(names: &[String]) -> bool {
    names.windows(2).all(|pair| pair[0] < pair[1])
}

fn says_night_or_season(text: &str) -> bool {
    let lower = text.to_lowercase();
    lower.contains("night") || lower.contains("season")
}

#[test]
fn a_draft_refuses_an_hour_count_outside_1_to_24() {
    for count in [0, 25] {
        let options = CreateOptions {
            hours: Some(count),
            ..denver_options("2026-10-14", "2026-10-28")
        };
        let refused = create(&options).unwrap_err();
        assert_eq!(refused, Refusal::HoursOutOfRange, "{count}");
        assert!(
            refused.message().contains("1 to 24"),
            "{}",
            refused.message()
        );
    }
}

#[test]
fn a_draft_refuses_an_unreadable_start_time() {
    let options = CreateOptions {
        start: Some("5pm".to_owned()),
        ..denver_options("2026-10-14", "2026-10-28")
    };
    let refused = create(&options).unwrap_err();
    assert_eq!(refused, Refusal::StartFormat);
    assert!(refused.message().contains("17:00"), "{}", refused.message());
}

#[test]
fn a_draft_refuses_an_abbreviated_time_zone() {
    let options = CreateOptions {
        time_zone: Some("MDT".to_owned()),
        ..denver_options("2026-10-14", "2026-10-28")
    };
    let refused = create(&options).unwrap_err();
    assert_eq!(refused, Refusal::ZoneUnknown);
    assert!(
        refused.message().contains("America/Denver"),
        "{}",
        refused.message()
    );
}

#[test]
fn typing_part_of_a_place_suggests_it() {
    assert_eq!(
        event_draft::zone_suggestions("denver", &[]),
        vec!["America/Denver".to_owned()]
    );
}

#[test]
fn an_empty_query_puts_the_server_zones_first() {
    let suggested = event_draft::zone_suggestions("", &["America/Denver".to_owned()]);
    assert_eq!(suggested.len(), 25);
    assert_eq!(suggested[0], "America/Denver");
    assert_eq!(suggested[1], "Africa/Abidjan");
    let rest = &suggested[1..];
    assert!(in_order(rest), "{rest:?}");
    assert!(rest.iter().all(|name| accepted(name)), "{rest:?}");
    assert!(!rest.contains(&"America/Denver".to_owned()));
}

#[test]
fn a_broad_query_stops_at_25_names() {
    let suggested = event_draft::zone_suggestions("a", &[]);
    assert_eq!(suggested.len(), 25);
    assert!(in_order(&suggested), "{suggested:?}");
    assert!(
        suggested
            .iter()
            .all(|name| accepted(name) && name.to_lowercase().contains('a')),
        "{suggested:?}"
    );
}

#[test]
fn many_server_zones_fill_every_suggestion() {
    let zones = [
        "America/Denver",
        "America/Chicago",
        "America/New_York",
        "America/Los_Angeles",
        "America/Phoenix",
        "America/Anchorage",
        "America/Halifax",
        "America/Toronto",
        "America/Vancouver",
        "America/Bogota",
        "America/Lima",
        "America/Santiago",
        "America/Caracas",
        "America/Havana",
        "America/Panama",
        "Asia/Tokyo",
        "Asia/Seoul",
        "Asia/Shanghai",
        "Asia/Kolkata",
        "Asia/Dubai",
        "Asia/Bangkok",
        "Asia/Jakarta",
        "Asia/Manila",
        "Asia/Karachi",
        "Australia/Sydney",
        "Australia/Perth",
        "Europe/Paris",
        "Europe/Madrid",
        "Europe/Athens",
        "Africa/Cairo",
    ];
    let events: Vec<Event> = zones
        .iter()
        .zip(1..)
        .map(|(zone, id)| {
            event(
                id,
                &format!("Sprint {id}"),
                wednesdays("2026-10-14", "2026-10-18", zone),
                u64::try_from(id).unwrap() * 1_000,
            )
        })
        .collect();
    assert_eq!(
        event_draft::zone_choices("a", Ok(events), BEFORE_THE_SESSIONS),
        [
            "Africa/Cairo",
            "Europe/Athens",
            "Europe/Madrid",
            "Europe/Paris",
            "Australia/Perth",
            "Australia/Sydney",
            "Asia/Karachi",
            "Asia/Manila",
            "Asia/Jakarta",
            "Asia/Bangkok",
            "Asia/Dubai",
            "Asia/Kolkata",
            "Asia/Shanghai",
            "Asia/Seoul",
            "Asia/Tokyo",
            "America/Panama",
            "America/Havana",
            "America/Caracas",
            "America/Santiago",
            "America/Lima",
            "America/Bogota",
            "America/Vancouver",
            "America/Toronto",
            "America/Halifax",
            "America/Anchorage",
        ]
        .map(str::to_owned)
    );
}

#[test]
fn a_query_matching_nothing_suggests_nothing() {
    assert!(event_draft::zone_suggestions("zzzz", &[]).is_empty());
    assert!(event_draft::zone_suggestions("est5", &[]).is_empty());
    assert!(event_draft::zone_suggestions(&"a".repeat(65), &[]).is_empty());
}

#[test]
fn a_server_zone_missing_from_the_database_is_not_suggested() {
    let suggested = event_draft::zone_suggestions("", &["Nowhere/Gone".to_owned()]);
    assert!(!suggested.contains(&"Nowhere/Gone".to_owned()));
    assert_eq!(suggested[0], "Africa/Abidjan");
}

#[tokio::test]
async fn a_failed_read_still_suggests_zones() {
    let pool = attendance_pool().await;
    sqlx::query("INSERT INTO cb_seasons (guild_id, channel_id, name, first_day, last_day, weekdays, start_minute, time_zone, hours, created_by, created_at_ms) VALUES (1, 10, 'Ranked Sprint', '2026-10-14', '2026-10-18', 108, 1410, 'Nowhere/City', 4, 100, 0)")
        .execute(&pool)
        .await
        .unwrap();
    let store = Attendance::with_pool(pool).await.unwrap();
    let read = store.events_in(SERVER).await;
    assert!(read.is_err());
    let suggested = event_draft::zone_choices("", read, BEFORE_THE_SESSIONS);
    assert_eq!(suggested.len(), 25);
    assert_eq!(suggested[0], "Africa/Abidjan");
    assert!(in_order(&suggested), "{suggested:?}");
    assert!(suggested.iter().all(|name| accepted(name)), "{suggested:?}");
}

#[test]
fn only_running_events_lend_their_zones() {
    let finished = event(
        4,
        "Ranked Warmup",
        wednesdays("2026-10-07", "2026-10-07", "America/Denver"),
        4_000,
    );
    assert_eq!(
        finished.span.sessions().last().unwrap().remove_at_unix(),
        1_791_430_200
    );
    let events = [
        event(
            1,
            "Tokyo Sprint",
            wednesdays("2026-10-14", "2026-10-18", "Asia/Tokyo"),
            1_000,
        ),
        event(
            2,
            "Berlin Sprint",
            wednesdays("2026-10-14", "2026-10-18", "Europe/Berlin"),
            2_000,
        ),
        event(
            3,
            "Tokyo Relay",
            wednesdays("2026-10-14", "2026-10-18", "Asia/Tokyo"),
            3_000,
        ),
        finished,
        Event {
            ended_at_ms: Some(1_791_800_000_000),
            ..event(
                5,
                "Paris Sprint",
                wednesdays("2026-10-14", "2026-10-18", "Europe/Paris"),
                5_000,
            )
        },
    ];
    assert_eq!(
        event_draft::server_zones(&events, BEFORE_THE_SESSIONS),
        vec!["Asia/Tokyo".to_owned(), "Europe/Berlin".to_owned()]
    );
}

#[test]
fn a_name_is_trimmed_and_kept() {
    let letters = "a".repeat(100);
    for (typed, kept) in [
        ("  Ranked Sprint  ", "Ranked Sprint"),
        ("*Alpha*", "*Alpha*"),
        ("@everyone raid", "@everyone raid"),
        (letters.as_str(), letters.as_str()),
    ] {
        let draft = create(&templated(typed, "2026-10-14", "2026-10-18")).unwrap();
        assert_eq!(draft.name(), kept);
    }
}

#[test]
fn an_empty_or_overlong_name_is_refused() {
    let letters = "a".repeat(101);
    for typed in ["", "   ", letters.as_str()] {
        assert_eq!(
            create(&templated(typed, "2026-10-14", "2026-10-18")),
            Err(Refusal::NameFormat),
            "{typed}"
        );
    }
}

#[test]
fn no_panel_text_says_night_or_season() {
    let created = create(&denver_options("2026-10-14", "2026-10-28")).unwrap();
    let sprint = denver_sprint();
    let edited = Draft::edit(
        &sprint,
        &EditOptions {
            hours: Some(6),
            ..EditOptions::default()
        },
        ADMIN,
        BEFORE_THE_SESSIONS,
    )
    .unwrap();
    let texts = [
        created.panel(PANEL, BEFORE_THE_SESSIONS).content,
        edited.panel(PANEL, BEFORE_THE_SESSIONS).content,
        text::event_created(&sprint, 3, Some(1_791_932_400)),
        text::event_edited(
            &sprint,
            &PostsReport {
                touched: 2,
                failures: 0,
            },
            &PostsReport {
                touched: 1,
                failures: 0,
            },
        ),
        text::event_edited(
            &sprint,
            &PostsReport {
                touched: 2,
                failures: 1,
            },
            &PostsReport {
                touched: 1,
                failures: 1,
            },
        ),
        text::PANEL_CANCELLED.to_owned(),
        text::CREATE_LABEL.to_owned(),
        text::SAVE_LABEL.to_owned(),
        text::WEEKDAYS_PLACEHOLDER.to_owned(),
        text::ROLES_PLACEHOLDER.to_owned(),
        text::everyone_label(true).to_owned(),
        text::everyone_label(false).to_owned(),
    ];
    let refusals = [
        Refusal::TemplateUnknown,
        Refusal::MissingOptions(vec!["start", "hours", "time_zone"]),
        Refusal::MissingOptions(vec!["hours"]),
        Refusal::StartFormat,
        Refusal::HoursOutOfRange,
        Refusal::ZoneUnknown,
        Refusal::WeekdaysInvalid,
        Refusal::TooManyRoles,
        Refusal::PanelUnknown,
        Refusal::PanelNotYours,
        Refusal::PanelExpired,
        Refusal::ChannelUnknown,
    ]
    .map(|refusal| refusal.message());
    for text in texts.iter().chain(refusals.iter()) {
        assert!(!says_night_or_season(text), "{text}");
    }
}

#[test]
fn the_clan_battles_template_fills_start_hours_and_zone() {
    let draft = create(&templated("Ranked Sprint", "2026-10-14", "2026-10-18")).unwrap();
    let schedule = draft.span().schedule();
    assert_eq!(schedule.start(), StartTime::parse("23:30").unwrap());
    assert_eq!(schedule.zone(), &Zone::find("UTC").unwrap());
    assert_eq!(schedule.hours(), Hours::new(4).unwrap());
    assert_eq!(
        schedule.weekdays(),
        Weekdays::new(&[
            Weekday::Wednesday,
            Weekday::Thursday,
            Weekday::Saturday,
            Weekday::Sunday
        ])
        .unwrap()
    );
}

#[test]
fn a_given_option_replaces_the_templates_value() {
    let options = CreateOptions {
        hours: Some(3),
        time_zone: Some("America/Denver".to_owned()),
        ..templated("Ranked Sprint", "2026-10-14", "2026-10-18")
    };
    let draft = create(&options).unwrap();
    let schedule = draft.span().schedule();
    assert_eq!(schedule.start(), StartTime::parse("23:30").unwrap());
    assert_eq!(schedule.zone(), &Zone::find("America/Denver").unwrap());
    assert_eq!(schedule.hours(), Hours::new(3).unwrap());
}

#[test]
fn a_template_with_an_unreadable_option_is_refused() {
    let options = CreateOptions {
        start: Some("5pm".to_owned()),
        ..templated("Ranked Sprint", "2026-10-14", "2026-10-18")
    };
    assert_eq!(create(&options), Err(Refusal::StartFormat));
}

#[test]
fn without_a_template_each_option_is_needed() {
    let draft = create(&denver_options("2026-10-14", "2026-10-28")).unwrap();
    let schedule = draft.span().schedule();
    assert_eq!(schedule.start(), StartTime::parse("17:00").unwrap());
    assert_eq!(schedule.zone(), &Zone::find("America/Denver").unwrap());
    assert_eq!(schedule.hours(), Hours::new(4).unwrap());
    let only_start = CreateOptions {
        start: Some("17:00".to_owned()),
        ..create_options("Ranked Sprint", "2026-10-14", "2026-10-28")
    };
    assert_eq!(
        create(&only_start),
        Err(Refusal::MissingOptions(vec!["hours", "time_zone"]))
    );
    assert_eq!(
        create(&create_options("Ranked Sprint", "2026-10-14", "2026-10-28")),
        Err(Refusal::MissingOptions(vec!["start", "hours", "time_zone"]))
    );
}

#[test]
fn an_unknown_template_is_refused() {
    let options = CreateOptions {
        template: Some("Raids".to_owned()),
        ..create_options("Ranked Sprint", "2026-10-14", "2026-10-18")
    };
    assert_eq!(create(&options), Err(Refusal::TemplateUnknown));
}

#[test]
fn an_edit_moving_the_schedule_needs_a_session_ahead() {
    let held = one_clan_battle();
    assert_eq!(
        held.span.sessions().next().unwrap().start_unix(),
        1_792_020_600
    );
    assert!(held.running(AFTER_THE_ONE_START));
    let later = EditOptions {
        start: Some("23:45".to_owned()),
        ..EditOptions::default()
    };
    assert_eq!(
        Draft::edit(&held, &later, ADMIN, AFTER_THE_ONE_START),
        Err(Refusal::NoSessionsLeft)
    );
    let thursday = Draft::edit(&held, &EditOptions::default(), ADMIN, AFTER_THE_ONE_START)
        .unwrap()
        .with_weekdays(&["thu".to_owned()])
        .unwrap();
    assert_eq!(
        thursday.ready(AFTER_THE_ONE_START, &[]),
        Err(Refusal::NoSessionsLeft)
    );
}

#[test]
fn an_edit_of_the_name_needs_no_session_ahead() {
    let held = one_clan_battle();
    let renamed = Draft::edit(
        &held,
        &EditOptions {
            name: Some("Ranked Finals".to_owned()),
            ..EditOptions::default()
        },
        ADMIN,
        AFTER_THE_ONE_START,
    )
    .unwrap();
    assert_eq!(
        renamed.ready(AFTER_THE_ONE_START, &[]),
        Ok(Ready::Edit {
            event: 3,
            created_at_ms: 1_791_800_000_000,
            change: EventChange {
                name: Some("Ranked Finals".to_owned()),
                ..EventChange::default()
            },
            warning: None,
        })
    );
    let shorter = Draft::edit(
        &held,
        &EditOptions {
            hours: Some(3),
            ..EditOptions::default()
        },
        ADMIN,
        AFTER_THE_ONE_START,
    )
    .unwrap();
    assert_eq!(
        shorter.ready(AFTER_THE_ONE_START, &[]),
        Ok(Ready::Edit {
            event: 3,
            created_at_ms: 1_791_800_000_000,
            change: EventChange {
                hours: Some(Hours::new(3).unwrap()),
                ..EventChange::default()
            },
            warning: None,
        })
    );
}

#[test]
fn valid_dates_make_a_draft() {
    let week = create(&templated("Ranked Sprint", "2026-10-14", "2026-10-18")).unwrap();
    assert_eq!(week.span().session_count(), 4);
    let one_day = create(&templated("Ranked Sprint", "2026-10-14", "2026-10-14")).unwrap();
    assert_eq!(one_day.span().session_count(), 1);
}

#[test]
fn bad_dates_are_refused_each_in_its_own_way() {
    for (first, last, refused) in [
        ("14/10/2026", "2026-10-18", Refusal::DateFormat),
        ("2026-02-30", "2026-10-18", Refusal::DateFormat),
        ("2026-10-18", "2026-10-14", Refusal::LastDayBeforeFirst),
        ("2026-10-14", "2027-06-01", Refusal::DatesTooFar),
        ("2026-10-12", "2026-10-13", Refusal::NoSessionsLeft),
    ] {
        assert_eq!(
            create(&templated("Ranked Sprint", first, last)),
            Err(refused),
            "{first} to {last}"
        );
    }
}

fn suggested_dates(typed: &str) -> Vec<(String, String)> {
    let today = schedule::today(BEFORE_THE_SESSIONS).unwrap();
    assert_eq!(today, day("2026-10-13"));
    event_draft::date_suggestions(typed, today)
}

#[test]
fn an_empty_date_query_suggests_the_next_25_days() {
    let suggested = suggested_dates("");
    assert_eq!(suggested.len(), 25);
    assert_eq!(
        suggested[0],
        ("2026-10-12 (Monday)".to_owned(), "2026-10-12".to_owned())
    );
    assert_eq!(suggested[24].1, "2026-11-05");
}

#[test]
fn a_month_query_suggests_that_month() {
    let suggested = suggested_dates("2026-12");
    assert_eq!(suggested.len(), 25);
    assert_eq!(suggested[0].1, "2026-12-01");
    assert_eq!(suggested[24].1, "2026-12-25");
}

#[test]
fn a_whole_date_query_suggests_that_date() {
    assert_eq!(
        suggested_dates("2026-10-20"),
        vec![("2026-10-20 (Tuesday)".to_owned(), "2026-10-20".to_owned())]
    );
}

#[test]
fn a_date_query_outside_the_window_suggests_nothing() {
    for typed in ["2027-09", "1999", "2026-10-11"] {
        assert!(suggested_dates(typed).is_empty(), "{typed}");
    }
}

#[test]
fn yesterdays_utc_date_is_suggested() {
    assert_eq!(
        suggested_dates("2026-10-12"),
        vec![("2026-10-12 (Monday)".to_owned(), "2026-10-12".to_owned())]
    );
}

#[test]
fn the_panel_previews_the_event() {
    let view = season_36().panel(PANEL, BEFORE_THE_SESSIONS);
    assert!(view.content.contains("Season 36"), "{}", view.content);
    assert!(
        view.content.contains("<t:1792020600:F>"),
        "{}",
        view.content
    );
    assert!(view.content.contains("4 sessions"), "{}", view.content);
    let rows = rows(&view);
    assert_eq!(rows.len(), 3);
    let days = &rows[0]["components"][0];
    assert_eq!(days["custom_id"], "barnacle-event:7:days");
    assert_eq!(days["min_values"], 1);
    assert_eq!(days["max_values"], 7);
    assert_eq!(
        days["options"]
            .as_array()
            .unwrap()
            .iter()
            .map(|option| option["label"].as_str().unwrap())
            .collect::<Vec<&str>>(),
        [
            "Monday",
            "Tuesday",
            "Wednesday",
            "Thursday",
            "Friday",
            "Saturday",
            "Sunday"
        ]
    );
    assert_eq!(picked_days(&view), ["wed", "thu", "sat", "sun"]);
    let roles = &rows[1]["components"][0];
    assert_eq!(roles["custom_id"], "barnacle-event:7:roles");
    assert_eq!(roles["type"], 6);
    assert_eq!(roles["min_values"], 0);
    assert_eq!(roles["max_values"], 5);
    assert!(picked_roles(&view).is_empty());
    assert_eq!(
        button_labels(&view),
        ["Create", "Ping @everyone: off", "Cancel"]
    );
    assert_eq!(
        rows[2]["components"]
            .as_array()
            .unwrap()
            .iter()
            .map(|button| button["custom_id"].as_str().unwrap())
            .collect::<Vec<&str>>(),
        [
            "barnacle-event:7:save",
            "barnacle-event:7:everyone",
            "barnacle-event:7:cancel"
        ]
    );
}

#[test]
fn a_draft_without_a_template_preselects_the_first_days_weekday() {
    let view = create(&denver_options("2026-10-14", "2026-10-28"))
        .unwrap()
        .panel(PANEL, BEFORE_THE_SESSIONS);
    assert_eq!(picked_days(&view), ["wed"]);
    assert!(view.content.contains("3 sessions"), "{}", view.content);
}

#[test]
fn picking_weekdays_redraws_the_preview() {
    let every_day: Vec<String> = ["mon", "tue", "wed", "thu", "fri", "sat", "sun"]
        .map(str::to_owned)
        .to_vec();
    let view = season_36()
        .with_weekdays(&every_day)
        .unwrap()
        .panel(PANEL, BEFORE_THE_SESSIONS);
    assert!(view.content.contains("5 sessions"), "{}", view.content);
    assert_eq!(
        picked_days(&view),
        ["mon", "tue", "wed", "thu", "fri", "sat", "sun"]
    );
}

#[test]
fn picking_no_or_unknown_weekdays_is_refused() {
    let draft = season_36();
    assert_eq!(draft.with_weekdays(&[]), Err(Refusal::WeekdaysInvalid));
    assert_eq!(
        draft.with_weekdays(&["funday".to_owned()]),
        Err(Refusal::WeekdaysInvalid)
    );
    assert_eq!(draft, season_36());
}

#[test]
fn picking_roles_sets_the_pings() {
    let draft = season_36();
    assert!(draft.with_roles(&[]).unwrap().pings().is_empty());
    assert_eq!(
        draft
            .with_roles(&[RoleId::new(11), RoleId::new(12)])
            .unwrap()
            .pings(),
        vec![Ping::Role(RoleId::new(11)), Ping::Role(RoleId::new(12))]
    );
    let everyone = draft.with_roles(&[RoleId::new(1)]).unwrap();
    assert_eq!(everyone.pings(), vec![Ping::Everyone]);
    let view = everyone.panel(PANEL, BEFORE_THE_SESSIONS);
    assert!(picked_roles(&view).is_empty());
    assert_eq!(button_labels(&view)[1], "Ping @everyone: on");
}

#[test]
fn picking_everyone_with_roles_or_six_roles_is_refused() {
    let draft = season_36();
    assert_eq!(
        draft.with_roles(&[RoleId::new(1), RoleId::new(11)]),
        Err(Refusal::PingMixesEveryone)
    );
    assert_eq!(
        draft.with_roles(&[
            RoleId::new(11),
            RoleId::new(12),
            RoleId::new(13),
            RoleId::new(14),
            RoleId::new(15),
            RoleId::new(16),
        ]),
        Err(Refusal::TooManyRoles)
    );
    assert_eq!(draft, season_36());
}

#[test]
fn the_everyone_button_turns_everyone_on_and_off() {
    let draft = season_36();
    let on = draft.with_everyone().unwrap();
    assert!(on.everyone());
    assert!(
        on.panel(PANEL, BEFORE_THE_SESSIONS)
            .content
            .contains("Pings @everyone.")
    );
    let off = on.with_everyone().unwrap();
    assert!(!off.everyone());
    let roled = off.with_roles(&[RoleId::new(11)]).unwrap();
    assert_eq!(roled.with_everyone(), Err(Refusal::PingMixesEveryone));
    assert_eq!(roled.roles(), [RoleId::new(11)]);
    assert!(!roled.everyone());
    let alone = roled.with_roles(&[]).unwrap().with_everyone().unwrap();
    assert_eq!(
        alone.with_roles(&[RoleId::new(11)]),
        Err(Refusal::PingMixesEveryone)
    );
    assert!(alone.everyone());
    assert!(alone.roles().is_empty());
}

#[test]
fn a_panel_belongs_to_its_admin_for_15_minutes() {
    let drafts = Drafts::default();
    drafts.open(PANEL, season_36(), 1_791_900_000_000);
    assert_eq!(
        drafts.with(PANEL, OTHER_ADMIN, 1_791_900_001_000),
        Err(Refusal::PanelNotYours)
    );
    assert_eq!(
        drafts.with(PANEL, ADMIN, 1_791_900_900_001),
        Err(Refusal::PanelExpired)
    );
    assert_eq!(
        drafts.take(PANEL, ADMIN, 1_791_900_900_001),
        Err(Refusal::PanelExpired)
    );
    assert_eq!(
        drafts.with(8, ADMIN, 1_791_900_001_000),
        Err(Refusal::PanelUnknown)
    );
    assert_eq!(
        drafts.take(8, ADMIN, 1_791_900_001_000),
        Err(Refusal::PanelUnknown)
    );
}

#[test]
fn cancel_discards_the_draft() {
    let drafts = Drafts::default();
    drafts.open(PANEL, season_36(), 1_791_900_000_000);
    assert_eq!(
        drafts.take(PANEL, ADMIN, 1_791_900_001_000),
        Ok(season_36())
    );
    assert_eq!(
        drafts.with(PANEL, ADMIN, 1_791_900_002_000),
        Err(Refusal::PanelUnknown)
    );
}

#[test]
fn create_checks_the_draft_again() {
    let draft = season_36().with_roles(&[RoleId::new(11)]).unwrap();
    let role = Ping::Role(RoleId::new(11));
    assert_eq!(
        draft.ready(1_792_366_201, &[]),
        Err(Refusal::NoSessionsLeft)
    );
    assert_eq!(
        draft.ready(
            BEFORE_THE_SESSIONS,
            &[(role, PingVerdict::BlockedByChannel)]
        ),
        Err(Refusal::PingBlockedByChannel {
            pings: vec![role],
            channel: CHANNEL,
        })
    );
    let Ok(Ready::Create { new, warning }) =
        draft.ready(BEFORE_THE_SESSIONS, &[(role, PingVerdict::Silent)])
    else {
        panic!("a silent ping does not stop a create");
    };
    let saved = Event {
        id: 1,
        guild: new.guild,
        channel: new.channel,
        name: new.name.clone(),
        span: new.span.clone(),
        created_by: new.created_by,
        created_at_ms: new.created_at_ms,
        ping_roles: new.ping_roles.clone(),
        ended_at_ms: None,
        posts_up: false,
    };
    let warned = draft
        .saved_reply(
            &CreateOutcome::Created(saved.clone()),
            warning,
            BEFORE_THE_SESSIONS,
        )
        .unwrap();
    assert!(
        warned.starts_with("<@&11> will not be pinged in <#10>"),
        "{warned}"
    );
    assert!(warned.contains("Season 36 will post in <#10>."), "{warned}");
    assert_eq!(
        draft.saved_reply(&CreateOutcome::NameTaken, None, BEFORE_THE_SESSIONS),
        Err(Refusal::NameTaken("Season 36".to_owned()))
    );
    let Ok(Ready::Create { new, warning }) =
        draft.ready(BEFORE_THE_SESSIONS, &[(role, PingVerdict::Sounds)])
    else {
        panic!("every check passing makes a create");
    };
    assert_eq!(warning, None);
    assert_eq!(new.guild, SERVER);
    assert_eq!(new.channel, CHANNEL);
    assert_eq!(new.name, "Season 36");
    assert_eq!(new.created_by, ADMIN);
    assert_eq!(new.created_at_ms, 1_791_900_000_000);
    assert_eq!(new.ping_roles, vec![RoleId::new(11)]);
    assert_eq!(new.span.first_day(), day("2026-10-14"));
    assert_eq!(new.span.last_day(), day("2026-10-18"));
    let reply = draft
        .saved_reply(&CreateOutcome::Created(saved), warning, BEFORE_THE_SESSIONS)
        .unwrap();
    assert!(
        reply.starts_with("Season 36 will post in <#10>."),
        "{reply}"
    );
}

#[test]
fn an_edit_panel_starts_from_the_event() {
    let draft = Draft::edit(
        &denver_sprint(),
        &EditOptions {
            hours: Some(6),
            ..EditOptions::default()
        },
        ADMIN,
        BEFORE_THE_SESSIONS,
    )
    .unwrap();
    let view = draft.panel(PANEL, BEFORE_THE_SESSIONS);
    assert_eq!(picked_days(&view), ["wed"]);
    assert_eq!(picked_roles(&view), [11]);
    assert!(view.content.contains("6 hours"), "{}", view.content);
    assert_eq!(
        button_labels(&view),
        ["Save", "Ping @everyone: off", "Cancel"]
    );
}

#[test]
fn an_edit_with_nothing_changed_is_refused() {
    let draft = Draft::edit(
        &denver_sprint(),
        &EditOptions::default(),
        ADMIN,
        BEFORE_THE_SESSIONS,
    )
    .unwrap();
    assert_eq!(
        draft.ready(BEFORE_THE_SESSIONS, &[]),
        Err(Refusal::NothingToChange)
    );
}

#[test]
fn an_edit_panel_of_an_everyone_event_shows_the_button_on() {
    let migrated = Event {
        ping_roles: vec![RoleId::new(1)],
        ..denver_sprint()
    };
    let draft = Draft::edit(
        &migrated,
        &EditOptions {
            name: Some("Ranked Finals".to_owned()),
            ..EditOptions::default()
        },
        ADMIN,
        BEFORE_THE_SESSIONS,
    )
    .unwrap();
    let view = draft.panel(PANEL, BEFORE_THE_SESSIONS);
    assert!(picked_roles(&view).is_empty());
    assert_eq!(button_labels(&view)[1], "Ping @everyone: on");
    assert_eq!(draft.ping_roles(), vec![RoleId::new(1)]);
    assert_eq!(
        draft.ready(
            BEFORE_THE_SESSIONS,
            &[(Ping::Everyone, PingVerdict::Sounds)]
        ),
        Ok(Ready::Edit {
            event: 4,
            created_at_ms: 1_791_800_000_000,
            change: EventChange {
                name: Some("Ranked Finals".to_owned()),
                ..EventChange::default()
            },
            warning: None,
        })
    );
}

#[test]
fn the_panel_preview_writes_times_as_timestamps() {
    let view = create(&denver_options("2026-10-14", "2026-10-28"))
        .unwrap()
        .panel(PANEL, BEFORE_THE_SESSIONS);
    assert!(
        view.content.contains("<t:1792018800:F>"),
        "{}",
        view.content
    );
    assert!(!view.content.contains("17:00"), "{}", view.content);
}
