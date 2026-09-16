use std::collections::BTreeMap;
use std::io::ErrorKind;
use std::path::PathBuf;

use barnacle_catalog::Catalog;
use barnacle_catalog::Provenance;
use wows_data_mgr::Dump;
use wows_data_mgr::builds::BuildEntry;
use wows_data_mgr::download_repo;

use crate::extract::BuildInputs;
use crate::extract::ExtractError;
use crate::extract::build_catalog;
use crate::versions;

const CATALOG_FILE: &str = "catalog.json";
const CURRENT_FILE: &str = "current";
const ENGLISH_CATALOG: &str = "translations/en/LC_MESSAGES/global.mo";

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("{path} could not be read or written")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("catalog {path} is not valid")]
    CatalogJson {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error("the HTTP client could not be created")]
    Client(#[from] reqwest::Error),
    #[error("the game data repository request failed")]
    Remote(#[source] Box<dyn std::error::Error + Send + Sync>),
    #[error("the game data repository has no published builds")]
    NoPublishedBuilds,
    #[error("build {build} is not in the game data repository's index")]
    BuildNotInIndex { build: u32 },
    #[error("build directory {dir} has no English translation catalog")]
    NoEnglishCatalog { dir: String },
    #[error("no catalog has been built yet; run `barnacle-data sync`")]
    NoCatalogs,
    #[error(transparent)]
    Extract(#[from] ExtractError),
}

fn io_error(path: PathBuf) -> impl FnOnce(std::io::Error) -> StoreError {
    move |source| StoreError::Io { path, source }
}

pub struct DataDir {
    root: PathBuf,
}

impl DataDir {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn store(&self) -> PathBuf {
        self.root.join("store")
    }

    pub fn catalogs(&self) -> PathBuf {
        self.root.join("catalog")
    }

    pub fn catalog_dir(&self, name: &str) -> PathBuf {
        self.catalogs().join(name)
    }

    pub fn current(&self) -> Result<Option<String>, StoreError> {
        let path = self.catalogs().join(CURRENT_FILE);
        match std::fs::read_to_string(&path) {
            Ok(text) => Ok(Some(text.trim().to_owned())),
            Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
            Err(source) => Err(StoreError::Io { path, source }),
        }
    }

    pub fn set_current(&self, name: &str) -> Result<(), StoreError> {
        let path = self.catalogs().join(CURRENT_FILE);
        std::fs::write(&path, format!("{name}\n")).map_err(io_error(path))
    }

    pub fn built(&self) -> Result<Vec<String>, StoreError> {
        let root = self.catalogs();
        let listing = match std::fs::read_dir(&root) {
            Ok(listing) => listing,
            Err(error) if error.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
            Err(source) => return Err(StoreError::Io { path: root, source }),
        };
        let entries = listing
            .collect::<Result<Vec<_>, _>>()
            .map_err(io_error(root))?;
        let by_build: BTreeMap<u32, String> = entries
            .iter()
            .filter(|entry| entry.path().join(CATALOG_FILE).is_file())
            .filter_map(|entry| {
                let name = entry.file_name().into_string().ok()?;
                let build = name.rsplit_once('_')?.1.parse().ok()?;
                Some((build, name))
            })
            .collect();
        Ok(by_build.into_values().collect())
    }

    pub fn newest(&self) -> Result<String, StoreError> {
        self.built()?.pop().ok_or(StoreError::NoCatalogs)
    }

    pub fn load(&self, name: &str) -> Result<Catalog, StoreError> {
        let path = self.catalog_dir(name).join(CATALOG_FILE);
        let text = std::fs::read_to_string(&path).map_err(io_error(path.clone()))?;
        Catalog::from_json(&text).map_err(|source| StoreError::CatalogJson { path, source })
    }

    pub fn save(&self, name: &str, catalog: &Catalog) -> Result<(), StoreError> {
        let path = self.catalog_dir(name).join(CATALOG_FILE);
        let json = catalog
            .to_json()
            .map_err(|source| StoreError::CatalogJson {
                path: path.clone(),
                source,
            })?;
        std::fs::write(&path, json).map_err(io_error(path))
    }
}

pub struct Downloaded {
    pub entry: BuildEntry,
    pub data_repo_commit: String,
}

pub async fn download(data: &DataDir, requested: Option<u32>) -> Result<Downloaded, StoreError> {
    let client = reqwest::Client::builder()
        .user_agent(concat!("barnacle-data/", env!("CARGO_PKG_VERSION")))
        .build()?;
    let base = download_repo::DEFAULT_REPO_BASE_URL;
    let remote = |report: rootcause::Report| StoreError::Remote(report.into());
    let index = download_repo::fetch_builds_index(&client, base)
        .await
        .map_err(remote)?;
    let target = match requested {
        Some(build) => build,
        None => index
            .builds
            .iter()
            .map(|entry| entry.build)
            .max()
            .ok_or(StoreError::NoPublishedBuilds)?,
    };
    let data_repo_commit = download_repo::fetch_repo_tip(&client)
        .await
        .map_err(remote)?;
    let store = data.store();
    std::fs::create_dir_all(&store).map_err(io_error(store.clone()))?;
    let progress = |done: u64, total: u64| {
        if total > 0 && (done == total || done % 250 == 0) {
            eprintln!("downloaded {done} of {total} objects");
        }
    };
    let build =
        download_repo::download_build(&client, base, &store, target, None, false, &progress)
            .await
            .map_err(remote)?;
    let entry = index
        .find_by_build(build)
        .cloned()
        .ok_or(StoreError::BuildNotInIndex { build })?;
    Ok(Downloaded {
        entry,
        data_repo_commit,
    })
}

pub fn build(data: &DataDir, downloaded: &Downloaded) -> Result<Catalog, StoreError> {
    let entry = &downloaded.entry;
    let dump = Dump::open(&data.store().join(&entry.dir));
    let english_mo =
        dump.derived_path(ENGLISH_CATALOG)
            .ok_or_else(|| StoreError::NoEnglishCatalog {
                dir: entry.dir.clone(),
            })?;
    let output_dir = data.catalog_dir(&entry.dir);
    std::fs::create_dir_all(&output_dir).map_err(io_error(output_dir.clone()))?;
    let vfs = dump.vfs();
    let catalog = build_catalog(BuildInputs {
        vfs: &vfs,
        english_mo: &english_mo,
        provenance: Provenance {
            game_version: entry.version.clone(),
            build: entry.build,
            data_repo_commit: downloaded.data_repo_commit.clone(),
            wowsunpack: versions::WOWSUNPACK.to_owned(),
            wows_data_mgr: versions::WOWS_DATA_MGR.to_owned(),
        },
        output_dir: &output_dir,
    })?;
    data.save(&entry.dir, &catalog)?;
    Ok(catalog)
}
