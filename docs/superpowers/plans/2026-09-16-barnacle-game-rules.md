# Barnacle Game Rules Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A `barnacle-guess` library that holds every rule of the `/guess` silhouette game (options, pool, draw, answers, hint, timing, judging, cancel permission) as pure logic, with no Discord types, so the bot in Plan 3 only wires it to Discord.

**Architecture:** One new crate, `crates/barnacle-guess`, depending only on `barnacle-catalog`, `rand` and `thiserror`. A `ShipBook` is built once from a `Catalog` and a `CurationConfig` and answers every per-round question. A `Draw` is one chosen ship with its answers, hint and reveal, and becomes a `Round` once the bot has posted it. The crate never reads a clock: elapsed time comes from Discord message IDs, and the answer windows are constants that the bot's timers use. The random source is passed in, so tests use a seeded generator.

**Tech Stack:** Rust 1.97 (edition 2024), `barnacle-catalog` (this workspace), `rand` 0.10, `thiserror` 2.

**Spec:** `docs/specs/2026-09-16-silhouette-game-spec.md`, sections 5 (game design, especially 5.5) and 8 (testing). Plan 1, already merged, built the catalog and curation this plan reads: `docs/superpowers/plans/2026-09-16-barnacle-data-pipeline.md`.

**Follow-up plan (not in this one):** Plan 3 builds `barnacle-bot`: Discord commands, the channel lock, the SQLite solves table, `/profile`, `/ship info`, `/about`, and all user-facing text.

## Global Constraints

- Toolchain: Rust `1.97`, from the existing `rust-toolchain.toml`. Edition `2024`.
- `barnacle-guess` depends on `barnacle-catalog`, `rand` and `thiserror` only. No wows-toolkit crate, no Discord crate, no async runtime (spec 2.3: "barnacle-guess/ library: game rules (pool, answers, hints, timing), no Discord types").
- `rand = "0.10"` in `[workspace.dependencies]`. 0.10.2 is current (released 2026-07-02, MIT OR Apache-2.0, needs Rust 1.85). In 0.10, `rand::Rng` is the core generator trait (it was `RngCore` before), `rand::seq::IndexedRandom::choose` picks a slice element, and `rand::rngs::StdRng` with `rand::SeedableRng::seed_from_u64` gives a seeded generator. `StdRng` is in the default features.
- Game values, copied from spec 5.5:
  - Tiers are 1-11. The defaults are `min_tier` 6 and `max_tier` 11, and a reversed range is swapped. `historical` defaults to false.
  - Draw uniformly, "skipping the channel's last 20 ships when the pool is larger than 20".
  - The answer window is "20 s, then a hint, then 10 s, then the answer is revealed".
  - The hint is the "Tier when the range spans several tiers, otherwise nation".
  - Cleaning: "Transliterate to ASCII, lowercase, remove whitespace and `- . ' ,`". This already exists as `barnacle_catalog::names::clean_answer`; reuse it and never reimplement it.
  - Elapsed time is the "Difference between the round post's and the winning message's snowflake timestamps".
  - Any non-bot user in the channel may answer.
  - The invoker, or a member with Manage Messages, may cancel.
- Discord message IDs (snowflakes) keep milliseconds since the Discord epoch in bits 63-22, read as `snowflake >> 22` (Discord developer docs, Reference, Snowflakes).
- License: Apache-2.0. Every dependency's license must be on the `deny.toml` allowlist. If a new license appears, stop and ask the owner; do not add it. `rand` 0.10 and its dependencies pass `cargo deny check licenses` (checked 2026-09-16).
- Never copy anything from padtrack/track: no code, no UI text, no entries from its `guess.toml`. This crate has no UI text. Its error `Display` strings are for logs only, and the bot writes its own wording.
- No code comments, docstrings or section-header comments. Tooling pragmas such as `#![allow(dead_code)]` are allowed.
- No emojis anywhere.
- Domain values are newtypes (`UserId`, `Snowflake`, and the catalog's `ShipIndex`, `Tier`, `Nation`). "Absent" is `Option`, never a sentinel value.
- Errors are `thiserror` enums with structured fields. Never parse an error's text.
- Build new values instead of mutating existing ones: `RecentShips::remember` returns a new value. The random generator is the only mutable input. Local accumulation inside one function is acceptable.
- Tests never touch the network or a clock. The one real-data test skips unless `BARNACLE_TEST_CATALOG_DIR` is set. It must be an absolute path, because Cargo runs integration tests from `crates/barnacle-guess`.
- Work on a branch `feat/game-rules` cut from `main`. One commit per task, using Conventional Commits and `git commit -m "..." -- <paths>`, never a bare `git commit`.

## Decisions this plan makes (confirm before executing)

1. **No clock object.** Spec 5.5 and 8 call for "an injected clock". This plan goes further and has no clock at all:
   - Elapsed time is computed from the two message IDs.
   - The 20 s and 10 s windows are the `Timing::STANDARD` constant, which the bot's timers read.
   - Every rule stays testable without a fake clock, and nothing in the crate can drift from wall time.
2. **Look-alikes bring their variants and aliases with them.** Spec 5.5 lists "look-alikes allowed by the options" but does not say whether a look-alike's own variant names and aliases count. This plan counts them: when a look-alike could have been drawn, every name accepted for that look-alike is accepted.
3. **One bound can flip the default range.** `max_tier` 3 with no `min_tier` gives tiers 3-6, because the default `min_tier` of 6 is above 3 and spec 5.1 says a reversed range is swapped.
4. **The no-repeat memory belongs to the bot.** `RecentShips` is a plain value. The bot keeps one per channel in memory, so it resets on restart, as spec 5.2 says.

## File Structure

```
Cargo.toml                                   workspace: add rand to [workspace.dependencies]
crates/barnacle-guess/
  Cargo.toml
  src/lib.rs                                 module wiring and re-exports
  src/options.rs                             RoundOptions: tier range, historical, defaults, swap
  src/ids.rs                                 UserId, Snowflake and elapsed time between two IDs
  src/recent.rs                              RecentShips: the last 20 ships drawn in a channel
  src/reveal.rs                              Reveal: name, tier, nation, class shown after a round
  src/book.rs                                ShipBook: pool, answers, look-alikes, draw
  src/draw.rs                                Draw and Hint
  src/error.rs                               GameError
  src/round.rs                               Round, Guess, Solve, Timing, Draw::start
  tests/common/mod.rs                        test builders
  tests/options.rs
  tests/ids.rs
  tests/recent.rs
  tests/pool.rs
  tests/answers.rs
  tests/draw.rs
  tests/round.rs
  tests/real_catalog.rs                      skips unless BARNACLE_TEST_CATALOG_DIR is set
```

`barnacle-catalog` types this plan uses, all already on `main`:
- `Catalog { provenance: Provenance, ships: Vec<Ship> }`, with `Catalog::from_json(&str)` and `Catalog::get(&ShipIndex) -> Option<&Ship>`.
- `Ship { id, index, tier, group, class, nation, is_paper, name: Option<ShipName>, silhouette: Option<Silhouette> }`.
- `ShipName { short: String, full: Option<String> }`, whose `display()` returns the full name if present, otherwise the short name.
- `Tier::new(u32) -> Result<Tier, ModelError>` and `Tier::get() -> u32`. `Tier` is `Copy + Ord`.
- `ShipIndex::parse(&str)`, `ShipIndex::as_str()`, and `Display`.
- `Nation::new`, `ShipGroup::new`, `ParamId::new`, and the `ShipClass` enum.
- `curation::CurationConfig::from_toml(&str)`, whose `groups` field is required. Its `aliases: Vec<AliasEntry { index, names }>` and `lookalikes: Vec<LookalikeGroup { ships }>` default to empty.
- `curation::curate(&Catalog, &CurationConfig) -> Curated { pool: BTreeSet<ShipIndex>, removed: BTreeMap<ShipIndex, Removal> }`.
- `Removal::base() -> Option<&ShipIndex>`, which already follows copy-of-a-copy chains to a base in the pool.
- `names::clean_answer(&str) -> String`.

---

### Task 1: Crate and round options

**Files:**
- Create: `crates/barnacle-guess/Cargo.toml`
- Create: `crates/barnacle-guess/src/lib.rs`
- Create: `crates/barnacle-guess/src/options.rs`
- Create: `crates/barnacle-guess/tests/common/mod.rs`
- Test: `crates/barnacle-guess/tests/options.rs`

**Interfaces:**
- Consumes: `barnacle_catalog::Tier`.
- Produces:
  - `RoundOptions::new(min_tier: Option<Tier>, max_tier: Option<Tier>, historical: Option<bool>) -> RoundOptions`, plus `Default`, `Copy` and `PartialEq`.
  - `min_tier(&self) -> Tier`, `max_tier(&self) -> Tier`, `historical(&self) -> bool`.
  - `spans_several_tiers(&self) -> bool`.
  - `allows(&self, tier: Tier, is_paper: bool) -> bool`.
  - The test helpers `common::index(&str) -> ShipIndex` and `common::tier(u32) -> Tier`.

- [ ] **Step 1: Cut the branch and scaffold the crate**

Run: `git switch -c feat/game-rules main`

The workspace already includes `crates/*`, so the crate is picked up without editing the root manifest.

Whole file `crates/barnacle-guess/Cargo.toml`:

```toml
[package]
name = "barnacle-guess"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true

[dependencies]
barnacle-catalog.workspace = true
```

Whole file `crates/barnacle-guess/src/lib.rs`:

```rust
```

- [ ] **Step 2: Write the failing tests**

Whole file `crates/barnacle-guess/tests/common/mod.rs`:

```rust
#![allow(dead_code)]

use barnacle_catalog::ShipIndex;
use barnacle_catalog::Tier;

pub fn index(value: &str) -> ShipIndex {
    ShipIndex::parse(value).unwrap()
}

pub fn tier(value: u32) -> Tier {
    Tier::new(value).unwrap()
}
```

Whole file `crates/barnacle-guess/tests/options.rs`:

```rust
mod common;

use barnacle_guess::RoundOptions;
use common::tier;

#[test]
fn defaults_are_tiers_six_to_eleven_without_the_historical_filter() {
    let options = RoundOptions::default();
    assert_eq!(
        (options.min_tier(), options.max_tier(), options.historical()),
        (tier(6), tier(11), false)
    );
}

#[test]
fn a_reversed_range_is_swapped() {
    let options = RoundOptions::new(Some(tier(9)), Some(tier(4)), Some(true));
    assert_eq!(
        (options.min_tier(), options.max_tier(), options.historical()),
        (tier(4), tier(9), true)
    );
}

#[test]
fn a_single_bound_keeps_the_other_default_and_is_swapped_when_reversed() {
    let only_max = RoundOptions::new(None, Some(tier(3)), None);
    assert_eq!(
        (only_max.min_tier(), only_max.max_tier()),
        (tier(3), tier(6))
    );
    let only_min = RoundOptions::new(Some(tier(8)), None, None);
    assert_eq!(
        (only_min.min_tier(), only_min.max_tier()),
        (tier(8), tier(11))
    );
}

#[test]
fn allows_only_tiers_inside_the_inclusive_range() {
    let options = RoundOptions::new(Some(tier(5)), Some(tier(7)), None);
    assert!(!options.allows(tier(4), false));
    assert!(options.allows(tier(5), false));
    assert!(options.allows(tier(7), false));
    assert!(!options.allows(tier(8), false));
}

#[test]
fn historical_rejects_paper_ships_and_nothing_else() {
    let historical = RoundOptions::new(None, None, Some(true));
    assert!(historical.allows(tier(8), false));
    assert!(!historical.allows(tier(8), true));
    assert!(RoundOptions::default().allows(tier(8), true));
}

#[test]
fn spans_several_tiers_only_when_the_bounds_differ() {
    assert!(RoundOptions::default().spans_several_tiers());
    assert!(!RoundOptions::new(Some(tier(10)), Some(tier(10)), None).spans_several_tiers());
}
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test -p barnacle-guess --test options`
Expected: FAIL to compile with `error[E0432]: unresolved import `barnacle_guess::RoundOptions``.

- [ ] **Step 4: Implement the options**

Whole file `crates/barnacle-guess/src/options.rs`:

```rust
use barnacle_catalog::Tier;

const DEFAULT_MIN_TIER: u32 = 6;
const DEFAULT_MAX_TIER: u32 = 11;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RoundOptions {
    min_tier: Tier,
    max_tier: Tier,
    historical: bool,
}

impl RoundOptions {
    pub fn new(min_tier: Option<Tier>, max_tier: Option<Tier>, historical: Option<bool>) -> Self {
        let first = min_tier.unwrap_or_else(|| default_tier(DEFAULT_MIN_TIER));
        let second = max_tier.unwrap_or_else(|| default_tier(DEFAULT_MAX_TIER));
        Self {
            min_tier: first.min(second),
            max_tier: first.max(second),
            historical: historical.unwrap_or(false),
        }
    }

    pub fn min_tier(&self) -> Tier {
        self.min_tier
    }

    pub fn max_tier(&self) -> Tier {
        self.max_tier
    }

    pub fn historical(&self) -> bool {
        self.historical
    }

    pub fn spans_several_tiers(&self) -> bool {
        self.min_tier != self.max_tier
    }

    pub fn allows(&self, tier: Tier, is_paper: bool) -> bool {
        (self.min_tier..=self.max_tier).contains(&tier) && !(self.historical && is_paper)
    }
}

impl Default for RoundOptions {
    fn default() -> Self {
        Self::new(None, None, None)
    }
}

fn default_tier(value: u32) -> Tier {
    Tier::new(value).expect("default tiers are within 1-11")
}
```

Whole file `crates/barnacle-guess/src/lib.rs`:

```rust
mod options;

pub use options::RoundOptions;
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p barnacle-guess --test options && cargo clippy -p barnacle-guess --all-targets -- -D warnings`
Expected: `test result: ok. 6 passed`, and clippy reports no warnings.

- [ ] **Step 6: Commit**

```bash
git add Cargo.lock crates/barnacle-guess
git commit -m "feat(guess): add round options with tier defaults" -- Cargo.lock crates/barnacle-guess
```

---

### Task 2: Discord IDs and elapsed time

**Files:**
- Create: `crates/barnacle-guess/src/ids.rs`
- Modify: `crates/barnacle-guess/src/lib.rs`
- Test: `crates/barnacle-guess/tests/ids.rs`

**Interfaces:**
- Consumes: nothing from earlier tasks.
- Produces:
  - `UserId`, with `const fn new(u64)` and `const fn get(self) -> u64`.
  - `Snowflake`, with `const fn new(u64)`, `const fn get(self) -> u64`, `const fn millis_since_discord_epoch(self) -> u64`, and `fn elapsed_until(self, later: Snowflake) -> Duration`.
  - Both types are `Copy + Ord + Hash`.

- [ ] **Step 1: Write the failing tests**

`175928847299117063` is the example ID in Discord's developer documentation. Its timestamp bits hold 41944705796 ms since the Discord epoch, which is 2016-04-30 11:18:25.796 UTC.

Whole file `crates/barnacle-guess/tests/ids.rs`:

```rust
use std::time::Duration;

use barnacle_guess::Snowflake;
use barnacle_guess::UserId;

fn at(millis: u64, increment: u64) -> Snowflake {
    Snowflake::new((millis << 22) | increment)
}

#[test]
fn reads_the_timestamp_of_discords_documented_example_id() {
    assert_eq!(
        Snowflake::new(175_928_847_299_117_063).millis_since_discord_epoch(),
        41_944_705_796
    );
}

#[test]
fn elapsed_time_is_the_difference_between_the_two_timestamps() {
    assert_eq!(
        at(1_000, 4_095).elapsed_until(at(13_345, 0)),
        Duration::from_millis(12_345)
    );
}

#[test]
fn elapsed_time_is_zero_when_the_later_id_is_older() {
    assert_eq!(at(5_000, 0).elapsed_until(at(4_000, 0)), Duration::ZERO);
}

#[test]
fn ids_round_trip_their_raw_value() {
    assert_eq!(UserId::new(42).get(), 42);
    assert_eq!(Snowflake::new(7).get(), 7);
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p barnacle-guess --test ids`
Expected: FAIL to compile with `error[E0432]: unresolved import `barnacle_guess::Snowflake``.

- [ ] **Step 3: Implement the IDs**

Whole file `crates/barnacle-guess/src/ids.rs`:

```rust
use std::time::Duration;

const TIMESTAMP_SHIFT: u32 = 22;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct UserId(u64);

impl UserId {
    pub const fn new(raw: u64) -> Self {
        Self(raw)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Snowflake(u64);

impl Snowflake {
    pub const fn new(raw: u64) -> Self {
        Self(raw)
    }

    pub const fn get(self) -> u64 {
        self.0
    }

    pub const fn millis_since_discord_epoch(self) -> u64 {
        self.0 >> TIMESTAMP_SHIFT
    }

    pub fn elapsed_until(self, later: Snowflake) -> Duration {
        Duration::from_millis(
            later
                .millis_since_discord_epoch()
                .saturating_sub(self.millis_since_discord_epoch()),
        )
    }
}
```

Whole file `crates/barnacle-guess/src/lib.rs`:

```rust
mod ids;
mod options;

pub use ids::Snowflake;
pub use ids::UserId;
pub use options::RoundOptions;
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p barnacle-guess && cargo clippy -p barnacle-guess --all-targets -- -D warnings`
Expected: `options` 6 passed and `ids` 4 passed. Clippy reports no warnings.

- [ ] **Step 5: Commit**

```bash
git add crates/barnacle-guess/src/ids.rs crates/barnacle-guess/src/lib.rs crates/barnacle-guess/tests/ids.rs
git commit -m "feat(guess): read elapsed time from Discord message ids" -- crates/barnacle-guess/src/ids.rs crates/barnacle-guess/src/lib.rs crates/barnacle-guess/tests/ids.rs
```

---

### Task 3: A channel's recent ships

**Files:**
- Create: `crates/barnacle-guess/src/recent.rs`
- Modify: `crates/barnacle-guess/src/lib.rs`
- Modify: `crates/barnacle-guess/tests/common/mod.rs`
- Test: `crates/barnacle-guess/tests/recent.rs`

**Interfaces:**
- Consumes: `barnacle_catalog::ShipIndex`.
- Produces:
  - `RecentShips`, which is `Default + Clone + PartialEq`.
  - `RecentShips::LIMIT: usize = 20`.
  - `remember(&self, ShipIndex) -> RecentShips`, which returns a new value.
  - `contains(&self, &ShipIndex) -> bool`, `len(&self) -> usize`, `is_empty(&self) -> bool`.
  - The test helper `common::numbered(usize) -> ShipIndex`, which gives `PXSX000`, `PXSX001` and so on.

- [ ] **Step 1: Write the failing tests**

Whole file `crates/barnacle-guess/tests/common/mod.rs`:

```rust
#![allow(dead_code)]

use barnacle_catalog::ShipIndex;
use barnacle_catalog::Tier;

pub fn index(value: &str) -> ShipIndex {
    ShipIndex::parse(value).unwrap()
}

pub fn tier(value: u32) -> Tier {
    Tier::new(value).unwrap()
}

pub fn numbered(number: usize) -> ShipIndex {
    index(&format!("PXSX{number:03}"))
}
```

Whole file `crates/barnacle-guess/tests/recent.rs`:

```rust
mod common;

use barnacle_guess::RecentShips;
use common::numbered;

#[test]
fn starts_empty() {
    assert!(RecentShips::default().is_empty());
}

#[test]
fn remembering_returns_a_new_memory_and_leaves_the_old_one_alone() {
    let empty = RecentShips::default();
    let one = empty.remember(numbered(1));
    assert!(empty.is_empty());
    assert!(one.contains(&numbered(1)));
    assert!(!one.contains(&numbered(2)));
}

#[test]
fn keeps_only_the_last_twenty_ships() {
    let recent = (0..=20).fold(RecentShips::default(), |recent, number| {
        recent.remember(numbered(number))
    });
    assert_eq!(recent.len(), 20);
    assert!(!recent.contains(&numbered(0)));
    assert!(recent.contains(&numbered(1)));
    assert!(recent.contains(&numbered(20)));
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p barnacle-guess --test recent`
Expected: FAIL to compile with `error[E0432]: unresolved import `barnacle_guess::RecentShips``.

- [ ] **Step 3: Implement the memory**

Whole file `crates/barnacle-guess/src/recent.rs`:

```rust
use barnacle_catalog::ShipIndex;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RecentShips(Vec<ShipIndex>);

impl RecentShips {
    pub const LIMIT: usize = 20;

    pub fn remember(&self, index: ShipIndex) -> Self {
        let kept = self.0.len().min(Self::LIMIT - 1);
        Self(
            self.0[self.0.len() - kept..]
                .iter()
                .cloned()
                .chain(std::iter::once(index))
                .collect(),
        )
    }

    pub fn contains(&self, index: &ShipIndex) -> bool {
        self.0.contains(index)
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}
```

Whole file `crates/barnacle-guess/src/lib.rs`:

```rust
mod ids;
mod options;
mod recent;

pub use ids::Snowflake;
pub use ids::UserId;
pub use options::RoundOptions;
pub use recent::RecentShips;
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p barnacle-guess && cargo clippy -p barnacle-guess --all-targets -- -D warnings`
Expected: `options` 6 passed, `ids` 4 passed, `recent` 3 passed. Clippy reports no warnings.

- [ ] **Step 5: Commit**

```bash
git add crates/barnacle-guess/src/recent.rs crates/barnacle-guess/src/lib.rs crates/barnacle-guess/tests/common/mod.rs crates/barnacle-guess/tests/recent.rs
git commit -m "feat(guess): remember a channel's last twenty ships" -- crates/barnacle-guess/src/recent.rs crates/barnacle-guess/src/lib.rs crates/barnacle-guess/tests/common/mod.rs crates/barnacle-guess/tests/recent.rs
```

---

### Task 4: The ship pool

**Files:**
- Create: `crates/barnacle-guess/src/reveal.rs`
- Create: `crates/barnacle-guess/src/book.rs`
- Modify: `crates/barnacle-guess/src/lib.rs`
- Modify: `crates/barnacle-guess/tests/common/mod.rs` (final version; later tasks do not change it)
- Test: `crates/barnacle-guess/tests/pool.rs`

**Interfaces:**
- Consumes:
  - `RoundOptions::allows` (Task 1).
  - `barnacle_catalog::curation::curate`, `Curated::pool`, `Ship` and `ShipName::display`.
- Produces:
  - `Reveal { pub index: ShipIndex, pub name: String, pub tier: Tier, pub nation: Nation, pub class: ShipClass }`. Its `name` is the full English name when one exists.
  - `ShipBook::new(&Catalog, &CurationConfig) -> ShipBook`.
  - `ShipBook::pool(&self, &RoundOptions) -> Vec<&ShipIndex>`, in index order.
  - Test helpers:
    - Ship builders: `common::ship(index, name, tier, silhouette) -> Ship`, whose group is `upgradeable`, class `Cruiser`, nation `USA`, not paper, and with no full name.
    - Ship modifiers: `with_full_name`, `with_nation`, `with_group` and `paper`, each returning a new `Ship`.
    - Other helpers:
      - `fleet(size)` gives ships `PXSX000`... named `Hull 0`..., all tier 8, each with its own silhouette.
      - `set([&str; N]) -> BTreeSet<String>`.
      - `at(millis) -> Snowflake`.
      - `book(ships, curation_toml) -> ShipBook`, with `groups = ["upgradeable"]` prepended to the curation.

- [ ] **Step 1: Write the failing tests**

Whole file `crates/barnacle-guess/tests/common/mod.rs`:

```rust
#![allow(dead_code)]

use std::collections::BTreeSet;

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
use barnacle_guess::ShipBook;
use barnacle_guess::Snowflake;

pub fn index(value: &str) -> ShipIndex {
    ShipIndex::parse(value).unwrap()
}

pub fn tier(value: u32) -> Tier {
    Tier::new(value).unwrap()
}

pub fn numbered(number: usize) -> ShipIndex {
    index(&format!("PXSX{number:03}"))
}

pub fn at(millis: u64) -> Snowflake {
    Snowflake::new(millis << 22)
}

pub fn set<const N: usize>(answers: [&str; N]) -> BTreeSet<String> {
    answers.iter().map(|answer| (*answer).to_owned()).collect()
}

pub fn ship(value: &str, name: &str, tier_value: u32, silhouette: &str) -> Ship {
    Ship {
        id: ParamId::new(1),
        index: index(value),
        tier: tier(tier_value),
        group: ShipGroup::new("upgradeable"),
        class: ShipClass::Cruiser,
        nation: Nation::new("USA"),
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

pub fn with_full_name(ship: Ship, full: &str) -> Ship {
    let short = ship.name.as_ref().map(|name| name.short.clone()).unwrap();
    Ship {
        name: Some(ShipName {
            short,
            full: Some(full.to_owned()),
        }),
        ..ship
    }
}

pub fn with_nation(ship: Ship, nation: &str) -> Ship {
    Ship {
        nation: Nation::new(nation),
        ..ship
    }
}

pub fn with_group(ship: Ship, group: &str) -> Ship {
    Ship {
        group: ShipGroup::new(group),
        ..ship
    }
}

pub fn paper(ship: Ship) -> Ship {
    Ship {
        is_paper: true,
        ..ship
    }
}

pub fn fleet(size: usize) -> Vec<Ship> {
    (0..size)
        .map(|number| {
            ship(
                numbered(number).as_str(),
                &format!("Hull {number}"),
                8,
                &format!("silhouette-{number}"),
            )
        })
        .collect()
}

pub fn book(ships: Vec<Ship>, curation: &str) -> ShipBook {
    let catalog = Catalog {
        provenance: Provenance {
            game_version: "15.8.0".to_owned(),
            build: 13187581,
            data_repo_commit: "0000000".to_owned(),
            wowsunpack: "0.45.0".to_owned(),
            wows_data_mgr: "0.21.0".to_owned(),
        },
        ships,
    };
    let config =
        CurationConfig::from_toml(&format!("groups = [\"upgradeable\"]\n{curation}")).unwrap();
    ShipBook::new(&catalog, &config)
}
```

Whole file `crates/barnacle-guess/tests/pool.rs`:

```rust
mod common;

use barnacle_guess::RoundOptions;
use common::book;
use common::index;
use common::paper;
use common::ship;
use common::tier;
use common::with_group;

#[test]
fn the_pool_holds_eligible_base_ships_inside_the_tier_range() {
    let book = book(
        vec![
            ship("PASB004", "Wyoming", 4, "wyoming"),
            ship("PASB005", "New York", 5, "new-york"),
            ship("PASB006", "New Mexico", 6, "new-mexico"),
            ship("PASB010", "Montana", 10, "montana"),
            ship("PASB510", "Montana B", 10, "montana"),
            with_group(
                ship("PASX006", "Prototype", 6, "prototype"),
                "demoWithoutStats",
            ),
        ],
        "",
    );
    let options = RoundOptions::new(Some(tier(5)), Some(tier(10)), None);
    assert_eq!(
        book.pool(&options),
        [&index("PASB005"), &index("PASB006"), &index("PASB010")]
    );
}

#[test]
fn historical_drops_paper_ships_from_the_pool() {
    let book = book(
        vec![
            ship("PASB009", "Iowa", 9, "iowa"),
            paper(ship("PASB109", "Minnesota", 9, "minnesota")),
        ],
        "",
    );
    assert_eq!(book.pool(&RoundOptions::default()).len(), 2);
    assert_eq!(
        book.pool(&RoundOptions::new(None, None, Some(true))),
        [&index("PASB009")]
    );
}

#[test]
fn ships_the_curation_excludes_never_enter_the_pool() {
    let book = book(
        vec![
            ship("PJSC007", "Myoko", 7, "myoko"),
            ship("PJSC707", "ARP Myoko", 7, "arp-myoko"),
        ],
        "",
    );
    assert_eq!(book.pool(&RoundOptions::default()), [&index("PJSC007")]);
}
```

The fixtures lean on Plan 1's curation rules:
- `Montana B` shares Montana's silhouette, so rule 3 makes it a variant.
- `demoWithoutStats` is not in `groups`, so rule 1 drops the prototype.
- `ARP Myoko` has the collaboration prefix and no twin, so rule 4 excludes it.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p barnacle-guess --test pool`
Expected: FAIL to compile with `error[E0432]: unresolved import `barnacle_guess::ShipBook``, raised from `tests/common/mod.rs`. The other test targets fail to compile the same way until Step 3, since they share `common`.

- [ ] **Step 3: Implement the pool**

Whole file `crates/barnacle-guess/src/reveal.rs`:

```rust
use barnacle_catalog::Nation;
use barnacle_catalog::ShipClass;
use barnacle_catalog::ShipIndex;
use barnacle_catalog::Tier;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reveal {
    pub index: ShipIndex,
    pub name: String,
    pub tier: Tier,
    pub nation: Nation,
    pub class: ShipClass,
}
```

Whole file `crates/barnacle-guess/src/book.rs`:

```rust
use std::collections::BTreeMap;

use barnacle_catalog::Catalog;
use barnacle_catalog::Ship;
use barnacle_catalog::ShipIndex;
use barnacle_catalog::curation::CurationConfig;
use barnacle_catalog::curation::curate;

use crate::options::RoundOptions;
use crate::reveal::Reveal;

#[derive(Debug, Clone, PartialEq, Eq)]
struct Entry {
    reveal: Reveal,
    is_paper: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShipBook {
    entries: BTreeMap<ShipIndex, Entry>,
}

impl ShipBook {
    pub fn new(catalog: &Catalog, config: &CurationConfig) -> Self {
        let curated = curate(catalog, config);
        let ships: BTreeMap<&ShipIndex, &Ship> = catalog
            .ships
            .iter()
            .map(|ship| (&ship.index, ship))
            .collect();
        let entries = curated
            .pool
            .iter()
            .filter_map(|index| {
                let ship = ships.get(index)?;
                let name = ship.name.as_ref()?;
                let reveal = Reveal {
                    index: index.clone(),
                    name: name.display().to_owned(),
                    tier: ship.tier,
                    nation: ship.nation.clone(),
                    class: ship.class.clone(),
                };
                Some((
                    index.clone(),
                    Entry {
                        reveal,
                        is_paper: ship.is_paper,
                    },
                ))
            })
            .collect();
        Self { entries }
    }

    pub fn pool(&self, options: &RoundOptions) -> Vec<&ShipIndex> {
        self.eligible(options).map(|(index, _)| index).collect()
    }

    fn eligible(&self, options: &RoundOptions) -> impl Iterator<Item = (&ShipIndex, &Entry)> {
        let options = *options;
        self.entries
            .iter()
            .filter(move |(_, entry)| options.allows(entry.reveal.tier, entry.is_paper))
    }
}
```

Whole file `crates/barnacle-guess/src/lib.rs`:

```rust
mod book;
mod ids;
mod options;
mod recent;
mod reveal;

pub use book::ShipBook;
pub use ids::Snowflake;
pub use ids::UserId;
pub use options::RoundOptions;
pub use recent::RecentShips;
pub use reveal::Reveal;
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p barnacle-guess && cargo clippy -p barnacle-guess --all-targets -- -D warnings`
Expected: `options` 6, `ids` 4, `recent` 3 and `pool` 3 passed. Clippy reports no warnings.

- [ ] **Step 5: Commit**

```bash
git add crates/barnacle-guess/src/reveal.rs crates/barnacle-guess/src/book.rs crates/barnacle-guess/src/lib.rs crates/barnacle-guess/tests/common/mod.rs crates/barnacle-guess/tests/pool.rs
git commit -m "feat(guess): build the ship pool from the curated catalog" -- crates/barnacle-guess/src/reveal.rs crates/barnacle-guess/src/book.rs crates/barnacle-guess/src/lib.rs crates/barnacle-guess/tests/common/mod.rs crates/barnacle-guess/tests/pool.rs
```

---

### Task 5: Accepted answers

**Files:**
- Modify: `crates/barnacle-guess/src/book.rs` (whole file replaced)
- Test: `crates/barnacle-guess/tests/answers.rs`

**Interfaces:**
- Consumes:
  - `Removal::base()`, `CurationConfig::aliases` and `CurationConfig::lookalikes`.
  - `barnacle_catalog::names::clean_answer`.
- Produces: `ShipBook::answers(&self, index: &ShipIndex, options: &RoundOptions) -> BTreeSet<String>`. It is empty for a ship outside the book. Otherwise it holds the cleaned, non-empty forms of:
  - the ship's short and full names;
  - the names of every variant whose base is this ship;
  - aliases for the ship and for those variants;
  - the same for every look-alike that `options.allows`.

- [ ] **Step 1: Write the failing tests**

Whole file `crates/barnacle-guess/tests/answers.rs`:

```rust
mod common;

use barnacle_guess::RoundOptions;
use common::book;
use common::index;
use common::paper;
use common::set;
use common::ship;
use common::tier;
use common::with_full_name;

#[test]
fn short_and_full_names_are_accepted_in_cleaned_form() {
    let book = book(
        vec![with_full_name(
            ship("PASB017", "W. Virginia '41", 8, "west-virginia"),
            "West Virginia '41",
        )],
        "",
    );
    assert_eq!(
        book.answers(&index("PASB017"), &RoundOptions::default()),
        set(["wvirginia41", "westvirginia41"])
    );
}

#[test]
fn accented_names_are_accepted_in_ascii() {
    let book = book(vec![ship("PGSC109", "Ägir", 9, "agir")], "");
    assert_eq!(
        book.answers(&index("PGSC109"), &RoundOptions::default()),
        set(["agir"])
    );
}

#[test]
fn variant_and_alias_names_count_for_the_base_ship() {
    let book = book(
        vec![
            ship("PASB010", "Montana", 10, "montana"),
            ship("PASB510", "Montana B", 10, "montana"),
        ],
        r#"
[[aliases]]
index = "PASB010"
names = ["Monty"]

[[aliases]]
index = "PASB510"
names = ["Big Monty"]
"#,
    );
    assert_eq!(
        book.answers(&index("PASB010"), &RoundOptions::default()),
        set(["montana", "monty", "montanab", "bigmonty"])
    );
}

#[test]
fn a_collaboration_reskin_without_a_twin_adds_no_names() {
    let book = book(
        vec![
            ship("PJSC007", "Myoko", 7, "myoko"),
            ship("PJSC707", "ARP Myoko", 7, "arp-myoko"),
        ],
        "",
    );
    assert_eq!(
        book.answers(&index("PJSC007"), &RoundOptions::default()),
        set(["myoko"])
    );
}

#[test]
fn a_lookalike_counts_only_when_its_tier_is_in_the_range() {
    let book = book(
        vec![
            ship("PBSC507", "Belfast", 7, "belfast"),
            ship("PBSC528", "Belfast '43", 8, "belfast-43"),
            ship("PBSC108", "Edinburgh", 8, "edinburgh"),
        ],
        r#"
[[lookalikes]]
ships = ["PBSC507", "PBSC528"]
"#,
    );
    let both_tiers = RoundOptions::new(Some(tier(7)), Some(tier(8)), None);
    let tier_seven = RoundOptions::new(Some(tier(7)), Some(tier(7)), None);
    assert_eq!(
        book.answers(&index("PBSC507"), &both_tiers),
        set(["belfast", "belfast43"])
    );
    assert_eq!(
        book.answers(&index("PBSC528"), &both_tiers),
        set(["belfast", "belfast43"])
    );
    assert_eq!(
        book.answers(&index("PBSC507"), &tier_seven),
        set(["belfast"])
    );
}

#[test]
fn a_paper_lookalike_does_not_count_in_a_historical_round() {
    let book = book(
        vec![
            ship("PASB009", "Iowa", 9, "iowa"),
            paper(ship("PASB109", "Minnesota", 9, "minnesota")),
        ],
        r#"
[[lookalikes]]
ships = ["PASB009", "PASB109"]
"#,
    );
    assert_eq!(
        book.answers(&index("PASB009"), &RoundOptions::default()),
        set(["iowa", "minnesota"])
    );
    assert_eq!(
        book.answers(
            &index("PASB009"),
            &RoundOptions::new(None, None, Some(true))
        ),
        set(["iowa"])
    );
}

#[test]
fn a_ship_outside_the_pool_has_no_answers() {
    let book = book(vec![ship("PASB009", "Iowa", 9, "iowa")], "");
    assert!(
        book.answers(&index("PZSX999"), &RoundOptions::default())
            .is_empty()
    );
}

#[test]
fn a_name_that_cleans_to_nothing_is_not_an_answer() {
    let book = book(
        vec![ship("PASB009", "Iowa", 9, "iowa")],
        r#"
[[aliases]]
index = "PASB009"
names = ["- . ,"]
"#,
    );
    assert_eq!(
        book.answers(&index("PASB009"), &RoundOptions::default()),
        set(["iowa"])
    );
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p barnacle-guess --test answers`
Expected: FAIL to compile with `error[E0599]: no method named `answers` found for struct `ShipBook``.

- [ ] **Step 3: Implement the answers**

Whole file `crates/barnacle-guess/src/book.rs`:

```rust
use std::collections::BTreeMap;
use std::collections::BTreeSet;

use barnacle_catalog::Catalog;
use barnacle_catalog::Ship;
use barnacle_catalog::ShipIndex;
use barnacle_catalog::curation::CurationConfig;
use barnacle_catalog::curation::curate;
use barnacle_catalog::names::clean_answer;

use crate::options::RoundOptions;
use crate::reveal::Reveal;

#[derive(Debug, Clone, PartialEq, Eq)]
struct Entry {
    reveal: Reveal,
    is_paper: bool,
    answers: BTreeSet<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShipBook {
    entries: BTreeMap<ShipIndex, Entry>,
    lookalikes: BTreeMap<ShipIndex, Vec<ShipIndex>>,
}

impl ShipBook {
    pub fn new(catalog: &Catalog, config: &CurationConfig) -> Self {
        let curated = curate(catalog, config);
        let ships: BTreeMap<&ShipIndex, &Ship> = catalog
            .ships
            .iter()
            .map(|ship| (&ship.index, ship))
            .collect();
        let variants = group_by_key(
            curated
                .removed
                .iter()
                .filter_map(|(index, removal)| removal.base().map(|base| (base, index))),
        );
        let aliases = group_by_key(config.aliases.iter().flat_map(|entry| {
            entry
                .names
                .iter()
                .map(move |name| (&entry.index, name.as_str()))
        }));
        let entries = curated
            .pool
            .iter()
            .filter_map(|index| {
                let ship = ships.get(index)?;
                let name = ship.name.as_ref()?;
                let answers = std::iter::once(index)
                    .chain(variants.get(index).into_iter().flatten().copied())
                    .flat_map(|member| cleaned_names(member, &ships, &aliases))
                    .collect();
                let reveal = Reveal {
                    index: index.clone(),
                    name: name.display().to_owned(),
                    tier: ship.tier,
                    nation: ship.nation.clone(),
                    class: ship.class.clone(),
                };
                Some((
                    index.clone(),
                    Entry {
                        reveal,
                        is_paper: ship.is_paper,
                        answers,
                    },
                ))
            })
            .collect();
        let lookalikes = config
            .lookalikes
            .iter()
            .flat_map(|group| {
                group.ships.iter().map(|member| {
                    let others = group
                        .ships
                        .iter()
                        .filter(|other| *other != member)
                        .cloned()
                        .collect::<Vec<_>>();
                    (member.clone(), others)
                })
            })
            .collect();
        Self {
            entries,
            lookalikes,
        }
    }

    pub fn pool(&self, options: &RoundOptions) -> Vec<&ShipIndex> {
        self.eligible(options).map(|(index, _)| index).collect()
    }

    pub fn answers(&self, index: &ShipIndex, options: &RoundOptions) -> BTreeSet<String> {
        let lookalikes = self
            .lookalikes
            .get(index)
            .into_iter()
            .flatten()
            .filter_map(|other| self.entries.get(other))
            .filter(|entry| options.allows(entry.reveal.tier, entry.is_paper));
        self.entries
            .get(index)
            .into_iter()
            .chain(lookalikes)
            .flat_map(|entry| entry.answers.iter().cloned())
            .collect()
    }

    fn eligible(&self, options: &RoundOptions) -> impl Iterator<Item = (&ShipIndex, &Entry)> {
        let options = *options;
        self.entries
            .iter()
            .filter(move |(_, entry)| options.allows(entry.reveal.tier, entry.is_paper))
    }
}

fn group_by_key<K: Ord, V>(pairs: impl Iterator<Item = (K, V)>) -> BTreeMap<K, Vec<V>> {
    pairs.fold(BTreeMap::new(), |mut grouped, (key, value)| {
        grouped.entry(key).or_insert_with(Vec::new).push(value);
        grouped
    })
}

fn cleaned_names(
    index: &ShipIndex,
    ships: &BTreeMap<&ShipIndex, &Ship>,
    aliases: &BTreeMap<&ShipIndex, Vec<&str>>,
) -> Vec<String> {
    let official = ships
        .get(index)
        .and_then(|ship| ship.name.as_ref())
        .into_iter()
        .flat_map(|name| std::iter::once(name.short.as_str()).chain(name.full.as_deref()));
    let extra = aliases.get(index).into_iter().flatten().copied();
    official
        .chain(extra)
        .map(clean_answer)
        .filter(|answer| !answer.is_empty())
        .collect()
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p barnacle-guess && cargo clippy -p barnacle-guess --all-targets -- -D warnings`
Expected: `answers` 8 passed and every earlier target still passes. Clippy reports no warnings.

- [ ] **Step 5: Commit**

```bash
git add crates/barnacle-guess/src/book.rs crates/barnacle-guess/tests/answers.rs
git commit -m "feat(guess): accept names, variants, aliases and look-alikes" -- crates/barnacle-guess/src/book.rs crates/barnacle-guess/tests/answers.rs
```

---

### Task 6: Drawing a ship

**Files:**
- Modify: `Cargo.toml` (workspace root)
- Modify: `crates/barnacle-guess/Cargo.toml`
- Create: `crates/barnacle-guess/src/error.rs`
- Create: `crates/barnacle-guess/src/draw.rs`
- Modify: `crates/barnacle-guess/src/book.rs` (whole file replaced)
- Modify: `crates/barnacle-guess/src/lib.rs`
- Test: `crates/barnacle-guess/tests/draw.rs`

**Interfaces:**
- Consumes: `RecentShips::LIMIT` and `contains` (Task 3), `ShipBook::answers` (Task 5), and `RoundOptions` (Task 1).
- Produces:
  - `GameError::EmptyPool { min_tier: u32, max_tier: u32, historical: bool }`.
  - `Hint::{Tier(Tier), Nation(Nation)}`, plus the crate-internal `Hint::for_ship(&Reveal, &RoundOptions) -> Hint`.
  - `Draw`, with crate-visible fields `options`, `answers`, `hint` and `reveal`, and accessors `ship() -> &ShipIndex`, `options() -> &RoundOptions`, `answers() -> &BTreeSet<String>`, `hint() -> &Hint` and `reveal() -> &Reveal`.
  - `ShipBook::draw<R: rand::Rng + ?Sized>(&self, &RoundOptions, &RecentShips, &mut R) -> Result<Draw, GameError>`.

- [ ] **Step 1: Add the dependencies and write the failing tests**

In the workspace `Cargo.toml`, under `[workspace.dependencies]`, add this line between `image` and `serde`:

```toml
rand = "0.10"
```

Whole file `crates/barnacle-guess/Cargo.toml`:

```toml
[package]
name = "barnacle-guess"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true

[dependencies]
barnacle-catalog.workspace = true
rand.workspace = true
thiserror.workspace = true
```

Whole file `crates/barnacle-guess/tests/draw.rs`:

```rust
mod common;

use std::collections::BTreeSet;

use barnacle_catalog::Nation;
use barnacle_catalog::ShipClass;
use barnacle_catalog::ShipIndex;
use barnacle_guess::GameError;
use barnacle_guess::Hint;
use barnacle_guess::RecentShips;
use barnacle_guess::Reveal;
use barnacle_guess::RoundOptions;
use barnacle_guess::ShipBook;
use common::book;
use common::fleet;
use common::index;
use common::numbered;
use common::set;
use common::ship;
use common::tier;
use common::with_full_name;
use common::with_nation;
use rand::SeedableRng;
use rand::rngs::StdRng;

fn remembered(numbers: std::ops::Range<usize>) -> RecentShips {
    numbers.fold(RecentShips::default(), |recent, number| {
        recent.remember(numbered(number))
    })
}

fn drawn_over_many_rounds(book: &ShipBook, recent: &RecentShips) -> BTreeSet<ShipIndex> {
    let mut rng = StdRng::seed_from_u64(20260916);
    (0..500)
        .map(|_| {
            book.draw(&RoundOptions::default(), recent, &mut rng)
                .unwrap()
                .ship()
                .clone()
        })
        .collect()
}

#[test]
fn a_pool_of_twenty_is_drawn_in_full_even_when_every_ship_is_recent() {
    let book = book(fleet(20), "");
    assert_eq!(
        drawn_over_many_rounds(&book, &remembered(0..20)),
        (0..20).map(numbered).collect()
    );
}

#[test]
fn a_pool_larger_than_twenty_skips_the_recent_ships() {
    let book = book(fleet(21), "");
    assert_eq!(
        drawn_over_many_rounds(&book, &remembered(0..20)),
        BTreeSet::from([numbered(20)])
    );
}

#[test]
fn recent_ships_outside_the_pool_do_not_shrink_it() {
    let book = book(fleet(25), "");
    let recent = (100..120).fold(RecentShips::default(), |recent, number| {
        recent.remember(numbered(number))
    });
    assert_eq!(
        drawn_over_many_rounds(&book, &recent),
        (0..25).map(numbered).collect()
    );
}

#[test]
fn drawing_from_an_empty_pool_is_an_error() {
    let book = book(vec![ship("PASB005", "New York", 5, "new-york")], "");
    let options = RoundOptions::new(Some(tier(9)), Some(tier(8)), Some(true));
    assert_eq!(
        book.draw(
            &options,
            &RecentShips::default(),
            &mut StdRng::seed_from_u64(1)
        ),
        Err(GameError::EmptyPool {
            min_tier: 8,
            max_tier: 9,
            historical: true,
        })
    );
}

#[test]
fn a_draw_carries_its_options_answers_and_reveal() {
    let book = book(
        vec![with_nation(
            with_full_name(ship("PBSC507", "Belfast", 7, "belfast"), "HMS Belfast"),
            "United_Kingdom",
        )],
        "",
    );
    let options = RoundOptions::new(Some(tier(7)), Some(tier(7)), None);
    let draw = book
        .draw(
            &options,
            &RecentShips::default(),
            &mut StdRng::seed_from_u64(1),
        )
        .unwrap();
    assert_eq!(draw.ship(), &index("PBSC507"));
    assert_eq!(draw.options(), &options);
    assert_eq!(draw.answers(), &set(["belfast", "hmsbelfast"]));
    assert_eq!(
        draw.reveal(),
        &Reveal {
            index: index("PBSC507"),
            name: "HMS Belfast".to_owned(),
            tier: tier(7),
            nation: Nation::new("United_Kingdom"),
            class: ShipClass::Cruiser,
        }
    );
}

#[test]
fn the_hint_is_the_nation_when_the_range_is_a_single_tier() {
    let book = book(
        vec![with_nation(
            ship("PBSC507", "Belfast", 7, "belfast"),
            "United_Kingdom",
        )],
        "",
    );
    let options = RoundOptions::new(Some(tier(7)), Some(tier(7)), None);
    let draw = book
        .draw(
            &options,
            &RecentShips::default(),
            &mut StdRng::seed_from_u64(1),
        )
        .unwrap();
    assert_eq!(draw.hint(), &Hint::Nation(Nation::new("United_Kingdom")));
}

#[test]
fn the_hint_is_the_tier_when_the_range_spans_several_tiers() {
    let book = book(vec![ship("PBSC507", "Belfast", 7, "belfast")], "");
    let draw = book
        .draw(
            &RoundOptions::default(),
            &RecentShips::default(),
            &mut StdRng::seed_from_u64(1),
        )
        .unwrap();
    assert_eq!(draw.hint(), &Hint::Tier(tier(7)));
}
```

The draw tests assert which ships can be drawn, never which ship a given seed yields. A seeded generator's exact sequence can change between `rand` releases, and these tests must survive a `rand` bump.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p barnacle-guess --test draw`
Expected: FAIL to compile with `error[E0432]: unresolved import `barnacle_guess::GameError``, and the same for `Hint`, plus `no method named `draw``. If the first build prints `Downloaded rand v0.10.x`, that is the new dependency arriving.

- [ ] **Step 3: Implement the draw**

Whole file `crates/barnacle-guess/src/error.rs`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum GameError {
    #[error(
        "no ship is eligible for tiers {min_tier} to {max_tier} with historical set to {historical}"
    )]
    EmptyPool {
        min_tier: u32,
        max_tier: u32,
        historical: bool,
    },
}
```

Whole file `crates/barnacle-guess/src/draw.rs`:

```rust
use std::collections::BTreeSet;

use barnacle_catalog::Nation;
use barnacle_catalog::ShipIndex;
use barnacle_catalog::Tier;

use crate::options::RoundOptions;
use crate::reveal::Reveal;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Hint {
    Tier(Tier),
    Nation(Nation),
}

impl Hint {
    pub(crate) fn for_ship(reveal: &Reveal, options: &RoundOptions) -> Self {
        if options.spans_several_tiers() {
            Self::Tier(reveal.tier)
        } else {
            Self::Nation(reveal.nation.clone())
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Draw {
    pub(crate) options: RoundOptions,
    pub(crate) answers: BTreeSet<String>,
    pub(crate) hint: Hint,
    pub(crate) reveal: Reveal,
}

impl Draw {
    pub fn ship(&self) -> &ShipIndex {
        &self.reveal.index
    }

    pub fn options(&self) -> &RoundOptions {
        &self.options
    }

    pub fn answers(&self) -> &BTreeSet<String> {
        &self.answers
    }

    pub fn hint(&self) -> &Hint {
        &self.hint
    }

    pub fn reveal(&self) -> &Reveal {
        &self.reveal
    }
}
```

Whole file `crates/barnacle-guess/src/book.rs`:

```rust
use std::collections::BTreeMap;
use std::collections::BTreeSet;

use barnacle_catalog::Catalog;
use barnacle_catalog::Ship;
use barnacle_catalog::ShipIndex;
use barnacle_catalog::curation::CurationConfig;
use barnacle_catalog::curation::curate;
use barnacle_catalog::names::clean_answer;
use rand::Rng;
use rand::seq::IndexedRandom;

use crate::draw::Draw;
use crate::draw::Hint;
use crate::error::GameError;
use crate::options::RoundOptions;
use crate::recent::RecentShips;
use crate::reveal::Reveal;

#[derive(Debug, Clone, PartialEq, Eq)]
struct Entry {
    reveal: Reveal,
    is_paper: bool,
    answers: BTreeSet<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShipBook {
    entries: BTreeMap<ShipIndex, Entry>,
    lookalikes: BTreeMap<ShipIndex, Vec<ShipIndex>>,
}

impl ShipBook {
    pub fn new(catalog: &Catalog, config: &CurationConfig) -> Self {
        let curated = curate(catalog, config);
        let ships: BTreeMap<&ShipIndex, &Ship> = catalog
            .ships
            .iter()
            .map(|ship| (&ship.index, ship))
            .collect();
        let variants = group_by_key(
            curated
                .removed
                .iter()
                .filter_map(|(index, removal)| removal.base().map(|base| (base, index))),
        );
        let aliases = group_by_key(config.aliases.iter().flat_map(|entry| {
            entry
                .names
                .iter()
                .map(move |name| (&entry.index, name.as_str()))
        }));
        let entries = curated
            .pool
            .iter()
            .filter_map(|index| {
                let ship = ships.get(index)?;
                let name = ship.name.as_ref()?;
                let answers = std::iter::once(index)
                    .chain(variants.get(index).into_iter().flatten().copied())
                    .flat_map(|member| cleaned_names(member, &ships, &aliases))
                    .collect();
                let reveal = Reveal {
                    index: index.clone(),
                    name: name.display().to_owned(),
                    tier: ship.tier,
                    nation: ship.nation.clone(),
                    class: ship.class.clone(),
                };
                Some((
                    index.clone(),
                    Entry {
                        reveal,
                        is_paper: ship.is_paper,
                        answers,
                    },
                ))
            })
            .collect();
        let lookalikes = config
            .lookalikes
            .iter()
            .flat_map(|group| {
                group.ships.iter().map(|member| {
                    let others = group
                        .ships
                        .iter()
                        .filter(|other| *other != member)
                        .cloned()
                        .collect::<Vec<_>>();
                    (member.clone(), others)
                })
            })
            .collect();
        Self {
            entries,
            lookalikes,
        }
    }

    pub fn pool(&self, options: &RoundOptions) -> Vec<&ShipIndex> {
        self.eligible(options).map(|(index, _)| index).collect()
    }

    pub fn answers(&self, index: &ShipIndex, options: &RoundOptions) -> BTreeSet<String> {
        let lookalikes = self
            .lookalikes
            .get(index)
            .into_iter()
            .flatten()
            .filter_map(|other| self.entries.get(other))
            .filter(|entry| options.allows(entry.reveal.tier, entry.is_paper));
        self.entries
            .get(index)
            .into_iter()
            .chain(lookalikes)
            .flat_map(|entry| entry.answers.iter().cloned())
            .collect()
    }

    pub fn draw<R: Rng + ?Sized>(
        &self,
        options: &RoundOptions,
        recent: &RecentShips,
        rng: &mut R,
    ) -> Result<Draw, GameError> {
        let eligible: Vec<(&ShipIndex, &Entry)> = self.eligible(options).collect();
        let fresh: Vec<(&ShipIndex, &Entry)> = if eligible.len() > RecentShips::LIMIT {
            eligible
                .into_iter()
                .filter(|(index, _)| !recent.contains(index))
                .collect()
        } else {
            eligible
        };
        let (index, entry) = fresh.choose(rng).copied().ok_or(GameError::EmptyPool {
            min_tier: options.min_tier().get(),
            max_tier: options.max_tier().get(),
            historical: options.historical(),
        })?;
        Ok(Draw {
            options: *options,
            answers: self.answers(index, options),
            hint: Hint::for_ship(&entry.reveal, options),
            reveal: entry.reveal.clone(),
        })
    }

    fn eligible(&self, options: &RoundOptions) -> impl Iterator<Item = (&ShipIndex, &Entry)> {
        let options = *options;
        self.entries
            .iter()
            .filter(move |(_, entry)| options.allows(entry.reveal.tier, entry.is_paper))
    }
}

fn group_by_key<K: Ord, V>(pairs: impl Iterator<Item = (K, V)>) -> BTreeMap<K, Vec<V>> {
    pairs.fold(BTreeMap::new(), |mut grouped, (key, value)| {
        grouped.entry(key).or_insert_with(Vec::new).push(value);
        grouped
    })
}

fn cleaned_names(
    index: &ShipIndex,
    ships: &BTreeMap<&ShipIndex, &Ship>,
    aliases: &BTreeMap<&ShipIndex, Vec<&str>>,
) -> Vec<String> {
    let official = ships
        .get(index)
        .and_then(|ship| ship.name.as_ref())
        .into_iter()
        .flat_map(|name| std::iter::once(name.short.as_str()).chain(name.full.as_deref()));
    let extra = aliases.get(index).into_iter().flatten().copied();
    official
        .chain(extra)
        .map(clean_answer)
        .filter(|answer| !answer.is_empty())
        .collect()
}
```

Whole file `crates/barnacle-guess/src/lib.rs`:

```rust
mod book;
mod draw;
mod error;
mod ids;
mod options;
mod recent;
mod reveal;

pub use book::ShipBook;
pub use draw::Draw;
pub use draw::Hint;
pub use error::GameError;
pub use ids::Snowflake;
pub use ids::UserId;
pub use options::RoundOptions;
pub use recent::RecentShips;
pub use reveal::Reveal;
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p barnacle-guess && cargo clippy -p barnacle-guess --all-targets -- -D warnings`
Expected: `draw` 7 passed and every earlier target still passes. Clippy reports no warnings.

- [ ] **Step 5: Check the new dependency's licenses**

Run: `cargo deny check licenses`
Expected: `licenses ok`. If a license outside the allowlist appears, stop and ask the owner.

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml Cargo.lock crates/barnacle-guess/Cargo.toml crates/barnacle-guess/src/error.rs crates/barnacle-guess/src/draw.rs crates/barnacle-guess/src/book.rs crates/barnacle-guess/src/lib.rs crates/barnacle-guess/tests/draw.rs
git commit -m "feat(guess): draw a ship without recent repeats" -- Cargo.toml Cargo.lock crates/barnacle-guess/Cargo.toml crates/barnacle-guess/src/error.rs crates/barnacle-guess/src/draw.rs crates/barnacle-guess/src/book.rs crates/barnacle-guess/src/lib.rs crates/barnacle-guess/tests/draw.rs
```

---

### Task 7: Rounds, guesses and cancelling

**Files:**
- Create: `crates/barnacle-guess/src/round.rs`
- Modify: `crates/barnacle-guess/src/lib.rs`
- Test: `crates/barnacle-guess/tests/round.rs`

**Interfaces:**
- Consumes: `Draw` and its crate-visible `answers` field (Task 6), `Snowflake::elapsed_until` and `UserId` (Task 2), and `clean_answer`.
- Produces:
  - `Draw::start(self, invoker: UserId, posted: Snowflake) -> Round`.
  - `Round`, with `draw() -> &Draw`, `invoker() -> UserId`, `posted() -> Snowflake`, `judge(&self, &Guess<'_>) -> Option<Solve>` and `may_cancel(&self, user: UserId, can_manage_messages: bool) -> bool`.
  - `Guess<'a> { pub author: UserId, pub author_is_bot: bool, pub message: Snowflake, pub text: &'a str }`.
  - `Solve { pub winner: UserId, pub ship: ShipIndex, pub elapsed: Duration }`.
  - `Timing { pub before_hint: Duration, pub after_hint: Duration }`, with `Timing::STANDARD` set to 20 s and 10 s.

- [ ] **Step 1: Write the failing tests**

Whole file `crates/barnacle-guess/tests/round.rs`:

```rust
mod common;

use std::time::Duration;

use barnacle_guess::Guess;
use barnacle_guess::RecentShips;
use barnacle_guess::Round;
use barnacle_guess::RoundOptions;
use barnacle_guess::Solve;
use barnacle_guess::Timing;
use barnacle_guess::UserId;
use common::at;
use common::book;
use common::index;
use common::ship;
use common::tier;
use rand::SeedableRng;
use rand::rngs::StdRng;

const INVOKER: UserId = UserId::new(10);
const PLAYER: UserId = UserId::new(20);
const MODERATOR: UserId = UserId::new(30);

fn started() -> Round {
    let options = RoundOptions::new(Some(tier(5)), Some(tier(5)), None);
    book(vec![ship("PJSB005", "Kongō", 5, "kongo")], "")
        .draw(
            &options,
            &RecentShips::default(),
            &mut StdRng::seed_from_u64(1),
        )
        .unwrap()
        .start(INVOKER, at(1_000))
}

fn guess(text: &str, author_is_bot: bool) -> Guess<'_> {
    Guess {
        author: PLAYER,
        author_is_bot,
        message: at(4_250),
        text,
    }
}

#[test]
fn a_correct_guess_solves_the_round_with_the_time_since_the_post() {
    assert_eq!(
        started().judge(&guess("kongo", false)),
        Some(Solve {
            winner: PLAYER,
            ship: index("PJSB005"),
            elapsed: Duration::from_millis(3_250),
        })
    );
}

#[test]
fn guesses_are_cleaned_before_they_are_compared() {
    let round = started();
    assert!(round.judge(&guess("  KONGŌ ", false)).is_some());
    assert!(round.judge(&guess("Kon-go.", false)).is_some());
}

#[test]
fn wrong_empty_and_bot_guesses_do_not_solve_the_round() {
    let round = started();
    assert_eq!(round.judge(&guess("kirishima", false)), None);
    assert_eq!(round.judge(&guess("...", false)), None);
    assert_eq!(round.judge(&guess("kongo", true)), None);
}

#[test]
fn the_invoker_or_a_member_who_manages_messages_may_cancel() {
    let round = started();
    assert!(round.may_cancel(INVOKER, false));
    assert!(round.may_cancel(MODERATOR, true));
    assert!(!round.may_cancel(MODERATOR, false));
}

#[test]
fn a_round_keeps_its_draw_invoker_and_post() {
    let round = started();
    assert_eq!(round.draw().ship(), &index("PJSB005"));
    assert_eq!(round.invoker(), INVOKER);
    assert_eq!(round.posted(), at(1_000));
}

#[test]
fn the_standard_timing_is_twenty_seconds_then_ten_after_the_hint() {
    assert_eq!(
        Timing::STANDARD,
        Timing {
            before_hint: Duration::from_secs(20),
            after_hint: Duration::from_secs(10),
        }
    );
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p barnacle-guess --test round`
Expected: FAIL to compile with `error[E0432]: unresolved import `barnacle_guess::Guess``, and the same for `Round`, `Solve` and `Timing`.

- [ ] **Step 3: Implement the round**

Whole file `crates/barnacle-guess/src/round.rs`:

```rust
use std::time::Duration;

use barnacle_catalog::ShipIndex;
use barnacle_catalog::names::clean_answer;

use crate::draw::Draw;
use crate::ids::Snowflake;
use crate::ids::UserId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Timing {
    pub before_hint: Duration,
    pub after_hint: Duration,
}

impl Timing {
    pub const STANDARD: Self = Self {
        before_hint: Duration::from_secs(20),
        after_hint: Duration::from_secs(10),
    };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Guess<'a> {
    pub author: UserId,
    pub author_is_bot: bool,
    pub message: Snowflake,
    pub text: &'a str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Solve {
    pub winner: UserId,
    pub ship: ShipIndex,
    pub elapsed: Duration,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Round {
    draw: Draw,
    invoker: UserId,
    posted: Snowflake,
}

impl Draw {
    pub fn start(self, invoker: UserId, posted: Snowflake) -> Round {
        Round {
            draw: self,
            invoker,
            posted,
        }
    }
}

impl Round {
    pub fn draw(&self) -> &Draw {
        &self.draw
    }

    pub fn invoker(&self) -> UserId {
        self.invoker
    }

    pub fn posted(&self) -> Snowflake {
        self.posted
    }

    pub fn judge(&self, guess: &Guess<'_>) -> Option<Solve> {
        let correct = !guess.author_is_bot && self.draw.answers.contains(&clean_answer(guess.text));
        correct.then(|| Solve {
            winner: guess.author,
            ship: self.draw.ship().clone(),
            elapsed: self.posted.elapsed_until(guess.message),
        })
    }

    pub fn may_cancel(&self, user: UserId, can_manage_messages: bool) -> bool {
        user == self.invoker || can_manage_messages
    }
}
```

Whole file `crates/barnacle-guess/src/lib.rs`:

```rust
mod book;
mod draw;
mod error;
mod ids;
mod options;
mod recent;
mod reveal;
mod round;

pub use book::ShipBook;
pub use draw::Draw;
pub use draw::Hint;
pub use error::GameError;
pub use ids::Snowflake;
pub use ids::UserId;
pub use options::RoundOptions;
pub use recent::RecentShips;
pub use reveal::Reveal;
pub use round::Guess;
pub use round::Round;
pub use round::Solve;
pub use round::Timing;
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p barnacle-guess && cargo clippy -p barnacle-guess --all-targets -- -D warnings`
Expected: `round` 6 passed and every earlier target still passes. Clippy reports no warnings.

- [ ] **Step 5: Commit**

```bash
git add crates/barnacle-guess/src/round.rs crates/barnacle-guess/src/lib.rs crates/barnacle-guess/tests/round.rs
git commit -m "feat(guess): judge guesses and permit cancelling a round" -- crates/barnacle-guess/src/round.rs crates/barnacle-guess/src/lib.rs crates/barnacle-guess/tests/round.rs
```

---

### Task 8: Real catalog check and full verification

**Files:**
- Test: `crates/barnacle-guess/tests/real_catalog.rs`

**Interfaces:**
- Consumes: the whole public API, a built catalog directory holding `catalog.json`, and the repository's `curation/ships.toml`.
- Produces: no new API. This test is the only check that the real 15.8.0 catalog and the real curation file produce a usable game.

This test guards data, not new behaviour, so there is no failing step. Every rule it relies on was proved red and then green in Tasks 1-7.

- [ ] **Step 1: Write the test**

Whole file `crates/barnacle-guess/tests/real_catalog.rs`:

```rust
use std::path::Path;

use barnacle_catalog::Catalog;
use barnacle_catalog::Tier;
use barnacle_catalog::curation::CurationConfig;
use barnacle_catalog::names::clean_answer;
use barnacle_guess::RecentShips;
use barnacle_guess::RoundOptions;
use barnacle_guess::ShipBook;
use rand::SeedableRng;
use rand::rngs::StdRng;

#[test]
fn every_ship_in_the_real_pool_can_be_drawn_and_answered() {
    let Some(dir) = std::env::var_os("BARNACLE_TEST_CATALOG_DIR") else {
        eprintln!(
            "skipping: set BARNACLE_TEST_CATALOG_DIR to the absolute path of a built catalog"
        );
        return;
    };
    let catalog =
        Catalog::from_json(&std::fs::read_to_string(Path::new(&dir).join("catalog.json")).unwrap())
            .unwrap();
    let curation = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../curation/ships.toml");
    let config = CurationConfig::from_toml(&std::fs::read_to_string(curation).unwrap()).unwrap();
    let book = ShipBook::new(&catalog, &config);

    let every_tier = RoundOptions::new(Tier::new(1).ok(), Tier::new(11).ok(), None);
    let pool = book.pool(&every_tier);
    assert!(!pool.is_empty());
    for index in &pool {
        let answers = book.answers(index, &every_tier);
        let name = catalog
            .get(index)
            .and_then(|ship| ship.name.as_ref())
            .unwrap();
        assert!(
            answers.contains(&clean_answer(&name.short)),
            "{index} does not accept its own short name {:?}",
            name.short
        );
        assert!(
            answers.iter().all(|answer| answer.is_ascii()
                && !answer.is_empty()
                && !answer
                    .chars()
                    .any(|c| c.is_whitespace() || c.is_ascii_uppercase())),
            "{index} has an answer that is not cleaned: {answers:?}"
        );
    }

    let default_pool = book.pool(&RoundOptions::default()).len();
    let historical_pool = book.pool(&RoundOptions::new(None, None, Some(true))).len();
    assert!(historical_pool < default_pool);
    let draw = book
        .draw(
            &RoundOptions::default(),
            &RecentShips::default(),
            &mut StdRng::seed_from_u64(1),
        )
        .unwrap();
    assert!(!draw.answers().is_empty());
    eprintln!(
        "pool sizes: every tier {}, default {default_pool}, default historical {historical_pool}",
        pool.len()
    );
}
```

The test does not assert that answers contain only letters and digits. Real ship names contain `(`, `)`, `[`, `]`, `+`, `#` and `<` (counted in `15.8.0_13187581_r4` on 2026-09-16), and cleaning deliberately keeps those characters (spec 5.5).

- [ ] **Step 2: Run it without data, then against the real catalog**

Run: `cargo test -p barnacle-guess --test real_catalog -- --nocapture`
Expected: `1 passed`, with a `skipping:` line.

Run: `BARNACLE_TEST_CATALOG_DIR="$PWD/data/catalog/$(cat data/catalog/current)" cargo test -p barnacle-guess --test real_catalog -- --nocapture`
Expected: `1 passed` and no `skipping:` line. For `15.8.0_13187581_r4` with the current `curation/ships.toml`, the line printed is `pool sizes: every tier 791, default 614, default historical 284`, measured on 2026-09-16 with this plan's code. Record the numbers you see. If they differ, note it in the task report, because the catalog or the curation file has changed since.

- [ ] **Step 3: Run the full workspace checks CI runs**

Run: `cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace && cargo deny check licenses`
Expected: no formatting diff, no clippy warnings, and every test passes: the 86 from Plan 1 plus 38 from this plan, 124 in total. Then `licenses ok`.

- [ ] **Step 4: Commit**

```bash
git add crates/barnacle-guess/tests/real_catalog.rs
git commit -m "test(guess): check the real catalog can be drawn and answered" -- crates/barnacle-guess/tests/real_catalog.rs
```

Pushing the branch and opening the pull request are separate steps, and only happen when the owner asks.

---

## What Plan 3 takes from this crate

- **At startup:** load the catalog named by `data/catalog/current` and `curation/ships.toml`, then build one `ShipBook`.
- **Per channel:** keep a `RecentShips` value and a lock holding the active `Round`, both in memory (spec 6).
- **`/guess`:**
  1. Build `RoundOptions::new` from the command options.
  2. Call `book.draw(options, recent, rng)`. On `GameError::EmptyPool`, reply with the bot's own message.
  3. Post `data/catalog/<name>/silhouettes/<index>.png`.
  4. Call `draw.start(invoker, posted_message_id)`.
  5. Store `recent.remember(index)`.
- **Timers:** after `Timing::STANDARD.before_hint`, post `round.draw().hint()`. After `after_hint` more, post `round.draw().reveal()` and end the round.
- **Messages:** pass each message in the channel to `round.judge`, marking bot authors as bots. The first `Some(Solve)` ends the round and becomes one `guess_solves` row. Deciding whether it is a new best time is a database query in Plan 3.
- **Cancel:** call `round.may_cancel(user, has_manage_messages)`. If it returns true, post the reveal and end the round.

## Self-review

| Spec 5.5 rule | Task |
|---|---|
| Options: tiers 1-11, defaults 6 and 11, swap, historical off | 1 |
| Pool: eligible bases in range, paper removed when historical | 4 |
| Empty pool is an error, not a panic | 6 |
| Draw: uniform, skip the last 20 when the pool is larger than 20 | 3, 6 |
| Answers: short and full names, variants, look-alikes allowed by the options, aliases | 5 |
| Cleaning, including transliteration | 5 (names), 7 (guesses); both reuse `clean_answer` |
| Timing: 20 s, hint, 10 s | 7 |
| Hint: tier across several tiers, otherwise nation | 6 |
| Elapsed time from the two message IDs | 2, 7 |
| Who may answer: non-bot users | 7 |
| Cancel: the invoker or Manage Messages | 7 (permission). Showing the reveal is the bot's job in Plan 3. |
| Reveal: name, tier, nation, class (spec 5.2) | 4 |
| Spec 8 test targets: look-alike tier boundaries, reversed ranges, no-repeat memory, empty pool | 5, 1, 3 and 6, 6 |

**How the plan's code was checked (2026-09-16):**
- **Build and tests:** the code was built in a scratch copy against `barnacle-catalog` on `main`. The final code passed rustfmt and clippy with `-D warnings`, and all 38 tests passed.
- **Real data:** the real-catalog test passed against `15.8.0_13187581_r4`.
- **Deliberate breaks:** fourteen rules were broken one at a time, and each break made at least one test fail:
  - the recent-ship skip, the pool-size boundary and the memory length;
  - the historical filter, the inclusive tier range and the swap of a reversed range;
  - look-alike filtering, variant names, aliases, full names and the empty-name filter;
  - the bot filter, cancel permission and the hint choice.
- **Task-by-task replay:** a script applied this plan task by task to a clean copy of `main` (a54573d).
  - Every "verify they fail" step failed with the named compiler error.
  - Every "verify they pass" step passed tests and clippy.
  - `cargo deny check licenses` passed.
  - The final workspace passed all 124 tests.
  - The replayed crate was file-for-file identical to the prototype that passed the deliberate-break checks.
- **Not checked:** the commit steps. The replay ran outside git.
