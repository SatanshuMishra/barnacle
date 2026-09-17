mod announcer;
mod board;
mod commands;
mod events;

use std::sync::Arc;
use std::time::Duration;

use barnacle_catalog::Catalog;
use barnacle_catalog::curation::Curated;
use barnacle_catalog::store::CatalogRoot;
use barnacle_guess::Timing;
use poise::serenity_prelude as serenity;
use rand::rngs::StdRng;
use tokio::sync::Semaphore;

use crate::attendance::Signups;
use crate::attendance_store::Attendance;
use crate::config::CommandScope;
use crate::lookup::Directory;
use crate::solves::Solves;
use crate::startup::Loaded;
use crate::table::Table;

pub use announcer::DiscordAnnouncer;
pub use board::DiscordBoard;

const MEMBER_LOOKUPS_AT_ONCE: usize = 5;
const TICK: Duration = Duration::from_secs(60);

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
    loaded: Loaded,
    solves: Solves,
    attendance: Attendance,
) -> Result<(), RunError> {
    let intents = serenity::GatewayIntents::GUILDS
        | serenity::GatewayIntents::GUILD_MESSAGES
        | serenity::GatewayIntents::MESSAGE_CONTENT;
    let framework = poise::Framework::builder()
        .options(poise::FrameworkOptions {
            commands: commands::all(),
            event_handler: |framework, event| Box::pin(events::handle(framework, event)),
            on_error: |error| Box::pin(events::on_error(error)),
            ..Default::default()
        })
        .setup(move |ctx, ready, _framework| {
            Box::pin(async move {
                let announcer = DiscordAnnouncer::new(Arc::clone(&ctx.http));
                let rng: StdRng = rand::make_rng();
                let table = Table::new(loaded.book, solves, announcer, Timing::STANDARD, rng);
                let board = DiscordBoard::new(Arc::clone(&ctx.http), ready.user.id);
                let signups = Signups::new(board, attendance);
                let ticker = Arc::clone(&signups);
                tokio::spawn(async move {
                    let mut beat = tokio::time::interval(TICK);
                    beat.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
                    loop {
                        beat.tick().await;
                        let Some(now_ms) = now_ms() else {
                            tracing::warn!("the clock reads before 1970, so this beat was skipped");
                            continue;
                        };
                        let beating = Arc::clone(&ticker);
                        let beat_result =
                            tokio::spawn(async move { beating.tick(now_unix(), now_ms).await })
                                .await;
                        match beat_result {
                            Ok(report) if report.failures > 0 => tracing::warn!(
                                failures = report.failures,
                                "a sign-up step failed and will be retried"
                            ),
                            Ok(_) => {}
                            Err(error) => tracing::error!(
                                %error,
                                "a sign-up beat died; the next beat will be attempted"
                            ),
                        }
                    }
                });
                Ok::<Data, Error>(Data {
                    table,
                    root: loaded.root,
                    catalog_name: loaded.catalog_name,
                    catalog: loaded.catalog,
                    curated: loaded.curated,
                    directory: loaded.directory,
                    member_lookups: Arc::new(Semaphore::new(MEMBER_LOOKUPS_AT_ONCE)),
                    signups,
                })
            })
        })
        .build();
    let mut client = serenity::ClientBuilder::new(token, intents)
        .framework(framework)
        .await?;
    let application = client.http.get_current_application_info().await?;
    client.http.set_application_id(application.id);
    register(&client.http, &commands::all(), &scope)
        .await
        .map_err(RunError::Registration)?;
    client.start().await?;
    Ok(())
}

async fn register(
    http: &Arc<serenity::Http>,
    commands: &[poise::Command<Data, Error>],
    scope: &CommandScope,
) -> Result<(), serenity::Error> {
    match scope {
        CommandScope::Global => {
            poise::builtins::register_globally(http, commands).await?;
            tracing::info!("commands registered globally");
        }
        CommandScope::Guilds { guilds } => {
            for guild in guilds {
                poise::builtins::register_in_guild(http, commands, serenity::GuildId::new(*guild))
                    .await?;
                tracing::info!(guild, "commands registered in server");
            }
        }
    }
    Ok(())
}
