use std::path::Path;
use std::path::PathBuf;

use barnacle_catalog::Catalog;
use barnacle_catalog::ShipIndex;
use barnacle_catalog::Tier;
use barnacle_catalog::curation::Curated;
use barnacle_catalog::curation::CurationConfig;
use barnacle_catalog::curation::Problem;
use barnacle_catalog::curation::curate;
use barnacle_catalog::curation::validate;
use barnacle_catalog::store::CURRENT_FILE;
use barnacle_catalog::store::CatalogDirError;
use barnacle_catalog::store::CatalogRoot;
use barnacle_catalog::store::SILHOUETTES_DIR;
use barnacle_guess::RoundOptions;
use barnacle_guess::ShipBook;

use crate::config::Config;
use crate::config::ConfigError;
use crate::lookup::Directory;
use crate::solves::Solves;
use crate::solves::SolvesError;

#[derive(Debug, thiserror::Error)]
pub enum StartupError {
    #[error("{path} could not be read")]
    ConfigIo {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("{path} is not a valid Barnacle config")]
    Config {
        path: PathBuf,
        #[source]
        source: ConfigError,
    },
    #[error("DISCORD_TOKEN is not set")]
    MissingToken,
    #[error("no catalog is selected in {path}; run `barnacle-data use <catalog>`")]
    NoCurrentCatalog { path: PathBuf },
    #[error("the current catalog could not be loaded")]
    Catalog(#[from] CatalogDirError),
    #[error("{path} could not be read")]
    CurationIo {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("{path} is not a valid curation file")]
    CurationToml {
        path: PathBuf,
        #[source]
        source: toml::de::Error,
    },
    #[error("curation has {} problem(s); run `barnacle-data validate`", .problems.len())]
    Curation { problems: Vec<Problem> },
    #[error("{} ship(s) have no silhouette file in {dir}", .missing.len())]
    MissingSilhouettes {
        dir: PathBuf,
        missing: Vec<ShipIndex>,
    },
    #[error(
        "the solves database {path} is not ready; create it with `sqlite3 {path} < migrations/0001_guess_solves.sql`"
    )]
    Database {
        path: PathBuf,
        #[source]
        source: SolvesError,
    },
}

pub struct Loaded {
    pub root: CatalogRoot,
    pub catalog_name: String,
    pub catalog: Catalog,
    pub curated: Curated,
    pub book: ShipBook,
    pub directory: Directory,
}

pub fn read_config(path: &Path) -> Result<Config, StartupError> {
    let text = std::fs::read_to_string(path).map_err(|source| StartupError::ConfigIo {
        path: path.to_owned(),
        source,
    })?;
    Config::from_toml(&text).map_err(|source| StartupError::Config {
        path: path.to_owned(),
        source,
    })
}

pub fn read_token(value: Option<String>) -> Result<String, StartupError> {
    value
        .map(|token| token.trim().to_owned())
        .filter(|token| !token.is_empty())
        .ok_or(StartupError::MissingToken)
}

pub fn load(config: &Config) -> Result<Loaded, StartupError> {
    let root = CatalogRoot::new(config.catalogs());
    let catalog_name = root
        .current()?
        .ok_or_else(|| StartupError::NoCurrentCatalog {
            path: root.path().join(CURRENT_FILE),
        })?;
    let catalog = root.load(&catalog_name)?;
    let text =
        std::fs::read_to_string(&config.curation).map_err(|source| StartupError::CurationIo {
            path: config.curation.clone(),
            source,
        })?;
    let curation =
        CurationConfig::from_toml(&text).map_err(|source| StartupError::CurationToml {
            path: config.curation.clone(),
            source,
        })?;
    let curated = curate(&catalog, &curation);
    let problems = validate(&catalog, &curation, &curated);
    if !problems.is_empty() {
        return Err(StartupError::Curation { problems });
    }
    let book = ShipBook::new(&catalog, &curation);
    let every_tier = RoundOptions::new(Tier::new(1).ok(), Tier::new(11).ok(), None);
    let missing: Vec<ShipIndex> = book
        .pool(&every_tier)
        .into_iter()
        .filter(|index| !root.silhouette(&catalog_name, index).is_file())
        .cloned()
        .collect();
    if !missing.is_empty() {
        return Err(StartupError::MissingSilhouettes {
            dir: root.dir(&catalog_name).join(SILHOUETTES_DIR),
            missing,
        });
    }
    let directory = Directory::new(&catalog);
    Ok(Loaded {
        root,
        catalog_name,
        catalog,
        curated,
        book,
        directory,
    })
}

pub async fn open_solves(path: &Path) -> Result<Solves, StartupError> {
    Solves::open(path)
        .await
        .map_err(|source| StartupError::Database {
            path: path.to_owned(),
            source,
        })
}

pub fn describe(error: &(dyn std::error::Error + 'static)) -> String {
    let causes = std::iter::successors(error.source(), |cause| cause.source())
        .map(|cause| format!("  caused by: {cause}"));
    let problems =
        error
            .downcast_ref::<StartupError>()
            .into_iter()
            .flat_map(|startup| match startup {
                StartupError::Curation { problems } => problems
                    .iter()
                    .map(|problem| format!("  - {problem}"))
                    .collect(),
                StartupError::MissingSilhouettes { missing, .. } => {
                    missing.iter().map(|index| format!("  - {index}")).collect()
                }
                _ => Vec::new(),
            });
    std::iter::once(error.to_string())
        .chain(problems)
        .chain(causes)
        .collect::<Vec<_>>()
        .join("\n")
}
