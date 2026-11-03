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
            zone: Zone::find("America/Edmonton"),
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
        .with_typed_dates(first, last, NOW)
        .unwrap()
        .with_weekdays(&strings(weekdays))
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
    let pings = time
        .with_typed_time("14:30", "18:30")
        .unwrap()
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

fn custom_ids(view: &PanelView) -> Vec<String> {
    components(view)
        .iter()
        .filter_map(|component| component["custom_id"].as_str().map(str::to_owned))
        .collect()
}

fn button_labels(view: &PanelView) -> Vec<String> {
    components(view)
        .iter()
        .filter(|component| component["type"] == 2)
        .map(|button| button["label"].as_str().unwrap().to_owned())
        .collect()
}

fn time_form(draft: &Draft) -> Vec<serde_json::Value> {
    serde_json::to_value(draft.time_modal(PANEL)).unwrap()["components"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| row["components"][0].clone())
        .collect()
}

fn time_form_values(draft: &Draft) -> Vec<Option<String>> {
    time_form(draft)
        .iter()
        .map(|input| input["value"].as_str().map(str::to_owned))
        .collect()
}

#[test]
fn create_opens_the_days_step_with_nothing_picked() {
    let draft = create("Scrim Nights", None).unwrap();
    assert_eq!(draft.step(), Step::Days);
    assert_eq!(draft.first_day(), None);
    assert_eq!(draft.last_day(), None);
    assert_eq!(draft.weekdays(), None);
    let view = draft.panel(PANEL, NOW);
    assert!(
        !custom_ids(&view).contains(&panel_id(PANEL, PanelPart::Days)),
        "{:?}",
        custom_ids(&view)
    );
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
fn the_days_step_starts_with_set_dates_next_and_cancel() {
    let view = fresh().panel(PANEL, NOW);
    assert_eq!(view.components.len(), 1);
    assert_eq!(button_labels(&view), ["Set dates", "Next: time", "Cancel"]);
}

#[test]
fn next_is_disabled_until_days_and_weekdays_are_picked() {
    assert!(next_disabled(&fresh()));
    let dates = fresh()
        .with_typed_dates("2026-11-02", "2026-11-06", NOW)
        .unwrap();
    assert!(next_disabled(&dates));
    let weekdays = dates
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
            .contains("None of the picked event days fall between those dates."),
        "{}",
        view.content
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
    assert_eq!(draft.first_day(), Some(date(2026, 12, 14)));
    assert_eq!(draft.last_day(), Some(date(2026, 12, 18)));
    let view = draft.panel(PANEL, NOW);
    assert_eq!(
        values(&view, PanelPart::Days),
        ["mon", "tue", "wed", "thu", "fri"]
    );
}

#[test]
fn a_typed_date_with_its_weekday_label_is_read() {
    let draft = fresh()
        .with_typed_dates("2026-11-06 (Friday)", "2026-11-13 (Friday)", NOW)
        .unwrap();
    assert_eq!(draft.first_day(), Some(date(2026, 11, 6)));
    assert_eq!(draft.last_day(), Some(date(2026, 11, 13)));
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
fn the_time_form_asks_for_start_and_end() {
    let modal = serde_json::to_value(at_time().time_modal(PANEL)).unwrap();
    assert_eq!(modal["custom_id"], "barnacle-event:7:time-modal");
    assert_eq!(modal["title"], "Session time");
    let inputs = time_form(&at_time());
    assert_eq!(inputs.len(), 2);
    assert_eq!(inputs[0]["custom_id"], "start");
    assert_eq!(inputs[0]["label"], "Start");
    assert_eq!(inputs[0]["placeholder"], "19:00");
    assert_eq!(inputs[0]["style"], 1);
    assert_eq!(inputs[1]["custom_id"], "end");
    assert_eq!(inputs[1]["label"], "End");
    assert_eq!(inputs[1]["placeholder"], "19:00");
    assert_eq!(time_form_values(&at_time()), [None, None]);
}

#[test]
fn the_time_step_names_its_zone_and_offers_no_menu() {
    let view = at_time().panel(PANEL, NOW);
    assert_eq!(
        button_labels(&view),
        ["Back", "Set time", "Next: pings", "Cancel"]
    );
    assert!(
        components(&view)
            .iter()
            .all(|component| component["type"] == 2),
        "{:?}",
        components(&view)
    );
    assert!(
        view.content.contains("Times in America/Edmonton"),
        "{}",
        view.content
    );
}

#[test]
fn next_pings_waits_for_the_session_time() {
    assert!(next_disabled(&at_time()));
    let timed = at_time().with_typed_time("14:30", "18:30").unwrap();
    let view = timed.panel(PANEL, NOW);
    assert!(!disabled(&view, PanelPart::Next));
    assert!(
        view.content
            .contains("Starts at 14:30 America/Edmonton and runs 4 hours."),
        "{}",
        view.content
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
    assert_eq!(
        time_form_values(&time),
        [Some("14:30".to_owned()), Some("18:30".to_owned())]
    );
    let days = time.back();
    assert_eq!(days.step(), Step::Days);
    assert_eq!(days.first_day(), Some(date(2026, 11, 2)));
    assert_eq!(days.last_day(), Some(date(2026, 11, 6)));
    let days_view = days.panel(PANEL, NOW);
    assert_eq!(defaults(&days_view, PanelPart::Days), ["mon", "wed", "fri"]);
}

#[test]
fn the_clan_battles_template_preselects_weekdays_and_time() {
    let template = Schedule::clan_battles();
    let draft = create("Season 37", Some("Clan Battles")).unwrap();
    assert_eq!(draft.weekdays(), Some(template.weekdays()));
    let dated = draft
        .with_typed_dates("2026-11-02", "2026-11-08", NOW)
        .unwrap();
    let template_weekdays: Vec<String> = WEEKDAY_VALUES
        .iter()
        .filter(|(day, _)| template.weekdays().contains(*day))
        .map(|(_, value)| (*value).to_owned())
        .collect();
    assert_eq!(
        defaults(&dated.panel(PANEL, NOW), PanelPart::Days),
        template_weekdays
    );
    let time = dated.next().unwrap();
    assert_eq!(
        time_form_values(&time),
        [Some("23:30".to_owned()), Some("03:30".to_owned())]
    );
    assert_eq!(time.zone(), Some(template.zone()));
}

#[test]
fn edit_preselects_the_events_schedule() {
    let draft = edit_draft();
    let days_view = draft.panel(PANEL, NOW);
    assert_eq!(draft.first_day(), Some(date(2026, 10, 28)));
    assert_eq!(draft.last_day(), Some(date(2026, 11, 20)));
    assert_eq!(defaults(&days_view, PanelPart::Days), ["wed"]);
    assert!(
        days_view.content.contains("Times in America/Denver"),
        "{}",
        days_view.content
    );
    let time = draft.next().unwrap();
    assert_eq!(
        time_form_values(&time),
        [Some("17:00".to_owned()), Some("20:00".to_owned())]
    );
}

#[test]
fn an_unchanged_edit_is_refused_at_save() {
    let pings = edit_draft().next().unwrap().next().unwrap();
    assert_eq!(pings.step(), Step::Pings);
    assert_eq!(pings.ready(NOW, &[]), Err(Refusal::NothingToChange));
}
