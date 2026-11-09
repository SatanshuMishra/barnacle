mod common;

use barnacle_bot::attendance::Delivery;
use barnacle_bot::attendance::HourTally;
use barnacle_bot::attendance::SignupView;
use barnacle_bot::discord::Data;
use barnacle_bot::discord::Error;
use barnacle_bot::discord::access_rows;
use barnacle_bot::discord::allowed_mentions;
use barnacle_bot::discord::announcement_message;
use barnacle_bot::discord::command_list;
use barnacle_bot::discord::command_list_for;
use barnacle_bot::discord::hours_menu_reply;
use barnacle_bot::discord::organiser_roles_text;
use barnacle_bot::discord::signup_rows;
use barnacle_bot::ids::Ping;
use barnacle_bot::ids::RoleId;
use barnacle_bot::release_notes;
use barnacle_bot::schedule::Hour;
use barnacle_bot::schedule::Hours;
use barnacle_bot::schedule::Schedule;
use barnacle_bot::schedule::Span;
use barnacle_bot::schedule::StartTime;
use barnacle_bot::schedule::Weekdays;
use barnacle_bot::schedule::Zone;
use barnacle_bot::schedule::parse_day;
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

fn leaves(commands: &[poise::Command<Data, Error>], prefix: &str) -> Vec<(String, Vec<String>)> {
    commands
        .iter()
        .flat_map(|command| {
            let path = format!("{prefix}{}", command.name);
            if command.subcommands.is_empty() {
                vec![(
                    path,
                    command
                        .parameters
                        .iter()
                        .map(|parameter| parameter.name.to_string())
                        .collect(),
                )]
            } else {
                leaves(&command.subcommands, &format!("{path} "))
            }
        })
        .collect()
}

fn view_for(count: i64) -> SignupView {
    let schedule = Schedule::new(
        Weekdays::EVERY_DAY,
        StartTime::parse("23:30").unwrap(),
        Zone::find("UTC").unwrap(),
        Hours::new(count).unwrap(),
    );
    let session = schedule
        .session_at(parse_day("2026-10-14").unwrap())
        .unwrap();
    SignupView {
        event: 1,
        name: "Ranked Sprint".to_owned(),
        session,
        open: true,
        pings: Vec::new(),
        hours: session
            .hours()
            .each()
            .map(|hour| HourTally {
                hour,
                attending: 0,
                nope: 0,
            })
            .collect(),
        rows: Vec::new(),
        hidden: 0,
    }
}

fn row_labels(view: &SignupView) -> Vec<Vec<String>> {
    signup_rows(view)
        .iter()
        .map(|row| {
            serde_json::to_value(row).unwrap()["components"]
                .as_array()
                .unwrap()
                .iter()
                .map(|button| button["label"].as_str().unwrap().to_owned())
                .collect()
        })
        .collect()
}

#[test]
fn a_short_event_gets_one_row_per_hour() {
    for (count, rows) in [(1, 2), (2, 3), (4, 5)] {
        let labels = row_labels(&view_for(count));
        assert_eq!(labels.len(), rows, "{count} hours: {labels:?}");
        assert_eq!(labels[0], ["Attend all", "Nope all"]);
        for (number, row) in labels.iter().enumerate().skip(1) {
            assert_eq!(
                row,
                &[
                    format!("Hour {number}: Attending"),
                    format!("Hour {number}: Nope")
                ]
            );
        }
    }
}

#[test]
fn a_long_event_gets_one_row_with_choose_hours() {
    for count in [5, 12, 24] {
        assert_eq!(
            row_labels(&view_for(count)),
            [["Attend all", "Nope all", "Choose hours"]],
            "{count} hours"
        );
    }
    let rows = signup_rows(&view_for(12));
    let row = serde_json::to_value(&rows[0]).unwrap();
    assert_eq!(
        row["components"][2]["custom_id"],
        "barnacle-cb-pick:1:2026-10-14"
    );
    assert_eq!(
        row["components"][0]["custom_id"],
        "barnacle-cb:1:2026-10-14:all:in"
    );
}

fn denver_six_hours() -> SignupView {
    let schedule = Schedule::new(
        Weekdays::new(&[jiff::civil::Weekday::Wednesday]).unwrap(),
        StartTime::parse("17:00").unwrap(),
        Zone::find("America/Denver").unwrap(),
        Hours::new(6).unwrap(),
    );
    let session = schedule
        .session_at(parse_day("2026-10-14").unwrap())
        .unwrap();
    SignupView {
        session,
        hours: session
            .hours()
            .each()
            .map(|hour| HourTally {
                hour,
                attending: 0,
                nope: 0,
            })
            .collect(),
        ..view_for(6)
    }
}

fn hour_list(numbers: &[u8]) -> Vec<Hour> {
    numbers
        .iter()
        .map(|number| Hour::new(*number).unwrap())
        .collect()
}

fn menu_reply(attending: &[u8]) -> serde_json::Value {
    serde_json::to_value(hours_menu_reply(&denver_six_hours(), &hour_list(attending))).unwrap()
}

fn denver_six_hour_day() -> Span {
    Span::new(
        parse_day("2026-10-14").unwrap(),
        parse_day("2026-10-14").unwrap(),
        Schedule::new(
            Weekdays::new(&[jiff::civil::Weekday::Wednesday]).unwrap(),
            StartTime::parse("17:00").unwrap(),
            Zone::find("America/Denver").unwrap(),
            Hours::new(6).unwrap(),
        ),
    )
    .unwrap()
}

fn menu_options(reply: &serde_json::Value) -> Vec<(String, String, bool)> {
    reply["components"][0]["components"][0]["options"]
        .as_array()
        .unwrap()
        .iter()
        .map(|option| {
            (
                option["label"].as_str().unwrap().to_owned(),
                option["value"].as_str().unwrap().to_owned(),
                option["default"].as_bool().unwrap_or(false),
            )
        })
        .collect()
}

#[tokio::test]
async fn the_hours_menu_starts_from_the_members_hours() {
    let reply = common::hours_menu_json(
        denver_six_hour_day(),
        &[
            (200, 1, false),
            (200, 2, true),
            (200, 3, true),
            (200, 4, false),
            (200, 7, true),
            (300, 5, true),
        ],
        200,
        1_791_932_400,
    )
    .await;
    let content = reply["content"].as_str().unwrap();
    for (number, start, end) in [
        (1, 1_792_018_800, 1_792_022_400),
        (2, 1_792_022_400, 1_792_026_000),
        (3, 1_792_026_000, 1_792_029_600),
        (4, 1_792_029_600, 1_792_033_200),
        (5, 1_792_033_200, 1_792_036_800),
        (6, 1_792_036_800, 1_792_040_400),
    ] {
        let line = format!("**Hour {number}** · <t:{start}:t> – <t:{end}:t>");
        assert!(content.contains(&line), "{content}");
    }
    assert!(!content.contains("**Hour 7**"), "{content}");
    assert_eq!(reply["flags"], 64);
    assert_eq!(
        reply["allowed_mentions"],
        serde_json::to_value(serenity::CreateAllowedMentions::new()).unwrap()
    );
    let menu = &reply["components"][0]["components"][0];
    assert_eq!(menu["custom_id"], "barnacle-cb-hours:1:2026-10-14");
    assert_eq!(menu["min_values"], 0);
    assert_eq!(menu["max_values"], 6);
    assert_eq!(
        menu_options(&reply),
        [
            ("Hour 1".to_owned(), "1".to_owned(), false),
            ("Hour 2".to_owned(), "2".to_owned(), true),
            ("Hour 3".to_owned(), "3".to_owned(), true),
            ("Hour 4".to_owned(), "4".to_owned(), false),
            ("Hour 5".to_owned(), "5".to_owned(), false),
            ("Hour 6".to_owned(), "6".to_owned(), false),
        ]
    );
}

#[tokio::test]
async fn a_fresh_hours_menu_selects_nothing() {
    let reply = common::hours_menu_json(
        denver_six_hour_day(),
        &[
            (200, 1, false),
            (200, 2, false),
            (200, 3, false),
            (200, 4, false),
            (200, 5, false),
            (200, 6, false),
        ],
        200,
        1_791_932_400,
    )
    .await;
    assert_eq!(
        menu_options(&reply),
        [
            ("Hour 1".to_owned(), "1".to_owned(), false),
            ("Hour 2".to_owned(), "2".to_owned(), false),
            ("Hour 3".to_owned(), "3".to_owned(), false),
            ("Hour 4".to_owned(), "4".to_owned(), false),
            ("Hour 5".to_owned(), "5".to_owned(), false),
            ("Hour 6".to_owned(), "6".to_owned(), false),
        ]
    );
}

#[test]
fn the_hours_menu_writes_times_as_timestamps() {
    let reply = menu_reply(&[]);
    let content = reply["content"].as_str().unwrap();
    assert!(content.contains("<t:1792018800:t>"), "{content}");
    assert!(!content.contains("17:00"), "{content}");
}

#[test]
fn no_server_is_sent_a_cb_command() {
    assert!(!names(&command_list(false)).iter().any(|name| name == "cb"));
    assert!(!names(&command_list(true)).iter().any(|name| name == "cb"));
}

#[test]
fn rehearse_start_takes_a_name_and_a_session_count() {
    let commands = command_list(true);
    let start = commands
        .iter()
        .find(|command| command.name == GROUP)
        .and_then(|group| {
            group
                .subcommands
                .iter()
                .find(|command| command.name == "start")
        })
        .expect("rehearse start is registered");
    assert_eq!(
        start
            .parameters
            .iter()
            .map(|parameter| (parameter.name.to_string(), parameter.required))
            .collect::<Vec<(String, bool)>>(),
        [
            ("name".to_owned(), true),
            ("ping_role".to_owned(), false),
            ("ping_role_2".to_owned(), false),
            ("ping_role_3".to_owned(), false),
            ("ping_role_4".to_owned(), false),
            ("ping_role_5".to_owned(), false),
            ("sessions".to_owned(), false),
        ]
    );
    let sessions = start
        .parameters
        .iter()
        .find(|parameter| parameter.name == "sessions")
        .and_then(|parameter| parameter.create_as_slash_command_option())
        .map(|option| serde_json::to_value(option).unwrap())
        .unwrap();
    assert_eq!(sessions["min_value"], 1.0);
    assert_eq!(sessions["max_value"], 7.0);
    assert_eq!(sessions["required"], false);
}

#[test]
fn the_commands_outside_events_keep_their_options() {
    let commands: Vec<poise::Command<Data, Error>> = command_list(false)
        .into_iter()
        .filter(|command| command.name != "event")
        .collect();
    assert_eq!(
        names(&commands),
        [
            "guess", "ship", "about", "voice", "updates", "iq", "barnacle"
        ]
    );
    let expected: Vec<(String, Vec<String>)> = [
        ("guess start", &["min_tier", "max_tier", "historical"][..]),
        (
            "guess series",
            &["min_tier", "max_tier", "historical", "rounds"],
        ),
        ("guess leaderboard", &["sort", "limit"]),
        ("guess profile", &["user"]),
        ("ship info", &["ship"]),
        ("about", &[]),
        ("voice hub create", &["room_name", "name", "category"]),
        (
            "voice hub edit",
            &["hub", "name", "room_name", "category", "top_level"],
        ),
        ("voice hub remove", &["hub"]),
        ("voice hub list", &[]),
        ("updates setup", &["channel", "role"]),
        ("updates announce", &[]),
        ("iq start", &["time_zone"]),
        ("iq edit", &[]),
        ("iq stop", &[]),
        ("iq complete", &[]),
        ("iq cleanup", &[]),
        ("iq judges", &[]),
        ("iq points", &["member", "points", "reason"]),
        ("iq reject", &["message_id", "reason"]),
        ("iq leaderboard", &[]),
        ("barnacle access", &[]),
    ]
    .iter()
    .map(|(path, options)| {
        (
            (*path).to_owned(),
            options.iter().map(|option| (*option).to_owned()).collect(),
        )
    })
    .collect();
    assert_eq!(leaves(&commands, ""), expected);
}

fn event_subcommand(name: &str) -> poise::Command<Data, Error> {
    command_list(false)
        .into_iter()
        .find(|command| command.name == "event")
        .and_then(|group| {
            group
                .subcommands
                .into_iter()
                .find(|command| command.name == name)
        })
        .unwrap_or_else(|| panic!("event {name} is registered"))
}

fn option_json(command: &poise::Command<Data, Error>, name: &str) -> serde_json::Value {
    command
        .parameters
        .iter()
        .find(|parameter| parameter.name == name)
        .and_then(|parameter| parameter.create_as_slash_command_option())
        .map(|option| serde_json::to_value(option).unwrap())
        .unwrap_or_else(|| panic!("{name} is an option of {}", command.name))
}

#[test]
fn the_template_option_offers_clan_battles() {
    let template = option_json(&event_subcommand("create"), "template");
    assert_eq!(template["type"], 4);
    assert_eq!(template["required"], false);
    assert_eq!(
        template["choices"]
            .as_array()
            .unwrap()
            .iter()
            .map(|choice| choice["name"].as_str().unwrap())
            .collect::<Vec<&str>>(),
        ["Clan Battles"]
    );
}

#[test]
fn the_event_command_has_its_five_subcommands() {
    let commands = command_list(false);
    let event = commands
        .iter()
        .find(|command| command.name == "event")
        .expect("event is registered");
    assert!(event.guild_only);
    assert!(event.subcommand_required);
    assert_eq!(
        event.default_member_permissions,
        serenity::Permissions::empty()
    );
    assert_eq!(event.required_permissions, serenity::Permissions::empty());
    assert_eq!(event.checks.len(), 1);
    for subcommand in &event.subcommands {
        assert_eq!(subcommand.checks.len(), 1, "{}", subcommand.name);
    }
    assert_eq!(
        names(&event.subcommands),
        ["create", "edit", "repost", "end", "list"]
    );
    let options: Vec<(String, Vec<(String, bool)>)> = event
        .subcommands
        .iter()
        .map(|command| {
            (
                command.name.to_string(),
                command
                    .parameters
                    .iter()
                    .map(|parameter| (parameter.name.to_string(), parameter.required))
                    .collect(),
            )
        })
        .collect();
    let expected: Vec<(String, Vec<(String, bool)>)> = [
        (
            "create",
            &[("name", true), ("template", false), ("time_zone", false)][..],
        ),
        (
            "edit",
            &[
                ("event", true),
                ("name", false),
                ("channel", false),
                ("time_zone", false),
            ],
        ),
        ("repost", &[("event", true), ("ping_again", false)]),
        ("end", &[("event", true)]),
        ("list", &[]),
    ]
    .iter()
    .map(|(command, options)| {
        (
            (*command).to_owned(),
            options
                .iter()
                .map(|(option, required)| ((*option).to_owned(), *required))
                .collect(),
        )
    })
    .collect();
    assert_eq!(options, expected);
}

#[test]
fn the_edit_channel_option_takes_text_channels() {
    let channel = option_json(&event_subcommand("edit"), "channel");
    assert_eq!(channel["type"], 7);
    assert_eq!(channel["channel_types"], serde_json::json!([0]));
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
    let series_of = |commands: Vec<poise::Command<Data, Error>>| {
        commands
            .into_iter()
            .find(|command| command.name == "guess")
            .and_then(|guess| {
                guess
                    .subcommands
                    .into_iter()
                    .find(|command| command.name == "series")
            })
    };
    let series = series_of(command_list(false)).expect("guess series is registered");
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
    assert!(series_of(command_list(true)).is_some());
}

#[test]
fn announce_is_owner_only_and_updates_setup_is_for_organisers() {
    let commands = command_list(false);
    let updates = commands
        .iter()
        .find(|command| command.name == "updates")
        .expect("updates is registered");
    let announce = updates
        .subcommands
        .iter()
        .find(|command| command.name == "announce")
        .expect("updates announce is registered");
    assert!(announce.owners_only);
    assert!(announce.guild_only);
    assert_eq!(
        announce.default_member_permissions,
        serenity::Permissions::empty()
    );
    assert_eq!(announce.checks.len(), 1);
    assert!(updates.guild_only);
    assert!(updates.subcommand_required);
    assert_eq!(
        updates.default_member_permissions,
        serenity::Permissions::empty()
    );
    assert_eq!(updates.required_permissions, serenity::Permissions::empty());
    assert_eq!(updates.checks.len(), 1);
    assert_eq!(updates.subcommands[0].checks.len(), 1);
    assert_eq!(names(&updates.subcommands), ["setup", "announce"]);
    assert_eq!(
        updates.subcommands[0]
            .parameters
            .iter()
            .map(|parameter| (parameter.name.to_string(), parameter.required))
            .collect::<Vec<(String, bool)>>(),
        [("channel".to_owned(), true), ("role".to_owned(), false)]
    );
}

#[test]
fn an_update_post_is_a_plain_message_that_pings_only_its_role() {
    let releases = release_notes::all().unwrap();
    let release = release_notes::find(&releases, "0.2.0").unwrap();
    let message = serde_json::to_value(announcement_message(
        release,
        Some(Ping::Role(RoleId::new(123))),
    ))
    .unwrap();
    assert_eq!(message["content"], format!("<@&123>\n{}", release.render()));
    assert!(
        message
            .get("embeds")
            .is_none_or(|embeds| embeds.as_array().is_some_and(Vec::is_empty)),
        "{message}"
    );
    assert_eq!(
        message["allowed_mentions"],
        serde_json::to_value(
            serenity::CreateAllowedMentions::new().roles([serenity::RoleId::new(123)])
        )
        .unwrap()
    );
}

#[test]
fn voice_and_rehearse_are_for_organisers_at_every_level() {
    let commands = command_list(true);
    for name in ["voice", "rehearse"] {
        let command = commands
            .iter()
            .find(|command| command.name == name)
            .unwrap_or_else(|| panic!("{name} is registered"));
        assert_eq!(
            command.default_member_permissions,
            serenity::Permissions::empty(),
            "{name}"
        );
        assert_eq!(
            command.required_permissions,
            serenity::Permissions::empty(),
            "{name}"
        );
        let mut pending: Vec<&poise::Command<Data, Error>> = vec![command];
        while let Some(next) = pending.pop() {
            assert_eq!(next.checks.len(), 1, "{name} {}", next.name);
            pending.extend(next.subcommands.iter());
        }
    }
}

#[test]
fn the_access_command_is_listed_for_administrators_only() {
    for rehearsing in [false, true] {
        let commands = command_list(rehearsing);
        let barnacle = commands
            .iter()
            .find(|command| command.name == "barnacle")
            .expect("barnacle is registered");
        assert!(barnacle.guild_only);
        assert!(barnacle.subcommand_required);
        assert_eq!(
            barnacle.default_member_permissions,
            serenity::Permissions::ADMINISTRATOR
        );
        assert_eq!(
            barnacle.required_permissions,
            serenity::Permissions::ADMINISTRATOR
        );
        assert_eq!(names(&barnacle.subcommands), ["access"]);
        assert!(barnacle.subcommands[0].parameters.is_empty());
    }
}

#[test]
fn the_access_panel_offers_up_to_25_roles_with_the_stored_ones_picked() {
    let rows = serde_json::to_value(access_rows(42, &[RoleId::new(7), RoleId::new(9)])).unwrap();
    let rows = rows.as_array().unwrap();
    assert_eq!(rows.len(), 2);
    let menu = &rows[0]["components"][0];
    assert_eq!(menu["custom_id"], "barnacle-access:42:roles");
    assert_eq!(menu["type"], 6);
    assert_eq!(menu["min_values"], 0);
    assert_eq!(menu["max_values"], 25);
    assert_eq!(
        menu["default_values"],
        serde_json::json!([
            {"id": 7, "type": "role"},
            {"id": 9, "type": "role"},
        ])
    );
    let buttons: Vec<(&str, &str)> = rows[1]["components"]
        .as_array()
        .unwrap()
        .iter()
        .map(|button| {
            (
                button["custom_id"].as_str().unwrap(),
                button["label"].as_str().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        buttons,
        [
            ("barnacle-access:42:save", "Save"),
            ("barnacle-access:42:cancel", "Cancel"),
        ]
    );
}

#[test]
fn the_access_panel_starts_empty_when_no_roles_are_stored() {
    let rows = serde_json::to_value(access_rows(42, &[])).unwrap();
    let menu = &rows[0]["components"][0];
    assert_eq!(menu["min_values"], 0);
    assert_eq!(menu["max_values"], 25);
    assert!(menu.get("default_values").is_none(), "{menu}");
}

#[test]
fn saved_organiser_roles_are_named_as_role_mentions() {
    assert_eq!(
        organiser_roles_text(&[RoleId::new(1), RoleId::new(2)]),
        "Organiser roles: <@&1>, <@&2>."
    );
    assert_eq!(
        organiser_roles_text(&[]),
        "No organiser roles set. Only Administrators can run organiser commands."
    );
}
