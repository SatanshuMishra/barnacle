use std::ops::Range;
use std::path::PathBuf;

use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub data_dir: PathBuf,
    pub curation: PathBuf,
    pub database: PathBuf,
    pub commands: CommandScope,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandScope {
    Guilds { guilds: Vec<u64> },
    Global,
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("the config is not valid: {message}")]
    Toml {
        message: String,
        span: Option<Range<usize>>,
    },
    #[error("commands.scope is \"guilds\" but commands.guilds lists no server")]
    NoGuilds,
    #[error("commands.guilds contains 0, which is not a Discord server ID")]
    ZeroGuild,
    #[error("commands.scope is \"global\", so commands.guilds must be left out")]
    GuildsWithGlobal,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ConfigFile {
    #[serde(default = "default_data_dir")]
    data_dir: PathBuf,
    #[serde(default = "default_curation")]
    curation: PathBuf,
    #[serde(default = "default_database")]
    database: PathBuf,
    commands: CommandsFile,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CommandsFile {
    scope: ScopeName,
    guilds: Option<Vec<u64>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum ScopeName {
    Guilds,
    Global,
}

impl Config {
    pub fn from_toml(text: &str) -> Result<Self, ConfigError> {
        let file: ConfigFile = toml::from_str(text).map_err(|error| ConfigError::Toml {
            message: error.message().to_owned(),
            span: error.span(),
        })?;
        let commands = match (file.commands.scope, file.commands.guilds) {
            (ScopeName::Global, None) => CommandScope::Global,
            (ScopeName::Global, Some(_)) => return Err(ConfigError::GuildsWithGlobal),
            (ScopeName::Guilds, None) => return Err(ConfigError::NoGuilds),
            (ScopeName::Guilds, Some(guilds)) if guilds.is_empty() => {
                return Err(ConfigError::NoGuilds);
            }
            (ScopeName::Guilds, Some(guilds)) if guilds.contains(&0) => {
                return Err(ConfigError::ZeroGuild);
            }
            (ScopeName::Guilds, Some(guilds)) => CommandScope::Guilds { guilds },
        };
        Ok(Self {
            data_dir: file.data_dir,
            curation: file.curation,
            database: file.database,
            commands,
        })
    }

    pub fn catalogs(&self) -> PathBuf {
        self.data_dir.join("catalog")
    }
}

fn default_data_dir() -> PathBuf {
    PathBuf::from("data")
}

fn default_curation() -> PathBuf {
    PathBuf::from("curation/ships.toml")
}

fn default_database() -> PathBuf {
    PathBuf::from("data/barnacle.sqlite3")
}
