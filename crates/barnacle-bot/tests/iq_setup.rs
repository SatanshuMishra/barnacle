use barnacle_bot::event_draft::PanelView;
use barnacle_bot::ids::ChannelId;
use barnacle_bot::ids::GuildId;
use barnacle_bot::ids::RoleId;
use barnacle_bot::iq::Criterion;
use barnacle_bot::iq::JudgingMarks;
use barnacle_bot::iq::Mark;
use barnacle_bot::iq::Status;
use barnacle_bot::iq_setup::CompetitionChannels;
use barnacle_bot::iq_setup::GuildRole;
use barnacle_bot::iq_setup::MarkField;
use barnacle_bot::iq_setup::SetupChecks;
use barnacle_bot::iq_setup::SetupDraft;
use barnacle_bot::iq_setup::SetupReady;
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
        .renamed("November IQ")
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

fn with_october_rules(draft: SetupDraft) -> SetupDraft {
    OCTOBER_RULES
        .lines()
        .fold(draft, |held, rule| held.with_rule(rule, None).unwrap())
}

fn at_rules() -> SetupDraft {
    let rules = at_criteria()
        .with_criterion("Drunken Kraken", "Five kills in one battle.", "5", None)
        .unwrap()
        .next()
        .unwrap();
    assert_eq!(rules.step(), SetupStep::Rules);
    rules
}

fn at_review() -> SetupDraft {
    let rules = with_october_rules(at_rules());
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
    let rules = with_october_rules(rules);
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
    let rules = (0..25).fold(longest.next().unwrap(), |held, number| {
        held.with_rule(&format!("{number:02}{}", "r".repeat(148)), None)
            .unwrap()
    });
    assert_eq!(rules.step(), SetupStep::Rules);
    assert_eq!(rules.rules().len(), 25);
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
    assert_eq!(
        picked.rules().to_vec(),
        ["Post your own battles only.", "One post per battle."]
    );
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

#[test]
fn the_review_lists_the_three_marks() {
    let review = at_review()
        .with_mark(MarkField::Awarded, Mark::parse("<:kraken:42>").unwrap())
        .unwrap();
    let view = review.panel(PANEL, NOW, &passing());
    assert!(
        view.content
            .contains("Denied: \u{274C}. Awarded: <:kraken:42>. Confirm awarded: \u{2705}."),
        "{}",
        view.content
    );
    assert!(
        view.content
            .contains("> #iq-judge-log \u{b7} judges only, a record of judge actions"),
        "{}",
        view.content
    );
}

#[test]
fn a_custom_mark_shows_on_its_button_by_id() {
    let who = at_review()
        .back()
        .with_mark(MarkField::Denied, Mark::parse("<a:spin:77>").unwrap())
        .unwrap();
    assert_eq!(who.step(), SetupStep::Who);
    let button = component(&who.panel(PANEL, NOW, &passing()), IqPanelPart::DeniedMark);
    assert_eq!(button["label"], "Denied", "{button}");
    assert_eq!(button["emoji"]["id"], "77", "{button}");
    assert_eq!(button["emoji"]["name"], "spin", "{button}");
    assert_eq!(button["emoji"]["animated"], true, "{button}");
}

#[test]
fn a_started_competition_stores_its_judge_log() {
    let Ok(SetupReady::Start(plan)) = at_review().ready() else {
        panic!("the review step is ready to start");
    };
    let competition = plan.competition(
        CompetitionChannels {
            category: ChannelId::new(40),
            announcements: ChannelId::new(41),
            submissions: ChannelId::new(42),
            judge_log: ChannelId::new(43),
        },
        1_793_569_140_000,
    );
    assert_eq!(competition.judge_log, Some(ChannelId::new(43)));
}

#[test]
fn rules_are_added_edited_and_removed_one_at_a_time() {
    let rules = with_october_rules(at_rules());
    assert_eq!(rules.rules().len(), 2);
    let edited = rules
        .with_picked_rule(1)
        .unwrap()
        .with_rule("  One post   per battle, please. ", Some(1))
        .unwrap();
    assert_eq!(
        edited.rules().to_vec(),
        [
            "Post your own battles only.",
            "One post per battle, please."
        ]
    );
    assert_eq!(edited.picked_rule(), None);
    let removed = edited.with_picked_rule(0).unwrap().remove_picked().unwrap();
    assert_eq!(removed.rules().to_vec(), ["One post per battle, please."]);
    assert_eq!(removed.criteria().len(), 1);
    assert_eq!(
        removed.with_picked_rule(1).err(),
        Some(SetupRefusal::RuleUnknown)
    );
}

#[test]
fn a_rule_is_refused_when_blank_too_long_or_one_too_many() {
    let rules = at_rules();
    assert_eq!(
        rules.with_rule("   ", None).err(),
        Some(SetupRefusal::RuleLength)
    );
    assert_eq!(
        rules.with_rule(&"r".repeat(151), None).err(),
        Some(SetupRefusal::RuleLength)
    );
    let full = (0..25).fold(rules, |held, number| {
        held.with_rule(&format!("Rule {number}"), None).unwrap()
    });
    assert_eq!(
        full.with_rule("One more", None).err(),
        Some(SetupRefusal::TooManyRules)
    );
}

#[test]
fn the_rules_step_numbers_each_rule_and_offers_it_in_the_menu() {
    let view = with_october_rules(at_rules()).panel(PANEL, NOW, &passing());
    assert!(
        view.content
            .contains("**1.** Post your own battles only.\n**2.** One post per battle."),
        "{}",
        view.content
    );
    let labels: Vec<serde_json::Value> = component(&view, IqPanelPart::Rule)["options"]
        .as_array()
        .unwrap()
        .iter()
        .map(|option| option["label"].clone())
        .collect();
    assert_eq!(
        labels,
        ["1. Post your own battles only.", "2. One post per battle."]
    );
}

#[test]
fn a_started_competition_stores_its_rules_one_per_line() {
    let Ok(SetupReady::Start(plan)) = at_review().ready() else {
        panic!("the review is ready");
    };
    assert_eq!(plan.rules, OCTOBER_RULES);
}

#[test]
fn a_numbered_template_loses_its_typed_numbers() {
    let numbered = Competition {
        rules: "1. Post your own battles only.\n2) One post per battle.".to_owned(),
        ..october()
    };
    let draft = opened(vec![numbered]);
    let value = template_value(&draft, "October IQ");
    let picked = draft.with_template(&value).unwrap();
    assert_eq!(
        picked.rules().to_vec(),
        ["Post your own battles only.", "One post per battle."]
    );
}

fn guild_role(id: u64, name: &str, position: u16, managed: bool) -> GuildRole {
    GuildRole {
        id: RoleId::new(id),
        name: name.to_owned(),
        position,
        managed,
    }
}

fn server_roles() -> Vec<GuildRole> {
    vec![
        guild_role(GUILD.get(), "@everyone", 0, false),
        guild_role(31, "Periscope", 2, false),
        guild_role(60, "IQ-Judge", 5, false),
        guild_role(61, "Barnacle", 9, true),
        guild_role(62, "Shrimpy", 3, false),
    ]
}

#[test]
fn the_roles_menu_leaves_out_everyone_bot_roles_and_the_judge_role() {
    let draft = opened(vec![]).with_role_list(&server_roles(), Some(RoleId::new(60)));
    let names: Vec<String> = draft
        .role_options()
        .into_iter()
        .map(|(_, name)| name)
        .collect();
    assert_eq!(names, ["Shrimpy", "Periscope"]);
}

#[test]
fn a_template_naming_the_judge_role_loses_it() {
    let october_with_judges = Competition {
        roles: vec![TARGET_ROLE, RoleId::new(60)],
        ..october()
    };
    let draft =
        opened(vec![october_with_judges]).with_role_list(&server_roles(), Some(RoleId::new(60)));
    let value = template_value(&draft, "October IQ");
    let picked = draft.with_template(&value).unwrap();
    assert_eq!(picked.roles().to_vec(), vec![TARGET_ROLE]);
    assert_eq!(
        picked
            .with_roles(vec![RoleId::new(62), RoleId::new(60)])
            .unwrap()
            .roles()
            .to_vec(),
        vec![RoleId::new(62)]
    );
}

#[test]
fn the_roles_menu_keeps_picked_roles_when_the_server_has_more_than_it_shows() {
    let many: Vec<GuildRole> = (0..40)
        .map(|number| {
            guild_role(
                100 + number,
                &format!("Role {number}"),
                200 - number as u16,
                false,
            )
        })
        .collect();
    let draft = opened(vec![])
        .with_role_list(&many, None)
        .with_roles(vec![RoleId::new(139), RoleId::new(4242)])
        .unwrap();
    let options = draft.role_options();
    assert_eq!(options.len(), 25);
    assert!(options.contains(&(RoleId::new(139), "Role 39".to_owned())));
    assert!(options.contains(&(RoleId::new(4242), "Deleted role".to_owned())));
    assert_eq!(options[0], (RoleId::new(100), "Role 0".to_owned()));
}
