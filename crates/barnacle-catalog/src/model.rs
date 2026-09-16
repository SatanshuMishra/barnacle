use std::fmt;

use serde::Deserialize;
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ModelError {
    #[error("ship index {value:?} is not 7 uppercase ASCII letters or digits")]
    InvalidIndex { value: String },
    #[error("tier {value} is outside 1-11")]
    InvalidTier { value: u32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ParamId(u64);

impl ParamId {
    pub fn new(raw: u64) -> Self {
        Self(raw)
    }

    pub fn get(self) -> u64 {
        self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct ShipIndex(String);

impl ShipIndex {
    pub fn parse(value: &str) -> Result<Self, ModelError> {
        let well_formed = value.len() == 7
            && value
                .bytes()
                .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit());
        if well_formed {
            Ok(Self(value.to_owned()))
        } else {
            Err(ModelError::InvalidIndex {
                value: value.to_owned(),
            })
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for ShipIndex {
    type Error = ModelError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(&value)
    }
}

impl From<ShipIndex> for String {
    fn from(index: ShipIndex) -> Self {
        index.0
    }
}

impl fmt::Display for ShipIndex {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "u32", into = "u32")]
pub struct Tier(u8);

impl Tier {
    pub fn new(value: u32) -> Result<Self, ModelError> {
        u8::try_from(value)
            .ok()
            .filter(|tier| (1..=11).contains(tier))
            .map(Self)
            .ok_or(ModelError::InvalidTier { value })
    }

    pub fn get(self) -> u32 {
        u32::from(self.0)
    }
}

impl TryFrom<u32> for Tier {
    type Error = ModelError;

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<Tier> for u32 {
    fn from(tier: Tier) -> Self {
        tier.get()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ShipGroup(String);

impl ShipGroup {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ShipGroup {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Nation(String);

impl Nation {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Nation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShipClass {
    Destroyer,
    Cruiser,
    Battleship,
    AircraftCarrier,
    Submarine,
    Other(String),
    Unspecified,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShipName {
    pub short: String,
    pub full: Option<String>,
}

impl ShipName {
    pub fn display(&self) -> &str {
        self.full.as_deref().unwrap_or(&self.short)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Silhouette {
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ship {
    pub id: ParamId,
    pub index: ShipIndex,
    pub tier: Tier,
    pub group: ShipGroup,
    pub class: ShipClass,
    pub nation: Nation,
    pub is_paper: bool,
    pub name: Option<ShipName>,
    pub silhouette: Option<Silhouette>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Provenance {
    pub game_version: String,
    pub build: u32,
    pub data_repo_commit: String,
    pub wowsunpack: String,
    pub wows_data_mgr: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Catalog {
    pub provenance: Provenance,
    pub ships: Vec<Ship>,
}

impl Catalog {
    pub fn from_json(text: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(text)
    }

    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    pub fn get(&self, index: &ShipIndex) -> Option<&Ship> {
        self.ships.iter().find(|ship| &ship.index == index)
    }
}
