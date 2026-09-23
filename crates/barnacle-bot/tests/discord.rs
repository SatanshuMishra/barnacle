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
        allowed_mentions(ping_allowance(Delivery::New, Some(Ping::Everyone))),
        serenity::CreateAllowedMentions::new().everyone(true)
    );
    assert_eq!(
        allowed_mentions(ping_allowance(Delivery::Redraw, Some(Ping::Everyone))),
        serenity::CreateAllowedMentions::new()
    );
    assert_eq!(
        allowed_mentions(ping_allowance(
            Delivery::New,
            Some(Ping::Role(RoleId::new(123)))
        )),
        serenity::CreateAllowedMentions::new().roles([serenity::RoleId::new(123)])
    );
    assert_eq!(
        allowed_mentions(ping_allowance(
            Delivery::Redraw,
            Some(Ping::Role(RoleId::new(123)))
        )),
        serenity::CreateAllowedMentions::new()
    );
}
