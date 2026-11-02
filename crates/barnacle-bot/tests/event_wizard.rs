use barnacle_bot::attendance_store::Event;
use barnacle_bot::event_draft::CreateOptions;
use barnacle_bot::event_draft::Draft;
use barnacle_bot::event_draft::EditOptions;
use barnacle_bot::event_draft::PanelView;
use barnacle_bot::event_draft::Ready;
use barnacle_bot::event_draft::Step;
use barnacle_bot::ids::ChannelId;
use barnacle_bot::ids::GuildId;
use barnacle_bot::schedule::Hours;
use barnacle_bot::schedule::Schedule;
use barnacle_bot::schedule::Span;
use barnacle_bot::schedule::StartTime;
use barnacle_bot::schedule::Weekdays;
use barnacle_bot::schedule::Zone;
use barnacle_bot::text::Refusal;
use barnacle_bot::wiring::PanelPart;
use barnacle_bot::wiring::panel_id;
use barnacle_guess::UserId;
use jiff::civil::Weekday;
use jiff::civil::date;

const SERVER: GuildId = GuildId::new(1);
const CHANNEL: ChannelId = ChannelId::new(10);
const ADMIN: UserId = UserId::new(100);
const PANEL: u64 = 7;
const NOW: i64 = 1_793_569_140;
const NO_SESSION_LEFT: &str =
    "Every session in these dates has already started. Pick a later start time or more days.";
const WEEKDAY_VALUES: [(Weekday, &str); 7] = [
    (Weekday::Monday, "mon"),
    (Weekday::Tuesday, "tue"),
    (Weekday::Wednesday, "wed"),
    (Weekday::Thursday, "thu"),
    (Weekday::Friday, "fri"),
    (Weekday::Saturday, "sat"),
    (Weekday::Sunday, "sun"),
];

fn strings(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}

fn create(name: &str, template: Option<&str>) -> Result<Draft, Refusal> {
    Draft::create(
        &CreateOptions {
            name: name.to_owned(),
            template: template.map(str::to_owned),
        },
        SERVER,
        CHANNEL,
        ADMIN,
        NOW,
    )
}

fn fresh() -> Draft {
    create("Scrim Nights", None).unwrap()
}

fn days_picked(first: &str, last: &str, weekdays: &[&str]) -> Draft {
    fresh()
        .with_first_day(first)
        .unwrap()
        .with_last_day(last)
        .unwrap()
        .with_weekdays(&strings(weekdays))
        .unwrap()
}

fn timed(draft: Draft, hour: &str, minute: &str, hours: &str, zone: &str) -> Draft {
    draft
        .with_hour(hour)
        .unwrap()
        .with_minute(minute)
        .unwrap()
        .with_hours(hours)
        .unwrap()
        .with_zone(zone)
        .unwrap()
}

fn at_time() -> Draft {
    let draft = days_picked("2026-11-02", "2026-11-06", &["mon", "fri"])
        .next()
        .unwrap();
    assert_eq!(draft.step(), Step::Time);
    draft
}

fn at_pings(first: &str, last: &str, weekdays: &[&str]) -> Draft {
    let time = days_picked(first, last, weekdays).next().unwrap();
    let pings = timed(time, "14", "30", "4", "America/Edmonton")
        .next()
        .unwrap();
    assert_eq!(pings.step(), Step::Pings);
    pings
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

fn options(view: &PanelView, part: PanelPart) -> Vec<serde_json::Value> {
    component(view, part)["options"]
        .as_array()
        .cloned()
        .unwrap_or_else(|| panic!("{} is a string menu", panel_id(PANEL, part)))
}

fn values(view: &PanelView, part: PanelPart) -> Vec<String> {
    options(view, part)
        .iter()
        .map(|option| option["value"].as_str().unwrap().to_owned())
        .collect()
}

fn defaults(view: &PanelView, part: PanelPart) -> Vec<String> {
    options(view, part)
        .iter()
        .filter(|option| option["default"] == true)
        .map(|option| option["value"].as_str().unwrap().to_owned())
        .collect()
}

fn disabled(view: &PanelView, part: PanelPart) -> bool {
    component(view, part)["disabled"].as_bool().unwrap_or(false)
}

fn next_disabled(draft: &Draft) -> bool {
    disabled(&draft.panel(PANEL, NOW), PanelPart::Next)
}

fn denver_wednesdays() -> Event {
    Event {
        id: 4,
        guild: SERVER,
        channel: CHANNEL,
        name: "Ranked Sprint".to_owned(),
        span: Span::new(
            date(2026, 10, 28),
            date(2026, 11, 20),
            Schedule::new(
                Weekdays::new(&[Weekday::Wednesday]).unwrap(),
                StartTime::parse("17:00").unwrap(),
                Zone::find("America/Denver").unwrap(),
                Hours::new(3).unwrap(),
            ),
        )
        .unwrap(),
        created_by: ADMIN,
        created_at_ms: 1_793_000_000_000,
        ping_roles: Vec::new(),
        ended_at_ms: None,
        posts_up: false,
    }
}

fn edit_draft() -> Draft {
    Draft::edit(&denver_wednesdays(), &EditOptions::default(), ADMIN, NOW).unwrap()
}

#[test]
fn create_opens_the_days_step_with_nothing_picked() {
    let draft = create("Scrim Nights", None).unwrap();
    assert_eq!(draft.step(), Step::Days);
    let view = draft.panel(PANEL, NOW);
    assert_eq!(defaults(&view, PanelPart::FirstDay), Vec::<String>::new());
    assert_eq!(defaults(&view, PanelPart::LastDay), Vec::<String>::new());
    assert_eq!(defaults(&view, PanelPart::Days), Vec::<String>::new());
}

#[test]
fn create_is_not_refused_for_sessions_before_weekdays_are_picked() {
    let created = create("Scrim Nights", None);
    assert!(created.is_ok(), "{created:?}");
    assert_ne!(created.err(), Some(Refusal::NoSessionsLeft));
}

#[test]
fn an_empty_name_is_still_refused() {
    assert_eq!(create("   ", None).err(), Some(Refusal::NameFormat));
}

#[test]
fn the_first_day_menu_lists_today_and_the_next_24_days() {
    let view = fresh().panel(PANEL, NOW);
    let first_days = options(&view, PanelPart::FirstDay);
    assert_eq!(first_days.len(), 25);
    assert_eq!(first_days[0]["value"], "2026-11-01");
    assert_eq!(first_days[0]["label"], "Sun 1 Nov");
    assert_eq!(first_days[0]["description"], "Today");
    assert_eq!(first_days[1]["description"], "Tomorrow");
    assert_eq!(first_days[24]["value"], "2026-11-25");
    assert_eq!(first_days[24]["description"], "In 24 days");
}

#[test]
fn next_is_disabled_until_days_and_weekdays_are_picked() {
    let first = fresh().with_first_day("2026-11-02").unwrap();
    assert!(next_disabled(&first));
    let last = first.with_last_day("2026-11-06").unwrap();
    assert!(next_disabled(&last));
    let weekdays = last
        .with_weekdays(&strings(&["mon", "wed", "fri"]))
        .unwrap();
    assert!(!next_disabled(&weekdays));
}

#[test]
fn next_stays_disabled_when_no_picked_weekday_falls_in_the_dates() {
    let draft = days_picked("2026-11-02", "2026-11-03", &["sat"]);
    let view = draft.panel(PANEL, NOW);
    assert!(disabled(&view, PanelPart::Next));
    assert!(
        view.content
            .contains("None of the picked weekdays fall between those dates."),
        "{}",
        view.content
    );
}

#[test]
fn a_last_day_before_the_first_is_refused() {
    let first = fresh().with_first_day("2026-11-10").unwrap();
    assert_eq!(
        first.with_last_day("2026-11-05").err(),
        Some(Refusal::LastDayBeforeFirst)
    );
}

#[test]
fn next_moves_to_the_time_step() {
    let moved = days_picked("2026-11-02", "2026-11-06", &["mon", "fri"]).next();
    assert_eq!(moved.map(|draft| draft.step()), Ok(Step::Time));
}

#[test]
fn a_plain_typed_date_is_read() {
    let draft = fresh()
        .with_typed_dates("2026-12-14", "2026-12-18", NOW)
        .unwrap();
    let view = draft.panel(PANEL, NOW);
    assert_eq!(defaults(&view, PanelPart::FirstDay), ["2026-12-14"]);
    assert_eq!(defaults(&view, PanelPart::LastDay), ["2026-12-18"]);
}

#[test]
fn a_typed_date_with_its_weekday_label_is_read() {
    let draft = fresh()
        .with_typed_dates("2026-11-06 (Friday)", "2026-11-13 (Friday)", NOW)
        .unwrap();
    let view = draft.panel(PANEL, NOW);
    assert_eq!(defaults(&view, PanelPart::FirstDay), ["2026-11-06"]);
    assert_eq!(defaults(&view, PanelPart::LastDay), ["2026-11-13"]);
}

#[test]
fn an_unreadable_typed_date_is_refused() {
    assert_eq!(
        fresh()
            .with_typed_dates("14/12/2026", "2026-12-18", NOW)
            .err(),
        Some(Refusal::DateFormat)
    );
}

#[test]
fn a_typed_first_day_in_the_past_is_refused() {
    assert_eq!(
        fresh()
            .with_typed_dates("2026-10-20", "2026-11-20", NOW)
            .err(),
        Some(Refusal::FirstDayPast)
    );
}

#[test]
fn a_typed_last_day_before_the_first_is_refused() {
    assert_eq!(
        fresh()
            .with_typed_dates("2026-12-18", "2026-12-14", NOW)
            .err(),
        Some(Refusal::LastDayBeforeFirst)
    );
}

#[test]
fn typed_dates_outside_the_window_are_refused() {
    assert_eq!(
        fresh()
            .with_typed_dates("2028-01-01", "2028-01-05", NOW)
            .err(),
        Some(Refusal::DatesTooFar)
    );
}

#[test]
fn the_time_step_offers_hours_minutes_and_lengths() {
    let view = at_time().panel(PANEL, NOW);
    let hours = options(&view, PanelPart::Hour);
    assert_eq!(hours.len(), 24);
    assert_eq!(hours[14]["value"], "14");
    assert_eq!(hours[14]["label"], "14:00");
    assert_eq!(hours[14]["description"], "2 PM");
    assert_eq!(values(&view, PanelPart::Minute), ["00", "15", "30", "45"]);
    let lengths = values(&view, PanelPart::Length);
    assert_eq!(lengths.len(), 24);
    assert_eq!(lengths.first().map(String::as_str), Some("1"));
    assert_eq!(lengths.last().map(String::as_str), Some("24"));
}

#[test]
fn the_zone_menu_puts_server_zones_first_and_ends_with_another_zone() {
    let draft = at_time().with_server_zones(strings(&["Asia/Tokyo", "America/Denver"]));
    let view = draft.panel(PANEL, NOW);
    assert_eq!(
        values(&view, PanelPart::Zone),
        [
            "Asia/Tokyo",
            "America/Denver",
            "America/Vancouver",
            "America/Edmonton",
            "America/Chicago",
            "America/Toronto",
            "America/New_York",
            "Europe/London",
            "Europe/Berlin",
            "UTC",
            "other",
        ]
    );
    let zones = options(&view, PanelPart::Zone);
    let another = zones.last().unwrap();
    assert_eq!(another["value"], "other");
    assert_eq!(another["label"], "Another zone");
}

#[test]
fn next_pings_waits_for_every_time_choice() {
    let hour = at_time().with_hour("14").unwrap();
    assert!(next_disabled(&hour));
    let minute = hour.with_minute("30").unwrap();
    assert!(next_disabled(&minute));
    let hours = minute.with_hours("4").unwrap();
    assert!(next_disabled(&hours));
    let zone = hours.with_zone("America/Edmonton").unwrap();
    let view = zone.panel(PANEL, NOW);
    assert!(!disabled(&view, PanelPart::Next));
    assert!(
        view.content
            .contains("Starts at 14:30 America/Edmonton and runs 4 hours."),
        "{}",
        view.content
    );
}

#[test]
fn a_typed_unknown_zone_is_refused() {
    assert_eq!(
        at_time().with_zone("Mars/Olympus").err(),
        Some(Refusal::ZoneUnknown)
    );
}

#[test]
fn create_is_disabled_when_every_session_has_started() {
    let draft = at_pings("2026-11-01", "2026-11-01", &["sun"]);
    let view = draft.panel(PANEL, NOW);
    assert!(disabled(&view, PanelPart::Save));
    assert!(view.content.contains(NO_SESSION_LEFT), "{}", view.content);
    assert_eq!(draft.ready(NOW, &[]), Err(Refusal::NoSessionsLeft));
}

#[test]
fn create_carries_every_picked_value() {
    let draft = at_pings("2026-11-02", "2026-11-06", &["mon", "wed", "fri"]);
    let new = match draft.ready(NOW, &[]) {
        Ok(Ready::Create { new, .. }) => new,
        other => panic!("a create is ready: {other:?}"),
    };
    let expected = Span::new(
        date(2026, 11, 2),
        date(2026, 11, 6),
        Schedule::new(
            Weekdays::new(&[Weekday::Monday, Weekday::Wednesday, Weekday::Friday]).unwrap(),
            StartTime::parse("14:30").unwrap(),
            Zone::find("America/Edmonton").unwrap(),
            Hours::new(4).unwrap(),
        ),
    )
    .unwrap();
    assert_eq!(new.span, expected);
}

#[test]
fn back_returns_to_the_previous_step() {
    let pings = at_pings("2026-11-02", "2026-11-06", &["mon", "wed", "fri"]);
    let time = pings.back();
    assert_eq!(time.step(), Step::Time);
    let time_view = time.panel(PANEL, NOW);
    assert_eq!(defaults(&time_view, PanelPart::Hour), ["14"]);
    assert_eq!(defaults(&time_view, PanelPart::Minute), ["30"]);
    assert_eq!(defaults(&time_view, PanelPart::Length), ["4"]);
    assert_eq!(defaults(&time_view, PanelPart::Zone), ["America/Edmonton"]);
    let days = time.back();
    assert_eq!(days.step(), Step::Days);
    let days_view = days.panel(PANEL, NOW);
    assert_eq!(defaults(&days_view, PanelPart::FirstDay), ["2026-11-02"]);
    assert_eq!(defaults(&days_view, PanelPart::LastDay), ["2026-11-06"]);
    assert_eq!(defaults(&days_view, PanelPart::Days), ["mon", "wed", "fri"]);
}

#[test]
fn the_clan_battles_template_preselects_weekdays_and_time() {
    let template = Schedule::clan_battles();
    let draft = create("Season 37", Some("Clan Battles")).unwrap();
    let days_view = draft.panel(PANEL, NOW);
    let template_weekdays: Vec<String> = WEEKDAY_VALUES
        .iter()
        .filter(|(day, _)| template.weekdays().contains(*day))
        .map(|(_, value)| (*value).to_owned())
        .collect();
    assert_eq!(defaults(&days_view, PanelPart::Days), template_weekdays);
    let time = draft
        .with_first_day("2026-11-02")
        .unwrap()
        .with_last_day("2026-11-08")
        .unwrap()
        .next()
        .unwrap();
    let time_view = time.panel(PANEL, NOW);
    assert_eq!(
        defaults(&time_view, PanelPart::Hour),
        [format!("{:02}", template.start().hour())]
    );
    assert_eq!(
        defaults(&time_view, PanelPart::Minute),
        [format!("{:02}", template.start().minute())]
    );
    assert_eq!(
        defaults(&time_view, PanelPart::Length),
        [template.hours().get().to_string()]
    );
    assert_eq!(
        defaults(&time_view, PanelPart::Zone),
        [template.zone().name().to_owned()]
    );
}

#[test]
fn edit_preselects_the_events_schedule() {
    let draft = edit_draft();
    let days_view = draft.panel(PANEL, NOW);
    let first_days = options(&days_view, PanelPart::FirstDay);
    assert_eq!(first_days.len(), 25);
    assert_eq!(first_days[0]["value"], "2026-10-28");
    assert_eq!(first_days[0]["default"], true);
    assert_eq!(defaults(&days_view, PanelPart::Days), ["wed"]);
    let time_view = draft.next().unwrap().panel(PANEL, NOW);
    assert_eq!(defaults(&time_view, PanelPart::Hour), ["17"]);
    assert_eq!(defaults(&time_view, PanelPart::Minute), ["00"]);
    assert_eq!(defaults(&time_view, PanelPart::Length), ["3"]);
    assert_eq!(defaults(&time_view, PanelPart::Zone), ["America/Denver"]);
}

#[test]
fn an_unchanged_edit_is_refused_at_save() {
    let pings = edit_draft().next().unwrap().next().unwrap();
    assert_eq!(pings.step(), Step::Pings);
    assert_eq!(pings.ready(NOW, &[]), Err(Refusal::NothingToChange));
}
