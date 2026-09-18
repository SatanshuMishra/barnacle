use std::path::PathBuf;

use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub data_dir: PathBuf,
    pub curation: PathBuf,
    pub database: PathBuf,
    pub commands: CommandScope,
    pub rehearsal: Vec<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandScope {
    Guilds { guilds: Vec<u64> },
    Global,
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error(
        "the config is not valid at line {line}, column {column}; compare it with barnacle.example.toml"
    )]
    Toml { line: usize, column: usize },
    #[error("commands.scope is \"guilds\" but commands.guilds lists no server")]
    NoGuilds,
    #[error("commands.guilds contains 0, which is not a Discord server ID")]
    ZeroGuild,
    #[error("commands.scope is \"global\", so commands.guilds must be left out")]
    GuildsWithGlobal,
    #[error("rehearsal.guilds contains 0, which is not a Discord server ID")]
    ZeroRehearsalGuild,
    #[error("commands.scope is \"global\", so rehearsal.guilds must be left out")]
    RehearsalWithGlobal,
    #[error("rehearsal.guilds lists {guild}, which is not in commands.guilds")]
    RehearsalNotRegistered { guild: u64 },
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
    rehearsal: Option<RehearsalFile>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CommandsFile {
    scope: ScopeName,
    guilds: Option<Vec<u64>>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RehearsalFile {
    guilds: Vec<u64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum ScopeName {
    Guilds,
    Global,
}

impl Config {
    pub fn from_toml(text: &str) -> Result<Self, ConfigError> {
        let file: ConfigFile = toml::from_str(text).map_err(|error| {
            let (line, column) = position(text, error.span().map_or(0, |span| span.start));
            ConfigError::Toml { line, column }
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
        let rehearsal = rehearsal(&commands, file.rehearsal)?;
        Ok(Self {
            data_dir: file.data_dir,
            curation: file.curation,
            database: file.database,
            commands,
            rehearsal,
        })
    }

    pub fn catalogs(&self) -> PathBuf {
        self.data_dir.join("catalog")
    }
}

fn rehearsal(
    commands: &CommandScope,
    section: Option<RehearsalFile>,
) -> Result<Vec<u64>, ConfigError> {
    match (commands, section) {
        (_, None) => Ok(Vec::new()),
        (CommandScope::Global, Some(_)) => Err(ConfigError::RehearsalWithGlobal),
        (CommandScope::Guilds { .. }, Some(RehearsalFile { guilds })) if guilds.contains(&0) => {
            Err(ConfigError::ZeroRehearsalGuild)
        }
        (CommandScope::Guilds { guilds: registered }, Some(RehearsalFile { guilds })) => {
            match guilds.iter().find(|guild| !registered.contains(guild)) {
                Some(guild) => Err(ConfigError::RehearsalNotRegistered { guild: *guild }),
                None => Ok(guilds),
            }
        }
    }
}

fn position(text: &str, offset: usize) -> (usize, usize) {
    let before = text.get(..offset).unwrap_or(text);
    let line = before.matches('\n').count() + 1;
    let column = before
        .rsplit('\n')
        .next()
        .map_or(0, |last| last.chars().count())
        + 1;
    (line, column)
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
