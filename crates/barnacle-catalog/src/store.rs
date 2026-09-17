use std::io::ErrorKind;
use std::path::Path;
use std::path::PathBuf;

use crate::model::Catalog;
use crate::model::ShipIndex;

pub const CATALOG_FILE: &str = "catalog.json";
pub const CURRENT_FILE: &str = "current";
pub const SILHOUETTES_DIR: &str = "silhouettes";

#[derive(Debug, thiserror::Error)]
pub enum CatalogDirError {
    #[error("{path} could not be read")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("catalog {path} is not valid")]
    Json {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogRoot(PathBuf);

impl CatalogRoot {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self(path.into())
    }

    pub fn path(&self) -> &Path {
        &self.0
    }

    pub fn dir(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }

    pub fn silhouette(&self, name: &str, index: &ShipIndex) -> PathBuf {
        self.dir(name)
            .join(SILHOUETTES_DIR)
            .join(format!("{index}.png"))
    }

    pub fn current(&self) -> Result<Option<String>, CatalogDirError> {
        let path = self.0.join(CURRENT_FILE);
        match std::fs::read_to_string(&path) {
            Ok(text) => Ok(Some(text.trim().to_owned())),
            Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
            Err(source) => Err(CatalogDirError::Io { path, source }),
        }
    }

    pub fn load(&self, name: &str) -> Result<Catalog, CatalogDirError> {
        let path = self.dir(name).join(CATALOG_FILE);
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(source) => return Err(CatalogDirError::Io { path, source }),
        };
        Catalog::from_json(&text).map_err(|source| CatalogDirError::Json { path, source })
    }
}
