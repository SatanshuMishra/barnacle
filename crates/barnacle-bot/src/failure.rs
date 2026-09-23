use std::fmt;
use std::num::NonZeroU64;

use barnacle_guess::Snowflake;
use barnacle_guess::UserId;
use poise::serenity_prelude as serenity;
use rand::seq::IndexedRandom;

use crate::ids::ChannelId;
use crate::ids::GuildId;
use crate::ids::Ping;
use crate::wiring::PingReach;
use crate::wiring::PingVerdict;

const REFERENCE_ALPHABET: &[u8] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";
const REFERENCE_LENGTH: usize = 8;
const RATE_LIMITED: u16 = 429;
const SERVER_ERROR: u16 = 500;

const DISCORD_CODES: [(isize, Kind); 11] = [
    (50013, Kind::MissingPermissions),
    (50001, Kind::MissingAccess),
    (10003, Kind::UnknownChannel),
    (10008, Kind::UnknownMessage),
    (10007, Kind::UnknownMember),
    (10011, Kind::UnknownRole),
    (10004, Kind::UnknownGuild),
    (10062, Kind::UnknownInteraction),
    (40032, Kind::NotInVoice),
    (30013, Kind::ChannelLimit),
    (50035, Kind::InvalidRequest),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    MissingPermissions,
    MissingAccess,
    UnknownChannel,
    UnknownMessage,
    UnknownMember,
    UnknownRole,
    UnknownGuild,
    UnknownInteraction,
    NotInVoice,
    ChannelLimit,
    InvalidRequest,
    RateLimited,
    DiscordUnavailable,
    DiscordRefused,
    Network,
    Database,
    Storage,
    Timeout,
    Internal,
}

impl Kind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Kind::MissingPermissions => "discord.missing_permissions",
            Kind::MissingAccess => "discord.missing_access",
            Kind::UnknownChannel => "discord.unknown_channel",
            Kind::UnknownMessage => "discord.unknown_message",
            Kind::UnknownMember => "discord.unknown_member",
            Kind::UnknownRole => "discord.unknown_role",
            Kind::UnknownGuild => "discord.unknown_guild",
            Kind::UnknownInteraction => "discord.unknown_interaction",
            Kind::NotInVoice => "discord.not_in_voice",
            Kind::ChannelLimit => "discord.channel_limit",
            Kind::InvalidRequest => "discord.invalid_request",
            Kind::RateLimited => "discord.rate_limited",
            Kind::DiscordUnavailable => "discord.unavailable",
            Kind::DiscordRefused => "discord.refused",
            Kind::Network => "network",
            Kind::Database => "database",
            Kind::Storage => "storage",
            Kind::Timeout => "timeout",
            Kind::Internal => "internal",
        }
    }

    pub const fn on_barnacles_side(self) -> bool {
        matches!(self, Kind::Database | Kind::Storage | Kind::Internal)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscordRefusal {
    pub status: u16,
    pub code: isize,
    pub message: String,
    pub fields: Vec<String>,
}

impl DiscordRefusal {
    pub fn from_serenity(error: &serenity::Error) -> Option<DiscordRefusal> {
        match error {
            serenity::Error::Http(serenity::HttpError::UnsuccessfulRequest(response)) => {
                Some(DiscordRefusal {
                    status: response.status_code.as_u16(),
                    code: response.error.code,
                    message: response.error.message.clone(),
                    fields: response
                        .error
                        .errors
                        .iter()
                        .map(|field| format!("{}: {}", field.path, field.message))
                        .collect(),
                })
            }
            _ => None,
        }
    }

    fn kind(&self) -> Kind {
        DISCORD_CODES
            .iter()
            .find(|(code, _)| *code == self.code)
            .map(|(_, kind)| *kind)
            .unwrap_or(match self.status {
                RATE_LIMITED => Kind::RateLimited,
                status if status >= SERVER_ERROR => Kind::DiscordUnavailable,
                _ => Kind::DiscordRefused,
            })
    }
}

impl fmt::Display for DiscordRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} (Discord error {}, HTTP {})",
            self.message, self.code, self.status
        )
    }
}

impl std::error::Error for DiscordRefusal {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reference(pub String);

impl Reference {
    pub fn new() -> Self {
        let mut rng = rand::rng();
        Self(
            (0..REFERENCE_LENGTH)
                .filter_map(|_| REFERENCE_ALPHABET.choose(&mut rng))
                .map(|&letter| char::from(letter))
                .collect(),
        )
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for Reference {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for Reference {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Failure {
    pub kind: Kind,
    pub reference: Reference,
    pub message: String,
    pub discord: Option<DiscordRefusal>,
    pub missing: Vec<&'static str>,
}

impl Failure {
    pub fn from_error(error: &(dyn std::error::Error + 'static)) -> Failure {
        let chain = || std::iter::successors(Some(error), |cause| cause.source());
        let causes: Vec<String> = chain().map(|cause| cause.to_string()).collect();
        let message = causes
            .iter()
            .zip(std::iter::once(None).chain(causes.iter().map(Some)))
            .filter(|(cause, above)| *above != Some(*cause))
            .map(|(cause, _)| cause.as_str())
            .collect::<Vec<_>>()
            .join(": ");
        let (kind, discord) = chain().find_map(classify).unwrap_or((Kind::Internal, None));
        Failure {
            kind,
            reference: Reference::new(),
            message,
            discord,
            missing: Vec::new(),
        }
    }

    pub fn internal(message: impl Into<String>) -> Failure {
        Failure {
            kind: Kind::Internal,
            reference: Reference::new(),
            message: message.into(),
            discord: None,
            missing: Vec::new(),
        }
    }

    pub fn missing_permissions(names: Vec<&'static str>) -> Failure {
        Failure {
            kind: Kind::MissingPermissions,
            reference: Reference::new(),
            message: format!("Barnacle lacks {}", join_names(&names)),
            discord: None,
            missing: names,
        }
    }

    pub fn explanation(&self) -> String {
        format!("{} (Reference: {})", self.advice(), self.reference)
    }

    fn advice(&self) -> String {
        match self.kind {
            Kind::MissingPermissions if self.missing.is_empty() => "Barnacle is missing a permission it needs here. A server admin can check Barnacle's role and this channel's permissions, then try again.".to_owned(),
            Kind::MissingPermissions => format!(
                "Barnacle is missing {} here. A server admin can grant {} to Barnacle's role, then try again.",
                join_names(&self.missing),
                if self.missing.len() == 1 { "it" } else { "them" }
            ),
            Kind::MissingAccess => "Barnacle cannot see a channel it needs here. A server admin can give Barnacle's role View Channel on it, then try again.".to_owned(),
            Kind::UnknownChannel => "The channel involved no longer exists.".to_owned(),
            Kind::UnknownMessage => "The message involved was deleted.".to_owned(),
            Kind::UnknownMember => "The member involved has left the server.".to_owned(),
            Kind::UnknownRole => "The role involved no longer exists.".to_owned(),
            Kind::UnknownGuild => "Barnacle is no longer in this server.".to_owned(),
            Kind::UnknownInteraction => "Discord stopped waiting for Barnacle's answer. Try again.".to_owned(),
            Kind::NotInVoice => "The member left voice before Barnacle could move them.".to_owned(),
            Kind::ChannelLimit => "This server has reached Discord's limit of 500 channels. A server admin can delete unused channels, then try again.".to_owned(),
            Kind::InvalidRequest => format!(
                "Discord rejected what Barnacle sent{}. Check the options you gave and try again.",
                self.detail()
            ),
            Kind::RateLimited => "Discord is limiting how fast Barnacle can act. Try again in a minute.".to_owned(),
            Kind::DiscordUnavailable => "Discord is having problems right now. Try again in a few minutes.".to_owned(),
            Kind::DiscordRefused => format!(
                "Discord refused the request: {}.",
                self.discord
                    .as_ref()
                    .map_or(self.message.as_str(), |refusal| refusal.message.as_str())
            ),
            Kind::Network => "Barnacle could not reach Discord. Try again in a few minutes.".to_owned(),
            Kind::Database => "Barnacle could not read or save its data. This is a problem on Barnacle's side; tell whoever runs Barnacle and quote the reference.".to_owned(),
            Kind::Storage => "Barnacle could not read one of its files. This is a problem on Barnacle's side; tell whoever runs Barnacle and quote the reference.".to_owned(),
            Kind::Timeout => "Discord took too long to answer. Try again in a minute.".to_owned(),
            Kind::Internal => "Something went wrong inside Barnacle. Tell whoever runs Barnacle and quote the reference.".to_owned(),
        }
    }

    fn detail(&self) -> String {
        let fields = self
            .discord
            .as_ref()
            .map(|refusal| refusal.fields.join("; "))
            .unwrap_or_default();
        if fields.is_empty() {
            String::new()
        } else {
            format!(" ({fields})")
        }
    }
}

fn classify(error: &(dyn std::error::Error + 'static)) -> Option<(Kind, Option<DiscordRefusal>)> {
    if let Some(refusal) = error.downcast_ref::<DiscordRefusal>() {
        return Some((refusal.kind(), Some(refusal.clone())));
    }
    if let Some(error) = error.downcast_ref::<serenity::Error>() {
        return match DiscordRefusal::from_serenity(error) {
            Some(refusal) => Some((refusal.kind(), Some(refusal))),
            None if matches!(error, serenity::Error::Http(_)) => Some((Kind::Network, None)),
            None => None,
        };
    }
    if error.is::<sqlx::Error>() {
        return Some((Kind::Database, None));
    }
    if error.is::<std::io::Error>() {
        return Some((Kind::Storage, None));
    }
    if error.is::<tokio::time::error::Elapsed>() {
        return Some((Kind::Timeout, None));
    }
    None
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptionProblem {
    Unreadable(String),
    Failed(Failure),
}

impl OptionProblem {
    pub fn of(error: &(dyn std::error::Error + 'static)) -> OptionProblem {
        let failure = Failure::from_error(error);
        if failure.kind == Kind::Internal {
            OptionProblem::Unreadable(failure.message)
        } else {
            OptionProblem::Failed(failure)
        }
    }
}

pub fn failed(what: &str, failure: &Failure) -> String {
    format!("{what}. {}", failure.explanation())
}

pub fn command_failed(command: &str, failure: &Failure) -> String {
    failed(&format!("`/{command}` did not finish"), failure)
}

pub fn join_names(names: &[&str]) -> String {
    match names {
        [] => String::new(),
        [only] => (*only).to_owned(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    }
}

pub fn barnacle_permissions(
    cache: &serenity::Cache,
    guild: GuildId,
) -> Option<serenity::Permissions> {
    let me = cache.current_user().id;
    let guild = cache.guild(NonZeroU64::new(guild.get())?)?;
    let member = guild.members.get(&me)?;
    Some(guild.member_permissions(member))
}

pub fn barnacle_permissions_in(
    cache: &serenity::Cache,
    guild: GuildId,
    channel: ChannelId,
) -> Option<serenity::Permissions> {
    let me = cache.current_user().id;
    let guild = cache.guild(NonZeroU64::new(guild.get())?)?;
    let channel = guild
        .channels
        .get(&serenity::ChannelId::from(NonZeroU64::new(channel.get())?))?;
    let member = guild.members.get(&me)?;
    Some(guild.user_permissions_in(channel, member))
}

pub fn copy_blockers(
    granted: serenity::Permissions,
    overwrites: &[serenity::PermissionOverwrite],
    extra: serenity::Permissions,
) -> Vec<&'static str> {
    if granted.administrator() {
        return Vec::new();
    }
    let copied = overwrites
        .iter()
        .fold(serenity::Permissions::empty(), |copied, overwrite| {
            copied | overwrite.allow | overwrite.deny
        });
    let administrator = if copied.manage_roles() {
        serenity::Permissions::ADMINISTRATOR
    } else {
        serenity::Permissions::empty()
    };
    let needed = extra | (copied - serenity::Permissions::MANAGE_ROLES) | administrator;
    (needed - granted).get_permission_names()
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Scope {
    pub guild: Option<GuildId>,
    pub channel: Option<ChannelId>,
    pub user: Option<UserId>,
    pub interaction: Option<u64>,
    pub message: Option<Snowflake>,
    pub command: Option<String>,
    pub season: Option<i64>,
    pub night: Option<String>,
    pub hub: Option<ChannelId>,
    pub room: Option<ChannelId>,
    pub room_number: Option<u32>,
    pub round: Option<u32>,
}

impl Scope {
    pub fn of_command(ctx: crate::discord::Context<'_>) -> Scope {
        let interaction = match ctx {
            poise::Context::Application(application) => Some(application.interaction.id.get()),
            poise::Context::Prefix(_) => None,
        };
        Scope {
            guild: ctx.guild_id().map(|guild| GuildId::new(guild.get())),
            channel: Some(ChannelId::new(ctx.channel_id().get())),
            user: Some(UserId::new(ctx.author().id.get())),
            interaction,
            command: Some(ctx.command().qualified_name.to_string()),
            ..Scope::default()
        }
    }

    pub fn of_component(component: &serenity::ComponentInteraction) -> Scope {
        Scope {
            guild: component.guild_id.map(|guild| GuildId::new(guild.get())),
            channel: Some(ChannelId::new(component.channel_id.get())),
            user: Some(UserId::new(component.user.id.get())),
            interaction: Some(component.id.get()),
            message: Some(Snowflake::new(component.message.id.get())),
            ..Scope::default()
        }
    }

    pub fn guild(self, guild: GuildId) -> Scope {
        Scope {
            guild: Some(guild),
            ..self
        }
    }

    pub fn channel(self, channel: ChannelId) -> Scope {
        Scope {
            channel: Some(channel),
            ..self
        }
    }

    pub fn user(self, user: UserId) -> Scope {
        Scope {
            user: Some(user),
            ..self
        }
    }

    pub fn interaction(self, interaction: u64) -> Scope {
        Scope {
            interaction: Some(interaction),
            ..self
        }
    }

    pub fn message(self, message: Snowflake) -> Scope {
        Scope {
            message: Some(message),
            ..self
        }
    }

    pub fn command(self, command: impl Into<String>) -> Scope {
        Scope {
            command: Some(command.into()),
            ..self
        }
    }

    pub fn season(self, season: i64) -> Scope {
        Scope {
            season: Some(season),
            ..self
        }
    }

    pub fn night(self, night: impl Into<String>) -> Scope {
        Scope {
            night: Some(night.into()),
            ..self
        }
    }

    pub fn hub(self, hub: ChannelId) -> Scope {
        Scope {
            hub: Some(hub),
            ..self
        }
    }

    pub fn room(self, room: ChannelId) -> Scope {
        Scope {
            room: Some(room),
            ..self
        }
    }

    pub fn room_number(self, room_number: u32) -> Scope {
        Scope {
            room_number: Some(room_number),
            ..self
        }
    }

    pub fn round(self, round: u32) -> Scope {
        Scope {
            round: Some(round),
            ..self
        }
    }
}

macro_rules! scoped_event {
    ($level:expr, $event:expr, $outcome:expr, $summary:expr, $scope:expr, $($fields:tt)*) => {
        tracing::event!(
            $level,
            "event.name" = $event,
            "event.outcome" = $outcome,
            $($fields)*
            "discord.guild.id" = $scope.guild.map(|guild| tracing::field::display(guild.get())),
            "discord.channel.id" = $scope.channel.map(|channel| tracing::field::display(channel.get())),
            "discord.user.id" = $scope.user.map(|user| tracing::field::display(user.get())),
            "discord.interaction.id" = $scope.interaction.map(tracing::field::display),
            "discord.message.id" = $scope.message.map(|message| tracing::field::display(message.get())),
            "discord.command.name" = $scope.command.as_deref(),
            "barnacle.season.id" = $scope.season.map(tracing::field::display),
            "barnacle.night" = $scope.night.as_deref(),
            "barnacle.hub.id" = $scope.hub.map(|hub| tracing::field::display(hub.get())),
            "barnacle.room.id" = $scope.room.map(|room| tracing::field::display(room.get())),
            "barnacle.room.number" = $scope.room_number,
            "barnacle.round.number" = $scope.round,
            "{}",
            $summary
        )
    };
}

macro_rules! failure_event {
    ($level:expr, $event:expr, $summary:expr, $failure:expr, $scope:expr) => {
        scoped_event!(
            $level,
            $event,
            "failure",
            $summary,
            $scope,
            "error.type" = $failure.kind.as_str(),
            "exception.message" = $failure.message.as_str(),
            "barnacle.reference" = $failure.reference.as_str(),
            "http.response.status_code" = $failure.discord.as_ref().map(|refusal| refusal.status),
            "discord.error.code" = $failure.discord.as_ref().map(|refusal| refusal.code),
            "discord.error.message" = $failure
                .discord
                .as_ref()
                .map(|refusal| refusal.message.as_str()),
            "barnacle.permissions.missing" =
                (!$failure.missing.is_empty()).then(|| $failure.missing.join(", ")),
        )
    };
}

pub fn report(event: &'static str, summary: &str, failure: &Failure, scope: &Scope) {
    if failure.kind.on_barnacles_side() {
        failure_event!(tracing::Level::ERROR, event, summary, failure, scope);
    } else {
        failure_event!(tracing::Level::WARN, event, summary, failure, scope);
    }
}

pub fn refused(event: &'static str, summary: &str, reason: &'static str, scope: &Scope) {
    scoped_event!(
        tracing::Level::INFO,
        event,
        "refused",
        summary,
        scope,
        "barnacle.refusal.reason" = reason,
    );
}

pub fn refused_because(
    event: &'static str,
    summary: &str,
    reason: &'static str,
    detail: &str,
    scope: &Scope,
) {
    scoped_event!(
        tracing::Level::INFO,
        event,
        "refused",
        summary,
        scope,
        "barnacle.refusal.reason" = reason,
        "exception.message" = detail,
    );
}

pub fn record(event: &'static str, summary: &str, scope: &Scope) {
    scoped_event!(tracing::Level::INFO, event, "success", summary, scope,);
}

macro_rules! ping_checked_event {
    ($level:expr, $outcome:expr, $ping:expr, $verdict:expr, $reach:expr, $scope:expr) => {
        scoped_event!(
            $level,
            "signup.ping.checked",
            $outcome,
            "a season's ping was checked",
            $scope,
            "barnacle.ping" = $ping.as_str(),
            "barnacle.ping.verdict" = $verdict,
            "barnacle.ping.mentionable" = $reach.map(|reach| reach.role_mentionable),
            "barnacle.ping.server_grant" = $reach.map(|reach| reach.server_grant),
            "barnacle.ping.channel_grant" = $reach.map(|reach| reach.channel_grant),
        )
    };
}

pub fn ping_checked(
    ping: Ping,
    verdict: PingVerdict,
    reach: Option<PingReach>,
    refused: bool,
    scope: &Scope,
) {
    let named = ping_name(ping);
    let judged = verdict_name(verdict);
    match (verdict, refused) {
        (PingVerdict::Sounds, _) => {
            ping_checked_event!(tracing::Level::INFO, "success", named, judged, reach, scope)
        }
        (_, true) => {
            ping_checked_event!(tracing::Level::WARN, "refused", named, judged, reach, scope)
        }
        (_, false) => {
            ping_checked_event!(tracing::Level::WARN, "failure", named, judged, reach, scope)
        }
    }
}

pub fn ping_published(summary: &str, ping: Ping, heard: bool, scope: &Scope) {
    scoped_event!(
        tracing::Level::INFO,
        "signup.post.published",
        "success",
        summary,
        scope,
        "barnacle.ping" = ping_name(ping).as_str(),
        "barnacle.ping.heard" = heard,
    );
}

pub fn click_recorded(target: &str, choice: &str, scope: &Scope) {
    scoped_event!(
        tracing::Level::INFO,
        "signup.click.recorded",
        "success",
        "a sign-up click was recorded",
        scope,
        "barnacle.signup.target" = target,
        "barnacle.signup.choice" = choice,
    );
}

pub fn ping_silent(ping: Ping, scope: &Scope) {
    scoped_event!(
        tracing::Level::WARN,
        "signup.ping.silent",
        "failure",
        "Discord did not register a sign-up post's ping, so nobody was notified",
        scope,
        "barnacle.ping" = ping_name(ping).as_str(),
    );
}

fn ping_name(ping: Ping) -> String {
    match ping {
        Ping::Everyone => "everyone".to_owned(),
        Ping::Role(role) => role.to_string(),
    }
}

fn verdict_name(verdict: PingVerdict) -> &'static str {
    match verdict {
        PingVerdict::Sounds => "sounds",
        PingVerdict::BlockedByChannel => "blocked_by_channel",
        PingVerdict::Silent => "silent",
        PingVerdict::Unchecked => "unchecked",
    }
}
