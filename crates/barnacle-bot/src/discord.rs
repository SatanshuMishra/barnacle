mod announcer;
mod board;
mod commands;
mod events;
mod rooms;

use std::sync::Arc;
use std::time::Duration;

use barnacle_catalog::Catalog;
use barnacle_catalog::curation::Curated;
use barnacle_catalog::store::CatalogRoot;
use barnacle_guess::Snowflake;
use barnacle_guess::Timing;
use poise::serenity_prelude as serenity;
use rand::rngs::StdRng;
use tokio::sync::Semaphore;

use crate::attendance::Signups;
use crate::attendance_store::Attendance;
use crate::config::CommandScope;
use crate::failure;
use crate::failure::Failure;
use crate::failure::Scope;
use crate::ids::GuildId;
use crate::logging;
use crate::lookup::Directory;
use crate::solves::Solves;
use crate::startup::Loaded;
use crate::table::Table;
use crate::voice::VoiceRooms;
use crate::voice_store::VoiceStore;

pub use announcer::DiscordAnnouncer;
pub use board::DiscordBoard;
pub use board::allowed_mentions;
pub use rooms::DiscordRooms;

const MEMBER_LOOKUPS_AT_ONCE: usize = 5;
const TICK: Duration = Duration::from_secs(60);
const ROOM_SWEEP: Duration = Duration::from_secs(30);

pub type Error = Box<dyn std::error::Error + Send + Sync>;
pub type Context<'a> = poise::Context<'a, Data, Error>;

pub struct Data {
    pub table: Arc<Table<DiscordAnnouncer, Solves>>,
    pub root: CatalogRoot,
    pub catalog_name: String,
    pub catalog: Catalog,
    pub curated: Curated,
    pub directory: Directory,
    pub member_lookups: Arc<Semaphore>,
    pub signups: Arc<Signups<DiscordBoard>>,
    pub rehearsal: Vec<GuildId>,
    pub voice: Arc<VoiceRooms<DiscordRooms>>,
}

fn now_unix() -> i64 {
    jiff::Timestamp::now().as_second()
}

fn now_ms() -> Option<u64> {
    u64::try_from(jiff::Timestamp::now().as_millisecond()).ok()
}

#[derive(Debug, thiserror::Error)]
pub enum RunError {
    #[error(
        "Discord refused the Message Content intent; turn on Message Content Intent under Bot, Privileged Gateway Intents, in the Discord Developer Portal"
    )]
    MessageContentDisabled,
    #[error("Discord refused to register the commands")]
    Registration(#[source] serenity::Error),
    #[error("the Discord client failed")]
    Discord(#[source] serenity::Error),
}

impl From<serenity::Error> for RunError {
    fn from(error: serenity::Error) -> Self {
        match error {
            serenity::Error::Gateway(serenity::GatewayError::DisallowedGatewayIntents) => {
                Self::MessageContentDisabled
            }
            other => Self::Discord(other),
        }
    }
}

pub async fn run(
    token: String,
    scope: CommandScope,
    rehearsal: Vec<u64>,
    loaded: Loaded,
    solves: Solves,
    attendance: Attendance,
    voice: VoiceStore,
) -> Result<(), RunError> {
    let intents = serenity::GatewayIntents::GUILDS
        | serenity::GatewayIntents::GUILD_MESSAGES
        | serenity::GatewayIntents::MESSAGE_CONTENT
        | serenity::GatewayIntents::GUILD_VOICE_STATES;
    let rehearsal_guilds: Vec<GuildId> = rehearsal.iter().copied().map(GuildId::new).collect();
    let framework = poise::Framework::builder()
        .options(poise::FrameworkOptions {
            commands: command_list(!rehearsal.is_empty()),
            event_handler: |framework, event| Box::pin(events::handle(framework, event)),
            on_error: |error| Box::pin(events::on_error(error)),
            post_command: |ctx| Box::pin(completed(ctx)),
            ..Default::default()
        })
        .setup(move |ctx, ready, _framework| {
            Box::pin(async move {
                let announcer = DiscordAnnouncer::new(Arc::clone(&ctx.http));
                let rng: StdRng = rand::make_rng();
                let table = Table::new(loaded.book, solves, announcer, Timing::STANDARD, rng);
                let board = DiscordBoard::new(Arc::clone(&ctx.http), ready.user.id);
                let signups = Signups::rehearsing(board, attendance, rehearsal_guilds.clone());
                match signups
                    .store()
                    .clear_rehearsal_clocks_except(&rehearsal_guilds)
                    .await
                {
                    Ok(cleared) => tracing::info!(
                        "event.name" = "rehearsal.clock.cleared",
                        "event.outcome" = "success",
                        "barnacle.cleared" = cleared,
                        "rehearsal clocks cleared for unlisted servers"
                    ),
                    Err(error) => failure::report(
                        "rehearsal.clock.clear_failed",
                        "stale rehearsal clocks could not be cleared",
                        &Failure::from_error(&error),
                        &Scope::default(),
                    ),
                }
                let ticker = Arc::clone(&signups);
                tokio::spawn(async move {
                    let mut beat = tokio::time::interval(TICK);
                    beat.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
                    loop {
                        beat.tick().await;
                        let Some(now_ms) = now_ms() else {
                            failure::report(
                                "signup.tick.failed",
                                "the clock reads before 1970, so this beat was skipped",
                                &Failure::internal("the clock reads before 1970"),
                                &Scope::default(),
                            );
                            continue;
                        };
                        let beating = Arc::clone(&ticker);
                        let beat_result =
                            tokio::spawn(async move { beating.tick(now_unix(), now_ms).await })
                                .await;
                        match beat_result {
                            Ok(report) => tracing::debug!(
                                "event.name" = "signup.tick.completed",
                                "event.outcome" = "success",
                                "barnacle.failures" = report.failures,
                                "a sign-up beat ran"
                            ),
                            Err(error) => failure::report(
                                "signup.tick.failed",
                                "a sign-up beat died; the next beat will be attempted",
                                &Failure::from_error(&error),
                                &Scope::default(),
                            ),
                        }
                    }
                });
                let voice = VoiceRooms::new(
                    DiscordRooms::new(Arc::clone(&ctx.http), ready.user.id),
                    voice,
                );
                let sweeper = Arc::clone(&voice);
                let cache = Arc::clone(&ctx.cache);
                tokio::spawn(async move {
                    let mut beat = tokio::time::interval(ROOM_SWEEP);
                    beat.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
                    loop {
                        beat.tick().await;
                        let Some(now_ms) = now_ms() else {
                            failure::report(
                                "voice.sweep.failed",
                                "the clock reads before 1970, so this sweep was skipped",
                                &Failure::internal("the clock reads before 1970"),
                                &Scope::default(),
                            );
                            continue;
                        };
                        let sweeping = Arc::clone(&sweeper);
                        let cache = Arc::clone(&cache);
                        let sweep_result = tokio::spawn(async move {
                            let occupancy =
                                |guild, channel| rooms::occupancy(&cache, guild, channel);
                            sweeping.sweep(&occupancy, now_ms).await
                        })
                        .await;
                        match sweep_result {
                            Ok(report) => tracing::debug!(
                                "event.name" = "voice.sweep.completed",
                                "event.outcome" = "success",
                                "barnacle.failures" = report.failures,
                                "a room sweep ran"
                            ),
                            Err(error) => failure::report(
                                "voice.sweep.failed",
                                "a room sweep died; the next sweep will be attempted",
                                &Failure::from_error(&error),
                                &Scope::default(),
                            ),
                        }
                    }
                });
                let data = Data {
                    table,
                    root: loaded.root,
                    catalog_name: loaded.catalog_name,
                    catalog: loaded.catalog,
                    curated: loaded.curated,
                    directory: loaded.directory,
                    member_lookups: Arc::new(Semaphore::new(MEMBER_LOOKUPS_AT_ONCE)),
                    signups,
                    rehearsal: rehearsal_guilds,
                    voice,
                };
                tracing::info!(
                    "event.name" = "service.started",
                    "event.outcome" = "success",
                    "service.name" = "barnacle",
                    "service.version" = env!("CARGO_PKG_VERSION"),
                    "barnacle.catalog" = data.catalog_name.as_str(),
                    "barnacle.log.schema" = logging::SCHEMA,
                    "barnacle.guilds" = ready.guilds.len(),
                    "Barnacle started"
                );
                Ok::<Data, Error>(data)
            })
        })
        .build();
    let mut client = serenity::ClientBuilder::new(token, intents)
        .framework(framework)
        .await?;
    let application = client.http.get_current_application_info().await?;
    client.http.set_application_id(application.id);
    register(&client.http, &scope, &rehearsal)
        .await
        .map_err(RunError::Registration)?;
    client.start().await?;
    Ok(())
}

async fn completed(ctx: Context<'_>) {
    let scope = Scope::of_command(ctx);
    let started_ms = Snowflake::new(ctx.id()).unix_millis();
    tracing::info!(
        "event.name" = "command.completed",
        "event.outcome" = "success",
        "discord.guild.id" = scope
            .guild
            .map(|guild| tracing::field::display(guild.get())),
        "discord.channel.id" = scope
            .channel
            .map(|channel| tracing::field::display(channel.get())),
        "discord.user.id" = scope.user.map(|user| tracing::field::display(user.get())),
        "discord.interaction.id" = scope.interaction.map(tracing::field::display),
        "discord.command.name" = scope.command.as_deref(),
        "barnacle.duration_ms" = now_ms().map(|now_ms| now_ms.saturating_sub(started_ms)),
        "a command finished"
    );
}

pub fn command_list(rehearsing: bool) -> Vec<poise::Command<Data, Error>> {
    let extra = if rehearsing {
        commands::rehearsal()
    } else {
        Vec::new()
    };
    commands::all().into_iter().chain(extra).collect()
}

pub fn command_list_for(guild: u64, rehearsal: &[u64]) -> Vec<poise::Command<Data, Error>> {
    command_list(rehearsal.contains(&guild))
}

async fn register(
    http: &Arc<serenity::Http>,
    scope: &CommandScope,
    rehearsal: &[u64],
) -> Result<(), serenity::Error> {
    match scope {
        CommandScope::Global => {
            poise::builtins::register_globally(http, &command_list(false)).await?;
            failure::record(
                "commands.registered",
                "commands registered globally",
                &Scope::default(),
            );
        }
        CommandScope::Guilds { guilds } => {
            tracing::debug!(
                count = rehearsal.len(),
                "rehearsal servers named in the config"
            );
            for guild in guilds {
                poise::builtins::register_in_guild(
                    http,
                    &command_list_for(*guild, rehearsal),
                    serenity::GuildId::new(*guild),
                )
                .await?;
                failure::record(
                    "commands.registered",
                    "commands registered in a server",
                    &Scope::default().guild(GuildId::new(*guild)),
                );
            }
        }
    }
    Ok(())
}
