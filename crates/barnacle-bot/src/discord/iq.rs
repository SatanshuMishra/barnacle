use std::collections::BTreeMap;
use std::future::Future;
use std::num::NonZeroU64;
use std::time::Duration;

use barnacle_guess::Snowflake;
use barnacle_guess::UserId;
use poise::serenity_prelude as serenity;

use super::Context;
use super::Data;
use super::Error;
use super::access::organiser_check;
use super::access::organiser_pressed;
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
use crate::iq::JudgingMarks;
use crate::iq::Mark;
use crate::iq::Standing;
use crate::iq::StartChoice;
use crate::iq::Status;
use crate::iq_setup;
use crate::iq_setup::CompetitionChannels;
use crate::iq_setup::ConfigChange;
use crate::iq_setup::GuildRole;
use crate::iq_setup::MarkField;
use crate::iq_setup::Purpose;
use crate::iq_setup::SetupChecks;
use crate::iq_setup::SetupDraft;
use crate::iq_setup::SetupPlan;
use crate::iq_setup::SetupReady;
use crate::iq_setup::SetupRefusal;
use crate::iq_setup::SetupStep;
use crate::iq_store::AdjustOutcome;
use crate::iq_store::AwardOutcome;
use crate::iq_store::Competition;
use crate::iq_store::CreateOutcome;
use crate::iq_store::IqStoreError;
use crate::iq_text;
use crate::schedule;
use crate::schedule::Zone;
use crate::schedule::ZoneProblem;
use crate::text::Refusal;
use crate::text::escape;
use crate::voice::Overwrite;
use crate::voice::OverwriteTarget;
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
const UNREAD_CRITERION: &str = "a criterion that could not be read";
const MESSAGE_LINK: &str = "https://discord.com/channels";
const ANNOUNCED_LINES: usize = 5;
const MENU_MAX: u8 = 25;
const PAGE_SIZE: u8 = 100;
const CATCH_UP_PAGES: usize = 500;
const SHOWN_END_OFFSET: i64 = 60;
const MILLIS_PER_SECOND: u64 = 1_000;
const DISCORD_WAIT: Duration = Duration::from_secs(30);
const MARK_WAIT: Duration = Duration::from_secs(120);
const RESCAN_LIMIT: u32 = 500;
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
    NoJudgeRole,
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
            IqError::NoJudgeRole => formatter.write_str("the IQ-Judge role could not be found"),
        }
    }
}

impl std::error::Error for IqError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            IqError::Discord(error) => Some(error.as_ref()),
            IqError::TimedOut(error) => Some(error),
            IqError::Store(error) => Some(error.as_ref()),
            IqError::UnknownZone(_) | IqError::ZeroId | IqError::Clock | IqError::NoJudgeRole => {
                None
            }
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

#[poise::command(slash_command, rename = "start", guild_only, check = "organiser_check")]
pub(super) async fn iq_start(
    ctx: Context<'_>,
    #[description = "The time zone the competition ends in, remembered for next time"]
    #[autocomplete = "commands::suggest_zones"]
    #[max_length = 64]
    time_zone: Option<String>,
) -> Result<(), Error> {
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
        ) => open_setup(ctx, guild, time_zone.as_deref()).await,
    }
}

#[poise::command(slash_command, rename = "edit", guild_only, check = "organiser_check")]
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

#[poise::command(slash_command, rename = "stop", guild_only, check = "organiser_check")]
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
    check = "organiser_check"
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
        Err(CompleteRefusal::NotLive) => {
            refuse(
                ctx,
                "needs_cleanup",
                iq_text::needs_cleanup(&competition.name),
            )
            .await
        }
        Ok(completion) => {
            ctx.defer_ephemeral().await?;
            let standings = iq::standings(&ctx.data().iq.scores(competition.id).await?);
            let (named, points) =
                named_winners(&ctx.serenity_context().http, guild, &standings).await;
            let winner_names: Vec<&str> = named.iter().map(String::as_str).collect();
            let question = iq_text::complete_question(
                &competition.name,
                end,
                completion,
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
    check = "organiser_check"
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
    check = "organiser_check"
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
    reason: String,
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
    let reason = reason.trim();
    let target = UserId::new(member.id.get());
    let judge = author(ctx);
    let total = match data
        .iq
        .adjust(competition.id, target, points, reason, judge, now_ms)
        .await
    {
        Ok(AdjustOutcome::Adjusted { total }) => total,
        Ok(AdjustOutcome::BelowZero { total }) => {
            return refuse(ctx, "below_zero", iq_text::below_zero(target, total)).await;
        }
        Err(IqStoreError::ReasonMissing) => {
            return refuse(ctx, "reason_missing", iq_text::REASON_MISSING).await;
        }
        Err(IqStoreError::ReasonTooLong) => {
            return refuse(ctx, "reason_too_long", iq_text::REASON_TOO_LONG).await;
        }
        Err(error) => return Err(error.into()),
    };
    let scope = Scope::of_command(ctx);
    failure::record(
        "iq.points.adjusted",
        "a judge changed a member's points",
        &scope,
    );
    let replied = commands::private(ctx, iq_text::points_done(target, total)).await;
    log_judging(
        data,
        ctx.serenity_context(),
        &competition,
        &iq_text::judge_log_points(judge, target, points, reason, total),
        &scope,
    )
    .await;
    replied
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

async fn open_setup(ctx: Context<'_>, guild: GuildId, typed: Option<&str>) -> Result<(), Error> {
    let member = author(ctx);
    let given = commands::given_zone(typed);
    let remembered = ctx.data().zones.remembered(member).await?;
    let zone = match schedule::zone_for(given, remembered.as_deref()) {
        Ok(zone) => zone,
        Err(ZoneProblem::Missing) => {
            let needed = Refusal::ZoneNeeded;
            return refuse(ctx, needed.code(), needed.message()).await;
        }
        Err(ZoneProblem::Unknown(typed)) => {
            return commands::refuse_unknown_zone(ctx, &typed).await;
        }
    };
    if given.is_some() {
        commands::remember_zone(ctx, member, Some(&zone)).await;
    }
    let now_unix = super::now_unix();
    let templates = ctx.data().iq.templates(guild).await?;
    let Some(today) = zone.today(now_unix) else {
        return commands::fail(ctx, Failure::from_error(&IqError::Clock)).await;
    };
    let draft = SetupDraft::new(guild, member, templates, zone, today);
    open_draft(ctx, guild, draft).await
}

async fn open_draft(ctx: Context<'_>, guild: GuildId, draft: SetupDraft) -> Result<(), Error> {
    let Some(now_ms) = super::now_ms() else {
        return commands::fail(ctx, Failure::from_error(&IqError::Clock)).await;
    };
    let scope = Scope::of_command(ctx);
    let checks = setup_checks(ctx.serenity_context(), ctx.data(), guild, &scope).await;
    let judge_role = current_judge_role(ctx.data(), ctx.cache(), guild, &scope)
        .await
        .map(|role| RoleId::new(role.get()));
    let draft = draft.with_role_list(&guild_roles(ctx.cache(), guild), judge_role);
    let panel = ctx.id();
    let view = draft.panel(panel, super::now_unix(), &checks);
    ctx.data().iq_drafts.open(panel, draft, now_ms);
    ctx.send(event_draft::panel_reply(view)).await?;
    Ok(())
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

fn guild_roles(cache: &serenity::Cache, guild: GuildId) -> Vec<GuildRole> {
    NonZeroU64::new(guild.get())
        .and_then(|id| cache.guild(id))
        .map(|held| {
            held.roles
                .values()
                .map(|role| GuildRole {
                    id: RoleId::new(role.id.get()),
                    name: role.name.clone(),
                    position: role.position,
                    managed: role.managed,
                })
                .collect()
        })
        .unwrap_or_default()
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
                    match draft.step() {
                        SetupStep::Rules => iq_setup::rule_modal(panel, None),
                        _ => iq_setup::criterion_modal(panel, None),
                    }
                })
                .map_err(Turned::from);
            open_modal(serenity_context, component, opened, &scope).await;
        }
        IqPanelPart::Edit => {
            let opened = drafts
                .with(panel, user, now_ms)
                .map_err(Turned::from)
                .and_then(|draft| match draft.step() {
                    SetupStep::Rules => draft
                        .picked_rule()
                        .and_then(|index| draft.rules().get(index))
                        .map(|rule| iq_setup::rule_modal(panel, Some(rule)))
                        .ok_or(Turned::Setup(SetupRefusal::RuleUnknown)),
                    _ => draft
                        .picked()
                        .and_then(|index| draft.criteria().get(index))
                        .map(|criterion| iq_setup::criterion_modal(panel, Some(criterion)))
                        .ok_or(Turned::Setup(SetupRefusal::CriterionUnknown)),
                });
            open_modal(serenity_context, component, opened, &scope).await;
        }
        IqPanelPart::SetDates => {
            let opened = drafts
                .with(panel, user, now_ms)
                .map(|_| iq_setup::dates_modal(panel))
                .map_err(Turned::from);
            open_modal(serenity_context, component, opened, &scope).await;
        }
        IqPanelPart::DeniedMark | IqPanelPart::AwardedMark | IqPanelPart::ConfirmMark => {
            let Some(field) = iq_setup::mark_field(part) else {
                return;
            };
            pick_mark(
                data,
                serenity_context,
                component,
                panel,
                field,
                user,
                guild,
                &scope,
            )
            .await;
        }
        IqPanelPart::Template
        | IqPanelPart::Criterion
        | IqPanelPart::Rule
        | IqPanelPart::Remove
        | IqPanelPart::Roles
        | IqPanelPart::Judges
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
        | IqPanelPart::RuleModal
        | IqPanelPart::DatesModal => {
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
                IqPanelPart::RuleModal => {
                    draft.with_rule(&typed(iq_setup::RULE_INPUT), draft.picked_rule())
                }
                IqPanelPart::DatesModal => draft.with_typed_dates(
                    &typed(event_draft::FIRST_DAY_INPUT),
                    &typed(event_draft::LAST_DAY_INPUT),
                ),
                IqPanelPart::Template
                | IqPanelPart::SetDates
                | IqPanelPart::Rename
                | IqPanelPart::Criterion
                | IqPanelPart::Add
                | IqPanelPart::Edit
                | IqPanelPart::Remove
                | IqPanelPart::Rule
                | IqPanelPart::Roles
                | IqPanelPart::Judges
                | IqPanelPart::DeniedMark
                | IqPanelPart::AwardedMark
                | IqPanelPart::ConfirmMark
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
        (IqPanelPart::Criterion, serenity::ComponentInteractionDataKind::StringSelect { .. }) => {
            let index = single
                .parse::<usize>()
                .map_err(|_| SetupRefusal::CriterionUnknown)?;
            draft.with_picked_criterion(index)?
        }
        (IqPanelPart::Rule, serenity::ComponentInteractionDataKind::StringSelect { .. }) => {
            let index = single
                .parse::<usize>()
                .map_err(|_| SetupRefusal::RuleUnknown)?;
            draft.with_picked_rule(index)?
        }
        (IqPanelPart::Roles, serenity::ComponentInteractionDataKind::StringSelect { values }) => {
            draft.with_roles(
                values
                    .iter()
                    .filter_map(|value| value.parse::<u64>().ok())
                    .map(RoleId::new)
                    .collect(),
            )?
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
    if !organiser_pressed(data, serenity_context, component, guild, scope).await {
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
        SetupReady::Edit(change) => save_changes(data, serenity_context, change, user, scope).await,
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
    if !organiser_pressed(data, serenity_context, component, guild, scope).await {
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
            Ok(draft) => {
                save_judges(data, serenity_context, guild, user, draft.judges(), scope).await
            }
            Err(refusal) => Ok(turned_text(&Turned::Panel(refusal), scope)),
        },
        IqPanelPart::Template
        | IqPanelPart::SetDates
        | IqPanelPart::Rename
        | IqPanelPart::Criterion
        | IqPanelPart::Add
        | IqPanelPart::Edit
        | IqPanelPart::Remove
        | IqPanelPart::Rule
        | IqPanelPart::Roles
        | IqPanelPart::Judges
        | IqPanelPart::DeniedMark
        | IqPanelPart::AwardedMark
        | IqPanelPart::ConfirmMark
        | IqPanelPart::Next
        | IqPanelPart::Back
        | IqPanelPart::Start
        | IqPanelPart::Cancel
        | IqPanelPart::JudgesUsers
        | IqPanelPart::RenameModal
        | IqPanelPart::CriterionModal
        | IqPanelPart::RuleModal
        | IqPanelPart::DatesModal => Ok(turned_text(&Turned::Panel(Refusal::PanelUnknown), scope)),
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
    if let Err(CompleteRefusal::NotLive) =
        iq::may_complete(competition.status, end, super::now_unix())
    {
        return Ok(iq_text::needs_cleanup(&competition.name));
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
    let channels = competition.judge_log.into_iter().chain([
        competition.submissions,
        competition.announcements,
        competition.category,
    ]);
    for channel in channels {
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
    organiser: UserId,
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
            marks: competition.marks.clone(),
        })
        .await
        .map_err(|error| progress.stopped(error))?;
    failure::record(
        "iq.judges.saved",
        "the IQ-Judge role was handed out and taken away",
        scope,
    );
    log_judge_changes(
        data,
        serenity_context,
        &competition,
        organiser,
        judges,
        scope,
    )
    .await;
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
    let judge_log = create_judge_log(http, category, seats).await?;
    made.channels.push(judge_log);
    let channels = CompetitionChannels {
        category: ChannelId::new(category.get()),
        announcements: ChannelId::new(announcements.get()),
        submissions: ChannelId::new(submissions.get()),
        judge_log: ChannelId::new(judge_log.get()),
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
    organiser: UserId,
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
    if judges_changed {
        log_judge_changes(
            data,
            serenity_context,
            &before,
            organiser,
            &change.judges,
            scope,
        )
        .await;
    }
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
            failure::record("iq.submission.valid", "a submission was valid", scope);
            Judged::Valid
        }
        Checked::Invalid(problems) => {
            mark(
                http,
                message.channel_id,
                message.id,
                &competition.marks.denied,
                scope,
            )
            .await;
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

async fn mark(
    http: &serenity::Http,
    channel: serenity::ChannelId,
    message: serenity::MessageId,
    mark: &Mark,
    scope: &Scope,
) {
    let Some(reaction) = mark.reaction() else {
        failure::report(
            "iq.submission.mark_failed",
            "a submission's mark could not be added because its emoji ID is 0",
            &Failure::from_error(&IqError::ZeroId),
            scope,
        );
        return;
    };
    let reacted = bounded(channel.create_reaction(http, message, reaction)).await;
    if let Err(error) = reacted {
        failure::report(
            "iq.submission.mark_failed",
            "a submission's mark could not be added",
            &Failure::from_error(&error),
            scope,
        );
    }
}

fn is_mark(reaction: &serenity::ReactionType, wanted: &Mark) -> bool {
    Mark::from_reaction(reaction).is_some_and(|found| found == *wanted)
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
    let Some(found) = Mark::from_reaction(&reaction.emoji) else {
        return;
    };
    if found != competition.marks.awarded {
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
    if !iq::counts_as_award(&found, &competition.marks, holds) {
        failure::refused(
            "iq.award.refused",
            "a judging reaction came from a member without IQ-Judge",
            "not_a_judge",
            &scope,
        );
        return;
    }
    award(
        data,
        serenity_context,
        &competition,
        reaction.message_id.get(),
        judge,
        &scope,
    )
    .await;
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

async fn award(
    data: &Data,
    serenity_context: &serenity::Context,
    competition: &Competition,
    message: u64,
    judge: UserId,
    scope: &Scope,
) {
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
        Ok(AwardOutcome::Awarded { user, points }) => {
            failure::record(
                "iq.award.given",
                "a judge's reaction awarded a submission's points",
                scope,
            );
            awarded(
                data,
                serenity_context,
                competition,
                message,
                (judge, user, points),
                scope,
            )
            .await;
        }
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

async fn awarded(
    data: &Data,
    serenity_context: &serenity::Context,
    competition: &Competition,
    message: u64,
    (judge, player, points): (UserId, UserId, u32),
    scope: &Scope,
) {
    let http = &serenity_context.http;
    let target = discord_channel(competition.submissions).and_then(|channel| {
        nonzero(message).map(|message| (channel, serenity::MessageId::from(message)))
    });
    let (channel, message_id) = match target {
        Ok(target) => target,
        Err(error) => {
            failure::report(
                "iq.submission.mark_failed",
                "an awarded submission could not be marked",
                &Failure::from_error(&error),
                scope,
            );
            return;
        }
    };
    mark(http, channel, message_id, &competition.marks.confirm, scope).await;
    let criterion = match data.iq.criterion(message).await {
        Ok(Some(criterion)) => criterion,
        Ok(None) => UNREAD_CRITERION.to_owned(),
        Err(error) => {
            failure::report(
                "iq.award.unread",
                "an awarded submission's criterion could not be read, so it is not named",
                &Failure::from_error(&error),
                scope,
            );
            UNREAD_CRITERION.to_owned()
        }
    };
    let link = format!(
        "{MESSAGE_LINK}/{}/{}/{message}",
        competition.guild.get(),
        competition.submissions.get()
    );
    let line = iq_text::judge_log_award(judge, player, &criterion, i64::from(points), &link);
    log_judging(data, serenity_context, competition, &line, scope).await;
}

async fn log_judging(
    data: &Data,
    serenity_context: &serenity::Context,
    competition: &Competition,
    line: &str,
    scope: &Scope,
) {
    let posted = match judge_log_of(data, serenity_context, competition, scope).await {
        Ok(channel) => post(
            &serenity_context.http,
            channel,
            line,
            serenity::CreateAllowedMentions::new(),
        )
        .await
        .map(|_| ()),
        Err(error) => Err(error),
    };
    match posted {
        Ok(()) => failure::record(
            "iq.judge_log.posted",
            "a line was posted in the judge log",
            scope,
        ),
        Err(error) => {
            failure::report(
                "iq.judge_log.failed",
                "a judge log line could not be posted, so it goes to the bot's log",
                &Failure::from_error(&error),
                scope,
            );
            tracing::warn!(
                "barnacle.judge_log.line" = line,
                "a judge log line that could not be posted"
            );
        }
    }
}

static JUDGE_LOG_CREATION: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

async fn stored_judge_log(
    data: &Data,
    competition: &Competition,
) -> Result<Option<ChannelId>, IqError> {
    Ok(data
        .iq
        .competition(competition.id)
        .await?
        .map_or(competition.judge_log, |current| current.judge_log))
}

async fn create_judge_log(
    http: &serenity::Http,
    category: serenity::ChannelId,
    seats: Seats,
) -> Result<serenity::ChannelId, IqError> {
    Ok(bounded(
        seats.guild.create_channel(
            http,
            serenity::CreateChannel::new(iq_setup::JUDGE_LOG_NAME)
                .kind(serenity::ChannelType::Text)
                .category(category)
                .permissions(judge_log_permissions(seats)?),
        ),
    )
    .await?
    .id)
}

async fn judge_log_of(
    data: &Data,
    serenity_context: &serenity::Context,
    competition: &Competition,
    scope: &Scope,
) -> Result<ChannelId, IqError> {
    if let Some(channel) = stored_judge_log(data, competition).await? {
        return Ok(channel);
    }
    let _creating = JUDGE_LOG_CREATION.lock().await;
    if let Some(channel) = stored_judge_log(data, competition).await? {
        return Ok(channel);
    }
    let judge = current_judge_role(data, &serenity_context.cache, competition.guild, scope)
        .await
        .ok_or(IqError::NoJudgeRole)?;
    let guild = discord_guild(competition.guild)?;
    let seats = Seats {
        guild,
        judge,
        bot: serenity_context.cache.current_user().id,
    };
    let created = create_judge_log(
        &serenity_context.http,
        discord_channel(competition.category)?,
        seats,
    )
    .await?;
    let channel = ChannelId::new(created.get());
    if let Err(error) = data.iq.set_judge_log(competition.id, channel).await {
        failure::report(
            "iq.judge_log.unsaved",
            "the judge log channel was created but its ID was not saved",
            &Failure::from_error(&error),
            scope,
        );
    }
    failure::record(
        "iq.judge_log.created",
        "a judge log channel was created for a running competition",
        scope,
    );
    Ok(channel)
}

fn judge_log_permissions(seats: Seats) -> Result<Vec<serenity::PermissionOverwrite>, IqError> {
    iq_setup::judge_log_overwrites(
        RoleId::new(seats.guild.everyone_role().get()),
        RoleId::new(seats.judge.get()),
        UserId::new(seats.bot.get()),
    )
    .into_iter()
    .map(permission_overwrite)
    .collect()
}

fn permission_overwrite(overwrite: Overwrite) -> Result<serenity::PermissionOverwrite, IqError> {
    let kind = match overwrite.target {
        OverwriteTarget::Role(role) => serenity::PermissionOverwriteType::Role(discord_role(role)?),
        OverwriteTarget::Member(member) => {
            serenity::PermissionOverwriteType::Member(discord_user(member)?)
        }
    };
    Ok(serenity::PermissionOverwrite {
        allow: serenity::Permissions::from_bits_truncate(overwrite.allow),
        deny: serenity::Permissions::from_bits_truncate(overwrite.deny),
        kind,
    })
}

async fn log_judge_changes(
    data: &Data,
    serenity_context: &serenity::Context,
    competition: &Competition,
    organiser: UserId,
    after: &[UserId],
    scope: &Scope,
) {
    let before = &competition.judges;
    let added: Vec<UserId> = after
        .iter()
        .filter(|judge| !before.contains(judge))
        .copied()
        .collect();
    let removed: Vec<UserId> = before
        .iter()
        .filter(|judge| !after.contains(judge))
        .copied()
        .collect();
    if added.is_empty() && removed.is_empty() {
        return;
    }
    let line = iq_text::judge_log_judges(organiser, &added, &removed);
    log_judging(data, serenity_context, competition, &line, scope).await;
}

#[allow(clippy::too_many_arguments)]
async fn pick_mark(
    data: &Data,
    serenity_context: &serenity::Context,
    component: &serenity::ComponentInteraction,
    panel: u64,
    field: MarkField,
    user: UserId,
    guild: GuildId,
    scope: &Scope,
) {
    let drafts = &data.iq_drafts;
    let Some(asked_ms) = super::now_ms() else {
        let content = failed_text(
            "a mark button could not be used because the clock reads before 1970",
            &IqError::Clock,
            scope,
        );
        tell(
            serenity_context,
            component,
            Answer::Response,
            &content,
            scope,
        )
        .await;
        return;
    };
    if let Err(refusal) = drafts.with(panel, user, asked_ms) {
        refuse_press(serenity_context, component, &Turned::Panel(refusal), scope).await;
        return;
    }
    if !acknowledge(serenity_context, component, scope).await {
        return;
    }
    let prompt = serenity::CreateMessage::new()
        .content(iq_text::mark_prompt(user, field))
        .allowed_mentions(serenity::CreateAllowedMentions::new().users([component.user.id]));
    let sent = match bounded(
        component
            .channel_id
            .send_message(&serenity_context.http, prompt),
    )
    .await
    {
        Ok(sent) => sent,
        Err(error) => {
            let content = failed_text("a mark prompt could not be posted", &error, scope);
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
    };
    let picked = serenity::ReactionCollector::new(serenity_context)
        .message_id(sent.id)
        .author_id(component.user.id)
        .timeout(MARK_WAIT)
        .await;
    remove_prompt(&serenity_context.http, &sent, scope).await;
    let Some(mark) = picked.and_then(|reaction| Mark::from_reaction(&reaction.emoji)) else {
        failure::refused(
            "interaction.refused",
            "a mark prompt got no emoji in time",
            "mark_not_picked",
            scope,
        );
        tell(
            serenity_context,
            component,
            Answer::Followup,
            iq_text::NO_EMOJI_PICKED,
            scope,
        )
        .await;
        return;
    };
    let Some(now_ms) = super::now_ms() else {
        let content = failed_text(
            "a picked mark could not be saved because the clock reads before 1970",
            &IqError::Clock,
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
        return;
    };
    let changed = drafts
        .with(panel, user, now_ms)
        .map_err(|refusal| turned_text(&Turned::Panel(refusal), scope))
        .and_then(|draft| match draft.with_mark(field, mark.clone()) {
            Ok(next) => Ok(next),
            Err(SetupRefusal::MarkRepeated) => {
                let generic = turned_text(&Turned::Setup(SetupRefusal::MarkRepeated), scope);
                Err(mark_holder(draft.marks(), &mark, field)
                    .map_or(generic, iq_text::mark_repeated))
            }
            Err(refusal) => Err(turned_text(&Turned::Setup(refusal), scope)),
        });
    let next = match changed {
        Ok(next) => next,
        Err(content) => {
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
    };
    drafts.replace(panel, next.clone());
    let checks = setup_checks(serenity_context, data, guild, scope).await;
    let view = next.panel(panel, super::now_unix(), &checks);
    let edited = component
        .edit_response(
            serenity_context,
            serenity::EditInteractionResponse::new()
                .content(view.content)
                .components(view.components)
                .allowed_mentions(serenity::CreateAllowedMentions::new()),
        )
        .await;
    match edited {
        Ok(_) => failure::record("iq.mark.picked", "an IQ Competition mark was picked", scope),
        Err(error) => failure::report(
            "interaction.failed",
            "an IQ Competition panel could not be redrawn after a mark was picked",
            &Failure::from_error(&error),
            scope,
        ),
    }
}

fn mark_holder(marks: &JudgingMarks, mark: &Mark, field: MarkField) -> Option<MarkField> {
    [
        (MarkField::Denied, &marks.denied),
        (MarkField::Awarded, &marks.awarded),
        (MarkField::Confirm, &marks.confirm),
    ]
    .into_iter()
    .find(|(held_field, held)| *held_field != field && *held == mark)
    .map(|(held_field, _)| held_field)
}

async fn remove_prompt(http: &serenity::Http, prompt: &serenity::Message, scope: &Scope) {
    match bounded(prompt.channel_id.delete_message(http, prompt.id)).await {
        Ok(()) => {}
        Err(error) if is_kind(&error, Kind::UnknownMessage) => {}
        Err(error) => failure::report(
            "iq.mark.prompt_kept",
            "a mark prompt could not be deleted",
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
                let caught = catch_up(data, serenity_context, &competition, &scope).await;
                rescan(data, serenity_context, &competition, &scope).await;
                match caught {
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
            catch_up_one(data, serenity_context, competition, judge_role, message).await;
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
    serenity_context: &serenity::Context,
    competition: &Competition,
    judge_role: Option<serenity::RoleId>,
    message: &serenity::Message,
) {
    let http: &serenity::Http = &serenity_context.http;
    let scope = message_scope(competition.guild, message);
    let hold = data
        .iq_unsettled
        .hold(competition.submissions, message.id.get());
    let recorded = match data.iq.recorded(competition.id, message.id.get()).await {
        Ok(recorded) => recorded,
        Err(error) => {
            failure::report(
                "iq.catch_up.unread",
                "a submission could not be looked up, so catch-up left it for the next start",
                &Failure::from_error(&error),
                &scope,
            );
            hold.settle(Judged::Failed);
            return;
        }
    };
    if !iq::checks_at_catch_up(
        checkable(message),
        already_marked(message, &competition.marks),
        recorded,
    ) {
        hold.settle(Judged::Ignored);
        seen(data, competition, message, &scope).await;
        return;
    }
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
        award(
            data,
            serenity_context,
            competition,
            message.id.get(),
            judge,
            &scope.clone().user(judge),
        )
        .await;
    }
}

async fn rescan(
    data: &Data,
    serenity_context: &serenity::Context,
    competition: &Competition,
    scope: &Scope,
) {
    let unawarded = match data.iq.unawarded(competition.id, RESCAN_LIMIT).await {
        Ok(unawarded) => unawarded,
        Err(error) => {
            failure::report(
                "iq.rescan.failed",
                "the unawarded submissions could not be listed, so offline judging was not counted",
                &Failure::from_error(&error),
                scope,
            );
            return;
        }
    };
    let channel = match discord_channel(competition.submissions) {
        Ok(channel) => channel,
        Err(error) => {
            failure::report(
                "iq.rescan.failed",
                "the submissions channel's stored ID is 0, so offline judging was not counted",
                &Failure::from_error(&error),
                scope,
            );
            return;
        }
    };
    let judge_role =
        current_judge_role(data, &serenity_context.cache, competition.guild, scope).await;
    for message in &unawarded {
        rescan_one(
            data,
            serenity_context,
            competition,
            judge_role,
            channel,
            *message,
            &scope.clone().message(Snowflake::new(*message)),
        )
        .await;
    }
    if u32::try_from(unawarded.len()).is_ok_and(|listed| listed == RESCAN_LIMIT) {
        failure::refused(
            "iq.rescan.stopped",
            "the offline judging check stopped at 500 unawarded submissions; more may remain",
            "rescan_limit",
            scope,
        );
    }
}

async fn rescan_one(
    data: &Data,
    serenity_context: &serenity::Context,
    competition: &Competition,
    judge_role: Option<serenity::RoleId>,
    channel: serenity::ChannelId,
    message: u64,
    scope: &Scope,
) {
    let http = &serenity_context.http;
    let Some(message_id) = NonZeroU64::new(message).map(serenity::MessageId::from) else {
        return;
    };
    let found = match bounded(channel.message(http, message_id)).await {
        Ok(found) => found,
        Err(error) if is_kind(&error, Kind::UnknownMessage) => {
            match data.iq.forget(competition.id, message).await {
                Ok(()) => failure::record(
                    "iq.rescan.forgotten",
                    "a deleted submission left the offline judging check",
                    scope,
                ),
                Err(error) => failure::report(
                    "iq.rescan.failed",
                    "a deleted submission could not be dropped from the offline judging check",
                    &Failure::from_error(&error),
                    scope,
                ),
            }
            return;
        }
        Err(error) => {
            failure::report(
                "iq.rescan.skipped",
                "a submission could not be read, so its offline judging was skipped",
                &Failure::from_error(&error),
                scope,
            );
            return;
        }
    };
    if let Some(judge) = reacted_judge(http, competition, judge_role, &found, scope).await {
        award(
            data,
            serenity_context,
            competition,
            message,
            judge,
            &scope.clone().user(judge),
        )
        .await;
    }
}

fn already_marked(message: &serenity::Message, marks: &JudgingMarks) -> bool {
    message.reactions.iter().any(|reaction| {
        reaction.me
            && (is_mark(&reaction.reaction_type, &marks.denied)
                || is_mark(&reaction.reaction_type, &marks.confirm))
    })
}

async fn reacted_judge(
    http: &serenity::Http,
    competition: &Competition,
    judge_role: Option<serenity::RoleId>,
    message: &serenity::Message,
    scope: &Scope,
) -> Option<UserId> {
    let wanted = &competition.marks.awarded;
    let reaction = message
        .reactions
        .iter()
        .find(|reaction| reaction.count > 0 && is_mark(&reaction.reaction_type, wanted))?;
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
        SetupRefusal::RuleLength => "rule_length",
        SetupRefusal::TooManyRules => "too_many_rules",
        SetupRefusal::RuleUnknown => "rule_unknown",
        SetupRefusal::TooManyRoles => "too_many_roles",
        SetupRefusal::TooManyJudges => "too_many_judges",
        SetupRefusal::TemplateUnknown => "template_unknown",
        SetupRefusal::DateFormat => "date_format",
        SetupRefusal::DayNotOffered => "day_not_offered",
        SetupRefusal::LastDayNotAfterFirst => "last_day_not_after_first",
        SetupRefusal::DatesTooFar => "dates_too_far",
        SetupRefusal::MarkRepeated => "mark_repeated",
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
