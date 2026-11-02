use barnacle_bot::event_draft::PanelView;
use barnacle_bot::ids::ChannelId;
use barnacle_bot::ids::GuildId;
use barnacle_bot::ids::RoleId;
use barnacle_bot::iq::Criterion;
use barnacle_bot::iq::JudgingMarks;
use barnacle_bot::iq::Mark;
use barnacle_bot::iq::Status;
use barnacle_bot::iq_setup::SetupChecks;
use barnacle_bot::iq_setup::SetupDraft;
use barnacle_bot::iq_setup::SetupRefusal;
use barnacle_bot::iq_setup::SetupStep;
use barnacle_bot::iq_store::Competition;
use barnacle_bot::schedule::Zone;
use barnacle_bot::wiring::IqPanelPart;
use barnacle_bot::wiring::iq_panel_id;
use barnacle_guess::UserId;
use jiff::civil::Date;
use jiff::civil::date;

const GUILD: GuildId = GuildId::new(5);
const ADMIN: UserId = UserId::new(100);
const PANEL: u64 = 7;
const NOW: i64 = 1_793_569_140;
const TARGET_ROLE: RoleId = RoleId::new(31);
const JUDGE: UserId = UserId::new(21);
const SECOND_JUDGE: UserId = UserId::new(22);
const START_BLANK: &str = "Start blank";
const OCTOBER_RULES: &str = "Post your own battles only.\nOne post per battle.";
const ROLE_BELOW_JUDGE: &str = "I can't hand out @IQ-Judge: my role sits below it. Drag my role above @IQ-Judge in Server Settings, then press Start again.";
const MISSING_PERMISSIONS: &str =
    "I need Manage Channels and Manage Roles in this server to set up the competition.";
const NO_TEMPLATES: &str =
    "No past competitions yet, so there are no templates. The last 10 are kept once you have some.";

fn today() -> Date {
    date(2026, 11, 1)
}

fn passing() -> SetupChecks {
    SetupChecks {
        role_below_judge: false,
        missing: vec![],
    }
}

fn criterion(name: &str, description: &str, points: u32) -> Criterion {
    Criterion {
        name: name.to_owned(),
        description: description.to_owned(),
        points,
    }
}

fn october() -> Competition {
    Competition {
        id: 4,
        status: Status::Archived,
        last_seen: Some(1_456_074_443_980_800_000),
        guild: GUILD,
        name: "October IQ".to_owned(),
        first_day: date(2026, 10, 1),
        last_day: date(2026, 10, 31),
        zone: "America/Edmonton".to_owned(),
        category: ChannelId::new(40),
        announcements: ChannelId::new(41),
        submissions: ChannelId::new(42),
        criteria: vec![
            criterion("Drunken Kraken", "Five kills in one battle.", 5),
            criterion("Double Down", "Two kills with one salvo.", 2),
        ],
        rules: OCTOBER_RULES.to_owned(),
        roles: vec![TARGET_ROLE],
        judges: vec![JUDGE, SECOND_JUDGE],
        marks: JudgingMarks::new(
            Mark::parse("\u{274C}").unwrap(),
            Mark::parse("\u{2B50}").unwrap(),
            Mark::parse("\u{2705}").unwrap(),
        )
        .unwrap(),
        judge_log: None,
        created_by: ADMIN,
        created_at_ms: 1_790_000_000_000,
    }
}

fn opened(templates: Vec<Competition>) -> SetupDraft {
    SetupDraft::new(
        GUILD,
        ADMIN,
        templates,
        Zone::find("America/Edmonton").unwrap(),
        today(),
    )
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

fn disabled(view: &PanelView, part: IqPanelPart) -> bool {
    component(view, part)["disabled"].as_bool().unwrap_or(false)
}

fn next_disabled(draft: &SetupDraft) -> bool {
    disabled(&draft.panel(PANEL, NOW, &passing()), IqPanelPart::Next)
}

fn template_options(draft: &SetupDraft) -> Vec<serde_json::Value> {
    component(&draft.panel(PANEL, NOW, &passing()), IqPanelPart::Template)["options"]
        .as_array()
        .cloned()
        .expect("the template menu is a string menu")
}

fn template_value(draft: &SetupDraft, label: &str) -> String {
    template_options(draft)
        .iter()
        .find(|option| option["label"] == label)
        .and_then(|option| option["value"].as_str())
        .unwrap_or_else(|| panic!("the template menu offers {label}"))
        .to_owned()
}

fn basics_done(draft: SetupDraft) -> SetupDraft {
    let blank = template_value(&draft, START_BLANK);
    draft
        .with_template(&blank)
        .unwrap()
        .with_typed_dates("2026-11-02", "2026-11-26")
        .unwrap()
}

fn at_criteria() -> SetupDraft {
    let draft = basics_done(opened(vec![])).next().unwrap();
    assert_eq!(draft.step(), SetupStep::Criteria);
    draft
}

fn with_criteria(draft: SetupDraft, count: usize) -> SetupDraft {
    (0..count).fold(draft, |held, number| {
        held.with_criterion(&format!("Criterion {number}"), "", "1", None)
            .unwrap()
    })
}

fn at_review() -> SetupDraft {
    let rules = at_criteria()
        .with_criterion("Drunken Kraken", "Five kills in one battle.", "5", None)
        .unwrap()
        .next()
        .unwrap()
        .with_rules(OCTOBER_RULES)
        .unwrap();
    let review = rules
        .next()
        .unwrap()
        .with_roles(vec![TARGET_ROLE])
        .unwrap()
        .with_judges(vec![JUDGE])
        .unwrap()
        .next()
        .unwrap();
    assert_eq!(review.step(), SetupStep::Review);
    review
}

#[test]
fn a_criterion_name_taken_in_other_capitals_is_refused() {
    let draft = at_criteria()
        .with_criterion("Drunken Kraken", "Five kills in one battle.", "5", None)
        .unwrap();
    assert_eq!(
        draft.with_criterion("drunken kraken", "", "3", None).err(),
        Some(SetupRefusal::CriterionNameTaken(
            "drunken kraken".to_string()
        ))
    );
}

#[test]
fn points_must_be_a_whole_number_from_1_to_1000() {
    let draft = at_criteria();
    for points in ["0", "1001", "2.5", "abc"] {
        assert_eq!(
            draft.with_criterion("Torpedo Soup", "", points, None).err(),
            Some(SetupRefusal::CriterionPoints),
            "{points}"
        );
    }
    assert!(
        draft
            .with_criterion("Torpedo Soup", "", "1000", None)
            .is_ok()
    );
}

#[test]
fn a_twenty_sixth_criterion_is_refused() {
    let draft = with_criteria(at_criteria(), 25);
    assert_eq!(
        draft.with_criterion("Extra", "", "1", None).err(),
        Some(SetupRefusal::TooManyCriteria)
    );
}

#[test]
fn each_step_waits_for_what_it_needs() {
    let fresh = opened(vec![]);
    assert_eq!(fresh.step(), SetupStep::Basics);
    assert!(next_disabled(&fresh));
    let basics = basics_done(fresh);
    assert!(!next_disabled(&basics));

    let criteria = basics.next().unwrap();
    assert_eq!(criteria.step(), SetupStep::Criteria);
    assert!(next_disabled(&criteria));
    let criteria = criteria
        .with_criterion("Drunken Kraken", "Five kills in one battle.", "5", None)
        .unwrap();
    assert!(!next_disabled(&criteria));

    let rules = criteria.next().unwrap();
    assert_eq!(rules.step(), SetupStep::Rules);
    assert!(next_disabled(&rules));
    let rules = rules.with_rules(OCTOBER_RULES).unwrap();
    assert!(!next_disabled(&rules));

    let who = rules.next().unwrap();
    assert_eq!(who.step(), SetupStep::Who);
    assert!(next_disabled(&who));
    let who = who
        .with_roles(vec![TARGET_ROLE])
        .unwrap()
        .with_judges(vec![JUDGE])
        .unwrap();
    assert!(!next_disabled(&who));
}

#[test]
fn the_panel_fits_discord_with_the_most_criteria_and_rules() {
    let longest = (0..25).fold(at_criteria(), |held, number| {
        held.with_criterion(
            &format!("{number:02}{}", "N".repeat(48)),
            &"d".repeat(300),
            "1000",
            None,
        )
        .unwrap()
    });
    let criteria_view = longest.panel(PANEL, NOW, &passing());
    assert!(
        criteria_view.content.chars().count() <= 2000,
        "{}",
        criteria_view.content.chars().count()
    );
    let rules_text: String = (0..50)
        .map(|line| format!("Rule {line:03}: {}", "r".repeat(88)))
        .collect::<Vec<String>>()
        .join("\n")
        .chars()
        .take(4000)
        .collect();
    assert_eq!(rules_text.chars().count(), 4000);
    let rules = longest.next().unwrap().with_rules(&rules_text).unwrap();
    assert_eq!(rules.step(), SetupStep::Rules);
    let rules_view = rules.panel(PANEL, NOW, &passing());
    assert!(
        rules_view.content.chars().count() <= 2000,
        "{}",
        rules_view.content.chars().count()
    );
}

#[test]
fn start_is_disabled_when_the_bot_cannot_hand_out_the_judge_role() {
    let view = at_review().panel(
        PANEL,
        NOW,
        &SetupChecks {
            role_below_judge: true,
            missing: vec![],
        },
    );
    assert!(disabled(&view, IqPanelPart::Start));
    assert!(view.content.contains(ROLE_BELOW_JUDGE), "{}", view.content);
}

#[test]
fn start_is_disabled_without_manage_channels_and_roles() {
    let view = at_review().panel(
        PANEL,
        NOW,
        &SetupChecks {
            role_below_judge: false,
            missing: vec!["Manage Channels"],
        },
    );
    assert!(disabled(&view, IqPanelPart::Start));
    assert!(
        view.content.contains(MISSING_PERMISSIONS),
        "{}",
        view.content
    );
}

#[test]
fn picking_a_template_copies_its_setup() {
    let draft = opened(vec![october()]);
    let value = template_value(&draft, "October IQ");
    let picked = draft.with_template(&value).unwrap();
    assert_eq!(picked.criteria().to_vec(), october().criteria);
    assert_eq!(picked.rules().to_owned(), OCTOBER_RULES.to_owned());
    assert_eq!(picked.roles().to_vec(), vec![TARGET_ROLE]);
    assert_eq!(picked.judges().to_vec(), vec![JUDGE, SECOND_JUDGE]);
    assert_eq!(picked.marks().awarded, Mark::parse("\u{2B50}").unwrap());
}

#[test]
fn the_first_competition_has_no_templates_to_offer() {
    let draft = opened(vec![]);
    let labels: Vec<String> = template_options(&draft)
        .iter()
        .map(|option| option["label"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(labels, [START_BLANK]);
    let view = draft.panel(PANEL, NOW, &passing());
    assert!(view.content.contains(NO_TEMPLATES), "{}", view.content);
}

#[test]
fn the_dates_form_asks_for_the_first_and_last_day() {
    let modal = serde_json::to_value(barnacle_bot::iq_setup::dates_modal(PANEL)).unwrap();
    assert_eq!(modal["custom_id"], "iq-panel:7:dates-modal", "{modal}");
    let inputs: Vec<(String, String)> = modal["components"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|row| row["components"].as_array().cloned().unwrap_or_default())
        .map(|input| {
            (
                input["custom_id"].as_str().unwrap_or_default().to_owned(),
                input["label"].as_str().unwrap_or_default().to_owned(),
            )
        })
        .collect();
    assert_eq!(
        inputs,
        [
            ("first".to_owned(), "First day".to_owned()),
            ("last".to_owned(), "Last day".to_owned()),
        ]
    );
}
