use barnacle_bot::event_draft::PanelView;
use barnacle_bot::ids::GuildId;
use barnacle_bot::iq_setup::SetupChecks;
use barnacle_bot::iq_setup::SetupDraft;
use barnacle_bot::iq_setup::SetupRefusal;
use barnacle_bot::iq_setup::SetupStep;
use barnacle_bot::schedule::Zone;
use barnacle_bot::wiring::IqPanelPart;
use barnacle_bot::wiring::iq_panel_id;
use barnacle_guess::UserId;
use jiff::civil::date;

const PANEL: u64 = 7;
const NOW: i64 = 1_793_640_000;
const BUTTON: u64 = 2;

fn opened() -> SetupDraft {
    SetupDraft::new(
        GuildId::new(1),
        UserId::new(100),
        vec![],
        Zone::find("Europe/Berlin").unwrap(),
        date(2026, 11, 2),
    )
}

fn panel(draft: &SetupDraft) -> PanelView {
    draft.panel(PANEL, NOW, &SetupChecks::default())
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

fn component(view: &PanelView, part: IqPanelPart) -> serde_json::Value {
    let id = iq_panel_id(PANEL, part);
    components(view)
        .into_iter()
        .find(|component| component["custom_id"] == id.as_str())
        .unwrap_or_else(|| panic!("the panel has a component {id}: {:?}", components(view)))
}

#[test]
fn the_basics_step_sets_dates_with_one_button() {
    let view = panel(&opened());
    let present: Vec<String> = components(&view)
        .iter()
        .filter_map(|component| component["custom_id"].as_str().map(str::to_owned))
        .collect();
    for removed in [
        "iq-panel:7:first-day",
        "iq-panel:7:last-day",
        "iq-panel:7:zone",
    ] {
        assert!(
            !present.iter().any(|id| id == removed),
            "the panel still has {removed}: {present:?}"
        );
    }
    let button = component(&view, IqPanelPart::SetDates);
    assert_eq!(button["type"], BUTTON, "{button}");
    assert_eq!(button["label"], "Set dates", "{button}");
}

#[test]
fn typed_dates_set_the_competition_days() {
    let draft = opened();
    let typed = draft.with_typed_dates("2026-11-02", "2027-01-31");
    assert!(typed.is_ok(), "{typed:?}");
    let typed = typed.unwrap();
    assert_eq!(typed.first_day(), Some(date(2026, 11, 2)));
    assert_eq!(typed.last_day(), Some(date(2027, 1, 31)));
    assert_eq!(
        draft.with_typed_dates("2026-11-05", "2026-11-05").err(),
        Some(SetupRefusal::LastDayNotAfterFirst)
    );
    assert_eq!(
        draft.with_typed_dates("2026-11-01", "2026-11-30").err(),
        Some(SetupRefusal::DayNotOffered)
    );
    assert_eq!(
        draft.with_typed_dates("tomorrow", "2026-11-30").err(),
        Some(SetupRefusal::DateFormat)
    );
    assert_eq!(
        draft.with_typed_dates("2026-11-02", "2027-12-01").err(),
        Some(SetupRefusal::DatesTooFar)
    );
}

#[test]
fn basics_needs_no_template() {
    let draft = opened();
    let menu = component(&panel(&draft), IqPanelPart::Template);
    assert_eq!(
        menu["placeholder"], "Start blank or pick a template",
        "{menu}"
    );
    let picked: Vec<&serde_json::Value> = menu["options"]
        .as_array()
        .unwrap_or_else(|| panic!("{menu} is a string menu"))
        .iter()
        .filter(|option| option["default"] == true)
        .collect();
    assert!(picked.is_empty(), "{picked:?}");
    let moved = draft
        .with_typed_dates("2026-11-02", "2026-11-30")
        .unwrap()
        .next();
    assert!(moved.is_ok(), "{moved:?}");
    let moved = moved.unwrap();
    assert_eq!(moved.step(), SetupStep::Criteria);
    assert_eq!(moved.template(), None);
}

#[test]
fn the_competition_uses_the_given_zone() {
    let draft = opened();
    assert_eq!(draft.zone(), "Europe/Berlin");
    let view = panel(&draft);
    assert!(
        view.content
            .contains("Ends 23:59 on the last day, Europe/Berlin time"),
        "{}",
        view.content
    );
}
