use std::collections::BTreeMap;
use std::io::ErrorKind;
use std::path::Component;
use std::path::Path;
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
const RAW_GITHUB: &str = "https://raw.githubusercontent.com/";

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
    #[error("asked for build {requested} but the repository returned build {downloaded}")]
    UnexpectedBuild { requested: u32, downloaded: u32 },
    #[error(
        "the repository index names build directory {dir:?}, which is not a safe directory name"
    )]
    UnsafeBuildDir { dir: String },
    #[error("the repository tip {commit:?} is not a commit id")]
    UnexpectedCommit { commit: String },
    #[error("the game data repository URL {url} is not a raw GitHub main-branch URL")]
    UnexpectedRepoUrl { url: String },
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

struct CatalogName<'a> {
    build_dir: &'a str,
    build: u32,
    revision: u32,
}

fn parse_catalog_name(name: &str) -> Option<CatalogName<'_>> {
    let (build_dir, revision) = name.rsplit_once("_r")?;
    let (_, build) = build_dir.rsplit_once('_')?;
    Some(CatalogName {
        build_dir,
        build: build.parse().ok()?,
        revision: revision.parse().ok()?,
    })
}

pub fn pinned_base_url(base: &str, commit: &str) -> Result<String, StoreError> {
    let is_commit = commit.len() == 40 && commit.bytes().all(|byte| byte.is_ascii_hexdigit());
    if !is_commit {
        return Err(StoreError::UnexpectedCommit {
            commit: commit.to_owned(),
        });
    }
    base.strip_suffix("/main")
        .filter(|repo| repo.starts_with(RAW_GITHUB))
        .map(|repo| format!("{repo}/{commit}"))
        .ok_or_else(|| StoreError::UnexpectedRepoUrl {
            url: base.to_owned(),
        })
}

pub fn check_build_dir(entry: &BuildEntry) -> Result<(), StoreError> {
    let expected = format!("{}_{}", entry.version, entry.build);
    let components: Vec<Component<'_>> = Path::new(&entry.dir).components().collect();
    let single_name = matches!(components.as_slice(), [Component::Normal(_)]);
    if entry.dir == expected && single_name {
        Ok(())
    } else {
        Err(StoreError::UnsafeBuildDir {
            dir: entry.dir.clone(),
        })
    }
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

    fn directory_names(&self) -> Result<Vec<String>, StoreError> {
        let root = self.catalogs();
        let listing = match std::fs::read_dir(&root) {
            Ok(listing) => listing,
            Err(error) if error.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
            Err(source) => return Err(StoreError::Io { path: root, source }),
        };
        let entries = listing
            .collect::<Result<Vec<_>, _>>()
            .map_err(io_error(root))?;
        Ok(entries
            .iter()
            .filter(|entry| entry.path().is_dir())
            .filter_map(|entry| entry.file_name().into_string().ok())
            .collect())
    }

    pub fn built(&self) -> Result<Vec<String>, StoreError> {
        let names = self.directory_names()?;
        let ordered: BTreeMap<(u32, u32), &String> = names
            .iter()
            .filter(|name| self.catalog_dir(name).join(CATALOG_FILE).is_file())
            .filter_map(|name| {
                let parsed = parse_catalog_name(name)?;
                Some(((parsed.build, parsed.revision), name))
            })
            .collect();
        Ok(ordered.into_values().cloned().collect())
    }

    pub fn newest(&self) -> Result<String, StoreError> {
        self.built()?.pop().ok_or(StoreError::NoCatalogs)
    }

    pub fn reserve_catalog_dir(&self, build_dir: &str) -> Result<(String, PathBuf), StoreError> {
        let root = self.catalogs();
        std::fs::create_dir_all(&root).map_err(io_error(root.clone()))?;
        let names = self.directory_names()?;
        let revision = names
            .iter()
            .filter_map(|name| parse_catalog_name(name))
            .filter(|parsed| parsed.build_dir == build_dir)
            .map(|parsed| parsed.revision)
            .max()
            .map_or(1, |latest| latest + 1);
        let name = format!("{build_dir}_r{revision}");
        let path = root.join(&name);
        std::fs::create_dir(&path).map_err(io_error(path.clone()))?;
        Ok((name, path))
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
    pub refreshed: bool,
}

pub async fn download(data: &DataDir, requested: Option<u32>) -> Result<Downloaded, StoreError> {
    let client = reqwest::Client::builder()
        .user_agent(concat!("barnacle-data/", env!("CARGO_PKG_VERSION")))
        .build()?;
    let remote = |report: rootcause::Report| StoreError::Remote(report.into());
    let commit = download_repo::fetch_repo_tip(&client)
        .await
        .map_err(remote)?;
    let base = pinned_base_url(download_repo::DEFAULT_REPO_BASE_URL, &commit)?;
    let index = download_repo::fetch_builds_index(&client, &base)
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
    let entry = index
        .find_by_build(target)
        .cloned()
        .ok_or(StoreError::BuildNotInIndex { build: target })?;
    check_build_dir(&entry)?;

    let store = data.store();
    std::fs::create_dir_all(&store).map_err(io_error(store.clone()))?;
    let stale = download_repo::check_for_updates(&client, &base, &store, None)
        .await
        .map_err(remote)?;
    let refreshed = stale
        .updates
        .iter()
        .any(|update| update.build == entry.build);
    let progress = |done: u64, total: u64| {
        if total > 0 && (done == total || done.is_multiple_of(250)) {
            eprintln!("downloaded {done} of {total} objects");
        }
    };
    let downloaded = download_repo::download_build(
        &client,
        &base,
        &store,
        entry.build,
        None,
        refreshed,
        &progress,
    )
    .await
    .map_err(remote)?;
    if downloaded != entry.build {
        return Err(StoreError::UnexpectedBuild {
            requested: entry.build,
            downloaded,
        });
    }
    Ok(Downloaded {
        entry,
        data_repo_commit: commit,
        refreshed,
    })
}

pub struct Built {
    pub name: String,
    pub catalog: Catalog,
}

pub fn build(data: &DataDir, downloaded: &Downloaded) -> Result<Built, StoreError> {
    let entry = &downloaded.entry;
    let dump = Dump::open(&data.store().join(&entry.dir));
    let english_mo =
        dump.derived_path(ENGLISH_CATALOG)
            .ok_or_else(|| StoreError::NoEnglishCatalog {
                dir: entry.dir.clone(),
            })?;
    let (name, output_dir) = data.reserve_catalog_dir(&entry.dir)?;
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
    data.save(&name, &catalog)?;
    Ok(Built { name, catalog })
}
