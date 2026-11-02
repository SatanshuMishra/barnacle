mod common;

use std::sync::Arc;

use barnacle_bot::attendance::PostsReport;
use barnacle_bot::attendance::Signups;
use barnacle_bot::attendance_store::Attendance;
use barnacle_bot::attendance_store::CreateOutcome;
use barnacle_bot::attendance_store::Event;
use barnacle_bot::attendance_store::EventChange;
use barnacle_bot::attendance_store::NewEvent;
use barnacle_bot::event_draft;
use barnacle_bot::event_draft::CreateOptions;
use barnacle_bot::event_draft::Draft;
use barnacle_bot::event_draft::Drafts;
use barnacle_bot::event_draft::EditOptions;
use barnacle_bot::event_draft::PanelView;
use barnacle_bot::event_draft::Ready;
use barnacle_bot::event_draft::SaveAnswer;
use barnacle_bot::event_draft::Step;
use barnacle_bot::ids::ChannelId;
use barnacle_bot::ids::GuildId;
use barnacle_bot::ids::Ping;
use barnacle_bot::ids::RoleId;
use barnacle_bot::schedule::Hours;
use barnacle_bot::schedule::Schedule;
use barnacle_bot::schedule::Span;
use barnacle_bot::schedule::StartTime;
use barnacle_bot::schedule::Weekdays;
use barnacle_bot::schedule::Zone;
use barnacle_bot::schedule::ZoneProblem;
use barnacle_bot::schedule::parse_day;
use barnacle_bot::schedule::zone_for;
use barnacle_bot::text;
use barnacle_bot::text::Refusal;
use barnacle_bot::wiring::PanelPart;
use barnacle_bot::wiring::PingVerdict;
use barnacle_bot::wiring::panel_id;
use barnacle_guess::UserId;
use common::attendance_pool;
use common::fakes::FakeBoard;
use jiff::civil::Date;
use jiff::civil::Weekday;
use poise::serenity_prelude as serenity;

const SERVER: GuildId = GuildId::new(1);
const CHANNEL: ChannelId = ChannelId::new(10);
const ADMIN: UserId = UserId::new(100);
const OTHER_ADMIN: UserId = UserId::new(200);
const PANEL: u64 = 7;
const BEFORE_THE_SESSIONS: i64 = 1_791_900_000;
const OPENED_AT_MS: u64 = 1_791_900_000_000;
const AFTER_THE_ONE_START: i64 = 1_792_030_000;
const EVENING_IN_EDMONTON: i64 = 1_793_584_800;
const AFTERNOON_IN_EDMONTON: i64 = 1_793_569_140;

fn day(text: &str) -> Date {
    parse_day(text).unwrap()
}

fn strings(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}

fn create(name: &str, template: Option<&str>) -> Result<Draft, Refusal> {
    create_in(name, template, "America/Denver")
}

fn create_in(name: &str, template: Option<&str>, zone: &str) -> Result<Draft, Refusal> {
    Draft::create(
        &CreateOptions {
            name: name.to_owned(),
            template: template.map(str::to_owned),
            zone: Zone::find(zone),
        },
        SERVER,
        CHANNEL,
        ADMIN,
        BEFORE_THE_SESSIONS,
    )
}

fn templated(name: &str) -> Draft {
    create(name, Some("Clan Battles")).unwrap()
}

fn dated(draft: Draft, first: &str, last: &str) -> Draft {
    draft
        .with_typed_dates(first, last, BEFORE_THE_SESSIONS)
        .unwrap()
}

fn to_pings(draft: Draft) -> Draft {
    let pings = draft.next().unwrap().next().unwrap();
    assert_eq!(pings.step(), Step::Pings);
    pings
}

fn season_36() -> Draft {
    to_pings(dated(templated("Season 36"), "2026-10-14", "2026-10-18"))
}

fn denver_draft(first: &str, last: &str) -> Draft {
    dated(create("Ranked Sprint", None).unwrap(), first, last)
        .with_weekdays(&strings(&["wed"]))
        .unwrap()
        .with_typed_time("17:00", "21:00")
        .unwrap()
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

fn components(view: &PanelView) -> Vec<serde_json::Value> {
    view.components
        .iter()
        .flat_map(|row| {
            serde_json::to_value(row).unwrap()["components"]
                .as_array()
                .cloned()
                .unwrap_or_default()
        })
        .collect()
}

fn component(view: &PanelView, part: PanelPart) -> serde_json::Value {
    let id = panel_id(PANEL, part);
    components(view)
        .into_iter()
        .find(|component| component["custom_id"] == id.as_str())
        .unwrap_or_else(|| panic!("the panel has a component {id}: {:?}", components(view)))
}

fn time_form_values(draft: &Draft) -> Vec<Option<String>> {
    serde_json::to_value(draft.time_modal(PANEL)).unwrap()["components"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| row["components"][0]["value"].as_str().map(str::to_owned))
        .collect()
}

fn picked_days(view: &PanelView) -> Vec<String> {
    component(view, PanelPart::Days)["options"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|option| option["default"] == true)
        .map(|option| option["value"].as_str().unwrap().to_owned())
        .collect()
}

fn picked_roles(view: &PanelView) -> Vec<u64> {
    component(view, PanelPart::Roles)["default_values"]
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
    components(view)
        .iter()
        .filter(|component| component["type"] == 2)
        .map(|button| button["label"].as_str().unwrap().to_owned())
        .collect()
}

fn opened(draft: Draft, now_ms: u64) -> Drafts {
    let drafts = Drafts::default();
    drafts.open(PANEL, draft, now_ms);
    drafts
}

fn stored(drafts: &Drafts) -> Draft {
    drafts.with(PANEL, ADMIN, OPENED_AT_MS).unwrap()
}

async fn empty_signups() -> (Arc<Signups<FakeBoard>>, Attendance) {
    let store = Attendance::with_pool(attendance_pool().await)
        .await
        .unwrap();
    (Signups::new(FakeBoard::new(), store.clone()), store)
}

fn no_pings(_draft: &Draft) -> Vec<(Ping, PingVerdict)> {
    Vec::new()
}

fn judged_as(verdict: PingVerdict) -> impl FnOnce(&Draft) -> Vec<(Ping, PingVerdict)> {
    move |draft: &Draft| {
        draft
            .pings()
            .into_iter()
            .map(|ping| (ping, verdict))
            .collect()
    }
}

fn says_night_or_season(text: &str) -> bool {
    let lower = text.to_lowercase();
    lower.contains("night") || lower.contains("season")
}

fn option_values(options: &[serenity::CreateSelectMenuOption]) -> Vec<String> {
    options
        .iter()
        .map(|option| {
            serde_json::to_value(option).unwrap()["value"]
                .as_str()
                .unwrap()
                .to_owned()
        })
        .collect()
}

#[test]
fn a_draft_refuses_a_typed_time_that_is_not_hh_mm() {
    let draft = templated("Ranked Sprint");
    for typed in [
        "5pm", "24:00", "7", "-1:00", "", "19:5", "19:60", "1900", "19.00",
    ] {
        assert_eq!(
            draft.with_typed_time(typed, "21:00").err(),
            Some(Refusal::TimeFormat),
            "{typed}"
        );
        assert_eq!(
            draft.with_typed_time("19:00", typed).err(),
            Some(Refusal::TimeFormat),
            "{typed}"
        );
    }
}

#[test]
fn a_draft_refuses_a_length_that_is_not_whole_hours() {
    let draft = templated("Ranked Sprint");
    for (start, end) in [("19:00", "19:30"), ("19:15", "21:00"), ("23:30", "00:00")] {
        assert_eq!(
            draft.with_typed_time(start, end).err(),
            Some(Refusal::LengthNotWholeHours),
            "{start} to {end}"
        );
    }
    let refused = Refusal::LengthNotWholeHours.message();
    assert!(refused.contains("whole number of hours"), "{refused}");
}

#[test]
fn an_edit_keeps_its_own_minute() {
    let held = Event {
        span: Span::new(
            day("2026-10-14"),
            day("2026-10-28"),
            Schedule::new(
                Weekdays::new(&[Weekday::Wednesday]).unwrap(),
                StartTime::parse("17:10").unwrap(),
                Zone::find("America/Denver").unwrap(),
                Hours::new(4).unwrap(),
            ),
        )
        .unwrap(),
        ..denver_sprint()
    };
    let draft = Draft::edit(&held, &EditOptions::default(), ADMIN, BEFORE_THE_SESSIONS).unwrap();
    assert_eq!(draft.start(), StartTime::new(17, 10));
    assert_eq!(
        time_form_values(&draft),
        [Some("17:10".to_owned()), Some("21:10".to_owned())]
    );
    let quarter = draft.with_typed_time("17:15", "21:15").unwrap();
    assert_eq!(
        quarter
            .with_typed_time("17:10", "21:10")
            .map(|draft| draft.start()),
        Ok(StartTime::new(17, 10))
    );
    assert_eq!(
        templated("Ranked Sprint")
            .with_typed_time("17:10", "21:10")
            .map(|draft| draft.start()),
        Ok(StartTime::new(17, 10))
    );
}

#[test]
fn a_command_refuses_an_abbreviated_time_zone() {
    assert_eq!(
        zone_for(Some("MDT"), None).err(),
        Some(ZoneProblem::Unknown("MDT".to_owned()))
    );
    assert_eq!(
        zone_for(Some("other"), None).err(),
        Some(ZoneProblem::Unknown("other".to_owned()))
    );
    assert_eq!(
        text::zone_unknown("MDT"),
        "MDT is not a time zone I know. Pick one from the time_zone list."
    );
}

#[test]
fn the_zone_menu_lists_a_picked_zone_first_and_stays_within_25() {
    let server: Vec<String> = [
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
        "America/Lima",
        "America/Bogota",
        "America/Havana",
        "Nowhere/Gone",
    ]
    .map(str::to_owned)
    .to_vec();
    let values = option_values(&event_draft::zone_options(
        &server,
        Some("Pacific/Auckland"),
    ));
    assert_eq!(values.len(), 25);
    assert_eq!(values[0], "Pacific/Auckland");
    assert_eq!(values[1], "Asia/Tokyo");
    assert_eq!(values[18], "America/Havana");
    assert_eq!(values[19], "America/Vancouver");
    assert_eq!(values[23], "America/Toronto");
    assert_eq!(values[24], "other");
    assert!(!values.contains(&"Nowhere/Gone".to_owned()));
}

#[test]
fn the_zone_menu_marks_a_listed_picked_zone_in_place() {
    let options: Vec<serde_json::Value> = event_draft::zone_options(&[], Some("UTC"))
        .iter()
        .map(|option| serde_json::to_value(option).unwrap())
        .collect();
    assert_eq!(options.len(), 10);
    assert_eq!(options[8]["value"], "UTC");
    assert_eq!(options[8]["default"], true);
    assert_eq!(options[9]["description"], "Type any zone");
    assert_eq!(
        options
            .iter()
            .filter(|option| option["default"] == true)
            .count(),
        1
    );
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
        assert_eq!(templated(typed).name(), kept);
    }
}

#[test]
fn an_empty_or_overlong_name_is_refused() {
    let letters = "a".repeat(101);
    for typed in ["", "   ", letters.as_str()] {
        assert_eq!(
            create(typed, Some("Clan Battles")),
            Err(Refusal::NameFormat),
            "{typed}"
        );
    }
}

#[test]
fn no_panel_text_says_night_or_season() {
    let created = denver_draft("2026-10-14", "2026-10-28");
    let cancelled = event_draft::cancel_pressed(
        &opened(created.clone(), OPENED_AT_MS),
        PANEL,
        ADMIN,
        OPENED_AT_MS,
    )
    .unwrap();
    let sprint = denver_sprint();
    let edited = Draft::edit(&sprint, &EditOptions::default(), ADMIN, BEFORE_THE_SESSIONS)
        .unwrap()
        .with_typed_time("17:00", "23:00")
        .unwrap();
    let steps = [
        created.clone(),
        created.next().unwrap(),
        to_pings(created),
        edited.clone(),
        to_pings(edited),
    ];
    let texts = steps
        .iter()
        .map(|draft| draft.panel(PANEL, BEFORE_THE_SESSIONS).content)
        .chain([
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
            cancelled,
        ])
        .chain(
            [
                text::CREATE_LABEL,
                text::SAVE_LABEL,
                text::EVENT_DAYS_PLACEHOLDER,
                text::ROLES_PLACEHOLDER,
                text::NEXT_TIME_LABEL,
                text::NEXT_PINGS_LABEL,
                text::BACK_LABEL,
                text::SET_DATES_LABEL,
                text::SET_TIME_LABEL,
                text::DATES_MODAL_TITLE,
                text::TIME_MODAL_TITLE,
                text::START_INPUT_LABEL,
                text::END_INPUT_LABEL,
                text::ZONE_MISSING,
                text::DAYS_PROMPT,
                text::NO_WEEKDAY_IN_DATES,
                text::EVERY_SESSION_STARTED,
                text::everyone_label(true),
                text::everyone_label(false),
            ]
            .map(str::to_owned),
        )
        .collect::<Vec<String>>();
    let refusals: Vec<String> = common::every_refusal()
        .iter()
        .map(Refusal::message)
        .collect();
    assert_eq!(refusals.len(), 44);
    for text in texts.iter().chain(refusals.iter()) {
        assert!(!says_night_or_season(text), "{text}");
    }
}

#[test]
fn the_clan_battles_template_fills_start_hours_and_zone() {
    let draft = templated("Ranked Sprint");
    assert_eq!(draft.start(), StartTime::parse("23:30"));
    assert_eq!(draft.zone(), Zone::find("UTC").as_ref());
    assert_eq!(draft.hours(), Hours::new(4));
    assert_eq!(
        draft.weekdays(),
        Weekdays::new(&[
            Weekday::Wednesday,
            Weekday::Thursday,
            Weekday::Saturday,
            Weekday::Sunday
        ])
    );
    assert_eq!(draft.first_day(), None);
    assert_eq!(draft.last_day(), None);
}

#[test]
fn a_templates_values_can_be_changed_in_the_panel() {
    let draft = dated(templated("Ranked Sprint"), "2026-10-14", "2026-10-18")
        .with_typed_time("23:30", "02:30")
        .unwrap();
    let span = draft.span().unwrap();
    let schedule = span.schedule();
    assert_eq!(schedule.start(), StartTime::parse("23:30").unwrap());
    assert_eq!(schedule.zone(), &Zone::find("UTC").unwrap());
    assert_eq!(schedule.hours(), Hours::new(3).unwrap());
}

#[test]
fn without_a_template_nothing_is_picked_and_each_step_names_what_is_missing() {
    let draft = create("Ranked Sprint", None).unwrap();
    assert_eq!(draft.weekdays(), None);
    assert_eq!(draft.start(), None);
    assert_eq!(draft.hours(), None);
    assert_eq!(draft.zone(), Zone::find("America/Denver").as_ref());
    assert_eq!(draft.span(), None);
    assert_eq!(
        draft.next(),
        Err(Refusal::MissingOptions(vec!["the dates", "the event days"]))
    );
    let days = dated(draft, "2026-10-14", "2026-10-28")
        .with_weekdays(&strings(&["wed"]))
        .unwrap()
        .next()
        .unwrap();
    assert_eq!(
        days.next(),
        Err(Refusal::MissingOptions(vec!["the session time"]))
    );
    assert_eq!(
        days.ready(BEFORE_THE_SESSIONS, &[]),
        Err(Refusal::MissingOptions(vec!["the session time"]))
    );
}

#[test]
fn an_unknown_template_is_refused() {
    assert_eq!(
        create("Ranked Sprint", Some("Raids")),
        Err(Refusal::TemplateUnknown)
    );
}

#[test]
fn an_edit_moving_the_schedule_needs_a_session_ahead() {
    let held = one_clan_battle();
    assert_eq!(
        held.span.sessions().next().unwrap().start_unix(),
        1_792_020_600
    );
    assert!(held.running(AFTER_THE_ONE_START));
    let opened_edit =
        Draft::edit(&held, &EditOptions::default(), ADMIN, AFTER_THE_ONE_START).unwrap();
    let later = to_pings(opened_edit.with_typed_time("23:45", "03:45").unwrap());
    assert_eq!(
        later.ready(AFTER_THE_ONE_START, &[]),
        Err(Refusal::NoSessionsLeft)
    );
    let view = later.panel(PANEL, AFTER_THE_ONE_START);
    assert_eq!(component(&view, PanelPart::Save)["disabled"], true);
    assert!(
        view.content.contains(text::EVERY_SESSION_STARTED),
        "{}",
        view.content
    );
    let thursday = opened_edit.with_weekdays(&strings(&["thu"])).unwrap();
    assert_eq!(thursday.next(), Err(Refusal::WeekdaysInvalid));
    assert_eq!(
        thursday.ready(AFTER_THE_ONE_START, &[]),
        Err(Refusal::WeekdaysInvalid)
    );
}

#[test]
fn an_edit_of_the_name_needs_no_session_ahead() {
    let held = one_clan_battle();
    let renamed = to_pings(
        Draft::edit(
            &held,
            &EditOptions {
                name: Some("Ranked Finals".to_owned()),
                ..EditOptions::default()
            },
            ADMIN,
            AFTER_THE_ONE_START,
        )
        .unwrap(),
    );
    let view = renamed.panel(PANEL, AFTER_THE_ONE_START);
    assert_ne!(component(&view, PanelPart::Save)["disabled"], true);
    assert!(
        !view.content.contains(text::EVERY_SESSION_STARTED),
        "{}",
        view.content
    );
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
    let shorter = Draft::edit(&held, &EditOptions::default(), ADMIN, AFTER_THE_ONE_START)
        .unwrap()
        .with_typed_time("23:30", "02:30")
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
fn picking_the_held_value_again_changes_nothing() {
    let draft = Draft::edit(
        &denver_sprint(),
        &EditOptions {
            zone: Zone::find("America/Denver"),
            ..EditOptions::default()
        },
        ADMIN,
        BEFORE_THE_SESSIONS,
    )
    .unwrap()
    .with_typed_dates("2026-10-14", "2026-10-28", BEFORE_THE_SESSIONS)
    .unwrap()
    .with_weekdays(&strings(&["wed"]))
    .unwrap()
    .with_typed_time("17:00", "21:00")
    .unwrap();
    assert_eq!(
        to_pings(draft).ready(BEFORE_THE_SESSIONS, &[]),
        Err(Refusal::NothingToChange)
    );
}

#[test]
fn valid_dates_make_a_draft() {
    let week = dated(templated("Ranked Sprint"), "2026-10-14", "2026-10-18");
    assert_eq!(week.span().unwrap().session_count(), 4);
    let one_day = dated(templated("Ranked Sprint"), "2026-10-14", "2026-10-14");
    assert_eq!(one_day.span().unwrap().session_count(), 1);
}

#[test]
fn bad_typed_dates_are_refused_each_in_its_own_way() {
    for (first, last, refused) in [
        ("14/10/2026", "2026-10-18", Refusal::DateFormat),
        ("2026-02-30", "2026-10-18", Refusal::DateFormat),
        ("2026-10-14 (Funday)", "2026-10-18", Refusal::DateFormat),
        ("2026-10-14 Wednesday", "2026-10-18", Refusal::DateFormat),
        ("", "2026-10-18", Refusal::DateFormat),
        ("2026-10-18", "2026-10-14", Refusal::LastDayBeforeFirst),
        ("2026-10-14", "2027-06-01", Refusal::DatesTooFar),
        ("2026-10-12", "2026-10-20", Refusal::FirstDayPast),
    ] {
        assert_eq!(
            templated("Ranked Sprint")
                .with_typed_dates(first, last, BEFORE_THE_SESSIONS)
                .map(|draft| draft.span()),
            Err(refused),
            "{first} to {last}"
        );
    }
}

#[test]
fn a_typed_weekday_is_read_in_any_capitals() {
    let draft = templated("Ranked Sprint")
        .with_typed_dates(
            "2026-10-14 (wednesday)",
            " 2026-10-18 ( SUNDAY ) ",
            BEFORE_THE_SESSIONS,
        )
        .unwrap();
    assert_eq!(draft.first_day(), Some(day("2026-10-14")));
    assert_eq!(draft.last_day(), Some(day("2026-10-18")));
}

#[test]
fn an_edit_may_keep_a_first_day_in_the_past() {
    let draft = Draft::edit(
        &denver_sprint(),
        &EditOptions::default(),
        ADMIN,
        AFTER_THE_ONE_START,
    )
    .unwrap()
    .with_typed_dates("2026-10-14", "2026-11-04", AFTER_THE_ONE_START)
    .unwrap();
    assert_eq!(draft.first_day(), Some(day("2026-10-14")));
    assert_eq!(draft.last_day(), Some(day("2026-11-04")));
    assert!(draft.changed().last_day);
    assert!(!draft.changed().first_day);
}

#[test]
fn no_picked_weekday_between_the_dates_holds_the_days_step() {
    let draft = dated(
        create("Ranked Sprint", None).unwrap(),
        "2026-10-13",
        "2026-10-14",
    )
    .with_weekdays(&strings(&["sat"]))
    .unwrap();
    assert_eq!(draft.next(), Err(Refusal::WeekdaysInvalid));
}

#[test]
fn typed_dates_count_today_in_the_servers_zone() {
    let fresh = create("Ranked Sprint", None).unwrap();
    let edmonton = fresh.with_server_zones(strings(&["America/Edmonton", "Asia/Tokyo"]));
    assert_eq!(
        fresh.with_typed_dates("2026-11-01", "2026-11-03", EVENING_IN_EDMONTON),
        Err(Refusal::FirstDayPast)
    );
    assert!(
        edmonton
            .with_typed_dates("2026-11-01", "2026-11-03", EVENING_IN_EDMONTON)
            .is_ok()
    );
}

#[test]
fn typed_dates_measure_the_window_from_the_same_day_as_create() {
    let tokyo = create_in("Ranked Sprint", None, "Asia/Tokyo")
        .unwrap()
        .with_server_zones(strings(&["Asia/Tokyo"]));
    assert_eq!(
        tokyo.with_typed_dates("2027-04-26", "2027-05-02", AFTERNOON_IN_EDMONTON),
        Err(Refusal::DatesTooFar)
    );
    let draft = tokyo
        .with_typed_dates("2027-04-26", "2027-05-01", AFTERNOON_IN_EDMONTON)
        .unwrap()
        .with_weekdays(&strings(&["mon"]))
        .unwrap()
        .with_typed_time("17:00", "21:00")
        .unwrap();
    assert!(draft.ready(AFTERNOON_IN_EDMONTON, &[]).is_ok());
}

#[test]
fn a_picked_day_outside_the_menu_comes_first_and_the_menu_stays_at_25() {
    let today = day("2026-11-01");
    let options: Vec<serde_json::Value> =
        event_draft::day_options(today, today, Some(day("2026-10-28")))
            .iter()
            .map(|option| serde_json::to_value(option).unwrap())
            .collect();
    assert_eq!(options.len(), 25);
    assert_eq!(options[0]["value"], "2026-10-28");
    assert_eq!(options[0]["label"], "Wed 28 Oct");
    assert_eq!(options[0]["description"], "4 days ago");
    assert_eq!(options[0]["default"], true);
    assert_eq!(options[1]["value"], "2026-11-01");
    assert_eq!(options[24]["value"], "2026-11-24");
    let inside = option_values(&event_draft::day_options(
        today,
        today,
        Some(day("2026-11-03")),
    ));
    assert_eq!(inside.len(), 25);
    assert_eq!(inside[0], "2026-11-01");
    assert_eq!(inside[24], "2026-11-25");
}

#[test]
fn the_days_line_says_what_is_left_to_pick() {
    let fresh = create("Ranked Sprint", None).unwrap();
    let content = |draft: &Draft| draft.panel(PANEL, BEFORE_THE_SESSIONS).content;
    assert_eq!(
        content(&fresh),
        "**Ranked Sprint** in <#10>\n**Step 1 of 3:** Days\nTimes in America/Denver\nPress Set dates to type the first and last day, then pick the event days."
    );
    let weekdays = fresh.with_weekdays(&strings(&["mon", "fri"])).unwrap();
    assert!(
        content(&weekdays)
            .ends_with("\nMondays, Fridays. Now press Set dates to type the first and last day."),
        "{}",
        content(&weekdays)
    );
    let dates = dated(fresh, "2026-11-02", "2026-11-06");
    assert!(
        content(&dates).ends_with("\nRuns from Mon 2 Nov to Fri 6 Nov. Now pick the event days."),
        "{}",
        content(&dates)
    );
    let both = dates.with_weekdays(&strings(&["mon", "fri"])).unwrap();
    assert!(
        content(&both).ends_with("\nMondays, Fridays from Mon 2 Nov to Fri 6 Nov: 2 sessions."),
        "{}",
        content(&both)
    );
}

#[test]
fn the_pings_step_says_who_is_pinged() {
    let pings = season_36();
    let view = pings.panel(PANEL, BEFORE_THE_SESSIONS);
    assert!(view.content.contains("\nPings nobody."), "{}", view.content);
    let roled = pings
        .with_roles(&[RoleId::new(11), RoleId::new(12)])
        .unwrap();
    assert!(
        roled
            .panel(PANEL, BEFORE_THE_SESSIONS)
            .content
            .contains("\nPings <@&11> and <@&12>."),
        "{}",
        roled.panel(PANEL, BEFORE_THE_SESSIONS).content
    );
    assert!(
        !pings
            .back()
            .panel(PANEL, BEFORE_THE_SESSIONS)
            .content
            .contains("Pings"),
        "the Time step leaves pings out"
    );
}

#[test]
fn the_modals_ask_for_days_and_a_session_time() {
    let dates = serde_json::to_value(event_draft::dates_modal(PANEL)).unwrap();
    assert_eq!(dates["custom_id"], panel_id(PANEL, PanelPart::DatesModal));
    assert_eq!(dates["title"], "Event dates");
    let inputs: Vec<&serde_json::Value> = dates["components"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| &row["components"][0])
        .collect();
    assert_eq!(inputs.len(), 2);
    assert_eq!(inputs[0]["custom_id"], "first");
    assert_eq!(inputs[0]["label"], "First day");
    assert_eq!(inputs[0]["placeholder"], "2026-12-14");
    assert_eq!(inputs[0]["max_length"], 32);
    assert_eq!(inputs[0]["style"], 1);
    assert_eq!(inputs[1]["custom_id"], "last");
    assert_eq!(inputs[1]["label"], "Last day");
    assert_eq!(inputs[1]["placeholder"], "2026-12-18");
    assert_eq!(inputs[1]["max_length"], 32);
    let time = serde_json::to_value(templated("Ranked Sprint").time_modal(PANEL)).unwrap();
    assert_eq!(time["custom_id"], panel_id(PANEL, PanelPart::TimeModal));
    assert_eq!(time["title"], "Session time");
    let start = &time["components"][0]["components"][0];
    assert_eq!(start["custom_id"], "start");
    assert_eq!(start["label"], "Start");
    assert_eq!(start["placeholder"], "19:00");
    assert_eq!(start["value"], "23:30");
    assert_eq!(start["max_length"], 16);
    let end = &time["components"][1]["components"][0];
    assert_eq!(end["custom_id"], "end");
    assert_eq!(end["label"], "End");
    assert_eq!(end["value"], "03:30");
}

#[test]
fn only_the_panels_admin_opens_its_modals() {
    let drafts = opened(templated("Ranked Sprint"), OPENED_AT_MS);
    assert!(
        event_draft::modal_opened(
            &drafts,
            PANEL,
            ADMIN,
            OPENED_AT_MS,
            event_draft::dates_modal
        )
        .is_ok()
    );
    assert_eq!(
        event_draft::modal_opened(
            &drafts,
            PANEL,
            OTHER_ADMIN,
            OPENED_AT_MS,
            event_draft::dates_modal
        )
        .err(),
        Some(Refusal::PanelNotYours)
    );
    assert!(event_draft::time_modal_opened(&drafts, PANEL, ADMIN, OPENED_AT_MS).is_ok());
    assert_eq!(
        event_draft::time_modal_opened(&drafts, PANEL, OTHER_ADMIN, OPENED_AT_MS).err(),
        Some(Refusal::PanelNotYours)
    );
}

#[test]
fn typed_dates_redraw_the_stored_panel() {
    let drafts = opened(templated("Ranked Sprint"), OPENED_AT_MS);
    let view = event_draft::dates_typed(
        &drafts,
        PANEL,
        ADMIN,
        "2026-12-14 (Monday)",
        "2026-12-18",
        OPENED_AT_MS,
    )
    .unwrap();
    assert_ne!(component(&view, PanelPart::Next)["disabled"], true);
    assert_eq!(stored(&drafts).first_day(), Some(day("2026-12-14")));
    assert_eq!(
        event_draft::dates_typed(&drafts, PANEL, ADMIN, "soon", "2026-12-18", OPENED_AT_MS).err(),
        Some(Refusal::DateFormat)
    );
    assert_eq!(stored(&drafts).last_day(), Some(day("2026-12-18")));
}

#[test]
fn a_typed_time_redraws_the_stored_panel() {
    let drafts = opened(season_36().back(), OPENED_AT_MS);
    let view =
        event_draft::time_typed(&drafts, PANEL, ADMIN, "20:00", "22:00", OPENED_AT_MS).unwrap();
    assert!(
        view.content
            .contains("Starts at 20:00 UTC and runs 2 hours."),
        "{}",
        view.content
    );
    assert_eq!(stored(&drafts).start(), StartTime::new(20, 0));
    assert_eq!(stored(&drafts).hours(), Hours::new(2));
    assert_eq!(
        event_draft::time_typed(&drafts, PANEL, ADMIN, "8pm", "22:00", OPENED_AT_MS).err(),
        Some(Refusal::TimeFormat)
    );
    assert_eq!(stored(&drafts).start(), StartTime::new(20, 0));
}

#[test]
fn the_panel_previews_the_event() {
    let reply = serde_json::to_value(
        event_draft::panel_reply(season_36().panel(PANEL, BEFORE_THE_SESSIONS))
            .to_slash_initial_response(serenity::CreateInteractionResponseMessage::new()),
    )
    .unwrap();
    assert_eq!(reply["flags"], 64);
    assert_eq!(
        reply["allowed_mentions"],
        serde_json::to_value(serenity::CreateAllowedMentions::new()).unwrap()
    );
    let content = reply["content"].as_str().unwrap();
    assert!(content.contains("Season 36"), "{content}");
    assert!(content.contains("**Step 3 of 3:** Pings"), "{content}");
    assert!(content.contains("<t:1792020600:F>"), "{content}");
    assert!(content.contains("4 sessions"), "{content}");
    assert!(
        content.contains("Starts at 23:30 UTC and runs 4 hours."),
        "{content}"
    );
    let rows = reply["components"].as_array().unwrap().clone();
    assert_eq!(rows.len(), 2);
    let roles = &rows[0]["components"][0];
    assert_eq!(roles["custom_id"], "barnacle-event:7:roles");
    assert_eq!(roles["type"], 6);
    assert_eq!(roles["min_values"], 0);
    assert_eq!(roles["max_values"], 5);
    assert_eq!(
        rows[1]["components"]
            .as_array()
            .unwrap()
            .iter()
            .map(|button| (
                button["custom_id"].as_str().unwrap(),
                button["label"].as_str().unwrap()
            ))
            .collect::<Vec<(&str, &str)>>(),
        [
            ("barnacle-event:7:back", "Back"),
            ("barnacle-event:7:save", "Create"),
            ("barnacle-event:7:everyone", "Ping @everyone: off"),
            ("barnacle-event:7:cancel", "Cancel")
        ]
    );
}

#[test]
fn the_days_step_offers_dates_weekdays_and_its_buttons() {
    let view = season_36().back().back().panel(PANEL, BEFORE_THE_SESSIONS);
    assert_eq!(view.components.len(), 2);
    let days = component(&view, PanelPart::Days);
    assert_eq!(days["min_values"], 1);
    assert_eq!(days["max_values"], 5);
    assert_eq!(days["placeholder"], "Event days");
    assert_eq!(
        days["options"]
            .as_array()
            .unwrap()
            .iter()
            .map(|option| option["label"].as_str().unwrap())
            .collect::<Vec<&str>>(),
        ["Wednesday", "Thursday", "Friday", "Saturday", "Sunday"]
    );
    assert_eq!(picked_days(&view), ["wed", "thu", "sat", "sun"]);
    assert_eq!(button_labels(&view), ["Set dates", "Next: time", "Cancel"]);
    assert_eq!(component(&view, PanelPart::Next)["style"], 1);
    assert_eq!(component(&view, PanelPart::TypeDates)["style"], 2);
}

#[test]
fn the_time_step_has_back_set_time_next_and_cancel() {
    let view = season_36().back().panel(PANEL, BEFORE_THE_SESSIONS);
    assert_eq!(view.components.len(), 1);
    assert_eq!(
        button_labels(&view),
        ["Back", "Set time", "Next: pings", "Cancel"]
    );
    assert_eq!(component(&view, PanelPart::SetTime)["style"], 2);
    assert_eq!(component(&view, PanelPart::Next)["style"], 1);
}

#[test]
fn a_draft_without_a_template_guesses_no_weekday() {
    let view = dated(
        create("Ranked Sprint", None).unwrap(),
        "2026-10-14",
        "2026-10-28",
    )
    .panel(PANEL, BEFORE_THE_SESSIONS);
    assert!(picked_days(&view).is_empty());
    assert_eq!(component(&view, PanelPart::Next)["disabled"], true);
}

#[test]
fn picking_weekdays_redraws_the_preview() {
    let every_day = strings(&["mon", "tue", "wed", "thu", "fri", "sat", "sun"]);
    let drafts = opened(season_36(), OPENED_AT_MS);
    let view =
        event_draft::weekdays_picked(&drafts, PANEL, ADMIN, &every_day, OPENED_AT_MS).unwrap();
    assert!(view.content.contains("5 sessions"), "{}", view.content);
    assert_eq!(stored(&drafts).weekdays(), Some(Weekdays::EVERY_DAY));
    assert_eq!(
        picked_days(
            &stored(&drafts)
                .back()
                .back()
                .panel(PANEL, BEFORE_THE_SESSIONS)
        ),
        ["wed", "thu", "fri", "sat", "sun"]
    );
}

#[test]
fn picking_no_or_unknown_weekdays_is_refused() {
    let drafts = opened(season_36(), OPENED_AT_MS);
    assert_eq!(
        event_draft::weekdays_picked(&drafts, PANEL, ADMIN, &[], OPENED_AT_MS).err(),
        Some(Refusal::WeekdaysInvalid)
    );
    assert_eq!(
        event_draft::weekdays_picked(&drafts, PANEL, ADMIN, &["funday".to_owned()], OPENED_AT_MS)
            .err(),
        Some(Refusal::WeekdaysInvalid)
    );
    assert_eq!(stored(&drafts), season_36());
}

#[test]
fn picking_roles_sets_the_pings() {
    let drafts = opened(season_36(), OPENED_AT_MS);
    event_draft::roles_picked(&drafts, PANEL, ADMIN, &[], OPENED_AT_MS).unwrap();
    assert!(stored(&drafts).pings().is_empty());
    event_draft::roles_picked(
        &drafts,
        PANEL,
        ADMIN,
        &[RoleId::new(11), RoleId::new(12)],
        OPENED_AT_MS,
    )
    .unwrap();
    assert_eq!(
        stored(&drafts).pings(),
        vec![Ping::Role(RoleId::new(11)), Ping::Role(RoleId::new(12))]
    );
    let view =
        event_draft::roles_picked(&drafts, PANEL, ADMIN, &[RoleId::new(1)], OPENED_AT_MS).unwrap();
    assert_eq!(stored(&drafts).pings(), vec![Ping::Everyone]);
    assert!(picked_roles(&view).is_empty());
    assert_eq!(button_labels(&view)[2], "Ping @everyone: on");
}

#[test]
fn picking_everyone_with_roles_or_six_roles_is_refused() {
    let drafts = opened(season_36(), OPENED_AT_MS);
    assert_eq!(
        event_draft::roles_picked(
            &drafts,
            PANEL,
            ADMIN,
            &[RoleId::new(1), RoleId::new(11)],
            OPENED_AT_MS
        )
        .err(),
        Some(Refusal::PingMixesEveryone)
    );
    assert_eq!(
        event_draft::roles_picked(
            &drafts,
            PANEL,
            ADMIN,
            &[
                RoleId::new(11),
                RoleId::new(12),
                RoleId::new(13),
                RoleId::new(14),
                RoleId::new(15),
                RoleId::new(16),
            ],
            OPENED_AT_MS
        )
        .err(),
        Some(Refusal::TooManyRoles)
    );
    assert_eq!(stored(&drafts), season_36());
}

#[test]
fn the_everyone_button_turns_everyone_on_and_off() {
    let drafts = opened(season_36(), OPENED_AT_MS);
    let on = event_draft::everyone_pressed(&drafts, PANEL, ADMIN, OPENED_AT_MS).unwrap();
    assert!(stored(&drafts).everyone());
    assert!(on.content.contains("Pings @everyone."), "{}", on.content);
    assert_eq!(button_labels(&on)[2], "Ping @everyone: on");
    let off = event_draft::everyone_pressed(&drafts, PANEL, ADMIN, OPENED_AT_MS).unwrap();
    assert!(!stored(&drafts).everyone());
    assert_eq!(button_labels(&off)[2], "Ping @everyone: off");
    event_draft::roles_picked(&drafts, PANEL, ADMIN, &[RoleId::new(11)], OPENED_AT_MS).unwrap();
    assert_eq!(
        event_draft::everyone_pressed(&drafts, PANEL, ADMIN, OPENED_AT_MS).err(),
        Some(Refusal::PingMixesEveryone)
    );
    let roled = stored(&drafts);
    assert_eq!(roled.roles(), [RoleId::new(11)]);
    assert!(!roled.everyone());
    event_draft::roles_picked(&drafts, PANEL, ADMIN, &[], OPENED_AT_MS).unwrap();
    event_draft::everyone_pressed(&drafts, PANEL, ADMIN, OPENED_AT_MS).unwrap();
    assert_eq!(
        event_draft::roles_picked(&drafts, PANEL, ADMIN, &[RoleId::new(11)], OPENED_AT_MS).err(),
        Some(Refusal::PingMixesEveryone)
    );
    let alone = stored(&drafts);
    assert!(alone.everyone());
    assert!(alone.roles().is_empty());
}

#[tokio::test]
async fn a_panel_belongs_to_its_admin_for_15_minutes() {
    let (signups, store) = empty_signups().await;
    let drafts = opened(season_36(), 1_791_900_000_000);
    assert_eq!(
        event_draft::save_pressed(
            &drafts,
            PANEL,
            OTHER_ADMIN,
            &signups,
            no_pings,
            1_791_900_001_000
        )
        .await,
        SaveAnswer::Refused(Refusal::PanelNotYours)
    );
    assert_eq!(
        event_draft::save_pressed(&drafts, PANEL, ADMIN, &signups, no_pings, 1_791_900_900_001)
            .await,
        SaveAnswer::Refused(Refusal::PanelExpired)
    );
    assert_eq!(
        event_draft::save_pressed(&drafts, 8, ADMIN, &signups, no_pings, 1_791_900_001_000).await,
        SaveAnswer::Refused(Refusal::PanelUnknown)
    );
    assert!(store.events_in(SERVER).await.unwrap().is_empty());
}

#[tokio::test]
async fn cancel_discards_the_draft() {
    let (signups, store) = empty_signups().await;
    let drafts = opened(season_36(), 1_791_900_000_000);
    assert_eq!(
        event_draft::cancel_pressed(&drafts, PANEL, ADMIN, 1_791_900_001_000),
        Ok("Cancelled. Nothing was saved.".to_owned())
    );
    assert_eq!(
        drafts.with(PANEL, ADMIN, 1_791_900_002_000),
        Err(Refusal::PanelUnknown)
    );
    assert_eq!(
        event_draft::save_pressed(&drafts, PANEL, ADMIN, &signups, no_pings, 1_791_900_002_000)
            .await,
        SaveAnswer::Refused(Refusal::PanelUnknown)
    );
    assert!(store.events_in(SERVER).await.unwrap().is_empty());
}

#[tokio::test]
async fn create_checks_the_draft_again() {
    let draft = season_36().with_roles(&[RoleId::new(11)]).unwrap();
    let role = Ping::Role(RoleId::new(11));

    let (signups, store) = empty_signups().await;
    let drafts = opened(draft.clone(), 1_792_366_201_000);
    assert_eq!(
        event_draft::save_pressed(
            &drafts,
            PANEL,
            ADMIN,
            &signups,
            judged_as(PingVerdict::Sounds),
            1_792_366_201_000
        )
        .await,
        SaveAnswer::Refused(Refusal::NoSessionsLeft)
    );
    assert!(store.events_in(SERVER).await.unwrap().is_empty());
    assert!(drafts.with(PANEL, ADMIN, 1_792_366_201_000).is_ok());

    let (signups, store) = empty_signups().await;
    let drafts = opened(draft.clone(), OPENED_AT_MS);
    assert_eq!(
        event_draft::save_pressed(
            &drafts,
            PANEL,
            ADMIN,
            &signups,
            judged_as(PingVerdict::BlockedByChannel),
            OPENED_AT_MS
        )
        .await,
        SaveAnswer::Refused(Refusal::PingBlockedByChannel {
            pings: vec![role],
            channel: CHANNEL,
        })
    );
    assert!(store.events_in(SERVER).await.unwrap().is_empty());

    let (signups, store) = empty_signups().await;
    let drafts = opened(draft.clone(), OPENED_AT_MS);
    let SaveAnswer::Saved(warned) = event_draft::save_pressed(
        &drafts,
        PANEL,
        ADMIN,
        &signups,
        judged_as(PingVerdict::Silent),
        OPENED_AT_MS,
    )
    .await
    else {
        panic!("a silent ping does not stop a create");
    };
    assert!(
        warned.starts_with("<@&11> will not be pinged in <#10>"),
        "{warned}"
    );
    assert!(warned.contains("Season 36 will post in <#10>."), "{warned}");
    let saved = store.events_in(SERVER).await.unwrap();
    assert_eq!(
        saved
            .iter()
            .map(|event| event.name.as_str())
            .collect::<Vec<&str>>(),
        ["Season 36"]
    );
    assert_eq!(
        drafts.with(PANEL, ADMIN, OPENED_AT_MS),
        Err(Refusal::PanelUnknown)
    );

    let (signups, store) = empty_signups().await;
    let earlier = match store
        .create_event(&NewEvent {
            guild: SERVER,
            channel: CHANNEL,
            name: "Season 36".to_owned(),
            span: draft.span().unwrap(),
            created_by: OTHER_ADMIN,
            created_at_ms: 1_791_800_000_000,
            ping_roles: Vec::new(),
        })
        .await
        .unwrap()
    {
        CreateOutcome::Created(event) => event,
        CreateOutcome::NameTaken => panic!("the earlier Season 36 was not stored"),
    };
    let drafts = opened(draft.clone(), OPENED_AT_MS);
    assert_eq!(
        event_draft::save_pressed(
            &drafts,
            PANEL,
            ADMIN,
            &signups,
            judged_as(PingVerdict::Sounds),
            OPENED_AT_MS
        )
        .await,
        SaveAnswer::Refused(Refusal::NameTaken("Season 36".to_owned()))
    );
    assert_eq!(store.events_in(SERVER).await.unwrap(), vec![earlier]);

    let (signups, store) = empty_signups().await;
    let drafts = opened(draft, OPENED_AT_MS);
    let SaveAnswer::Saved(reply) = event_draft::save_pressed(
        &drafts,
        PANEL,
        ADMIN,
        &signups,
        judged_as(PingVerdict::Sounds),
        OPENED_AT_MS,
    )
    .await
    else {
        panic!("every check passing makes a create");
    };
    assert!(
        reply.starts_with("Season 36 will post in <#10>."),
        "{reply}"
    );
    let saved = store.events_in(SERVER).await.unwrap();
    assert_eq!(saved.len(), 1);
    let created = &saved[0];
    assert_eq!(created.guild, SERVER);
    assert_eq!(created.channel, CHANNEL);
    assert_eq!(created.name, "Season 36");
    assert_eq!(created.created_by, ADMIN);
    assert_eq!(created.created_at_ms, 1_791_900_000_000);
    assert_eq!(created.ping_roles, vec![RoleId::new(11)]);
    assert_eq!(created.span.first_day(), day("2026-10-14"));
    assert_eq!(created.span.last_day(), day("2026-10-18"));
    assert_eq!(created.span.schedule(), &Schedule::clan_battles());
}

#[test]
fn an_edit_panel_starts_from_the_event() {
    let draft = Draft::edit(
        &denver_sprint(),
        &EditOptions::default(),
        ADMIN,
        BEFORE_THE_SESSIONS,
    )
    .unwrap()
    .with_typed_time("17:00", "23:00")
    .unwrap();
    assert_eq!(
        picked_days(&draft.panel(PANEL, BEFORE_THE_SESSIONS)),
        ["wed"]
    );
    let view = to_pings(draft).panel(PANEL, BEFORE_THE_SESSIONS);
    assert_eq!(picked_roles(&view), [11]);
    assert!(view.content.contains("6 hours"), "{}", view.content);
    assert_eq!(
        button_labels(&view),
        ["Back", "Save", "Ping @everyone: off", "Cancel"]
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
    let view = to_pings(draft.clone()).panel(PANEL, BEFORE_THE_SESSIONS);
    assert!(picked_roles(&view).is_empty());
    assert_eq!(button_labels(&view)[2], "Ping @everyone: on");
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
fn the_panel_writes_the_first_session_as_a_timestamp() {
    let view = denver_draft("2026-10-14", "2026-10-28").panel(PANEL, BEFORE_THE_SESSIONS);
    assert!(
        view.content
            .contains("First session: <t:1792018800:F>. 3 sessions, 3 still ahead."),
        "{}",
        view.content
    );
    assert!(
        view.content
            .contains("Starts at 17:00 America/Denver and runs 4 hours."),
        "{}",
        view.content
    );
}
