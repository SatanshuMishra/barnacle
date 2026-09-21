use barnacle_bot::discord::Data;
use barnacle_bot::discord::Error;
use barnacle_bot::discord::command_list;
use barnacle_bot::discord::command_list_for;

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
