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
const STAGING_PREFIX: &str = ".staging-";

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
    #[error("catalog revisions for {build_dir} have run out of numbers")]
    RevisionOverflow { build_dir: String },
    #[error("{source}; its staging directory {dir} could not be removed either: {cleanup}")]
    LeftStaging {
        dir: PathBuf,
        #[source]
        source: Box<StoreError>,
        cleanup: std::io::Error,
    },
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

fn parse_number(text: &str) -> Option<u32> {
    let canonical = !text.is_empty()
        && text.bytes().all(|byte| byte.is_ascii_digit())
        && (text == "0" || !text.starts_with('0'));
    canonical.then(|| text.parse().ok()).flatten()
}

fn staged_name(directory: &str) -> Option<&str> {
    directory
        .strip_prefix(STAGING_PREFIX)?
        .rsplit_once('-')
        .map(|(name, _)| name)
}

fn parse_catalog_name(name: &str) -> Option<CatalogName<'_>> {
    let (build_dir, revision) = name.rsplit_once("_r")?;
    let (version, build) = build_dir.rsplit_once('_')?;
    if version.is_empty() || version.starts_with('.') {
        return None;
    }
    Some(CatalogName {
        build_dir,
        build: parse_number(build)?,
        revision: parse_number(revision).filter(|revision| *revision > 0)?,
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
        let ordered: BTreeMap<(u32, u32, &str), &String> = names
            .iter()
            .filter(|name| self.catalog_dir(name).join(CATALOG_FILE).is_file())
            .filter_map(|name| {
                let parsed = parse_catalog_name(name)?;
                Some(((parsed.build, parsed.revision, parsed.build_dir), name))
            })
            .collect();
        Ok(ordered.into_values().cloned().collect())
    }

    pub fn newest(&self) -> Result<String, StoreError> {
        self.built()?.pop().ok_or(StoreError::NoCatalogs)
    }

    pub fn stage_catalog(&self, build_dir: &str) -> Result<StagedCatalog, StoreError> {
        let root = self.catalogs();
        std::fs::create_dir_all(&root).map_err(io_error(root.clone()))?;
        let names = self.directory_names()?;
        let latest = names
            .iter()
            .map(|name| staged_name(name).unwrap_or(name))
            .filter_map(parse_catalog_name)
            .filter(|parsed| parsed.build_dir == build_dir)
            .map(|parsed| parsed.revision)
            .max();
        let revision = match latest {
            None => 1,
            Some(latest) => latest
                .checked_add(1)
                .ok_or_else(|| StoreError::RevisionOverflow {
                    build_dir: build_dir.to_owned(),
                })?,
        };
        let name = format!("{build_dir}_r{revision}");
        let dir = root.join(format!("{STAGING_PREFIX}{name}-{}", std::process::id()));
        std::fs::create_dir(&dir).map_err(io_error(dir.clone()))?;
        Ok(StagedCatalog { name, dir })
    }

    pub fn publish(&self, staged: StagedCatalog, catalog: &Catalog) -> Result<String, StoreError> {
        match self.write_and_move(&staged, catalog) {
            Ok(()) => Ok(staged.name),
            Err(error) => Err(self.discard_after(staged, error)),
        }
    }

    fn write_and_move(&self, staged: &StagedCatalog, catalog: &Catalog) -> Result<(), StoreError> {
        let file = staged.dir.join(CATALOG_FILE);
        let json = catalog
            .to_json()
            .map_err(|source| StoreError::CatalogJson {
                path: file.clone(),
                source,
            })?;
        std::fs::write(&file, json).map_err(io_error(file))?;
        let target = self.catalog_dir(&staged.name);
        std::fs::rename(&staged.dir, &target).map_err(io_error(target))
    }

    pub fn discard_after(&self, staged: StagedCatalog, error: StoreError) -> StoreError {
        match std::fs::remove_dir_all(&staged.dir) {
            Ok(()) => error,
            Err(cleanup) => StoreError::LeftStaging {
                dir: staged.dir,
                source: Box::new(error),
                cleanup,
            },
        }
    }

    pub fn load(&self, name: &str) -> Result<Catalog, StoreError> {
        let path = self.catalog_dir(name).join(CATALOG_FILE);
        let text = std::fs::read_to_string(&path).map_err(io_error(path.clone()))?;
        Catalog::from_json(&text).map_err(|source| StoreError::CatalogJson { path, source })
    }
}

pub struct StagedCatalog {
    pub name: String,
    pub dir: PathBuf,
}

pub struct Downloaded {
    pub entry: BuildEntry,
    pub data_repo_commit: String,
}

pub async fn download(data: &DataDir, requested: Option<u32>) -> Result<Downloaded, StoreError> {
    let client = reqwest::Client::builder()
        .user_agent(concat!("barnacle-data/", env!("CARGO_PKG_VERSION")))
        .build()?;
    let commit = download_repo::fetch_repo_tip(&client)
        .await
        .map_err(remote_error)?;
    let base = pinned_base_url(download_repo::DEFAULT_REPO_BASE_URL, &commit)?;
    download_from(&client, &base, &commit, &data.store(), requested).await
}

fn remote_error(report: rootcause::Report) -> StoreError {
    StoreError::Remote(report.into())
}

pub async fn download_from(
    client: &reqwest::Client,
    base: &str,
    commit: &str,
    store: &Path,
    requested: Option<u32>,
) -> Result<Downloaded, StoreError> {
    let index = download_repo::fetch_builds_index(client, base)
        .await
        .map_err(remote_error)?;
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

    std::fs::create_dir_all(store).map_err(io_error(store.to_path_buf()))?;
    let progress = |done: u64, total: u64| {
        if total > 0 && (done == total || done.is_multiple_of(250)) {
            eprintln!("downloaded {done} of {total} objects");
        }
    };
    let downloaded =
        download_repo::download_build(client, base, store, entry.build, None, true, &progress)
            .await
            .map_err(remote_error)?;
    if downloaded != entry.build {
        return Err(StoreError::UnexpectedBuild {
            requested: entry.build,
            downloaded,
        });
    }
    Ok(Downloaded {
        entry,
        data_repo_commit: commit.to_owned(),
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
    let staged = data.stage_catalog(&entry.dir)?;
    let vfs = dump.vfs();
    let result = build_catalog(BuildInputs {
        vfs: &vfs,
        english_mo: &english_mo,
        provenance: Provenance {
            game_version: entry.version.clone(),
            build: entry.build,
            data_repo_commit: downloaded.data_repo_commit.clone(),
            wowsunpack: versions::WOWSUNPACK.to_owned(),
            wows_data_mgr: versions::WOWS_DATA_MGR.to_owned(),
        },
        output_dir: &staged.dir,
    });
    match result {
        Ok(catalog) => {
            let name = data.publish(staged, &catalog)?;
            Ok(Built { name, catalog })
        }
        Err(source) => Err(data.discard_after(staged, StoreError::Extract(source))),
    }
}
