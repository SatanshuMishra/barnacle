use barnacle_bot::attendance::Delivery;
use barnacle_bot::discord::Data;
use barnacle_bot::discord::Error;
use barnacle_bot::discord::allowed_mentions;
use barnacle_bot::discord::command_list;
use barnacle_bot::discord::command_list_for;
use barnacle_bot::ids::Ping;
use barnacle_bot::ids::RoleId;
use barnacle_bot::wiring::ping_allowance;
use poise::serenity_prelude as serenity;

const CLAN: u64 = 111_111_111_111_111_111;
const REHEARSAL: u64 = 222_222_222_222_222_222;
const GROUP: &str = "rehearse";

fn names(commands: &[poise::Command<Data, Error>]) -> Vec<String> {
    commands
        .iter()
        .map(|command| command.name.to_string())
        .collect()
}

fn carries_rehearse(guild: u64, rehearsal: &[u64]) -> bool {
    names(&command_list_for(guild, rehearsal))
        .iter()
        .any(|name| name == GROUP)
}

#[test]
fn a_server_outside_the_rehearsal_list_is_sent_no_rehearsal_commands() {
    assert!(!carries_rehearse(CLAN, &[REHEARSAL]));
    assert!(!carries_rehearse(CLAN, &[]));
    assert!(!carries_rehearse(CLAN, &[REHEARSAL, REHEARSAL]));
}

#[test]
fn a_rehearsal_server_is_sent_the_rehearsal_commands() {
    assert!(carries_rehearse(REHEARSAL, &[REHEARSAL]));
    assert!(carries_rehearse(REHEARSAL, &[CLAN, REHEARSAL]));
}

#[test]
fn the_rehearsal_list_adds_nothing_but_the_rehearse_group() {
    let plain = names(&command_list(false));
    let rehearsing = names(&command_list(true));
    assert_eq!(rehearsing.len(), plain.len() + 1);
    for name in &plain {
        assert!(rehearsing.contains(name));
    }
    assert!(!plain.iter().any(|name| name == GROUP));
}

#[test]
fn every_server_is_sent_the_voice_commands() {
    assert!(
        names(&command_list(false))
            .iter()
            .any(|name| name == "voice")
    );
    assert!(
        names(&command_list(true))
            .iter()
            .any(|name| name == "voice")
    );
}

#[test]
fn only_a_first_post_asks_discord_to_ping_everyone() {
    assert_eq!(
        allowed_mentions(ping_allowance(Delivery::New, &[Ping::Everyone])),
        serenity::CreateAllowedMentions::new().everyone(true)
    );
    assert_eq!(
        allowed_mentions(ping_allowance(Delivery::Redraw, &[Ping::Everyone])),
        serenity::CreateAllowedMentions::new()
    );
    assert_eq!(
        allowed_mentions(ping_allowance(
            Delivery::New,
            &[Ping::Role(RoleId::new(123))]
        )),
        serenity::CreateAllowedMentions::new().roles([serenity::RoleId::new(123)])
    );
    assert_eq!(
        allowed_mentions(ping_allowance(
            Delivery::Redraw,
            &[Ping::Role(RoleId::new(123))]
        )),
        serenity::CreateAllowedMentions::new()
    );
    assert_eq!(
        allowed_mentions(ping_allowance(
            Delivery::New,
            &[Ping::Role(RoleId::new(123)), Ping::Role(RoleId::new(456))]
        )),
        serenity::CreateAllowedMentions::new()
            .roles([serenity::RoleId::new(123), serenity::RoleId::new(456)])
    );
}

#[test]
fn every_server_is_sent_guess_series_with_its_rounds_bounds() {
    let commands = command_list(false);
    let series = commands
        .iter()
        .find(|command| command.name == "guess-series")
        .expect("guess-series is registered");
    assert_eq!(
        series
            .parameters
            .iter()
            .map(|parameter| parameter.name.to_string())
            .collect::<Vec<String>>(),
        ["min_tier", "max_tier", "historical", "rounds"]
    );
    let rounds = series
        .parameters
        .iter()
        .find(|parameter| parameter.name == "rounds")
        .and_then(|parameter| parameter.create_as_slash_command_option())
        .map(|option| serde_json::to_value(option).unwrap())
        .unwrap();
    assert_eq!(rounds["min_value"], 2.0);
    assert_eq!(rounds["max_value"], 20.0);
    assert_eq!(rounds["required"], false);
    assert!(
        names(&command_list(true))
            .iter()
            .any(|name| name == "guess-series")
    );
}

#[test]
fn announce_is_owner_only_and_updates_setup_needs_manage_server() {
    let commands = command_list(false);
    let named = |name: &str| {
        commands
            .iter()
            .find(|command| command.name == name)
            .unwrap_or_else(|| panic!("{name} is registered"))
    };
    let announce = named("announce");
    assert!(announce.owners_only);
    assert!(announce.guild_only);
    assert_eq!(
        announce.default_member_permissions,
        serenity::Permissions::MANAGE_GUILD
    );
    let updates = named("updates");
    assert!(updates.guild_only);
    assert!(updates.subcommand_required);
    assert_eq!(
        updates.default_member_permissions,
        serenity::Permissions::MANAGE_GUILD
    );
    assert_eq!(
        updates.required_permissions,
        serenity::Permissions::MANAGE_GUILD
    );
    assert_eq!(names(&updates.subcommands), ["setup"]);
    assert_eq!(
        updates.subcommands[0]
            .parameters
            .iter()
            .map(|parameter| (parameter.name.to_string(), parameter.required))
            .collect::<Vec<(String, bool)>>(),
        [("channel".to_owned(), true), ("role".to_owned(), false)]
    );
}
