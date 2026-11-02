use barnacle_bot::discord::command_list;
use poise::serenity_prelude as serenity;

const ADMIN_SUBCOMMANDS: [&str; 6] = ["start", "edit", "stop", "complete", "cleanup", "judges"];
const OPEN_SUBCOMMANDS: [&str; 2] = ["points", "leaderboard"];

struct Subcommand {
    name: String,
    required_permissions: serenity::Permissions,
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
fn admin_subcommands_need_manage_server() {
    for name in ADMIN_SUBCOMMANDS {
        assert_eq!(
            subcommand(name).required_permissions,
            serenity::Permissions::MANAGE_GUILD,
            "{name}"
        );
    }
    for name in OPEN_SUBCOMMANDS {
        assert_eq!(
            subcommand(name).required_permissions,
            serenity::Permissions::empty(),
            "{name}"
        );
    }
    let points = subcommand("points").parameters;
    let names: Vec<&str> = points.iter().map(|(name, _)| name.as_str()).collect();
    assert_eq!(names, ["member", "points", "reason"]);
    assert_eq!(
        points.iter().find(|(name, _)| name == "reason"),
        Some(&("reason".to_owned(), false))
    );
}

#[test]
fn only_the_iq_command_itself_carries_discord_visibility() {
    let iq = command_list(false)
        .into_iter()
        .find(|command| command.name == "iq")
        .expect("iq is registered");
    for subcommand in &iq.subcommands {
        assert_eq!(
            subcommand.default_member_permissions,
            serenity::Permissions::empty(),
            "{}",
            subcommand.name
        );
    }
}
