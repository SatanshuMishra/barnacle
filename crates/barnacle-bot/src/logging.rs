use std::io::IsTerminal;

use tracing_subscriber::EnvFilter;
use tracing_subscriber::fmt::MakeWriter;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::util::TryInitError;

pub const SCHEMA: &str = "1";

const FORMAT_VARIABLE: &str = "BARNACLE_LOG_FORMAT";
const LEVEL_VARIABLE: &str = "BARNACLE_LOG_LEVEL";
const DEFAULT_LEVEL: &str = "info";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LogFormat {
    Text,
    Json,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogSettings {
    pub format: LogFormat,
    pub level: String,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LogSettingsError {
    #[error("BARNACLE_LOG_FORMAT is {value:?}; it must be text or json")]
    Format { value: String },
    #[error(
        "BARNACLE_LOG_LEVEL is {value:?}, which is not a level filter such as info or info,barnacle_bot=debug"
    )]
    Level { value: String },
}

#[derive(Debug, thiserror::Error)]
#[error("the log output could not be set up")]
pub struct LogInitError(#[from] TryInitError);

impl LogSettings {
    pub fn from_lookup(
        lookup: impl Fn(&str) -> Option<String>,
    ) -> Result<LogSettings, LogSettingsError> {
        let format = lookup(FORMAT_VARIABLE).map_or(Ok(LogFormat::Text), |value| format(&value))?;
        let level =
            lookup(LEVEL_VARIABLE).map_or(Ok(DEFAULT_LEVEL.to_owned()), |value| level(&value))?;
        Ok(LogSettings { format, level })
    }
}

fn format(value: &str) -> Result<LogFormat, LogSettingsError> {
    match value.trim().to_ascii_lowercase().as_str() {
        "" | "text" => Ok(LogFormat::Text),
        "json" => Ok(LogFormat::Json),
        _ => Err(LogSettingsError::Format {
            value: value.to_owned(),
        }),
    }
}

fn level(value: &str) -> Result<String, LogSettingsError> {
    let directives = match value.trim() {
        "" => DEFAULT_LEVEL,
        directives => directives,
    };
    EnvFilter::try_new(directives)
        .map(|_| directives.to_owned())
        .map_err(|_| LogSettingsError::Level {
            value: value.to_owned(),
        })
}

pub fn subscriber<W>(
    settings: &LogSettings,
    writer: W,
    ansi: bool,
) -> Box<dyn tracing::Subscriber + Send + Sync>
where
    W: for<'w> MakeWriter<'w> + Send + Sync + 'static,
{
    let filter = EnvFilter::new(&settings.level);
    match settings.format {
        LogFormat::Text => Box::new(
            tracing_subscriber::fmt()
                .with_env_filter(filter)
                .with_writer(writer)
                .with_ansi(ansi)
                .finish(),
        ),
        LogFormat::Json => Box::new(
            tracing_subscriber::fmt()
                .json()
                .flatten_event(true)
                .with_current_span(false)
                .with_span_list(false)
                .with_env_filter(filter)
                .with_writer(writer)
                .with_ansi(false)
                .finish(),
        ),
    }
}

pub fn init(settings: &LogSettings) -> Result<(), LogInitError> {
    subscriber(settings, std::io::stdout, std::io::stdout().is_terminal()).try_init()?;
    Ok(())
}
