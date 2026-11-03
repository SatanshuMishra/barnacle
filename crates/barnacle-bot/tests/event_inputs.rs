use barnacle_bot::attendance_store::Event;
use barnacle_bot::event_draft::CreateOptions;
use barnacle_bot::event_draft::Draft;
use barnacle_bot::event_draft::EditOptions;
use barnacle_bot::event_draft::PanelView;
use barnacle_bot::event_draft::Step;
use barnacle_bot::event_draft::Template;
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
const ORGANISER: UserId = UserId::new(100);
const PANEL: u64 = 7;
const NOW: i64 = 1_793_569_140;
const BUTTON: u64 = 2;

fn strings(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}

fn create(name: &str, template: Option<&str>, zone: Option<Zone>) -> Result<Draft, Refusal> {
    Draft::create(
        &CreateOptions {
            name: name.to_owned(),
            template: template.map(str::to_owned),
            zone,
        },
        SERVER,
        CHANNEL,
        ORGANISER,
        NOW,
    )
}

fn fresh() -> Draft {
    create("Scrim Nights", None, Zone::find("America/Edmonton")).unwrap()
}

fn at_time() -> Draft {
    let draft = fresh()
        .with_typed_dates("2026-11-02", "2026-11-30", NOW)
        .unwrap()
        .with_weekdays(&strings(&["mon"]))
        .unwrap()
        .next()
        .unwrap();
    assert_eq!(draft.step(), Step::Time);
    draft
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

fn custom_ids(view: &PanelView) -> Vec<String> {
    components(view)
        .iter()
        .filter_map(|component| component["custom_id"].as_str().map(str::to_owned))
        .collect()
}

fn component(view: &PanelView, part: PanelPart) -> serde_json::Value {
    let id = panel_id(PANEL, part);
    components(view)
        .into_iter()
        .find(|component| component["custom_id"] == id.as_str())
        .unwrap_or_else(|| panic!("the panel has a component {id}: {:?}", components(view)))
}

fn values(menu: &serde_json::Value) -> Vec<String> {
    menu["options"]
        .as_array()
        .unwrap_or_else(|| panic!("{menu} is a string menu"))
        .iter()
        .map(|option| option["value"].as_str().unwrap().to_owned())
        .collect()
}

fn assert_absent(view: &PanelView, removed: &[&str]) {
    let present = custom_ids(view);
    for id in removed {
        assert!(
            !present.iter().any(|found| found == id),
            "the panel still has {id}: {present:?}"
        );
    }
}

fn denver_wednesdays() -> Event {
    Event {
        id: 4,
        guild: SERVER,
        channel: CHANNEL,
        name: "Ranked Sprint".to_owned(),
        span: Span::new(
            date(2026, 11, 4),
            date(2026, 12, 30),
            Schedule::new(
                Weekdays::new(&[Weekday::Wednesday]).unwrap(),
                StartTime::new(19, 0).unwrap(),
                Zone::find("America/Denver").unwrap(),
                Hours::new(2).unwrap(),
            ),
        )
        .unwrap(),
        created_by: ORGANISER,
        created_at_ms: 1_793_000_000_000,
        ping_roles: Vec::new(),
        ended_at_ms: None,
        posts_up: false,
    }
}

fn edit(zone: Option<Zone>) -> Draft {
    Draft::edit(
        &denver_wednesdays(),
        &EditOptions {
            name: None,
            channel: None,
            zone,
        },
        ORGANISER,
        NOW,
    )
    .unwrap()
}

#[test]
fn the_days_step_sets_dates_with_one_button() {
    let view = fresh().panel(PANEL, NOW);
    assert_absent(
        &view,
        &[
            "barnacle-event:7:first-day",
            "barnacle-event:7:last-day",
            "barnacle-event:7:days",
        ],
    );
    let button = component(&view, PanelPart::TypeDates);
    assert_eq!(button["type"], BUTTON, "{button}");
    assert_eq!(button["label"], "Set dates", "{button}");
}

#[test]
fn a_three_month_season_shows_its_event_days() {
    let typed = fresh().with_typed_dates("2026-11-02", "2027-01-31", NOW);
    assert!(typed.is_ok(), "{typed:?}");
    let view = typed.unwrap().panel(PANEL, NOW);
    let menu = component(&view, PanelPart::Days);
    assert_eq!(menu["placeholder"], "Event days", "{menu}");
    assert_eq!(
        values(&menu),
        ["mon", "tue", "wed", "thu", "fri", "sat", "sun"]
    );
}

#[test]
fn event_days_offer_only_days_in_the_range() {
    let view = fresh()
        .with_typed_dates("2026-11-02", "2026-11-06", NOW)
        .unwrap()
        .panel(PANEL, NOW);
    let menu = component(&view, PanelPart::Days);
    assert_eq!(values(&menu), ["mon", "tue", "wed", "thu", "fri"]);
    assert_eq!(menu["max_values"], 5, "{menu}");
}

#[test]
fn a_picked_day_outside_new_dates_is_dropped() {
    let draft = fresh()
        .with_typed_dates("2026-11-02", "2026-11-08", NOW)
        .unwrap()
        .with_weekdays(&strings(&["mon", "sat"]))
        .unwrap()
        .with_typed_dates("2026-11-02", "2026-11-06", NOW)
        .unwrap();
    let weekdays = draft.weekdays();
    assert!(weekdays.is_some(), "{draft:?}");
    let weekdays = weekdays.unwrap();
    assert!(weekdays.contains(Weekday::Monday), "{:?}", weekdays.days());
    assert!(
        !weekdays.contains(Weekday::Saturday),
        "{:?}",
        weekdays.days()
    );
}

#[test]
fn the_time_step_has_one_set_time_button() {
    let view = at_time().panel(PANEL, NOW);
    assert_absent(
        &view,
        &[
            "barnacle-event:7:hour",
            "barnacle-event:7:minute",
            "barnacle-event:7:length",
            "barnacle-event:7:zone",
        ],
    );
    let button = component(&view, PanelPart::SetTime);
    assert_eq!(button["type"], BUTTON, "{button}");
    assert_eq!(button["label"], "Set time", "{button}");
}

#[test]
fn typed_start_and_end_set_the_session() {
    let timed = at_time().with_typed_time("19:00", "21:00");
    assert!(timed.is_ok(), "{timed:?}");
    let timed = timed.unwrap();
    assert_eq!(timed.start(), StartTime::new(19, 0));
    assert_eq!(timed.hours(), Hours::new(2));
}

#[test]
fn an_end_past_midnight_runs_into_the_next_day() {
    let late = at_time().with_typed_time("23:00", "01:00").unwrap();
    assert_eq!(late.hours(), Hours::new(2));
    let whole_day = at_time().with_typed_time("07:15", "07:15").unwrap();
    assert_eq!(whole_day.start(), StartTime::new(7, 15));
    assert_eq!(whole_day.hours(), Hours::new(24));
}

#[test]
fn spaces_around_a_time_are_ignored() {
    let timed = at_time().with_typed_time(" 19:00 ", "21:00 ");
    assert!(timed.is_ok(), "{timed:?}");
    let timed = timed.unwrap();
    assert_eq!(timed.start(), StartTime::new(19, 0));
    assert_eq!(timed.hours(), Hours::new(2));
}

#[test]
fn a_part_hour_length_is_refused() {
    assert_eq!(
        at_time().with_typed_time("19:00", "21:30").err(),
        Some(Refusal::LengthNotWholeHours)
    );
}

#[test]
fn an_unreadable_time_is_refused() {
    assert_eq!(
        at_time().with_typed_time("7pm", "21:00").err(),
        Some(Refusal::TimeFormat)
    );
    assert_eq!(
        at_time().with_typed_time("24:00", "01:00").err(),
        Some(Refusal::TimeFormat)
    );
}

#[test]
fn create_uses_the_given_zone() {
    let created = create("Scrim Nights", None, Zone::find("Europe/Berlin"));
    assert!(created.is_ok(), "{created:?}");
    assert_eq!(created.unwrap().zone().unwrap().name(), "Europe/Berlin");
}

#[test]
fn the_clan_battles_template_keeps_its_zone() {
    let created = create(
        "Clan Battles",
        Some("Clan Battles"),
        Zone::find("Europe/Berlin"),
    );
    assert!(created.is_ok(), "{created:?}");
    let template = Template::ClanBattles.schedule();
    assert_eq!(created.unwrap().zone(), Some(template.zone()));
}

#[test]
fn create_without_any_zone_is_refused() {
    assert_eq!(
        create("Scrim Nights", None, None).err(),
        Some(Refusal::ZoneNeeded)
    );
}

#[test]
fn the_panel_names_the_zone_in_use() {
    let draft = create("Scrim Nights", None, Zone::find("Europe/Berlin")).unwrap();
    let view = draft.panel(PANEL, NOW);
    assert!(
        view.content.contains("Times in Europe/Berlin"),
        "{}",
        view.content
    );
}

#[test]
fn edit_keeps_the_events_zone_unless_one_is_given() {
    let kept = edit(None);
    assert_eq!(kept.zone().map(Zone::name), Some("America/Denver"));
    let moved = edit(Zone::find("Europe/Berlin"));
    assert_eq!(moved.zone().map(Zone::name), Some("Europe/Berlin"));
}

#[test]
fn an_edit_that_changes_only_zone_or_time_can_be_saved() {
    assert_eq!(
        edit(None).ready(NOW, &[]).err(),
        Some(Refusal::NothingToChange)
    );
    let zone_changed = edit(Zone::find("Europe/Berlin")).ready(NOW, &[]);
    assert!(zone_changed.is_ok(), "{zone_changed:?}");
    let time_changed = edit(None)
        .with_typed_time("20:00", "22:00")
        .unwrap()
        .ready(NOW, &[]);
    assert!(time_changed.is_ok(), "{time_changed:?}");
}
