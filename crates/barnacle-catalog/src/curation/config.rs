use std::fmt;

use serde::Deserialize;

use crate::model::ShipGroup;
use crate::model::ShipIndex;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CurationConfig {
    pub reviewed_through: Option<u32>,
    pub groups: Vec<ShipGroup>,
    #[serde(default)]
    pub exclude: Vec<ExcludeEntry>,
    #[serde(default)]
    pub keep: Vec<KeepEntry>,
    #[serde(default)]
    pub lookalikes: Vec<LookalikeGroup>,
    #[serde(default)]
    pub aliases: Vec<AliasEntry>,
}

impl CurationConfig {
    pub fn from_toml(text: &str) -> Result<Self, toml::de::Error> {
        toml::from_str(text)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExcludeReason {
    CarbonCopy,
    BadSilhouette,
}

impl fmt::Display for ExcludeReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::CarbonCopy => "carbon copy",
            Self::BadSilhouette => "bad silhouette",
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExcludeEntry {
    pub index: ShipIndex,
    pub reason: ExcludeReason,
    pub base: Option<ShipIndex>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KeepEntry {
    pub index: ShipIndex,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LookalikeGroup {
    pub ships: Vec<ShipIndex>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AliasEntry {
    pub index: ShipIndex,
    pub names: Vec<String>,
}
