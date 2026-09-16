use std::fs::File;
use std::path::Path;
use std::path::PathBuf;

use barnacle_catalog::ShipIndex;
use barnacle_catalog::ShipName;

#[derive(Debug, thiserror::Error)]
pub enum NamesError {
    #[error("translation catalog {path} could not be opened")]
    Open {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("translation catalog {path} could not be parsed")]
    Parse {
        path: PathBuf,
        #[source]
        source: gettext::Error,
    },
}

pub struct EnglishNames {
    catalog: gettext::Catalog,
}

impl EnglishNames {
    pub fn load(path: &Path) -> Result<Self, NamesError> {
        let file = File::open(path).map_err(|source| NamesError::Open {
            path: path.to_owned(),
            source,
        })?;
        let catalog = gettext::Catalog::parse(file).map_err(|source| NamesError::Parse {
            path: path.to_owned(),
            source,
        })?;
        Ok(Self { catalog })
    }

    pub fn ship_name(&self, index: &ShipIndex) -> Option<ShipName> {
        let short = self.lookup(&format!("IDS_{index}"))?;
        Some(ShipName {
            short,
            full: self.lookup(&format!("IDS_{index}_FULL")),
        })
    }

    fn lookup(&self, key: &str) -> Option<String> {
        let value = self.catalog.gettext(key);
        (value != key && !value.trim().is_empty()).then(|| value.to_owned())
    }
}
