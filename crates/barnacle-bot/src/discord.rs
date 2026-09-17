mod announcer;
mod commands;
mod events;

use std::sync::Arc;

use barnacle_catalog::Catalog;
use barnacle_catalog::curation::Curated;
use barnacle_catalog::store::CatalogRoot;
use barnacle_guess::Timing;
use poise::serenity_prelude as serenity;
use rand::rngs::StdRng;
use tokio::sync::Semaphore;

use crate::config::CommandScope;
use crate::lookup::Directory;
use crate::solves::Solves;
use crate::startup::Loaded;
use crate::table::Table;

pub use announcer::DiscordAnnouncer;

const MEMBER_LOOKUPS_AT_ONCE: usize = 5;

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
        .setup(move |ctx, _ready, _framework| {
            Box::pin(async move {
                let announcer = DiscordAnnouncer::new(Arc::clone(&ctx.http));
                let rng: StdRng = rand::make_rng();
                let table = Table::new(loaded.book, solves, announcer, Timing::STANDARD, rng);
                Ok::<Data, Error>(Data {
                    table,
                    root: loaded.root,
                    catalog_name: loaded.catalog_name,
                    catalog: loaded.catalog,
                    curated: loaded.curated,
                    directory: loaded.directory,
                    member_lookups: Arc::new(Semaphore::new(MEMBER_LOOKUPS_AT_ONCE)),
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
