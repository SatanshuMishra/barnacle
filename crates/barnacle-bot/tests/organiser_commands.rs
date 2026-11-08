use barnacle_bot::discord;
use barnacle_bot::discord::Data;
use barnacle_bot::discord::Error;
use barnacle_bot::text;
use poise::serenity_prelude as serenity;

const ORGANISER_COMMANDS: [&str; 4] = ["event", "rehearse", "updates", "voice"];
const ORGANISER_IQ_SUBCOMMANDS: [&str; 6] =
    ["start", "edit", "stop", "complete", "cleanup", "judges"];

fn named<'a>(
    commands: &'a [poise::Command<Data, Error>],
    name: &str,
) -> &'a poise::Command<Data, Error> {
    commands
        .iter()
        .find(|command| command.name == name)
        .unwrap_or_else(|| panic!("{name} is registered"))
}

#[test]
fn organiser_commands_carry_the_organiser_check() {
    let commands = discord::command_list(true);
    for name in ORGANISER_COMMANDS {
        let command = named(&commands, name);
        assert!(
            command.default_member_permissions.is_empty(),
            "{name} has default member permissions {:?}",
            command.default_member_permissions
        );
        assert!(!command.checks.is_empty(), "{name} has no checks");
    }
    let updates = named(&commands, "updates");
    for name in ["setup", "announce"] {
        let subcommand = named(&updates.subcommands, name);
        assert!(
            !subcommand.checks.is_empty(),
            "updates {name} has no checks"
        );
    }
    let iq = named(&commands, "iq");
    assert!(
        iq.default_member_permissions.is_empty(),
        "iq has default member permissions {:?}",
        iq.default_member_permissions
    );
    for name in ORGANISER_IQ_SUBCOMMANDS {
        let subcommand = named(&iq.subcommands, name);
        assert!(!subcommand.checks.is_empty(), "iq {name} has no checks");
    }
    let leaderboard = named(&iq.subcommands, "leaderboard");
    assert!(
        leaderboard.checks.is_empty(),
        "iq leaderboard has {} checks",
        leaderboard.checks.len()
    );
}

#[test]
fn the_access_command_needs_administrator() {
    let commands = discord::command_list(false);
    let barnacle = named(&commands, "barnacle");
    named(&barnacle.subcommands, "access");
    assert_eq!(
        barnacle.default_member_permissions,
        serenity::Permissions::ADMINISTRATOR
    );
}

#[test]
fn a_refused_member_is_told_who_grants_access() {
    assert_eq!(
        text::NOT_AN_ORGANISER,
        "Only organisers can use this command. An Administrator can name organiser roles with /barnacle access."
    );
}
