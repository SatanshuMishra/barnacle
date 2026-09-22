use std::path::PathBuf;
use std::process::ExitCode;

use barnacle_bot::discord;
use barnacle_bot::failure::Failure;
use barnacle_bot::logging;
use barnacle_bot::logging::LogFormat;
use barnacle_bot::logging::LogSettings;
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
    let settings = match LogSettings::from_lookup(|name| std::env::var(name).ok()) {
        Ok(settings) => settings,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::FAILURE;
        }
    };
    if let Err(error) = logging::init(&settings) {
        eprintln!("{error}");
        return ExitCode::FAILURE;
    }
    match run(Cli::parse()).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(AppError::Startup(error)) => {
            fail(&settings.format, "startup.failed", &error);
            ExitCode::FAILURE
        }
        Err(AppError::Run(error)) => {
            fail(&settings.format, "service.failed", &error);
            ExitCode::FAILURE
        }
    }
}

fn fail(format: &LogFormat, event: &'static str, error: &(dyn std::error::Error + 'static)) {
    let description = startup::describe(error);
    match format {
        LogFormat::Text => eprintln!("{description}"),
        LogFormat::Json => {
            let failure = Failure::from_error(error);
            let lines: Vec<&str> = description.lines().map(str::trim).collect();
            tracing::error!(
                "event.name" = event,
                "event.outcome" = "failure",
                "error.type" = failure.kind.as_str(),
                "exception.message" = lines.join(" | "),
                "{}",
                lines.first().copied().unwrap_or_default()
            );
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
