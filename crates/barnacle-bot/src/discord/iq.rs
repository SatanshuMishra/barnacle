use std::collections::BTreeMap;
use std::future::Future;
use std::num::NonZeroU64;
use std::time::Duration;

use barnacle_guess::Snowflake;
use barnacle_guess::UserId;
use jiff::ToSpan;
use jiff::civil::Date;
use poise::serenity_prelude as serenity;

use super::Context;
use super::Data;
use super::Error;
use super::commands;
use super::events::Answer;
use super::events::acknowledge;
use super::events::tell;
use crate::event_draft;
use crate::failure;
use crate::failure::Failure;
use crate::failure::Kind;
use crate::failure::Scope;
use crate::ids::ChannelId;
use crate::ids::GuildId;
use crate::ids::RoleId;
use crate::iq;
use crate::iq::CheckPath;
use crate::iq::Checked;
use crate::iq::CleanupRefusal;
use crate::iq::CompleteRefusal;
use crate::iq::Criterion;
use crate::iq::Done;
use crate::iq::Judged;
use crate::iq::Standing;
use crate::iq::StartChoice;
use crate::iq::Status;
use crate::iq_setup;
use crate::iq_setup::CompetitionChannels;
use crate::iq_setup::ConfigChange;
use crate::iq_setup::Purpose;
use crate::iq_setup::SetupChecks;
use crate::iq_setup::SetupDraft;
use crate::iq_setup::SetupPlan;
use crate::iq_setup::SetupReady;
use crate::iq_setup::SetupRefusal;
use crate::iq_setup::SetupStep;
use crate::iq_store::AwardOutcome;
use crate::iq_store::Competition;
use crate::iq_store::CreateOutcome;
use crate::iq_store::IqStoreError;
use crate::iq_text;
use crate::schedule;
use crate::schedule::Zone;
use crate::text::Refusal;
use crate::text::escape;
use crate::wiring::IqPanelPart;
use crate::wiring::iq_panel_id;
use crate::wiring::same_name;

const JUDGE_ROLE_NAME: &str = "IQ-Judge";
const ANNOUNCEMENTS_NAME: &str = "iq-announcements";
const SUBMISSIONS_NAME: &str = "iq-submissions";
const PAUSE_LABEL: &str = "Pause competition";
const RESUME_LABEL: &str = "Resume";
const POST_RESULT_LABEL: &str = "Post the result";
const DELETE_CHANNELS_LABEL: &str = "Delete channels";
const SAVE_JUDGES_LABEL: &str = "Save judges";
const NOTHING_CREATED: &str = "Cancelled. Nothing was created.";
const ZERO_POINTS: &str = "Give or take at least 1 point. 0 changes nothing.";
const MANAGE_SERVER_NEEDED: &str = "You need Manage Server to use this button.";
const LOG_FAILED: &str =
    "The points were saved, but the note in the submissions channel was not posted";
const ANNOUNCED_LINES: usize = 5;
const MENU_MAX: u8 = 25;
const PAGE_SIZE: u8 = 100;
const CATCH_UP_PAGES: usize = 500;
const SHOWN_END_OFFSET: i64 = 60;
const MILLIS_PER_SECOND: u64 = 1_000;
const VARIATION_SELECTOR: char = '\u{FE0F}';
const DISCORD_WAIT: Duration = Duration::from_secs(30);
const SETUP_ACCESS: serenity::Permissions =
    serenity::Permissions::MANAGE_CHANNELS.union(serenity::Permissions::MANAGE_ROLES);
const QUIET_FOR_PLAYERS: serenity::Permissions = serenity::Permissions::SEND_MESSAGES
    .union(serenity::Permissions::ADD_REACTIONS)
    .union(serenity::Permissions::CREATE_PUBLIC_THREADS)
    .union(serenity::Permissions::CREATE_PRIVATE_THREADS)
    .union(serenity::Permissions::SEND_MESSAGES_IN_THREADS);
const ANNOUNCING_BOT: serenity::Permissions =
    serenity::Permissions::SEND_MESSAGES.union(serenity::Permissions::MENTION_EVERYONE);
const POSTING_PLAYERS: serenity::Permissions = serenity::Permissions::SEND_MESSAGES
    .union(serenity::Permissions::ATTACH_FILES)
    .union(serenity::Permissions::ADD_REACTIONS)
    .union(serenity::Permissions::READ_MESSAGE_HISTORY);
const CHECKING_BOT: serenity::Permissions = serenity::Permissions::ADD_REACTIONS
    .union(serenity::Permissions::READ_MESSAGE_HISTORY)
    .union(serenity::Permissions::SEND_MESSAGES);

#[derive(Debug)]
struct Unfinished {
    done: Vec<Done>,
    error: IqError,
}

impl Unfinished {
    fn untouched(error: impl Into<IqError>) -> Self {
        Self {
            done: Vec::new(),
            error: error.into(),
        }
    }
}

#[derive(Debug, Clone, Default)]
struct Progress {
    done: Vec<Done>,
}

impl Progress {
    fn with(self, step: Done) -> Self {
        Self {
            done: self.done.into_iter().chain([step]).collect(),
        }
    }

    fn stopped(&self, error: impl Into<IqError>) -> Unfinished {
        Unfinished {
            done: self.done.clone(),
            error: error.into(),
        }
    }
}

#[derive(Debug)]
enum IqError {
    Discord(Box<serenity::Error>),
    TimedOut(tokio::time::error::Elapsed),
    Store(Box<IqStoreError>),
    UnknownZone(String),
    ZeroId,
    Clock,
}

impl std::fmt::Display for IqError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IqError::Discord(_) => formatter.write_str("Discord refused a request"),
            IqError::TimedOut(_) => formatter.write_str("Discord did not answer in time"),
            IqError::Store(_) => {
                formatter.write_str("the IQ Competition records could not be used")
            }
            IqError::UnknownZone(zone) => write!(
                formatter,
                "the stored time zone {zone} is not in the zone database"
            ),
            IqError::ZeroId => formatter.write_str("a stored Discord ID is 0"),
            IqError::Clock => formatter.write_str("the clock reads before 1970"),
        }
    }
}

impl std::error::Error for IqError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            IqError::Discord(error) => Some(error.as_ref()),
            IqError::TimedOut(error) => Some(error),
            IqError::Store(error) => Some(error.as_ref()),
            IqError::UnknownZone(_) | IqError::ZeroId | IqError::Clock => None,
        }
    }
}

impl From<serenity::Error> for IqError {
    fn from(error: serenity::Error) -> Self {
        IqError::Discord(Box::new(error))
    }
}

impl From<tokio::time::error::Elapsed> for IqError {
    fn from(error: tokio::time::error::Elapsed) -> Self {
        IqError::TimedOut(error)
    }
}

impl From<IqStoreError> for IqError {
    fn from(error: IqStoreError) -> Self {
        IqError::Store(Box::new(error))
    }
}

enum Turned {
    Panel(Refusal),
    Setup(SetupRefusal),
}

impl From<Refusal> for Turned {
    fn from(refusal: Refusal) -> Self {
        Turned::Panel(refusal)
    }
}

impl From<SetupRefusal> for Turned {
    fn from(refusal: SetupRefusal) -> Self {
        Turned::Setup(refusal)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Room {
    Category,
    Announcements,
    Submissions,
}

#[derive(Debug, Clone, Copy)]
struct Seats {
    guild: serenity::GuildId,
    judge: serenity::RoleId,
    bot: serenity::UserId,
}

#[derive(Debug, Default)]
struct Made {
    role: Option<serenity::RoleId>,
    channels: Vec<serenity::ChannelId>,
    granted: Vec<(serenity::UserId, serenity::RoleId)>,
}

struct Built {
    role: serenity::RoleId,
    channels: CompetitionChannels,
    messages: Vec<u64>,
    end_unix: i64,
}

#[poise::command(slash_command, guild_only)]
pub(super) async fn iq(_ctx: Context<'_>) -> Result<(), Error> {
    Ok(())
}

#[poise::command(
    slash_command,
    rename = "start",
    guild_only,
    required_permissions = "MANAGE_GUILD"
)]
pub(super) async fn iq_start(ctx: Context<'_>) -> Result<(), Error> {
    let Some(guild) = guild_of(ctx) else {
        return Ok(());
    };
    let live = ctx.data().iq.live(guild).await?;
    let choice = iq::start_choice(live.as_ref().map(|competition| competition.status));
    match (choice, live) {
        (StartChoice::Resume, Some(competition)) => {
            let question = iq_text::resume_question(&competition.name, end_unix(&competition)?);
            confirm(
                ctx,
                question,
                IqPanelPart::ConfirmResume,
                RESUME_LABEL,
                serenity::ButtonStyle::Primary,
            )
            .await
        }
        (StartChoice::AlreadyRunning, Some(competition)) => {
            refuse(
                ctx,
                "already_running",
                iq_text::already_running(&competition.name),
            )
            .await
        }
        (StartChoice::NeedsCleanup, Some(competition)) => {
            refuse(
                ctx,
                "needs_cleanup",
                iq_text::needs_cleanup(&competition.name),
            )
            .await
        }
        (
            StartChoice::Setup
            | StartChoice::Resume
            | StartChoice::AlreadyRunning
            | StartChoice::NeedsCleanup,
            _,
        ) => open_setup(ctx, guild).await,
    }
}

#[poise::command(
    slash_command,
    rename = "edit",
    guild_only,
    required_permissions = "MANAGE_GUILD"
)]
pub(super) async fn iq_edit(ctx: Context<'_>) -> Result<(), Error> {
    let Some(guild) = guild_of(ctx) else {
        return Ok(());
    };
    let Some(competition) = ctx.data().iq.live(guild).await? else {
        return refuse(ctx, "no_competition", iq_text::NO_COMPETITION_RUNNING).await;
    };
    match competition.status {
        Status::Running | Status::Stopped => {
            let draft = SetupDraft::edit(&competition, author(ctx));
            open_draft(ctx, guild, draft).await
        }
        Status::Completed => {
            refuse(
                ctx,
                "needs_cleanup",
                iq_text::needs_cleanup(&competition.name),
            )
            .await
        }
        Status::Archived => refuse(ctx, "no_competition", iq_text::NO_COMPETITION_RUNNING).await,
    }
}

#[poise::command(
    slash_command,
    rename = "stop",
    guild_only,
    required_permissions = "MANAGE_GUILD"
)]
pub(super) async fn iq_stop(ctx: Context<'_>) -> Result<(), Error> {
    let Some(guild) = guild_of(ctx) else {
        return Ok(());
    };
    let Some(competition) = ctx.data().iq.live(guild).await? else {
        return refuse(ctx, "no_competition", iq_text::NO_COMPETITION_RUNNING).await;
    };
    match competition.status {
        Status::Running => {
            confirm(
                ctx,
                iq_text::pause_question(&competition.name, competition.submissions),
                IqPanelPart::ConfirmStop,
                PAUSE_LABEL,
                serenity::ButtonStyle::Primary,
            )
            .await
        }
        Status::Completed => {
            refuse(
                ctx,
                "needs_cleanup",
                iq_text::needs_cleanup(&competition.name),
            )
            .await
        }
        Status::Stopped | Status::Archived => {
            refuse(ctx, "not_running", iq_text::NO_COMPETITION_RUNNING).await
        }
    }
}

#[poise::command(
    slash_command,
    rename = "complete",
    guild_only,
    required_permissions = "MANAGE_GUILD"
)]
pub(super) async fn iq_complete(ctx: Context<'_>) -> Result<(), Error> {
    let Some(guild) = guild_of(ctx) else {
        return Ok(());
    };
    let Some(competition) = ctx.data().iq.live(guild).await? else {
        return refuse(ctx, "no_competition", iq_text::NO_COMPETITION_RUNNING).await;
    };
    let end = end_unix(&competition)?;
    match iq::may_complete(competition.status, end, super::now_unix()) {
        Err(CompleteRefusal::TooEarly) => {
            refuse(
                ctx,
                "too_early",
                iq_text::complete_too_early(&competition.name, end),
            )
            .await
        }
        Err(CompleteRefusal::NotLive) => {
            refuse(
                ctx,
                "needs_cleanup",
                iq_text::needs_cleanup(&competition.name),
            )
            .await
        }
        Ok(()) => {
            ctx.defer_ephemeral().await?;
            let standings = iq::standings(&ctx.data().iq.scores(competition.id).await?);
            let (named, points) =
                named_winners(&ctx.serenity_context().http, guild, &standings).await;
            let winner_names: Vec<&str> = named.iter().map(String::as_str).collect();
            let question = iq_text::complete_question(
                &competition.name,
                end,
                competition.announcements,
                &competition.roles,
                &winner_names,
                points,
            );
            confirm(
                ctx,
                question,
                IqPanelPart::ConfirmComplete,
                POST_RESULT_LABEL,
                serenity::ButtonStyle::Success,
            )
            .await
        }
    }
}

#[poise::command(
    slash_command,
    rename = "cleanup",
    guild_only,
    required_permissions = "MANAGE_GUILD"
)]
pub(super) async fn iq_cleanup(ctx: Context<'_>) -> Result<(), Error> {
    let Some(guild) = guild_of(ctx) else {
        return Ok(());
    };
    let Some(competition) = ctx.data().iq.live(guild).await? else {
        return refuse(ctx, "no_competition", iq_text::NO_COMPETITION_RUNNING).await;
    };
    match iq::may_clean_up(competition.status) {
        Err(CleanupRefusal::Running) => {
            refuse(
                ctx,
                "still_running",
                iq_text::cleanup_while_running(&competition.name),
            )
            .await
        }
        Ok(()) => {
            confirm(
                ctx,
                iq_text::cleanup_question(&competition.name),
                IqPanelPart::ConfirmCleanup,
                DELETE_CHANNELS_LABEL,
                serenity::ButtonStyle::Danger,
            )
            .await
        }
    }
}

#[poise::command(
    slash_command,
    rename = "judges",
    guild_only,
    required_permissions = "MANAGE_GUILD"
)]
pub(super) async fn iq_judges(ctx: Context<'_>) -> Result<(), Error> {
    let Some(guild) = guild_of(ctx) else {
        return Ok(());
    };
    let Some(competition) = ctx.data().iq.live(guild).await? else {
        return refuse(ctx, "no_competition", iq_text::NO_COMPETITION_RUNNING).await;
    };
    let Some(now_ms) = super::now_ms() else {
        return commands::fail(ctx, Failure::from_error(&IqError::Clock)).await;
    };
    let panel = ctx.id();
    let reply = commands::private_reply(iq_text::judges_prompt(&competition.name))
        .components(judges_rows(panel, &competition.judges));
    ctx.data()
        .iq_drafts
        .open(panel, SetupDraft::edit(&competition, author(ctx)), now_ms);
    ctx.send(reply).await?;
    Ok(())
}

#[poise::command(slash_command, rename = "points", guild_only)]
pub(super) async fn iq_points(
    ctx: Context<'_>,
    #[description = "The member whose points change"] member: serenity::User,
    #[description = "Points to give, or a negative number to take"]
    #[min = -1000]
    #[max = 1000]
    points: i64,
    #[description = "Why the points change"]
    #[max_length = 200]
    reason: Option<String>,
) -> Result<(), Error> {
    let Some(guild) = guild_of(ctx) else {
        return Ok(());
    };
    let data = ctx.data();
    let judge_role =
        current_judge_role(data, ctx.cache(), guild, &Scope::default().guild(guild)).await;
    let holds = match (judge_role, ctx.author_member().await) {
        (Some(role), Some(held)) => held.roles.iter().any(|id| id.get() == role.get()),
        _ => false,
    };
    if !holds {
        return refuse(ctx, "not_a_judge", iq_text::ONLY_JUDGES).await;
    }
    let Some(points) = iq::adjustment(points) else {
        return refuse(ctx, "zero_points", ZERO_POINTS).await;
    };
    let competition = match data.iq.live(guild).await? {
        Some(competition) if matches!(competition.status, Status::Running | Status::Stopped) => {
            competition
        }
        _ => return refuse(ctx, "no_competition", iq_text::NO_COMPETITION_RUNNING).await,
    };
    let Some(now_ms) = super::now_ms().and_then(|now_ms| i64::try_from(now_ms).ok()) else {
        return commands::fail(ctx, Failure::from_error(&IqError::Clock)).await;
    };
    let reason = reason
        .as_deref()
        .map(str::trim)
        .filter(|reason| !reason.is_empty());
    let target = UserId::new(member.id.get());
    let judge = author(ctx);
    data.iq
        .adjust(competition.id, target, points, reason, judge, now_ms)
        .await?;
    let scope = Scope::of_command(ctx);
    failure::record(
        "iq.points.adjusted",
        "a judge changed a member's points",
        &scope,
    );
    let total = data
        .iq
        .scores(competition.id)
        .await?
        .into_iter()
        .find(|score| score.user == target)
        .map_or(0, |score| score.points);
    let logged = post(
        &ctx.serenity_context().http,
        competition.submissions,
        &iq_text::points_log(judge, target, points, reason),
        serenity::CreateAllowedMentions::new(),
    )
    .await;
    let content = match logged {
        Ok(_) => {
            iq_text::points_confirmation(target, points, total, reason, competition.submissions)
        }
        Err(error) => {
            let failure = Failure::from_error(&error);
            failure::report(
                "iq.points.log_failed",
                "a points change could not be logged in the submissions channel",
                &failure,
                &scope,
            );
            failure::failed(LOG_FAILED, &failure)
        }
    };
    commands::private(ctx, content).await
}

#[poise::command(slash_command, rename = "leaderboard", guild_only)]
pub(super) async fn iq_leaderboard(ctx: Context<'_>) -> Result<(), Error> {
    let Some(guild) = guild_of(ctx) else {
        return Ok(());
    };
    ctx.defer().await?;
    let data = ctx.data();
    let http = &ctx.serenity_context().http;
    let requester = author(ctx);
    let content = match data.iq.live(guild).await? {
        Some(competition) => {
            let standings = iq::standings(&data.iq.scores(competition.id).await?);
            let rows = iq::board_rows(&standings, requester);
            let users: Vec<UserId> = rows.iter().map(|row| row.user).collect();
            let names = names_of(http, guild, &users).await;
            let named: Vec<(iq::BoardRow, String)> = rows
                .iter()
                .map(|row| (*row, iq_text::name_of(row.user, &names).to_owned()))
                .collect();
            iq_text::leaderboard(
                &competition.name,
                end_unix(&competition)?,
                super::now_unix(),
                &named,
                standings.len(),
            )
        }
        None => match data.iq.last_archived(guild).await? {
            Some(last) => {
                let standings = iq::standings(&data.iq.scores(last.id).await?);
                let (named, points) = named_winners(http, guild, &standings).await;
                let winner_names: Vec<&str> = named.iter().map(String::as_str).collect();
                iq_text::no_competition(Some((&last.name, &winner_names, points)))
            }
            None => iq_text::no_competition(None),
        },
    };
    ctx.send(
        poise::CreateReply::default()
            .content(content)
            .allowed_mentions(serenity::CreateAllowedMentions::new()),
    )
    .await?;
    Ok(())
}

fn guild_of(ctx: Context<'_>) -> Option<GuildId> {
    commands::place(ctx).map(|place| place.guild)
}

fn author(ctx: Context<'_>) -> UserId {
    UserId::new(ctx.author().id.get())
}

async fn refuse(
    ctx: Context<'_>,
    reason: &'static str,
    content: impl Into<String>,
) -> Result<(), Error> {
    failure::refused(
        "command.refused",
        "an IQ Competition command was refused",
        reason,
        &Scope::of_command(ctx),
    );
    commands::private(ctx, content).await
}

async fn confirm(
    ctx: Context<'_>,
    question: String,
    part: IqPanelPart,
    label: &str,
    style: serenity::ButtonStyle,
) -> Result<(), Error> {
    let panel = ctx.id();
    let row = serenity::CreateActionRow::Buttons(vec![
        serenity::CreateButton::new(iq_panel_id(panel, part))
            .label(label)
            .style(style),
        serenity::CreateButton::new(iq_panel_id(panel, IqPanelPart::Cancel))
            .label(iq_text::CANCEL_LABEL)
            .style(serenity::ButtonStyle::Secondary),
    ]);
    ctx.send(commands::private_reply(question).components(vec![row]))
        .await?;
    Ok(())
}

async fn open_setup(ctx: Context<'_>, guild: GuildId) -> Result<(), Error> {
    let now_unix = super::now_unix();
    let templates = ctx.data().iq.templates(guild).await?;
    let zones = commands::recent_zones(ctx, guild, now_unix).await;
    let Some(today) = local_today(now_unix) else {
        return commands::fail(ctx, Failure::from_error(&IqError::Clock)).await;
    };
    let draft = SetupDraft::new(guild, author(ctx), templates, zones, today);
    open_draft(ctx, guild, draft).await
}

async fn open_draft(ctx: Context<'_>, guild: GuildId, draft: SetupDraft) -> Result<(), Error> {
    let Some(now_ms) = super::now_ms() else {
        return commands::fail(ctx, Failure::from_error(&IqError::Clock)).await;
    };
    let scope = Scope::of_command(ctx);
    let checks = setup_checks(ctx.serenity_context(), ctx.data(), guild, &scope).await;
    let panel = ctx.id();
    let view = draft.panel(panel, super::now_unix(), &checks);
    ctx.data().iq_drafts.open(panel, draft, now_ms);
    ctx.send(event_draft::panel_reply(view)).await?;
    Ok(())
}

fn local_today(now_unix: i64) -> Option<Date> {
    let zone = Zone::find(iq_setup::DEFAULT_ZONE)?;
    let utc_today = schedule::today(now_unix)?;
    [-1_i64, 0, 1]
        .into_iter()
        .filter_map(|offset| utc_today.checked_add(offset.days()).ok())
        .find(|day| {
            let began = day
                .checked_sub(1.day())
                .is_ok_and(|before| zone.midnight_after(before) <= now_unix);
            began && now_unix < zone.midnight_after(*day)
        })
}

async fn setup_checks(
    serenity_context: &serenity::Context,
    data: &Data,
    guild: GuildId,
    scope: &Scope,
) -> SetupChecks {
    let stored = stored_judge_role(data, guild, scope).await;
    let cache = &serenity_context.cache;
    let missing = failure::barnacle_permissions(cache, guild).map_or(Vec::new(), |granted| {
        match granted.administrator() {
            true => Vec::new(),
            false => (SETUP_ACCESS - granted).get_permission_names(),
        }
    });
    SetupChecks {
        role_below_judge: judge_above_me(cache, guild, stored),
        missing,
    }
}

async fn stored_judge_role(data: &Data, guild: GuildId, scope: &Scope) -> Option<RoleId> {
    match data.iq.judge_role(guild).await {
        Ok(role) => role,
        Err(error) => {
            failure::report(
                "iq.judge_role.read_failed",
                "the stored IQ-Judge role could not be read, so it is looked up by name",
                &Failure::from_error(&error),
                scope,
            );
            None
        }
    }
}

fn find_judge_role(
    cache: &serenity::Cache,
    guild: GuildId,
    stored: Option<RoleId>,
) -> Option<serenity::RoleId> {
    let held = cache.guild(NonZeroU64::new(guild.get())?)?;
    let by_id = stored
        .and_then(|role| NonZeroU64::new(role.get()))
        .map(serenity::RoleId::from)
        .filter(|role| held.roles.contains_key(role));
    by_id.or_else(|| {
        held.roles
            .values()
            .filter(|role| role.name == JUDGE_ROLE_NAME)
            .map(|role| role.id)
            .min()
    })
}

fn judge_above_me(cache: &serenity::Cache, guild: GuildId, stored: Option<RoleId>) -> bool {
    let Some(judge) = find_judge_role(cache, guild, stored) else {
        return false;
    };
    let me = cache.current_user().id;
    let Some(held) = NonZeroU64::new(guild.get()).and_then(|id| cache.guild(id)) else {
        return false;
    };
    let Some(member) = held.members.get(&me) else {
        return false;
    };
    let top = member
        .roles
        .iter()
        .filter_map(|role| held.roles.get(role))
        .map(|role| role.position)
        .max()
        .unwrap_or(0);
    held.roles
        .get(&judge)
        .is_some_and(|judge| judge.position >= top)
}

fn end_unix(competition: &Competition) -> Result<i64, IqError> {
    Zone::find(&competition.zone)
        .map(|zone| iq::end_unix(competition.last_day, &zone))
        .ok_or_else(|| IqError::UnknownZone(competition.zone.clone()))
}

fn nonzero(raw: u64) -> Result<NonZeroU64, IqError> {
    NonZeroU64::new(raw).ok_or(IqError::ZeroId)
}

fn discord_channel(channel: ChannelId) -> Result<serenity::ChannelId, IqError> {
    nonzero(channel.get()).map(serenity::ChannelId::from)
}

fn discord_role(role: RoleId) -> Result<serenity::RoleId, IqError> {
    nonzero(role.get()).map(serenity::RoleId::from)
}

fn discord_user(user: UserId) -> Result<serenity::UserId, IqError> {
    nonzero(user.get()).map(serenity::UserId::from)
}

fn discord_guild(guild: GuildId) -> Result<serenity::GuildId, IqError> {
    nonzero(guild.get()).map(serenity::GuildId::from)
}

async fn bounded<T>(work: impl Future<Output = serenity::Result<T>>) -> Result<T, IqError> {
    Ok(tokio::time::timeout(DISCORD_WAIT, work).await??)
}

fn is_kind(error: &IqError, kind: Kind) -> bool {
    Failure::from_error(error).kind == kind
}

async fn post(
    http: &serenity::Http,
    channel: ChannelId,
    content: &str,
    mentions: serenity::CreateAllowedMentions,
) -> Result<Vec<u64>, IqError> {
    let channel = discord_channel(channel)?;
    let mut sent = Vec::new();
    for chunk in iq::message_chunks(content) {
        let message = bounded(
            channel.send_message(
                http,
                serenity::CreateMessage::new()
                    .content(chunk)
                    .allowed_mentions(mentions.clone()),
            ),
        )
        .await?;
        sent.push(message.id.get());
    }
    Ok(sent)
}

async fn named_winners(
    http: &serenity::Http,
    guild: GuildId,
    standings: &[Standing],
) -> (Vec<String>, i64) {
    let winners = iq::winners(standings);
    let users: Vec<UserId> = winners.iter().map(|standing| standing.user).collect();
    let names = names_of(http, guild, &users).await;
    let named = users
        .iter()
        .map(|user| iq_text::name_of(*user, &names).to_owned())
        .collect();
    (named, winners.first().map_or(0, |standing| standing.points))
}

async fn names_of(
    http: &serenity::Http,
    guild: GuildId,
    users: &[UserId],
) -> Vec<(UserId, String)> {
    let mut found = Vec::with_capacity(users.len());
    for user in users {
        if let Some(name) = name_of(http, guild, *user).await {
            found.push((*user, escape(&name)));
        }
    }
    found
}

async fn name_of(http: &serenity::Http, guild: GuildId, user: UserId) -> Option<String> {
    let scope = Scope::default().guild(guild).user(user);
    let (Ok(guild_id), Ok(user_id)) = (discord_guild(guild), discord_user(user)) else {
        return None;
    };
    match bounded(http.get_member(guild_id, user_id)).await {
        Ok(member) => return Some(member.display_name().to_owned()),
        Err(error) if is_kind(&error, Kind::UnknownMember) => {}
        Err(error) => failure::report(
            "iq.name.lookup_failed",
            "a member's server name could not be read, so their username is used",
            &Failure::from_error(&error),
            &scope,
        ),
    }
    match bounded(http.get_user(user_id)).await {
        Ok(found) => Some(found.name),
        Err(error) => {
            failure::report(
                "iq.name.lookup_failed",
                "a member's username could not be read, so they are shown as an unknown member",
                &Failure::from_error(&error),
                &scope,
            );
            None
        }
    }
}

fn overwrites(
    seats: Seats,
    targets: &[serenity::RoleId],
    room: Room,
) -> Vec<serenity::PermissionOverwrite> {
    let view = serenity::Permissions::VIEW_CHANNEL;
    let (players_allow, players_deny, bot_allow) = match room {
        Room::Category => (
            serenity::Permissions::empty(),
            serenity::Permissions::empty(),
            serenity::Permissions::empty(),
        ),
        Room::Announcements => (
            serenity::Permissions::empty(),
            QUIET_FOR_PLAYERS,
            ANNOUNCING_BOT,
        ),
        Room::Submissions => (
            POSTING_PLAYERS,
            serenity::Permissions::empty(),
            CHECKING_BOT,
        ),
    };
    let everyone = (
        serenity::PermissionOverwriteType::Role(seats.guild.everyone_role()),
        serenity::Permissions::empty(),
        view,
    );
    let players = targets.iter().copied().chain([seats.judge]).map(|role| {
        (
            serenity::PermissionOverwriteType::Role(role),
            view | players_allow,
            players_deny,
        )
    });
    let bot = (
        serenity::PermissionOverwriteType::Member(seats.bot),
        view | bot_allow,
        serenity::Permissions::empty(),
    );
    let merged = std::iter::once(everyone).chain(players).chain([bot]).fold(
        BTreeMap::new(),
        |merged, (kind, allow, deny)| {
            let key = overwrite_key(kind);
            let (held_allow, held_deny) = merged.get(&key).map_or(
                (
                    serenity::Permissions::empty(),
                    serenity::Permissions::empty(),
                ),
                |(_, a, d)| (*a, *d),
            );
            let mut next = merged;
            next.insert(key, (kind, held_allow | allow, held_deny | deny));
            next
        },
    );
    merged
        .into_values()
        .map(|(kind, allow, deny)| serenity::PermissionOverwrite {
            allow,
            deny: deny - allow,
            kind,
        })
        .collect()
}

fn overwrite_key(kind: serenity::PermissionOverwriteType) -> (u8, u64) {
    match kind {
        serenity::PermissionOverwriteType::Role(role) => (0, role.get()),
        serenity::PermissionOverwriteType::Member(member) => (1, member.get()),
        _ => (2, 0),
    }
}

fn target_roles(roles: &[RoleId]) -> Result<Vec<serenity::RoleId>, IqError> {
    roles.iter().map(|role| discord_role(*role)).collect()
}

pub(super) async fn pressed(
    data: &Data,
    serenity_context: &serenity::Context,
    component: &serenity::ComponentInteraction,
    panel: u64,
    part: IqPanelPart,
    guild: GuildId,
) {
    let scope = Scope::of_component(component).guild(guild);
    let user = UserId::new(component.user.id.get());
    let Some(now_ms) = super::now_ms() else {
        let content = failed_text(
            "an IQ Competition panel could not be used because the clock reads before 1970",
            &IqError::Clock,
            &scope,
        );
        tell(
            serenity_context,
            component,
            Answer::Response,
            &content,
            &scope,
        )
        .await;
        return;
    };
    let drafts = &data.iq_drafts;
    match part {
        IqPanelPart::Rename => {
            let opened = drafts
                .with(panel, user, now_ms)
                .map(|draft| iq_setup::rename_modal(panel, draft.name()))
                .map_err(Turned::from);
            open_modal(serenity_context, component, opened, &scope).await;
        }
        IqPanelPart::Add => {
            let opened = drafts
                .with(panel, user, now_ms)
                .map(|draft| {
                    drafts.replace(panel, draft.unpicked());
                    iq_setup::criterion_modal(panel, None)
                })
                .map_err(Turned::from);
            open_modal(serenity_context, component, opened, &scope).await;
        }
        IqPanelPart::Edit => {
            let opened = drafts
                .with(panel, user, now_ms)
                .map_err(Turned::from)
                .and_then(|draft| {
                    draft
                        .picked()
                        .and_then(|index| draft.criteria().get(index))
                        .map(|criterion| iq_setup::criterion_modal(panel, Some(criterion)))
                        .ok_or(Turned::Setup(SetupRefusal::CriterionUnknown))
                });
            open_modal(serenity_context, component, opened, &scope).await;
        }
        IqPanelPart::Rules => {
            let opened = drafts
                .with(panel, user, now_ms)
                .map(|draft| iq_setup::rules_modal(panel, draft.rules()))
                .map_err(Turned::from);
            open_modal(serenity_context, component, opened, &scope).await;
        }
        IqPanelPart::Zone if picked(&component.data.kind) == Some(event_draft::ANOTHER_ZONE) => {
            let opened = drafts
                .with(panel, user, now_ms)
                .map(|_| iq_setup::zone_modal(panel))
                .map_err(Turned::from);
            open_modal(serenity_context, component, opened, &scope).await;
        }
        IqPanelPart::Template
        | IqPanelPart::FirstDay
        | IqPanelPart::LastDay
        | IqPanelPart::Zone
        | IqPanelPart::Criterion
        | IqPanelPart::Remove
        | IqPanelPart::Roles
        | IqPanelPart::Judges
        | IqPanelPart::Reaction
        | IqPanelPart::Next
        | IqPanelPart::Back => {
            match changed(drafts, panel, part, &component.data.kind, user, now_ms) {
                Ok(draft) => {
                    let checks = setup_checks(serenity_context, data, guild, &scope).await;
                    let view = draft.panel(panel, super::now_unix(), &checks);
                    update(
                        serenity_context,
                        component,
                        view.content,
                        view.components,
                        &scope,
                    )
                    .await;
                }
                Err(turned) => refuse_press(serenity_context, component, &turned, &scope).await,
            }
        }
        IqPanelPart::Start => {
            finish(
                data,
                serenity_context,
                component,
                panel,
                user,
                now_ms,
                guild,
                &scope,
            )
            .await;
        }
        IqPanelPart::Cancel => {
            let content = match drafts.take(panel, user, now_ms) {
                Ok(draft) => match draft.purpose() {
                    Purpose::Start => NOTHING_CREATED,
                    Purpose::Edit { .. } => iq_text::CANCELLED,
                },
                Err(Refusal::PanelUnknown) => iq_text::CANCELLED,
                Err(refusal) => {
                    refuse_press(serenity_context, component, &Turned::Panel(refusal), &scope)
                        .await;
                    return;
                }
            };
            failure::record(
                "iq.panel.cancelled",
                "an IQ Competition panel was cancelled",
                &scope,
            );
            update(
                serenity_context,
                component,
                content.to_owned(),
                Vec::new(),
                &scope,
            )
            .await;
        }
        IqPanelPart::JudgesUsers => {
            let chosen = match &component.data.kind {
                serenity::ComponentInteractionDataKind::UserSelect { values } => {
                    values.iter().map(|user| UserId::new(user.get())).collect()
                }
                _ => Vec::new(),
            };
            let redrawn = drafts
                .with(panel, user, now_ms)
                .map_err(Turned::from)
                .and_then(|draft| draft.with_judges(chosen).map_err(Turned::from));
            match redrawn {
                Ok(draft) => {
                    drafts.replace(panel, draft.clone());
                    update(
                        serenity_context,
                        component,
                        iq_text::judges_prompt(draft.name()),
                        judges_rows(panel, draft.judges()),
                        &scope,
                    )
                    .await;
                }
                Err(turned) => refuse_press(serenity_context, component, &turned, &scope).await,
            }
        }
        IqPanelPart::ConfirmResume
        | IqPanelPart::ConfirmStop
        | IqPanelPart::ConfirmComplete
        | IqPanelPart::ConfirmCleanup
        | IqPanelPart::JudgesSave => {
            confirmed(
                data,
                serenity_context,
                component,
                panel,
                part,
                user,
                now_ms,
                guild,
                &scope,
            )
            .await;
        }
        IqPanelPart::RenameModal
        | IqPanelPart::CriterionModal
        | IqPanelPart::RulesModal
        | IqPanelPart::ZoneModal => {
            refuse_press(
                serenity_context,
                component,
                &Turned::Panel(Refusal::PanelUnknown),
                &scope,
            )
            .await;
        }
    }
}

pub(super) async fn submitted(
    data: &Data,
    serenity_context: &serenity::Context,
    modal: &serenity::ModalInteraction,
    panel: u64,
    part: IqPanelPart,
    guild: GuildId,
) {
    let user = UserId::new(modal.user.id.get());
    let scope = Scope {
        guild: Some(guild),
        channel: Some(ChannelId::new(modal.channel_id.get())),
        user: Some(user),
        interaction: Some(modal.id.get()),
        message: modal
            .message
            .as_ref()
            .map(|message| Snowflake::new(message.id.get())),
        ..Scope::default()
    };
    let Some(now_ms) = super::now_ms() else {
        let content = failed_text(
            "an IQ Competition form could not be used because the clock reads before 1970",
            &IqError::Clock,
            &scope,
        );
        answer_modal(serenity_context, modal, Err(content), &scope).await;
        return;
    };
    let typed = |id: &str| typed_value(&modal.data, id);
    let drafts = &data.iq_drafts;
    let changed: Result<SetupDraft, Turned> = drafts
        .with(panel, user, now_ms)
        .map_err(Turned::from)
        .and_then(|draft| {
            let next = match part {
                IqPanelPart::RenameModal => draft.renamed(&typed(iq_setup::NAME_INPUT)),
                IqPanelPart::CriterionModal => draft.with_criterion(
                    &typed(iq_setup::NAME_INPUT),
                    &typed(iq_setup::DESCRIPTION_INPUT),
                    &typed(iq_setup::POINTS_INPUT),
                    draft.picked(),
                ),
                IqPanelPart::RulesModal => draft.with_rules(&typed(iq_setup::RULES_INPUT)),
                IqPanelPart::ZoneModal => draft.with_zone(&typed(event_draft::ZONE_INPUT)),
                IqPanelPart::Template
                | IqPanelPart::FirstDay
                | IqPanelPart::LastDay
                | IqPanelPart::Zone
                | IqPanelPart::Rename
                | IqPanelPart::Criterion
                | IqPanelPart::Add
                | IqPanelPart::Edit
                | IqPanelPart::Remove
                | IqPanelPart::Rules
                | IqPanelPart::Roles
                | IqPanelPart::Judges
                | IqPanelPart::Reaction
                | IqPanelPart::Next
                | IqPanelPart::Back
                | IqPanelPart::Start
                | IqPanelPart::Cancel
                | IqPanelPart::ConfirmResume
                | IqPanelPart::ConfirmStop
                | IqPanelPart::ConfirmComplete
                | IqPanelPart::ConfirmCleanup
                | IqPanelPart::JudgesUsers
                | IqPanelPart::JudgesSave => return Err(Turned::Panel(Refusal::PanelUnknown)),
            };
            next.map_err(Turned::from)
        });
    let answer = match changed {
        Ok(draft) => {
            drafts.replace(panel, draft.clone());
            let checks = setup_checks(serenity_context, data, guild, &scope).await;
            Ok(draft.panel(panel, super::now_unix(), &checks))
        }
        Err(turned) => Err(turned_text(&turned, &scope)),
    };
    answer_modal(serenity_context, modal, answer, &scope).await;
}

fn typed_value(data: &serenity::ModalInteractionData, id: &str) -> String {
    data.components
        .iter()
        .flat_map(|row| row.components.iter())
        .find_map(|component| match component {
            serenity::ActionRowComponent::InputText(input) if input.custom_id == id => {
                Some(input.value.clone().unwrap_or_default())
            }
            _ => None,
        })
        .unwrap_or_default()
}

fn picked(kind: &serenity::ComponentInteractionDataKind) -> Option<&str> {
    match kind {
        serenity::ComponentInteractionDataKind::StringSelect { values } => {
            values.first().map(String::as_str)
        }
        _ => None,
    }
}

fn changed(
    drafts: &iq_setup::Drafts,
    panel: u64,
    part: IqPanelPart,
    kind: &serenity::ComponentInteractionDataKind,
    user: UserId,
    now_ms: u64,
) -> Result<SetupDraft, Turned> {
    let draft = drafts.with(panel, user, now_ms)?;
    let single = picked(kind).unwrap_or_default();
    let next = match (part, kind) {
        (IqPanelPart::Template, serenity::ComponentInteractionDataKind::StringSelect { .. }) => {
            draft.with_template(single)?
        }
        (IqPanelPart::FirstDay, serenity::ComponentInteractionDataKind::StringSelect { .. }) => {
            draft.with_first_day(single)?
        }
        (IqPanelPart::LastDay, serenity::ComponentInteractionDataKind::StringSelect { .. }) => {
            draft.with_last_day(single)?
        }
        (IqPanelPart::Zone, serenity::ComponentInteractionDataKind::StringSelect { .. }) => {
            draft.with_zone(single)?
        }
        (IqPanelPart::Criterion, serenity::ComponentInteractionDataKind::StringSelect { .. }) => {
            let index = single
                .parse::<usize>()
                .map_err(|_| SetupRefusal::CriterionUnknown)?;
            draft.with_picked_criterion(index)?
        }
        (IqPanelPart::Reaction, serenity::ComponentInteractionDataKind::StringSelect { .. }) => {
            draft.with_reaction(single)?
        }
        (IqPanelPart::Roles, serenity::ComponentInteractionDataKind::RoleSelect { values }) => {
            draft.with_roles(values.iter().map(|role| RoleId::new(role.get())).collect())?
        }
        (IqPanelPart::Judges, serenity::ComponentInteractionDataKind::UserSelect { values }) => {
            draft.with_judges(values.iter().map(|user| UserId::new(user.get())).collect())?
        }
        (IqPanelPart::Remove, serenity::ComponentInteractionDataKind::Button) => {
            draft.remove_picked()?
        }
        (IqPanelPart::Next, serenity::ComponentInteractionDataKind::Button) => draft.next()?,
        (IqPanelPart::Back, serenity::ComponentInteractionDataKind::Button) => draft.back(),
        _ => return Err(Turned::Panel(Refusal::PanelUnknown)),
    };
    drafts.replace(panel, next.clone());
    Ok(next)
}

fn judges_rows(panel: u64, judges: &[UserId]) -> Vec<serenity::CreateActionRow> {
    let menu = serenity::CreateSelectMenu::new(
        iq_panel_id(panel, IqPanelPart::JudgesUsers),
        serenity::CreateSelectMenuKind::User {
            default_users: (!judges.is_empty()).then(|| {
                judges
                    .iter()
                    .filter_map(|judge| NonZeroU64::new(judge.get()).map(serenity::UserId::from))
                    .collect()
            }),
        },
    )
    .placeholder(iq_text::JUDGES_PLACEHOLDER)
    .min_values(0)
    .max_values(MENU_MAX);
    vec![
        serenity::CreateActionRow::SelectMenu(menu),
        serenity::CreateActionRow::Buttons(vec![
            serenity::CreateButton::new(iq_panel_id(panel, IqPanelPart::JudgesSave))
                .label(SAVE_JUDGES_LABEL)
                .style(serenity::ButtonStyle::Success),
            serenity::CreateButton::new(iq_panel_id(panel, IqPanelPart::Cancel))
                .label(iq_text::CANCEL_LABEL)
                .style(serenity::ButtonStyle::Secondary),
        ]),
    ]
}

fn manages_server(component: &serenity::ComponentInteraction) -> bool {
    component
        .member
        .as_ref()
        .and_then(|member| member.permissions)
        .is_some_and(|permissions| permissions.manage_guild())
}

async fn refuse_manage_server(
    serenity_context: &serenity::Context,
    component: &serenity::ComponentInteraction,
    scope: &Scope,
) {
    failure::refused(
        "interaction.refused",
        "an IQ Competition button was pressed by a member without Manage Server",
        "missing_user_permissions",
        scope,
    );
    tell(
        serenity_context,
        component,
        Answer::Response,
        MANAGE_SERVER_NEEDED,
        scope,
    )
    .await;
}

#[allow(clippy::too_many_arguments)]
async fn finish(
    data: &Data,
    serenity_context: &serenity::Context,
    component: &serenity::ComponentInteraction,
    panel: u64,
    user: UserId,
    now_ms: u64,
    guild: GuildId,
    scope: &Scope,
) {
    if !manages_server(component) {
        refuse_manage_server(serenity_context, component, scope).await;
        return;
    }
    let drafts = &data.iq_drafts;
    let draft = match drafts.with(panel, user, now_ms) {
        Ok(draft) => draft,
        Err(refusal) => {
            refuse_press(serenity_context, component, &Turned::Panel(refusal), scope).await;
            return;
        }
    };
    let ready = match draft.ready() {
        Ok(ready) => ready,
        Err(refusal) => {
            refuse_press(serenity_context, component, &Turned::Setup(refusal), scope).await;
            return;
        }
    };
    let checks = setup_checks(serenity_context, data, guild, scope).await;
    if matches!(ready, SetupReady::Start(_))
        && (checks.role_below_judge || !checks.missing.is_empty())
    {
        failure::refused(
            "interaction.refused",
            "Start was pressed while the bot lacks a permission or sits below IQ-Judge",
            "setup_checks_failed",
            scope,
        );
        let view = draft.panel(panel, super::now_unix(), &checks);
        update(
            serenity_context,
            component,
            view.content,
            view.components,
            scope,
        )
        .await;
        return;
    }
    if !acknowledge(serenity_context, component, scope).await {
        return;
    }
    if let Err(refusal) = drafts.take(panel, user, now_ms) {
        let content = turned_text(&Turned::Panel(refusal), scope);
        tell(
            serenity_context,
            component,
            Answer::Followup,
            &content,
            scope,
        )
        .await;
        return;
    }
    let answer = match &ready {
        SetupReady::Start(plan) => start_competition(data, serenity_context, plan, scope)
            .await
            .map_err(Unfinished::untouched),
        SetupReady::Edit(change) => save_changes(data, serenity_context, change, scope).await,
    };
    match answer {
        Ok(content) => {
            edit_reply(serenity_context, component, content, scope).await;
        }
        Err(error) => {
            drafts.open(panel, draft, now_ms);
            let content = unfinished_text(
                "an IQ Competition could not be started or saved",
                &error,
                scope,
            );
            tell(
                serenity_context,
                component,
                Answer::Followup,
                &content,
                scope,
            )
            .await;
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn confirmed(
    data: &Data,
    serenity_context: &serenity::Context,
    component: &serenity::ComponentInteraction,
    panel: u64,
    part: IqPanelPart,
    user: UserId,
    now_ms: u64,
    guild: GuildId,
    scope: &Scope,
) {
    if !manages_server(component) {
        refuse_manage_server(serenity_context, component, scope).await;
        return;
    }
    if !acknowledge(serenity_context, component, scope).await {
        return;
    }
    let http = &serenity_context.http;
    let answer = match part {
        IqPanelPart::ConfirmResume => resume(data, http, guild, scope)
            .await
            .map_err(Unfinished::untouched),
        IqPanelPart::ConfirmStop => stop(data, http, guild, scope)
            .await
            .map_err(Unfinished::untouched),
        IqPanelPart::ConfirmComplete => complete(data, http, guild, scope).await,
        IqPanelPart::ConfirmCleanup => clean_up(data, serenity_context, guild, scope).await,
        IqPanelPart::JudgesSave => match data.iq_drafts.take(panel, user, now_ms) {
            Ok(draft) => save_judges(data, serenity_context, guild, draft.judges(), scope).await,
            Err(refusal) => Ok(turned_text(&Turned::Panel(refusal), scope)),
        },
        IqPanelPart::Template
        | IqPanelPart::FirstDay
        | IqPanelPart::LastDay
        | IqPanelPart::Zone
        | IqPanelPart::Rename
        | IqPanelPart::Criterion
        | IqPanelPart::Add
        | IqPanelPart::Edit
        | IqPanelPart::Remove
        | IqPanelPart::Rules
        | IqPanelPart::Roles
        | IqPanelPart::Judges
        | IqPanelPart::Reaction
        | IqPanelPart::Next
        | IqPanelPart::Back
        | IqPanelPart::Start
        | IqPanelPart::Cancel
        | IqPanelPart::JudgesUsers
        | IqPanelPart::RenameModal
        | IqPanelPart::CriterionModal
        | IqPanelPart::RulesModal
        | IqPanelPart::ZoneModal => Ok(turned_text(&Turned::Panel(Refusal::PanelUnknown), scope)),
    };
    let content = match answer {
        Ok(content) => content,
        Err(error) => unfinished_text("an IQ Competition button did not finish", &error, scope),
    };
    edit_reply(serenity_context, component, content, scope).await;
}

async fn resume(
    data: &Data,
    http: &serenity::Http,
    guild: GuildId,
    scope: &Scope,
) -> Result<String, IqError> {
    let Some(competition) = data.iq.live(guild).await? else {
        return Ok(iq_text::NO_COMPETITION_RUNNING.to_owned());
    };
    match iq::start_choice(Some(competition.status)) {
        StartChoice::Resume => {}
        StartChoice::AlreadyRunning => return Ok(iq_text::already_running(&competition.name)),
        StartChoice::NeedsCleanup => return Ok(iq_text::needs_cleanup(&competition.name)),
        StartChoice::Setup => return Ok(iq_text::NO_COMPETITION_RUNNING.to_owned()),
    }
    data.iq.resume(competition.id, super::now_unix()).await?;
    data.iq_caught_up
        .finish(competition.guild, data.iq_caught_up.current());
    failure::record("iq.resumed", "an IQ Competition resumed", scope);
    notice(
        http,
        competition.announcements,
        &iq_text::resumed_notice(&competition.name),
        scope,
    )
    .await;
    Ok(iq_text::RESUMED.to_owned())
}

async fn stop(
    data: &Data,
    http: &serenity::Http,
    guild: GuildId,
    scope: &Scope,
) -> Result<String, IqError> {
    let Some(competition) = data.iq.live(guild).await? else {
        return Ok(iq_text::NO_COMPETITION_RUNNING.to_owned());
    };
    match competition.status {
        Status::Running => {}
        Status::Completed => return Ok(iq_text::needs_cleanup(&competition.name)),
        Status::Stopped | Status::Archived => {
            return Ok(iq_text::NO_COMPETITION_RUNNING.to_owned());
        }
    }
    data.iq.set_status(competition.id, Status::Stopped).await?;
    failure::record("iq.stopped", "an IQ Competition was paused", scope);
    notice(
        http,
        competition.announcements,
        &iq_text::paused_notice(&competition.name, competition.submissions),
        scope,
    )
    .await;
    Ok(iq_text::paused(competition.announcements))
}

async fn notice(http: &serenity::Http, channel: ChannelId, content: &str, scope: &Scope) {
    if let Err(error) = post(
        http,
        channel,
        content,
        serenity::CreateAllowedMentions::new(),
    )
    .await
    {
        failure::report(
            "iq.notice.failed",
            "a notice could not be posted in the announcements channel",
            &Failure::from_error(&error),
            scope,
        );
    }
}

async fn complete(
    data: &Data,
    http: &serenity::Http,
    guild: GuildId,
    scope: &Scope,
) -> Result<String, Unfinished> {
    let Some(competition) = data.iq.live(guild).await.map_err(Unfinished::untouched)? else {
        return Ok(iq_text::NO_COMPETITION_RUNNING.to_owned());
    };
    let end = end_unix(&competition).map_err(Unfinished::untouched)?;
    match iq::may_complete(competition.status, end, super::now_unix()) {
        Ok(()) => {}
        Err(CompleteRefusal::TooEarly) => {
            return Ok(iq_text::complete_too_early(&competition.name, end));
        }
        Err(CompleteRefusal::NotLive) => return Ok(iq_text::needs_cleanup(&competition.name)),
    }
    let standings = iq::standings(
        &data
            .iq
            .scores(competition.id)
            .await
            .map_err(Unfinished::untouched)?,
    );
    let winners = iq::winners(&standings);
    let shown: Vec<UserId> = standings
        .iter()
        .take(ANNOUNCED_LINES)
        .map(|standing| standing.user)
        .collect();
    let names = names_of(http, guild, &shown).await;
    let text =
        iq_text::winner_announcement(&competition.name, &competition.roles, &standings, &names);
    let winner_users: Vec<serenity::UserId> = winners
        .iter()
        .map(|standing| discord_user(standing.user))
        .collect::<Result<_, _>>()
        .map_err(Unfinished::untouched)?;
    let mentions = serenity::CreateAllowedMentions::new()
        .roles(target_roles(&competition.roles).map_err(Unfinished::untouched)?)
        .users(winner_users);
    data.iq
        .set_status(competition.id, Status::Completed)
        .await
        .map_err(Unfinished::untouched)?;
    if let Err(error) = post(http, competition.announcements, &text, mentions).await {
        return Err(
            match data
                .iq
                .revert_completed(competition.id, competition.status)
                .await
            {
                Ok(_) => Unfinished::untouched(error),
                Err(reverted) => {
                    failure::report(
                        "iq.complete.unreverted",
                        "the winner announcement failed and the competition could not be set back",
                        &Failure::from_error(&IqError::from(reverted)),
                        scope,
                    );
                    Progress::default().with(Done::Completed).stopped(error)
                }
            },
        );
    }
    failure::record(
        "iq.completed",
        "an IQ Competition's winner was announced",
        scope,
    );
    Ok(iq_text::COMPLETED.to_owned())
}

async fn clean_up(
    data: &Data,
    serenity_context: &serenity::Context,
    guild: GuildId,
    scope: &Scope,
) -> Result<String, Unfinished> {
    let http = &serenity_context.http;
    let Some(competition) = data.iq.live(guild).await.map_err(Unfinished::untouched)? else {
        return Ok(iq_text::NO_COMPETITION_RUNNING.to_owned());
    };
    match iq::may_clean_up(competition.status) {
        Ok(()) => {}
        Err(CleanupRefusal::Running) => {
            return Ok(iq_text::cleanup_while_running(&competition.name));
        }
    }
    let progress = Progress::default();
    let role = current_judge_role(data, &serenity_context.cache, guild, scope).await;
    let progress = match (role, competition.judges.is_empty()) {
        (Some(role), false) => {
            change_judges(http, guild, role, &competition.judges, &[])
                .await
                .map_err(|error| progress.stopped(error))?;
            progress.with(Done::JudgesChanged)
        }
        _ => progress,
    };
    for channel in [
        competition.submissions,
        competition.announcements,
        competition.category,
    ] {
        remove_channel(
            http,
            discord_channel(channel).map_err(|error| progress.stopped(error))?,
        )
        .await
        .map_err(|error| progress.stopped(error))?;
    }
    let progress = progress.with(Done::ChannelsDeleted);
    data.iq
        .archive(competition.id)
        .await
        .map_err(|error| progress.stopped(error))?;
    failure::record(
        "iq.cleaned_up",
        "an IQ Competition was cleaned up and archived",
        scope,
    );
    let templates = match data.iq.templates(guild).await {
        Ok(templates) => templates.len(),
        Err(error) => {
            failure::report(
                "iq.cleanup.templates_unread",
                "the saved templates could not be counted after clean-up",
                &Failure::from_error(&IqError::from(error)),
                scope,
            );
            0
        }
    };
    Ok(iq_text::cleaned_up(&competition.name, templates))
}

async fn remove_channel(
    http: &serenity::Http,
    channel: serenity::ChannelId,
) -> Result<(), IqError> {
    match bounded(channel.delete(http)).await {
        Ok(_) => Ok(()),
        Err(error) if is_kind(&error, Kind::UnknownChannel) => Ok(()),
        Err(error) => Err(error),
    }
}

async fn save_judges(
    data: &Data,
    serenity_context: &serenity::Context,
    guild: GuildId,
    judges: &[UserId],
    scope: &Scope,
) -> Result<String, Unfinished> {
    let Some(competition) = data.iq.live(guild).await.map_err(Unfinished::untouched)? else {
        return Ok(iq_text::NO_COMPETITION_RUNNING.to_owned());
    };
    let role = judge_role(data, serenity_context, guild, scope)
        .await
        .map_err(Unfinished::untouched)?;
    change_judges(
        &serenity_context.http,
        guild,
        role,
        &competition.judges,
        judges,
    )
    .await
    .map_err(Unfinished::untouched)?;
    let progress = Progress::default().with(Done::JudgesChanged);
    data.iq
        .update_config(&ConfigChange {
            competition: competition.id,
            criteria: competition.criteria.clone(),
            rules: competition.rules.clone(),
            roles: competition.roles.clone(),
            judges: judges.to_vec(),
            reaction: competition.reaction,
        })
        .await
        .map_err(|error| progress.stopped(error))?;
    failure::record(
        "iq.judges.saved",
        "the IQ-Judge role was handed out and taken away",
        scope,
    );
    Ok(iq_text::judges_saved(judges))
}

async fn judge_role(
    data: &Data,
    serenity_context: &serenity::Context,
    guild: GuildId,
    scope: &Scope,
) -> Result<serenity::RoleId, IqError> {
    let stored = stored_judge_role(data, guild, scope).await;
    let found = find_judge_role(&serenity_context.cache, guild, stored);
    let role = match found {
        Some(role) => role,
        None => {
            bounded(discord_guild(guild)?.create_role(
                &serenity_context.http,
                serenity::EditRole::new().name(JUDGE_ROLE_NAME),
            ))
            .await?
            .id
        }
    };
    if stored.map(RoleId::get) != Some(role.get()) {
        data.iq
            .set_judge_role(guild, RoleId::new(role.get()))
            .await?;
    }
    Ok(role)
}

async fn change_judges(
    http: &serenity::Http,
    guild: GuildId,
    role: serenity::RoleId,
    before: &[UserId],
    after: &[UserId],
) -> Result<(), IqError> {
    let guild_id = discord_guild(guild)?;
    for judge in after.iter().filter(|judge| !before.contains(judge)) {
        bounded(http.add_member_role(guild_id, discord_user(*judge)?, role, None)).await?;
    }
    for judge in before.iter().filter(|judge| !after.contains(judge)) {
        match bounded(http.remove_member_role(guild_id, discord_user(*judge)?, role, None)).await {
            Ok(()) => {}
            Err(error) if iq::role_already_off(Failure::from_error(&error).kind) => {}
            Err(error) => return Err(error),
        }
    }
    Ok(())
}

async fn start_competition(
    data: &Data,
    serenity_context: &serenity::Context,
    plan: &SetupPlan,
    scope: &Scope,
) -> Result<String, IqError> {
    let created_at_ms = super::now_ms()
        .and_then(|now_ms| i64::try_from(now_ms).ok())
        .ok_or(IqError::Clock)?;
    let stored = stored_judge_role(data, plan.guild, scope).await;
    let found = find_judge_role(&serenity_context.cache, plan.guild, stored);
    let mut made = Made::default();
    let built = build(serenity_context, plan, found, &mut made).await;
    let built = match built {
        Ok(built) => built,
        Err(error) => {
            undo(&serenity_context.http, plan.guild, made, scope).await;
            return Err(error);
        }
    };
    let saved = data
        .iq
        .create(&plan.competition(built.channels, created_at_ms))
        .await;
    let competition = match saved {
        Ok(CreateOutcome::Created(competition)) => {
            data.iq_caught_up
                .finish(competition.guild, data.iq_caught_up.current());
            competition
        }
        Ok(CreateOutcome::AlreadyLive) => {
            undo(&serenity_context.http, plan.guild, made, scope).await;
            return Ok(iq_text::already_running(&plan.name));
        }
        Err(error) => {
            undo(&serenity_context.http, plan.guild, made, scope).await;
            return Err(error.into());
        }
    };
    if let Err(error) = data
        .iq
        .set_announcements(competition.id, &built.messages)
        .await
    {
        failure::report(
            "iq.start.announcements_unsaved",
            "the start announcement's message IDs were not saved, so /iq edit sends a fresh one",
            &Failure::from_error(&error),
            scope,
        );
    }
    if let Err(error) = data
        .iq
        .set_judge_role(plan.guild, RoleId::new(built.role.get()))
        .await
    {
        failure::report(
            "iq.start.judge_role_unsaved",
            "the IQ-Judge role's ID was not saved, so it is found by name next time",
            &Failure::from_error(&error),
            scope,
        );
    }
    failure::record("iq.started", "an IQ Competition started", scope);
    Ok(started_text(
        &plan.name,
        built.channels.submissions,
        built.end_unix,
    ))
}

fn started_text(name: &str, submissions: ChannelId, end_unix: i64) -> String {
    format!(
        "**{}** has started. Players can post in <#{}> until <t:{}:F>.",
        escape(name),
        submissions.get(),
        end_unix - SHOWN_END_OFFSET
    )
}

async fn build(
    serenity_context: &serenity::Context,
    plan: &SetupPlan,
    found: Option<serenity::RoleId>,
    made: &mut Made,
) -> Result<Built, IqError> {
    let http = &serenity_context.http;
    let guild = discord_guild(plan.guild)?;
    let zone = Zone::find(&plan.zone).ok_or_else(|| IqError::UnknownZone(plan.zone.clone()))?;
    let end = iq::end_unix(plan.last_day, &zone);
    let role = match found {
        Some(role) => role,
        None => {
            let created =
                bounded(guild.create_role(http, serenity::EditRole::new().name(JUDGE_ROLE_NAME)))
                    .await?
                    .id;
            made.role = Some(created);
            created
        }
    };
    let seats = Seats {
        guild,
        judge: role,
        bot: serenity_context.cache.current_user().id,
    };
    let targets = target_roles(&plan.roles)?;
    let category = bounded(
        guild.create_channel(
            http.as_ref(),
            serenity::CreateChannel::new(plan.name.clone())
                .kind(serenity::ChannelType::Category)
                .permissions(overwrites(seats, &targets, Room::Category)),
        ),
    )
    .await?
    .id;
    made.channels.push(category);
    let announcements = bounded(
        guild.create_channel(
            http.as_ref(),
            serenity::CreateChannel::new(ANNOUNCEMENTS_NAME)
                .kind(serenity::ChannelType::Text)
                .category(category)
                .permissions(overwrites(seats, &targets, Room::Announcements)),
        ),
    )
    .await?
    .id;
    made.channels.push(announcements);
    let submissions = bounded(
        guild.create_channel(
            http.as_ref(),
            serenity::CreateChannel::new(SUBMISSIONS_NAME)
                .kind(serenity::ChannelType::Text)
                .category(category)
                .permissions(overwrites(seats, &targets, Room::Submissions)),
        ),
    )
    .await?
    .id;
    made.channels.push(submissions);
    let channels = CompetitionChannels {
        category: ChannelId::new(category.get()),
        announcements: ChannelId::new(announcements.get()),
        submissions: ChannelId::new(submissions.get()),
    };
    let text = iq_text::start_announcement(
        &plan.name,
        end,
        &plan.roles,
        channels.submissions,
        &plan.criteria,
        &plan.rules,
        &plan.judges,
    );
    let messages = post(
        http,
        channels.announcements,
        &text,
        serenity::CreateAllowedMentions::new().roles(targets.clone()),
    )
    .await?;
    for judge in &plan.judges {
        let user = discord_user(*judge)?;
        let already = match made.role {
            Some(_) => false,
            None => bounded(http.get_member(guild, user))
                .await
                .is_ok_and(|member| member.roles.contains(&role)),
        };
        if !already {
            bounded(http.add_member_role(guild, user, role, None)).await?;
            made.granted.push((user, role));
        }
    }
    Ok(Built {
        role,
        channels,
        messages,
        end_unix: end,
    })
}

async fn undo(http: &serenity::Http, guild: GuildId, made: Made, scope: &Scope) {
    let report = |what: &str, error: &IqError| {
        failure::report(
            "iq.start.undo_failed",
            what,
            &Failure::from_error(error),
            scope,
        );
    };
    let Ok(guild_id) = discord_guild(guild) else {
        return;
    };
    if made.role.is_none() {
        for (user, role) in &made.granted {
            if let Err(error) = bounded(http.remove_member_role(guild_id, *user, *role, None)).await
            {
                report(
                    "a judge granted during a failed start kept the IQ-Judge role",
                    &error,
                );
            }
        }
    }
    for channel in made.channels.iter().rev() {
        if let Err(error) = remove_channel(http, *channel).await {
            report(
                "a channel created during a failed start could not be deleted",
                &error,
            );
        }
    }
    if let Some(role) = made.role
        && let Err(error) = bounded(guild_id.delete_role(http, role)).await
    {
        report(
            "the IQ-Judge role created during a failed start could not be deleted",
            &error,
        );
    }
    failure::record("iq.start.undone", "a failed start was undone", scope);
}

async fn save_changes(
    data: &Data,
    serenity_context: &serenity::Context,
    change: &ConfigChange,
    scope: &Scope,
) -> Result<String, Unfinished> {
    let http = &serenity_context.http;
    let Some(before) = data
        .iq
        .competition(change.competition)
        .await
        .map_err(Unfinished::untouched)?
    else {
        return Ok(iq_text::NO_COMPETITION_RUNNING.to_owned());
    };
    match before.status {
        Status::Running | Status::Stopped => {}
        Status::Completed => return Ok(iq_text::needs_cleanup(&before.name)),
        Status::Archived => return Ok(iq_text::NO_COMPETITION_RUNNING.to_owned()),
    }
    let roles_changed = !same_set(&before.roles, &change.roles);
    let judges_changed = !same_set(&before.judges, &change.judges);
    let role = match roles_changed || judges_changed {
        true => Some(
            judge_role(data, serenity_context, before.guild, scope)
                .await
                .map_err(Unfinished::untouched)?,
        ),
        false => None,
    };
    let text = iq_text::start_announcement(
        &before.name,
        end_unix(&before).map_err(Unfinished::untouched)?,
        &change.roles,
        before.submissions,
        &change.criteria,
        &change.rules,
        &change.judges,
    );
    let held = data
        .iq
        .announcements(before.id)
        .await
        .map_err(Unfinished::untouched)?;
    let progress = Progress::default();
    let progress = match (role, roles_changed) {
        (Some(role), true) => {
            let seats = Seats {
                guild: discord_guild(before.guild).map_err(Unfinished::untouched)?,
                judge: role,
                bot: serenity_context.cache.current_user().id,
            };
            let targets = target_roles(&change.roles).map_err(Unfinished::untouched)?;
            for (channel, room) in [
                (before.category, Room::Category),
                (before.announcements, Room::Announcements),
                (before.submissions, Room::Submissions),
            ] {
                bounded(
                    discord_channel(channel)
                        .map_err(|error| progress.stopped(error))?
                        .edit(
                            http.as_ref(),
                            serenity::EditChannel::new()
                                .permissions(overwrites(seats, &targets, room)),
                        ),
                )
                .await
                .map_err(|error| progress.stopped(error))?;
            }
            progress.with(Done::ChannelsUpdated)
        }
        _ => progress,
    };
    let progress = match (role, judges_changed) {
        (Some(role), true) => {
            change_judges(http, before.guild, role, &before.judges, &change.judges)
                .await
                .map_err(|error| progress.stopped(error))?;
            progress.with(Done::JudgesChanged)
        }
        _ => progress,
    };
    let kept = sync_announcements(
        http,
        before.announcements,
        &held,
        &iq::message_chunks(&text),
    )
    .await
    .map_err(|error| progress.stopped(error))?;
    let progress = progress.with(Done::AnnouncementUpdated);
    data.iq
        .set_announcements(before.id, &kept)
        .await
        .map_err(|error| progress.stopped(error))?;
    data.iq
        .update_config(change)
        .await
        .map_err(|error| progress.stopped(error))?;
    let (added, altered, removed) = criteria_changes(&before.criteria, &change.criteria);
    let note = iq_text::edit_note(&added, &altered, &removed, before.rules != change.rules);
    notice(http, before.announcements, &note, scope).await;
    failure::record(
        "iq.edited",
        "an IQ Competition's settings were changed",
        scope,
    );
    Ok(saved_text(before.announcements))
}

fn saved_text(announcements: ChannelId) -> String {
    format!(
        "Saved. The announcement in <#{}> is updated, with a note saying what changed.",
        announcements.get()
    )
}

fn same_set<T: Ord + Clone>(before: &[T], after: &[T]) -> bool {
    let sorted = |items: &[T]| {
        let mut held = items.to_vec();
        held.sort();
        held.dedup();
        held
    };
    sorted(before) == sorted(after)
}

fn criteria_changes(
    before: &[Criterion],
    after: &[Criterion],
) -> (Vec<Criterion>, Vec<Criterion>, Vec<Criterion>) {
    let earlier = |criterion: &Criterion| {
        before
            .iter()
            .find(|held| same_name(&held.name, &criterion.name))
    };
    let added = after
        .iter()
        .filter(|criterion| earlier(criterion).is_none())
        .cloned()
        .collect();
    let altered = after
        .iter()
        .filter(|criterion| earlier(criterion).is_some_and(|held| held != *criterion))
        .cloned()
        .collect();
    let removed = before
        .iter()
        .filter(|held| {
            !after
                .iter()
                .any(|criterion| same_name(&held.name, &criterion.name))
        })
        .cloned()
        .collect();
    (added, altered, removed)
}

async fn sync_announcements(
    http: &serenity::Http,
    channel: ChannelId,
    held: &[u64],
    chunks: &[String],
) -> Result<Vec<u64>, IqError> {
    let channel_id = discord_channel(channel)?;
    let mut kept = Vec::with_capacity(chunks.len());
    for index in 0..held.len().max(chunks.len()) {
        match (held.get(index), chunks.get(index)) {
            (Some(message), Some(text)) => {
                let message = serenity::MessageId::from(nonzero(*message)?);
                let edited = bounded(
                    channel_id.edit_message(
                        http,
                        message,
                        serenity::EditMessage::new()
                            .content(text.clone())
                            .allowed_mentions(serenity::CreateAllowedMentions::new()),
                    ),
                )
                .await;
                match edited {
                    Ok(edited) => kept.push(edited.id.get()),
                    Err(error) if is_kind(&error, Kind::UnknownMessage) => kept.extend(
                        post(http, channel, text, serenity::CreateAllowedMentions::new()).await?,
                    ),
                    Err(error) => return Err(error),
                }
            }
            (None, Some(text)) => kept
                .extend(post(http, channel, text, serenity::CreateAllowedMentions::new()).await?),
            (Some(message), None) => {
                let message = serenity::MessageId::from(nonzero(*message)?);
                match bounded(channel_id.delete_message(http, message)).await {
                    Ok(()) => {}
                    Err(error) if is_kind(&error, Kind::UnknownMessage) => {}
                    Err(error) => return Err(error),
                }
            }
            (None, None) => {}
        }
    }
    Ok(kept)
}

pub(super) async fn message_created(
    data: &Data,
    serenity_context: &serenity::Context,
    message: &serenity::Message,
) {
    let Some(guild) = message.guild_id.map(|guild| GuildId::new(guild.get())) else {
        return;
    };
    if !checkable(message) {
        return;
    }
    let channel = ChannelId::new(message.channel_id.get());
    let hold = data.iq_unsettled.hold(channel, message.id.get());
    let scope = message_scope(guild, message);
    let competition = match data.iq.by_submissions_channel(channel).await {
        Ok(Some(competition)) if competition.guild == guild => competition,
        Ok(_) => return,
        Err(error) => {
            failure::report(
                "iq.submission.failed",
                "a message's IQ Competition could not be looked up",
                &Failure::from_error(&error),
                &scope,
            );
            hold.settle(Judged::Failed);
            return;
        }
    };
    check_message(
        data,
        &serenity_context.http,
        &competition,
        message,
        CheckPath::Live,
        hold,
        &scope,
    )
    .await;
}

fn checkable(message: &serenity::Message) -> bool {
    !message.author.bot
        && matches!(
            message.kind,
            serenity::MessageType::Regular | serenity::MessageType::InlineReply
        )
}

fn message_scope(guild: GuildId, message: &serenity::Message) -> Scope {
    Scope::default()
        .guild(guild)
        .channel(ChannelId::new(message.channel_id.get()))
        .user(UserId::new(message.author.id.get()))
        .message(Snowflake::new(message.id.get()))
}

async fn check_message(
    data: &Data,
    http: &serenity::Http,
    competition: &Competition,
    message: &serenity::Message,
    path: CheckPath,
    hold: iq::Hold<'_>,
    scope: &Scope,
) -> Judged {
    let judged = judge_message(data, http, competition, message, scope).await;
    hold.settle(judged);
    if iq::moves_last_seen(path, data.iq_caught_up.holds(competition.guild)) {
        seen(data, competition, message, scope).await;
    }
    judged
}

async fn seen(data: &Data, competition: &Competition, message: &serenity::Message, scope: &Scope) {
    let Some(ceiling) = data
        .iq_unsettled
        .mark_ceiling(competition.submissions, message.id.get())
    else {
        return;
    };
    if let Err(error) = data.iq.mark_seen(competition.id, ceiling).await {
        failure::report(
            "iq.submission.unseen",
            "the last message checked could not be saved",
            &Failure::from_error(&error),
            scope,
        );
    }
}

async fn judge_message(
    data: &Data,
    http: &serenity::Http,
    competition: &Competition,
    message: &serenity::Message,
    scope: &Scope,
) -> Judged {
    let sent_ms = Snowflake::new(message.id.get()).unix_millis();
    let sent_unix = i64::try_from(sent_ms / MILLIS_PER_SECOND).unwrap_or(i64::MAX);
    let end = match end_unix(competition) {
        Ok(end) => end,
        Err(error) => {
            failure::report(
                "iq.submission.failed",
                "a submission was not checked because the competition's end could not be worked out",
                &Failure::from_error(&error),
                scope,
            );
            return Judged::Failed;
        }
    };
    if !iq::should_check(competition.status, end, sent_unix) {
        return Judged::Ignored;
    }
    let attachments: Vec<iq::Attachment> = message
        .attachments
        .iter()
        .map(|attachment| iq::Attachment {
            content_type: attachment.content_type.clone(),
        })
        .collect();
    match iq::check_submission(&message.content, &attachments, &competition.criteria) {
        Checked::Valid { criterion, points } => {
            let recorded = data
                .iq
                .record_submission(
                    competition.id,
                    message.id.get(),
                    UserId::new(message.author.id.get()),
                    &criterion,
                    points,
                    i64::try_from(sent_ms).unwrap_or(i64::MAX),
                )
                .await;
            if let Err(error) = recorded {
                failure::report(
                    "iq.submission.failed",
                    "a valid submission could not be saved, so it was not marked",
                    &Failure::from_error(&error),
                    scope,
                );
                return Judged::Failed;
            }
            mark(http, message, iq::VALID_MARK, scope).await;
            failure::record("iq.submission.valid", "a submission was valid", scope);
            Judged::Valid
        }
        Checked::Invalid(problems) => {
            mark(http, message, iq::INVALID_MARK, scope).await;
            let reply = serenity::CreateMessage::new()
                .content(iq_text::submission_reply(&problems))
                .reference_message(message)
                .allowed_mentions(serenity::CreateAllowedMentions::new().replied_user(true));
            if let Err(error) = bounded(message.channel_id.send_message(http, reply)).await {
                failure::report(
                    "iq.submission.reply_failed",
                    "the reply to an invalid submission could not be sent",
                    &Failure::from_error(&error),
                    scope,
                );
            }
            failure::record("iq.submission.invalid", "a submission was invalid", scope);
            Judged::Invalid
        }
    }
}

async fn mark(http: &serenity::Http, message: &serenity::Message, emoji: &str, scope: &Scope) {
    let reacted = bounded(message.channel_id.create_reaction(
        http,
        message.id,
        serenity::ReactionType::Unicode(emoji.to_owned()),
    ))
    .await;
    if let Err(error) = reacted {
        failure::report(
            "iq.submission.mark_failed",
            "a submission's check or cross could not be added",
            &Failure::from_error(&error),
            scope,
        );
    }
}

fn same_emoji(found: &str, wanted: &str) -> bool {
    found.trim_end_matches(VARIATION_SELECTOR) == wanted.trim_end_matches(VARIATION_SELECTOR)
}

fn unicode_is(reaction: &serenity::ReactionType, wanted: &str) -> bool {
    match reaction {
        serenity::ReactionType::Unicode(found) => same_emoji(found, wanted),
        _ => false,
    }
}

pub(super) async fn reacted(
    data: &Data,
    serenity_context: &serenity::Context,
    reaction: &serenity::Reaction,
) {
    let (Some(guild), Some(user)) = (reaction.guild_id, reaction.user_id) else {
        return;
    };
    if user == serenity_context.cache.current_user().id {
        return;
    }
    let guild = GuildId::new(guild.get());
    let judge = UserId::new(user.get());
    let scope = Scope::default()
        .guild(guild)
        .channel(ChannelId::new(reaction.channel_id.get()))
        .user(judge)
        .message(Snowflake::new(reaction.message_id.get()));
    let channel = ChannelId::new(reaction.channel_id.get());
    let competition = match data.iq.by_submissions_channel(channel).await {
        Ok(Some(competition)) if competition.guild == guild => competition,
        Ok(_) => return,
        Err(error) => {
            failure::report(
                "iq.award.failed",
                "a reaction's IQ Competition could not be looked up",
                &Failure::from_error(&error),
                &scope,
            );
            return;
        }
    };
    if !unicode_is(&reaction.emoji, competition.reaction.emoji()) {
        return;
    }
    let judge_role = current_judge_role(data, &serenity_context.cache, guild, &scope).await;
    let holds = holds_judge_role(
        &serenity_context.http,
        judge_role,
        guild,
        judge,
        reaction.member.as_ref(),
        &scope,
    )
    .await;
    if !holds {
        failure::refused(
            "iq.award.refused",
            "a judging reaction came from a member without IQ-Judge",
            "not_a_judge",
            &scope,
        );
        return;
    }
    award(data, reaction.message_id.get(), judge, &scope).await;
}

async fn current_judge_role(
    data: &Data,
    cache: &serenity::Cache,
    guild: GuildId,
    scope: &Scope,
) -> Option<serenity::RoleId> {
    let stored = stored_judge_role(data, guild, scope).await;
    find_judge_role(cache, guild, stored).or_else(|| {
        stored
            .and_then(|role| NonZeroU64::new(role.get()))
            .map(serenity::RoleId::from)
    })
}

async fn holds_judge_role(
    http: &serenity::Http,
    judge_role: Option<serenity::RoleId>,
    guild: GuildId,
    user: UserId,
    member: Option<&serenity::Member>,
    scope: &Scope,
) -> bool {
    let Some(role) = judge_role else {
        return false;
    };
    let holds = |roles: &[serenity::RoleId]| roles.iter().any(|held| held.get() == role.get());
    if let Some(member) = member {
        return holds(&member.roles);
    }
    let (Ok(guild_id), Ok(user_id)) = (discord_guild(guild), discord_user(user)) else {
        return false;
    };
    match bounded(http.get_member(guild_id, user_id)).await {
        Ok(found) => holds(&found.roles),
        Err(error) => {
            failure::report(
                "iq.award.failed",
                "a reacting member's roles could not be read, so the reaction awarded nothing",
                &Failure::from_error(&error),
                scope,
            );
            false
        }
    }
}

async fn award(data: &Data, message: u64, judge: UserId, scope: &Scope) {
    let Some(now_ms) = super::now_ms().and_then(|now_ms| i64::try_from(now_ms).ok()) else {
        failure::report(
            "iq.award.failed",
            "a judging reaction awarded nothing because the clock reads before 1970",
            &Failure::from_error(&IqError::Clock),
            scope,
        );
        return;
    };
    match data.iq.award(message, judge, now_ms).await {
        Ok(AwardOutcome::Awarded { .. }) => failure::record(
            "iq.award.given",
            "a judge's reaction awarded a submission's points",
            scope,
        ),
        Ok(AwardOutcome::AlreadyAwarded) => failure::refused(
            "iq.award.refused",
            "a judging reaction came on a submission already awarded",
            "already_awarded",
            scope,
        ),
        Ok(AwardOutcome::NotASubmission) => failure::refused(
            "iq.award.refused",
            "a judging reaction came on a message that is not a valid submission",
            "not_a_submission",
            scope,
        ),
        Ok(AwardOutcome::Closed) => failure::refused(
            "iq.award.refused",
            "a judging reaction came after the competition was completed",
            "closed",
            scope,
        ),
        Err(error) => failure::report(
            "iq.award.failed",
            "a judging reaction could not be saved",
            &Failure::from_error(&error),
            scope,
        ),
    }
}

pub(super) async fn caught_up(
    data: &Data,
    serenity_context: &serenity::Context,
    ready: &serenity::Ready,
) {
    let connection = data.iq_caught_up.connect();
    for unavailable in &ready.guilds {
        let guild = GuildId::new(unavailable.id.get());
        let scope = Scope::default().guild(guild);
        match data.iq.live(guild).await {
            Ok(Some(competition)) if competition.status == Status::Running => {
                match catch_up(data, serenity_context, &competition, &scope).await {
                    Ok(true) => {
                        data.iq_caught_up.finish(guild, connection);
                        failure::record(
                            "iq.catch_up.completed",
                            "posts made while Barnacle was offline were checked",
                            &scope,
                        );
                    }
                    Ok(false) => {}
                    Err(error) => failure::report(
                        "iq.catch_up.failed",
                        "posts made while Barnacle was offline could not all be checked",
                        &Failure::from_error(&error),
                        &scope,
                    ),
                }
            }
            Ok(_) => data.iq_caught_up.finish(guild, connection),
            Err(error) => failure::report(
                "iq.catch_up.failed",
                "a server's IQ Competition could not be looked up after connecting",
                &Failure::from_error(&error),
                &scope,
            ),
        }
    }
}

async fn catch_up(
    data: &Data,
    serenity_context: &serenity::Context,
    competition: &Competition,
    scope: &Scope,
) -> Result<bool, IqError> {
    let http: &serenity::Http = &serenity_context.http;
    let judge_role =
        current_judge_role(data, &serenity_context.cache, competition.guild, scope).await;
    let channel = discord_channel(competition.submissions)?;
    let first = competition
        .last_seen
        .unwrap_or_else(|| competition.submissions.get());
    let mut after = first;
    for _ in 0..CATCH_UP_PAGES {
        let mut page = bounded(
            channel.messages(
                http,
                serenity::GetMessages::new()
                    .after(serenity::MessageId::from(nonzero(after)?))
                    .limit(PAGE_SIZE),
            ),
        )
        .await?;
        page.sort_by_key(|message| message.id);
        for message in &page {
            catch_up_one(data, http, competition, judge_role, message).await;
        }
        match page.last() {
            Some(last) if page.len() == usize::from(PAGE_SIZE) => after = last.id.get(),
            _ => return Ok(true),
        }
    }
    failure::refused(
        "iq.catch_up.stopped",
        "catch-up stopped at its page limit; the rest is checked on the next start",
        "page_limit",
        scope,
    );
    Ok(false)
}

async fn catch_up_one(
    data: &Data,
    http: &serenity::Http,
    competition: &Competition,
    judge_role: Option<serenity::RoleId>,
    message: &serenity::Message,
) {
    let scope = message_scope(competition.guild, message);
    if !checkable(message) || already_marked(message) {
        seen(data, competition, message, &scope).await;
        return;
    }
    let hold = data
        .iq_unsettled
        .hold(competition.submissions, message.id.get());
    let judged = check_message(
        data,
        http,
        competition,
        message,
        CheckPath::CatchUp,
        hold,
        &scope,
    )
    .await;
    if judged != Judged::Valid {
        return;
    }
    if let Some(judge) = reacted_judge(http, competition, judge_role, message, &scope).await {
        award(data, message.id.get(), judge, &scope.clone().user(judge)).await;
    }
}

fn already_marked(message: &serenity::Message) -> bool {
    message.reactions.iter().any(|reaction| {
        reaction.me
            && (unicode_is(&reaction.reaction_type, iq::VALID_MARK)
                || unicode_is(&reaction.reaction_type, iq::INVALID_MARK))
    })
}

async fn reacted_judge(
    http: &serenity::Http,
    competition: &Competition,
    judge_role: Option<serenity::RoleId>,
    message: &serenity::Message,
    scope: &Scope,
) -> Option<UserId> {
    let wanted = competition.reaction.emoji();
    let reaction = message
        .reactions
        .iter()
        .find(|reaction| reaction.count > 0 && unicode_is(&reaction.reaction_type, wanted))?;
    let users = bounded(message.channel_id.reaction_users(
        http,
        message.id,
        reaction.reaction_type.clone(),
        Some(PAGE_SIZE),
        None::<serenity::UserId>,
    ))
    .await;
    let users = match users {
        Ok(users) => users,
        Err(error) => {
            failure::report(
                "iq.catch_up.failed",
                "the judging reactions on a submission could not be read",
                &Failure::from_error(&error),
                scope,
            );
            return None;
        }
    };
    for user in users.iter().filter(|user| !user.bot) {
        let judge = UserId::new(user.id.get());
        if holds_judge_role(http, judge_role, competition.guild, judge, None, scope).await {
            return Some(judge);
        }
    }
    None
}

async fn update(
    serenity_context: &serenity::Context,
    component: &serenity::ComponentInteraction,
    content: String,
    components: Vec<serenity::CreateActionRow>,
    scope: &Scope,
) {
    let updated = component
        .create_response(
            serenity_context,
            serenity::CreateInteractionResponse::UpdateMessage(
                serenity::CreateInteractionResponseMessage::new()
                    .content(content)
                    .components(components)
                    .allowed_mentions(serenity::CreateAllowedMentions::new()),
            ),
        )
        .await;
    if let Err(error) = updated {
        failure::report(
            "interaction.failed",
            "an IQ Competition panel could not be redrawn",
            &Failure::from_error(&error),
            scope,
        );
    }
}

async fn edit_reply(
    serenity_context: &serenity::Context,
    component: &serenity::ComponentInteraction,
    content: String,
    scope: &Scope,
) {
    let edited = component
        .edit_response(
            serenity_context,
            serenity::EditInteractionResponse::new()
                .content(content)
                .components(Vec::new())
                .allowed_mentions(serenity::CreateAllowedMentions::new()),
        )
        .await;
    if let Err(error) = edited {
        failure::report(
            "interaction.failed",
            "the answer to an IQ Competition button could not be sent",
            &Failure::from_error(&error),
            scope,
        );
    }
}

async fn refuse_press(
    serenity_context: &serenity::Context,
    component: &serenity::ComponentInteraction,
    turned: &Turned,
    scope: &Scope,
) {
    let content = turned_text(turned, scope);
    tell(
        serenity_context,
        component,
        Answer::Response,
        &content,
        scope,
    )
    .await;
}

async fn open_modal(
    serenity_context: &serenity::Context,
    component: &serenity::ComponentInteraction,
    opened: Result<serenity::CreateModal, Turned>,
    scope: &Scope,
) {
    let modal = match opened {
        Ok(modal) => modal,
        Err(turned) => return refuse_press(serenity_context, component, &turned, scope).await,
    };
    let shown = component
        .create_response(
            serenity_context,
            serenity::CreateInteractionResponse::Modal(modal),
        )
        .await;
    if let Err(error) = shown {
        failure::report(
            "interaction.failed",
            "an IQ Competition panel could not open its form",
            &Failure::from_error(&error),
            scope,
        );
    }
}

async fn answer_modal(
    serenity_context: &serenity::Context,
    modal: &serenity::ModalInteraction,
    answer: Result<event_draft::PanelView, String>,
    scope: &Scope,
) {
    let response = match answer {
        Ok(view) => serenity::CreateInteractionResponse::UpdateMessage(
            serenity::CreateInteractionResponseMessage::new()
                .content(view.content)
                .components(view.components)
                .allowed_mentions(serenity::CreateAllowedMentions::new()),
        ),
        Err(content) => serenity::CreateInteractionResponse::Message(
            serenity::CreateInteractionResponseMessage::new()
                .content(content)
                .allowed_mentions(serenity::CreateAllowedMentions::new())
                .ephemeral(true),
        ),
    };
    if let Err(error) = modal.create_response(serenity_context, response).await {
        failure::report(
            "interaction.failed",
            "the answer to an IQ Competition form could not be sent",
            &Failure::from_error(&error),
            scope,
        );
    }
}

fn turned_text(turned: &Turned, scope: &Scope) -> String {
    match turned {
        Turned::Panel(refusal) => {
            failure::refused(
                "interaction.refused",
                "an IQ Competition panel press was refused",
                refusal.code(),
                scope,
            );
            refusal.message()
        }
        Turned::Setup(refusal) => {
            failure::refused(
                "interaction.refused",
                "an IQ Competition setup choice was refused",
                setup_code(refusal),
                scope,
            );
            iq_text::setup_refusal(refusal)
        }
    }
}

fn setup_code(refusal: &SetupRefusal) -> &'static str {
    match refusal {
        SetupRefusal::CriterionNameTaken(_) => "criterion_name_taken",
        SetupRefusal::CriterionPoints => "criterion_points",
        SetupRefusal::TooManyCriteria => "too_many_criteria",
        SetupRefusal::CriterionName => "criterion_name",
        SetupRefusal::CriterionDescription => "criterion_description",
        SetupRefusal::CriterionUnknown => "criterion_unknown",
        SetupRefusal::NameFormat => "name_format",
        SetupRefusal::RulesTooLong => "rules_too_long",
        SetupRefusal::TooManyRoles => "too_many_roles",
        SetupRefusal::TooManyJudges => "too_many_judges",
        SetupRefusal::TemplateUnknown => "template_unknown",
        SetupRefusal::DayNotOffered => "day_not_offered",
        SetupRefusal::LastDayNotAfterFirst => "last_day_not_after_first",
        SetupRefusal::ZoneUnknown => "zone_unknown",
        SetupRefusal::ReactionUnknown => "reaction_unknown",
        SetupRefusal::FixedAfterStart => "fixed_after_start",
        SetupRefusal::StepIncomplete(SetupStep::Basics) => "basics_incomplete",
        SetupRefusal::StepIncomplete(SetupStep::Criteria) => "criteria_incomplete",
        SetupRefusal::StepIncomplete(SetupStep::Rules) => "rules_incomplete",
        SetupRefusal::StepIncomplete(SetupStep::Who) => "who_incomplete",
        SetupRefusal::StepIncomplete(SetupStep::Review) => "review_incomplete",
    }
}

fn failed_text(summary: &str, error: &IqError, scope: &Scope) -> String {
    let failure = Failure::from_error(error);
    failure::report("interaction.failed", summary, &failure, scope);
    failure::failed(&iq_text::unfinished(&[]), &failure)
}

fn unfinished_text(summary: &str, unfinished: &Unfinished, scope: &Scope) -> String {
    let failure = Failure::from_error(&unfinished.error);
    failure::report("interaction.failed", summary, &failure, scope);
    failure::failed(&iq_text::unfinished(&unfinished.done), &failure)
}
