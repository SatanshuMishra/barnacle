use barnacle_bot::discord::command_list;

fn event_parameters(subcommand: &str) -> Vec<String> {
    command_list(false)
        .into_iter()
        .find(|command| command.name == "event")
        .expect("event is registered")
        .subcommands
        .into_iter()
        .find(|command| command.name == subcommand)
        .unwrap_or_else(|| panic!("event {subcommand} is registered"))
        .parameters
        .iter()
        .map(|parameter| parameter.name.to_string())
        .collect()
}

#[test]
fn event_create_and_edit_take_only_the_kept_options() {
    assert_eq!(
        event_parameters("create"),
        ["name", "template", "time_zone"]
    );
    assert_eq!(
        event_parameters("edit"),
        ["event", "name", "channel", "time_zone"]
    );
}
