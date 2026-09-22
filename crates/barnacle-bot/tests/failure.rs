use std::path::PathBuf;
use std::time::Duration;

use barnacle_bot::attendance::BoardError;
use barnacle_bot::attendance_store::AttendanceError;
use barnacle_bot::discord::RunError;
use barnacle_bot::failure;
use barnacle_bot::failure::DiscordRefusal;
use barnacle_bot::failure::Failure;
use barnacle_bot::failure::Kind;
use barnacle_bot::failure::OptionProblem;
use barnacle_bot::failure::Reference;
use barnacle_bot::failure::Scope;
use barnacle_bot::ids::ChannelId;
use barnacle_bot::ids::GuildId;
use barnacle_bot::startup::StartupError;
use barnacle_guess::Snowflake;
use barnacle_guess::UserId;
use poise::serenity_prelude as serenity;
use serenity::PermissionOverwrite;
use serenity::PermissionOverwriteType;
use serenity::Permissions;

const CROCKFORD: &str = "0123456789ABCDEFGHJKMNPQRSTVWXYZ";

const KINDS: [Kind; 19] = [
    Kind::MissingPermissions,
    Kind::MissingAccess,
    Kind::UnknownChannel,
    Kind::UnknownMessage,
    Kind::UnknownMember,
    Kind::UnknownRole,
    Kind::UnknownGuild,
    Kind::UnknownInteraction,
    Kind::NotInVoice,
    Kind::ChannelLimit,
    Kind::InvalidRequest,
    Kind::RateLimited,
    Kind::DiscordUnavailable,
    Kind::DiscordRefused,
    Kind::Network,
    Kind::Database,
    Kind::Storage,
    Kind::Timeout,
    Kind::Internal,
];

fn refusal(status: u16, code: isize, message: &str) -> DiscordRefusal {
    DiscordRefusal {
        status,
        code,
        message: message.to_owned(),
        fields: Vec::new(),
    }
}

fn failure_of(kind: Kind) -> Failure {
    Failure {
        kind,
        reference: Reference::new(),
        message: "the step failed".to_owned(),
        discord: None,
        missing: Vec::new(),
    }
}

fn fixed(kind: Kind) -> Failure {
    Failure {
        reference: Reference("7K2M9QXA".to_owned()),
        ..failure_of(kind)
    }
}

fn is_reference(text: &str) -> bool {
    text.chars().count() == 8 && text.chars().all(|letter| CROCKFORD.contains(letter))
}

fn role(allow: Permissions, deny: Permissions) -> PermissionOverwrite {
    PermissionOverwrite {
        allow,
        deny,
        kind: PermissionOverwriteType::Role(serenity::RoleId::new(7)),
    }
}

#[test]
fn discord_refusals_are_classified_by_their_code() {
    let cases = [
        (
            refusal(403, 50013, "Missing Permissions"),
            Kind::MissingPermissions,
        ),
        (refusal(403, 50001, "Missing Access"), Kind::MissingAccess),
        (refusal(404, 10003, "Unknown Channel"), Kind::UnknownChannel),
        (refusal(404, 10008, "Unknown Message"), Kind::UnknownMessage),
        (refusal(404, 10007, "Unknown Member"), Kind::UnknownMember),
        (refusal(404, 10011, "Unknown Role"), Kind::UnknownRole),
        (refusal(404, 10004, "Unknown Guild"), Kind::UnknownGuild),
        (
            refusal(404, 10062, "Unknown interaction"),
            Kind::UnknownInteraction,
        ),
        (
            refusal(400, 40032, "Target user is not connected to voice."),
            Kind::NotInVoice,
        ),
        (
            refusal(400, 30013, "Maximum number of guild channels reached (500)"),
            Kind::ChannelLimit,
        ),
        (
            refusal(400, 50035, "Invalid Form Body"),
            Kind::InvalidRequest,
        ),
        (
            refusal(429, 0, "You are being rate limited."),
            Kind::RateLimited,
        ),
        (
            refusal(503, 0, "Service Unavailable"),
            Kind::DiscordUnavailable,
        ),
        (
            refusal(500, 0, "Internal Server Error"),
            Kind::DiscordUnavailable,
        ),
        (
            refusal(400, 12345, "Something unusual"),
            Kind::DiscordRefused,
        ),
        (
            refusal(429, 50013, "Missing Permissions"),
            Kind::MissingPermissions,
        ),
    ];
    for (refused, kind) in cases {
        let failure = Failure::from_error(&refused);
        assert_eq!(failure.kind, kind, "{refused}");
        assert_eq!(failure.discord, Some(refused));
        assert!(failure.missing.is_empty());
    }
}

#[test]
fn the_innermost_known_cause_decides_the_kind() {
    let database = AttendanceError::Database(sqlx::Error::RowNotFound);
    let failure = Failure::from_error(&database);
    assert_eq!(failure.kind, Kind::Database);
    assert_eq!(
        failure.message,
        format!(
            "the attendance database could not be used: {}",
            sqlx::Error::RowNotFound
        )
    );
    assert_eq!(failure.discord, None);

    let storage = StartupError::CurationIo {
        path: PathBuf::from("ships.toml"),
        source: std::io::Error::new(std::io::ErrorKind::NotFound, "no such file"),
    };
    let failure = Failure::from_error(&storage);
    assert_eq!(failure.kind, Kind::Storage);
    assert_eq!(
        failure.message,
        "ships.toml could not be read: no such file"
    );

    let refused = refusal(403, 50001, "Missing Access");
    let board = BoardError(Box::new(refused.clone()));
    let failure = Failure::from_error(&board);
    assert_eq!(failure.kind, Kind::MissingAccess);
    assert_eq!(
        failure.message,
        "the Discord call failed: Missing Access (Discord error 50001, HTTP 403)"
    );
    assert_eq!(failure.discord, Some(refused));

    let plain: Box<dyn std::error::Error + Send + Sync> = "the beat overran".into();
    let failure = Failure::from_error(plain.as_ref());
    assert_eq!(failure.kind, Kind::Internal);
    assert_eq!(failure.message, "the beat overran");
    assert_eq!(failure.discord, None);
}

#[test]
fn a_known_error_outranks_the_causes_inside_it() {
    let wrapped = AttendanceError::Database(sqlx::Error::Io(std::io::Error::other("disk gone")));
    assert_eq!(Failure::from_error(&wrapped).kind, Kind::Database);
}

#[test]
fn an_http_error_without_an_answer_from_discord_is_a_network_failure() {
    let run = RunError::Discord(serenity::Error::Http(
        serenity::HttpError::ApplicationIdMissing,
    ));
    let failure = Failure::from_error(&run);
    assert_eq!(failure.kind, Kind::Network);
    assert_eq!(failure.discord, None);
    assert_eq!(
        DiscordRefusal::from_serenity(&serenity::Error::Http(
            serenity::HttpError::ApplicationIdMissing
        )),
        None
    );
}

#[test]
fn a_serenity_error_outside_http_is_classified_by_what_it_carries() {
    let other = serenity::Error::Other("the shard stopped");
    assert_eq!(Failure::from_error(&other).kind, Kind::Internal);
    assert_eq!(DiscordRefusal::from_serenity(&other), None);
}

#[tokio::test]
async fn a_timeout_is_classified_as_a_timeout() {
    let elapsed = tokio::time::timeout(Duration::ZERO, std::future::pending::<()>())
        .await
        .unwrap_err();
    assert_eq!(Failure::from_error(&elapsed).kind, Kind::Timeout);
}

#[test]
fn every_explanation_ends_with_its_reference() {
    let named = Failure::missing_permissions(vec!["Manage Channels"]);
    for failure in KINDS.into_iter().map(failure_of).chain([named]) {
        let explanation = failure.explanation();
        let suffix = format!(" (Reference: {})", failure.reference);
        assert!(explanation.ends_with(&suffix), "{explanation}");
        assert!(is_reference(failure.reference.as_str()), "{explanation}");
        assert_eq!(failure.reference.to_string(), failure.reference.as_str());
        let message = failure::command_failed("voice hub create", &failure);
        assert!(
            message.starts_with("`/voice hub create` did not finish."),
            "{message}"
        );
        assert!(message.ends_with(&suffix), "{message}");
    }
}

#[test]
fn references_are_drawn_fresh_for_each_failure() {
    let references: Vec<Reference> = (0..20).map(|_| Reference::new()).collect();
    assert!(
        references
            .iter()
            .all(|reference| is_reference(reference.as_str()))
    );
    assert!(
        references
            .iter()
            .skip(1)
            .any(|reference| *reference != references[0])
    );
    assert_ne!(
        Failure::internal("one").reference,
        Failure::internal("two").reference
    );
}

#[test]
fn copy_blockers_name_what_barnacle_lacks() {
    let voice_manager = Permissions::VIEW_CHANNEL
        | Permissions::CONNECT
        | Permissions::MANAGE_CHANNELS
        | Permissions::MOVE_MEMBERS
        | Permissions::MANAGE_ROLES;
    assert!(
        failure::copy_blockers(
            Permissions::ADMINISTRATOR,
            &[role(
                Permissions::PRIORITY_SPEAKER | Permissions::MANAGE_ROLES,
                Permissions::STREAM
            )],
            Permissions::MANAGE_CHANNELS
        )
        .is_empty()
    );
    assert_eq!(
        failure::copy_blockers(
            voice_manager,
            &[role(Permissions::PRIORITY_SPEAKER, Permissions::empty())],
            Permissions::empty()
        ),
        vec!["Priority Speaker"]
    );
    assert_eq!(
        failure::copy_blockers(
            voice_manager,
            &[role(Permissions::empty(), Permissions::MANAGE_ROLES)],
            Permissions::empty()
        ),
        vec!["Administrator"]
    );
    assert_eq!(
        failure::copy_blockers(
            voice_manager,
            &[role(Permissions::CONNECT, Permissions::STREAM)],
            Permissions::empty()
        ),
        vec!["Stream"]
    );
    assert_eq!(
        failure::copy_blockers(
            Permissions::VIEW_CHANNEL,
            &[],
            Permissions::MANAGE_CHANNELS | Permissions::MOVE_MEMBERS
        ),
        vec!["Manage Channels", "Move Members"]
    );
    assert!(
        failure::copy_blockers(
            voice_manager,
            &[role(Permissions::CONNECT, Permissions::VIEW_CHANNEL)],
            Permissions::MANAGE_CHANNELS | Permissions::MOVE_MEMBERS
        )
        .is_empty()
    );
}

#[test]
fn explanations_say_why_and_what_to_do() {
    assert_eq!(
        Failure {
            missing: vec!["Manage Channels"],
            ..fixed(Kind::MissingPermissions)
        }
        .explanation(),
        "Barnacle is missing Manage Channels here. A server admin can grant it to Barnacle's role, then try again. (Reference: 7K2M9QXA)"
    );
    assert_eq!(
        Failure {
            missing: vec!["Manage Channels", "Move Members"],
            ..fixed(Kind::MissingPermissions)
        }
        .explanation(),
        "Barnacle is missing Manage Channels and Move Members here. A server admin can grant them to Barnacle's role, then try again. (Reference: 7K2M9QXA)"
    );
    assert_eq!(
        fixed(Kind::MissingPermissions).explanation(),
        "Barnacle is missing a permission it needs here. A server admin can check Barnacle's role and this channel's permissions, then try again. (Reference: 7K2M9QXA)"
    );
    assert_eq!(
        fixed(Kind::InvalidRequest).explanation(),
        "Discord rejected what Barnacle sent. Check the options you gave and try again. (Reference: 7K2M9QXA)"
    );
    assert_eq!(
        Failure {
            discord: Some(DiscordRefusal {
                fields: vec![
                    "name: Must be 100 or fewer in length.".to_owned(),
                    "user_limit: Must be 99 or fewer.".to_owned()
                ],
                ..refusal(400, 50035, "Invalid Form Body")
            }),
            ..fixed(Kind::InvalidRequest)
        }
        .explanation(),
        "Discord rejected what Barnacle sent (name: Must be 100 or fewer in length.; user_limit: Must be 99 or fewer.). Check the options you gave and try again. (Reference: 7K2M9QXA)"
    );
    assert_eq!(
        Failure {
            discord: Some(refusal(400, 12345, "Something unusual")),
            ..fixed(Kind::DiscordRefused)
        }
        .explanation(),
        "Discord refused the request: Something unusual. (Reference: 7K2M9QXA)"
    );
    assert_eq!(
        fixed(Kind::Database).explanation(),
        "Barnacle could not read or save its data. This is a problem on Barnacle's side; tell whoever runs Barnacle and quote the reference. (Reference: 7K2M9QXA)"
    );
    assert_eq!(
        fixed(Kind::UnknownInteraction).explanation(),
        "Discord stopped waiting for Barnacle's answer. Try again. (Reference: 7K2M9QXA)"
    );
    assert_eq!(
        fixed(Kind::ChannelLimit).explanation(),
        "This server has reached Discord's limit of 500 channels. A server admin can delete unused channels, then try again. (Reference: 7K2M9QXA)"
    );
}

#[test]
fn failed_names_what_failed_before_the_explanation() {
    assert_eq!(
        failure::failed(
            "The sign-up post was not published",
            &fixed(Kind::UnknownChannel)
        ),
        "The sign-up post was not published. The channel involved no longer exists. (Reference: 7K2M9QXA)"
    );
    assert_eq!(
        failure::command_failed("cb season start", &fixed(Kind::RateLimited)),
        "`/cb season start` did not finish. Discord is limiting how fast Barnacle can act. Try again in a minute. (Reference: 7K2M9QXA)"
    );
}

#[test]
fn join_names_lists_names_in_plain_english() {
    assert_eq!(failure::join_names(&[]), "");
    assert_eq!(failure::join_names(&["Connect"]), "Connect");
    assert_eq!(
        failure::join_names(&["Connect", "Move Members"]),
        "Connect and Move Members"
    );
    assert_eq!(
        failure::join_names(&["View Channel", "Connect", "Move Members"]),
        "View Channel, Connect and Move Members"
    );
}

#[test]
fn constructed_failures_carry_their_kind_and_message() {
    let missing = Failure::missing_permissions(vec!["Manage Channels", "Move Members"]);
    assert_eq!(missing.kind, Kind::MissingPermissions);
    assert_eq!(
        missing.message,
        "Barnacle lacks Manage Channels and Move Members"
    );
    assert_eq!(missing.missing, vec!["Manage Channels", "Move Members"]);
    assert_eq!(missing.discord, None);
    let internal = Failure::internal("the room sweep died");
    assert_eq!(internal.kind, Kind::Internal);
    assert_eq!(internal.message, "the room sweep died");
    assert!(internal.missing.is_empty());
}

#[test]
fn discord_refusals_read_as_errors() {
    let refused = refusal(403, 50013, "Missing Permissions");
    assert_eq!(
        refused.to_string(),
        "Missing Permissions (Discord error 50013, HTTP 403)"
    );
    let error: &dyn std::error::Error = &refused;
    assert!(error.source().is_none());
}

#[test]
fn error_types_are_the_registered_low_cardinality_values() {
    let types: Vec<&str> = KINDS.into_iter().map(Kind::as_str).collect();
    assert_eq!(
        types,
        vec![
            "discord.missing_permissions",
            "discord.missing_access",
            "discord.unknown_channel",
            "discord.unknown_message",
            "discord.unknown_member",
            "discord.unknown_role",
            "discord.unknown_guild",
            "discord.unknown_interaction",
            "discord.not_in_voice",
            "discord.channel_limit",
            "discord.invalid_request",
            "discord.rate_limited",
            "discord.unavailable",
            "discord.refused",
            "network",
            "database",
            "storage",
            "timeout",
            "internal",
        ]
    );
    let ours: Vec<Kind> = KINDS
        .into_iter()
        .filter(|kind| kind.on_barnacles_side())
        .collect();
    assert_eq!(ours, vec![Kind::Database, Kind::Storage, Kind::Internal]);
}

#[test]
fn scope_builders_set_one_field_each() {
    let scope = Scope::default()
        .guild(GuildId::new(1))
        .channel(ChannelId::new(2))
        .user(UserId::new(3))
        .interaction(4)
        .message(Snowflake::new(5))
        .command("voice hub create")
        .season(6)
        .night("2026-09-21")
        .hub(ChannelId::new(7))
        .room(ChannelId::new(8))
        .room_number(9)
        .round(10);
    assert_eq!(
        scope,
        Scope {
            guild: Some(GuildId::new(1)),
            channel: Some(ChannelId::new(2)),
            user: Some(UserId::new(3)),
            interaction: Some(4),
            message: Some(Snowflake::new(5)),
            command: Some("voice hub create".to_owned()),
            season: Some(6),
            night: Some("2026-09-21".to_owned()),
            hub: Some(ChannelId::new(7)),
            room: Some(ChannelId::new(8)),
            room_number: Some(9),
            round: Some(10),
        }
    );
    assert_eq!(Scope::default().room(ChannelId::new(8)).guild, None);
}

#[test]
fn a_component_scope_names_the_server_channel_member_interaction_and_message() {
    let component: serenity::ComponentInteraction = serde_json::from_value(serde_json::json!({
        "id": "1180000000000000010",
        "application_id": "1180000000000000020",
        "type": 3,
        "data": { "custom_id": "cb:signup", "component_type": 2 },
        "guild_id": "1180000000000000001",
        "channel_id": "1180000000000000002",
        "user": { "id": "1180000000000000003", "username": "someone", "discriminator": "0", "global_name": null, "avatar": null },
        "token": "token",
        "version": 1,
        "message": {
            "id": "1180000000000000011",
            "channel_id": "1180000000000000002",
            "author": { "id": "1180000000000000020", "username": "Barnacle", "discriminator": "0", "global_name": null, "avatar": null, "bot": true },
            "content": "",
            "timestamp": "2026-09-21T00:00:00+00:00",
            "edited_timestamp": null,
            "tts": false,
            "mention_everyone": false,
            "mentions": [],
            "mention_roles": [],
            "attachments": [],
            "embeds": [],
            "pinned": false,
            "type": 0
        },
        "app_permissions": "0",
        "locale": "en-US",
        "entitlements": [],
        "authorizing_integration_owners": {},
        "context": 0,
        "attachment_size_limit": 26214400
    })).unwrap();
    assert_eq!(
        Scope::of_component(&component),
        Scope::default()
            .guild(GuildId::new(1_180_000_000_000_000_001))
            .channel(ChannelId::new(1_180_000_000_000_000_002))
            .user(UserId::new(1_180_000_000_000_000_003))
            .interaction(1_180_000_000_000_000_010)
            .message(Snowflake::new(1_180_000_000_000_000_011))
    );
}

#[test]
fn an_option_discord_would_not_look_up_is_a_failure_with_a_remedy() {
    let problem = OptionProblem::of(&refusal(403, 50001, "Missing Access"));
    let OptionProblem::Failed(failure) = problem else {
        panic!("expected a failure, got {problem:?}");
    };
    assert_eq!(failure.kind, Kind::MissingAccess);
    let reply = failure::command_failed("voice hub create", &failure);
    assert!(
        reply.starts_with(
            "`/voice hub create` did not finish. Barnacle cannot see a channel it needs here."
        ),
        "{reply}"
    );
    assert!(reply.contains("View Channel"), "{reply}");
    assert!(
        reply.ends_with(&format!("(Reference: {})", failure.reference)),
        "{reply}"
    );
}

#[test]
fn an_option_that_cannot_be_parsed_stays_a_refusal_with_its_detail() {
    let error: Box<dyn std::error::Error + Send + Sync> = "`next week` is not a number".into();
    assert_eq!(
        OptionProblem::of(&*error),
        OptionProblem::Unreadable("`next week` is not a number".to_owned())
    );
    let wrong_type = serenity::Error::Model(serenity::ModelError::InvalidChannelType);
    assert!(matches!(
        OptionProblem::of(&wrong_type),
        OptionProblem::Unreadable(_)
    ));
}
