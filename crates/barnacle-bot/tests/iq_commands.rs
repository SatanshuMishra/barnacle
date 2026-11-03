use barnacle_bot::discord::command_list;
use poise::serenity_prelude as serenity;

const ORGANISER_SUBCOMMANDS: [&str; 6] = ["start", "edit", "stop", "complete", "cleanup", "judges"];
const OPEN_SUBCOMMANDS: [&str; 2] = ["points", "leaderboard"];

struct Subcommand {
    name: String,
    required_permissions: serenity::Permissions,
    checks: usize,
    parameters: Vec<(String, bool)>,
}

fn iq_subcommands() -> Vec<Subcommand> {
    command_list(false)
        .into_iter()
        .find(|command| command.name == "iq")
        .expect("iq is registered")
        .subcommands
        .iter()
        .map(|command| Subcommand {
            name: command.name.to_string(),
            required_permissions: command.required_permissions,
            checks: command.checks.len(),
            parameters: command
                .parameters
                .iter()
                .map(|parameter| (parameter.name.to_string(), parameter.required))
                .collect(),
        })
        .collect()
}

fn subcommand(name: &str) -> Subcommand {
    iq_subcommands()
        .into_iter()
        .find(|subcommand| subcommand.name == name)
        .unwrap_or_else(|| panic!("iq {name} is registered"))
}

#[test]
fn the_iq_command_has_every_subcommand() {
    let names: Vec<String> = iq_subcommands()
        .into_iter()
        .map(|subcommand| subcommand.name)
        .collect();
    assert_eq!(
        names,
        [
            "start",
            "edit",
            "stop",
            "complete",
            "cleanup",
            "judges",
            "points",
            "leaderboard",
        ]
    );
}

#[test]
fn organiser_subcommands_carry_the_organiser_check_alone() {
    for name in ORGANISER_SUBCOMMANDS {
        let organiser = subcommand(name);
        assert_eq!(
            organiser.required_permissions,
            serenity::Permissions::empty(),
            "{name}"
        );
        assert_eq!(organiser.checks, 1, "{name}");
    }
    for name in OPEN_SUBCOMMANDS {
        let open = subcommand(name);
        assert_eq!(
            open.required_permissions,
            serenity::Permissions::empty(),
            "{name}"
        );
        assert_eq!(open.checks, 0, "{name}");
    }
    let points = subcommand("points").parameters;
    let names: Vec<&str> = points.iter().map(|(name, _)| name.as_str()).collect();
    assert_eq!(names, ["member", "points", "reason"]);
    assert_eq!(
        points.iter().find(|(name, _)| name == "reason"),
        Some(&("reason".to_owned(), true))
    );
}

#[test]
fn only_the_iq_command_itself_carries_discord_visibility() {
    let iq = command_list(false)
        .into_iter()
        .find(|command| command.name == "iq")
        .expect("iq is registered");
    assert_eq!(
        iq.default_member_permissions,
        serenity::Permissions::empty()
    );
    assert!(iq.checks.is_empty());
    for subcommand in &iq.subcommands {
        assert_eq!(
            subcommand.default_member_permissions,
            serenity::Permissions::empty(),
            "{}",
            subcommand.name
        );
    }
}
