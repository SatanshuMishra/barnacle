# Barnacle Data Pipeline Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A `barnacle-data` command that downloads a World of Warships build from wows-toolkit's data repository, builds Barnacle's ship catalog (with composited silhouettes), applies the automatic curation rules, and validates and diffs the result.

**Architecture:** Two crates. `barnacle-catalog` holds the catalog model, name normalisation, curation rules, validation and diffing, and has no wows-toolkit dependency, so the bot can use it without compiling the toolkit. `barnacle-data` holds everything that touches wows-toolkit (download, GameParams parsing, the paper-ship read, translations, silhouettes) plus the CLI.

**Tech Stack:** Rust 1.97, `wowsunpack` 0.45.0, `wows-data-mgr` 0.21.0, `pickled` 2.0.0-alpha10, `gettext` 0.4, `image` 0.25, `reqwest` 0.12, `rootcause` 0.12, `tokio`, `clap` 4, `serde`, `toml` 1, `thiserror` 2, `deunicode` 1, `sha2` 0.10.

**Spec:** `docs/specs/2026-09-16-silhouette-game-spec.md` (sections 2-4, 7, 9 and 11). Findings behind it: `docs/research/2026-09-16-track-silhouette-audit.md`.

**Follow-up plans (not in this one):** Plan 2 builds `barnacle-guess` (game rules, spec section 5) and Plan 3 builds `barnacle-bot`. Both wait for the owner to approve the defaults in spec section 5.

## Global Constraints

- Toolchain: Rust `1.97`, pinned in `rust-toolchain.toml`, matching wows-toolkit's own pin. The published toolkit crates need at least 1.92.
- `wowsunpack = "=0.45.0"` with `default-features = false, features = ["parsing", "vfs"]`.
- `wows-data-mgr = "=0.21.0"` with `default-features = false, features = ["download"]`.
- `pickled = "=2.0.0-alpha10"`, the exact version `wowsunpack` 0.45.0 uses.
- `reqwest` must be `0.12` and `rootcause` must be `0.12`, the versions `wows-data-mgr` 0.21.0 uses: `download_build` takes a `&reqwest::Client` and returns a `rootcause::Report` error.
- License: Apache-2.0. Every dependency's license must be on the `deny.toml` allowlist. If a new license appears, stop and ask the owner; do not add it.
- Never copy anything from padtrack/track: no code, no UI text, no entries from its `guess.toml`.
- No code comments, docstrings or section-header comments. Tooling pragmas such as `#![allow(dead_code)]` are allowed.
- No emojis anywhere.
- Domain values are newtypes (`ShipIndex`, `Tier`, `ParamId`, `ShipGroup`, `Nation`). "Absent" is `Option`, never a sentinel value.
- Errors are `thiserror` enums with structured fields. Never parse an error's text.
- Prefer building new values over mutating existing ones. Local accumulation inside one function is acceptable.
- Unit tests never touch the network. The one real-data test skips unless `BARNACLE_TEST_BUILD_DIR` is set.
- Anything that downloads (the Rust toolchain, the Apache license text, a game build of about 310 MB) needs the owner's go-ahead before it runs.
- Commits use Conventional Commits, one per task, staged by explicit path.

## File Structure

```
Cargo.toml                                   workspace, shared dependency pins
rust-toolchain.toml                          toolchain pin
.gitignore                                   target/ and data/
LICENSE                                      Apache-2.0 text
README.md                                    what Barnacle is, Wargaming notice
deny.toml                                    license allowlist
.github/workflows/ci.yml                     fmt, clippy, tests, licenses
.github/dependabot.yml                       weekly dependency updates
curation/ships.toml                          curation config (seed)
crates/barnacle-catalog/
  Cargo.toml
  src/lib.rs                                 module wiring and re-exports
  src/model.rs                               newtypes, Ship, Catalog, Provenance
  src/names.rs                               clean_answer, name_tokens
  src/curation/mod.rs                        re-exports
  src/curation/config.rs                     CurationConfig and its entries
  src/curation/rules.rs                      curate(), Curated, Removal
  src/curation/validate.rs                   validate(), Problem
  src/diff.rs                                diff(), year_refit_candidates()
  tests/common/mod.rs                        test builders
  tests/model.rs
  tests/names.rs
  tests/curation.rs
  tests/validate.rs
  tests/diff.rs
crates/barnacle-data/
  Cargo.toml
  src/lib.rs                                 module wiring
  src/main.rs                                CLI
  src/versions.rs                            toolkit versions recorded in catalogs
  src/extract/mod.rs                         build_catalog()
  src/extract/paper.rs                       paper-ship flags from the raw GameParams tree
  src/extract/params.rs                      typed ship fields via wowsunpack
  src/extract/translations.rs                English names from global.mo
  src/extract/silhouette.rs                  compositing and hashing
  src/store.rs                               data directory, download, build
  src/report.rs                              diff and validation reports
  tests/fixtures/make_fixtures.py            generates the synthetic fixtures below
  tests/fixtures/mini_gameparams.data
  tests/fixtures/mini_gameparams_list_root.data
  tests/fixtures/mini_gameparams_missing_flag.data
  tests/fixtures/mini_en.mo
  tests/versions.rs
  tests/paper.rs
  tests/translations.rs
  tests/silhouette.rs
  tests/store.rs
  tests/report.rs
  tests/real_build.rs
```

---

### Task 1: Workspace and catalog model

**Files:**
- Create: `Cargo.toml`, `rust-toolchain.toml`, `.gitignore`
- Create: `crates/barnacle-catalog/Cargo.toml`, `crates/barnacle-catalog/src/lib.rs`, `crates/barnacle-catalog/src/model.rs`
- Test: `crates/barnacle-catalog/tests/common/mod.rs`, `crates/barnacle-catalog/tests/model.rs`

**Interfaces:**
- Consumes: nothing.
- Produces, all re-exported from `barnacle_catalog`:
  - `ModelError { InvalidIndex { value: String }, InvalidTier { value: u32 } }`
  - `ParamId::new(u64) -> ParamId`, `ParamId::get(self) -> u64`
  - `ShipIndex::parse(&str) -> Result<ShipIndex, ModelError>`, `ShipIndex::as_str(&self) -> &str`, `Display`
  - `Tier::new(u32) -> Result<Tier, ModelError>`, `Tier::get(self) -> u32`
  - `ShipGroup::new(impl Into<String>)`, `ShipGroup::as_str(&self) -> &str`, `Display`
  - `Nation::new(impl Into<String>)`, `Nation::as_str(&self) -> &str`, `Display`
  - `enum ShipClass { Destroyer, Cruiser, Battleship, AircraftCarrier, Submarine, Other(String), Unspecified }`
  - `ShipName { short: String, full: Option<String> }`, `ShipName::display(&self) -> &str`
  - `Silhouette { sha256: String }`
  - `Ship { id: ParamId, index: ShipIndex, tier: Tier, group: ShipGroup, class: ShipClass, nation: Nation, is_paper: bool, name: Option<ShipName>, silhouette: Option<Silhouette> }`
  - `Provenance { game_version: String, build: u32, data_repo_commit: String, wowsunpack: String, wows_data_mgr: String }`
  - `Catalog { provenance: Provenance, ships: Vec<Ship> }`, `Catalog::from_json(&str)`, `Catalog::to_json(&self)`, `Catalog::get(&self, &ShipIndex) -> Option<&Ship>`

- [ ] **Step 1: Install the toolchain (ask the owner first; this downloads Rust 1.97)**

Run: `rustup toolchain install 1.97 --component rustfmt --component clippy`
Expected: `1.97-aarch64-apple-darwin installed` (or "unchanged" if already present).

- [ ] **Step 2: Create the workspace files**

`Cargo.toml`:

```toml
[workspace]
resolver = "3"
members = ["crates/*"]

[workspace.package]
version = "0.1.0"
edition = "2024"
rust-version = "1.97"
license = "Apache-2.0"
repository = "https://github.com/SatanshuMishra/barnacle"

[workspace.dependencies]
barnacle-catalog = { path = "crates/barnacle-catalog" }
wowsunpack = { version = "=0.45.0", default-features = false, features = ["parsing", "vfs"] }
wows-data-mgr = { version = "=0.21.0", default-features = false, features = ["download"] }
pickled = "=2.0.0-alpha10"
reqwest = { version = "0.12", default-features = false, features = ["rustls-tls-native-roots"] }
rootcause = "0.12"
clap = { version = "4", features = ["derive"] }
deunicode = "1"
gettext = "0.4"
image = { version = "0.25", default-features = false, features = ["png"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
sha2 = "0.10"
tempfile = "3"
thiserror = "2"
tokio = { version = "1", features = ["rt-multi-thread"] }
toml = "1"
```

`rust-toolchain.toml`:

```toml
[toolchain]
channel = "1.97"
components = ["rustfmt", "clippy"]
```

`.gitignore`:

```
/target
/data
```

`crates/barnacle-catalog/Cargo.toml`:

```toml
[package]
name = "barnacle-catalog"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true

[dependencies]
deunicode.workspace = true
serde.workspace = true
serde_json.workspace = true
thiserror.workspace = true
toml.workspace = true
```

`crates/barnacle-catalog/src/lib.rs`:

```rust
pub mod model;

pub use model::Catalog;
pub use model::ModelError;
pub use model::Nation;
pub use model::ParamId;
pub use model::Provenance;
pub use model::Ship;
pub use model::ShipClass;
pub use model::ShipGroup;
pub use model::ShipIndex;
pub use model::ShipName;
pub use model::Silhouette;
pub use model::Tier;
```

- [ ] **Step 3: Write the shared test builders and the failing model tests**

`crates/barnacle-catalog/tests/common/mod.rs`:

```rust
#![allow(dead_code)]

use barnacle_catalog::Catalog;
use barnacle_catalog::Nation;
use barnacle_catalog::ParamId;
use barnacle_catalog::Provenance;
use barnacle_catalog::Ship;
use barnacle_catalog::ShipClass;
use barnacle_catalog::ShipGroup;
use barnacle_catalog::ShipIndex;
use barnacle_catalog::ShipName;
use barnacle_catalog::Silhouette;
use barnacle_catalog::Tier;

pub fn index(value: &str) -> ShipIndex {
    ShipIndex::parse(value).unwrap()
}

pub fn ship(value: &str, name: &str, group: &str, tier: u32, silhouette: &str) -> Ship {
    Ship {
        id: ParamId::new(1),
        index: index(value),
        tier: Tier::new(tier).unwrap(),
        group: ShipGroup::new(group),
        class: ShipClass::Cruiser,
        nation: Nation::new("USA"),
        is_paper: false,
        name: Some(ShipName { short: name.to_owned(), full: Some(name.to_owned()) }),
        silhouette: Some(Silhouette { sha256: silhouette.to_owned() }),
    }
}

pub fn catalog(build: u32, ships: Vec<Ship>) -> Catalog {
    Catalog {
        provenance: Provenance {
            game_version: "15.8.0".to_owned(),
            build,
            data_repo_commit: "0000000".to_owned(),
            wowsunpack: "0.45.0".to_owned(),
            wows_data_mgr: "0.21.0".to_owned(),
        },
        ships,
    }
}
```

`crates/barnacle-catalog/tests/model.rs`:

```rust
mod common;

use barnacle_catalog::Catalog;
use barnacle_catalog::ModelError;
use barnacle_catalog::ShipIndex;
use barnacle_catalog::ShipName;
use barnacle_catalog::Tier;
use common::catalog;
use common::index;
use common::ship;

#[test]
fn ship_index_accepts_wargaming_indices() {
    assert_eq!(ShipIndex::parse("PASB008").unwrap().as_str(), "PASB008");
    assert_eq!(index("PGSC519").to_string(), "PGSC519");
}

#[test]
fn ship_index_rejects_malformed_values() {
    for bad in ["", "PASB08", "pasb008", "PASB0088", "PASB-08"] {
        assert_eq!(ShipIndex::parse(bad), Err(ModelError::InvalidIndex { value: bad.to_owned() }));
    }
}

#[test]
fn tier_accepts_one_through_eleven_only() {
    assert_eq!(Tier::new(1).unwrap().get(), 1);
    assert_eq!(Tier::new(11).unwrap().get(), 11);
    assert_eq!(Tier::new(0), Err(ModelError::InvalidTier { value: 0 }));
    assert_eq!(Tier::new(12), Err(ModelError::InvalidTier { value: 12 }));
    assert_eq!(Tier::new(300), Err(ModelError::InvalidTier { value: 300 }));
}

#[test]
fn display_name_prefers_the_full_name() {
    let arkansas = ShipName { short: "Arkansas B".to_owned(), full: Some("Arkansas Beta".to_owned()) };
    let goliath = ShipName { short: "Goliath".to_owned(), full: None };
    assert_eq!(arkansas.display(), "Arkansas Beta");
    assert_eq!(goliath.display(), "Goliath");
}

#[test]
fn catalog_round_trips_through_json() {
    let original = catalog(13187581, vec![ship("PASB008", "Colorado", "upgradeable", 7, "aa11")]);
    let json = original.to_json().unwrap();
    assert!(json.contains("\"index\": \"PASB008\""));
    assert_eq!(Catalog::from_json(&json).unwrap(), original);
}

#[test]
fn catalog_json_with_a_bad_index_or_tier_is_rejected() {
    let json = catalog(13187581, vec![ship("PASB008", "Colorado", "upgradeable", 7, "aa11")]).to_json().unwrap();
    assert!(Catalog::from_json(&json.replace("PASB008", "bad")).is_err());
    assert!(Catalog::from_json(&json.replace("\"tier\": 7", "\"tier\": 12")).is_err());
}

#[test]
fn catalog_finds_ships_by_index() {
    let found = catalog(13187581, vec![ship("PASB008", "Colorado", "upgradeable", 7, "aa11")]);
    assert_eq!(found.get(&index("PASB008")).map(|ship| ship.tier.get()), Some(7));
    assert!(found.get(&index("PBSC710")).is_none());
}
```

- [ ] **Step 4: Run the tests to verify they fail**

Run: `cargo test -p barnacle-catalog --test model`
Expected: compile error, `file not found for module model` (or unresolved imports from `barnacle_catalog`).

- [ ] **Step 5: Write the model**

`crates/barnacle-catalog/src/model.rs`:

```rust
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
        let well_formed =
            value.len() == 7 && value.bytes().all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit());
        if well_formed {
            Ok(Self(value.to_owned()))
        } else {
            Err(ModelError::InvalidIndex { value: value.to_owned() })
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
```

- [ ] **Step 6: Run the tests to verify they pass**

Run: `cargo test -p barnacle-catalog --test model`
Expected: `test result: ok. 7 passed; 0 failed`

- [ ] **Step 7: Lint**

Run: `cargo fmt --all --check && cargo clippy -p barnacle-catalog --all-targets -- -D warnings`
Expected: no output from fmt, clippy finishes with no warnings.

- [ ] **Step 8: Commit**

```bash
git add Cargo.toml Cargo.lock rust-toolchain.toml .gitignore crates/barnacle-catalog
git commit -m "feat(catalog): add workspace and ship catalog model"
```

---

### Task 2: Name normalisation

**Files:**
- Create: `crates/barnacle-catalog/src/names.rs`
- Modify: `crates/barnacle-catalog/src/lib.rs` (add `pub mod names;`)
- Test: `crates/barnacle-catalog/tests/names.rs`

**Interfaces:**
- Consumes: nothing.
- Produces:
  - `barnacle_catalog::names::clean_answer(&str) -> String`: ASCII-transliterated, lowercase, with whitespace and `- . ' ,` removed. Plan 2 uses it to compare chat messages with ship names.
  - `barnacle_catalog::names::name_tokens(&str) -> Vec<String>`: the same transliteration and character removal, split into lowercase words. The curation rules use it.

English ship names contain no middle dot (checked on 13.11 data), so unlike track's cleaning, `·` is not handled.

- [ ] **Step 1: Write the failing tests**

`crates/barnacle-catalog/tests/names.rs`:

```rust
use barnacle_catalog::names::clean_answer;
use barnacle_catalog::names::name_tokens;

#[test]
fn clean_answer_removes_spacing_and_punctuation() {
    assert_eq!(clean_answer("Des Moines"), "desmoines");
    assert_eq!(clean_answer("W. Virginia '41"), "wvirginia41");
    assert_eq!(clean_answer("Z-52"), "z52");
    assert_eq!(clean_answer("Sun Yat-Sen"), "sunyatsen");
    assert_eq!(clean_answer("  Kremlin  "), "kremlin");
}

#[test]
fn clean_answer_transliterates_accents_and_non_breaking_spaces() {
    assert_eq!(clean_answer("Ägir"), "agir");
    assert_eq!(clean_answer("Kongō"), "kongo");
    assert_eq!(clean_answer("Prins van\u{a0}Oranje"), "prinsvanoranje");
}

#[test]
fn name_tokens_splits_into_lowercase_ascii_words() {
    assert_eq!(name_tokens("AL Ägir"), ["al", "agir"]);
    assert_eq!(name_tokens("Prins van\u{a0}Oranje Golden"), ["prins", "van", "oranje", "golden"]);
    assert_eq!(name_tokens("Belfast '43"), ["belfast", "43"]);
    assert_eq!(name_tokens("AL Sov. Rossiya"), ["al", "sov", "rossiya"]);
    assert_eq!(name_tokens("Z-52"), ["z52"]);
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p barnacle-catalog --test names`
Expected: compile error, `could not find names in barnacle_catalog`.

- [ ] **Step 3: Implement**

`crates/barnacle-catalog/src/names.rs`:

```rust
use deunicode::deunicode;

const REMOVED: [char; 4] = ['-', '.', '\'', ','];

fn ascii_without_punctuation(text: &str) -> String {
    let spaced: String = text.chars().map(|c| if c.is_whitespace() { ' ' } else { c }).collect();
    deunicode(&spaced).chars().filter(|c| !REMOVED.contains(c)).collect()
}

pub fn clean_answer(text: &str) -> String {
    ascii_without_punctuation(text)
        .chars()
        .filter(|c| !c.is_whitespace())
        .flat_map(char::to_lowercase)
        .collect()
}

pub fn name_tokens(text: &str) -> Vec<String> {
    ascii_without_punctuation(text).split_whitespace().map(str::to_lowercase).collect()
}
```

Add to `crates/barnacle-catalog/src/lib.rs`, after `pub mod model;`:

```rust
pub mod names;
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p barnacle-catalog --test names`
Expected: `test result: ok. 3 passed`

If `deunicode` maps a character differently than these expectations, the test is right and the output is wrong: report the failing case to the owner rather than changing the expectation.

- [ ] **Step 5: Commit**

```bash
git add crates/barnacle-catalog/src/names.rs crates/barnacle-catalog/src/lib.rs crates/barnacle-catalog/tests/names.rs
git commit -m "feat(catalog): add answer cleaning and name tokens"
```

---

### Task 3: Curation config and automatic rules

**Files:**
- Create: `crates/barnacle-catalog/src/curation/mod.rs`, `crates/barnacle-catalog/src/curation/config.rs`, `crates/barnacle-catalog/src/curation/rules.rs`
- Modify: `crates/barnacle-catalog/src/lib.rs` (add `pub mod curation;`)
- Test: `crates/barnacle-catalog/tests/curation.rs`

**Interfaces:**
- Consumes: Task 1 model types; `names::name_tokens` from Task 2.
- Produces, re-exported from `barnacle_catalog::curation`:
  - `CurationConfig { reviewed_through: Option<u32>, groups: Vec<ShipGroup>, exclude: Vec<ExcludeEntry>, keep: Vec<KeepEntry>, lookalikes: Vec<LookalikeGroup>, aliases: Vec<AliasEntry> }` and `CurationConfig::from_toml(&str) -> Result<CurationConfig, toml::de::Error>`
  - `ExcludeEntry { index: ShipIndex, reason: ExcludeReason, base: Option<ShipIndex> }`
  - `enum ExcludeReason { CarbonCopy, BadSilhouette }` with `Display`
  - `KeepEntry { index: ShipIndex }`, `LookalikeGroup { ships: Vec<ShipIndex> }`, `AliasEntry { index: ShipIndex, names: Vec<String> }`
  - `enum Removal { GroupNotAllowed, NoEnglishName, NoSilhouette, IdenticalSilhouette { base: ShipIndex }, CollaborationPrefix, VariantSuffix { base: ShipIndex }, Manual { reason: ExcludeReason, base: Option<ShipIndex> } }` with `Removal::base(&self) -> Option<&ShipIndex>` and `Display`
  - `Curated { pool: BTreeSet<ShipIndex>, removed: BTreeMap<ShipIndex, Removal> }` with `Curated::variants_of(&self, &ShipIndex) -> Vec<&ShipIndex>`
  - `curate(&Catalog, &CurationConfig) -> Curated`
  - `COLLABORATION_PREFIXES: [&str; 5]`, `VARIANT_SUFFIXES: [&str; 4]`

The rules follow spec section 4.2, with one addition: a ship with no English name is removed (`NoEnglishName`), because it cannot be answered. `keep` protects a ship from rules 3-5 only. A manual `exclude` replaces an automatic 3-5 reason, which is how a reviewer sets a better base, but never overrides a group, name or silhouette removal.

- [ ] **Step 1: Write the failing tests**

Every index, name, group and tier below is real (13.11 data). Where two ships share a silhouette hash in a test, they share it in the 15.8.0 dump too.

`crates/barnacle-catalog/tests/curation.rs`:

```rust
mod common;

use barnacle_catalog::curation::CurationConfig;
use barnacle_catalog::curation::ExcludeReason;
use barnacle_catalog::curation::Removal;
use barnacle_catalog::curation::curate;
use common::catalog;
use common::index;
use common::ship;

const GROUPS: &str = r#"
groups = ["start", "special", "specialUnsellable", "ultimate", "upgradeable", "upgradeableExclusive", "upgradeableUltimate", "superShip"]
"#;

fn config(tables: &str) -> CurationConfig {
    CurationConfig::from_toml(&format!("{GROUPS}\n{tables}")).unwrap()
}

#[test]
fn config_parses_every_section() {
    let parsed = config(
        r#"
[[exclude]]
index = "PJSC708"
reason = "carbon_copy"
base = "PJSC038"

[[keep]]
index = "PGSD720"

[[lookalikes]]
ships = ["PBSC507", "PBSC528"]

[[aliases]]
index = "PRSB110"
names = ["kreml"]
"#,
    );
    assert_eq!(parsed.reviewed_through, None);
    assert_eq!(parsed.groups.len(), 8);
    assert_eq!(parsed.exclude[0].reason, ExcludeReason::CarbonCopy);
    assert_eq!(parsed.exclude[0].base, Some(index("PJSC038")));
    assert_eq!(parsed.keep[0].index, index("PGSD720"));
    assert_eq!(parsed.lookalikes[0].ships, vec![index("PBSC507"), index("PBSC528")]);
    assert_eq!(parsed.aliases[0].names, vec!["kreml".to_owned()]);
}

#[test]
fn config_rejects_unknown_keys_and_bad_indices() {
    assert!(CurationConfig::from_toml(&format!("{GROUPS}\nforbidden = []")).is_err());
    assert!(CurationConfig::from_toml(&format!("{GROUPS}\n[[keep]]\nindex = \"bad\"")).is_err());
}

#[test]
fn ships_outside_the_allowed_groups_are_removed() {
    let ships = catalog(1, vec![ship("PASS910", "Balao 2", "demoWithoutStatsPrem", 10, "a1")]);
    let curated = curate(&ships, &config(""));
    assert_eq!(curated.removed[&index("PASS910")], Removal::GroupNotAllowed);
    assert!(curated.pool.is_empty());
}

#[test]
fn ships_without_a_name_or_silhouette_are_removed() {
    let unnamed = barnacle_catalog::Ship { name: None, ..ship("PASB008", "Colorado", "upgradeable", 7, "a1") };
    let unseen = barnacle_catalog::Ship { silhouette: None, ..ship("PASB018", "Iowa", "upgradeable", 9, "a2") };
    let curated = curate(&catalog(1, vec![unnamed, unseen]), &config(""));
    assert_eq!(curated.removed[&index("PASB008")], Removal::NoEnglishName);
    assert_eq!(curated.removed[&index("PASB018")], Removal::NoSilhouette);
}

#[test]
fn identical_silhouettes_keep_the_tech_tree_ship() {
    let ships = catalog(
        1,
        vec![
            ship("PASD019", "Clemson", "upgradeable", 4, "db18"),
            ship("PASD704", "DD 214", "specialUnsellable", 4, "db18"),
            ship("PGSC519", "Ägir", "special", 9, "5fe2"),
            ship("PGSC899", "AL Ägir", "special", 9, "5fe2"),
            ship("PGSD710", "Georg Hoffmann", "ultimate", 10, "1e02"),
            ship("PGSD720", "Georg Hoffmann Golden", "ultimate", 10, "1e02"),
        ],
    );
    let curated = curate(&ships, &config(""));
    assert_eq!(curated.removed[&index("PASD704")], Removal::IdenticalSilhouette { base: index("PASD019") });
    assert_eq!(curated.removed[&index("PGSC899")], Removal::IdenticalSilhouette { base: index("PGSC519") });
    assert_eq!(curated.removed[&index("PGSD720")], Removal::IdenticalSilhouette { base: index("PGSD710") });
    assert_eq!(
        curated.pool.iter().map(|i| i.as_str()).collect::<Vec<_>>(),
        ["PASD019", "PGSC519", "PGSD710"]
    );
    assert_eq!(curated.variants_of(&index("PGSC519")), vec![&index("PGSC899")]);
}

#[test]
fn collaboration_reskins_are_removed_even_with_a_unique_silhouette() {
    let ships = catalog(
        1,
        vec![ship("PJSC038", "Atago", "special", 8, "57cd"), ship("PJSC708", "ARP Takao", "special", 8, "91e9")],
    );
    let curated = curate(&ships, &config(""));
    assert_eq!(curated.removed[&index("PJSC708")], Removal::CollaborationPrefix);
    assert!(curated.pool.contains(&index("PJSC038")));
}

#[test]
fn variant_suffixes_point_at_the_ship_they_copy() {
    let ships = catalog(
        1,
        vec![
            ship("PASB518", "Massachusetts", "special", 8, "23ba"),
            ship("PASB598", "Massachusetts B", "special", 8, "ebd0"),
        ],
    );
    let curated = curate(&ships, &config(""));
    assert_eq!(curated.removed[&index("PASB598")], Removal::VariantSuffix { base: index("PASB518") });
}

#[test]
fn different_ships_that_share_a_word_stay_in_the_pool() {
    let ships = catalog(
        1,
        vec![
            ship("PASD709", "Black", "special", 9, "c8f8"),
            ship("PBSC101", "Black Swan", "start", 1, "d313"),
            ship("PGSB105", "König", "upgradeable", 5, "66d7"),
            ship("PGSB503", "König Albert", "special", 3, "2538"),
            ship("PUSD503", "Vampire", "special", 3, "da5f"),
            ship("PUSD510", "Vampire II", "ultimate", 10, "d6f0"),
            ship("PBSC507", "Belfast", "special", 7, "c866"),
            ship("PBSC528", "Belfast '43", "special", 8, "61fc"),
        ],
    );
    let curated = curate(&ships, &config(""));
    assert!(curated.removed.is_empty());
    assert_eq!(curated.pool.len(), 8);
}

#[test]
fn keep_protects_a_ship_from_the_automatic_rules() {
    let ships = catalog(
        1,
        vec![
            ship("PGSD710", "Georg Hoffmann", "ultimate", 10, "1e02"),
            ship("PGSD720", "Georg Hoffmann Golden", "ultimate", 10, "1e02"),
        ],
    );
    let curated = curate(&ships, &config("[[keep]]\nindex = \"PGSD720\""));
    assert!(curated.removed.is_empty());
    assert!(curated.pool.contains(&index("PGSD720")));
}

#[test]
fn manual_exclusions_replace_automatic_reasons_but_not_group_removals() {
    let ships = catalog(
        1,
        vec![
            ship("PJSC038", "Atago", "special", 8, "57cd"),
            ship("PJSC708", "ARP Takao", "special", 8, "91e9"),
            ship("PASS910", "Balao 2", "demoWithoutStatsPrem", 10, "7640"),
        ],
    );
    let tables = r#"
[[exclude]]
index = "PJSC708"
reason = "carbon_copy"
base = "PJSC038"

[[exclude]]
index = "PASS910"
reason = "bad_silhouette"
"#;
    let curated = curate(&ships, &config(tables));
    assert_eq!(
        curated.removed[&index("PJSC708")],
        Removal::Manual { reason: ExcludeReason::CarbonCopy, base: Some(index("PJSC038")) }
    );
    assert_eq!(curated.removed[&index("PASS910")], Removal::GroupNotAllowed);
    assert_eq!(curated.variants_of(&index("PJSC038")), vec![&index("PJSC708")]);
}

#[test]
fn removals_explain_themselves() {
    assert_eq!(Removal::VariantSuffix { base: index("PASB518") }.to_string(), "variant of PASB518");
    assert_eq!(
        Removal::Manual { reason: ExcludeReason::BadSilhouette, base: None }.to_string(),
        "excluded by curation: bad silhouette"
    );
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p barnacle-catalog --test curation`
Expected: compile error, `could not find curation in barnacle_catalog`.

- [ ] **Step 3: Implement the config**

`crates/barnacle-catalog/src/curation/config.rs`:

```rust
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
```

The four `#[serde(default)]` lists default to empty because an absent section means "no entries", which is exactly what an empty list means.

- [ ] **Step 4: Implement the rules**

`crates/barnacle-catalog/src/curation/rules.rs`:

```rust
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::fmt;

use crate::curation::config::CurationConfig;
use crate::curation::config::ExcludeReason;
use crate::model::Catalog;
use crate::model::Ship;
use crate::model::ShipIndex;
use crate::names::name_tokens;

pub const COLLABORATION_PREFIXES: [&str; 5] = ["arp", "al", "hsf", "ba", "star"];
pub const VARIANT_SUFFIXES: [&str; 4] = ["b", "golden", "clr", "beta"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Removal {
    GroupNotAllowed,
    NoEnglishName,
    NoSilhouette,
    IdenticalSilhouette { base: ShipIndex },
    CollaborationPrefix,
    VariantSuffix { base: ShipIndex },
    Manual { reason: ExcludeReason, base: Option<ShipIndex> },
}

impl Removal {
    pub fn base(&self) -> Option<&ShipIndex> {
        match self {
            Self::IdenticalSilhouette { base } | Self::VariantSuffix { base } => Some(base),
            Self::Manual { base, .. } => base.as_ref(),
            Self::GroupNotAllowed | Self::NoEnglishName | Self::NoSilhouette | Self::CollaborationPrefix => None,
        }
    }
}

impl fmt::Display for Removal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::GroupNotAllowed => f.write_str("group not allowed"),
            Self::NoEnglishName => f.write_str("no English name"),
            Self::NoSilhouette => f.write_str("no silhouette"),
            Self::IdenticalSilhouette { base } => write!(f, "same silhouette as {base}"),
            Self::CollaborationPrefix => f.write_str("collaboration reskin"),
            Self::VariantSuffix { base } => write!(f, "variant of {base}"),
            Self::Manual { reason, base: Some(base) } => write!(f, "excluded by curation: {reason} of {base}"),
            Self::Manual { reason, base: None } => write!(f, "excluded by curation: {reason}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Curated {
    pub pool: BTreeSet<ShipIndex>,
    pub removed: BTreeMap<ShipIndex, Removal>,
}

impl Curated {
    pub fn variants_of(&self, base: &ShipIndex) -> Vec<&ShipIndex> {
        self.removed.iter().filter(|(_, removal)| removal.base() == Some(base)).map(|(index, _)| index).collect()
    }
}

#[derive(Clone)]
struct Candidate<'a> {
    ship: &'a Ship,
    tokens: Vec<String>,
    silhouette: &'a str,
}

impl<'a> Candidate<'a> {
    fn from_ship(ship: &'a Ship) -> Option<Self> {
        let name = ship.name.as_ref()?;
        let silhouette = ship.silhouette.as_ref()?;
        Some(Self { ship, tokens: name_tokens(name.display()), silhouette: &silhouette.sha256 })
    }

    fn index(&self) -> &'a ShipIndex {
        &self.ship.index
    }

    fn collaboration_prefix(&self) -> bool {
        self.tokens.len() > 1
            && self.tokens.first().is_some_and(|token| COLLABORATION_PREFIXES.contains(&token.as_str()))
    }

    fn variant_stem(&self) -> Option<&[String]> {
        match self.tokens.split_last() {
            Some((last, stem)) if !stem.is_empty() && VARIANT_SUFFIXES.contains(&last.as_str()) => Some(stem),
            _ => None,
        }
    }

    fn base_rank(&self) -> (bool, u8, &'a ShipIndex) {
        let marked = self.collaboration_prefix() || self.variant_stem().is_some();
        let group = match self.ship.group.as_str() {
            "upgradeable" => 0,
            "start" => 1,
            "special" => 2,
            _ => 3,
        };
        (marked, group, self.index())
    }
}

fn best_base<'a, 'c>(members: impl Iterator<Item = &'c Candidate<'a>>) -> Option<&'a ShipIndex>
where
    'a: 'c,
{
    members.min_by(|a, b| a.base_rank().cmp(&b.base_rank())).map(Candidate::index)
}

fn group_by<'a, 'c, K: Ord>(
    candidates: &'c [Candidate<'a>],
    key: impl Fn(&'c Candidate<'a>) -> K,
) -> BTreeMap<K, Vec<&'c Candidate<'a>>> {
    let mut groups: BTreeMap<K, Vec<&'c Candidate<'a>>> = BTreeMap::new();
    for candidate in candidates {
        groups.entry(key(candidate)).or_default().push(candidate);
    }
    groups
}

fn without<'a>(candidates: &[Candidate<'a>], removed: &BTreeMap<ShipIndex, Removal>) -> Vec<Candidate<'a>> {
    candidates.iter().filter(|candidate| !removed.contains_key(candidate.index())).cloned().collect()
}

fn basic_removal(ship: &Ship, config: &CurationConfig) -> Option<Removal> {
    if !config.groups.contains(&ship.group) {
        Some(Removal::GroupNotAllowed)
    } else if ship.name.is_none() {
        Some(Removal::NoEnglishName)
    } else if ship.silhouette.is_none() {
        Some(Removal::NoSilhouette)
    } else {
        None
    }
}

fn identical_silhouettes(candidates: &[Candidate<'_>], kept: &BTreeSet<&ShipIndex>) -> BTreeMap<ShipIndex, Removal> {
    group_by(candidates, |candidate| candidate.silhouette)
        .into_values()
        .filter(|members| members.len() > 1)
        .flat_map(|members| {
            let base = best_base(members.iter().copied());
            members
                .into_iter()
                .filter_map(move |member| {
                    let base = base?;
                    (member.index() != base && !kept.contains(member.index()))
                        .then(|| (member.index().clone(), Removal::IdenticalSilhouette { base: base.clone() }))
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

fn collaboration_prefixes(candidates: &[Candidate<'_>], kept: &BTreeSet<&ShipIndex>) -> BTreeMap<ShipIndex, Removal> {
    candidates
        .iter()
        .filter(|candidate| candidate.collaboration_prefix() && !kept.contains(candidate.index()))
        .map(|candidate| (candidate.index().clone(), Removal::CollaborationPrefix))
        .collect()
}

fn variant_suffixes(candidates: &[Candidate<'_>], kept: &BTreeSet<&ShipIndex>) -> BTreeMap<ShipIndex, Removal> {
    let by_name = group_by(candidates, |candidate| candidate.tokens.as_slice());
    candidates
        .iter()
        .filter(|candidate| !kept.contains(candidate.index()))
        .filter_map(|candidate| {
            let stem = candidate.variant_stem()?;
            let others = by_name.get(stem)?.iter().copied().filter(|other| other.index() != candidate.index());
            let base = best_base(others)?;
            Some((candidate.index().clone(), Removal::VariantSuffix { base: base.clone() }))
        })
        .collect()
}

fn manual_exclusions(
    catalog: &Catalog,
    config: &CurationConfig,
    basic: &BTreeMap<ShipIndex, Removal>,
) -> BTreeMap<ShipIndex, Removal> {
    config
        .exclude
        .iter()
        .filter(|entry| catalog.get(&entry.index).is_some() && !basic.contains_key(&entry.index))
        .map(|entry| (entry.index.clone(), Removal::Manual { reason: entry.reason, base: entry.base.clone() }))
        .collect()
}

pub fn curate(catalog: &Catalog, config: &CurationConfig) -> Curated {
    let kept: BTreeSet<&ShipIndex> = config.keep.iter().map(|entry| &entry.index).collect();
    let basic: BTreeMap<ShipIndex, Removal> = catalog
        .ships
        .iter()
        .filter_map(|ship| basic_removal(ship, config).map(|removal| (ship.index.clone(), removal)))
        .collect();
    let candidates: Vec<Candidate<'_>> = catalog
        .ships
        .iter()
        .filter(|ship| !basic.contains_key(&ship.index))
        .filter_map(Candidate::from_ship)
        .collect();

    let identical = identical_silhouettes(&candidates, &kept);
    let after_identical = without(&candidates, &identical);
    let collaboration = collaboration_prefixes(&after_identical, &kept);
    let after_collaboration = without(&after_identical, &collaboration);
    let suffixes = variant_suffixes(&after_collaboration, &kept);
    let manual = manual_exclusions(catalog, config, &basic);

    let removed: BTreeMap<ShipIndex, Removal> =
        basic.into_iter().chain(identical).chain(collaboration).chain(suffixes).chain(manual).collect();
    let pool = candidates
        .iter()
        .map(|candidate| candidate.index().clone())
        .filter(|index| !removed.contains_key(index))
        .collect();
    Curated { pool, removed }
}
```

`crates/barnacle-catalog/src/curation/mod.rs`:

```rust
mod config;
mod rules;

pub use config::AliasEntry;
pub use config::CurationConfig;
pub use config::ExcludeEntry;
pub use config::ExcludeReason;
pub use config::KeepEntry;
pub use config::LookalikeGroup;
pub use rules::COLLABORATION_PREFIXES;
pub use rules::Curated;
pub use rules::Removal;
pub use rules::VARIANT_SUFFIXES;
pub use rules::curate;
```

Add to `crates/barnacle-catalog/src/lib.rs`, above `pub mod model;`:

```rust
pub mod curation;
```

The chained `collect` into one map relies on later entries replacing earlier ones for the same index, which is what makes manual exclusions win over automatic reasons.

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p barnacle-catalog --test curation`
Expected: `test result: ok. 11 passed`

If the borrow checker rejects a lifetime in `best_base` or `group_by`, fix the signature, not the behaviour; the tests define the behaviour.

- [ ] **Step 6: Lint and commit**

Run: `cargo fmt --all --check && cargo clippy -p barnacle-catalog --all-targets -- -D warnings`
Expected: clean.

```bash
git add crates/barnacle-catalog/src/curation crates/barnacle-catalog/src/lib.rs crates/barnacle-catalog/tests/curation.rs
git commit -m "feat(catalog): add curation config and automatic carbon-copy rules"
```

---

### Task 4: Validation and catalog diffs

**Files:**
- Create: `crates/barnacle-catalog/src/curation/validate.rs`, `crates/barnacle-catalog/src/diff.rs`
- Modify: `crates/barnacle-catalog/src/curation/mod.rs`, `crates/barnacle-catalog/src/lib.rs`
- Test: `crates/barnacle-catalog/tests/validate.rs`, `crates/barnacle-catalog/tests/diff.rs`

**Interfaces:**
- Consumes: Tasks 1-3.
- Produces:
  - `barnacle_catalog::curation::validate(&Catalog, &CurationConfig, &Curated) -> Vec<Problem>`
  - `enum Section { Exclude, ExcludeBase, Keep, Lookalikes, Aliases }`
  - `enum Problem { UnknownIndex { section, index }, DuplicateExclude { index }, ExcludedAndKept { index }, InSeveralLookalikeGroups { index }, LookalikeNotInPool { index }, LookalikeGroupTooSmall { position: usize, eligible: usize }, NotReviewed { build: u32 }, ReviewBehind { reviewed_through: u32, build: u32 } }` with `Display`
  - `barnacle_catalog::diff::diff(Option<&Catalog>, &Catalog) -> CatalogDiff`
  - `CatalogDiff { added: Vec<ShipIndex>, removed: Vec<ShipIndex>, regrouped: Vec<Regrouped>, new_groups: Vec<ShipGroup> }`
  - `Regrouped { index: ShipIndex, from: ShipGroup, to: ShipGroup }`
  - `barnacle_catalog::diff::year_refit_candidates(&Catalog, &CurationConfig, &Curated) -> Vec<LookalikeCandidate>`
  - `LookalikeCandidate { original: ShipIndex, refit: ShipIndex }`

- [ ] **Step 1: Write the failing validation tests**

The first two cases are track's real defects: `PBSC710` named in a group while missing from the data (D1), and `PASA898` excluded twice (D2).

`crates/barnacle-catalog/tests/validate.rs`:

```rust
mod common;

use barnacle_catalog::curation::CurationConfig;
use barnacle_catalog::curation::Problem;
use barnacle_catalog::curation::Section;
use barnacle_catalog::curation::curate;
use barnacle_catalog::curation::validate;
use common::catalog;
use common::index;
use common::ship;

const BUILD: u32 = 13187581;

fn problems(ships: Vec<barnacle_catalog::Ship>, text: &str) -> Vec<Problem> {
    let catalog = catalog(BUILD, ships);
    let config = CurationConfig::from_toml(text).unwrap();
    let curated = curate(&catalog, &config);
    validate(&catalog, &config, &curated)
}

fn reviewed(tables: &str) -> String {
    format!("reviewed_through = {BUILD}\ngroups = [\"special\", \"upgradeable\"]\n{tables}")
}

#[test]
fn a_reviewed_config_with_resolvable_entries_has_no_problems() {
    let ships = vec![
        ship("PBSC507", "Belfast", "special", 7, "c866"),
        ship("PBSC528", "Belfast '43", "special", 8, "61fc"),
    ];
    assert_eq!(problems(ships, &reviewed("[[lookalikes]]\nships = [\"PBSC507\", \"PBSC528\"]")), vec![]);
}

#[test]
fn a_lookalike_naming_a_missing_ship_is_reported() {
    let ships = vec![ship("PBSC210", "Goliath", "upgradeable", 10, "b1")];
    let found = problems(ships, &reviewed("[[lookalikes]]\nships = [\"PBSC210\", \"PBSC710\"]"));
    assert_eq!(
        found,
        vec![
            Problem::UnknownIndex { section: Section::Lookalikes, index: index("PBSC710") },
            Problem::LookalikeGroupTooSmall { position: 1, eligible: 1 },
        ]
    );
}

#[test]
fn a_ship_excluded_twice_is_reported() {
    let ships = vec![ship("PASA898", "AL Hornet", "special", 8, "c1")];
    let tables = "[[exclude]]\nindex = \"PASA898\"\nreason = \"carbon_copy\"\n\n[[exclude]]\nindex = \"PASA898\"\nreason = \"carbon_copy\"";
    assert_eq!(problems(ships, &reviewed(tables)), vec![Problem::DuplicateExclude { index: index("PASA898") }]);
}

#[test]
fn excluded_and_kept_and_unknown_bases_are_reported() {
    let ships = vec![ship("PJSC708", "ARP Takao", "special", 8, "91e9")];
    let tables = "[[exclude]]\nindex = \"PJSC708\"\nreason = \"carbon_copy\"\nbase = \"PJSC038\"\n\n[[keep]]\nindex = \"PJSC708\"";
    assert_eq!(
        problems(ships, &reviewed(tables)),
        vec![
            Problem::UnknownIndex { section: Section::ExcludeBase, index: index("PJSC038") },
            Problem::ExcludedAndKept { index: index("PJSC708") },
        ]
    );
}

#[test]
fn lookalike_members_must_be_in_the_pool_and_in_one_group() {
    let ships = vec![
        ship("PGSC519", "Ägir", "special", 9, "5fe2"),
        ship("PGSC899", "AL Ägir", "special", 9, "5fe2"),
        ship("PBSC507", "Belfast", "special", 7, "c866"),
    ];
    let tables = "[[lookalikes]]\nships = [\"PGSC519\", \"PGSC899\"]\n\n[[lookalikes]]\nships = [\"PGSC519\", \"PBSC507\"]";
    assert_eq!(
        problems(ships, &reviewed(tables)),
        vec![
            Problem::InSeveralLookalikeGroups { index: index("PGSC519") },
            Problem::LookalikeNotInPool { index: index("PGSC899") },
            Problem::LookalikeGroupTooSmall { position: 1, eligible: 1 },
        ]
    );
}

#[test]
fn curation_must_be_reviewed_for_the_catalog_build() {
    let ships = vec![ship("PBSC507", "Belfast", "special", 7, "c866")];
    assert_eq!(
        problems(ships.clone(), "groups = [\"special\"]"),
        vec![Problem::NotReviewed { build: BUILD }]
    );
    assert_eq!(
        problems(ships, "reviewed_through = 13015811\ngroups = [\"special\"]"),
        vec![Problem::ReviewBehind { reviewed_through: 13015811, build: BUILD }]
    );
}

#[test]
fn problems_read_as_sentences() {
    assert_eq!(
        Problem::UnknownIndex { section: Section::Lookalikes, index: index("PBSC710") }.to_string(),
        "lookalikes names PBSC710, which is not in the catalog"
    );
    assert_eq!(
        Problem::NotReviewed { build: BUILD }.to_string(),
        "curation has never been reviewed; review build 13187581 and set reviewed_through"
    );
}
```

- [ ] **Step 2: Write the failing diff tests**

`crates/barnacle-catalog/tests/diff.rs`:

```rust
mod common;

use barnacle_catalog::ShipGroup;
use barnacle_catalog::curation::CurationConfig;
use barnacle_catalog::curation::curate;
use barnacle_catalog::diff::LookalikeCandidate;
use barnacle_catalog::diff::Regrouped;
use barnacle_catalog::diff::diff;
use barnacle_catalog::diff::year_refit_candidates;
use common::catalog;
use common::index;
use common::ship;

#[test]
fn diff_lists_added_removed_and_regrouped_ships() {
    let old = catalog(
        13015811,
        vec![
            ship("PASB008", "Colorado", "upgradeable", 7, "a1"),
            ship("PASS910", "Balao 2", "demoWithoutStatsPrem", 10, "a2"),
            ship("PBSC710", "Monmouth", "upgradeable", 7, "a3"),
        ],
    );
    let new = catalog(
        13187581,
        vec![
            ship("PASB008", "Colorado", "upgradeable", 7, "a1"),
            ship("PASS910", "Balao 2", "special", 10, "a2"),
            ship("PBSC210", "Goliath", "upgradeable", 10, "a4"),
        ],
    );
    let changes = diff(Some(&old), &new);
    assert_eq!(changes.added, vec![index("PBSC210")]);
    assert_eq!(changes.removed, vec![index("PBSC710")]);
    assert_eq!(
        changes.regrouped,
        vec![Regrouped {
            index: index("PASS910"),
            from: ShipGroup::new("demoWithoutStatsPrem"),
            to: ShipGroup::new("special"),
        }]
    );
    assert_eq!(changes.new_groups, vec![ShipGroup::new("special")]);
}

#[test]
fn a_first_diff_treats_every_ship_and_group_as_new() {
    let new = catalog(13187581, vec![ship("PASB008", "Colorado", "upgradeable", 7, "a1")]);
    let changes = diff(None, &new);
    assert_eq!(changes.added, vec![index("PASB008")]);
    assert_eq!(changes.new_groups, vec![ShipGroup::new("upgradeable")]);
}

#[test]
fn year_suffixed_refits_are_lookalike_candidates_until_grouped() {
    let ships = catalog(
        13187581,
        vec![
            ship("PBSC507", "Belfast", "special", 7, "c866"),
            ship("PBSC528", "Belfast '43", "special", 8, "61fc"),
            ship("PGSD710", "Georg Hoffmann", "ultimate", 10, "1e02"),
        ],
    );
    let groups = "groups = [\"special\", \"ultimate\"]";
    let ungrouped = CurationConfig::from_toml(groups).unwrap();
    assert_eq!(
        year_refit_candidates(&ships, &ungrouped, &curate(&ships, &ungrouped)),
        vec![LookalikeCandidate { original: index("PBSC507"), refit: index("PBSC528") }]
    );
    let grouped =
        CurationConfig::from_toml(&format!("{groups}\n[[lookalikes]]\nships = [\"PBSC507\", \"PBSC528\"]")).unwrap();
    assert!(year_refit_candidates(&ships, &grouped, &curate(&ships, &grouped)).is_empty());
}
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test -p barnacle-catalog --test validate --test diff`
Expected: compile errors for the missing `validate`, `Problem`, `Section` and `diff` items.

- [ ] **Step 4: Implement validation**

`crates/barnacle-catalog/src/curation/validate.rs`:

```rust
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::fmt;

use crate::curation::config::CurationConfig;
use crate::curation::rules::Curated;
use crate::model::Catalog;
use crate::model::ShipIndex;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Section {
    Exclude,
    ExcludeBase,
    Keep,
    Lookalikes,
    Aliases,
}

impl fmt::Display for Section {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Exclude => "exclude",
            Self::ExcludeBase => "exclude base",
            Self::Keep => "keep",
            Self::Lookalikes => "lookalikes",
            Self::Aliases => "aliases",
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Problem {
    #[error("{section} names {index}, which is not in the catalog")]
    UnknownIndex { section: Section, index: ShipIndex },
    #[error("{index} is excluded more than once")]
    DuplicateExclude { index: ShipIndex },
    #[error("{index} is both excluded and kept")]
    ExcludedAndKept { index: ShipIndex },
    #[error("{index} appears in more than one lookalike group")]
    InSeveralLookalikeGroups { index: ShipIndex },
    #[error("{index} is in a lookalike group but is not in the pool")]
    LookalikeNotInPool { index: ShipIndex },
    #[error("lookalike group {position} has {eligible} ship(s) in the pool; it needs at least 2")]
    LookalikeGroupTooSmall { position: usize, eligible: usize },
    #[error("curation has never been reviewed; review build {build} and set reviewed_through")]
    NotReviewed { build: u32 },
    #[error("curation was reviewed through build {reviewed_through}, but the catalog is build {build}")]
    ReviewBehind { reviewed_through: u32, build: u32 },
}

pub fn validate(catalog: &Catalog, config: &CurationConfig, curated: &Curated) -> Vec<Problem> {
    [
        unknown_indices(catalog, config),
        duplicate_excludes(config),
        excluded_and_kept(config),
        lookalike_problems(config, curated),
        review_state(catalog, config),
    ]
    .into_iter()
    .flatten()
    .collect()
}

fn unknown_indices(catalog: &Catalog, config: &CurationConfig) -> Vec<Problem> {
    config
        .exclude
        .iter()
        .map(|entry| (Section::Exclude, &entry.index))
        .chain(config.exclude.iter().filter_map(|entry| entry.base.as_ref()).map(|base| (Section::ExcludeBase, base)))
        .chain(config.keep.iter().map(|entry| (Section::Keep, &entry.index)))
        .chain(config.lookalikes.iter().flat_map(|group| &group.ships).map(|index| (Section::Lookalikes, index)))
        .chain(config.aliases.iter().map(|entry| (Section::Aliases, &entry.index)))
        .filter(|(_, index)| catalog.get(index).is_none())
        .map(|(section, index)| Problem::UnknownIndex { section, index: index.clone() })
        .collect()
}

fn repeated<'a>(indices: impl Iterator<Item = &'a ShipIndex>) -> Vec<ShipIndex> {
    let counts = indices.fold(BTreeMap::<&ShipIndex, usize>::new(), |mut counts, index| {
        *counts.entry(index).or_insert(0) += 1;
        counts
    });
    counts.into_iter().filter(|(_, count)| *count > 1).map(|(index, _)| index.clone()).collect()
}

fn duplicate_excludes(config: &CurationConfig) -> Vec<Problem> {
    repeated(config.exclude.iter().map(|entry| &entry.index))
        .into_iter()
        .map(|index| Problem::DuplicateExclude { index })
        .collect()
}

fn excluded_and_kept(config: &CurationConfig) -> Vec<Problem> {
    let kept: BTreeSet<&ShipIndex> = config.keep.iter().map(|entry| &entry.index).collect();
    config
        .exclude
        .iter()
        .filter(|entry| kept.contains(&entry.index))
        .map(|entry| Problem::ExcludedAndKept { index: entry.index.clone() })
        .collect()
}

fn lookalike_problems(config: &CurationConfig, curated: &Curated) -> Vec<Problem> {
    let members = || config.lookalikes.iter().flat_map(|group| &group.ships);
    let several = repeated(members()).into_iter().map(|index| Problem::InSeveralLookalikeGroups { index });
    let outside = members()
        .filter(|index| curated.removed.contains_key(*index))
        .map(|index| Problem::LookalikeNotInPool { index: index.clone() });
    let small = config.lookalikes.iter().enumerate().filter_map(|(position, group)| {
        let eligible = group.ships.iter().filter(|index| curated.pool.contains(*index)).count();
        (eligible < 2).then_some(Problem::LookalikeGroupTooSmall { position: position + 1, eligible })
    });
    several.chain(outside).chain(small).collect()
}

fn review_state(catalog: &Catalog, config: &CurationConfig) -> Vec<Problem> {
    let build = catalog.provenance.build;
    match config.reviewed_through {
        None => vec![Problem::NotReviewed { build }],
        Some(reviewed_through) if reviewed_through < build => vec![Problem::ReviewBehind { reviewed_through, build }],
        Some(_) => Vec::new(),
    }
}
```

Update `crates/barnacle-catalog/src/curation/mod.rs`: add `mod validate;` under `mod rules;`, and these exports at the end:

```rust
pub use validate::Problem;
pub use validate::Section;
pub use validate::validate;
```

- [ ] **Step 5: Implement diffs**

`crates/barnacle-catalog/src/diff.rs`:

```rust
use std::collections::BTreeMap;
use std::collections::BTreeSet;

use crate::curation::Curated;
use crate::curation::CurationConfig;
use crate::model::Catalog;
use crate::model::Ship;
use crate::model::ShipGroup;
use crate::model::ShipIndex;
use crate::names::name_tokens;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Regrouped {
    pub index: ShipIndex,
    pub from: ShipGroup,
    pub to: ShipGroup,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogDiff {
    pub added: Vec<ShipIndex>,
    pub removed: Vec<ShipIndex>,
    pub regrouped: Vec<Regrouped>,
    pub new_groups: Vec<ShipGroup>,
}

pub fn diff(old: Option<&Catalog>, new: &Catalog) -> CatalogDiff {
    let old_ships: BTreeMap<&ShipIndex, &Ship> =
        old.into_iter().flat_map(|catalog| &catalog.ships).map(|ship| (&ship.index, ship)).collect();
    let new_ships: BTreeMap<&ShipIndex, &Ship> = new.ships.iter().map(|ship| (&ship.index, ship)).collect();
    let old_groups: BTreeSet<&ShipGroup> = old_ships.values().map(|ship| &ship.group).collect();
    let new_groups: BTreeSet<&ShipGroup> = new_ships.values().map(|ship| &ship.group).collect();
    CatalogDiff {
        added: new_ships.keys().filter(|index| !old_ships.contains_key(**index)).map(|index| (*index).clone()).collect(),
        removed: old_ships.keys().filter(|index| !new_ships.contains_key(**index)).map(|index| (*index).clone()).collect(),
        regrouped: new_ships
            .values()
            .filter_map(|ship| {
                let before = old_ships.get(&ship.index)?;
                (before.group != ship.group).then(|| Regrouped {
                    index: ship.index.clone(),
                    from: before.group.clone(),
                    to: ship.group.clone(),
                })
            })
            .collect(),
        new_groups: new_groups.difference(&old_groups).map(|group| (*group).clone()).collect(),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LookalikeCandidate {
    pub original: ShipIndex,
    pub refit: ShipIndex,
}

pub fn year_refit_candidates(catalog: &Catalog, config: &CurationConfig, curated: &Curated) -> Vec<LookalikeCandidate> {
    let pool: Vec<(&Ship, Vec<String>)> = catalog
        .ships
        .iter()
        .filter(|ship| curated.pool.contains(&ship.index))
        .filter_map(|ship| ship.name.as_ref().map(|name| (ship, name_tokens(name.display()))))
        .collect();
    let by_name: BTreeMap<&[String], &Ship> = pool.iter().map(|(ship, tokens)| (tokens.as_slice(), *ship)).collect();
    pool.iter()
        .filter_map(|(refit, tokens)| {
            let (year, stem) = tokens.split_last()?;
            let is_year = year.len() == 2 && year.bytes().all(|byte| byte.is_ascii_digit());
            let original = by_name.get(stem).filter(|_| is_year && !stem.is_empty())?;
            let grouped = config
                .lookalikes
                .iter()
                .any(|group| group.ships.contains(&original.index) && group.ships.contains(&refit.index));
            (!grouped).then(|| LookalikeCandidate { original: original.index.clone(), refit: refit.index.clone() })
        })
        .collect()
}
```

Add to `crates/barnacle-catalog/src/lib.rs`, after `pub mod curation;`:

```rust
pub mod diff;
```

- [ ] **Step 6: Run the whole catalog suite**

Run: `cargo test -p barnacle-catalog`
Expected: every test binary reports `ok`; `validate` 7 passed, `diff` 3 passed.

- [ ] **Step 7: Lint and commit**

Run: `cargo fmt --all --check && cargo clippy -p barnacle-catalog --all-targets -- -D warnings`
Expected: clean.

```bash
git add crates/barnacle-catalog/src crates/barnacle-catalog/tests/validate.rs crates/barnacle-catalog/tests/diff.rs
git commit -m "feat(catalog): validate curation and diff catalogs"
```

---

### Task 5: Leaf extractors: paper flags, English names, silhouettes

**Files:**
- Create: `crates/barnacle-data/Cargo.toml`, `crates/barnacle-data/src/lib.rs`, `crates/barnacle-data/src/extract/mod.rs`
- Create: `crates/barnacle-data/src/extract/paper.rs`, `crates/barnacle-data/src/extract/translations.rs`, `crates/barnacle-data/src/extract/silhouette.rs`
- Create: `crates/barnacle-data/tests/fixtures/make_fixtures.py` and the four fixture files it writes
- Test: `crates/barnacle-data/tests/paper.rs`, `crates/barnacle-data/tests/translations.rs`, `crates/barnacle-data/tests/silhouette.rs`

**Interfaces:**
- Consumes: `barnacle_catalog::{ShipIndex, ShipName}`.
- Produces:
  - `barnacle_data::extract::paper::paper_flags(Vec<u8>) -> Result<BTreeMap<String, bool>, PaperError>`, keyed by ship index
  - `PaperError { Decode(GameDataError), UnexpectedRoot, MissingFlag { indices: Vec<String> } }`
  - `barnacle_data::extract::translations::EnglishNames::load(&Path) -> Result<EnglishNames, NamesError>` and `EnglishNames::ship_name(&self, &ShipIndex) -> Option<ShipName>`
  - `NamesError { Open { path, source }, Parse { path, source } }`
  - `barnacle_data::extract::silhouette::{BACKGROUND: Rgba<u8>, composite(&[u8], Rgba<u8>) -> Result<Vec<u8>, image::ImageError>, sha256_hex(&[u8]) -> String}`

The fixtures are synthetic: a few hand-written dictionaries in WG's GameParams container format (a pickle, zlib-compressed, byte-reversed), and a tiny gettext catalog. They contain no Wargaming data.

- [ ] **Step 1: Create the crate**

`crates/barnacle-data/Cargo.toml`:

```toml
[package]
name = "barnacle-data"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true

[dependencies]
barnacle-catalog.workspace = true
clap.workspace = true
gettext.workspace = true
image.workspace = true
pickled.workspace = true
reqwest.workspace = true
rootcause.workspace = true
serde_json.workspace = true
sha2.workspace = true
thiserror.workspace = true
tokio.workspace = true
toml.workspace = true
wows-data-mgr.workspace = true
wowsunpack.workspace = true

[dev-dependencies]
tempfile.workspace = true
```

`crates/barnacle-data/src/lib.rs`:

```rust
pub mod extract;
```

`crates/barnacle-data/src/extract/mod.rs`:

```rust
pub mod paper;
pub mod silhouette;
pub mod translations;
```

- [ ] **Step 2: Write and run the fixture generator**

`crates/barnacle-data/tests/fixtures/make_fixtures.py`:

```python
import pickle
import struct
import zlib
from pathlib import Path

HERE = Path(__file__).parent


def ship(index, paper):
    entry = {
        "index": index,
        "typeinfo": {"type": "Ship", "nation": "USA", "species": "Battleship"},
    }
    if paper is not None:
        entry["isPaperShip"] = paper
    return entry


def game_params(root):
    return zlib.compress(pickle.dumps(root, protocol=2))[::-1]


def mo_file(messages):
    keys = sorted(messages)
    ids = [key.encode() for key in keys]
    strs = [messages[key].encode() for key in keys]
    count = len(keys)
    data_start = 28 + 16 * count
    offsets = []
    position = data_start
    for blob in ids + strs:
        offsets.append((len(blob), position))
        position += len(blob) + 1
    header = struct.pack("<7I", 0x950412DE, 0, count, 28, 28 + 8 * count, 0, 0)
    tables = b"".join(struct.pack("<2I", length, offset) for length, offset in offsets)
    return header + tables + b"".join(blob + b"\0" for blob in ids + strs)


ships = {
    "PASB008_Colorado": ship("PASB008", False),
    "PASB110_Vermont": ship("PASB110", True),
    "PAPT001_Torpedo": {
        "index": "PAPT001",
        "typeinfo": {"type": "Projectile", "nation": "USA", "species": "Torpedo"},
    },
}

(HERE / "mini_gameparams.data").write_bytes(game_params({"": ships}))
(HERE / "mini_gameparams_list_root.data").write_bytes(game_params([ships]))
(HERE / "mini_gameparams_missing_flag.data").write_bytes(
    game_params({"": {**ships, "PJSB018_Yamato": ship("PJSB018", None)}})
)
(HERE / "mini_en.mo").write_bytes(
    mo_file(
        {
            "": "Content-Type: text/plain; charset=UTF-8\n",
            "IDS_PASB008": "Colorado",
            "IDS_PASB008_FULL": "Colorado",
            "IDS_PBSC210": "Goliath",
            "IDS_PGSC519": "Ägir",
            "IDS_PGSC519_FULL": "Ägir",
        }
    )
)
```

Run: `python3 crates/barnacle-data/tests/fixtures/make_fixtures.py && ls crates/barnacle-data/tests/fixtures`
Expected: the script, plus `mini_en.mo`, `mini_gameparams.data`, `mini_gameparams_list_root.data`, `mini_gameparams_missing_flag.data`.

- [ ] **Step 3: Write the failing tests**

`crates/barnacle-data/tests/paper.rs`:

```rust
use barnacle_data::extract::paper::PaperError;
use barnacle_data::extract::paper::paper_flags;

fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"))).unwrap()
}

#[test]
fn reads_paper_flags_for_ships_only() {
    let flags = paper_flags(fixture("mini_gameparams.data")).unwrap();
    assert_eq!(flags.get("PASB008"), Some(&false));
    assert_eq!(flags.get("PASB110"), Some(&true));
    assert_eq!(flags.get("PAPT001"), None);
    assert_eq!(flags.len(), 2);
}

#[test]
fn reads_the_older_list_rooted_layout() {
    let flags = paper_flags(fixture("mini_gameparams_list_root.data")).unwrap();
    assert_eq!(flags.get("PASB110"), Some(&true));
}

#[test]
fn a_ship_without_the_flag_fails_loudly() {
    let error = paper_flags(fixture("mini_gameparams_missing_flag.data")).unwrap_err();
    assert!(matches!(error, PaperError::MissingFlag { ref indices } if indices == &["PJSB018".to_owned()]));
}

#[test]
fn bytes_that_are_not_game_params_are_a_decode_error() {
    assert!(matches!(paper_flags(b"not game params".to_vec()), Err(PaperError::Decode(_))));
}
```

`crates/barnacle-data/tests/translations.rs`:

```rust
use std::path::Path;

use barnacle_catalog::ShipIndex;
use barnacle_catalog::ShipName;
use barnacle_data::extract::translations::EnglishNames;
use barnacle_data::extract::translations::NamesError;

fn names() -> EnglishNames {
    EnglishNames::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/mini_en.mo")).unwrap()
}

fn index(value: &str) -> ShipIndex {
    ShipIndex::parse(value).unwrap()
}

#[test]
fn finds_short_and_full_names() {
    assert_eq!(
        names().ship_name(&index("PASB008")),
        Some(ShipName { short: "Colorado".to_owned(), full: Some("Colorado".to_owned()) })
    );
    assert_eq!(names().ship_name(&index("PGSC519")).map(|name| name.short), Some("Ägir".to_owned()));
}

#[test]
fn a_missing_full_name_is_absent_not_the_key() {
    assert_eq!(names().ship_name(&index("PBSC210")), Some(ShipName { short: "Goliath".to_owned(), full: None }));
}

#[test]
fn a_ship_without_any_name_has_none() {
    assert_eq!(names().ship_name(&index("PBSC710")), None);
}

#[test]
fn a_missing_catalog_file_is_an_open_error() {
    let error = EnglishNames::load(Path::new("/nonexistent/global.mo")).err().unwrap();
    assert!(matches!(error, NamesError::Open { .. }));
}
```

`crates/barnacle-data/tests/silhouette.rs`:

```rust
use barnacle_data::extract::silhouette::BACKGROUND;
use barnacle_data::extract::silhouette::composite;
use barnacle_data::extract::silhouette::sha256_hex;
use image::ImageFormat;
use image::Rgba;
use image::RgbaImage;

fn encode(image: &RgbaImage) -> Vec<u8> {
    let mut cursor = std::io::Cursor::new(Vec::new());
    image.write_to(&mut cursor, ImageFormat::Png).unwrap();
    cursor.into_inner()
}

#[test]
fn transparent_pixels_take_the_background_and_opaque_pixels_stay() {
    let white = Rgba([255, 255, 255, 255]);
    let clear = Rgba([0, 0, 0, 0]);
    let source = RgbaImage::from_fn(404, 155, |x, _| if x < 200 { white } else { clear });
    let result = image::load_from_memory(&composite(&encode(&source), BACKGROUND).unwrap()).unwrap().to_rgba8();
    assert_eq!(result.dimensions(), (404, 155));
    assert_eq!(*result.get_pixel(10, 10), white);
    assert_eq!(*result.get_pixel(300, 10), BACKGROUND);
}

#[test]
fn bytes_that_are_not_a_png_are_rejected() {
    assert!(composite(b"not a png", BACKGROUND).is_err());
}

#[test]
fn hash_is_lowercase_sha256_hex() {
    assert_eq!(sha256_hex(b""), "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
}
```

- [ ] **Step 4: Run the tests to verify they fail**

Run: `cargo test -p barnacle-data --test paper --test translations --test silhouette`
Expected: compile errors, `file not found for module paper` (and the same for the other two). The first build also compiles wowsunpack and wows-data-mgr, which takes a few minutes.

- [ ] **Step 5: Implement the paper-flag reader**

This mirrors the traversal in `wowsunpack` 0.45.0 `src/game_params/provider.rs` (`ValueDictExt` and `params_from_data`). If a `pickled` method name differs, run `cargo doc -p pickled --open` and match the name wowsunpack uses; do not change the behaviour.

`crates/barnacle-data/src/extract/paper.rs`:

```rust
use std::collections::BTreeMap;

use pickled::HashableValue;
use pickled::Value;
use pickled::object::DictObject;
use pickled::value::Shared;
use wowsunpack::error::GameDataError;
use wowsunpack::game_params::convert::game_params_to_pickle;

#[derive(Debug, thiserror::Error)]
pub enum PaperError {
    #[error("GameParams could not be decoded")]
    Decode(#[from] GameDataError),
    #[error("GameParams root is not a dictionary, list or tuple")]
    UnexpectedRoot,
    #[error("ships without a boolean isPaperShip: {indices:?}")]
    MissingFlag { indices: Vec<String> },
}

fn key(name: &str) -> HashableValue {
    HashableValue::String(name.to_owned().into())
}

fn as_dict(value: &Value) -> Option<Shared<pickled::Dict>> {
    match value {
        Value::Dict(dict) => Some(dict.clone()),
        Value::Object(object) => {
            let object = object.inner();
            let state = object.as_any().downcast_ref::<DictObject>()?.state().clone();
            Some(Shared::new(state))
        }
        _ => None,
    }
}

fn params_dict(root: &Value) -> Option<Shared<pickled::Dict>> {
    if let Some(dict) = as_dict(root) {
        let wrapped = dict.inner().get(&key("")).and_then(as_dict);
        return Some(wrapped.unwrap_or(dict));
    }
    let first = root
        .list_ref()
        .and_then(|list| list.inner().first().cloned())
        .or_else(|| root.tuple_ref().and_then(|tuple| tuple.inner().first().cloned()))?;
    as_dict(&first)
}

fn string_field(entry: &pickled::Dict, name: &str) -> Option<String> {
    entry.get(&key(name)).and_then(Value::string_ref).map(|text| text.inner().to_string())
}

fn is_ship(entry: &pickled::Dict) -> bool {
    entry
        .get(&key("typeinfo"))
        .and_then(as_dict)
        .and_then(|typeinfo| string_field(&typeinfo.inner(), "type"))
        .is_some_and(|kind| kind == "Ship")
}

pub fn paper_flags(game_params: Vec<u8>) -> Result<BTreeMap<String, bool>, PaperError> {
    let root = game_params_to_pickle(game_params)?;
    let params = params_dict(&root).ok_or(PaperError::UnexpectedRoot)?;
    let ships: Vec<(String, Option<bool>)> = params
        .inner()
        .values()
        .filter_map(as_dict)
        .filter_map(|entry| {
            let entry = entry.inner();
            if !is_ship(&entry) {
                return None;
            }
            let index = string_field(&entry, "index")?;
            let flag = entry.get(&key("isPaperShip")).and_then(Value::bool_ref).copied();
            Some((index, flag))
        })
        .collect();
    let missing: Vec<String> =
        ships.iter().filter(|(_, flag)| flag.is_none()).map(|(index, _)| index.clone()).collect();
    if !missing.is_empty() {
        return Err(PaperError::MissingFlag { indices: missing });
    }
    Ok(ships.into_iter().filter_map(|(index, flag)| flag.map(|flag| (index, flag))).collect())
}
```

A GameParams entry with no `index` is not a ship players can own and is skipped. A ship entry with no boolean `isPaperShip` fails the whole build, because a silent default would put paper ships into historical rounds.

- [ ] **Step 6: Implement English names**

`crates/barnacle-data/src/extract/translations.rs`:

```rust
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
        let file = File::open(path).map_err(|source| NamesError::Open { path: path.to_owned(), source })?;
        let catalog =
            gettext::Catalog::parse(file).map_err(|source| NamesError::Parse { path: path.to_owned(), source })?;
        Ok(Self { catalog })
    }

    pub fn ship_name(&self, index: &ShipIndex) -> Option<ShipName> {
        let short = self.lookup(&format!("IDS_{index}"))?;
        Some(ShipName { short, full: self.lookup(&format!("IDS_{index}_FULL")) })
    }

    fn lookup(&self, key: &str) -> Option<String> {
        let value = self.catalog.gettext(key);
        (value != key && !value.trim().is_empty()).then(|| value.to_owned())
    }
}
```

`gettext` returns the key itself when a message is missing, which is why `lookup` compares against the key.

- [ ] **Step 7: Implement silhouettes**

`crates/barnacle-data/src/extract/silhouette.rs`:

```rust
use image::ImageFormat;
use image::Rgba;
use image::RgbaImage;
use sha2::Digest;
use sha2::Sha256;

pub const BACKGROUND: Rgba<u8> = Rgba([0x0B, 0x1F, 0x33, 0xFF]);

pub fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

pub fn composite(silhouette_png: &[u8], background: Rgba<u8>) -> Result<Vec<u8>, image::ImageError> {
    let silhouette = image::load_from_memory_with_format(silhouette_png, ImageFormat::Png)?.to_rgba8();
    let mut canvas = RgbaImage::from_pixel(silhouette.width(), silhouette.height(), background);
    image::imageops::overlay(&mut canvas, &silhouette, 0, 0);
    let mut encoded = std::io::Cursor::new(Vec::new());
    canvas.write_to(&mut encoded, ImageFormat::Png)?;
    Ok(encoded.into_inner())
}
```

`BACKGROUND` is a dark navy placeholder for Barnacle's palette; Task 9 checks it against real silhouettes before launch.

- [ ] **Step 8: Run the tests to verify they pass**

Run: `cargo test -p barnacle-data --test paper --test translations --test silhouette`
Expected: `paper` 4 passed, `translations` 4 passed, `silhouette` 3 passed.

If `reads_paper_flags_for_ships_only` fails with `UnexpectedRoot`, print the decoded root with `{:?}` in a scratch test to see how `pickled` represents the fixture, then adjust `params_dict`. Do not change the fixture.

- [ ] **Step 9: Lint and commit**

Run: `cargo fmt --all --check && cargo clippy -p barnacle-data --all-targets -- -D warnings`
Expected: clean.

```bash
git add Cargo.toml Cargo.lock crates/barnacle-data
git commit -m "feat(data): read paper flags, English names and silhouettes"
```

---

### Task 6: Catalog builder

**Files:**
- Create: `crates/barnacle-data/src/extract/params.rs`, `crates/barnacle-data/src/versions.rs`
- Modify: `crates/barnacle-data/src/extract/mod.rs`, `crates/barnacle-data/src/lib.rs`
- Test: `crates/barnacle-data/tests/versions.rs`, `crates/barnacle-data/tests/real_build.rs`, unit tests inside `params.rs`

**Interfaces:**
- Consumes: Task 5 extractors; `wowsunpack::game_params::provider::GameMetadataProvider::params_from_data`; `wowsunpack::vfs::VfsPath`.
- Produces:
  - `barnacle_data::extract::BuildInputs<'a> { vfs: &'a VfsPath, english_mo: &'a Path, provenance: Provenance, output_dir: &'a Path }`
  - `barnacle_data::extract::build_catalog(BuildInputs<'_>) -> Result<Catalog, ExtractError>`. It writes composited PNGs to `<output_dir>/silhouettes/<index>.png` and returns the catalog sorted by index; it does not write `catalog.json`.
  - `ExtractError` (variants below)
  - `barnacle_data::versions::{WOWSUNPACK, WOWS_DATA_MGR}: &str`

- [ ] **Step 1: Write the failing version test**

`crates/barnacle-data/tests/versions.rs`:

```rust
fn locked_version(lock: &toml::Table, name: &str) -> Option<String> {
    lock.get("package")?
        .as_array()?
        .iter()
        .find(|package| package.get("name").and_then(toml::Value::as_str) == Some(name))?
        .get("version")?
        .as_str()
        .map(str::to_owned)
}

#[test]
fn recorded_toolkit_versions_match_the_lockfile() {
    let text = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../Cargo.lock")).unwrap();
    let lock: toml::Table = toml::from_str(&text).unwrap();
    assert_eq!(locked_version(&lock, "wowsunpack").as_deref(), Some(barnacle_data::versions::WOWSUNPACK));
    assert_eq!(locked_version(&lock, "wows-data-mgr").as_deref(), Some(barnacle_data::versions::WOWS_DATA_MGR));
}
```

This test is what keeps a catalog's recorded toolkit versions honest after a dependency bump.

- [ ] **Step 2: Write the real-build test**

It needs a downloaded build, so it skips unless `BARNACLE_TEST_BUILD_DIR` points at one (Task 8 produces `data/store/15.8.0_13187581`). The ship facts come from 13.11 data and are stable across patches.

`crates/barnacle-data/tests/real_build.rs`:

```rust
use std::path::Path;

use barnacle_catalog::Provenance;
use barnacle_catalog::ShipClass;
use barnacle_catalog::ShipIndex;
use barnacle_catalog::curation::CurationConfig;
use barnacle_catalog::curation::Removal;
use barnacle_catalog::curation::curate;
use barnacle_data::extract::BuildInputs;
use barnacle_data::extract::build_catalog;
use wows_data_mgr::Dump;

fn index(value: &str) -> ShipIndex {
    ShipIndex::parse(value).unwrap()
}

#[test]
fn builds_and_curates_a_real_catalog() {
    let Some(dir) = std::env::var_os("BARNACLE_TEST_BUILD_DIR") else {
        eprintln!("skipping: set BARNACLE_TEST_BUILD_DIR to a downloaded build directory");
        return;
    };
    let dump = Dump::open(Path::new(&dir));
    let english_mo = dump.derived_path("translations/en/LC_MESSAGES/global.mo").unwrap();
    let output = tempfile::tempdir().unwrap();
    let vfs = dump.vfs();
    let catalog = build_catalog(BuildInputs {
        vfs: &vfs,
        english_mo: &english_mo,
        provenance: Provenance {
            game_version: "test".to_owned(),
            build: 0,
            data_repo_commit: "test".to_owned(),
            wowsunpack: "test".to_owned(),
            wows_data_mgr: "test".to_owned(),
        },
        output_dir: output.path(),
    })
    .unwrap();

    assert!(catalog.ships.len() > 1000, "only {} ships", catalog.ships.len());
    let colorado = catalog.get(&index("PASB008")).unwrap();
    assert_eq!(colorado.tier.get(), 7);
    assert_eq!(colorado.class, ShipClass::Battleship);
    assert_eq!(colorado.nation.as_str(), "USA");
    assert!(!colorado.is_paper);
    assert_eq!(colorado.name.as_ref().map(|name| name.short.as_str()), Some("Colorado"));
    assert!(colorado.silhouette.is_some());
    assert!(output.path().join("silhouettes/PASB008.png").is_file());
    assert!(catalog.get(&index("PASB110")).unwrap().is_paper);

    let config = CurationConfig::from_toml(
        r#"groups = ["start", "special", "specialUnsellable", "ultimate", "upgradeable", "upgradeableExclusive", "upgradeableUltimate", "superShip"]"#,
    )
    .unwrap();
    let curated = curate(&catalog, &config);
    assert_eq!(curated.removed[&index("PGSC899")], Removal::IdenticalSilhouette { base: index("PGSC519") });
    assert_eq!(curated.removed[&index("PASB598")], Removal::VariantSuffix { base: index("PASB518") });
    assert_eq!(curated.removed[&index("PJSC708")], Removal::CollaborationPrefix);
    for kept in ["PASB008", "PASD709", "PBSC101", "PGSB105", "PGSB503", "PBSC507", "PBSC528"] {
        assert!(curated.pool.contains(&index(kept)), "{kept} should be in the pool");
    }
    assert!(
        catalog.ships.iter().filter(|ship| ship.group.as_str().starts_with("demo")).all(|ship| !curated.pool.contains(&ship.index)),
        "a test ship reached the pool"
    );
}
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test -p barnacle-data --test versions --test real_build`
Expected: compile errors for the missing `versions` module and `build_catalog`.

- [ ] **Step 4: Implement the version constants**

`crates/barnacle-data/src/versions.rs`:

```rust
pub const WOWSUNPACK: &str = "0.45.0";
pub const WOWS_DATA_MGR: &str = "0.21.0";
```

Update `crates/barnacle-data/src/lib.rs`:

```rust
pub mod extract;
pub mod versions;
```

- [ ] **Step 5: Implement the typed ship fields, with unit tests**

`crates/barnacle-data/src/extract/params.rs`:

```rust
use barnacle_catalog::ModelError;
use barnacle_catalog::Nation;
use barnacle_catalog::ParamId;
use barnacle_catalog::ShipClass;
use barnacle_catalog::ShipGroup;
use barnacle_catalog::ShipIndex;
use barnacle_catalog::Tier;
use wowsunpack::error::GameDataError;
use wowsunpack::game_params::provider::GameMetadataProvider;
use wowsunpack::game_params::types::Species;
use wowsunpack::recognized::Recognized;

#[derive(Debug, thiserror::Error)]
pub enum ParamsError {
    #[error("GameParams could not be parsed")]
    Parse(#[from] GameDataError),
    #[error("a ship in GameParams has invalid data")]
    Model(#[from] ModelError),
}

pub struct TypedShip {
    pub id: ParamId,
    pub index: ShipIndex,
    pub tier: Tier,
    pub group: ShipGroup,
    pub class: ShipClass,
    pub nation: Nation,
}

pub fn typed_ships(game_params: Vec<u8>) -> Result<Vec<TypedShip>, ParamsError> {
    GameMetadataProvider::params_from_data(game_params)?
        .iter()
        .filter_map(|param| param.vehicle().map(|vehicle| (param, vehicle)))
        .map(|(param, vehicle)| {
            Ok(TypedShip {
                id: ParamId::new(param.id().raw()),
                index: ShipIndex::parse(param.index())?,
                tier: Tier::new(vehicle.level())?,
                group: ShipGroup::new(vehicle.group()),
                class: ship_class(param.species()),
                nation: Nation::new(param.nation()),
            })
        })
        .collect()
}

pub fn ship_class(species: Option<&Recognized<Species>>) -> ShipClass {
    match species {
        Some(Recognized::Known(Species::Destroyer)) => ShipClass::Destroyer,
        Some(Recognized::Known(Species::Cruiser)) => ShipClass::Cruiser,
        Some(Recognized::Known(Species::Battleship)) => ShipClass::Battleship,
        Some(Recognized::Known(Species::AirCarrier)) => ShipClass::AircraftCarrier,
        Some(Recognized::Known(Species::Submarine)) => ShipClass::Submarine,
        Some(Recognized::Known(other)) => ShipClass::Other(format!("{other:?}")),
        Some(Recognized::Unknown(raw)) => ShipClass::Other(raw.clone()),
        None => ShipClass::Unspecified,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_the_five_playable_classes() {
        let cases = [
            (Species::Destroyer, ShipClass::Destroyer),
            (Species::Cruiser, ShipClass::Cruiser),
            (Species::Battleship, ShipClass::Battleship),
            (Species::AirCarrier, ShipClass::AircraftCarrier),
            (Species::Submarine, ShipClass::Submarine),
        ];
        for (species, class) in cases {
            assert_eq!(ship_class(Some(&Recognized::Known(species))), class);
        }
    }

    #[test]
    fn keeps_other_and_missing_species_visible() {
        assert_eq!(ship_class(Some(&Recognized::Known(Species::Auxiliary))), ShipClass::Other("Auxiliary".to_owned()));
        assert_eq!(ship_class(Some(&Recognized::Unknown("Hovercraft".to_owned()))), ShipClass::Other("Hovercraft".to_owned()));
        assert_eq!(ship_class(None), ShipClass::Unspecified);
    }
}
```

`Species::Auxiliary` exists in wowsunpack 0.45.0 (`src/game_params/types.rs`). "Hovercraft" is a deliberately unknown value to exercise the `Unknown` arm.

- [ ] **Step 6: Implement the builder**

Replace `crates/barnacle-data/src/extract/mod.rs` with:

```rust
pub mod paper;
pub mod params;
pub mod silhouette;
pub mod translations;

use std::collections::BTreeMap;
use std::io::Read;
use std::path::Path;

use barnacle_catalog::Catalog;
use barnacle_catalog::Provenance;
use barnacle_catalog::Ship;
use barnacle_catalog::ShipIndex;
use barnacle_catalog::Silhouette;
use wowsunpack::vfs::VfsError;
use wowsunpack::vfs::VfsPath;

#[derive(Debug, thiserror::Error)]
pub enum ExtractError {
    #[error("game data could not be read")]
    Vfs(#[from] VfsError),
    #[error("a file could not be read or written")]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Paper(#[from] paper::PaperError),
    #[error(transparent)]
    Params(#[from] params::ParamsError),
    #[error(transparent)]
    Names(#[from] translations::NamesError),
    #[error("a silhouette image could not be processed")]
    Image(#[from] image::ImageError),
    #[error("ship {index} has no paper-ship flag")]
    NoPaperFlag { index: ShipIndex },
    #[error("GameParams lists {listed} ships but only {unique} distinct indices")]
    DuplicateIndex { listed: usize, unique: usize },
}

pub struct BuildInputs<'a> {
    pub vfs: &'a VfsPath,
    pub english_mo: &'a Path,
    pub provenance: Provenance,
    pub output_dir: &'a Path,
}

fn read(vfs: &VfsPath, path: &str) -> Result<Vec<u8>, ExtractError> {
    let mut bytes = Vec::new();
    vfs.join(path)?.open_file()?.read_to_end(&mut bytes)?;
    Ok(bytes)
}

fn read_if_present(vfs: &VfsPath, path: &str) -> Result<Option<Vec<u8>>, ExtractError> {
    if vfs.join(path)?.exists()? { read(vfs, path).map(Some) } else { Ok(None) }
}

pub fn build_catalog(inputs: BuildInputs<'_>) -> Result<Catalog, ExtractError> {
    let game_params = read(inputs.vfs, "content/GameParams.data")?;
    let paper = paper::paper_flags(game_params.clone())?;
    let typed = params::typed_ships(game_params)?;
    let names = translations::EnglishNames::load(inputs.english_mo)?;
    let silhouettes = inputs.output_dir.join("silhouettes");
    std::fs::create_dir_all(&silhouettes)?;

    let ships = typed
        .into_iter()
        .map(|typed| {
            let is_paper = *paper
                .get(typed.index.as_str())
                .ok_or_else(|| ExtractError::NoPaperFlag { index: typed.index.clone() })?;
            let silhouette = read_if_present(inputs.vfs, &format!("gui/ships_silhouettes/{}.png", typed.index))?
                .map(|png| -> Result<Silhouette, ExtractError> {
                    let composed = silhouette::composite(&png, silhouette::BACKGROUND)?;
                    std::fs::write(silhouettes.join(format!("{}.png", typed.index)), composed)?;
                    Ok(Silhouette { sha256: silhouette::sha256_hex(&png) })
                })
                .transpose()?;
            Ok(Ship {
                name: names.ship_name(&typed.index),
                id: typed.id,
                index: typed.index,
                tier: typed.tier,
                group: typed.group,
                class: typed.class,
                nation: typed.nation,
                is_paper,
                silhouette,
            })
        })
        .collect::<Result<Vec<Ship>, ExtractError>>()?;

    let listed = ships.len();
    let by_index: BTreeMap<ShipIndex, Ship> = ships.into_iter().map(|ship| (ship.index.clone(), ship)).collect();
    if by_index.len() != listed {
        return Err(ExtractError::DuplicateIndex { listed, unique: by_index.len() });
    }
    Ok(Catalog { provenance: inputs.provenance, ships: by_index.into_values().collect() })
}
```

GameParams is decoded twice, once by wowsunpack's typed parser and once for the paper flag. That costs build time only, never bot time, and disappears once wowsunpack exposes the flag (spec 3.2).

- [ ] **Step 7: Run the tests**

Run: `cargo test -p barnacle-data`
Expected: all pass; `real_build` prints `skipping: set BARNACLE_TEST_BUILD_DIR ...` and passes. `params` unit tests: 2 passed. `versions`: 1 passed.

- [ ] **Step 8: Lint and commit**

Run: `cargo fmt --all --check && cargo clippy -p barnacle-data --all-targets -- -D warnings`
Expected: clean.

```bash
git add crates/barnacle-data/src crates/barnacle-data/tests/versions.rs crates/barnacle-data/tests/real_build.rs
git commit -m "feat(data): build the ship catalog from a game build"
```

---

### Task 7: Data directory, download, reports and the CLI

**Files:**
- Create: `crates/barnacle-data/src/store.rs`, `crates/barnacle-data/src/report.rs`, `crates/barnacle-data/src/main.rs`
- Modify: `crates/barnacle-data/src/lib.rs`
- Test: `crates/barnacle-data/tests/store.rs`, `crates/barnacle-data/tests/report.rs`

**Interfaces:**
- Consumes: Task 6 `build_catalog`; Tasks 3-4 `curate`, `validate`, `diff`, `year_refit_candidates`; `wows_data_mgr::{Dump, download_repo}`.
- Produces:
  - `DataDir::new(impl Into<PathBuf>)`, `store()`, `catalogs()`, `catalog_dir(&str)`, `current() -> Result<Option<String>, StoreError>`, `set_current(&str)`, `built() -> Result<Vec<String>, StoreError>` (oldest build first), `newest() -> Result<String, StoreError>`, `load(&str) -> Result<Catalog, StoreError>`, `save(&str, &Catalog)`
  - `async fn download(&DataDir, Option<u32>) -> Result<Downloaded, StoreError>`; `Downloaded { entry: BuildEntry, data_repo_commit: String }`
  - `fn build(&DataDir, &Downloaded) -> Result<Catalog, StoreError>`
  - `report::diff_report(Option<&Catalog>, &Catalog, &CurationConfig) -> String`
  - `report::problems_report(&str, &[Problem]) -> String`
  - The `barnacle-data` binary with `sync`, `validate`, `diff` and `use`

Catalog directories are named exactly like the store's build directories (`<version>_<build>`, for example `15.8.0_13187581`). `data/catalog/current` is a one-line text file naming the catalog the bot should load; a text file works the same on every platform, unlike a symlink.

- [ ] **Step 1: Write the failing store tests**

`crates/barnacle-data/tests/store.rs`:

```rust
use barnacle_catalog::Catalog;
use barnacle_catalog::Provenance;
use barnacle_data::store::DataDir;
use barnacle_data::store::StoreError;

fn empty_catalog(build: u32) -> Catalog {
    Catalog {
        provenance: Provenance {
            game_version: "15.8.0".to_owned(),
            build,
            data_repo_commit: "0000000".to_owned(),
            wowsunpack: "0.45.0".to_owned(),
            wows_data_mgr: "0.21.0".to_owned(),
        },
        ships: Vec::new(),
    }
}

fn data_dir_with(names: &[&str]) -> (tempfile::TempDir, DataDir) {
    let root = tempfile::tempdir().unwrap();
    let data = DataDir::new(root.path());
    for name in names {
        std::fs::create_dir_all(data.catalog_dir(name)).unwrap();
        data.save(name, &empty_catalog(1)).unwrap();
    }
    (root, data)
}

#[test]
fn current_is_absent_until_set() {
    let (_root, data) = data_dir_with(&["15.8.0_13187581"]);
    assert_eq!(data.current().unwrap(), None);
    data.set_current("15.8.0_13187581").unwrap();
    assert_eq!(data.current().unwrap().as_deref(), Some("15.8.0_13187581"));
}

#[test]
fn built_catalogs_are_ordered_by_build_number_not_by_name() {
    let (_root, data) = data_dir_with(&["15.8.0_13187581", "9.9.1_2979658", "15.7.0_13015811"]);
    std::fs::create_dir_all(data.catalog_dir("15.9.0_13300000")).unwrap();
    assert_eq!(data.built().unwrap(), ["9.9.1_2979658", "15.7.0_13015811", "15.8.0_13187581"]);
    assert_eq!(data.newest().unwrap(), "15.8.0_13187581");
}

#[test]
fn an_empty_data_directory_has_no_catalogs() {
    let (_root, data) = data_dir_with(&[]);
    assert_eq!(data.built().unwrap(), Vec::<String>::new());
    assert!(matches!(data.newest(), Err(StoreError::NoCatalogs)));
}

#[test]
fn saved_catalogs_load_back() {
    let (_root, data) = data_dir_with(&["15.8.0_13187581"]);
    assert_eq!(data.load("15.8.0_13187581").unwrap(), empty_catalog(1));
    assert!(matches!(data.load("15.7.0_13015811"), Err(StoreError::Io { .. })));
}
```

`15.9.0_13300000` is a deliberately incomplete directory (no `catalog.json`), standing in for a build whose catalog step crashed.

- [ ] **Step 2: Write the failing report tests**

`crates/barnacle-data/tests/report.rs`:

```rust
use barnacle_catalog::Catalog;
use barnacle_catalog::Nation;
use barnacle_catalog::ParamId;
use barnacle_catalog::Provenance;
use barnacle_catalog::Ship;
use barnacle_catalog::ShipClass;
use barnacle_catalog::ShipGroup;
use barnacle_catalog::ShipIndex;
use barnacle_catalog::ShipName;
use barnacle_catalog::Silhouette;
use barnacle_catalog::Tier;
use barnacle_catalog::curation::CurationConfig;
use barnacle_catalog::curation::Problem;
use barnacle_data::report::diff_report;
use barnacle_data::report::problems_report;

fn ship(index: &str, name: &str, group: &str, tier: u32, hash: &str) -> Ship {
    Ship {
        id: ParamId::new(1),
        index: ShipIndex::parse(index).unwrap(),
        tier: Tier::new(tier).unwrap(),
        group: ShipGroup::new(group),
        class: ShipClass::Cruiser,
        nation: Nation::new("Germany"),
        is_paper: false,
        name: Some(ShipName { short: name.to_owned(), full: Some(name.to_owned()) }),
        silhouette: Some(Silhouette { sha256: hash.to_owned() }),
    }
}

fn catalog(version: &str, build: u32, ships: Vec<Ship>) -> Catalog {
    Catalog {
        provenance: Provenance {
            game_version: version.to_owned(),
            build,
            data_repo_commit: "0000000".to_owned(),
            wowsunpack: "0.45.0".to_owned(),
            wows_data_mgr: "0.21.0".to_owned(),
        },
        ships,
    }
}

#[test]
fn diff_report_shows_each_new_ship_and_what_curation_did() {
    let old = catalog("15.7.0", 13015811, vec![ship("PGSC519", "Ägir", "special", 9, "5fe2")]);
    let new = catalog(
        "15.8.0",
        13187581,
        vec![ship("PGSC519", "Ägir", "special", 9, "5fe2"), ship("PGSC899", "AL Ägir", "special", 9, "5fe2")],
    );
    let config = CurationConfig::from_toml("reviewed_through = 13015811\ngroups = [\"special\"]").unwrap();
    let report = diff_report(Some(&old), &new, &config);
    assert!(report.contains("Catalog 15.8.0 (build 13187581), compared with 15.7.0 (build 13015811)"), "{report}");
    assert!(report.contains("Added (1)"), "{report}");
    assert!(report.contains("PGSC899  AL Ägir  tier 9  special  -> removed: same silhouette as PGSC519"), "{report}");
    assert!(report.contains("curation was reviewed through build 13015811, but the catalog is build 13187581"), "{report}");
}

#[test]
fn a_first_diff_says_there_is_nothing_to_compare_with() {
    let new = catalog("15.8.0", 13187581, vec![ship("PGSC519", "Ägir", "special", 9, "5fe2")]);
    let config = CurationConfig::from_toml("reviewed_through = 13187581\ngroups = [\"special\"]").unwrap();
    let report = diff_report(None, &new, &config);
    assert!(report.contains("compared with nothing (first catalog)"), "{report}");
    assert!(report.contains("PGSC519  Ägir  tier 9  special  -> in pool"), "{report}");
    assert!(report.contains("New ship groups: special"), "{report}");
    assert!(report.contains("Curation problems: none"), "{report}");
}

#[test]
fn problems_report_lists_each_problem() {
    assert_eq!(problems_report("15.8.0_13187581", &[]), "15.8.0_13187581: no curation problems");
    assert_eq!(
        problems_report("15.8.0_13187581", &[Problem::NotReviewed { build: 13187581 }]),
        "15.8.0_13187581: 1 curation problem(s)\n  - curation has never been reviewed; review build 13187581 and set reviewed_through"
    );
}
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test -p barnacle-data --test store --test report`
Expected: compile errors for the missing `store` and `report` modules.

- [ ] **Step 4: Implement the store**

`crates/barnacle-data/src/store.rs`:

```rust
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
        let entries = listing.collect::<Result<Vec<_>, _>>().map_err(io_error(root))?;
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
        let json = catalog.to_json().map_err(|source| StoreError::CatalogJson { path: path.clone(), source })?;
        std::fs::write(&path, json).map_err(io_error(path))
    }
}

pub struct Downloaded {
    pub entry: BuildEntry,
    pub data_repo_commit: String,
}

pub async fn download(data: &DataDir, requested: Option<u32>) -> Result<Downloaded, StoreError> {
    let client =
        reqwest::Client::builder().user_agent(concat!("barnacle-data/", env!("CARGO_PKG_VERSION"))).build()?;
    let base = download_repo::DEFAULT_REPO_BASE_URL;
    let remote = |report: rootcause::Report| StoreError::Remote(report.into());
    let index = download_repo::fetch_builds_index(&client, base).await.map_err(remote)?;
    let target = match requested {
        Some(build) => build,
        None => index.builds.iter().map(|entry| entry.build).max().ok_or(StoreError::NoPublishedBuilds)?,
    };
    let data_repo_commit = download_repo::fetch_repo_tip(&client).await.map_err(remote)?;
    let store = data.store();
    std::fs::create_dir_all(&store).map_err(io_error(store.clone()))?;
    let progress = |done: u64, total: u64| {
        if total > 0 && (done == total || done % 250 == 0) {
            eprintln!("downloaded {done} of {total} objects");
        }
    };
    let build = download_repo::download_build(&client, base, &store, target, None, false, &progress)
        .await
        .map_err(remote)?;
    let entry = index.find_by_build(build).cloned().ok_or(StoreError::BuildNotInIndex { build })?;
    Ok(Downloaded { entry, data_repo_commit })
}

pub fn build(data: &DataDir, downloaded: &Downloaded) -> Result<Catalog, StoreError> {
    let entry = &downloaded.entry;
    let dump = Dump::open(&data.store().join(&entry.dir));
    let english_mo =
        dump.derived_path(ENGLISH_CATALOG).ok_or_else(|| StoreError::NoEnglishCatalog { dir: entry.dir.clone() })?;
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
```

`wows-data-mgr` returns `rootcause::Report` without re-exporting it, which is why this crate depends on `rootcause` directly. `report.into()` uses rootcause's `From<Report> for Box<dyn Error + Send + Sync>`, so the error chain survives without parsing any text.

`build` saves `catalog.json` only after every silhouette is written, so a crashed build never shows up in `built()`.

- [ ] **Step 5: Implement the reports**

`crates/barnacle-data/src/report.rs`:

```rust
use barnacle_catalog::Catalog;
use barnacle_catalog::ShipIndex;
use barnacle_catalog::curation::Curated;
use barnacle_catalog::curation::CurationConfig;
use barnacle_catalog::curation::Problem;
use barnacle_catalog::curation::curate;
use barnacle_catalog::curation::validate;
use barnacle_catalog::diff::diff;
use barnacle_catalog::diff::year_refit_candidates;

fn label(catalog: &Catalog) -> String {
    format!("{} (build {})", catalog.provenance.game_version, catalog.provenance.build)
}

fn ship_line(catalog: &Catalog, curated: &Curated, index: &ShipIndex) -> String {
    let Some(ship) = catalog.get(index) else {
        return format!("  {index}");
    };
    let name = ship.name.as_ref().map(|name| name.display()).unwrap_or("(no English name)");
    let outcome = match curated.removed.get(index) {
        Some(removal) => format!("removed: {removal}"),
        None => "in pool".to_owned(),
    };
    format!("  {index}  {name}  tier {}  {}  -> {outcome}", ship.tier.get(), ship.group)
}

fn name_of(catalog: &Catalog, index: &ShipIndex) -> String {
    catalog
        .get(index)
        .and_then(|ship| ship.name.as_ref())
        .map(|name| format!("{} ({index})", name.display()))
        .unwrap_or_else(|| index.to_string())
}

pub fn diff_report(old: Option<&Catalog>, new: &Catalog, config: &CurationConfig) -> String {
    let changes = diff(old, new);
    let curated = curate(new, config);
    let problems = validate(new, config, &curated);
    let candidates = year_refit_candidates(new, config, &curated);
    let compared = old.map(label).unwrap_or_else(|| "nothing (first catalog)".to_owned());
    let groups = changes.new_groups.iter().map(ToString::to_string).collect::<Vec<_>>().join(", ");

    let header = vec![
        format!("Catalog {}, compared with {compared}", label(new)),
        format!("Pool: {} ships", curated.pool.len()),
        format!("New ship groups: {}", if groups.is_empty() { "none".to_owned() } else { groups }),
    ];
    let regrouped = std::iter::once(format!("Regrouped ({})", changes.regrouped.len())).chain(
        changes.regrouped.iter().map(|change| format!("  {}  {} -> {}", change.index, change.from, change.to)),
    );
    let added = std::iter::once(format!("Added ({})", changes.added.len()))
        .chain(changes.added.iter().map(|index| ship_line(new, &curated, index)));
    let removed = std::iter::once(format!("Removed ({})", changes.removed.len()))
        .chain(changes.removed.iter().map(|index| format!("  {index}")));
    let lookalikes = std::iter::once(format!("Lookalike candidates ({})", candidates.len())).chain(
        candidates
            .iter()
            .map(|pair| format!("  {} and {}", name_of(new, &pair.original), name_of(new, &pair.refit))),
    );
    let problem_lines: Vec<String> = if problems.is_empty() {
        vec!["Curation problems: none".to_owned()]
    } else {
        std::iter::once(format!("Curation problems ({})", problems.len()))
            .chain(problems.iter().map(|problem| format!("  - {problem}")))
            .collect()
    };

    header
        .into_iter()
        .chain(regrouped)
        .chain(added)
        .chain(removed)
        .chain(lookalikes)
        .chain(problem_lines)
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn problems_report(name: &str, problems: &[Problem]) -> String {
    if problems.is_empty() {
        return format!("{name}: no curation problems");
    }
    std::iter::once(format!("{name}: {} curation problem(s)", problems.len()))
        .chain(problems.iter().map(|problem| format!("  - {problem}")))
        .collect::<Vec<_>>()
        .join("\n")
}
```

The two `unwrap_or` calls are display fallbacks, not data defaults: a ship without an English name is shown as such, and a missing name falls back to its index.

- [ ] **Step 6: Implement the CLI**

Update `crates/barnacle-data/src/lib.rs`:

```rust
pub mod extract;
pub mod report;
pub mod store;
pub mod versions;
```

`crates/barnacle-data/src/main.rs`:

```rust
use std::path::Path;
use std::path::PathBuf;
use std::process::ExitCode;

use barnacle_catalog::curation::CurationConfig;
use barnacle_catalog::curation::Problem;
use barnacle_catalog::curation::curate;
use barnacle_catalog::curation::validate;
use barnacle_data::report::diff_report;
use barnacle_data::report::problems_report;
use barnacle_data::store::DataDir;
use barnacle_data::store::StoreError;
use barnacle_data::store::build;
use barnacle_data::store::download;
use clap::Parser;
use clap::Subcommand;

#[derive(Parser)]
#[command(name = "barnacle-data", about = "Download World of Warships game data and build Barnacle's ship catalog")]
struct Cli {
    #[arg(long, default_value = "data")]
    data_dir: PathBuf,
    #[arg(long, default_value = "curation/ships.toml")]
    curation: PathBuf,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    #[command(about = "Download the newest published build (or --build N) and build its catalog")]
    Sync {
        #[arg(long)]
        build: Option<u32>,
    },
    #[command(about = "Check curation against a catalog (default: the newest built)")]
    Validate { catalog: Option<String> },
    #[command(about = "Compare two catalogs (default: current against newest) and show curation results")]
    Diff {
        #[arg(long)]
        from: Option<String>,
        #[arg(long)]
        to: Option<String>,
    },
    #[command(about = "Make a validated catalog the one the bot loads on its next start")]
    Use { catalog: String },
}

#[derive(Debug, thiserror::Error)]
enum AppError {
    #[error(transparent)]
    Store(#[from] StoreError),
    #[error("curation file {path} could not be read")]
    CurationIo {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("curation file {path} is not valid")]
    CurationToml {
        path: PathBuf,
        #[source]
        source: toml::de::Error,
    },
    #[error("the async runtime could not start")]
    Runtime(#[source] std::io::Error),
    #[error("catalog {catalog} has curation problems; run `barnacle-data validate {catalog}`")]
    Invalid { catalog: String },
}

fn load_curation(path: &Path) -> Result<CurationConfig, AppError> {
    let text = std::fs::read_to_string(path)
        .map_err(|source| AppError::CurationIo { path: path.to_owned(), source })?;
    CurationConfig::from_toml(&text).map_err(|source| AppError::CurationToml { path: path.to_owned(), source })
}

fn problems_for(data: &DataDir, curation: &Path, name: &str) -> Result<Vec<Problem>, AppError> {
    let config = load_curation(curation)?;
    let catalog = data.load(name)?;
    let curated = curate(&catalog, &config);
    Ok(validate(&catalog, &config, &curated))
}

fn run(cli: &Cli) -> Result<ExitCode, AppError> {
    let data = DataDir::new(&cli.data_dir);
    match &cli.command {
        Command::Sync { build: requested } => {
            let runtime = tokio::runtime::Builder::new_multi_thread().enable_all().build().map_err(AppError::Runtime)?;
            let downloaded = runtime.block_on(download(&data, *requested))?;
            let catalog = build(&data, &downloaded)?;
            println!(
                "built catalog {} with {} ships; next: barnacle-data diff",
                downloaded.entry.dir,
                catalog.ships.len()
            );
            Ok(ExitCode::SUCCESS)
        }
        Command::Validate { catalog } => {
            let name = match catalog {
                Some(name) => name.clone(),
                None => data.newest()?,
            };
            let problems = problems_for(&data, &cli.curation, &name)?;
            println!("{}", problems_report(&name, &problems));
            Ok(if problems.is_empty() { ExitCode::SUCCESS } else { ExitCode::FAILURE })
        }
        Command::Diff { from, to } => {
            let config = load_curation(&cli.curation)?;
            let to = match to {
                Some(name) => name.clone(),
                None => data.newest()?,
            };
            let from = match from {
                Some(name) => Some(name.clone()),
                None => data.current()?,
            };
            let old = from.as_deref().map(|name| data.load(name)).transpose()?;
            let new = data.load(&to)?;
            println!("{}", diff_report(old.as_ref(), &new, &config));
            Ok(ExitCode::SUCCESS)
        }
        Command::Use { catalog } => {
            if !problems_for(&data, &cli.curation, catalog)?.is_empty() {
                return Err(AppError::Invalid { catalog: catalog.clone() });
            }
            data.set_current(catalog)?;
            println!("the bot will load {catalog} on its next start");
            Ok(ExitCode::SUCCESS)
        }
    }
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(&cli) {
        Ok(code) => code,
        Err(error) => {
            eprintln!("error: {error}");
            std::iter::successors(std::error::Error::source(&error), |cause| cause.source())
                .for_each(|cause| eprintln!("  caused by: {cause}"));
            ExitCode::FAILURE
        }
    }
}
```

- [ ] **Step 7: Run the tests and the CLI help**

Run: `cargo test -p barnacle-data --test store --test report`
Expected: `store` 4 passed, `report` 3 passed.

Run: `cargo run -p barnacle-data -- --help`
Expected: usage text listing `sync`, `validate`, `diff` and `use`.

Run: `cargo run -p barnacle-data -- validate`
Expected: `error: no catalog has been built yet; run `barnacle-data sync`` and exit code 1.

- [ ] **Step 8: Lint and commit**

Run: `cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings`
Expected: clean.

```bash
git add Cargo.toml Cargo.lock crates/barnacle-data/src crates/barnacle-data/tests/store.rs crates/barnacle-data/tests/report.rs
git commit -m "feat(data): add sync, validate, diff and use commands"
```

---

### Task 8: Repository files, CI and the first real catalog

**Files:**
- Create: `LICENSE`, `README.md`, `deny.toml`, `.github/workflows/ci.yml`, `.github/dependabot.yml`, `curation/ships.toml`
- Modify: `crates/barnacle-data/src/extract/silhouette.rs` (only if the background colour is changed in Step 7)

**Interfaces:**
- Consumes: the `barnacle-data` binary from Task 7.
- Produces: a green CI run, a real `data/catalog/15.8.0_13187581/` on the owner's machine, and the first `diff` report for the curation review in spec section 9, step 4.

- [ ] **Step 1: Add the license (ask the owner first; this downloads the license text)**

Run: `curl -fsSL https://www.apache.org/licenses/LICENSE-2.0.txt -o LICENSE && head -3 LICENSE`
Expected: the first lines read `Apache License` and `Version 2.0, January 2004`.

- [ ] **Step 2: Add the README**

`README.md`:

```markdown
# Barnacle

A Discord bot for World of Warships players. Its first feature is a ship silhouette guessing game.

Ship data and silhouettes come from [wows-toolkit](https://github.com/landaire/wows-toolkit) and its published game data.

## Updating game data

    cargo run -p barnacle-data -- sync
    cargo run -p barnacle-data -- diff
    # review new ships, edit curation/ships.toml, raise reviewed_through
    cargo run -p barnacle-data -- use <version>_<build>

## License

Apache-2.0. See `LICENSE`.

## Wargaming notice
```

Then append, under "Wargaming notice", the notice text that Wargaming's Player Content Policy recommends, copied exactly from https://legal.wargaming.net/en/user-documents/content-policies/player-content-policy/view (section 2.5). Copy it from the page rather than retyping it, so the wording matches.

- [ ] **Step 3: Add the license allowlist and check it**

`deny.toml`:

```toml
[licenses]
allow = [
    "Apache-2.0",
    "Apache-2.0 WITH LLVM-exception",
    "BSD-2-Clause",
    "BSD-3-Clause",
    "CDLA-Permissive-2.0",
    "ISC",
    "MIT",
    "Unicode-3.0",
    "Zlib",
]
confidence-threshold = 0.9
```

Run: `cargo install cargo-deny --locked && cargo deny check licenses` (ask the owner before installing)
Expected: `licenses ok`. If cargo-deny rejects the file format, run `cargo deny init` in a scratch directory, copy its `[licenses]` layout, and keep the same allowlist. If a dependency's license is not on the list, stop and report it to the owner.

- [ ] **Step 4: Add CI and dependency updates**

`.github/workflows/ci.yml`:

```yaml
name: ci

on:
  push:
    branches: [main]
  pull_request:

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - run: rustup show
      - uses: Swatinem/rust-cache@v2
      - run: cargo fmt --all --check
      - run: cargo clippy --workspace --all-targets -- -D warnings
      - run: cargo test --workspace

  licenses:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: EmbarkStudios/cargo-deny-action@v2
        with:
          command: check licenses
```

`.github/dependabot.yml`:

```yaml
version: 2
updates:
  - package-ecosystem: cargo
    directory: /
    schedule:
      interval: weekly
  - package-ecosystem: github-actions
    directory: /
    schedule:
      interval: weekly
```

Before committing, confirm the current major versions of `actions/checkout`, `Swatinem/rust-cache` and `EmbarkStudios/cargo-deny-action`, and whether Dependabot bumps exact (`=`) Cargo requirements. Update the files if either differs.

- [ ] **Step 5: Seed the curation file**

`curation/ships.toml`:

```toml
groups = [
    "start",
    "special",
    "specialUnsellable",
    "ultimate",
    "upgradeable",
    "upgradeableExclusive",
    "upgradeableUltimate",
    "superShip",
]
```

`reviewed_through` is deliberately absent, so `validate` reports `NotReviewed` until the first review is done and `use` refuses to publish an unreviewed catalog.

- [ ] **Step 6: Build the first real catalog (ask the owner first; this downloads about 310 MB)**

Run: `cargo run --release -p barnacle-data -- sync`
Expected: `downloaded N of N objects` progress lines, then `built catalog 15.8.0_13187581 with <more than 1000> ships; next: barnacle-data diff`. The download is cached under `data/store/`; a later build only fetches the files that changed.

Run: `BARNACLE_TEST_BUILD_DIR=data/store/15.8.0_13187581 cargo test --release -p barnacle-data --test real_build -- --nocapture`
Expected: `1 passed` with no "skipping" line. If an assertion about a specific ship fails, report the actual value to the owner before changing anything: it may be a real change in 15.8.0.

- [ ] **Step 7: Check the silhouettes by eye**

Open five composited files, for example `data/catalog/15.8.0_13187581/silhouettes/PASB008.png`, `PJSB018.png`, `PASC020.png`, `PGSC519.png` and `PBSC210.png`, and show them to the owner. If the silhouettes are hard to see against the navy background, change `BACKGROUND` in `crates/barnacle-data/src/extract/silhouette.rs` to a colour the owner picks and rerun Step 6's `sync`.

- [ ] **Step 8: Produce the first curation report**

Run: `cargo run --release -p barnacle-data -- diff > data/first-diff.txt; tail -20 data/first-diff.txt`
Expected: `compared with nothing (first catalog)`, the pool size, every group in the build under "New ship groups", and `curation has never been reviewed` under problems.

This report is the input to spec section 9, step 4: the owner reviews which ships the automatic rules left in the pool, adds `exclude`, `keep`, `lookalikes` and `aliases` entries, sets `reviewed_through = 13187581`, and runs `cargo run -p barnacle-data -- use 15.8.0_13187581`. That review is the owner's work, not part of this task.

- [ ] **Step 9: Full check and commit**

Run: `cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace`
Expected: clean; every suite passes.

```bash
git add LICENSE README.md deny.toml .github curation/ships.toml crates/barnacle-data/src/extract/silhouette.rs
git commit -m "chore: add license, CI, dependency updates and curation seed"
```

---

## Self-review against the spec

| Spec requirement | Task |
|---|---|
| 2.3 workspace layout, toolkit isolated from the bot | 1, 5 (catalog crate has no toolkit dependency) |
| 2.4 data flow: download, typed params, paper flag, English names, silhouettes | 5, 6, 7 |
| 3.1 catalog record and provenance | 1, 6, 7 |
| 3.2 paper-ship read from the raw tree | 5 |
| 3.3 compositing on Barnacle's own background | 5, 8 (colour check) |
| 4.1 curation file | 3, 8 |
| 4.2 automatic rules 1-6, with the false-pair examples | 3, 6 (real data) |
| 4.3 validation, including the D1 and D2 cases | 4 |
| 4.4 year-suffixed look-alike candidates | 4, 7 (in the diff report) |
| 7.1 update procedure: sync, diff, review, use | 7, 8 |
| 7.2 dependency bot and pinned versions | 1, 6 (lockfile test), 8 |
| 11.3 license allowlist in CI | 8 |
| 11.4 non-affiliation notice, no committed silhouettes | 8, 1 (`data/` ignored) |

Not covered here, by design: near-identical and model-path look-alike candidates (spec 4.4 items 2-3, not needed for launch), the catalog equivalence check (spec 7.2 step 3, which needs a cached game build in CI and belongs with Plan 3), the upstream `is_paper_ship` pull request (spec 9, step 8), and everything in spec sections 5 and 6 (Plans 2 and 3).

Deviations from the first spec draft, already written back into the spec: `ShipGroup` and `Nation` are string newtypes rather than enums, since the allowlist is configured as strings and the diff reports unseen groups; ship names and silhouettes are `Option` structs; a ship with no English name is removed; `reviewed_through` is optional; the cleaning rule drops the middle dot, which no English name uses; and extraction lives in `barnacle-data`, so `barnacle-catalog` stays free of the toolkit.
