use barnacle_bot::discord::command_list;
use barnacle_bot::iq;
use barnacle_bot::iq_text;
use barnacle_guess::UserId;
use poise::serenity_prelude as serenity;

#[test]
fn iq_reject_takes_a_message_and_a_reason() {
    let commands = command_list(false);
    let reject = commands
        .iter()
        .find(|command| command.name == "iq")
        .and_then(|iq| {
            iq.subcommands
                .iter()
                .find(|command| command.name == "reject")
        })
        .expect("iq reject is registered");
    assert_eq!(
        reject
            .parameters
            .iter()
            .map(|parameter| (parameter.name.to_string(), parameter.required))
            .collect::<Vec<(String, bool)>>(),
        [("message_id".to_owned(), true), ("reason".to_owned(), true)]
    );
    assert!(reject.checks.is_empty());
    assert_eq!(reject.required_permissions, serenity::Permissions::empty());
}

#[test]
fn a_message_id_must_be_a_snowflake() {
    assert_eq!(
        iq::message_id("1234567890123456789"),
        Some(1234567890123456789)
    );
    assert_eq!(iq::message_id(" 42 "), Some(42));
    assert_eq!(iq::message_id("0"), None);
    assert_eq!(iq::message_id("abc"), None);
    assert_eq!(iq::message_id("12a"), None);
    assert_eq!(iq::message_id("99999999999999999999999"), None);
}

#[test]
fn the_rejection_reply_gives_the_reason() {
    assert_eq!(
        iq_text::rejection_reply("Wrong  *ship*"),
        "**Not counted.** Wrong \\*ship\\*\nPost it again as a new message. Edits are not re-checked."
    );
}

#[test]
fn the_judge_log_records_the_rejection() {
    assert_eq!(
        iq_text::judge_log_reject(UserId::new(21), UserId::new(11), "Wrong ship", "link"),
        "<@21> rejected <@11>'s post: Wrong ship. link"
    );
}
