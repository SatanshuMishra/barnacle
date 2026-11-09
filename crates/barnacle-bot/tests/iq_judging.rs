use barnacle_bot::discord;
use barnacle_bot::event_draft::PanelView;
use barnacle_bot::ids::GuildId;
use barnacle_bot::ids::RoleId;
use barnacle_bot::iq::Mark;
use barnacle_bot::iq_setup;
use barnacle_bot::iq_setup::MarkField;
use barnacle_bot::iq_setup::SetupChecks;
use barnacle_bot::iq_setup::SetupDraft;
use barnacle_bot::iq_setup::SetupRefusal;
use barnacle_bot::iq_setup::SetupStep;
use barnacle_bot::iq_text;
use barnacle_bot::schedule::Zone;
use barnacle_bot::voice::Overwrite;
use barnacle_bot::voice::OverwriteTarget;
use barnacle_bot::wiring::IqPanelPart;
use barnacle_bot::wiring::iq_panel_id;
use barnacle_guess::UserId;
use jiff::civil::date;
use poise::serenity_prelude::Permissions;

const PANEL: u64 = 7;
const NOW: i64 = 1_793_640_000;
const BUTTON: u64 = 2;

fn at_who() -> SetupDraft {
    let draft = SetupDraft::new(
        GuildId::new(1),
        UserId::new(100),
        vec![],
        Zone::find("Europe/Berlin").unwrap(),
        date(2026, 11, 2),
    )
    .with_typed_dates("2026-11-02", "2026-11-30")
    .unwrap()
    .renamed("November IQ")
    .unwrap()
    .next()
    .unwrap()
    .with_criterion("Drunken Kraken", "Five kills in one battle.", "5", None)
    .unwrap()
    .next()
    .unwrap()
    .with_rule("Post your own battles only.", None)
    .unwrap()
    .next()
    .unwrap()
    .with_roles(vec![RoleId::new(31)])
    .unwrap()
    .with_judges(vec![UserId::new(21)])
    .unwrap();
    assert_eq!(draft.step(), SetupStep::Who);
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

fn component(view: &PanelView, part: IqPanelPart) -> serde_json::Value {
    let id = iq_panel_id(PANEL, part);
    components(view)
        .into_iter()
        .find(|component| component["custom_id"] == id.as_str())
        .unwrap_or_else(|| panic!("the panel has a component {id}: {:?}", components(view)))
}

fn overwrite_for(overwrites: &[Overwrite], target: OverwriteTarget) -> Overwrite {
    *overwrites
        .iter()
        .find(|overwrite| overwrite.target == target)
        .unwrap_or_else(|| panic!("an overwrite for {target:?} in {overwrites:?}"))
}

fn holds(bits: u64, permissions: Permissions) -> bool {
    bits & permissions.bits() == permissions.bits()
}

#[test]
fn the_points_command_requires_a_reason() {
    let commands = discord::command_list(false);
    let points = commands
        .iter()
        .find(|command| command.name == "iq")
        .and_then(|iq| {
            iq.subcommands
                .iter()
                .find(|command| command.name == "points")
        })
        .expect("iq points is registered");
    let reason = points
        .parameters
        .iter()
        .find(|parameter| parameter.name == "reason")
        .expect("iq points has a reason option");
    assert!(reason.required);
}

#[test]
fn the_who_step_shows_three_mark_buttons() {
    let view = at_who().panel(PANEL, NOW, &SetupChecks::default());
    for (part, label, emoji) in [
        (IqPanelPart::DeniedMark, "Denied", "\u{274C}"),
        (IqPanelPart::AwardedMark, "Awarded", "\u{1F3C6}"),
        (IqPanelPart::ConfirmMark, "Confirm awarded", "\u{2705}"),
    ] {
        let button = component(&view, part);
        assert_eq!(button["type"], BUTTON, "{button}");
        assert_eq!(button["label"], label, "{button}");
        assert_eq!(button["emoji"]["name"], emoji, "{button}");
    }
    let present: Vec<String> = components(&view)
        .iter()
        .filter_map(|component| component["custom_id"].as_str().map(str::to_owned))
        .collect();
    assert!(
        !present.iter().any(|id| id == "iq-panel:7:reaction"),
        "{present:?}"
    );
}

#[test]
fn a_mark_another_field_uses_is_refused() {
    let draft = at_who();
    assert_eq!(
        draft
            .with_mark(MarkField::Awarded, Mark::parse("\u{2705}").unwrap())
            .err(),
        Some(SetupRefusal::MarkRepeated)
    );
    let kraken = draft.with_mark(MarkField::Awarded, Mark::parse("<:kraken:42>").unwrap());
    assert!(kraken.is_ok(), "{kraken:?}");
    assert_eq!(
        kraken.unwrap().marks().awarded,
        Mark::parse("<:kraken:42>").unwrap()
    );
}

#[test]
fn the_award_line_names_judge_player_criterion_and_post() {
    assert_eq!(
        iq_text::judge_log_award(
            UserId::new(21),
            &[UserId::new(31)],
            "Drunken Kraken",
            5,
            "https://discord.com/channels/5/42/1000"
        ),
        "<@21> awarded <@31> 5 points for Drunken Kraken: https://discord.com/channels/5/42/1000"
    );
}

#[test]
fn a_points_line_carries_the_reason_and_total() {
    assert_eq!(
        iq_text::judge_log_points(
            UserId::new(21),
            UserId::new(31),
            -30,
            "Wrong screenshot",
            20
        ),
        "<@21> took 30 points from <@31>: Wrong screenshot. New total: 20."
    );
    assert_eq!(
        iq_text::judge_log_points(UserId::new(21), UserId::new(31), 30, "Bonus round", 80),
        "<@21> gave <@31> 30 points: Bonus round. New total: 80."
    );
}

#[test]
fn a_judges_line_names_who_changed() {
    assert_eq!(
        iq_text::judge_log_judges(
            UserId::new(100),
            &[UserId::new(21), UserId::new(23)],
            &[UserId::new(22)]
        ),
        "<@100> changed the judges: added <@21>, <@23>; removed <@22>."
    );
    assert_eq!(
        iq_text::judge_log_judges(UserId::new(100), &[UserId::new(21)], &[]),
        "<@100> changed the judges: added <@21>."
    );
}

#[test]
fn a_removal_past_zero_names_the_most_that_can_be_removed() {
    assert_eq!(
        iq_text::below_zero(UserId::new(31), 50),
        "<@31> has 50 points, so at most 50 can be removed."
    );
}

#[test]
fn the_judge_log_is_private_to_judges_and_read_only() {
    let overwrites =
        iq_setup::judge_log_overwrites(RoleId::new(1), RoleId::new(50), UserId::new(999));
    let everyone = overwrite_for(&overwrites, OverwriteTarget::Role(RoleId::new(1)));
    assert!(
        holds(everyone.deny, Permissions::VIEW_CHANNEL),
        "{everyone:?}"
    );
    let judges = overwrite_for(&overwrites, OverwriteTarget::Role(RoleId::new(50)));
    assert!(
        holds(
            judges.allow,
            Permissions::VIEW_CHANNEL | Permissions::READ_MESSAGE_HISTORY
        ),
        "{judges:?}"
    );
    assert!(
        holds(
            judges.deny,
            Permissions::SEND_MESSAGES | Permissions::ADD_REACTIONS
        ),
        "{judges:?}"
    );
    let bot = overwrite_for(&overwrites, OverwriteTarget::Member(UserId::new(999)));
    assert!(
        holds(
            bot.allow,
            Permissions::VIEW_CHANNEL
                | Permissions::SEND_MESSAGES
                | Permissions::READ_MESSAGE_HISTORY
        ),
        "{bot:?}"
    );
    assert_eq!(iq_setup::JUDGE_LOG_NAME, "iq-judge-log");
}
