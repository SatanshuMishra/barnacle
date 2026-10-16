mod common;

use barnacle_bot::attendance::Delivery;
use barnacle_bot::attendance::HourTally;
use barnacle_bot::attendance::SignupView;
use barnacle_bot::discord::Data;
use barnacle_bot::discord::Error;
use barnacle_bot::discord::allowed_mentions;
use barnacle_bot::discord::announcement_message;
use barnacle_bot::discord::command_list;
use barnacle_bot::discord::command_list_for;
use barnacle_bot::discord::hours_menu_reply;
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
            "guess",
            "guess-series",
            "ship",
            "profile",
            "leaderboard",
            "about",
            "voice",
            "updates",
            "announce"
        ]
    );
    let expected: Vec<(String, Vec<String>)> = [
        ("guess", &["min_tier", "max_tier", "historical"][..]),
        (
            "guess-series",
            &["min_tier", "max_tier", "historical", "rounds"],
        ),
        ("ship info", &["ship"]),
        ("profile", &["user"]),
        ("leaderboard", &["sort", "limit"]),
        ("about", &[]),
        ("voice hub create", &["room_name", "name", "category"]),
        (
            "voice hub edit",
            &["hub", "name", "room_name", "category", "top_level"],
        ),
        ("voice hub remove", &["hub"]),
        ("voice hub list", &[]),
        ("updates setup", &["channel", "role"]),
        ("announce", &[]),
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
fn the_hours_options_take_1_to_24() {
    for name in ["create", "edit"] {
        let hours = option_json(&event_subcommand(name), "hours");
        assert_eq!(hours["type"], 4, "{name}");
        assert_eq!(hours["min_value"], 1.0, "{name}");
        assert_eq!(hours["max_value"], 24.0, "{name}");
    }
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
        serenity::Permissions::MANAGE_GUILD
    );
    assert_eq!(
        event.required_permissions,
        serenity::Permissions::MANAGE_GUILD
    );
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
            &[
                ("name", true),
                ("first_day", true),
                ("last_day", true),
                ("template", false),
                ("start", false),
                ("hours", false),
                ("time_zone", false),
            ][..],
        ),
        (
            "edit",
            &[
                ("event", true),
                ("name", false),
                ("first_day", false),
                ("last_day", false),
                ("start", false),
                ("hours", false),
                ("time_zone", false),
                ("channel", false),
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
