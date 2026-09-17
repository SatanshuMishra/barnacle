# Barnacle Discord Bot Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build `barnacle-bot`, the program that runs `/guess`, `/ship info`, `/profile` and `/about` on Discord, using the game rules in `barnacle-guess` and storing wins in a local SQLite file.

**Architecture:**
- **Game core.** A Discord-free module, `table`, runs rounds: one lock per channel, the 20 s and 10 s timers, judging, cancelling and recording wins. It talks to Discord only through a small `Announcer` interface, and to storage only through a `SolveStore` interface, so every rule can be tested with fakes on tokio's paused clock.
- **Discord layer.** A thin poise layer turns commands, chat messages and button clicks into calls on that core.
- **Startup.** Before the bot connects, it checks the config, the token, the catalog, the curation file, the silhouettes and the database.
- **Moved code.** The read-only catalog folder code moves from `barnacle-data` into `barnacle-catalog`, so the bot never compiles the game-data toolkit.

**Tech Stack:** Rust 1.97 (edition 2024); `poise` 0.7 on `serenity` 0.12.5; `sqlx` 0.9 with bundled SQLite; `tokio` 1; `tracing` 0.1 and `tracing-subscriber` 0.3; `clap` 4; `rand` 0.10; `thiserror` 2; `toml` 1.

**Spec:** `docs/superpowers/specs/2026-09-16-barnacle-bot-design.md` (the bot design, approved as decision `01M2PJCA9J95PSZ2VKS0YPM77F`), which extends section 6 of `docs/specs/2026-09-16-silhouette-game-spec.md`. The game rules this plan wires up come from `docs/superpowers/plans/2026-09-16-barnacle-game-rules.md`.

## Global Constraints

**Toolchain and crates**
- Rust `1.97` comes from the existing `rust-toolchain.toml`, with edition `2024`.
- `barnacle-bot` never depends on `barnacle-data`, `wowsunpack`, `wows-data-mgr` or `pickled` (design 4.1).
- Dependency versions, all in `[workspace.dependencies]`:
  - `poise = "0.7"`. poise 0.7.0 requires serenity `^0.12.5`.
  - `sqlx = { version = "0.9", default-features = false, features = ["runtime-tokio", "sqlite"] }`.
  - `tracing = "0.1"` and `tracing-subscriber = "0.3"`. `tokio`, `clap`, `rand`, `serde`, `thiserror` and `toml` are already there.

**API facts checked against the 0.7.0 and 0.9 sources on 2026-09-16**
- **Context:** a poise command context has `ctx.data()`. The event handler reads the same data from `FrameworkContext::user_data`, a field.
- **Autocomplete:** functions return `serenity::CreateAutocompleteResponse`.
- **Command descriptions:** poise's `command` macro reads a description only from a doc comment, which this project never writes. Each command is therefore built as a `poise::Command` value, and its `description` (and `subcommands`, `subcommand_required`) is set with struct update syntax.
- **SQL strings:** sqlx 0.9 accepts only literal SQL strings (its `SqlSafeStr` check). A `String` built at runtime does not compile.

**Database rules**
- Never connect to a live database.
- Tests use in-memory SQLite, or a file inside a temporary folder, filled with made-up data.
- A person applies `migrations/0001_guess_solves.sql`. The bot opens the file with `create_if_missing(false)` and never creates or alters tables.

**Secrets**
- The bot token comes only from the `DISCORD_TOKEN` environment variable.
- It is never written to a file, a log, a test or a commit.

**Wording**
- Every user-facing sentence lives in `text` and is Barnacle's own wording.
- Nothing is copied from padtrack/track: no code, no UI text, no `guess.toml` entries.
- The silhouette is always attached as `silhouette.png`, so the ship's index never reaches players.

**Code style**
- No code comments, docstrings or section-header comments. Tooling pragmas such as `#![allow(dead_code)]` are allowed.
- No emojis anywhere.
- Domain values are newtypes (`GuildId`, `ChannelId`, `Place`, plus those from `barnacle-catalog` and `barnacle-guess`). "Absent" is `Option`.
- Errors are `thiserror` enums with structured fields. Never parse an error's text.
- Build new values instead of mutating. Round state lives behind a lock and is replaced or taken there.

**Tests**
- Tests never use the network, a Discord account or a live database.
- Tests on tokio's paused clock (`start_paused = true`) never touch SQLite. SQLite works on a background thread, so the paused clock would jump to the next timer while a query runs. The round tests use `FakeStore` instead.

**Licenses**
- Add `CDLA-Permissive-2.0` to `deny.toml` (decision `01M2PHB4C2D8TG78RRB3TJ9MHG`).
- If any other new license appears, stop and ask the owner.

**Git**
- Work on the existing branch `feat/discord-bot`, which already holds the design commit.
- Make one commit per task, using Conventional Commits and `git commit -m "..." -- <paths>`.
- Pushing and opening a pull request wait for the owner.

## How to read the file steps

- **Whole file** `path`: write the block as the complete file.
- **Replace in** `path`: find the first block in the file exactly once, and replace it with the second.
- **Append to** `path`: add the block at the end of the file, after one blank line.

Blocks that contain triple backticks are fenced with four.

## File Structure

```
Cargo.toml                                   workspace: barnacle-guess, poise, sqlx, tracing, tracing-subscriber
deny.toml                                    allow CDLA-Permissive-2.0
.gitignore                                   ignore barnacle.toml
barnacle.example.toml                        config template (refused until servers are listed)
README.md                                    running the bot, player data
migrations/0001_guess_solves.sql             solves table, applied by a person
migrations/0001_guess_solves.down.sql        rollback (deletes every stored solve)
crates/barnacle-catalog/src/store.rs         CatalogRoot: current catalog, loading, silhouette paths
crates/barnacle-catalog/tests/store.rs
crates/barnacle-data/src/store.rs            DataDir delegates catalog reading to CatalogRoot
crates/barnacle-data/src/extract/mod.rs      uses SILHOUETTES_DIR
crates/barnacle-guess/src/ids.rs             Snowflake::unix_millis
crates/barnacle-guess/src/book.rs            ShipBook::names, ShipBook::lookalikes
crates/barnacle-bot/
  Cargo.toml
  src/lib.rs                                 module list
  src/main.rs                                command line, logging, startup, run
  src/ids.rs                                 GuildId, ChannelId, Place
  src/solves.rs                              SQLite store, SolveStore trait, schema check
  src/config.rs                              barnacle.toml
  src/text.rs                                all wording and labels
  src/lookup.rs                              /ship info autocomplete and name resolution
  src/info.rs                                /ship info card
  src/wiring.rs                              command options, Cancel button IDs
  src/table.rs                               the game core
  src/startup.rs                             startup checks and error report
  src/discord.rs                             Data, RunError, framework, command registration
  src/discord/announcer.rs                   serenity implementation of Announcer
  src/discord/commands.rs                    /guess, /ship info, /profile, /about
  src/discord/events.rs                      chat messages, Cancel clicks, command errors
  tests/common/mod.rs                        catalog and SQLite helpers
  tests/common/fakes.rs                      FakeDiscord, FakeStore, table_with
  tests/solves.rs tests/config.rs tests/text.rs tests/lookup.rs tests/info.rs
  tests/wiring.rs tests/table.rs tests/startup.rs
```

---

### Task 1: Move catalog folder reading into barnacle-catalog

**Files:**
- Create: `crates/barnacle-catalog/src/store.rs`
- Modify: `crates/barnacle-catalog/src/lib.rs`, `crates/barnacle-catalog/Cargo.toml`
- Modify: `crates/barnacle-data/src/store.rs`, `crates/barnacle-data/src/extract/mod.rs`
- Test: `crates/barnacle-catalog/tests/store.rs`

**Interfaces:**
- Consumes: `Catalog::from_json` and `ShipIndex` (existing).
- Produces:
  - Constants: `barnacle_catalog::store::{CATALOG_FILE, CURRENT_FILE, SILHOUETTES_DIR}`.
  - `CatalogDirError::{Io { path, source }, Json { path, source }}`.
  - `CatalogRoot::new(path)`, with `path()`, `dir(name) -> PathBuf`, `silhouette(name, &ShipIndex) -> PathBuf`, `current() -> Result<Option<String>, CatalogDirError>` and `load(name) -> Result<Catalog, CatalogDirError>`.
  - `DataDir::catalog_root() -> CatalogRoot`. `DataDir`'s `current`, `load` and `catalog_dir` keep their signatures and errors.

- [ ] **Step 1: Write the failing tests**

Append to `crates/barnacle-catalog/Cargo.toml`:

```toml
[dev-dependencies]
tempfile.workspace = true
```

Whole file `crates/barnacle-catalog/tests/store.rs`:

```rust
mod common;

use barnacle_catalog::store::CatalogDirError;
use barnacle_catalog::store::CatalogRoot;
use common::catalog;
use common::index;

fn root() -> (tempfile::TempDir, CatalogRoot) {
    let dir = tempfile::tempdir().unwrap();
    let root = CatalogRoot::new(dir.path());
    (dir, root)
}

#[test]
fn no_current_file_means_no_current_catalog() {
    let (_dir, root) = root();
    assert_eq!(root.current().unwrap(), None);
}

#[test]
fn the_current_file_names_the_catalog_without_surrounding_whitespace() {
    let (_dir, root) = root();
    std::fs::write(root.path().join("current"), "15.8.0_13187581_r4\n").unwrap();
    assert_eq!(
        root.current().unwrap().as_deref(),
        Some("15.8.0_13187581_r4")
    );
}

#[test]
fn a_saved_catalog_loads_back() {
    let (_dir, root) = root();
    let saved = catalog(13187581, Vec::new());
    std::fs::create_dir_all(root.dir("15.8.0_13187581_r4")).unwrap();
    std::fs::write(
        root.dir("15.8.0_13187581_r4").join("catalog.json"),
        saved.to_json().unwrap(),
    )
    .unwrap();
    assert_eq!(root.load("15.8.0_13187581_r4").unwrap(), saved);
}

#[test]
fn a_missing_catalog_is_an_io_error_and_a_broken_one_a_json_error() {
    let (_dir, root) = root();
    assert!(matches!(
        root.load("15.8.0_13187581_r4"),
        Err(CatalogDirError::Io { .. })
    ));
    std::fs::create_dir_all(root.dir("15.8.0_13187581_r5")).unwrap();
    std::fs::write(root.dir("15.8.0_13187581_r5").join("catalog.json"), "{").unwrap();
    assert!(matches!(
        root.load("15.8.0_13187581_r5"),
        Err(CatalogDirError::Json { .. })
    ));
}

#[test]
fn a_silhouette_lives_in_the_catalogs_silhouettes_folder() {
    let (_dir, root) = root();
    assert_eq!(
        root.silhouette("15.8.0_13187581_r4", &index("PASB008")),
        root.path()
            .join("15.8.0_13187581_r4")
            .join("silhouettes")
            .join("PASB008.png")
    );
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p barnacle-catalog --test store`
Expected: FAIL to compile with `error[E0432]: unresolved import `barnacle_catalog::store``.

- [ ] **Step 3: Add the module**

Whole file `crates/barnacle-catalog/src/store.rs`:

```rust
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
```

Replace in `crates/barnacle-catalog/src/lib.rs`:

```rust
pub mod names;
```

with:

```rust
pub mod names;
pub mod store;
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p barnacle-catalog --test store`
Expected: `test result: ok. 5 passed`.

- [ ] **Step 5: Make barnacle-data use the module**

Replace in `crates/barnacle-data/src/store.rs`:

```rust
use barnacle_catalog::Provenance;
```

with:

```rust
use barnacle_catalog::Provenance;
use barnacle_catalog::store::CATALOG_FILE;
use barnacle_catalog::store::CURRENT_FILE;
use barnacle_catalog::store::CatalogDirError;
use barnacle_catalog::store::CatalogRoot;
```

Replace in `crates/barnacle-data/src/store.rs`:

```rust
const CATALOG_FILE: &str = "catalog.json";
const CURRENT_FILE: &str = "current";
```

with:

```rust
```

Replace in `crates/barnacle-data/src/store.rs`:

```rust
pub struct DataDir {
```

with:

```rust
impl From<CatalogDirError> for StoreError {
    fn from(error: CatalogDirError) -> Self {
        match error {
            CatalogDirError::Io { path, source } => Self::Io { path, source },
            CatalogDirError::Json { path, source } => Self::CatalogJson { path, source },
        }
    }
}

pub struct DataDir {
```

Replace in `crates/barnacle-data/src/store.rs`:

```rust
    pub fn catalog_dir(&self, name: &str) -> PathBuf {
        self.catalogs().join(name)
    }
```

with:

```rust
    pub fn catalog_root(&self) -> CatalogRoot {
        CatalogRoot::new(self.catalogs())
    }

    pub fn catalog_dir(&self, name: &str) -> PathBuf {
        self.catalog_root().dir(name)
    }
```

Replace in `crates/barnacle-data/src/store.rs`:

```rust
    pub fn current(&self) -> Result<Option<String>, StoreError> {
        let path = self.catalogs().join(CURRENT_FILE);
        match std::fs::read_to_string(&path) {
            Ok(text) => Ok(Some(text.trim().to_owned())),
            Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
            Err(source) => Err(StoreError::Io { path, source }),
        }
    }
```

with:

```rust
    pub fn current(&self) -> Result<Option<String>, StoreError> {
        Ok(self.catalog_root().current()?)
    }
```

Replace in `crates/barnacle-data/src/store.rs`:

```rust
    pub fn load(&self, name: &str) -> Result<Catalog, StoreError> {
        let path = self.catalog_dir(name).join(CATALOG_FILE);
        let text = std::fs::read_to_string(&path).map_err(io_error(path.clone()))?;
        Catalog::from_json(&text).map_err(|source| StoreError::CatalogJson { path, source })
    }
```

with:

```rust
    pub fn load(&self, name: &str) -> Result<Catalog, StoreError> {
        Ok(self.catalog_root().load(name)?)
    }
```

Replace in `crates/barnacle-data/src/extract/mod.rs`:

```rust
use barnacle_catalog::Silhouette;
```

with:

```rust
use barnacle_catalog::Silhouette;
use barnacle_catalog::store::SILHOUETTES_DIR;
```

Replace in `crates/barnacle-data/src/extract/mod.rs`:

```rust
    let silhouettes = inputs.output_dir.join("silhouettes");
```

with:

```rust
    let silhouettes = inputs.output_dir.join(SILHOUETTES_DIR);
```

`CATALOG_FILE` and `CURRENT_FILE` are still used elsewhere in `store.rs`, and so is `ErrorKind`. The imports stay.

- [ ] **Step 6: Run the tests to verify nothing changed for barnacle-data**

Run: `cargo test -p barnacle-catalog -p barnacle-data && cargo clippy -p barnacle-catalog -p barnacle-data --all-targets -- -D warnings`
Expected:
- All tests pass, including `barnacle-data`'s own `current_is_absent_until_set` and `saved_catalogs_load_back`, which still see `StoreError::Io` for a missing catalog.
- `barnacle-catalog`'s `store` has 5 passing tests.
- Clippy reports no warnings.

- [ ] **Step 7: Commit**

```bash
git add crates/barnacle-catalog/Cargo.toml crates/barnacle-catalog/src/lib.rs crates/barnacle-catalog/src/store.rs crates/barnacle-catalog/tests/store.rs crates/barnacle-data/src/store.rs crates/barnacle-data/src/extract/mod.rs Cargo.lock
git commit -m "refactor(catalog): move catalog folder reading into barnacle-catalog" -- crates/barnacle-catalog/Cargo.toml crates/barnacle-catalog/src/lib.rs crates/barnacle-catalog/src/store.rs crates/barnacle-catalog/tests/store.rs crates/barnacle-data/src/store.rs crates/barnacle-data/src/extract/mod.rs Cargo.lock
```

---

### Task 2: A ship's names, its look-alikes, and message time

**Files:**
- Modify: `crates/barnacle-guess/src/ids.rs`, `crates/barnacle-guess/src/book.rs`
- Test: `crates/barnacle-guess/tests/ids.rs`, `crates/barnacle-guess/tests/answers.rs`

**Interfaces:**
- Produces:
  - `Snowflake::unix_millis(self) -> u64`, computed as `(id >> 22) + 1420070400000`, Discord's documented epoch.
  - `ShipBook::names(&self, &ShipIndex) -> BTreeSet<String>`: the ship's own cleaned names, its variants' names and its aliases. It is empty for a ship outside the book.
  - `ShipBook::lookalikes(&self, &ShipIndex) -> BTreeSet<&ShipIndex>`: every ship sharing a look-alike group with it.

- [ ] **Step 1: Write the failing tests**

Replace in `crates/barnacle-guess/tests/ids.rs`:

```rust
#[test]
fn elapsed_time_is_the_difference_between_the_two_timestamps() {
```

with:

```rust
#[test]
fn converts_discords_documented_example_id_to_unix_time() {
    assert_eq!(
        Snowflake::new(175_928_847_299_117_063).unix_millis(),
        1_462_015_105_796
    );
}

#[test]
fn elapsed_time_is_the_difference_between_the_two_timestamps() {
```

`1462015105796` is 2016-04-30 11:18:25.796 UTC, the timestamp Discord's reference documentation gives for that example ID.

Append to `crates/barnacle-guess/tests/answers.rs`:

```rust
#[test]
fn a_ships_own_names_and_its_lookalikes_can_be_listed() {
    let book = book(
        vec![
            ship("PBSC507", "Belfast", 7, "belfast"),
            ship("PBSC528", "Belfast '43", 8, "belfast-43"),
            ship("PBSC108", "Edinburgh", 8, "edinburgh"),
        ],
        r#"
[[lookalikes]]
ships = ["PBSC507", "PBSC528"]

[[lookalikes]]
ships = ["PBSC507", "PBSC108"]

[[aliases]]
index = "PBSC507"
names = ["Belfast Classic"]
"#,
    );
    assert_eq!(
        book.names(&index("PBSC507")),
        set(["belfast", "belfastclassic"])
    );
    assert_eq!(
        book.lookalikes(&index("PBSC507")),
        [&index("PBSC108"), &index("PBSC528")].into_iter().collect()
    );
    assert!(book.names(&index("PZSX999")).is_empty());
    assert!(
        book.lookalikes(&index("PBSC108"))
            .contains(&index("PBSC507"))
    );
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p barnacle-guess --test ids --test answers`
Expected: FAIL to compile with `error[E0599]: no method named `unix_millis` found for struct `Snowflake``, and the same for `names` and `lookalikes`.

- [ ] **Step 3: Implement**

Replace in `crates/barnacle-guess/src/ids.rs`:

```rust
const TIMESTAMP_SHIFT: u32 = 22;
```

with:

```rust
const TIMESTAMP_SHIFT: u32 = 22;
const DISCORD_EPOCH_MS: u64 = 1_420_070_400_000;
```

Replace in `crates/barnacle-guess/src/ids.rs`:

```rust
    pub fn elapsed_until(self, later: Snowflake) -> Duration {
```

with:

```rust
    pub const fn unix_millis(self) -> u64 {
        self.millis_since_discord_epoch() + DISCORD_EPOCH_MS
    }

    pub fn elapsed_until(self, later: Snowflake) -> Duration {
```

Replace in `crates/barnacle-guess/src/book.rs`:

```rust
    pub fn draw<R: Rng + ?Sized>(
```

with:

```rust
    pub fn names(&self, index: &ShipIndex) -> BTreeSet<String> {
        self.entries
            .get(index)
            .map(|entry| entry.answers.clone())
            .unwrap_or_default()
    }

    pub fn lookalikes(&self, index: &ShipIndex) -> BTreeSet<&ShipIndex> {
        self.lookalikes.get(index).into_iter().flatten().collect()
    }

    pub fn draw<R: Rng + ?Sized>(
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p barnacle-guess && cargo clippy -p barnacle-guess --all-targets -- -D warnings`
Expected: `ids` has 5 passing tests and `answers` 12, every other target still passes, and clippy reports no warnings.

- [ ] **Step 5: Commit**

```bash
git add crates/barnacle-guess/src/ids.rs crates/barnacle-guess/src/book.rs crates/barnacle-guess/tests/ids.rs crates/barnacle-guess/tests/answers.rs
git commit -m "feat(guess): expose a ship's names, look-alikes and message time" -- crates/barnacle-guess/src/ids.rs crates/barnacle-guess/src/book.rs crates/barnacle-guess/tests/ids.rs crates/barnacle-guess/tests/answers.rs
```

---

### Task 3: The bot crate and its SQLite store

**Files:**
- Modify: `Cargo.toml`, `deny.toml`, `.gitignore`
- Create: `migrations/0001_guess_solves.sql`, `migrations/0001_guess_solves.down.sql`
- Create: `crates/barnacle-bot/Cargo.toml`, `crates/barnacle-bot/src/lib.rs`, `crates/barnacle-bot/src/ids.rs`, `crates/barnacle-bot/src/solves.rs`
- Create: `crates/barnacle-bot/tests/common/mod.rs`
- Test: `crates/barnacle-bot/tests/solves.rs`

**Interfaces:**
- Consumes: `barnacle_guess::UserId`, `barnacle_catalog::ShipIndex`.
- Produces:
  - Newtypes: `GuildId::new/get`, `ChannelId::new/get` (both `const fn`, `Copy + Ord + Hash`), and `Place { guild, channel }`.
  - `SolveRecord { guild, user, ship, elapsed, solved_at_ms }` and `Profile { wins: u64, best: Option<Duration> }`.
  - The storage interface:
    - `trait SolveStore: Send + Sync + 'static { fn record(&self, &SolveRecord) -> impl Future<Output = Result<bool, SolvesError>> + Send; }`. The returned `bool` is true when the win is a personal best in that server.
    - `Solves::open(&Path)` opens an existing file. `Solves::with_pool(SqlitePool)` checks the schema. `Solves` implements `SolveStore` and has `profile(GuildId, UserId)`.
  - `SolvesError::{Database, MissingTable, UnexpectedColumns { found }, OutOfRange { value }}`.
  - Test helpers:
    - Catalog builders: `common::{index, tier, at, ship, fleet, catalog, curation_text, curation}`, plus `BUILD = 13187581`.
    - SQLite: `MIGRATION` (the SQL file's text), `memory_pool`, `migrated_pool` and `solves`.

- [ ] **Step 1: Add the dependencies, the migration and the failing tests**

Replace in `Cargo.toml`:

```toml
barnacle-catalog = { path = "crates/barnacle-catalog" }
```

with:

```toml
barnacle-catalog = { path = "crates/barnacle-catalog" }
barnacle-guess = { path = "crates/barnacle-guess" }
```

Replace in `Cargo.toml`:

```toml
rand = "0.10"
```

with:

```toml
poise = "0.7"
rand = "0.10"
```

Replace in `Cargo.toml`:

```toml
sha2 = "0.11"
```

with:

```toml
sha2 = "0.11"
sqlx = { version = "0.9", default-features = false, features = ["runtime-tokio", "sqlite"] }
```

Replace in `Cargo.toml`:

```toml
toml = "1"
```

with:

```toml
toml = "1"
tracing = "0.1"
tracing-subscriber = "0.3"
```

Replace in `deny.toml`:

```toml
    "CC0-1.0",
```

with:

```toml
    "CC0-1.0",
    "CDLA-Permissive-2.0",
```

Append to `.gitignore` (no blank line; the file is a plain list):

```text
/barnacle.toml
```

Whole file `migrations/0001_guess_solves.sql`:

```sql
BEGIN;

CREATE TABLE IF NOT EXISTS guess_solves (
    id INTEGER PRIMARY KEY,
    guild_id INTEGER NOT NULL,
    user_id INTEGER NOT NULL,
    ship_index TEXT NOT NULL,
    elapsed_ms INTEGER NOT NULL CHECK (elapsed_ms >= 0),
    solved_at_ms INTEGER NOT NULL
) STRICT;

CREATE INDEX IF NOT EXISTS guess_solves_by_player
    ON guess_solves (guild_id, user_id, elapsed_ms);

COMMIT;
```

Whole file `migrations/0001_guess_solves.down.sql`:

```sql
BEGIN;

DROP INDEX IF EXISTS guess_solves_by_player;
DROP TABLE IF EXISTS guess_solves;

COMMIT;
```

Whole file `crates/barnacle-bot/Cargo.toml`:

```toml
[package]
name = "barnacle-bot"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true

[dependencies]
barnacle-catalog.workspace = true
barnacle-guess.workspace = true
clap.workspace = true
poise.workspace = true
rand.workspace = true
serde.workspace = true
sqlx.workspace = true
thiserror.workspace = true
tokio = { workspace = true, features = ["fs", "macros", "sync", "time"] }
toml.workspace = true
tracing.workspace = true
tracing-subscriber.workspace = true

[dev-dependencies]
tempfile.workspace = true
tokio = { workspace = true, features = ["test-util"] }
```

`Cargo.toml` already lists every dependency later tasks use, so the manifest does not change again.

Whole file `crates/barnacle-bot/src/lib.rs`:

```rust
```

Whole file `crates/barnacle-bot/tests/common/mod.rs`:

```rust
#![allow(dead_code)]

use barnacle_bot::solves::Solves;
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
use barnacle_guess::Snowflake;
use sqlx::sqlite::SqlitePool;
use sqlx::sqlite::SqlitePoolOptions;

pub const MIGRATION: &str = include_str!("../../../../migrations/0001_guess_solves.sql");
pub const BUILD: u32 = 13187581;

pub fn index(value: &str) -> ShipIndex {
    ShipIndex::parse(value).unwrap()
}

pub fn tier(value: u32) -> Tier {
    Tier::new(value).unwrap()
}

pub fn at(millis: u64) -> Snowflake {
    Snowflake::new(millis << 22)
}

pub fn ship(value: &str, name: &str, tier_value: u32, silhouette: &str) -> Ship {
    Ship {
        id: ParamId::new(1),
        index: index(value),
        tier: tier(tier_value),
        group: ShipGroup::new("upgradeable"),
        class: ShipClass::Battleship,
        nation: Nation::new("Japan"),
        is_paper: false,
        name: Some(ShipName {
            short: name.to_owned(),
            full: None,
        }),
        silhouette: Some(Silhouette {
            sha256: silhouette.to_owned(),
        }),
    }
}

pub fn fleet(size: usize) -> Vec<Ship> {
    (0..size)
        .map(|number| {
            ship(
                &format!("PXSX{number:03}"),
                &format!("Hull {number}"),
                8,
                &format!("silhouette-{number}"),
            )
        })
        .collect()
}

pub fn catalog(ships: Vec<Ship>) -> Catalog {
    Catalog {
        provenance: Provenance {
            game_version: "15.8.0".to_owned(),
            build: BUILD,
            data_repo_commit: "442496ea2f27517507a562f6eb3ceee06003d3da".to_owned(),
            wowsunpack: "0.45.0".to_owned(),
            wows_data_mgr: "0.21.0".to_owned(),
        },
        ships,
    }
}

pub fn curation_text(tables: &str) -> String {
    format!("reviewed_through = {BUILD}\ngroups = [\"upgradeable\"]\n{tables}")
}

pub fn curation(tables: &str) -> CurationConfig {
    CurationConfig::from_toml(&curation_text(tables)).unwrap()
}

pub async fn memory_pool() -> SqlitePool {
    SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap()
}

pub async fn migrated_pool() -> SqlitePool {
    let pool = memory_pool().await;
    sqlx::raw_sql(MIGRATION).execute(&pool).await.unwrap();
    pool
}

pub async fn solves() -> Solves {
    Solves::with_pool(migrated_pool().await).await.unwrap()
}
```

Whole file `crates/barnacle-bot/tests/solves.rs`:

```rust
mod common;

use std::time::Duration;

use barnacle_bot::ids::GuildId;
use barnacle_bot::solves::Profile;
use barnacle_bot::solves::SolveRecord;
use barnacle_bot::solves::SolveStore;
use barnacle_bot::solves::Solves;
use barnacle_bot::solves::SolvesError;
use barnacle_guess::UserId;
use common::index;
use common::memory_pool;
use common::solves;
use sqlx::sqlite::SqliteConnectOptions;
use sqlx::sqlite::SqlitePoolOptions;

const GUILD: GuildId = GuildId::new(1);
const PLAYER: UserId = UserId::new(200);

fn win(guild: GuildId, user: UserId, millis: u64) -> SolveRecord {
    SolveRecord {
        guild,
        user,
        ship: index("PJSB018"),
        elapsed: Duration::from_millis(millis),
        solved_at_ms: 1_700_000_000_000,
    }
}

#[tokio::test]
async fn a_recorded_win_shows_in_the_profile() {
    let solves = solves().await;
    assert_eq!(
        solves.profile(GUILD, PLAYER).await.unwrap(),
        Profile {
            wins: 0,
            best: None
        }
    );
    solves.record(&win(GUILD, PLAYER, 3_251)).await.unwrap();
    solves.record(&win(GUILD, PLAYER, 4_000)).await.unwrap();
    assert_eq!(
        solves.profile(GUILD, PLAYER).await.unwrap(),
        Profile {
            wins: 2,
            best: Some(Duration::from_millis(3_251))
        }
    );
}

#[tokio::test]
async fn profiles_are_kept_per_server_and_per_player() {
    let solves = solves().await;
    solves.record(&win(GUILD, PLAYER, 3_000)).await.unwrap();
    assert_eq!(
        solves.profile(GuildId::new(2), PLAYER).await.unwrap().wins,
        0
    );
    assert_eq!(
        solves.profile(GUILD, UserId::new(201)).await.unwrap().wins,
        0
    );
}

#[tokio::test]
async fn only_a_strictly_faster_time_is_a_personal_best() {
    let solves = solves().await;
    assert!(solves.record(&win(GUILD, PLAYER, 5_000)).await.unwrap());
    assert!(!solves.record(&win(GUILD, PLAYER, 6_000)).await.unwrap());
    assert!(!solves.record(&win(GUILD, PLAYER, 5_000)).await.unwrap());
    assert!(solves.record(&win(GUILD, PLAYER, 4_999)).await.unwrap());
    assert!(
        solves
            .record(&win(GuildId::new(2), PLAYER, 9_000))
            .await
            .unwrap()
    );
}

#[tokio::test]
async fn a_database_without_the_table_is_refused() {
    assert!(matches!(
        Solves::with_pool(memory_pool().await).await,
        Err(SolvesError::MissingTable)
    ));
}

#[tokio::test]
async fn a_table_with_other_columns_is_refused() {
    let pool = memory_pool().await;
    sqlx::raw_sql("CREATE TABLE guess_solves (id INTEGER PRIMARY KEY, user_id INTEGER NOT NULL)")
        .execute(&pool)
        .await
        .unwrap();
    assert!(matches!(
        Solves::with_pool(pool).await,
        Err(SolvesError::UnexpectedColumns { .. })
    ));
}

#[tokio::test]
async fn a_table_with_the_right_names_but_other_types_is_refused() {
    let pool = memory_pool().await;
    sqlx::raw_sql(
        "CREATE TABLE guess_solves (id INTEGER PRIMARY KEY, guild_id INTEGER NOT NULL, user_id INTEGER NOT NULL, ship_index TEXT NOT NULL, elapsed_ms TEXT NOT NULL, solved_at_ms INTEGER NOT NULL) STRICT",
    )
    .execute(&pool)
    .await
    .unwrap();
    assert!(matches!(
        Solves::with_pool(pool).await,
        Err(SolvesError::UnexpectedColumns { .. })
    ));
}

#[tokio::test]
async fn an_id_too_large_for_sqlite_is_refused() {
    let solves = solves().await;
    assert!(matches!(
        solves
            .record(&win(GUILD, UserId::new(u64::MAX), 1_000))
            .await,
        Err(SolvesError::OutOfRange { .. })
    ));
}

#[tokio::test]
async fn opening_a_missing_file_fails_without_creating_it() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("barnacle.sqlite3");
    assert!(Solves::open(&path).await.is_err());
    assert!(!path.exists());
}

#[tokio::test]
async fn opening_a_migrated_file_works() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("barnacle.sqlite3");
    let setup = SqlitePoolOptions::new()
        .connect_with(
            SqliteConnectOptions::new()
                .filename(&path)
                .create_if_missing(true),
        )
        .await
        .unwrap();
    sqlx::raw_sql(common::MIGRATION)
        .execute(&setup)
        .await
        .unwrap();
    setup.close().await;
    let solves = Solves::open(&path).await.unwrap();
    assert!(solves.record(&win(GUILD, PLAYER, 1_000)).await.unwrap());
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p barnacle-bot --test solves`
Expected: FAIL to compile with `error[E0432]: unresolved import `barnacle_bot::solves``. The first build also downloads and compiles poise, serenity and sqlx, which takes a few minutes.

- [ ] **Step 3: Implement the IDs and the store**

Whole file `crates/barnacle-bot/src/ids.rs`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GuildId(u64);

impl GuildId {
    pub const fn new(raw: u64) -> Self {
        Self(raw)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ChannelId(u64);

impl ChannelId {
    pub const fn new(raw: u64) -> Self {
        Self(raw)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Place {
    pub guild: GuildId,
    pub channel: ChannelId,
}
```

Whole file `crates/barnacle-bot/src/solves.rs`:

```rust
use std::future::Future;
use std::path::Path;
use std::time::Duration;

use barnacle_catalog::ShipIndex;
use barnacle_guess::UserId;
use sqlx::sqlite::SqliteConnectOptions;
use sqlx::sqlite::SqlitePool;
use sqlx::sqlite::SqlitePoolOptions;

use crate::ids::GuildId;

const BUSY_TIMEOUT: Duration = Duration::from_secs(5);
const EXPECTED_COLUMNS: [(&str, &str, bool, bool); 6] = [
    ("id", "INTEGER", false, true),
    ("guild_id", "INTEGER", true, false),
    ("user_id", "INTEGER", true, false),
    ("ship_index", "TEXT", true, false),
    ("elapsed_ms", "INTEGER", true, false),
    ("solved_at_ms", "INTEGER", true, false),
];

#[derive(Debug, thiserror::Error)]
pub enum SolvesError {
    #[error("the solves database could not be used")]
    Database(#[from] sqlx::Error),
    #[error("the solves database has no guess_solves table")]
    MissingTable,
    #[error("guess_solves does not have the expected columns; found {found:?}")]
    UnexpectedColumns { found: Vec<Column> },
    #[error("{value} does not fit in a SQLite integer")]
    OutOfRange { value: u128 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Column {
    pub name: String,
    pub kind: String,
    pub not_null: bool,
    pub primary_key: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SolveRecord {
    pub guild: GuildId,
    pub user: UserId,
    pub ship: ShipIndex,
    pub elapsed: Duration,
    pub solved_at_ms: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Profile {
    pub wins: u64,
    pub best: Option<Duration>,
}

pub trait SolveStore: Send + Sync + 'static {
    fn record(
        &self,
        record: &SolveRecord,
    ) -> impl Future<Output = Result<bool, SolvesError>> + Send;
}

#[derive(Debug, Clone)]
pub struct Solves {
    pool: SqlitePool,
}

impl Solves {
    pub async fn open(path: &Path) -> Result<Self, SolvesError> {
        let options = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(false)
            .busy_timeout(BUSY_TIMEOUT);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await?;
        Self::with_pool(pool).await
    }

    pub async fn with_pool(pool: SqlitePool) -> Result<Self, SolvesError> {
        let found = columns(&pool).await?;
        let expected: Vec<Column> = EXPECTED_COLUMNS
            .iter()
            .map(|(name, kind, not_null, primary_key)| Column {
                name: (*name).to_owned(),
                kind: (*kind).to_owned(),
                not_null: *not_null,
                primary_key: *primary_key,
            })
            .collect();
        if found.is_empty() {
            Err(SolvesError::MissingTable)
        } else if found != expected {
            Err(SolvesError::UnexpectedColumns { found })
        } else {
            Ok(Self { pool })
        }
    }

    pub async fn profile(&self, guild: GuildId, user: UserId) -> Result<Profile, SolvesError> {
        let (wins, best): (i64, Option<i64>) = sqlx::query_as(
            "SELECT COUNT(*), MIN(elapsed_ms) FROM guess_solves WHERE guild_id = ? AND user_id = ?",
        )
        .bind(to_integer(guild.get().into())?)
        .bind(to_integer(user.get().into())?)
        .fetch_one(&self.pool)
        .await?;
        Ok(Profile {
            wins: u64::try_from(wins).unwrap_or_default(),
            best: best
                .and_then(|millis| u64::try_from(millis).ok())
                .map(Duration::from_millis),
        })
    }
}

impl SolveStore for Solves {
    async fn record(&self, record: &SolveRecord) -> Result<bool, SolvesError> {
        let guild = to_integer(record.guild.get().into())?;
        let user = to_integer(record.user.get().into())?;
        let elapsed = to_integer(record.elapsed.as_millis())?;
        let solved_at = to_integer(record.solved_at_ms.into())?;
        let mut transaction = self.pool.begin().await?;
        let previous: Option<i64> = sqlx::query_scalar(
            "SELECT MIN(elapsed_ms) FROM guess_solves WHERE guild_id = ? AND user_id = ?",
        )
        .bind(guild)
        .bind(user)
        .fetch_one(&mut *transaction)
        .await?;
        sqlx::query(
            "INSERT INTO guess_solves (guild_id, user_id, ship_index, elapsed_ms, solved_at_ms) VALUES (?, ?, ?, ?, ?)",
        )
        .bind(guild)
        .bind(user)
        .bind(record.ship.as_str())
        .bind(elapsed)
        .bind(solved_at)
        .execute(&mut *transaction)
        .await?;
        transaction.commit().await?;
        Ok(previous.is_none_or(|best| elapsed < best))
    }
}

async fn columns(pool: &SqlitePool) -> Result<Vec<Column>, SolvesError> {
    let rows: Vec<(String, String, i64, i64)> = sqlx::query_as(
        "SELECT name, type, \"notnull\", pk FROM pragma_table_info('guess_solves') ORDER BY cid",
    )
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|(name, kind, not_null, primary_key)| Column {
            name,
            kind,
            not_null: not_null != 0,
            primary_key: primary_key != 0,
        })
        .collect())
}

fn to_integer(value: u128) -> Result<i64, SolvesError> {
    i64::try_from(value).map_err(|_| SolvesError::OutOfRange { value })
}
```

Whole file `crates/barnacle-bot/src/lib.rs`:

```rust
pub mod ids;
pub mod solves;
```

- [ ] **Step 4: Run the tests, the license check and a migration check**

Run: `cargo test -p barnacle-bot && cargo clippy -p barnacle-bot --all-targets -- -D warnings && cargo deny check licenses`
Expected: `solves` has 9 passing tests, clippy reports no warnings, and the license check prints `licenses ok`. Without the `deny.toml` line, it would reject `webpki-roots` (CDLA-Permissive-2.0).

Run: `tmp=$(mktemp -d) && sqlite3 "$tmp/check.sqlite3" < migrations/0001_guess_solves.sql && sqlite3 "$tmp/check.sqlite3" < migrations/0001_guess_solves.sql && sqlite3 "$tmp/check.sqlite3" "SELECT name, type FROM pragma_table_info('guess_solves') ORDER BY cid;" && sqlite3 "$tmp/check.sqlite3" < migrations/0001_guess_solves.down.sql && sqlite3 "$tmp/check.sqlite3" "SELECT count(*) FROM sqlite_master;" && rm -r "$tmp"`
Expected:
- The migration applies twice without error.
- The columns print as `id|INTEGER`, `guild_id|INTEGER`, `user_id|INTEGER`, `ship_index|TEXT`, `elapsed_ms|INTEGER` and `solved_at_ms|INTEGER`.
- After the rollback, the last query prints `0`.

This uses a throwaway file in a temporary folder, never `data/barnacle.sqlite3`.

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml Cargo.lock deny.toml .gitignore migrations crates/barnacle-bot
git commit -m "feat(bot): store solves in SQLite" -- Cargo.toml Cargo.lock deny.toml .gitignore migrations crates/barnacle-bot
```

---

### Task 4: The bot configuration

**Files:**
- Create: `crates/barnacle-bot/src/config.rs`, `barnacle.example.toml`
- Modify: `crates/barnacle-bot/src/lib.rs`
- Test: `crates/barnacle-bot/tests/config.rs`

**Interfaces:**
- Produces:
  - `Config { data_dir, curation, database, commands: CommandScope }`, built by `Config::from_toml(&str) -> Result<Config, ConfigError>`. `Config::catalogs()` returns `data_dir/catalog`.
  - `CommandScope::{Guilds { guilds: Vec<u64> }, Global}`.
  - `ConfigError::{Toml, NoGuilds, ZeroGuild, GuildsWithGlobal}`.
  - The defaults are `data`, `curation/ships.toml` and `data/barnacle.sqlite3`. `commands` is required.

- [ ] **Step 1: Write the failing tests**

Whole file `barnacle.example.toml`:

```toml
data_dir = "data"
curation = "curation/ships.toml"
database = "data/barnacle.sqlite3"

[commands]
scope = "guilds"
guilds = []
```

The example lists no servers on purpose: the bot refuses it until the owner adds their test server's ID.

Whole file `crates/barnacle-bot/tests/config.rs`:

```rust
use std::path::PathBuf;

use barnacle_bot::config::CommandScope;
use barnacle_bot::config::Config;
use barnacle_bot::config::ConfigError;

#[test]
fn a_server_list_config_uses_the_default_paths() {
    let config = Config::from_toml(
        r#"
[commands]
scope = "guilds"
guilds = [123456789012345678]
"#,
    )
    .unwrap();
    assert_eq!(
        config,
        Config {
            data_dir: PathBuf::from("data"),
            curation: PathBuf::from("curation/ships.toml"),
            database: PathBuf::from("data/barnacle.sqlite3"),
            commands: CommandScope::Guilds {
                guilds: vec![123456789012345678],
            },
        }
    );
    assert_eq!(config.catalogs(), PathBuf::from("data/catalog"));
}

#[test]
fn a_global_config_can_set_every_path() {
    let config = Config::from_toml(
        r#"
data_dir = "/srv/barnacle/data"
curation = "/srv/barnacle/ships.toml"
database = "/srv/barnacle/barnacle.sqlite3"

[commands]
scope = "global"
"#,
    )
    .unwrap();
    assert_eq!(config.commands, CommandScope::Global);
    assert_eq!(
        config.catalogs(),
        PathBuf::from("/srv/barnacle/data/catalog")
    );
}

#[test]
fn a_server_list_must_name_real_servers() {
    assert!(matches!(
        Config::from_toml("[commands]\nscope = \"guilds\"\nguilds = []\n"),
        Err(ConfigError::NoGuilds)
    ));
    assert!(matches!(
        Config::from_toml("[commands]\nscope = \"guilds\"\nguilds = [0]\n"),
        Err(ConfigError::ZeroGuild)
    ));
}

#[test]
fn unknown_keys_and_a_missing_scope_are_refused() {
    assert!(matches!(
        Config::from_toml("[commands]\nscope = \"global\"\nguilds = [1]\n"),
        Err(ConfigError::GuildsWithGlobal)
    ));
    assert!(matches!(
        Config::from_toml("[commands]\nscope = \"guilds\"\n"),
        Err(ConfigError::NoGuilds)
    ));
    assert!(matches!(
        Config::from_toml("token = \"abc\"\n[commands]\nscope = \"global\"\n"),
        Err(ConfigError::Toml(_))
    ));
    assert!(matches!(
        Config::from_toml("data_dir = \"data\"\n"),
        Err(ConfigError::Toml(_))
    ));
}

#[test]
fn the_example_config_only_lacks_server_ids() {
    assert!(matches!(
        Config::from_toml(include_str!("../../../barnacle.example.toml")),
        Err(ConfigError::NoGuilds)
    ));
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p barnacle-bot --test config`
Expected: FAIL to compile with `error[E0432]: unresolved import `barnacle_bot::config``.

- [ ] **Step 3: Implement**

Whole file `crates/barnacle-bot/src/config.rs`:

```rust
use std::path::PathBuf;

use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub data_dir: PathBuf,
    pub curation: PathBuf,
    pub database: PathBuf,
    pub commands: CommandScope,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandScope {
    Guilds { guilds: Vec<u64> },
    Global,
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("the config is not valid")]
    Toml(#[from] toml::de::Error),
    #[error("commands.scope is \"guilds\" but commands.guilds lists no server")]
    NoGuilds,
    #[error("commands.guilds contains 0, which is not a Discord server ID")]
    ZeroGuild,
    #[error("commands.scope is \"global\", so commands.guilds must be left out")]
    GuildsWithGlobal,
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
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CommandsFile {
    scope: ScopeName,
    guilds: Option<Vec<u64>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum ScopeName {
    Guilds,
    Global,
}

impl Config {
    pub fn from_toml(text: &str) -> Result<Self, ConfigError> {
        let file: ConfigFile = toml::from_str(text)?;
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
        Ok(Self {
            data_dir: file.data_dir,
            curation: file.curation,
            database: file.database,
            commands,
        })
    }

    pub fn catalogs(&self) -> PathBuf {
        self.data_dir.join("catalog")
    }
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
```

Whole file `crates/barnacle-bot/src/lib.rs`:

```rust
pub mod config;
pub mod ids;
pub mod solves;
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p barnacle-bot && cargo clippy -p barnacle-bot --all-targets -- -D warnings`
Expected: `config` has 5 passing tests and `solves` 9, and clippy reports no warnings.

- [ ] **Step 5: Commit**

```bash
git add barnacle.example.toml crates/barnacle-bot/src/config.rs crates/barnacle-bot/src/lib.rs crates/barnacle-bot/tests/config.rs
git commit -m "feat(bot): read the bot configuration" -- barnacle.example.toml crates/barnacle-bot/src/config.rs crates/barnacle-bot/src/lib.rs crates/barnacle-bot/tests/config.rs
```

---

### Task 5: Wording and labels

**Files:**
- Create: `crates/barnacle-bot/src/text.rs`
- Modify: `crates/barnacle-bot/src/lib.rs`
- Test: `crates/barnacle-bot/tests/text.rs`

**Interfaces:**
- Consumes: `Reveal`, `Hint`, `RoundOptions`, `Timing` and `UserId` from `barnacle-guess`, and `Provenance`, `Nation`, `ShipClass` and `Tier` from `barnacle-catalog`.
- Produces:
  - **Constants:** `EMBED_COLOUR`, `ROUND_TITLE`, `ROUND_DESCRIPTION`, `TIERS_FIELD`, `PAPER_EXCLUDED`, `CANCEL_LABEL`, `ALREADY_RUNNING`, `CANCEL_REFUSED`, `ROUND_OVER`, `SOMETHING_WENT_WRONG`, `NO_SHIP_MATCHES`, `ROUNDS_WON`, `BEST_TIME`, `NO_WINS_HERE`, `ABOUT_TITLE`, `ABOUT_SUMMARY`, `SOURCE_URL`, `WARGAMING_NOTICE` and `FIELD_LIMIT = 1024`.
  - **Labels:** `tier_numeral`, `tier_range`, `class_label`, `nation_label` and `seconds`.
  - **Formatting:** `escape`, `round_footer`, `ship_line` and `listing(&[String], limit)`.
  - **Sentences:** `hint`, `win`, `timed_out`, `cancelled`, `empty_pool`, `best_time` and `about(&Provenance, catalog_name, bot_version)`.
  - A sentence never ends with two full stops, even after "U.K.". The wording is exactly design section 7.1, and the labels are exactly 7.2.

- [ ] **Step 1: Write the failing tests**

Whole file `crates/barnacle-bot/tests/text.rs`:

```rust
mod common;

use std::time::Duration;

use barnacle_bot::text;
use barnacle_catalog::Nation;
use barnacle_catalog::ShipClass;
use barnacle_guess::Hint;
use barnacle_guess::Reveal;
use barnacle_guess::RoundOptions;
use barnacle_guess::Timing;
use barnacle_guess::UserId;
use common::index;
use common::tier;

fn yamato() -> Reveal {
    Reveal {
        index: index("PJSB018"),
        name: "Yamato".to_owned(),
        tier: tier(10),
        nation: Nation::new("Japan"),
        class: ShipClass::Battleship,
    }
}

fn warspite() -> Reveal {
    Reveal {
        index: index("PBSB105"),
        name: "Warspite".to_owned(),
        tier: tier(6),
        nation: Nation::new("United_Kingdom"),
        class: ShipClass::Battleship,
    }
}

#[test]
fn tiers_are_roman_numerals() {
    let numerals: Vec<&str> = (1..=11)
        .map(|value| text::tier_numeral(tier(value)))
        .collect();
    assert_eq!(
        numerals,
        [
            "I", "II", "III", "IV", "V", "VI", "VII", "VIII", "IX", "X", "XI"
        ]
    );
    assert_eq!(text::tier_range(&RoundOptions::default()), "VI-XI");
    assert_eq!(
        text::tier_range(&RoundOptions::new(Some(tier(8)), Some(tier(8)), None)),
        "VIII"
    );
}

#[test]
fn nations_get_barnacles_labels() {
    let labels: Vec<String> = [
        "USA",
        "United_Kingdom",
        "Russia",
        "Pan_Asia",
        "Pan_America",
        "Events",
        "Japan",
        "Commonwealth",
        "Some_New_Nation",
    ]
    .into_iter()
    .map(|nation| text::nation_label(&Nation::new(nation)))
    .collect();
    assert_eq!(
        labels,
        [
            "U.S.A.",
            "U.K.",
            "U.S.S.R.",
            "Pan-Asia",
            "Pan-America",
            "Event",
            "Japan",
            "Commonwealth",
            "Some New Nation"
        ]
    );
}

#[test]
fn classes_get_plain_labels() {
    let labels: Vec<String> = [
        ShipClass::Destroyer,
        ShipClass::Cruiser,
        ShipClass::Battleship,
        ShipClass::AircraftCarrier,
        ShipClass::Submarine,
        ShipClass::Other("Auxiliary".to_owned()),
        ShipClass::Unspecified,
    ]
    .iter()
    .map(text::class_label)
    .collect();
    assert_eq!(
        labels,
        [
            "destroyer",
            "cruiser",
            "battleship",
            "aircraft carrier",
            "submarine",
            "Auxiliary",
            "unknown class"
        ]
    );
}

#[test]
fn times_show_three_decimals() {
    assert_eq!(text::seconds(Duration::from_millis(3_251)), "3.251 s");
    assert_eq!(text::seconds(Duration::from_millis(60_005)), "60.005 s");
    assert_eq!(text::best_time(None), "No rounds won here yet.");
    assert_eq!(text::best_time(Some(Duration::from_millis(900))), "0.900 s");
}

#[test]
fn hints_never_end_with_two_full_stops() {
    assert_eq!(text::hint(&Hint::Tier(tier(8))), "Hint: it's tier VIII.");
    assert_eq!(
        text::hint(&Hint::Nation(Nation::new("Japan"))),
        "Hint: it's from Japan."
    );
    assert_eq!(
        text::hint(&Hint::Nation(Nation::new("United_Kingdom"))),
        "Hint: it's from U.K."
    );
}

#[test]
fn round_endings_describe_the_ship() {
    assert_eq!(
        text::win(&yamato(), Duration::from_millis(3_251), Some(false)),
        "Correct: **Yamato**, tier X battleship, Japan. Solved in 3.251 s."
    );
    assert_eq!(
        text::win(&yamato(), Duration::from_millis(3_251), Some(true)),
        "Correct: **Yamato**, tier X battleship, Japan. Solved in 3.251 s. New personal best."
    );
    assert_eq!(
        text::win(&warspite(), Duration::from_millis(12_000), None),
        "Correct: **Warspite**, tier VI battleship, U.K. Solved in 12.000 s."
    );
    assert_eq!(
        text::timed_out(&yamato()),
        "Nobody named it. It was **Yamato**, tier X battleship, Japan."
    );
    assert_eq!(
        text::cancelled(UserId::new(42), &warspite()),
        "<@42> ended the round. It was **Warspite**, tier VI battleship, U.K."
    );
}

#[test]
fn ship_names_cannot_inject_formatting() {
    assert_eq!(text::escape("*Hood*_[x]"), "\\*Hood\\*\\_\\[x]");
}

#[test]
fn an_empty_pool_names_the_options() {
    assert_eq!(
        text::empty_pool(&RoundOptions::new(Some(tier(6)), Some(tier(8)), None)),
        "No ships fit tiers VI-VIII."
    );
    assert_eq!(
        text::empty_pool(&RoundOptions::new(Some(tier(6)), Some(tier(8)), Some(true))),
        "No ships fit tiers VI-VIII with paper ships excluded."
    );
    assert_eq!(
        text::empty_pool(&RoundOptions::new(Some(tier(2)), Some(tier(2)), None)),
        "No ships fit tier II."
    );
}

#[test]
fn the_footer_follows_the_timing() {
    assert_eq!(text::round_footer(Timing::STANDARD), "Hint in 20 seconds");
}

#[test]
fn long_lists_are_cut_to_the_limit() {
    let names: Vec<String> = ["alpha", "bravo", "charlie", "delta"]
        .map(str::to_owned)
        .to_vec();
    assert_eq!(text::listing(&[], 100), "None");
    assert_eq!(text::listing(&names, 100), "alpha, bravo, charlie, delta");
    assert_eq!(text::listing(&names, 25), "alpha, bravo and 2 more");
    assert_eq!(text::listing(&names, 7), "4 more");
    assert!(text::listing(&names, 25).chars().count() <= 25);
}

#[test]
fn about_names_the_data_and_carries_the_wargaming_notice() {
    let about = text::about(
        &common::catalog(Vec::new()).provenance,
        "15.8.0_13187581_r4",
        "0.1.0",
    );
    assert!(about.contains("World of Warships 15.8.0 (build 13187581), catalog 15.8.0_13187581_r4, data commit 442496e."));
    assert!(about.contains("wowsunpack 0.45.0 and wows-data-mgr 0.21.0"));
    assert!(about.contains("Barnacle 0.1.0"));
    assert!(about.ends_with(text::WARGAMING_NOTICE));
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p barnacle-bot --test text`
Expected: FAIL to compile with `error[E0432]: unresolved import `barnacle_bot::text``.

- [ ] **Step 3: Implement**

Whole file `crates/barnacle-bot/src/text.rs`:

```rust
use std::time::Duration;

use barnacle_catalog::Nation;
use barnacle_catalog::Provenance;
use barnacle_catalog::ShipClass;
use barnacle_catalog::Tier;
use barnacle_guess::Hint;
use barnacle_guess::Reveal;
use barnacle_guess::RoundOptions;
use barnacle_guess::Timing;
use barnacle_guess::UserId;

pub const EMBED_COLOUR: u32 = 0x2E6F6B;
pub const ROUND_TITLE: &str = "Name that ship";
pub const ROUND_DESCRIPTION: &str = "First correct answer in chat wins.";
pub const TIERS_FIELD: &str = "Tiers";
pub const PAPER_EXCLUDED: &str = "Paper ships excluded";
pub const CANCEL_LABEL: &str = "Cancel";
pub const ALREADY_RUNNING: &str = "This channel already has a round running.";
pub const CANCEL_REFUSED: &str =
    "Only the player who started this round, or someone who can manage messages, can end it.";
pub const ROUND_OVER: &str = "That round has already ended.";
pub const SOMETHING_WENT_WRONG: &str = "Something went wrong. Nothing was changed.";
pub const NO_SHIP_MATCHES: &str = "No ship matches that.";
pub const ROUNDS_WON: &str = "Rounds won";
pub const BEST_TIME: &str = "Best time";
pub const NO_WINS_HERE: &str = "No rounds won here yet.";
pub const ABOUT_TITLE: &str = "About Barnacle";
pub const ABOUT_SUMMARY: &str = "Barnacle is a Discord bot for World of Warships players. Its first game asks you to name a ship from its silhouette.";
pub const SOURCE_URL: &str = "https://github.com/SatanshuMishra/barnacle";
pub const WARGAMING_NOTICE: &str = "Barnacle is an unofficial fan project. It is not affiliated with, endorsed by, or supported by Wargaming. World of Warships, its ship names and its ship silhouettes are trademarks or copyrighted works of Wargaming.";
pub const FIELD_LIMIT: usize = 1024;

const NUMERALS: [&str; 11] = [
    "I", "II", "III", "IV", "V", "VI", "VII", "VIII", "IX", "X", "XI",
];
const MARKDOWN: [char; 7] = ['\\', '*', '_', '~', '`', '|', '['];

pub fn tier_numeral(tier: Tier) -> &'static str {
    NUMERALS[tier.get() as usize - 1]
}

pub fn tier_range(options: &RoundOptions) -> String {
    let low = tier_numeral(options.min_tier());
    if options.spans_several_tiers() {
        format!("{low}-{}", tier_numeral(options.max_tier()))
    } else {
        low.to_owned()
    }
}

pub fn class_label(class: &ShipClass) -> String {
    match class {
        ShipClass::Destroyer => "destroyer".to_owned(),
        ShipClass::Cruiser => "cruiser".to_owned(),
        ShipClass::Battleship => "battleship".to_owned(),
        ShipClass::AircraftCarrier => "aircraft carrier".to_owned(),
        ShipClass::Submarine => "submarine".to_owned(),
        ShipClass::Other(name) => name.clone(),
        ShipClass::Unspecified => "unknown class".to_owned(),
    }
}

pub fn nation_label(nation: &Nation) -> String {
    match nation.as_str() {
        "USA" => "U.S.A.".to_owned(),
        "United_Kingdom" => "U.K.".to_owned(),
        "Russia" => "U.S.S.R.".to_owned(),
        "Pan_Asia" => "Pan-Asia".to_owned(),
        "Pan_America" => "Pan-America".to_owned(),
        "Events" => "Event".to_owned(),
        other => other.replace('_', " "),
    }
}

pub fn seconds(elapsed: Duration) -> String {
    format!("{}.{:03} s", elapsed.as_secs(), elapsed.subsec_millis())
}

pub fn escape(text: &str) -> String {
    text.chars()
        .flat_map(|c| {
            let slash = MARKDOWN.contains(&c).then_some('\\');
            slash.into_iter().chain(std::iter::once(c))
        })
        .collect()
}

pub fn round_footer(timing: Timing) -> String {
    format!("Hint in {} seconds", timing.before_hint.as_secs())
}

pub fn ship_line(reveal: &Reveal) -> String {
    format!(
        "**{}**, tier {} {}, {}",
        escape(&reveal.name),
        tier_numeral(reveal.tier),
        class_label(&reveal.class),
        nation_label(&reveal.nation)
    )
}

pub fn hint(hint: &Hint) -> String {
    match hint {
        Hint::Tier(tier) => sentence(&format!("Hint: it's tier {}", tier_numeral(*tier))),
        Hint::Nation(nation) => sentence(&format!("Hint: it's from {}", nation_label(nation))),
    }
}

pub fn win(reveal: &Reveal, elapsed: Duration, personal_best: Option<bool>) -> String {
    let result = format!(
        "{} Solved in {}.",
        sentence(&format!("Correct: {}", ship_line(reveal))),
        seconds(elapsed)
    );
    if personal_best == Some(true) {
        format!("{result} New personal best.")
    } else {
        result
    }
}

pub fn timed_out(reveal: &Reveal) -> String {
    format!(
        "Nobody named it. {}",
        sentence(&format!("It was {}", ship_line(reveal)))
    )
}

pub fn cancelled(by: UserId, reveal: &Reveal) -> String {
    format!(
        "<@{}> ended the round. {}",
        by.get(),
        sentence(&format!("It was {}", ship_line(reveal)))
    )
}

pub fn empty_pool(options: &RoundOptions) -> String {
    let tiers = if options.spans_several_tiers() {
        format!("tiers {}", tier_range(options))
    } else {
        format!("tier {}", tier_range(options))
    };
    if options.historical() {
        format!("No ships fit {tiers} with paper ships excluded.")
    } else {
        format!("No ships fit {tiers}.")
    }
}

pub fn best_time(best: Option<Duration>) -> String {
    best.map(seconds).unwrap_or_else(|| NO_WINS_HERE.to_owned())
}

pub fn about(provenance: &Provenance, catalog_name: &str, bot_version: &str) -> String {
    let commit: String = provenance.data_repo_commit.chars().take(7).collect();
    [
        ABOUT_SUMMARY.to_owned(),
        format!(
            "Ship data: World of Warships {} (build {}), catalog {catalog_name}, data commit {commit}.",
            provenance.game_version, provenance.build
        ),
        format!(
            "Built with wowsunpack {} and wows-data-mgr {}. Barnacle {bot_version}, source at {SOURCE_URL}.",
            provenance.wowsunpack, provenance.wows_data_mgr
        ),
        WARGAMING_NOTICE.to_owned(),
    ]
    .join("\n\n")
}

pub fn listing(items: &[String], limit: usize) -> String {
    if items.is_empty() {
        return "None".to_owned();
    }
    (0..=items.len())
        .rev()
        .map(|shown| {
            let hidden = items.len() - shown;
            let head = items[..shown].join(", ");
            match (shown, hidden) {
                (_, 0) => head,
                (0, _) => format!("{hidden} more"),
                _ => format!("{head} and {hidden} more"),
            }
        })
        .find(|text| text.chars().count() <= limit)
        .unwrap_or_default()
}

fn sentence(text: &str) -> String {
    if text.ends_with('.') {
        text.to_owned()
    } else {
        format!("{text}.")
    }
}
```

Whole file `crates/barnacle-bot/src/lib.rs`:

```rust
pub mod config;
pub mod ids;
pub mod solves;
pub mod text;
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p barnacle-bot && cargo clippy -p barnacle-bot --all-targets -- -D warnings`
Expected: `text` has 11 passing tests, the earlier targets still pass, and clippy reports no warnings.

- [ ] **Step 5: Commit**

```bash
git add crates/barnacle-bot/src/text.rs crates/barnacle-bot/src/lib.rs crates/barnacle-bot/tests/text.rs
git commit -m "feat(bot): add the bot's wording and labels" -- crates/barnacle-bot/src/text.rs crates/barnacle-bot/src/lib.rs crates/barnacle-bot/tests/text.rs
```

---

### Task 6: Ship lookup, the ship card and command wiring

**Files:**
- Create: `crates/barnacle-bot/src/lookup.rs`, `crates/barnacle-bot/src/info.rs`, `crates/barnacle-bot/src/wiring.rs`
- Modify: `crates/barnacle-bot/src/lib.rs`
- Test: `crates/barnacle-bot/tests/lookup.rs`, `crates/barnacle-bot/tests/info.rs`, `crates/barnacle-bot/tests/wiring.rs`

**Interfaces:**
- Consumes:
  - From `barnacle-catalog`: `clean_answer` and `Curated`, including `Removal`'s `Display` and `base()`.
  - From Task 2: `ShipBook::names` and `lookalikes`.
  - From Task 5: `text::{tier_numeral, nation_label, class_label, listing, FIELD_LIMIT}`.
- Produces:
  - **Lookup:**
    - `Directory::new(&Catalog)` covers every ship with an English name.
    - `suggest(&str) -> Vec<Suggestion { label, index }>` returns at most `MAX_SUGGESTIONS = 25`, prefix matches first, alphabetical.
    - `resolve(&str) -> Option<&ShipIndex>` tries an index in any case first, then an exact cleaned name.
  - **Ship card:** `info::ship_card(&Catalog, &Curated, &ShipBook, &ShipIndex) -> Option<ShipCard { title, fields: Vec<(String, String)> }>`.
  - **Wiring:** `wiring::round_options(Option<i64>, Option<i64>, Option<bool>) -> RoundOptions`, `cancel_button_id(u64) -> String`, `round_number(&str) -> Option<u64>` and `ENDED_BUTTON_ID`.

- [ ] **Step 1: Write the failing tests**

Whole file `crates/barnacle-bot/tests/lookup.rs`:

```rust
mod common;

use barnacle_bot::lookup::Directory;
use barnacle_bot::lookup::MAX_SUGGESTIONS;
use barnacle_catalog::Ship;
use barnacle_catalog::ShipName;
use common::catalog;
use common::fleet;
use common::index;
use common::ship;

fn directory() -> Directory {
    let konig = Ship {
        name: Some(ShipName {
            short: "König".to_owned(),
            full: Some("König Albert".to_owned()),
        }),
        ..ship("PGSB105", "König", 5, "konig")
    };
    let nameless = Ship {
        name: None,
        ..ship("PXSX999", "unused", 5, "nameless")
    };
    Directory::new(&catalog(vec![
        ship("PBSC507", "Belfast", 7, "belfast"),
        ship("PBSC528", "Belfast '43", 8, "belfast-43"),
        ship("PJSB018", "Yamato", 10, "yamato"),
        ship("PASB510", "Montana B", 10, "montana"),
        konig,
        nameless,
    ]))
}

fn indexes(typed: &str) -> Vec<String> {
    directory()
        .suggest(typed)
        .into_iter()
        .map(|suggestion| suggestion.index.to_string())
        .collect()
}

#[test]
fn names_starting_with_the_text_come_before_names_containing_it() {
    let directory = Directory::new(&catalog(vec![
        ship("PASB001", "Westfast", 5, "a"),
        ship("PBSC507", "Belfast", 7, "b"),
        ship("PASB002", "Fastback", 5, "c"),
    ]));
    let labels: Vec<String> = directory
        .suggest("fast")
        .into_iter()
        .map(|suggestion| suggestion.label)
        .collect();
    assert_eq!(
        labels,
        [
            "Fastback (V, Japan)",
            "Belfast (VII, Japan)",
            "Westfast (V, Japan)"
        ]
    );
}

#[test]
fn matching_ignores_accents_case_and_punctuation() {
    assert_eq!(indexes("konig"), ["PGSB105"]);
    assert_eq!(indexes("KÖNIG AL"), ["PGSB105"]);
    assert_eq!(indexes("belfast 43"), ["PBSC528"]);
}

#[test]
fn excluded_ships_are_suggested_and_nameless_ships_are_not() {
    assert_eq!(indexes("montana"), ["PASB510"]);
    assert_eq!(indexes("").len(), 5);
    assert!(!indexes("").contains(&"PXSX999".to_owned()));
}

#[test]
fn at_most_twenty_five_suggestions_are_returned() {
    let directory = Directory::new(&catalog(fleet(30)));
    assert_eq!(directory.suggest("hull").len(), MAX_SUGGESTIONS);
    assert_eq!(MAX_SUGGESTIONS, 25);
}

#[test]
fn typed_text_resolves_as_an_id_then_as_an_exact_name() {
    let directory = directory();
    assert_eq!(directory.resolve("PJSB018"), Some(&index("PJSB018")));
    assert_eq!(directory.resolve(" pjsb018 "), Some(&index("PJSB018")));
    assert_eq!(directory.resolve("Belfast '43"), Some(&index("PBSC528")));
    assert_eq!(directory.resolve("konig albert"), Some(&index("PGSB105")));
    assert_eq!(directory.resolve("belf"), None);
    assert_eq!(directory.resolve("PXSX999"), None);
    assert_eq!(directory.resolve("..."), None);
}
```

Whole file `crates/barnacle-bot/tests/info.rs`:

```rust
mod common;

use barnacle_bot::info::ship_card;
use barnacle_catalog::curation::curate;
use barnacle_guess::ShipBook;
use common::catalog;
use common::curation;
use common::index;
use common::ship;

fn field<'a>(card: &'a barnacle_bot::info::ShipCard, name: &str) -> &'a str {
    card.fields
        .iter()
        .find(|(field, _)| field == name)
        .map(|(_, value)| value.as_str())
        .unwrap()
}

#[test]
fn a_pool_ship_shows_its_details_answers_and_lookalikes() {
    let catalog = catalog(vec![
        ship("PJSB018", "Yamato", 10, "yamato"),
        ship("PJSB518", "Yamato B", 10, "yamato"),
        ship("PJSB019", "Musashi", 9, "musashi"),
    ]);
    let config = curation("[[lookalikes]]\nships = [\"PJSB018\", \"PJSB019\"]\n");
    let curated = curate(&catalog, &config);
    let book = ShipBook::new(&catalog, &config);
    let card = ship_card(&catalog, &curated, &book, &index("PJSB018")).unwrap();
    assert_eq!(card.title, "Yamato");
    assert_eq!(
        card.fields
            .iter()
            .map(|(name, _)| name.as_str())
            .collect::<Vec<_>>(),
        [
            "ID",
            "Tier",
            "Class",
            "Nation",
            "Group",
            "Paper ship",
            "In /guess",
            "Accepted answers",
            "Look-alikes"
        ]
    );
    assert_eq!(field(&card, "ID"), "PJSB018");
    assert_eq!(field(&card, "Tier"), "X");
    assert_eq!(field(&card, "Class"), "battleship");
    assert_eq!(field(&card, "Nation"), "Japan");
    assert_eq!(field(&card, "Group"), "upgradeable");
    assert_eq!(field(&card, "Paper ship"), "No");
    assert_eq!(field(&card, "In /guess"), "Yes");
    assert_eq!(field(&card, "Accepted answers"), "yamato, yamatob");
    assert_eq!(field(&card, "Look-alikes"), "Musashi");
}

#[test]
fn an_excluded_ship_says_why_and_names_its_base() {
    let catalog = catalog(vec![
        ship("PJSB018", "Yamato", 10, "yamato"),
        ship("PJSB518", "Yamato B", 10, "yamato"),
    ]);
    let config = curation("");
    let curated = curate(&catalog, &config);
    let book = ShipBook::new(&catalog, &config);
    let card = ship_card(&catalog, &curated, &book, &index("PJSB518")).unwrap();
    assert_eq!(
        field(&card, "In /guess"),
        "No: same silhouette as PJSB018 (Yamato)"
    );
    assert_eq!(field(&card, "Accepted answers"), "None");
    assert_eq!(field(&card, "Look-alikes"), "None");
    assert!(ship_card(&catalog, &curated, &book, &index("PZSX999")).is_none());
}
```

Whole file `crates/barnacle-bot/tests/wiring.rs`:

```rust
mod common;

use barnacle_bot::wiring;
use barnacle_guess::RoundOptions;
use common::tier;

#[test]
fn command_options_become_round_options() {
    assert_eq!(
        wiring::round_options(None, None, None),
        RoundOptions::default()
    );
    assert_eq!(
        wiring::round_options(Some(9), Some(4), Some(true)),
        RoundOptions::new(Some(tier(9)), Some(tier(4)), Some(true))
    );
    assert_eq!(
        wiring::round_options(Some(0), Some(12), None),
        RoundOptions::default()
    );
}

#[test]
fn a_cancel_button_id_carries_its_round_number() {
    assert_eq!(
        wiring::round_number(&wiring::cancel_button_id(42)),
        Some(42)
    );
    assert_eq!(wiring::round_number(wiring::ENDED_BUTTON_ID), None);
    assert_eq!(wiring::round_number("other-button:42"), None);
    assert_eq!(wiring::round_number("barnacle-cancel:-1"), None);
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p barnacle-bot --test lookup --test info --test wiring`
Expected: FAIL to compile with `error[E0432]` for `barnacle_bot::lookup`, `barnacle_bot::info` and `barnacle_bot::wiring`.

- [ ] **Step 3: Implement**

Whole file `crates/barnacle-bot/src/lookup.rs`:

```rust
use std::collections::BTreeMap;

use barnacle_catalog::Catalog;
use barnacle_catalog::ShipIndex;
use barnacle_catalog::names::clean_answer;

use crate::text;

pub const MAX_SUGGESTIONS: usize = 25;
const MAX_LABEL_CHARS: usize = 100;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Suggestion {
    pub label: String,
    pub index: ShipIndex,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Listing {
    index: ShipIndex,
    label: String,
    forms: Vec<String>,
}

impl Listing {
    fn starts_with(&self, wanted: &str) -> bool {
        self.forms.iter().any(|form| form.starts_with(wanted))
    }

    fn contains(&self, wanted: &str) -> bool {
        self.forms.iter().any(|form| form.contains(wanted))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Directory {
    listings: Vec<Listing>,
}

impl Directory {
    pub fn new(catalog: &Catalog) -> Self {
        let ordered: BTreeMap<(String, ShipIndex), Listing> = catalog
            .ships
            .iter()
            .filter_map(|ship| {
                let name = ship.name.as_ref()?;
                let display = name.display().to_owned();
                let label: String = format!(
                    "{display} ({}, {})",
                    text::tier_numeral(ship.tier),
                    text::nation_label(&ship.nation)
                )
                .chars()
                .take(MAX_LABEL_CHARS)
                .collect();
                let forms = std::iter::once(name.short.as_str())
                    .chain(name.full.as_deref())
                    .map(clean_answer)
                    .filter(|form| !form.is_empty())
                    .collect();
                let listing = Listing {
                    index: ship.index.clone(),
                    label,
                    forms,
                };
                Some(((display, ship.index.clone()), listing))
            })
            .collect();
        Self {
            listings: ordered.into_values().collect(),
        }
    }

    pub fn suggest(&self, typed: &str) -> Vec<Suggestion> {
        let wanted = clean_answer(typed);
        let leading = self
            .listings
            .iter()
            .filter(|listing| listing.starts_with(&wanted));
        let inside = self
            .listings
            .iter()
            .filter(|listing| !listing.starts_with(&wanted) && listing.contains(&wanted));
        leading
            .chain(inside)
            .take(MAX_SUGGESTIONS)
            .map(|listing| Suggestion {
                label: listing.label.clone(),
                index: listing.index.clone(),
            })
            .collect()
    }

    pub fn resolve(&self, typed: &str) -> Option<&ShipIndex> {
        let by_index = ShipIndex::parse(&typed.trim().to_ascii_uppercase())
            .ok()
            .and_then(|index| self.listings.iter().find(|listing| listing.index == index));
        let wanted = clean_answer(typed);
        by_index
            .or_else(|| {
                self.listings
                    .iter()
                    .find(|listing| !wanted.is_empty() && listing.forms.contains(&wanted))
            })
            .map(|listing| &listing.index)
    }
}
```

Whole file `crates/barnacle-bot/src/info.rs`:

```rust
use barnacle_catalog::Catalog;
use barnacle_catalog::ShipIndex;
use barnacle_catalog::curation::Curated;
use barnacle_guess::ShipBook;

use crate::text;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShipCard {
    pub title: String,
    pub fields: Vec<(String, String)>,
}

pub fn ship_card(
    catalog: &Catalog,
    curated: &Curated,
    book: &ShipBook,
    index: &ShipIndex,
) -> Option<ShipCard> {
    let ship = catalog.get(index)?;
    let title = ship
        .name
        .as_ref()
        .map(|name| name.display().to_owned())
        .unwrap_or_else(|| index.to_string());
    let name_of = |other: &ShipIndex| {
        catalog
            .get(other)
            .and_then(|found| found.name.as_ref())
            .map(|name| name.display().to_owned())
    };
    let in_guess = match curated.removed.get(index) {
        None => "Yes".to_owned(),
        Some(removal) => {
            let base_name = removal
                .base()
                .and_then(name_of)
                .map(|name| format!(" ({name})"))
                .unwrap_or_default();
            format!("No: {removal}{base_name}")
        }
    };
    let answers: Vec<String> = book.names(index).into_iter().collect();
    let lookalikes: Vec<String> = book
        .lookalikes(index)
        .into_iter()
        .map(|other| name_of(other).unwrap_or_else(|| other.to_string()))
        .collect();
    let paper = if ship.is_paper { "Yes" } else { "No" };
    let fields = vec![
        ("ID".to_owned(), index.to_string()),
        ("Tier".to_owned(), text::tier_numeral(ship.tier).to_owned()),
        ("Class".to_owned(), text::class_label(&ship.class)),
        ("Nation".to_owned(), text::nation_label(&ship.nation)),
        ("Group".to_owned(), ship.group.to_string()),
        ("Paper ship".to_owned(), paper.to_owned()),
        (
            "In /guess".to_owned(),
            text::listing(&[in_guess], text::FIELD_LIMIT),
        ),
        (
            "Accepted answers".to_owned(),
            text::listing(&answers, text::FIELD_LIMIT),
        ),
        (
            "Look-alikes".to_owned(),
            text::listing(&lookalikes, text::FIELD_LIMIT),
        ),
    ];
    Some(ShipCard { title, fields })
}
```

Whole file `crates/barnacle-bot/src/wiring.rs`:

```rust
use barnacle_catalog::Tier;
use barnacle_guess::RoundOptions;

const CANCEL_PREFIX: &str = "barnacle-cancel:";
pub const ENDED_BUTTON_ID: &str = "barnacle-cancel:ended";

pub fn round_options(
    min_tier: Option<i64>,
    max_tier: Option<i64>,
    historical: Option<bool>,
) -> RoundOptions {
    RoundOptions::new(tier(min_tier), tier(max_tier), historical)
}

pub fn cancel_button_id(number: u64) -> String {
    format!("{CANCEL_PREFIX}{number}")
}

pub fn round_number(custom_id: &str) -> Option<u64> {
    custom_id.strip_prefix(CANCEL_PREFIX)?.parse().ok()
}

fn tier(value: Option<i64>) -> Option<Tier> {
    let value = u32::try_from(value?).ok()?;
    Tier::new(value).ok()
}
```

Whole file `crates/barnacle-bot/src/lib.rs`:

```rust
pub mod config;
pub mod ids;
pub mod info;
pub mod lookup;
pub mod solves;
pub mod text;
pub mod wiring;
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p barnacle-bot && cargo clippy -p barnacle-bot --all-targets -- -D warnings`
Expected: `lookup` has 5 passing tests, `info` 2 and `wiring` 2, the earlier targets still pass, and clippy reports no warnings.

- [ ] **Step 5: Commit**

```bash
git add crates/barnacle-bot/src/lookup.rs crates/barnacle-bot/src/info.rs crates/barnacle-bot/src/wiring.rs crates/barnacle-bot/src/lib.rs crates/barnacle-bot/tests/lookup.rs crates/barnacle-bot/tests/info.rs crates/barnacle-bot/tests/wiring.rs
git commit -m "feat(bot): look up ships and build the ship card" -- crates/barnacle-bot/src/lookup.rs crates/barnacle-bot/src/info.rs crates/barnacle-bot/src/wiring.rs crates/barnacle-bot/src/lib.rs crates/barnacle-bot/tests/lookup.rs crates/barnacle-bot/tests/info.rs crates/barnacle-bot/tests/wiring.rs
```

---

### Task 7: The game core

**Files:**
- Create: `crates/barnacle-bot/src/table.rs`, `crates/barnacle-bot/tests/common/fakes.rs`
- Modify: `crates/barnacle-bot/src/lib.rs`, `crates/barnacle-bot/tests/common/mod.rs`
- Test: `crates/barnacle-bot/tests/table.rs`

**Interfaces:**
- Consumes:
  - From `barnacle-guess`: `ShipBook::draw`, `Draw::start`, `Round::{judge, may_cancel, draw, posted}`, `RecentShips::remember`, `Snowflake::unix_millis` and `Timing`.
  - From Task 3: `SolveStore` and `SolveRecord`.
- Produces:
  - **Types:**
    - `AnnounceError(Box<dyn Error + Send + Sync>)`.
    - `Ending::{Solved { solve, reveal, message, personal_best: Option<bool> }, TimedOut { reveal }, Cancelled { by, reveal }}`.
    - `trait Announcer: Send + Sync + 'static` with `post_hint(ChannelId, &Hint)` and `post_ending(ChannelId, round_post: Snowflake, &Ending)`, each returning `impl Future<Output = Result<(), AnnounceError>> + Send`.
    - `StartOutcome<E>::{Started { number }, Busy, NoShips(GameError), PostFailed(E)}` and `CancelOutcome::{Cancelled, Refused, AlreadyOver}`.
  - **`Table<A: Announcer, S: SolveStore>` methods:**
    - `Table::new(ShipBook, S, A, Timing, StdRng) -> Arc<Table>`.
    - `book()` and `solves()`.
    - `start(self: &Arc<Self>, Place, RoundOptions, invoker, post: FnOnce(Draw, u64) -> impl Future<Output = Result<Snowflake, E>>)`.
    - `hear(Place, Guess<'_>)` and `cancel(Place, number, user, can_manage_messages) -> CancelOutcome`.
  - **Test helpers in `common::fakes`:**
    - Fake Discord: `Posted::{Hint, Ending}`, and `FakeDiscord::{new, failing, posted, endings}`, which records each post with the time since the test started.
    - Fake store: `FakeStore::{default, failing, saved}`.
    - Places and setup: `PLACE`, `OTHER_PLACE`, and `table_with(ships, FakeDiscord, FakeStore)`.

- [ ] **Step 1: Write the failing tests**

Whole file `crates/barnacle-bot/tests/common/fakes.rs`:

```rust
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

use barnacle_bot::ids::ChannelId;
use barnacle_bot::ids::GuildId;
use barnacle_bot::ids::Place;
use barnacle_bot::solves::SolveRecord;
use barnacle_bot::solves::SolveStore;
use barnacle_bot::solves::SolvesError;
use barnacle_bot::table::AnnounceError;
use barnacle_bot::table::Announcer;
use barnacle_bot::table::Ending;
use barnacle_bot::table::Table;
use barnacle_catalog::Ship;
use barnacle_guess::Hint;
use barnacle_guess::ShipBook;
use barnacle_guess::Snowflake;
use barnacle_guess::Timing;
use rand::SeedableRng;
use rand::rngs::StdRng;
use tokio::time::Instant;

use super::catalog;
use super::curation;

pub const PLACE: Place = Place {
    guild: GuildId::new(1),
    channel: ChannelId::new(10),
};
pub const OTHER_PLACE: Place = Place {
    guild: GuildId::new(1),
    channel: ChannelId::new(11),
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Posted {
    Hint {
        channel: ChannelId,
        hint: Hint,
    },
    Ending {
        channel: ChannelId,
        round_post: Snowflake,
        ending: Ending,
    },
}

#[derive(Clone)]
pub struct FakeDiscord {
    start: Instant,
    fail: bool,
    posted: Arc<Mutex<Vec<(Duration, Posted)>>>,
}

impl FakeDiscord {
    pub fn new() -> Self {
        Self {
            start: Instant::now(),
            fail: false,
            posted: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn failing() -> Self {
        Self {
            fail: true,
            ..Self::new()
        }
    }

    pub fn posted(&self) -> Vec<(Duration, Posted)> {
        self.posted.lock().unwrap().clone()
    }

    pub fn endings(&self) -> Vec<Ending> {
        self.posted()
            .into_iter()
            .filter_map(|(_, posted)| match posted {
                Posted::Ending { ending, .. } => Some(ending),
                Posted::Hint { .. } => None,
            })
            .collect()
    }

    fn record(&self, posted: Posted) -> Result<(), AnnounceError> {
        self.posted
            .lock()
            .unwrap()
            .push((self.start.elapsed(), posted));
        if self.fail {
            Err(AnnounceError("the fake Discord refuses every post".into()))
        } else {
            Ok(())
        }
    }
}

impl Announcer for FakeDiscord {
    async fn post_hint(&self, channel: ChannelId, hint: &Hint) -> Result<(), AnnounceError> {
        self.record(Posted::Hint {
            channel,
            hint: hint.clone(),
        })
    }

    async fn post_ending(
        &self,
        channel: ChannelId,
        round_post: Snowflake,
        ending: &Ending,
    ) -> Result<(), AnnounceError> {
        self.record(Posted::Ending {
            channel,
            round_post,
            ending: ending.clone(),
        })
    }
}

#[derive(Clone, Default)]
pub struct FakeStore {
    fail: bool,
    saved: Arc<Mutex<Vec<SolveRecord>>>,
}

impl FakeStore {
    pub fn failing() -> Self {
        Self {
            fail: true,
            ..Self::default()
        }
    }

    pub fn saved(&self) -> Vec<SolveRecord> {
        self.saved.lock().unwrap().clone()
    }
}

impl SolveStore for FakeStore {
    async fn record(&self, record: &SolveRecord) -> Result<bool, SolvesError> {
        if self.fail {
            return Err(SolvesError::MissingTable);
        }
        let mut saved = self.saved.lock().unwrap();
        let best = saved
            .iter()
            .filter(|earlier| earlier.guild == record.guild && earlier.user == record.user)
            .map(|earlier| earlier.elapsed)
            .min();
        saved.push(record.clone());
        Ok(best.is_none_or(|best| record.elapsed < best))
    }
}

pub fn table_with(
    ships: Vec<Ship>,
    discord: FakeDiscord,
    store: FakeStore,
) -> Arc<Table<FakeDiscord, FakeStore>> {
    let book = ShipBook::new(&catalog(ships), &curation(""));
    Table::new(
        book,
        store,
        discord,
        Timing::STANDARD,
        StdRng::seed_from_u64(7),
    )
}
```

Whole file `crates/barnacle-bot/tests/common/mod.rs`:

```rust
#![allow(dead_code)]

pub mod fakes;

use barnacle_bot::solves::Solves;
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
use barnacle_guess::Snowflake;
use sqlx::sqlite::SqlitePool;
use sqlx::sqlite::SqlitePoolOptions;

pub const MIGRATION: &str = include_str!("../../../../migrations/0001_guess_solves.sql");
pub const BUILD: u32 = 13187581;

pub fn index(value: &str) -> ShipIndex {
    ShipIndex::parse(value).unwrap()
}

pub fn tier(value: u32) -> Tier {
    Tier::new(value).unwrap()
}

pub fn at(millis: u64) -> Snowflake {
    Snowflake::new(millis << 22)
}

pub fn ship(value: &str, name: &str, tier_value: u32, silhouette: &str) -> Ship {
    Ship {
        id: ParamId::new(1),
        index: index(value),
        tier: tier(tier_value),
        group: ShipGroup::new("upgradeable"),
        class: ShipClass::Battleship,
        nation: Nation::new("Japan"),
        is_paper: false,
        name: Some(ShipName {
            short: name.to_owned(),
            full: None,
        }),
        silhouette: Some(Silhouette {
            sha256: silhouette.to_owned(),
        }),
    }
}

pub fn fleet(size: usize) -> Vec<Ship> {
    (0..size)
        .map(|number| {
            ship(
                &format!("PXSX{number:03}"),
                &format!("Hull {number}"),
                8,
                &format!("silhouette-{number}"),
            )
        })
        .collect()
}

pub fn catalog(ships: Vec<Ship>) -> Catalog {
    Catalog {
        provenance: Provenance {
            game_version: "15.8.0".to_owned(),
            build: BUILD,
            data_repo_commit: "442496ea2f27517507a562f6eb3ceee06003d3da".to_owned(),
            wowsunpack: "0.45.0".to_owned(),
            wows_data_mgr: "0.21.0".to_owned(),
        },
        ships,
    }
}

pub fn curation_text(tables: &str) -> String {
    format!("reviewed_through = {BUILD}\ngroups = [\"upgradeable\"]\n{tables}")
}

pub fn curation(tables: &str) -> CurationConfig {
    CurationConfig::from_toml(&curation_text(tables)).unwrap()
}

pub async fn memory_pool() -> SqlitePool {
    SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap()
}

pub async fn migrated_pool() -> SqlitePool {
    let pool = memory_pool().await;
    sqlx::raw_sql(MIGRATION).execute(&pool).await.unwrap();
    pool
}

pub async fn solves() -> Solves {
    Solves::with_pool(migrated_pool().await).await.unwrap()
}
```

The only change from Task 3's version is the new `pub mod fakes;` line.

Whole file `crates/barnacle-bot/tests/table.rs`:

```rust
mod common;

use std::collections::BTreeSet;
use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;

use barnacle_bot::ids::Place;
use barnacle_bot::table::CancelOutcome;
use barnacle_bot::table::Ending;
use barnacle_bot::table::StartOutcome;
use barnacle_bot::table::Table;
use barnacle_guess::Guess;
use barnacle_guess::Hint;
use barnacle_guess::RoundOptions;
use barnacle_guess::UserId;
use common::at;
use common::fakes::FakeDiscord;
use common::fakes::FakeStore;
use common::fakes::OTHER_PLACE;
use common::fakes::PLACE;
use common::fakes::Posted;
use common::fakes::table_with;
use common::fleet;
use common::index;
use common::ship;
use common::tier;

const INVOKER: UserId = UserId::new(100);
const PLAYER: UserId = UserId::new(200);
const OTHER: UserId = UserId::new(300);
const POSTED_AT: u64 = 1_000;

type TestTable = Arc<Table<FakeDiscord, FakeStore>>;

fn yamato_table(discord: FakeDiscord, store: FakeStore) -> TestTable {
    table_with(
        vec![ship("PJSB018", "Yamato", 10, "yamato")],
        discord,
        store,
    )
}

async fn start(table: &TestTable, place: Place) -> StartOutcome<Infallible> {
    table
        .start(
            place,
            RoundOptions::default(),
            INVOKER,
            |_draw, _number| async { Ok(at(POSTED_AT)) },
        )
        .await
}

async fn started(table: &TestTable, place: Place) -> u64 {
    match start(table, place).await {
        StartOutcome::Started { number } => number,
        other => panic!("the round did not start: {other:?}"),
    }
}

fn guess(author: UserId, author_is_bot: bool, sent_at: u64, text: &str) -> Guess<'_> {
    Guess {
        author,
        author_is_bot,
        message: at(sent_at),
        text,
    }
}

fn seconds(value: u64) -> Duration {
    Duration::from_secs(value)
}

async fn wait(value: u64) {
    tokio::time::sleep(seconds(value)).await;
}

#[tokio::test(start_paused = true)]
async fn an_unanswered_round_posts_the_hint_at_twenty_seconds_and_ends_at_thirty() {
    let discord = FakeDiscord::new();
    let table = yamato_table(discord.clone(), FakeStore::default());
    started(&table, PLACE).await;
    wait(60).await;
    let posted = discord.posted();
    assert_eq!(posted.len(), 2);
    assert_eq!(
        posted[0],
        (
            seconds(20),
            Posted::Hint {
                channel: PLACE.channel,
                hint: Hint::Tier(tier(10)),
            }
        )
    );
    assert_eq!(posted[1].0, seconds(30));
    assert!(matches!(
        &posted[1].1,
        Posted::Ending {
            round_post,
            ending: Ending::TimedOut { reveal },
            ..
        } if *round_post == at(POSTED_AT) && reveal.index == index("PJSB018")
    ));
}

#[tokio::test(start_paused = true)]
async fn a_win_before_the_hint_ends_the_round_and_nothing_else_is_posted() {
    let discord = FakeDiscord::new();
    let table = yamato_table(discord.clone(), FakeStore::default());
    started(&table, PLACE).await;
    wait(5).await;
    table
        .hear(PLACE, guess(PLAYER, false, POSTED_AT + 5_250, "Yamato"))
        .await;
    wait(60).await;
    let posted = discord.posted();
    assert_eq!(posted.len(), 1);
    assert_eq!(posted[0].0, seconds(5));
    assert!(matches!(
        &posted[0].1,
        Posted::Ending {
            ending: Ending::Solved { solve, message, personal_best: Some(true), .. },
            ..
        } if solve.winner == PLAYER
            && solve.elapsed == Duration::from_millis(5_250)
            && *message == at(POSTED_AT + 5_250)
    ));
}

#[tokio::test(start_paused = true)]
async fn a_win_after_the_hint_follows_the_hint() {
    let discord = FakeDiscord::new();
    let table = yamato_table(discord.clone(), FakeStore::default());
    started(&table, PLACE).await;
    wait(25).await;
    table
        .hear(PLACE, guess(PLAYER, false, POSTED_AT + 25_000, "yamato"))
        .await;
    wait(60).await;
    let posted = discord.posted();
    assert_eq!(posted.len(), 2);
    assert!(matches!(posted[0].1, Posted::Hint { .. }));
    assert_eq!(posted[1].0, seconds(25));
    assert!(matches!(
        posted[1].1,
        Posted::Ending {
            ending: Ending::Solved { .. },
            ..
        }
    ));
}

#[tokio::test(start_paused = true)]
async fn a_busy_channel_refuses_a_second_round_until_the_first_ends() {
    let table = yamato_table(FakeDiscord::new(), FakeStore::default());
    started(&table, PLACE).await;
    assert!(matches!(start(&table, PLACE).await, StartOutcome::Busy));
    table
        .hear(PLACE, guess(PLAYER, false, POSTED_AT + 1, "yamato"))
        .await;
    assert!(matches!(
        start(&table, PLACE).await,
        StartOutcome::Started { .. }
    ));
}

#[tokio::test(start_paused = true)]
async fn the_starter_may_cancel() {
    let discord = FakeDiscord::new();
    let table = yamato_table(discord.clone(), FakeStore::default());
    let number = started(&table, PLACE).await;
    assert_eq!(
        table.cancel(PLACE, number, INVOKER, false).await,
        CancelOutcome::Cancelled
    );
    assert!(matches!(
        discord.endings().as_slice(),
        [Ending::Cancelled { by, .. }] if *by == INVOKER
    ));
}

#[tokio::test(start_paused = true)]
async fn a_moderator_may_cancel_but_another_player_may_not() {
    let discord = FakeDiscord::new();
    let table = yamato_table(discord.clone(), FakeStore::default());
    let number = started(&table, PLACE).await;
    assert_eq!(
        table.cancel(PLACE, number, OTHER, false).await,
        CancelOutcome::Refused
    );
    assert!(discord.endings().is_empty());
    assert_eq!(
        table.cancel(PLACE, number, OTHER, true).await,
        CancelOutcome::Cancelled
    );
    assert!(matches!(
        discord.endings().as_slice(),
        [Ending::Cancelled { by, .. }] if *by == OTHER
    ));
}

#[tokio::test(start_paused = true)]
async fn a_click_on_a_round_that_is_not_running_is_already_over() {
    let table = yamato_table(FakeDiscord::new(), FakeStore::default());
    assert_eq!(
        table.cancel(PLACE, 1, INVOKER, true).await,
        CancelOutcome::AlreadyOver
    );
    let number = started(&table, PLACE).await;
    assert_eq!(
        table.cancel(PLACE, number + 1, INVOKER, true).await,
        CancelOutcome::AlreadyOver
    );
    assert_eq!(
        table.cancel(PLACE, number, INVOKER, false).await,
        CancelOutcome::Cancelled
    );
    assert_eq!(
        table.cancel(PLACE, number, INVOKER, false).await,
        CancelOutcome::AlreadyOver
    );
}

#[tokio::test(start_paused = true)]
async fn bots_early_messages_and_wrong_guesses_do_not_end_the_round() {
    let discord = FakeDiscord::new();
    let table = yamato_table(discord.clone(), FakeStore::default());
    started(&table, PLACE).await;
    table
        .hear(PLACE, guess(PLAYER, true, POSTED_AT + 1, "yamato"))
        .await;
    table
        .hear(PLACE, guess(PLAYER, false, POSTED_AT - 1, "yamato"))
        .await;
    table
        .hear(PLACE, guess(PLAYER, false, POSTED_AT + 2, "musashi"))
        .await;
    wait(60).await;
    assert!(matches!(
        discord.endings().as_slice(),
        [Ending::TimedOut { .. }]
    ));
}

#[tokio::test(start_paused = true)]
async fn two_channels_run_their_rounds_independently() {
    let discord = FakeDiscord::new();
    let table = yamato_table(discord.clone(), FakeStore::default());
    started(&table, PLACE).await;
    started(&table, OTHER_PLACE).await;
    table
        .hear(PLACE, guess(PLAYER, false, POSTED_AT + 1, "yamato"))
        .await;
    wait(60).await;
    let for_channel = |place: Place| {
        discord
            .posted()
            .into_iter()
            .filter(move |(_, posted)| match posted {
                Posted::Hint { channel, .. } | Posted::Ending { channel, .. } => {
                    *channel == place.channel
                }
            })
            .count()
    };
    assert_eq!(for_channel(PLACE), 1);
    assert_eq!(for_channel(OTHER_PLACE), 2);
}

#[tokio::test(start_paused = true)]
async fn a_win_and_a_cancel_at_the_same_moment_end_the_round_once() {
    let discord = FakeDiscord::new();
    let table = yamato_table(discord.clone(), FakeStore::default());
    let number = started(&table, PLACE).await;
    let (_, cancelled) = tokio::join!(
        table.hear(PLACE, guess(PLAYER, false, POSTED_AT + 1, "yamato")),
        table.cancel(PLACE, number, INVOKER, false)
    );
    wait(60).await;
    assert_eq!(discord.endings().len(), 1);
    assert_eq!(
        cancelled == CancelOutcome::Cancelled,
        matches!(discord.endings()[0], Ending::Cancelled { .. })
    );
}

#[tokio::test(start_paused = true)]
async fn a_failed_round_post_leaves_the_channel_free() {
    let discord = FakeDiscord::new();
    let table = yamato_table(discord.clone(), FakeStore::default());
    let failed = table
        .start(
            PLACE,
            RoundOptions::default(),
            INVOKER,
            |_draw, _number| async { Err::<barnacle_guess::Snowflake, &str>("upload failed") },
        )
        .await;
    assert!(matches!(failed, StartOutcome::PostFailed("upload failed")));
    wait(60).await;
    assert!(discord.posted().is_empty());
    assert!(matches!(
        start(&table, PLACE).await,
        StartOutcome::Started { .. }
    ));
}

#[tokio::test(start_paused = true)]
async fn an_empty_pool_starts_nothing() {
    let table = yamato_table(FakeDiscord::new(), FakeStore::default());
    let options = RoundOptions::new(Some(tier(1)), Some(tier(2)), None);
    let outcome = table
        .start(PLACE, options, INVOKER, |_draw, _number| async {
            Ok::<_, Infallible>(at(POSTED_AT))
        })
        .await;
    assert!(matches!(outcome, StartOutcome::NoShips(_)));
    assert!(matches!(
        start(&table, PLACE).await,
        StartOutcome::Started { .. }
    ));
}

#[tokio::test(start_paused = true)]
async fn a_channel_does_not_repeat_its_recent_ships() {
    let table = table_with(fleet(21), FakeDiscord::new(), FakeStore::default());
    let drawn = std::sync::Mutex::new(Vec::new());
    for _ in 0..21 {
        let outcome = table
            .start(PLACE, RoundOptions::default(), INVOKER, |draw, _number| {
                drawn.lock().unwrap().push(draw.ship().clone());
                async { Ok::<_, Infallible>(at(POSTED_AT)) }
            })
            .await;
        let StartOutcome::Started { number } = outcome else {
            panic!("the round did not start");
        };
        table.cancel(PLACE, number, INVOKER, false).await;
    }
    let drawn = drawn.into_inner().unwrap();
    assert_eq!(drawn.iter().collect::<BTreeSet<_>>().len(), 21);
}

#[tokio::test(start_paused = true)]
async fn a_win_is_recorded_with_its_server_player_ship_and_time() {
    let store = FakeStore::default();
    let table = yamato_table(FakeDiscord::new(), store.clone());
    started(&table, PLACE).await;
    table
        .hear(PLACE, guess(PLAYER, false, POSTED_AT + 4_000, "yamato"))
        .await;
    assert_eq!(
        store.saved(),
        [barnacle_bot::solves::SolveRecord {
            guild: PLACE.guild,
            user: PLAYER,
            ship: index("PJSB018"),
            elapsed: Duration::from_millis(4_000),
            solved_at_ms: at(POSTED_AT + 4_000).unix_millis(),
        }]
    );
}

#[tokio::test(start_paused = true)]
async fn a_failing_discord_does_not_stop_rounds_from_ending_and_being_recorded() {
    let store = FakeStore::default();
    let table = yamato_table(FakeDiscord::failing(), store.clone());
    started(&table, PLACE).await;
    table
        .hear(PLACE, guess(PLAYER, false, POSTED_AT + 1, "yamato"))
        .await;
    assert!(matches!(
        start(&table, PLACE).await,
        StartOutcome::Started { .. }
    ));
    assert_eq!(store.saved().len(), 1);
}

#[tokio::test(start_paused = true)]
async fn a_storage_failure_still_announces_the_win_without_a_best() {
    let discord = FakeDiscord::new();
    let table = yamato_table(discord.clone(), FakeStore::failing());
    started(&table, PLACE).await;
    table
        .hear(PLACE, guess(PLAYER, false, POSTED_AT + 1, "yamato"))
        .await;
    assert!(matches!(
        discord.endings().as_slice(),
        [Ending::Solved {
            personal_best: None,
            ..
        }]
    ));
}

#[tokio::test(start_paused = true)]
async fn an_earlier_rounds_timer_does_not_hint_at_the_next_round() {
    let discord = FakeDiscord::new();
    let table = yamato_table(discord.clone(), FakeStore::default());
    let first = started(&table, PLACE).await;
    wait(5).await;
    table.cancel(PLACE, first, INVOKER, false).await;
    wait(5).await;
    started(&table, PLACE).await;
    wait(60).await;
    let times: Vec<(Duration, bool)> = discord
        .posted()
        .into_iter()
        .map(|(time, posted)| (time, matches!(posted, Posted::Hint { .. })))
        .collect();
    assert_eq!(
        times,
        [
            (seconds(5), false),
            (seconds(30), true),
            (seconds(40), false)
        ]
    );
}

#[tokio::test(start_paused = true)]
async fn an_earlier_rounds_timer_does_not_end_the_next_round() {
    let discord = FakeDiscord::new();
    let table = yamato_table(discord.clone(), FakeStore::default());
    let first = started(&table, PLACE).await;
    wait(25).await;
    table.cancel(PLACE, first, INVOKER, false).await;
    wait(1).await;
    started(&table, PLACE).await;
    wait(9).await;
    assert_eq!(discord.endings().len(), 1);
    wait(60).await;
    let endings: Vec<Duration> = discord
        .posted()
        .into_iter()
        .filter(|(_, posted)| matches!(posted, Posted::Ending { .. }))
        .map(|(time, _)| time)
        .collect();
    assert_eq!(endings, [seconds(25), seconds(56)]);
}
```

What the round tests check:
- **Paused clock:** every round test runs with `start_paused = true`, and times are exact because the clock only moves when every task is waiting.
- **Stale timers:** the two "earlier round's timer" tests are the ones that fail if a timer stops checking its round number.
- **Why the fake store:** these tests use `FakeStore`, because SQLite under a paused clock would let time jump while a query runs.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p barnacle-bot --test table`
Expected: FAIL to compile with `error[E0432]: unresolved import `barnacle_bot::table``, raised from `tests/common/fakes.rs`. Every other bot test target also fails to compile until Step 3, since they share `common`.

- [ ] **Step 3: Implement**

Whole file `crates/barnacle-bot/src/table.rs`:

```rust
use std::collections::HashMap;
use std::future::Future;
use std::sync::Arc;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;

use barnacle_guess::Draw;
use barnacle_guess::GameError;
use barnacle_guess::Guess;
use barnacle_guess::Hint;
use barnacle_guess::RecentShips;
use barnacle_guess::Reveal;
use barnacle_guess::Round;
use barnacle_guess::RoundOptions;
use barnacle_guess::ShipBook;
use barnacle_guess::Snowflake;
use barnacle_guess::Solve;
use barnacle_guess::Timing;
use barnacle_guess::UserId;
use rand::rngs::StdRng;
use tokio::sync::Mutex;

use crate::ids::ChannelId;
use crate::ids::Place;
use crate::solves::SolveRecord;
use crate::solves::SolveStore;

#[derive(Debug, thiserror::Error)]
#[error("the Discord call failed")]
pub struct AnnounceError(#[source] pub Box<dyn std::error::Error + Send + Sync>);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ending {
    Solved {
        solve: Solve,
        reveal: Reveal,
        message: Snowflake,
        personal_best: Option<bool>,
    },
    TimedOut {
        reveal: Reveal,
    },
    Cancelled {
        by: UserId,
        reveal: Reveal,
    },
}

pub trait Announcer: Send + Sync + 'static {
    fn post_hint(
        &self,
        channel: ChannelId,
        hint: &Hint,
    ) -> impl Future<Output = Result<(), AnnounceError>> + Send;

    fn post_ending(
        &self,
        channel: ChannelId,
        round_post: Snowflake,
        ending: &Ending,
    ) -> impl Future<Output = Result<(), AnnounceError>> + Send;
}

#[derive(Debug)]
pub enum StartOutcome<E> {
    Started { number: u64 },
    Busy,
    NoShips(GameError),
    PostFailed(E),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CancelOutcome {
    Cancelled,
    Refused,
    AlreadyOver,
}

struct Active {
    number: u64,
    round: Round,
}

#[derive(Default)]
struct Seat {
    recent: RecentShips,
    active: Option<Active>,
}

pub struct Table<A, S> {
    book: ShipBook,
    solves: S,
    announcer: A,
    timing: Timing,
    rng: std::sync::Mutex<StdRng>,
    seats: std::sync::Mutex<HashMap<ChannelId, Arc<Mutex<Seat>>>>,
    next_number: AtomicU64,
}

impl<A: Announcer, S: SolveStore> Table<A, S> {
    pub fn new(book: ShipBook, solves: S, announcer: A, timing: Timing, rng: StdRng) -> Arc<Self> {
        Arc::new(Self {
            book,
            solves,
            announcer,
            timing,
            rng: std::sync::Mutex::new(rng),
            seats: std::sync::Mutex::new(HashMap::new()),
            next_number: AtomicU64::new(1),
        })
    }

    pub fn book(&self) -> &ShipBook {
        &self.book
    }

    pub fn solves(&self) -> &S {
        &self.solves
    }

    pub async fn start<P, F, E>(
        self: &Arc<Self>,
        place: Place,
        options: RoundOptions,
        invoker: UserId,
        post: P,
    ) -> StartOutcome<E>
    where
        P: FnOnce(Draw, u64) -> F,
        F: Future<Output = Result<Snowflake, E>>,
    {
        let seat = self.seat(place.channel);
        let mut seat = seat.lock().await;
        if seat.active.is_some() {
            return StartOutcome::Busy;
        }
        let draw = match self.draw(&options, &seat.recent) {
            Ok(draw) => draw,
            Err(error) => return StartOutcome::NoShips(error),
        };
        let number = self.next_number.fetch_add(1, Ordering::Relaxed);
        let posted = match post(draw.clone(), number).await {
            Ok(posted) => posted,
            Err(error) => return StartOutcome::PostFailed(error),
        };
        *seat = Seat {
            recent: seat.recent.remember(draw.ship().clone()),
            active: Some(Active {
                number,
                round: draw.start(invoker, posted),
            }),
        };
        drop(seat);
        tracing::info!(channel = place.channel.get(), number, "round started");
        self.spawn_timer(place, number);
        StartOutcome::Started { number }
    }

    pub async fn hear(&self, place: Place, guess: Guess<'_>) {
        if guess.author_is_bot {
            return;
        }
        let Some(seat) = self.existing_seat(place.channel) else {
            return;
        };
        let mut seat = seat.lock().await;
        let solved = seat
            .active
            .as_ref()
            .and_then(|active| active.round.judge(&guess));
        let Some(solve) = solved else {
            return;
        };
        let Some(active) = seat.active.take() else {
            return;
        };
        let record = SolveRecord {
            guild: place.guild,
            user: solve.winner,
            ship: solve.ship.clone(),
            elapsed: solve.elapsed,
            solved_at_ms: guess.message.unix_millis(),
        };
        let personal_best = match self.solves.record(&record).await {
            Ok(best) => Some(best),
            Err(error) => {
                tracing::error!(%error, "a solve could not be recorded");
                None
            }
        };
        let ending = Ending::Solved {
            reveal: active.round.draw().reveal().clone(),
            solve,
            message: guess.message,
            personal_best,
        };
        self.announce_ending(place.channel, &active, &ending).await;
    }

    pub async fn cancel(
        &self,
        place: Place,
        number: u64,
        user: UserId,
        can_manage_messages: bool,
    ) -> CancelOutcome {
        let Some(seat) = self.existing_seat(place.channel) else {
            return CancelOutcome::AlreadyOver;
        };
        let mut seat = seat.lock().await;
        let allowed = match &seat.active {
            Some(active) if active.number == number => {
                active.round.may_cancel(user, can_manage_messages)
            }
            Some(_) | None => return CancelOutcome::AlreadyOver,
        };
        if !allowed {
            return CancelOutcome::Refused;
        }
        let Some(active) = seat.active.take() else {
            return CancelOutcome::AlreadyOver;
        };
        let ending = Ending::Cancelled {
            by: user,
            reveal: active.round.draw().reveal().clone(),
        };
        self.announce_ending(place.channel, &active, &ending).await;
        CancelOutcome::Cancelled
    }

    fn draw(&self, options: &RoundOptions, recent: &RecentShips) -> Result<Draw, GameError> {
        let mut rng = self
            .rng
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        self.book.draw(options, recent, &mut *rng)
    }

    fn seat(&self, channel: ChannelId) -> Arc<Mutex<Seat>> {
        let mut seats = self
            .seats
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        Arc::clone(seats.entry(channel).or_default())
    }

    fn existing_seat(&self, channel: ChannelId) -> Option<Arc<Mutex<Seat>>> {
        self.seats
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(&channel)
            .map(Arc::clone)
    }

    fn spawn_timer(self: &Arc<Self>, place: Place, number: u64) {
        let table = Arc::clone(self);
        tokio::spawn(async move { table.run_timer(place.channel, number).await });
    }

    async fn run_timer(&self, channel: ChannelId, number: u64) {
        tokio::time::sleep(self.timing.before_hint).await;
        if !self.post_hint(channel, number).await {
            return;
        }
        tokio::time::sleep(self.timing.after_hint).await;
        self.time_out(channel, number).await;
    }

    async fn post_hint(&self, channel: ChannelId, number: u64) -> bool {
        let Some(seat) = self.existing_seat(channel) else {
            return false;
        };
        let seat = seat.lock().await;
        let Some(active) = seat
            .active
            .as_ref()
            .filter(|active| active.number == number)
        else {
            return false;
        };
        if let Err(error) = self
            .announcer
            .post_hint(channel, active.round.draw().hint())
            .await
        {
            tracing::error!(%error, "a hint could not be posted");
        }
        true
    }

    async fn time_out(&self, channel: ChannelId, number: u64) {
        let Some(seat) = self.existing_seat(channel) else {
            return;
        };
        let mut seat = seat.lock().await;
        if seat
            .active
            .as_ref()
            .is_none_or(|active| active.number != number)
        {
            return;
        }
        let Some(active) = seat.active.take() else {
            return;
        };
        let ending = Ending::TimedOut {
            reveal: active.round.draw().reveal().clone(),
        };
        self.announce_ending(channel, &active, &ending).await;
    }

    async fn announce_ending(&self, channel: ChannelId, active: &Active, ending: &Ending) {
        tracing::info!(
            channel = channel.get(),
            number = active.number,
            "round ended"
        );
        if let Err(error) = self
            .announcer
            .post_ending(channel, active.round.posted(), ending)
            .await
        {
            tracing::error!(%error, "a round ending could not be posted");
        }
    }
}
```

Whole file `crates/barnacle-bot/src/lib.rs`:

```rust
pub mod config;
pub mod ids;
pub mod info;
pub mod lookup;
pub mod solves;
pub mod table;
pub mod text;
pub mod wiring;
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p barnacle-bot && cargo clippy -p barnacle-bot --all-targets -- -D warnings`
Expected: `table` has 18 passing tests, every earlier bot target still passes, and clippy reports no warnings.

- [ ] **Step 5: Commit**

```bash
git add crates/barnacle-bot/src/table.rs crates/barnacle-bot/src/lib.rs crates/barnacle-bot/tests/common/mod.rs crates/barnacle-bot/tests/common/fakes.rs crates/barnacle-bot/tests/table.rs
git commit -m "feat(bot): run rounds behind a Discord interface" -- crates/barnacle-bot/src/table.rs crates/barnacle-bot/src/lib.rs crates/barnacle-bot/tests/common/mod.rs crates/barnacle-bot/tests/common/fakes.rs crates/barnacle-bot/tests/table.rs
```

---

### Task 8: Startup checks

**Files:**
- Create: `crates/barnacle-bot/src/startup.rs`
- Modify: `crates/barnacle-bot/src/lib.rs`
- Test: `crates/barnacle-bot/tests/startup.rs`

**Interfaces:**
- Consumes:
  - From `barnacle-catalog`: `CatalogRoot`, `curate`, `validate` and `Problem`.
  - `Config` (Task 4), `Directory` (Task 6) and `Solves` (Task 3).
- Produces:
  - `StartupError::{ConfigIo, Config, MissingToken, NoCurrentCatalog, Catalog, CurationIo, CurationToml, Curation { problems }, MissingSilhouettes { dir, missing }, Database { path, source }}`.
  - `Loaded { root, catalog_name, catalog, curated, book, directory }`.
  - Functions:
    - `read_config(&Path)` and `read_token(Option<String>)`, which trims the token and refuses a blank one.
    - `load(&Config) -> Result<Loaded, StartupError>`.
    - `open_solves(&Path)`.
    - `describe(&dyn Error) -> String`, which lists curation problems or missing silhouettes and then each cause.

- [ ] **Step 1: Write the failing tests**

Whole file `crates/barnacle-bot/tests/startup.rs`:

```rust
mod common;

use std::path::Path;

use barnacle_bot::config::CommandScope;
use barnacle_bot::config::Config;
use barnacle_bot::startup;
use barnacle_bot::startup::StartupError;
use barnacle_catalog::Catalog;
use common::catalog;
use common::curation_text;
use common::index;
use common::ship;

const NAME: &str = "15.8.0_13187581_r4";

struct Layout {
    _dir: tempfile::TempDir,
    config: Config,
}

fn layout(catalog: &Catalog, silhouettes: &[&str], curation: &str) -> Layout {
    let dir = tempfile::tempdir().unwrap();
    let catalogs = dir.path().join("data").join("catalog");
    let silhouette_dir = catalogs.join(NAME).join("silhouettes");
    std::fs::create_dir_all(&silhouette_dir).unwrap();
    std::fs::write(
        catalogs.join(NAME).join("catalog.json"),
        catalog.to_json().unwrap(),
    )
    .unwrap();
    std::fs::write(catalogs.join("current"), format!("{NAME}\n")).unwrap();
    for ship in silhouettes {
        std::fs::write(silhouette_dir.join(format!("{ship}.png")), b"png").unwrap();
    }
    let curation_path = dir.path().join("ships.toml");
    std::fs::write(&curation_path, curation).unwrap();
    let config = Config {
        data_dir: dir.path().join("data"),
        curation: curation_path,
        database: dir.path().join("barnacle.sqlite3"),
        commands: CommandScope::Global,
    };
    Layout { _dir: dir, config }
}

fn yamato_catalog() -> Catalog {
    catalog(vec![
        ship("PJSB018", "Yamato", 10, "yamato"),
        ship("PJSB019", "Musashi", 9, "musashi"),
    ])
}

#[test]
fn a_complete_layout_loads() {
    let layout = layout(
        &yamato_catalog(),
        &["PJSB018", "PJSB019"],
        &curation_text(""),
    );
    let loaded = startup::load(&layout.config).unwrap();
    assert_eq!(loaded.catalog_name, NAME);
    assert_eq!(loaded.catalog, yamato_catalog());
    assert_eq!(loaded.directory.resolve("musashi"), Some(&index("PJSB019")));
    assert_eq!(
        loaded
            .book
            .pool(&barnacle_guess::RoundOptions::default())
            .len(),
        2
    );
}

#[test]
fn no_selected_catalog_is_refused() {
    let layout = layout(
        &yamato_catalog(),
        &["PJSB018", "PJSB019"],
        &curation_text(""),
    );
    std::fs::remove_file(layout.config.catalogs().join("current")).unwrap();
    assert!(matches!(
        startup::load(&layout.config),
        Err(StartupError::NoCurrentCatalog { .. })
    ));
}

#[test]
fn curation_problems_are_refused_and_listed() {
    let layout = layout(
        &yamato_catalog(),
        &["PJSB018", "PJSB019"],
        "reviewed_through = 1\ngroups = [\"upgradeable\"]\n",
    );
    let error = startup::load(&layout.config).err().unwrap();
    assert!(matches!(error, StartupError::Curation { .. }));
    assert_eq!(
        startup::describe(&error),
        "curation has 1 problem(s); run `barnacle-data validate`\n  - curation was reviewed through build 1, but the catalog is build 13187581"
    );
}

#[test]
fn a_missing_silhouette_is_refused_and_named() {
    let layout = layout(&yamato_catalog(), &["PJSB018"], &curation_text(""));
    let error = startup::load(&layout.config).err().unwrap();
    assert!(matches!(
        &error,
        StartupError::MissingSilhouettes { missing, .. } if missing == &[index("PJSB019")]
    ));
    assert!(startup::describe(&error).ends_with("\n  - PJSB019"));
}

#[test]
fn a_missing_or_blank_token_is_refused() {
    assert!(matches!(
        startup::read_token(None),
        Err(StartupError::MissingToken)
    ));
    assert!(matches!(
        startup::read_token(Some("  ".to_owned())),
        Err(StartupError::MissingToken)
    ));
    assert_eq!(
        startup::read_token(Some(" secret-token\n".to_owned())).unwrap(),
        "secret-token"
    );
}

#[test]
fn a_missing_config_file_is_refused() {
    assert!(matches!(
        startup::read_config(Path::new("/nonexistent/barnacle.toml")),
        Err(StartupError::ConfigIo { .. })
    ));
}

#[tokio::test]
async fn a_missing_database_explains_how_to_create_it() {
    let layout = layout(
        &yamato_catalog(),
        &["PJSB018", "PJSB019"],
        &curation_text(""),
    );
    let error = startup::open_solves(&layout.config.database)
        .await
        .err()
        .unwrap();
    assert!(matches!(error, StartupError::Database { .. }));
    assert!(error.to_string().contains(&format!(
        "sqlite3 {} < migrations/0001_guess_solves.sql",
        layout.config.database.display()
    )));
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p barnacle-bot --test startup`
Expected: FAIL to compile with `error[E0432]: unresolved import `barnacle_bot::startup``.

- [ ] **Step 3: Implement**

Whole file `crates/barnacle-bot/src/startup.rs`:

```rust
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
```

Whole file `crates/barnacle-bot/src/lib.rs`:

```rust
pub mod config;
pub mod ids;
pub mod info;
pub mod lookup;
pub mod solves;
pub mod startup;
pub mod table;
pub mod text;
pub mod wiring;
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p barnacle-bot && cargo clippy -p barnacle-bot --all-targets -- -D warnings`
Expected: `startup` has 7 passing tests, every earlier bot target still passes, and clippy reports no warnings.

- [ ] **Step 5: Commit**

```bash
git add crates/barnacle-bot/src/startup.rs crates/barnacle-bot/src/lib.rs crates/barnacle-bot/tests/startup.rs
git commit -m "feat(bot): check everything before connecting" -- crates/barnacle-bot/src/startup.rs crates/barnacle-bot/src/lib.rs crates/barnacle-bot/tests/startup.rs
```

---

### Task 9: Discord wiring, the program, and the README

**Files:**
- Create: `crates/barnacle-bot/src/discord.rs`, `crates/barnacle-bot/src/discord/announcer.rs`, `crates/barnacle-bot/src/discord/commands.rs`, `crates/barnacle-bot/src/discord/events.rs`, `crates/barnacle-bot/src/main.rs`
- Modify: `crates/barnacle-bot/src/lib.rs`, `README.md`

**Interfaces:**
- Consumes: everything above.
- Produces:
  - `discord::run(token, CommandScope, Loaded, Solves) -> Result<(), RunError>` and `discord::DiscordAnnouncer`.
  - `RunError::{MessageContentDisabled, Discord}`. `serenity::Error::Gateway(GatewayError::DisallowedGatewayIntents)` maps to `MessageContentDisabled`.
  - The `barnacle-bot` binary with `--config <path>`, defaulting to `barnacle.toml`.

This task has no unit tests. The Discord layer only translates between serenity and the tested core. It is covered by the compile, lint and startup checks below, and by the owner's live checklist in Task 10. The design (section 10) assigns it that coverage.

- [ ] **Step 1: Write the Discord layer and the program**

Whole file `crates/barnacle-bot/src/discord.rs`:

```rust
mod announcer;
mod commands;
mod events;

use std::sync::Arc;

use barnacle_catalog::Catalog;
use barnacle_catalog::curation::Curated;
use barnacle_catalog::store::CatalogRoot;
use barnacle_guess::Timing;
use poise::serenity_prelude as serenity;
use rand::rngs::StdRng;

use crate::config::CommandScope;
use crate::lookup::Directory;
use crate::solves::Solves;
use crate::startup::Loaded;
use crate::table::Table;

pub use announcer::DiscordAnnouncer;

pub type Error = Box<dyn std::error::Error + Send + Sync>;
pub type Context<'a> = poise::Context<'a, Data, Error>;

pub struct Data {
    pub table: Arc<Table<DiscordAnnouncer, Solves>>,
    pub root: CatalogRoot,
    pub catalog_name: String,
    pub catalog: Catalog,
    pub curated: Curated,
    pub directory: Directory,
}

#[derive(Debug, thiserror::Error)]
pub enum RunError {
    #[error(
        "Discord refused the Message Content intent; turn on Message Content Intent under Bot, Privileged Gateway Intents, in the Discord Developer Portal"
    )]
    MessageContentDisabled,
    #[error("the Discord client failed")]
    Discord(#[source] serenity::Error),
}

impl From<serenity::Error> for RunError {
    fn from(error: serenity::Error) -> Self {
        match error {
            serenity::Error::Gateway(serenity::GatewayError::DisallowedGatewayIntents) => {
                Self::MessageContentDisabled
            }
            other => Self::Discord(other),
        }
    }
}

pub async fn run(
    token: String,
    scope: CommandScope,
    loaded: Loaded,
    solves: Solves,
) -> Result<(), RunError> {
    let intents = serenity::GatewayIntents::GUILDS
        | serenity::GatewayIntents::GUILD_MESSAGES
        | serenity::GatewayIntents::MESSAGE_CONTENT;
    let framework = poise::Framework::builder()
        .options(poise::FrameworkOptions {
            commands: commands::all(),
            event_handler: |framework, event| Box::pin(events::handle(framework, event)),
            on_error: |error| Box::pin(events::on_error(error)),
            ..Default::default()
        })
        .setup(move |ctx, _ready, framework| {
            Box::pin(async move {
                register(ctx, &framework.options().commands, &scope).await?;
                let announcer = DiscordAnnouncer::new(Arc::clone(&ctx.http));
                let rng: StdRng = rand::make_rng();
                let table = Table::new(loaded.book, solves, announcer, Timing::STANDARD, rng);
                Ok(Data {
                    table,
                    root: loaded.root,
                    catalog_name: loaded.catalog_name,
                    catalog: loaded.catalog,
                    curated: loaded.curated,
                    directory: loaded.directory,
                })
            })
        })
        .build();
    let mut client = serenity::ClientBuilder::new(token, intents)
        .framework(framework)
        .await?;
    client.start().await?;
    Ok(())
}

async fn register(
    ctx: &serenity::Context,
    commands: &[poise::Command<Data, Error>],
    scope: &CommandScope,
) -> Result<(), Error> {
    match scope {
        CommandScope::Global => {
            poise::builtins::register_globally(ctx, commands).await?;
            tracing::info!("commands registered globally");
        }
        CommandScope::Guilds { guilds } => {
            for guild in guilds {
                poise::builtins::register_in_guild(ctx, commands, serenity::GuildId::new(*guild))
                    .await?;
                tracing::info!(guild, "commands registered in server");
            }
        }
    }
    Ok(())
}
```

Whole file `crates/barnacle-bot/src/discord/announcer.rs`:

```rust
use std::sync::Arc;

use barnacle_guess::Hint;
use barnacle_guess::Snowflake;
use poise::serenity_prelude as serenity;

use crate::ids::ChannelId;
use crate::table::AnnounceError;
use crate::table::Announcer;
use crate::table::Ending;
use crate::text;
use crate::wiring;

pub struct DiscordAnnouncer {
    http: Arc<serenity::Http>,
}

impl DiscordAnnouncer {
    pub fn new(http: Arc<serenity::Http>) -> Self {
        Self { http }
    }
}

pub fn cancel_row(custom_id: String, disabled: bool) -> serenity::CreateActionRow {
    serenity::CreateActionRow::Buttons(vec![
        serenity::CreateButton::new(custom_id)
            .label(text::CANCEL_LABEL)
            .style(serenity::ButtonStyle::Secondary)
            .disabled(disabled),
    ])
}

impl Announcer for DiscordAnnouncer {
    async fn post_hint(&self, channel: ChannelId, hint: &Hint) -> Result<(), AnnounceError> {
        let message = serenity::CreateMessage::new().content(text::hint(hint));
        serenity::ChannelId::new(channel.get())
            .send_message(&self.http, message)
            .await
            .map_err(|error| AnnounceError(Box::new(error)))?;
        Ok(())
    }

    async fn post_ending(
        &self,
        channel: ChannelId,
        round_post: Snowflake,
        ending: &Ending,
    ) -> Result<(), AnnounceError> {
        let channel = serenity::ChannelId::new(channel.get());
        let message = match ending {
            Ending::Solved {
                solve,
                reveal,
                message,
                personal_best,
            } => serenity::CreateMessage::new()
                .content(text::win(reveal, solve.elapsed, *personal_best))
                .reference_message((channel, serenity::MessageId::new(message.get())))
                .allowed_mentions(serenity::CreateAllowedMentions::new().replied_user(true)),
            Ending::TimedOut { reveal } => {
                serenity::CreateMessage::new().content(text::timed_out(reveal))
            }
            Ending::Cancelled { by, reveal } => serenity::CreateMessage::new()
                .content(text::cancelled(*by, reveal))
                .allowed_mentions(serenity::CreateAllowedMentions::new()),
        };
        let disabled = serenity::EditMessage::new()
            .components(vec![cancel_row(wiring::ENDED_BUTTON_ID.to_owned(), true)]);
        let posted = channel.send_message(&self.http, message).await;
        let edited = channel
            .edit_message(
                &self.http,
                serenity::MessageId::new(round_post.get()),
                disabled,
            )
            .await;
        posted.map_err(|error| AnnounceError(Box::new(error)))?;
        edited.map_err(|error| AnnounceError(Box::new(error)))?;
        Ok(())
    }
}
```

Whole file `crates/barnacle-bot/src/discord/commands.rs`:

```rust
use barnacle_guess::Draw;
use barnacle_guess::Snowflake;
use barnacle_guess::Timing;
use barnacle_guess::UserId;
use poise::serenity_prelude as serenity;

use super::Context;
use super::Data;
use super::Error;
use super::announcer::cancel_row;
use crate::ids::ChannelId;
use crate::ids::GuildId;
use crate::ids::Place;
use crate::info;
use crate::table::StartOutcome;
use crate::text;
use crate::wiring;

const SILHOUETTE_FILE: &str = "silhouette.png";

pub fn all() -> Vec<poise::Command<Data, Error>> {
    vec![
        poise::Command {
            description: Some("Start a round: name the ship from its silhouette".into()),
            ..guess()
        },
        poise::Command {
            description: Some("Look up ships".into()),
            subcommands: vec![poise::Command {
                description: Some("Show a ship's details and how /guess treats it".into()),
                ..ship_info()
            }],
            subcommand_required: true,
            ..ship()
        },
        poise::Command {
            description: Some("Show rounds won and best time in this server".into()),
            ..profile()
        },
        poise::Command {
            description: Some("About Barnacle and its ship data".into()),
            ..about()
        },
    ]
}

fn place(ctx: Context<'_>) -> Option<Place> {
    let guild = ctx.guild_id()?;
    Some(Place {
        guild: GuildId::new(guild.get()),
        channel: ChannelId::new(ctx.channel_id().get()),
    })
}

async fn private(ctx: Context<'_>, content: impl Into<String>) -> Result<(), Error> {
    ctx.send(
        poise::CreateReply::default()
            .content(content)
            .ephemeral(true),
    )
    .await?;
    Ok(())
}

fn round_embed(draw: &Draw) -> serenity::CreateEmbed {
    let embed = serenity::CreateEmbed::new()
        .title(text::ROUND_TITLE)
        .description(text::ROUND_DESCRIPTION)
        .colour(text::EMBED_COLOUR)
        .field(text::TIERS_FIELD, text::tier_range(draw.options()), true)
        .image(format!("attachment://{SILHOUETTE_FILE}"))
        .footer(serenity::CreateEmbedFooter::new(text::round_footer(
            Timing::STANDARD,
        )));
    if draw.options().historical() {
        embed.field(text::PAPER_EXCLUDED, "Yes", true)
    } else {
        embed
    }
}

#[poise::command(slash_command, guild_only)]
async fn guess(
    ctx: Context<'_>,
    #[description = "Lowest tier, 1 to 11 (default 6)"]
    #[min = 1]
    #[max = 11]
    min_tier: Option<i64>,
    #[description = "Highest tier, 1 to 11 (default 11)"]
    #[min = 1]
    #[max = 11]
    max_tier: Option<i64>,
    #[description = "Leave out ships that were never built"] historical: Option<bool>,
) -> Result<(), Error> {
    let Some(place) = place(ctx) else {
        return Ok(());
    };
    let data = ctx.data();
    let options = wiring::round_options(min_tier, max_tier, historical);
    let invoker = UserId::new(ctx.author().id.get());
    let outcome = data
        .table
        .start(place, options, invoker, |draw, number| async move {
            let path = data.root.silhouette(&data.catalog_name, draw.ship());
            let png = tokio::fs::read(&path).await?;
            let reply = poise::CreateReply::default()
                .embed(round_embed(&draw))
                .attachment(serenity::CreateAttachment::bytes(png, SILHOUETTE_FILE))
                .components(vec![cancel_row(wiring::cancel_button_id(number), false)]);
            let handle = ctx.send(reply).await?;
            let message = handle.message().await?;
            Ok::<Snowflake, Error>(Snowflake::new(message.id.get()))
        })
        .await;
    match outcome {
        StartOutcome::Started { .. } => Ok(()),
        StartOutcome::Busy => private(ctx, text::ALREADY_RUNNING).await,
        StartOutcome::NoShips(_) => private(ctx, text::empty_pool(&options)).await,
        StartOutcome::PostFailed(error) => Err(error),
    }
}

#[poise::command(slash_command)]
async fn ship(_ctx: Context<'_>) -> Result<(), Error> {
    Ok(())
}

async fn suggest_ships(ctx: Context<'_>, partial: &str) -> serenity::CreateAutocompleteResponse {
    let choices = ctx
        .data()
        .directory
        .suggest(partial)
        .into_iter()
        .map(|suggestion| {
            serenity::AutocompleteChoice::new(suggestion.label, suggestion.index.to_string())
        })
        .collect();
    serenity::CreateAutocompleteResponse::new().set_choices(choices)
}

#[poise::command(slash_command, rename = "info")]
async fn ship_info(
    ctx: Context<'_>,
    #[description = "Ship name"]
    #[autocomplete = "suggest_ships"]
    ship: String,
) -> Result<(), Error> {
    let data = ctx.data();
    let card = data.directory.resolve(&ship).and_then(|index| {
        info::ship_card(&data.catalog, &data.curated, data.table.book(), index)
            .map(|card| (index.clone(), card))
    });
    let Some((index, card)) = card else {
        return private(ctx, text::NO_SHIP_MATCHES).await;
    };
    let embed = card.fields.into_iter().fold(
        serenity::CreateEmbed::new()
            .title(card.title)
            .colour(text::EMBED_COLOUR),
        |embed, (name, value)| embed.field(name, value, true),
    );
    let path = data.root.silhouette(&data.catalog_name, &index);
    let reply = match tokio::fs::read(&path).await {
        Ok(png) => poise::CreateReply::default()
            .embed(embed.image(format!("attachment://{SILHOUETTE_FILE}")))
            .attachment(serenity::CreateAttachment::bytes(png, SILHOUETTE_FILE)),
        Err(_) => poise::CreateReply::default().embed(embed),
    };
    ctx.send(reply.ephemeral(true)).await?;
    Ok(())
}

#[poise::command(slash_command, guild_only)]
async fn profile(
    ctx: Context<'_>,
    #[description = "Whose profile to show (default: you)"] user: Option<serenity::User>,
) -> Result<(), Error> {
    let Some(place) = place(ctx) else {
        return Ok(());
    };
    let target = user.as_ref().unwrap_or_else(|| ctx.author());
    let stats = ctx
        .data()
        .table
        .solves()
        .profile(place.guild, UserId::new(target.id.get()))
        .await?;
    let embed = serenity::CreateEmbed::new()
        .title(target.display_name())
        .colour(text::EMBED_COLOUR)
        .field(text::ROUNDS_WON, stats.wins.to_string(), true)
        .field(text::BEST_TIME, text::best_time(stats.best), true);
    ctx.send(poise::CreateReply::default().embed(embed)).await?;
    Ok(())
}

#[poise::command(slash_command)]
async fn about(ctx: Context<'_>) -> Result<(), Error> {
    let data = ctx.data();
    let embed = serenity::CreateEmbed::new()
        .title(text::ABOUT_TITLE)
        .colour(text::EMBED_COLOUR)
        .description(text::about(
            &data.catalog.provenance,
            &data.catalog_name,
            env!("CARGO_PKG_VERSION"),
        ));
    ctx.send(poise::CreateReply::default().embed(embed)).await?;
    Ok(())
}
```

Whole file `crates/barnacle-bot/src/discord/events.rs`:

```rust
use barnacle_guess::Guess;
use barnacle_guess::Snowflake;
use barnacle_guess::UserId;
use poise::serenity_prelude as serenity;

use super::Data;
use super::Error;
use crate::ids::ChannelId;
use crate::ids::GuildId;
use crate::ids::Place;
use crate::table::CancelOutcome;
use crate::text;
use crate::wiring;

pub async fn handle(
    framework: poise::FrameworkContext<'_, Data, Error>,
    event: &serenity::FullEvent,
) -> Result<(), Error> {
    let data = framework.user_data;
    match event {
        serenity::FullEvent::Message { new_message } => {
            let Some(guild) = new_message.guild_id else {
                return Ok(());
            };
            let place = Place {
                guild: GuildId::new(guild.get()),
                channel: ChannelId::new(new_message.channel_id.get()),
            };
            let guess = Guess {
                author: UserId::new(new_message.author.id.get()),
                author_is_bot: new_message.author.bot,
                message: Snowflake::new(new_message.id.get()),
                text: &new_message.content,
            };
            data.table.hear(place, guess).await;
            Ok(())
        }
        serenity::FullEvent::InteractionCreate {
            interaction: serenity::Interaction::Component(component),
        } => {
            let (Some(number), Some(guild)) = (
                wiring::round_number(&component.data.custom_id),
                component.guild_id,
            ) else {
                return Ok(());
            };
            let place = Place {
                guild: GuildId::new(guild.get()),
                channel: ChannelId::new(component.channel_id.get()),
            };
            let can_manage_messages = component
                .member
                .as_ref()
                .and_then(|member| member.permissions)
                .is_some_and(|permissions| permissions.manage_messages());
            let outcome = data
                .table
                .cancel(
                    place,
                    number,
                    UserId::new(component.user.id.get()),
                    can_manage_messages,
                )
                .await;
            let response = match outcome {
                CancelOutcome::Cancelled => serenity::CreateInteractionResponse::Acknowledge,
                CancelOutcome::Refused => private_response(text::CANCEL_REFUSED),
                CancelOutcome::AlreadyOver => private_response(text::ROUND_OVER),
            };
            component
                .create_response(framework.serenity_context, response)
                .await?;
            Ok(())
        }
        _ => Ok(()),
    }
}

fn private_response(content: &str) -> serenity::CreateInteractionResponse {
    serenity::CreateInteractionResponse::Message(
        serenity::CreateInteractionResponseMessage::new()
            .content(content)
            .ephemeral(true),
    )
}

pub async fn on_error(error: poise::FrameworkError<'_, Data, Error>) {
    match error {
        poise::FrameworkError::Command { error, ctx, .. } => {
            tracing::error!(command = %ctx.command().qualified_name, %error, "a command failed");
            let reply = poise::CreateReply::default()
                .content(text::SOMETHING_WENT_WRONG)
                .ephemeral(true);
            if let Err(error) = ctx.send(reply).await {
                tracing::error!(%error, "the failure notice could not be sent");
            }
        }
        other => {
            if let Err(error) = poise::builtins::on_error(other).await {
                tracing::error!(%error, "an error could not be handled");
            }
        }
    }
}
```

Whole file `crates/barnacle-bot/src/main.rs`:

```rust
use std::path::PathBuf;
use std::process::ExitCode;

use barnacle_bot::discord;
use barnacle_bot::startup;
use clap::Parser;

#[derive(Parser)]
#[command(
    version,
    about = "Barnacle, a Discord bot for World of Warships players"
)]
struct Cli {
    #[arg(long, default_value = "barnacle.toml")]
    config: PathBuf,
}

#[derive(Debug, thiserror::Error)]
enum AppError {
    #[error(transparent)]
    Startup(#[from] startup::StartupError),
    #[error(transparent)]
    Run(#[from] discord::RunError),
}

#[tokio::main]
async fn main() -> ExitCode {
    tracing_subscriber::fmt().init();
    match run(Cli::parse()).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(AppError::Startup(error)) => {
            eprintln!("{}", startup::describe(&error));
            ExitCode::FAILURE
        }
        Err(AppError::Run(error)) => {
            eprintln!("{}", startup::describe(&error));
            ExitCode::FAILURE
        }
    }
}

async fn run(cli: Cli) -> Result<(), AppError> {
    let config = startup::read_config(&cli.config)?;
    let token = startup::read_token(std::env::var("DISCORD_TOKEN").ok())?;
    let loaded = startup::load(&config)?;
    let solves = startup::open_solves(&config.database).await?;
    discord::run(token, config.commands, loaded, solves).await?;
    Ok(())
}
```

Whole file `crates/barnacle-bot/src/lib.rs`:

```rust
pub mod config;
pub mod discord;
pub mod ids;
pub mod info;
pub mod lookup;
pub mod solves;
pub mod startup;
pub mod table;
pub mod text;
pub mod wiring;
```

Notes on this code:
- **Command descriptions:** each command's `description` is set in `commands::all()`, because the macro would only take it from a doc comment. The `ship` parent gets its `info` subcommand and `subcommand_required` the same way. The macro refuses `subcommand_required` unless its `subcommands(...)` attribute lists the subcommands.
- **Order in `post_ending`:** the ending message is sent and the button is disabled before either error is reported. So a failed reply still disables the button.
- **Cancel clicks:** a successful cancel is acknowledged with `CreateInteractionResponse::Acknowledge`, because the ending post has already updated the message.

- [ ] **Step 2: Build and lint**

Run: `cargo build -p barnacle-bot && cargo clippy --workspace --all-targets -- -D warnings`
Expected: both succeed with no warnings.

- [ ] **Step 3: Check the startup failures without contacting Discord**

Each command below stops before the bot connects, so no real token is needed. `placeholder` is not a credential.

Run: `cargo run -q -p barnacle-bot -- --config barnacle.example.toml; echo "exit $?"`
Expected:
```
barnacle.example.toml is not a valid Barnacle config
  caused by: commands.scope is "guilds" but commands.guilds lists no server
exit 1
```

Run: `tmp=$(mktemp -d) && printf 'database = "%s/missing.sqlite3"\n\n[commands]\nscope = "guilds"\nguilds = [1]\n' "$tmp" > "$tmp/barnacle.toml" && env -u DISCORD_TOKEN cargo run -q -p barnacle-bot -- --config "$tmp/barnacle.toml"; echo "exit $?"; DISCORD_TOKEN=placeholder cargo run -q -p barnacle-bot -- --config "$tmp/barnacle.toml"; echo "exit $?"; ls "$tmp"`
Expected:
- The first run prints `DISCORD_TOKEN is not set` and `exit 1`.
- The second run loads the real catalog named by `data/catalog/current`, the real `curation/ships.toml` and every silhouette. It then prints `the solves database <tmp>/missing.sqlite3 is not ready; create it with `sqlite3 <tmp>/missing.sqlite3 < migrations/0001_guess_solves.sql``, followed by its causes and `exit 1`.
- `ls` shows only `barnacle.toml`, so the bot did not create the database.

If the second run reports curation problems or missing silhouettes instead, the data in `data/` has changed since this plan was written. Stop and report it.

- [ ] **Step 4: Document running the bot**

Replace in `README.md`:

````markdown
## License
````

with:

````markdown
## Running the bot

Barnacle runs on your own machine and serves the catalog that `use` selected.

1. In the [Discord Developer Portal](https://discord.com/developers/applications), create an application, add a bot to it, and copy the bot token.
2. On the Bot page, turn on Message Content Intent under Privileged Gateway Intents. Barnacle reads chat messages to check answers.
3. In the application settings, turn off Public Bot, so only you can add Barnacle to servers.
4. Invite the bot with the `bot` and `applications.commands` scopes and the View Channels, Send Messages, Embed Links and Attach Files permissions.
5. Copy `barnacle.example.toml` to `barnacle.toml` and list the IDs of the servers to register commands in. Once the bot should work in every server you add it to, change the scope to `"global"` and remove `guilds`.
6. Create the solves database once:

   ```bash
   sqlite3 data/barnacle.sqlite3 < migrations/0001_guess_solves.sql
   ```

7. Start the bot. Reading the token with `read` keeps it out of your shell history:

   ```bash
   read -rs DISCORD_TOKEN && export DISCORD_TOKEN && cargo run --release -p barnacle-bot
   ```

Before it connects, the bot checks the catalog, the curation file, the silhouettes and the database, and names anything that is missing.

### Player data

`data/barnacle.sqlite3` stores the server ID, the player's Discord user ID, the ship and the time of every won round. It stores no names and no messages.

- To delete a player's data, stop the bot and run `sqlite3 data/barnacle.sqlite3 "DELETE FROM guess_solves WHERE user_id = <user ID>;"`.
- To back the data up, copy the file while the bot is stopped.
- `migrations/0001_guess_solves.down.sql` removes the table and every stored solve with it. Back the file up before running it.

## License
````

- [ ] **Step 5: Commit**

```bash
git add crates/barnacle-bot/src/discord.rs crates/barnacle-bot/src/discord crates/barnacle-bot/src/main.rs crates/barnacle-bot/src/lib.rs README.md
git commit -m "feat(bot): connect the commands to Discord" -- crates/barnacle-bot/src/discord.rs crates/barnacle-bot/src/discord crates/barnacle-bot/src/main.rs crates/barnacle-bot/src/lib.rs README.md
```

---

### Task 10: Full verification and the owner's live checklist

**Files:** none changed.

- [ ] **Step 1: Run what CI runs, plus the real-data checks**

Run: `cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace && cargo deny check licenses`
Expected:
- No formatting diff and no clippy warnings.
- 196 tests passing: 130 from `main`, 7 new in `barnacle-catalog` and `barnacle-guess`, and 59 in `barnacle-bot`.
- `licenses ok`.

Run: `BARNACLE_TEST_CATALOG_DIR="$PWD/data/catalog/$(cat data/catalog/current)" cargo test -p barnacle-guess --test real_catalog -- --nocapture`
Expected: `1 passed`, with the pool sizes printed. This confirms Task 2 did not change the real pool.

- [ ] **Step 2: Hand the live checklist to the owner**

The executor cannot run this step: it needs the owner's Discord application and token. Report that the branch is ready for it, list the steps below, and record the owner's results when they come back.

1. Follow README "Running the bot" steps 1-7, with the owner's test server listed in `barnacle.toml`. The log shows `commands registered in server`.
2. **A full round with a win.** Run `/guess`.
   - The reply shows "Name that ship", the silhouette, Tiers VI-XI, "Hint in 20 seconds" and a Cancel button.
   - A wrong name typed in chat gets no reaction.
   - The right name gets a reply "Correct: ..., Solved in N s.", and the Cancel button turns grey.
3. **A timed-out round.** Run `/guess` and answer nothing. "Hint: it's tier ..." appears after about 20 s, "Nobody named it. It was ..." after about 30 s, and the button turns grey.
4. **A refused cancel, then an allowed one.** Run `/guess`.
   - A second account without Manage Messages clicks Cancel and gets a private "Only the player who started this round, ...".
   - The first account clicks Cancel, and "@user ended the round. It was ..." appears.
5. **A busy channel.** Run `/guess` twice in a row. The second run gets a private "This channel already has a round running."
6. **Options.** Run `/guess min_tier:8 max_tier:8 historical:True`. The embed shows Tiers VIII and "Paper ships excluded", and the hint names a nation.
7. **Ship lookup.** Type `/ship info ship:yama`. Suggestions appear. Picking one shows a private card with the silhouette, "In /guess", "Accepted answers" and "Look-alikes".
8. **Profiles.** Run `/profile`, then `/profile user:<the second account>`. Both show "Rounds won" and "Best time", or "No rounds won here yet."
9. **About.** Run `/about`. It shows the game version, build, catalog, data commit, toolkit versions and the Wargaming notice.
10. **A restart.** Stop the bot with Ctrl-C and start it again. Clicking Cancel on an older round gets a private "That round has already ended."
11. **Optional: Message Content off.** Turn off Message Content Intent and start the bot. It exits with "Discord refused the Message Content intent; turn on Message Content Intent ...". Turn it back on afterwards.

---

## Self-review

| Design section | Task |
|---|---|
| 4.1 crate, modules and dependencies | 3-9 |
| 4.2 moving catalog folder reading | 1 |
| 4.3 `barnacle-guess` additions | 2 |
| 5.1 configuration, 5.2 startup checks | 4, 8, 9 (main) |
| 6.1-6.5 channel state, start, guesses, timer, cancel | 7 (core), 9 (Discord events) |
| 6.6 the Discord interface and the store interface | 7, 9 (announcer) |
| 6.7 restarts and logging | 7 (tracing), 10 (checklist step 10) |
| 7.1-7.6 commands, wording, labels, lookup, profile, about, permissions | 5, 6, 9 |
| 8.1-8.4 schema, migration, queries, data handling | 3, 9 (README) |
| 9 errors | 7, 8, 9 (`RunError`, `on_error`) |
| 10 testing | 3-8; Discord wiring in 9 and 10 |
| 11 Discord setup | 9 (README), 10 |

**How the plan's code was checked (2026-09-16):**
- **Scratch build:** the code was built and tested in a scratch copy of this branch. rustfmt and clippy with `-D warnings` were clean, all 196 workspace tests passed, and `cargo deny check licenses` passed.
- **Real data:** the prototype binary loaded the real `15.8.0_13187581_r4` catalog, the real curation file and every silhouette. It then stopped at a missing database without creating it and without contacting Discord.
- **Deliberate breaks:** twenty rules were broken one at a time, and each break made at least one test fail. Seventeen were caught on the first run; tests were then added for the other three (a timer from an earlier round hinting at or ending the next round, and a same-length schema check).
  - **Round core:** the busy check, the round-number checks in cancel, hint and timeout, the recent-ships memory, taking a won round out of the channel, and the hint itself.
  - **Storage:** strict personal bests, per-server profiles, and the schema check.
  - **Wording and lookup:** the single full stop, prefix-first ordering, the 25-suggestion cap, and case-insensitive IDs.
  - **Wiring, startup and config:** Cancel button IDs, curation validation, the silhouette check, `global` without `guilds`, the base ship's name on the card, and blank tokens.
- **Task-by-task replay:** a script applied this plan to a clean git copy of `feat/discord-bot` at the design commit, with `data/` linked in for the real-data steps.
  - Every "verify they fail" step failed with its named compiler error, and every "verify they pass" step passed tests and clippy.
  - The startup checks in Task 9 printed the expected messages, and all nine commits went through with their pathspecs, leaving a clean tree.
  - The final tree matched the prototype file for file and passed all 196 tests.
- **Not checked by the prototype:** anything that needs Discord itself, which is the owner's checklist in Task 10.

## Changes after the code review (2026-09-16)

An independent review of the executed branch found one high, four medium and four low problems, recorded in design section 13. The branch now differs from the task listings above in these places, and the branch is the reference.

| Change | Commit |
|---|---|
| Commands are registered over HTTP before the gateway connects, and a failure ends the program (`RunError::Registration`) | `c38791a` |
| The round post, hint and ending post each get 5 seconds (`ANNOUNCE_TIMEOUT`, `StartOutcome::PostTimedOut`). `FakeDiscord::hanging` and `FakeStore::pausing` were added, and the race test now interleaves. | `ad68b5c` |
| Cancel clicks are deferred privately before the round ends | `ad39e9a` |
| `/guess` checks the bot's channel permissions (`wiring::ChannelAccess`, `wiring::missing_permissions`, `text::missing_permissions`). An untracked round post is deleted. The README invite list adds Read Message History. | `0fc4d6f` |
| Bot posts allow no pings, and the win reply survives a deleted guess | `f5d1007` |
| Config errors keep only the parser's message and span | `4220ee7` |
| The first correct answer opens a 250 ms settle window (`SETTLE_WINDOW`), and the earliest message ID wins. `hear` now takes `self: &Arc<Self>`. Round tests wait out the window. | `09eda6e` |

New tests (11): two stuck-call tests and six settle-window tests in `table`, and one each in `wiring`, `text` and `config`. A second review then found three medium and three low problems, fixed in `3922c6c`, `4562a8f`, `ce02ee6` and `51bcc43`, with `2054a9c` rewriting the race test:

- Config errors give only a line and column.
- Threads need Send Messages in Threads.
- A timed-out round post is deleted.
- Cancel clicks get a silent acknowledgement, and refusals get a private follow-up.
- The registration error leaves the cause to its chain.
- The race test runs both orders, and `FakeStore::pausing` was removed.

The workspace now has 209 tests.

Two more steps for the owner's live checklist in Task 10:

12. **A wrong server ID.** Put a server ID the bot has not joined into `commands.guilds` and start the bot. It exits with "Discord refused to register the commands", followed by a "caused by:" line carrying Discord's reason. Put the right ID back afterwards.
13. **A missing permission.** In one channel, deny the bot Read Message History and run `/guess` there. The reply is a private "I need these permissions in this channel to run a round: Read Message History." Restore the permission afterwards.

## Changes after the live checklist (2026-09-16)

The owner ran the live checklist and then asked for `/leaderboard`, described in design section 14. The workspace now has 218 tests. Four more steps for the owner's live checklist:

14. **The default board.** In a server with wins, run `/leaderboard`. The title is "Most wins in this server", and each row reads like "**1.** Name · 12 wins · best 3.412 s", ordered by wins. The numbers match `/profile` for the same players in the same server.
15. **Fastest times.** Run `/leaderboard sort:Fastest time`. The title is "Fastest times in this server", and the rows are ordered by best time.
16. **A long board.** Run `/leaderboard limit:50`. Every player with a win in the server is listed, at most 50. A server with no wins gets "No rounds won here yet."
17. **A player who left.** If a player with wins has left the server, their row says "Former member".
