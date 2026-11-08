mod common;

use std::path::Path;

use barnacle_bot::event_draft::PanelView;
use barnacle_bot::ids::ChannelId;
use barnacle_bot::ids::GuildId;
use barnacle_bot::ids::RoleId;
use barnacle_bot::iq::Criterion;
use barnacle_bot::iq::CriterionKind;
use barnacle_bot::iq::JudgingMarks;
use barnacle_bot::iq_setup::SetupChecks;
use barnacle_bot::iq_setup::SetupDraft;
use barnacle_bot::iq_setup::SetupRefusal;
use barnacle_bot::iq_setup::SetupStep;
use barnacle_bot::iq_store::CreateOutcome;
use barnacle_bot::iq_store::IqStore;
use barnacle_bot::iq_store::NewCompetition;
use barnacle_bot::iq_text;
use barnacle_bot::schedule::Zone;
use barnacle_bot::wiring::IqPanelPart;
use barnacle_bot::wiring::iq_panel_id;
use barnacle_guess::UserId;
use common::attendance_pool;
use jiff::civil::date;
use sqlx::sqlite::SqlitePool;

const CLI_DIRECTIVE: &str = ".bail on\n";
const MIGRATIONS: [&str; 3] = [
    "0009_iq_competition.sql",
    "0012_iq_marks_and_judge_log.sql",
    "0013_iq_players_and_rejections.sql",
];
const PANEL: u64 = 7;
const NOW: i64 = 1_793_640_000;

async fn apply(pool: &SqlitePool, file: &str) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../migrations")
        .join(file);
    let script = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("{} is readable: {error}", path.display()));
    sqlx::raw_sql(sqlx::AssertSqlSafe(
        script.trim_start_matches(CLI_DIRECTIVE).to_owned(),
    ))
    .execute(pool)
    .await
    .unwrap();
}

async fn store() -> IqStore {
    let pool = attendance_pool().await;
    for file in MIGRATIONS {
        apply(&pool, file).await;
    }
    IqStore::with_pool(pool).await.unwrap()
}

fn fresh_draft() -> SetupDraft {
    SetupDraft::new(
        GuildId::new(1),
        UserId::new(100),
        vec![],
        Zone::find("Europe/Berlin").unwrap(),
        date(2026, 11, 2),
    )
}

fn at_criteria() -> SetupDraft {
    let draft = fresh_draft()
        .with_typed_dates("2026-11-02", "2026-11-30")
        .unwrap()
        .renamed("November IQ")
        .unwrap()
        .next()
        .unwrap();
    assert_eq!(draft.step(), SetupStep::Criteria);
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

fn criterion(name: &str, points: u32, kind: CriterionKind) -> Criterion {
    Criterion {
        name: name.to_owned(),
        description: String::new(),
        points,
        kind,
    }
}

fn announcement(criteria: &[Criterion]) -> String {
    iq_text::start_announcement(
        "November IQ",
        2000,
        &[],
        ChannelId::new(42),
        criteria,
        "",
        &[],
    )
}

#[test]
fn a_new_criterion_is_single_player_by_default() {
    let draft = fresh_draft().with_criterion("Solo", "", "5", None).unwrap();
    assert_eq!(draft.criteria()[0].kind, CriterionKind::Single);
}

#[test]
fn multi_applies_to_the_next_criterion_only() {
    let draft = fresh_draft()
        .with_kind("multi")
        .unwrap()
        .with_criterion("Lockout", "", "8", None)
        .unwrap()
        .with_criterion("Solo", "", "5", None)
        .unwrap();
    assert_eq!(draft.criteria()[0].name, "Lockout");
    assert_eq!(draft.criteria()[0].kind, CriterionKind::Multi);
    assert_eq!(draft.criteria()[1].name, "Solo");
    assert_eq!(draft.criteria()[1].kind, CriterionKind::Single);
}

#[test]
fn the_kind_menu_changes_the_picked_criterion() {
    let draft = fresh_draft().with_criterion("Solo", "", "5", None).unwrap();
    assert_eq!(draft.criteria()[0].kind, CriterionKind::Single);
    let picked = draft.with_picked_criterion(0).unwrap();
    let changed = picked.with_kind("multi").unwrap();
    assert_eq!(changed.criteria()[0].kind, CriterionKind::Multi);
    assert_eq!(
        picked.with_kind("team").err(),
        Some(SetupRefusal::CriterionKindUnknown)
    );
}

#[test]
fn the_criteria_step_offers_the_kind_menu() {
    let view = at_criteria().panel(PANEL, NOW, &SetupChecks::default());
    let id = iq_panel_id(PANEL, IqPanelPart::CriterionKind);
    let found = components(&view);
    let menu = found
        .iter()
        .find(|component| component["custom_id"] == id.as_str())
        .unwrap_or_else(|| panic!("the panel has a component {id}: {found:?}"));
    let options = menu["options"].as_array().cloned().unwrap_or_default();
    let values: Vec<&str> = options
        .iter()
        .filter_map(|option| option["value"].as_str())
        .collect();
    assert_eq!(values, ["single", "multi"], "{menu}");
    let defaults: Vec<&str> = options
        .iter()
        .filter(|option| option["default"] == true)
        .filter_map(|option| option["value"].as_str())
        .collect();
    assert_eq!(defaults, ["single"], "{menu}");
}

#[tokio::test]
async fn a_stored_criterion_keeps_its_kind() {
    let store = store().await;
    let lockout = criterion("Leaderboard Lockout", 8, CriterionKind::Multi);
    let new = NewCompetition {
        guild: GuildId::new(5),
        name: "November IQ".to_owned(),
        first_day: date(2026, 11, 1),
        last_day: date(2026, 11, 30),
        zone: "America/Edmonton".to_owned(),
        category: ChannelId::new(40),
        announcements: ChannelId::new(41),
        submissions: ChannelId::new(42),
        criteria: vec![lockout.clone()],
        rules: String::new(),
        roles: vec![RoleId::new(31)],
        judges: vec![UserId::new(21)],
        marks: JudgingMarks::default(),
        judge_log: None,
        created_by: UserId::new(100),
        created_at_ms: 1_793_500_000_000,
    };
    let id = match store.create(&new).await.unwrap() {
        CreateOutcome::Created(competition) => competition.id,
        CreateOutcome::AlreadyLive => panic!("the competition was refused"),
    };
    let stored = store.competition(id).await.unwrap().unwrap();
    assert_eq!(stored.criteria, vec![lockout]);
    assert_eq!(stored.criteria[0].kind, CriterionKind::Multi);
}

#[test]
fn the_announcement_explains_multi_player_posts() {
    let multi = announcement(&[criterion("Leaderboard Lockout", 8, CriterionKind::Multi)]);
    let lines: Vec<&str> = multi.lines().collect();
    assert!(
        lines.contains(&"> **Leaderboard Lockout** \u{b7} 8 points \u{b7} 2 or 3 players"),
        "{multi}"
    );
    assert!(
        multi.contains("For a multi-player criterion, put the name and then ping the 1 or 2 players you played with, such as Leaderboard Lockout @Player."),
        "{multi}"
    );
    let single = announcement(&[criterion("Solo", 5, CriterionKind::Single)]);
    assert!(!single.contains("multi-player"), "{single}");
}
