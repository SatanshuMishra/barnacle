mod common;

use std::io::Read;
use std::io::Seek;

use barnacle_bot::failure;
use barnacle_bot::failure::DiscordRefusal;
use barnacle_bot::failure::Failure;
use barnacle_bot::failure::Kind;
use barnacle_bot::failure::Scope;
use barnacle_bot::ids::ChannelId;
use barnacle_bot::ids::GuildId;
use barnacle_bot::logging;
use barnacle_bot::logging::LogFormat;
use barnacle_bot::logging::LogSettings;
use barnacle_bot::logging::LogSettingsError;
use barnacle_guess::Snowflake;
use barnacle_guess::UserId;

const GUILD: u64 = 1_180_000_000_000_000_001;
const CHANNEL: u64 = 1_180_000_000_000_000_002;
const USER: u64 = 1_180_000_000_000_000_003;

const SCOPE_FIELDS: [&str; 12] = [
    "discord.guild.id",
    "discord.channel.id",
    "discord.user.id",
    "discord.interaction.id",
    "discord.message.id",
    "discord.command.name",
    "barnacle.season.id",
    "barnacle.night",
    "barnacle.hub.id",
    "barnacle.room.id",
    "barnacle.room.number",
    "barnacle.round.number",
];

fn settings(pairs: &[(&str, &str)]) -> Result<LogSettings, LogSettingsError> {
    LogSettings::from_lookup(|name| {
        pairs
            .iter()
            .find(|(key, _)| *key == name)
            .map(|(_, value)| (*value).to_owned())
    })
}

fn written(settings: &LogSettings, ansi: bool, emit: impl FnOnce()) -> String {
    let mut file = tempfile::tempfile().unwrap();
    tracing::subscriber::with_default(
        logging::subscriber(settings, file.try_clone().unwrap(), ansi),
        emit,
    );
    file.rewind().unwrap();
    let mut text = String::new();
    file.read_to_string(&mut text).unwrap();
    text
}

fn text_at(level: &str) -> LogSettings {
    LogSettings {
        format: LogFormat::Text,
        level: level.to_owned(),
    }
}

fn json_at(level: &str) -> LogSettings {
    LogSettings {
        format: LogFormat::Json,
        level: level.to_owned(),
    }
}

fn only(events: Vec<serde_json::Value>) -> serde_json::Value {
    assert_eq!(events.len(), 1, "{events:?}");
    events.into_iter().next().unwrap()
}

#[test]
fn json_events_carry_the_standard_fields_at_the_top_level() {
    let logs = common::logs::capture();
    let failure = Failure::missing_permissions(vec!["Manage Channels", "Move Members"]);
    let scope = Scope::default()
        .guild(GuildId::new(GUILD))
        .channel(ChannelId::new(CHANNEL))
        .user(UserId::new(USER));
    failure::report(
        "voice.room.open_failed",
        "a Join to Create room could not be opened",
        &failure,
        &scope,
    );
    let event = only(logs.named("voice.room.open_failed"));
    assert_eq!(event["level"], "WARN");
    assert_eq!(
        event["message"],
        "a Join to Create room could not be opened"
    );
    assert_eq!(event["target"], "barnacle_bot::failure");
    assert!(event["timestamp"].is_string());
    assert_eq!(event["event.name"], "voice.room.open_failed");
    assert_eq!(event["event.outcome"], "failure");
    assert_eq!(event["error.type"], "discord.missing_permissions");
    assert_eq!(
        event["exception.message"],
        "Barnacle lacks Manage Channels and Move Members"
    );
    assert_eq!(event["barnacle.reference"], failure.reference.as_str());
    assert_eq!(
        event["barnacle.permissions.missing"],
        "Manage Channels, Move Members"
    );
    assert_eq!(event["discord.guild.id"], GUILD.to_string());
    assert_eq!(event["discord.channel.id"], CHANNEL.to_string());
    assert_eq!(event["discord.user.id"], USER.to_string());
    let object = event.as_object().unwrap();
    for absent in SCOPE_FIELDS.iter().skip(3).chain(&[
        "http.response.status_code",
        "discord.error.code",
        "discord.error.message",
        "barnacle.refusal.reason",
        "fields",
        "span",
        "spans",
    ]) {
        assert!(!object.contains_key(*absent), "{absent} in {event}");
    }
}

#[test]
fn log_settings_default_to_text_at_info_and_reject_unknown_values() {
    let default = Ok(text_at("info"));
    assert_eq!(settings(&[]), default);
    assert_eq!(
        settings(&[("BARNACLE_LOG_FORMAT", ""), ("BARNACLE_LOG_LEVEL", "")]),
        default
    );
    assert_eq!(
        settings(&[("BARNACLE_LOG_FORMAT", "  "), ("BARNACLE_LOG_LEVEL", " ")]),
        default
    );
    assert_eq!(settings(&[("BARNACLE_LOG_FORMAT", "Text")]), default);
    assert_eq!(
        settings(&[("BARNACLE_LOG_FORMAT", " JSON ")]),
        Ok(json_at("info"))
    );
    assert_eq!(
        settings(&[
            ("BARNACLE_LOG_FORMAT", "json"),
            ("BARNACLE_LOG_LEVEL", "info,barnacle_bot=debug")
        ]),
        Ok(json_at("info,barnacle_bot=debug"))
    );
    assert_eq!(
        settings(&[("BARNACLE_LOG_LEVEL", "warn")]),
        Ok(text_at("warn"))
    );

    let format = settings(&[("BARNACLE_LOG_FORMAT", "yaml")]);
    assert_eq!(
        format,
        Err(LogSettingsError::Format {
            value: "yaml".to_owned()
        })
    );
    assert_eq!(
        format.unwrap_err().to_string(),
        "BARNACLE_LOG_FORMAT is \"yaml\"; it must be text or json"
    );
    let level = settings(&[("BARNACLE_LOG_LEVEL", "info,barnacle_bot=loud")]);
    assert_eq!(
        level,
        Err(LogSettingsError::Level {
            value: "info,barnacle_bot=loud".to_owned()
        })
    );
    assert_eq!(
        level.unwrap_err().to_string(),
        "BARNACLE_LOG_LEVEL is \"info,barnacle_bot=loud\", which is not a level filter such as info or info,barnacle_bot=debug"
    );
}

#[test]
fn failures_on_barnacles_side_are_errors() {
    let logs = common::logs::capture();
    let failure = Failure::from_error(&sqlx::Error::RowNotFound);
    failure::report(
        "signup.tick.failed",
        "a sign-up beat failed",
        &failure,
        &Scope::default().season(12).night("2026-09-21"),
    );
    let event = only(logs.named("signup.tick.failed"));
    assert_eq!(event["level"], "ERROR");
    assert_eq!(event["error.type"], "database");
    assert_eq!(event["barnacle.season.id"], "12");
    assert_eq!(event["barnacle.night"], "2026-09-21");
    assert!(event.get("barnacle.permissions.missing").is_none());
}

#[test]
fn discord_refusals_are_logged_with_their_status_code_and_message() {
    let logs = common::logs::capture();
    let refused = DiscordRefusal {
        status: 400,
        code: 50035,
        message: "Invalid Form Body".to_owned(),
        fields: vec!["name: Must be 100 or fewer in length.".to_owned()],
    };
    let failure = Failure::from_error(&refused);
    assert_eq!(failure.kind, Kind::InvalidRequest);
    failure::report(
        "signup.post.failed",
        "a sign-up post could not be published",
        &failure,
        &Scope::default().message(Snowflake::new(GUILD)),
    );
    let event = only(logs.named("signup.post.failed"));
    assert_eq!(event["level"], "WARN");
    assert_eq!(event["error.type"], "discord.invalid_request");
    assert_eq!(event["http.response.status_code"], 400);
    assert_eq!(event["discord.error.code"], 50035);
    assert_eq!(event["discord.error.message"], "Invalid Form Body");
    assert_eq!(
        event["exception.message"],
        "Invalid Form Body (Discord error 50035, HTTP 400)"
    );
    assert_eq!(event["discord.message.id"], GUILD.to_string());
}

#[test]
fn refusals_are_info_events_with_a_reason() {
    let logs = common::logs::capture();
    failure::refused(
        "voice.room.refused",
        "a member was refused a room",
        "hub_full",
        &Scope::default().hub(ChannelId::new(CHANNEL)).room_number(3),
    );
    let event = only(logs.named("voice.room.refused"));
    assert_eq!(event["level"], "INFO");
    assert_eq!(event["message"], "a member was refused a room");
    assert_eq!(event["event.outcome"], "refused");
    assert_eq!(event["barnacle.refusal.reason"], "hub_full");
    assert_eq!(event["barnacle.hub.id"], CHANNEL.to_string());
    assert_eq!(event["barnacle.room.number"], 3);
    assert!(event.get("error.type").is_none());
    assert!(event.get("barnacle.reference").is_none());
    assert!(event.get("discord.guild.id").is_none());
}

#[test]
fn a_refusal_with_an_underlying_error_logs_its_detail() {
    let logs = common::logs::capture();
    failure::refused_because(
        "command.refused",
        "a command option could not be read",
        "invalid_option",
        "`next week` is not a number",
        &Scope::default().command("cb season start"),
    );
    let event = only(logs.named("command.refused"));
    assert_eq!(event["level"], "INFO");
    assert_eq!(event["event.outcome"], "refused");
    assert_eq!(event["barnacle.refusal.reason"], "invalid_option");
    assert_eq!(event["exception.message"], "`next week` is not a number");
    assert_eq!(event["discord.command.name"], "cb season start");
    assert!(event.get("barnacle.reference").is_none());
    assert!(event.get("error.type").is_none());
}

#[test]
fn records_are_info_events_with_a_success_outcome() {
    let logs = common::logs::capture();
    let scope = Scope::default()
        .guild(GuildId::new(GUILD))
        .channel(ChannelId::new(CHANNEL))
        .user(UserId::new(USER))
        .interaction(GUILD + 10)
        .message(Snowflake::new(GUILD + 11))
        .command("voice hub create")
        .season(4)
        .night("2026-09-21")
        .hub(ChannelId::new(GUILD + 12))
        .room(ChannelId::new(GUILD + 13))
        .room_number(2)
        .round(7);
    failure::record("voice.room.opened", "a Join to Create room opened", &scope);
    let event = only(logs.named("voice.room.opened"));
    assert_eq!(event["level"], "INFO");
    assert_eq!(event["event.outcome"], "success");
    assert_eq!(event["message"], "a Join to Create room opened");
    assert_eq!(event["discord.guild.id"], GUILD.to_string());
    assert_eq!(event["discord.channel.id"], CHANNEL.to_string());
    assert_eq!(event["discord.user.id"], USER.to_string());
    assert_eq!(event["discord.interaction.id"], (GUILD + 10).to_string());
    assert_eq!(event["discord.message.id"], (GUILD + 11).to_string());
    assert_eq!(event["discord.command.name"], "voice hub create");
    assert_eq!(event["barnacle.season.id"], "4");
    assert_eq!(event["barnacle.night"], "2026-09-21");
    assert_eq!(event["barnacle.hub.id"], (GUILD + 12).to_string());
    assert_eq!(event["barnacle.room.id"], (GUILD + 13).to_string());
    assert_eq!(event["barnacle.room.number"], 2);
    assert_eq!(event["barnacle.round.number"], 7);
    assert!(event.get("error.type").is_none());
    assert!(event.get("barnacle.refusal.reason").is_none());
}

#[test]
fn text_logs_carry_colour_codes_only_when_asked() {
    let plain = written(&text_at("info"), false, || tracing::info!("the beat ran"));
    assert!(plain.contains("the beat ran"), "{plain}");
    assert!(!plain.contains('\u{1b}'), "{plain}");
    let coloured = written(&text_at("info"), true, || tracing::info!("the beat ran"));
    assert!(coloured.contains('\u{1b}'), "{coloured}");
    let json = written(&json_at("info"), true, || tracing::info!("the beat ran"));
    assert!(!json.contains('\u{1b}'), "{json}");
    let line: serde_json::Value = serde_json::from_str(json.trim()).unwrap();
    assert_eq!(line["message"], "the beat ran");
}

#[test]
fn the_level_setting_filters_what_is_written() {
    let quiet = written(&json_at("warn"), false, || {
        tracing::info!("the beat ran");
        tracing::warn!("the beat was late");
    });
    let lines: Vec<serde_json::Value> = quiet
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(lines.len(), 1, "{quiet}");
    assert_eq!(lines[0]["message"], "the beat was late");
    let detailed = written(&json_at("info,logging=debug"), false, || {
        tracing::debug!("a routine beat")
    });
    assert!(detailed.contains("a routine beat"), "{detailed}");
}
