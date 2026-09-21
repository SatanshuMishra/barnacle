use std::path::PathBuf;
use std::process::ExitCode;

use barnacle_bot::discord;
use barnacle_bot::startup;
use clap::Parser;

#[derive(Parser)]
#[command(
    version,
    about = "Barnacle, a Discord bot for World of Warships players"
)]
struct Cli {
    #[arg(long, default_value = "barnacle.toml")]
    config: PathBuf,
}

#[derive(Debug, thiserror::Error)]
enum AppError {
    #[error(transparent)]
    Startup(#[from] startup::StartupError),
    #[error(transparent)]
    Run(#[from] discord::RunError),
}

#[tokio::main]
async fn main() -> ExitCode {
    tracing_subscriber::fmt().init();
    match run(Cli::parse()).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(AppError::Startup(error)) => {
            eprintln!("{}", startup::describe(&error));
            ExitCode::FAILURE
        }
        Err(AppError::Run(error)) => {
            eprintln!("{}", startup::describe(&error));
            ExitCode::FAILURE
        }
    }
}

async fn run(cli: Cli) -> Result<(), AppError> {
    let config = startup::read_config(&cli.config)?;
    startup::load_env()?;
    let token = startup::read_token(std::env::var("DISCORD_TOKEN").ok())?;
    let loaded = startup::load(&config)?;
    let stores = startup::open_stores(&config.database).await?;
    discord::run(
        token,
        config.commands,
        config.rehearsal,
        loaded,
        stores.solves,
        stores.attendance,
        stores.voice,
    )
    .await?;
    Ok(())
}
