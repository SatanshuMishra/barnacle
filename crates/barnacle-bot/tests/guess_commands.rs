use barnacle_bot::discord::Data;
use barnacle_bot::discord::Error;
use barnacle_bot::discord::command_list;
use barnacle_bot::text;
use poise::serenity_prelude as serenity;

fn names(commands: &[poise::Command<Data, Error>]) -> Vec<String> {
    commands
        .iter()
        .map(|command| command.name.to_string())
        .collect()
}

fn parameters(command: &poise::Command<Data, Error>) -> Vec<String> {
    command
        .parameters
        .iter()
        .map(|parameter| parameter.name.to_string())
        .collect()
}

fn named<'a>(
    commands: &'a [poise::Command<Data, Error>],
    name: &str,
) -> &'a poise::Command<Data, Error> {
    commands
        .iter()
        .find(|command| command.name == name)
        .unwrap_or_else(|| panic!("{name} is registered among {:?}", names(commands)))
}

#[test]
fn every_feature_has_one_root() {
    let commands = command_list(false);
    assert_eq!(
        names(&commands),
        [
            "guess", "ship", "about", "event", "voice", "updates", "iq", "barnacle"
        ]
    );
}

#[test]
fn guess_groups_its_four_commands() {
    let commands = command_list(false);
    let guess = named(&commands, "guess");
    assert!(guess.subcommand_required);
    assert!(guess.guild_only);
    assert!(guess.parameters.is_empty(), "{:?}", parameters(guess));
    assert_eq!(
        names(&guess.subcommands),
        ["start", "series", "leaderboard", "profile"]
    );
}

#[test]
fn guess_subcommands_keep_their_options() {
    let commands = command_list(false);
    let guess = named(&commands, "guess");
    assert_eq!(
        parameters(named(&guess.subcommands, "start")),
        ["min_tier", "max_tier", "historical"]
    );
    assert_eq!(
        parameters(named(&guess.subcommands, "series")),
        ["min_tier", "max_tier", "historical", "rounds"]
    );
    assert_eq!(
        parameters(named(&guess.subcommands, "leaderboard")),
        ["sort", "limit"]
    );
    assert_eq!(parameters(named(&guess.subcommands, "profile")), ["user"]);
}

#[test]
fn updates_announce_stays_owner_only() {
    let commands = command_list(false);
    let updates = named(&commands, "updates");
    assert_eq!(names(&updates.subcommands), ["setup", "announce"]);
    let announce = named(&updates.subcommands, "announce");
    assert!(announce.owners_only);
    assert_eq!(announce.checks.len(), 1);
    assert!(announce.parameters.is_empty(), "{:?}", parameters(announce));
    assert_eq!(
        announce.default_member_permissions,
        serenity::Permissions::empty()
    );
}

#[test]
fn the_stale_preview_names_the_new_command() {
    assert_eq!(
        text::announce_version_changed("0.2.1"),
        "Barnacle has been updated to 0.2.1 since this preview, so nothing was posted. Run /updates announce again."
    );
}
