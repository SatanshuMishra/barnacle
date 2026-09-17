# Spec: Barnacle's ship silhouette game

Date: 2026-09-16
Status: approved on 2026-09-16. Every question in section 10 is decided. Implementation of Plan 1 (the data pipeline) started on 2026-09-16.
Findings this spec relies on: `docs/research/2026-09-16-track-silhouette-audit.md` (called "the audit" below). Requirement IDs R1-R14 are defined in section 6 of the audit. Measurements marked "13.11 data" were taken on track's last committed ship data, the newest per-ship data available without building a catalog.

## 1. Goal

Rebuild track's silhouette guessing game and its ship lookup for Barnacle, with ship data and silhouettes coming from wows-toolkit's crates and published data. A new game version or a toolkit release should reach the bot with a dependency bump, one command and a short curation review, never a hand-copied file.

Barnacle will grow beyond this game, so the game is the first module of a general bot, not the whole bot. The core idea is kept intact: a silhouette appears, anyone in the channel types the ship's name, the fastest correct answer wins, a hint arrives if nobody gets it, and the winner's time is recorded.

## 2. Architecture

### 2.1 Language: Rust (decided)

The toolkit is Rust-only, so a Rust bot calls it directly. When a toolkit release changes an API, the bot's build fails at the exact call site rather than drifting silently at runtime. The cost is slower iteration on Discord UI code and a heavier CI build.

Discord library: `poise` on top of `serenity`. `poise` 0.7.0 was released 2026-09-06 under MIT and `serenity` 0.12.5 under ISC ([crates.io](https://crates.io/crates/poise), [crates.io](https://crates.io/crates/serenity)). `poise` 0.7.0 requires `serenity` `^0.12.5`, which is the newest serenity release ([crates.io](https://crates.io/api/v1/crates/poise/0.7.0/dependencies), checked 2026-09-16).

### 2.2 Hosting: local (decided)

The bot runs as one process on the owner's machine. That makes an in-memory channel lock and a local SQLite file sufficient, and it makes "deploy" mean "run `barnacle-data sync` on that machine".

### 2.3 Workspace layout

```
barnacle/
  Cargo.toml                   workspace; pins the wows-toolkit crate versions in one place
  crates/
    barnacle-catalog/          library: catalog model, name cleaning, curation rules, validation, diffs (no toolkit, no Discord)
    barnacle-data/             binary: download, catalog extraction through the toolkit, sync, validate, diff, use
    barnacle-bot/              binary: Discord bot; loads a finished catalog, never parses GameParams
    barnacle-guess/            library: game rules (pool, answers, hints, timing), no Discord types
  curation/
    ships.toml                 groups allowlist, exclusions, look-alike groups, aliases
  data/                        gitignored; the wows-data-mgr store and built catalogs
```

Only `barnacle-data` depends on wows-toolkit, so a breaking upstream change touches one crate, and the bot never compiles the toolkit. The bot and the game rules depend only on `barnacle-catalog`.

The curation file is named `ships.toml` rather than after the game, because the same exclusions and look-alike groups will serve any later ship feature.

### 2.4 Data flow

```
landaire/wows-replay-data --(wows-data-mgr download_build)--> data/store/<version>_<build>/
        |
        v
barnacle-data: Dump::open -> vfs
   content/GameParams.data --> wowsunpack params_from_data --> Vehicle: level, group, species, nation, index, id
                           \-> game_params_to_pickle ------> isPaperShip (raw tree; section 3.2)
   translations/en/LC_MESSAGES/global.mo --> gettext --> IDS_<index>, IDS_<index>_FULL
   gui/ships_silhouettes/<index>.png --> SHA-256 --> silhouette hash
        |
        v
data/catalog/<version>_<build>_r<n>/catalog.json + silhouettes/ (composited PNGs; a new revision directory for every build)
        |
        v
barnacle-bot loads the catalog named by data/catalog/current at startup
```

## 3. The catalog

### 3.1 Record per ship

| Field | Type | Source |
|---|---|---|
| `id` | `ParamId` newtype | `Param::id()` |
| `index` | `ShipIndex` newtype (7 chars) | `Param::index()` |
| `tier` | `Tier` newtype, 1-11 | `Vehicle::level()` |
| `group` | `ShipGroup` newtype over WG's string | `Vehicle::group()` |
| `class` | `ShipClass` enum, with `Other(String)` and `Unspecified` arms | `Param::species()` |
| `nation` | `Nation` newtype over WG's string | `Param::nation()` |
| `is_paper` | bool | raw GameParams tree |
| `name` | `Option<ShipName { short, full: Option<String> }>` | `IDS_<index>` and `IDS_<index>_FULL` in the `en` gettext catalog |
| `silhouette` | `Option<Silhouette { sha256 }>` | SHA-256 of the silhouette PNG, absent when the dump has none |

The catalog also records the game version, build number, the wows-toolkit crate versions, and the data repository commit it was built from. The bot reports these in `/about`, so any wrong answer can be traced to its data.

Groups and nations are kept as WG's own strings. The curation diff lists every group it has not seen before, so a new WG group name gets a decision instead of silently vanishing from the pool or silently entering it. A ship index that is not 7 uppercase letters or digits, or a tier outside 1-11, fails the whole build loudly (both held for every ship in 13.11 data).

### 3.2 Filling the paper-ship gap

wowsunpack does not parse `isPaperShip` (audit section 3.3). Two ways to fill it:

1. **Now:** read it from the raw tree returned by `game_params_to_pickle`. wowsunpack's own parser shows the traversal to mirror: unwrap the root dictionary, including its empty-string wrapper key on modern builds, then read each entry's `typeinfo.type`, `index` and `isPaperShip` (`wows-toolkit/crates/wowsunpack/src/game_params/provider.rs:1721-1760`). This is roughly 40 lines. It needs a direct dependency on `pickled`, pinned to the exact version wowsunpack uses (2.0.0-alpha10 today), so the two agree on the value type. A mismatch fails the build rather than misreading data.
2. **Upstream:** open a pull request against landaire/wows-toolkit that adds `is_paper_ship` to `Vehicle`, following that repository's `AGENTS.md`. Once it ships, delete path 1 and the `pickled` pin.

Do both, and keep path 1 behind one function so removing it is a one-line change.

### 3.3 Silhouette compositing

Silhouettes are composited once, at catalog build time, not per game, so the bot only reads a finished PNG. The background is a flat colour from Barnacle's own palette rather than WG's `ship_background.png`. That gives the bot its own look and uses one fewer WG asset. How WG's silhouette PNGs look against a flat colour is **unverified**; check a handful in step 2 of section 9 and pick the colour then.

## 4. Curation (`curation/ships.toml`)

### 4.1 Contents

| Key | Meaning | Built from |
|---|---|---|
| `groups` | Allowlist of eligible ship groups | The groups WG uses for ships players can own |
| `exclude` | Ships the automatic rules miss, each with a `reason`: `carbon_copy`, `bad_silhouette` | Barnacle's own review of what 4.2 leaves |
| `keep` | Ships an automatic rule flagged wrongly | Barnacle's own review |
| `lookalikes` | Groups of distinct ships accepted for each other | Barnacle's own review, helped by the candidates in 4.4 |
| `aliases` | Extra accepted strings per index | Barnacle's own review |
| `reviewed_through` | Last build whose new ships were reviewed; absent until the first review | Set by the reviewer |

None of these lists is copied from track's `guess.toml` (section 11). Track's lists are used only to measure how well the automatic rules perform.

### 4.2 Automatic rules

Applied in order when the catalog is built. A ship removed by rules 3-5 is a **variant** of a base ship: it is never drawn, and its names count as correct answers whenever its base ship is drawn.

1. **Group.** Drop any ship whose group is not in `groups`. This removes every test ship, since `Vehicle::is_test_ship()` is true for every `demo*` group.
2. **Name and silhouette.** Drop any ship without an English name (it cannot be answered) or without a silhouette file. Also drop every ship whose silhouette file is identical to one excluded for bad art, since it shares the same bad image.
3. **Identical silhouette.** Group ships whose silhouette files hash the same. Keep one base per group: the one in `upgradeable`, `start` or `special`, in that order, then the lowest index.
4. **Collaboration prefix.** A ship whose English name starts with the word `ARP`, `AL`, `HSF`, `BA` or `STAR` is a collaboration reskin. Its base is the ship with the identical silhouette if rule 3 found one; otherwise it is excluded with no base, so no extra names are accepted.
5. **Variant suffix.** A ship whose English name ends with the word `B`, `Golden`, `CLR` or `Beta`, where the name without that word is another eligible ship's name, is a variant of that ship.
6. **Manual.** Apply `exclude`, then `keep`. A manually excluded ship is never chosen as a base by rules 3 and 5.
7. **Chains.** When a removed ship's base was itself removed, it points through to the first base that is in the pool, recorded as a copy of a copy.

Rules 4 and 5 are narrow on purpose. Matching any ship whose name contains another ship's name is unsafe: it would wrongly pair Black Swan with Black, Konig Albert with Konig, and Vampire II with Vampire (13.11 data).

Measured on the 99 ships track excluded from its allowed groups (13.11 data):

| Rule | Track exclusions it catches | Ships it flags that track kept |
|---|---|---|
| 3. Identical silhouette | 47 | 6 pairs where track kept both, although the silhouettes are the same file (audit section 4) |
| 4. Collaboration prefix | 30 | AL Agir, which rule 3 also shows is a copy of Agir |
| 5. Variant suffix | 37 | Georg Hoffmann Golden and Prins van Oranje Golden, which rule 3 also shows are copies |
| Rules 3-5 together | **75 of 99** | none that a player could tell apart from their base ship |

The 24 left are mostly event reskins with unique names (for example the Lunar New Year ships) and five ships track excluded for bad silhouette art. They go to manual review, together with every ship added since 13.11. A pixel-level comparison would likely shrink that list further, but it is **unverified** and not needed for launch.

### 4.3 Validation (R11)

`barnacle-data validate` fails, and CI fails with it, when any of these is true:

- An index in `exclude`, `keep`, `lookalikes` or `aliases` is not in the catalog. This is the check that would have caught D1.
- An index appears twice in `exclude`, or in two `lookalikes` groups. This would have caught D2.
- A `lookalikes` group has fewer than two eligible members.
- An excluded ship is also in a `lookalikes` group.
- `reviewed_through` is absent, or lower than the catalog's build number.

The last check forces a curation review on every game update without anyone having to remember it.

### 4.4 Finding look-alike candidates

Because track's 133 similar groups are not copied, Barnacle needs its own way to find look-alikes: ships that are genuinely different but hard to tell apart in silhouette. `barnacle-data diff` prints candidates from these sources so the reviewer never starts from a blank page:

1. **Year-suffixed refits.** A ship named like another plus a year, such as Belfast '43 or Renown '44. There are 10 in the pool (13.11 data).
2. **Near-identical silhouettes.** Alpha masks that differ in only a small share of pixels. The threshold is **unverified** and has to be measured.
3. **Shared hull models.** `Vehicle` carries a `model_path` (`wows-toolkit/crates/wowsunpack/src/game_params/types.rs:1427`). Whether sister ships share paths is **unverified**.

The game launches with whatever look-alike groups have been reviewed by then. An incomplete list makes a few rounds strict, not broken, so it does not block launch.

## 5. Game design (decided)

This section answers Q4; the owner approved it on 2026-09-16, keeping the command name `/guess`. Each row is track's behaviour, what Barnacle does, and why. "Keep" means the same behaviour, reimplemented; "change" means a small improvement; "drop" and "later" shrink the launch.

### 5.1 Command and options

| Setting | Track | Barnacle | Why |
|---|---|---|---|
| Command | `/guess` | **Keep** `/guess` | The owner's choice. Later rounds such as flags or maps can get their own commands; the data dump already carries nation flags and minimap images (`wows-toolkit/crates/wows-data-mgr/src/dump.rs:26-40`). |
| Difficulty | easy / normal / hard, default normal | **Drop** for launch | It only changes which look-alike names count, and nothing else. Hard turns identical-looking pairs into a coin flip. Easy differs from normal only for look-alikes outside the chosen tiers. The look-alike list starts small, so all three would behave the same at launch. |
| Tier range | 1-11, defaults 6 to 11 | **Keep**, named `min_tier` and `max_tier` | 6-11 is 500 of 672 eligible ships (13.11 data). Discord enforces the 1-11 bounds. |
| Min above max | Refused with an error | **Change:** swap them | The intent is unambiguous, and the round's embed shows the range used. |
| Historical | Off; on removes paper ships | **Keep** as an on/off option, default off | Paper ships are 263 of the 500 default-range ships (53%, 13.11 data), so this option changes the game noticeably. It costs the 40-line read in 3.2. |

### 5.2 A round

| Setting | Track | Barnacle | Why |
|---|---|---|---|
| Draw | Uniform over the pool | **Keep**, **plus** no repeat of the channel's last 20 ships | From 500 ships, the chance of at least one repeat within 20 rounds is 32%. Players notice repeats. The memory is per channel and resets on restart. |
| One round per channel | Yes | **Keep** | Stops overlapping answers |
| Who may answer | Anyone in the channel | **Keep**, ignoring bots | The social race is the core of the game. Ignoring bots fixes D5. |
| Answer window | 20 s, then a hint, then 10 s | **Keep** | No evidence to change it. Revisit with real round times from the solves table. |
| Hint | Tier if the range spans several tiers, otherwise nation | **Keep** | The hint always narrows the field, never repeats what the options already said. |
| Matching | Exact after cleaning; short or full name; ASCII transliteration | **Keep** exact; English only (Q5) | Typo tolerance would accept wrong ships: 16 pairs of eligible names are one typo apart, including Lion and Lyon, Maine and Mainz, and North and South Carolina (13.11 data). |
| Look-alikes | Depends on difficulty | **One rule:** a look-alike's name counts if that ship could have been drawn with this round's tier range and historical setting | It is normal mode's logic, which is the only fair one. If Texas is outside the chosen tiers, the silhouette cannot be Texas. |
| Variant names | Not accepted | **Change:** accepted for their base ship | Variants are never drawn, so accepting their names costs nothing and avoids rejecting a player who knows the reskin. |
| Wrong guesses | Ignored silently | **Keep** | Reacting to every wrong guess would flood the channel. |
| Cancel | Invoker only | **Change:** invoker, or anyone with Discord's Manage Messages permission | Moderators can clear an abandoned round. |
| Reveal | "The answer was" plus the name | **Change:** name, tier, nation and class | Uses data the catalog already has, and teaches the player something. |
| Win message | Time to 3 decimals, plus "new record" | **Keep** the content, in Barnacle's own wording (section 11.2) | |
| Timing | Game post to winning message, by message ID | **Keep** | Bot latency does not count against the player. |

### 5.3 Stats and other commands

| Setting | Track | Barnacle | Why |
|---|---|---|---|
| Stats stored | Two counters per user: solves and best time | **Change:** one row per solve: user, server, ship, time taken, timestamp | Same effort now. Solve count and best time are queries over it, and leaderboards and streaks can be added later without a schema change. |
| Profile | `/profile` shows solves and best time | **Keep** | |
| Leaderboard | None | **Later** | The per-solve rows make it a query, not a migration. |
| Ship lookup | `/inspect <ship>` | **Keep** as `/ship info <ship>` | Players use it, and it is the curation tool. It also shows why a ship is excluded or which ship it is a variant of. |
| Server enable and disable | Custom per-server settings | **Replace** with Discord's per-command permissions | Already decided (section 6) |
| Language | Per-user setting | **Drop** | English only (Q5) |

### 5.4 Launch scope

| At launch | Later |
|---|---|
| `/guess` with tier range and historical | A difficulty that players feel, such as no hint or a shorter clock |
| Automatic curation rules 1-5, plus reviewed leftovers | Pixel-level and model-based look-alike candidates |
| No-repeat memory, bot filter, moderator cancel | Leaderboards and streaks |
| Solves table, `/profile`, `/ship info`, `/about` | Other `/guess` rounds (flags, maps) |

### 5.5 Rules summary for `barnacle-guess`

This crate is pure logic with an injected clock and random source, so every rule can be tested without Discord.

| Rule | Behaviour | Requirement |
|---|---|---|
| Options | `min_tier`, `max_tier` 1-11, defaults 6 and 11, swapped if reversed; `historical` default false | R1 |
| Pool | Eligible base ships in the tier range, minus paper ships when historical; an empty pool is an error message, not a panic | R2 |
| Draw | Uniform over the pool, skipping the channel's last 20 ships when the pool is larger than 20 | R2 |
| Answers | The drawn ship's English short and full names, its variants' names, look-alikes allowed by the options, and aliases, all cleaned, with their ASCII transliterations | R5, R6 |
| Cleaning | Transliterate to ASCII, lowercase, remove whitespace and `- . ' ,` (no English name uses a middle dot) | R5 |
| Timing | 20 s, then a hint, then 10 s, then the answer is revealed | R7 |
| Hint | Tier when the range spans several tiers, otherwise nation | R7 |
| Elapsed time | Difference between the round post's and the winning message's snowflake timestamps | R8 |
| Who may answer | Any non-bot user in the channel | fixes D5 |
| Cancel | The invoker, or a member with Manage Messages; reveals the answer | R9 |

Romanization stays even with English-only answers, because some English names contain accented letters; the English name of `PGSC519` is written with an umlaut.

## 6. Bot (`barnacle-bot`)

The detailed design is `docs/superpowers/specs/2026-09-16-barnacle-bot-design.md`.

- **Commands:** `/guess` (section 5), `/ship info <ship>` (R14), `/profile` (R10), `/about` (catalog provenance and Wargaming's non-affiliation notice).
- **Channel lock (R3):** an in-memory map from channel ID to active round, enough for one process. The game module must not depend on it being in memory, which fixes D6 by design.
- **Persistence (R10):** a local SQLite file with one `guess_solves` table: user ID, server ID, ship index, time taken in milliseconds, solved-at timestamp. The migration SQL is authored as a file and applied by a person, per this project's rules.
- **Language:** English only, for answers and for the bot's own text. There is no language setting, which removes D4 entirely.
- **Enabling and disabling:** Discord's own per-command permissions.
- **Catalog reload:** the bot reads the catalog at startup; a new catalog takes effect on restart.

## 7. Update procedure

### 7.1 A new game version

```bash
cargo run -p barnacle-data -- sync
```

`sync` reads the data repository at its current commit, pinned for the whole run, so the commit recorded in the catalog is the one the files came from. It calls `download_build` for the newest published build with a forced refresh, which re-reads the build's manifest but keeps content already stored, so upstream changes are always picked up for the cost of one small request. The catalog is built in a hidden staging directory and renamed to `data/catalog/<version>_<build>_r<n>` only when complete; a failed build removes its staging directory. `sync` alone changes nothing the bot serves, because it never writes into an existing catalog directory. Directories without the `_r<n>` suffix are ignored.

```bash
cargo run -p barnacle-data -- diff
```

`diff` lists new, removed and regrouped ships, what the automatic rules did with each new ship, look-alike candidates, and every curation entry that no longer resolves. A person reviews the new ships, updates `curation/ships.toml` and raises `reviewed_through`. Once `validate` passes, `use <version>_<build>_r<n>` switches `data/catalog/current` to the new catalog and the bot is restarted. Nothing is copied by hand and no game install is needed.

When WG releases a version before landaire/wows-replay-data publishes it, the bot keeps serving the previous catalog. That is correct behaviour, not an outage. A person with a game install can run `wows-data-mgr dump-renderer-data` to produce the same layout locally.

### 7.2 A new wows-toolkit release

1. A pull request bumps the pinned toolkit versions in the workspace `Cargo.toml` by hand, together with `pickled` and `reqwest` whenever the new toolkit versions move them.
2. CI builds everything, runs the catalog tests against a small committed fixture, and rebuilds the catalog from the latest dump.
3. A **catalog equivalence check** compares the new catalog with the one built by the previous toolkit version on the same game build. Any difference fails the check and is listed, so a parser regression cannot reach players unnoticed.

Pins use exact versions (`=0.45.0`). The crates are 0.x and bump together, so a range buys nothing and hides which version produced a catalog.

Dependabot cannot open that pull request. Every wows-data-mgr release from 0.19.0 to 0.21.0 requires the matching wowsunpack minor version, and Dependabot's Cargo support loosens one requirement at a time, so each exact pin blocks the other crate's update and no pull request appears. It still proposes a patch release of one crate when the other's requirement allows it. Nothing currently announces a new toolkit release.

`pickled` and `reqwest` are used directly but must match the versions the toolkit uses, because their types cross into toolkit calls. Dependabot ignores every `pickled` update and every minor or major `reqwest` update, so both move only in a hand-made toolkit bump. `pickled` stays pinned exactly, because its pre-release tags order as text (`alpha9` sorts after `alpha11`), so a caret range would select the older, published `alpha9`. `rootcause` is not a direct dependency for the same reason: a rootcause 0.13 bump broke the build on 2026-09-16 while the toolkit still returned 0.12 reports.

## 8. Testing

| Layer | What is tested | How |
|---|---|---|
| `barnacle-guess` | Every rule in 5.5, including tier boundaries for look-alikes, reversed ranges, the no-repeat memory and an empty pool | Unit tests with a fixed clock and seeded random source |
| `barnacle-catalog` | Field extraction, the paper-flag path, name lookup, rules 3-5 in 4.2 | A small fixture if one can be cut down; otherwise a test that skips when no dump is present, the same pattern wows-data-mgr uses |
| Curation | Every check in 4.3, and the false-pair examples in 4.2 staying unpaired | Unit tests on hand-written configs, plus `validate` in CI against the real catalog |
| Bot | Command wiring, the channel lock, cancel permission, the bot filter | Integration tests against the game module with Discord calls behind a trait |

## 9. Delivery order

Steps 1-3 and the first catalog are planned in detail in `docs/superpowers/plans/2026-09-16-barnacle-data-pipeline.md`.

1. Workspace, pinned toolkit crates, `barnacle-data sync` downloading 15.8.0 (about 310 MB, cached; later builds fetch only what changed).
2. `barnacle-catalog` producing a catalog, with the paper-flag path and composited silhouettes.
3. Automatic curation rules, `validate`, `diff`.
4. Review what the rules leave: the unmatched reskins, bad silhouettes and ships added since 13.11, plus the year-suffixed look-alike candidates.
5. `barnacle-guess` rules with tests.
6. `barnacle-bot` with `/guess`, `/ship info`, `/profile`, `/about`.
7. CI, dependency bot, catalog equivalence check.
8. Upstream pull request for `is_paper_ship`.

## 10. Decisions and open questions

| # | Question | Status |
|---|---|---|
| Name | What is the bot called? | Decided 2026-09-16: Barnacle. The GitHub repository is `SatanshuMishra/barnacle`. |
| Q1 | Rust, or Python with a Rust data step? | Decided 2026-09-16: Rust (section 2.1) |
| Q2 | Commit composited silhouettes, or build them on each deploy? | Decided 2026-09-16: build on the machine running the bot. Nothing from the data dump, including the catalog, is committed; `data/` stays gitignored. |
| Q3 | May the curation lists be taken from track? | Decided 2026-09-16: no. Barnacle is Apache-2.0 and builds its own (section 11). |
| Q4 | Which game defaults? | Decided 2026-09-16: section 5 as written, with the command kept as `/guess` and historical as an on/off option |
| Q5 | Which names count as answers? | Decided 2026-09-16: English only |
| Q6 | Where does the bot run? | Decided 2026-09-16: locally, as one process (section 2.2) |
| Q7 | Which ship groups are allowed in 15.8.0? | Decided 2026-09-16: `premium` is allowed (14 owned ships moved there from `special`); `experimental` (6 paper ships) and `coopOnly` (1 copy of Schlieffen) are not |
| Q8 | Silhouette background? | Decided 2026-09-16: seafoam `#D3E6E1`. WG's own background is parchment `#C8C2B4` and its silhouettes are dark brown `#261D1A`, so the background must be light; seafoam was chosen over parchment and mist as nautical and distinct from WG's look |
| Q9 | May CC0-1.0 dependencies be used? | Decided 2026-09-16: yes. The `encoding` index crates pulled in by `gettext`, which wowsunpack requires, are CC0-1.0 |
| Q10 | May CDLA-Permissive-2.0 dependencies be used? | Decided 2026-09-16: yes. `webpki-roots`, the root certificate list that serenity's default TLS backend pulls in, is CDLA-Permissive-2.0 |

## 11. Licensing

Barnacle is licensed Apache-2.0.

### 11.1 Why the game concept can be reused

Copyright covers expression, not ideas. US law says protection never extends to "any idea, procedure, process, system, method of operation, concept" - [17 U.S.C. 102(b), Cornell LII](https://www.law.cornell.edu/uscode/text/17/102). For games specifically, the Copyright Office states that the idea for a game, its name, and the methods for playing it are not protected - [copyright.gov, Games](https://www.copyright.gov/register/tx-games.html). A silhouette guessing game with tier ranges, a timed hint and a best-time record is idea and method, so Barnacle may implement it.

AGPL-3.0 obligations attach to a "covered work", which means track itself or a work based on it, and "modify" means to copy from or adapt all or part of it in a way that needs copyright permission - [AGPL-3.0 section 0, gnu.org](https://www.gnu.org/licenses/agpl-3.0.txt). A reimplementation that copies nothing protected from track is not a covered work, so AGPL does not reach it.

Changing colours, command names or wording is not what makes Barnacle safe. What makes it safe is not copying track's expression. A copied file with new colours would still be a work based on track.

### 11.2 What must not be copied from track

| Item | Why |
|---|---|
| Source code, including translated line-by-line into Rust | Code is expression; a close translation is an adaptation |
| UI text: embed titles, hint and result messages, command descriptions | Literary expression - [copyright.gov, Games](https://www.copyright.gov/register/tx-games.html) |
| The `similar` and `forbidden` lists in `guess.toml` | A data compilation's copyright covers its "selection, coordination or arrangement", though not the data itself - [Copyright Office Circular 14](https://www.copyright.gov/circs/circ14.pdf). The 133 similar groups are a judgment-based selection. |
| The "Who's that Pokemon?" description (`track/bot/extensions/guess.py:338`) | Names a third-party mark; names and titles can be protected under trademark law - [Copyright Office Circular 33](https://www.copyright.gov/circs/circ33.pdf) |

Facts may be used freely: that a given ship index is a test ship, what group WG assigns to it, WG's naming conventions for reskins, and track's timings and tier defaults as numbers.

Running the bot locally does not change any of this. AGPL section 13 covers users "interacting with it remotely through a computer network" - [AGPL-3.0 section 13, gnu.org](https://www.gnu.org/licenses/agpl-3.0.txt), and Discord users interact with a bot over the network wherever it runs. That only matters if track code were copied, which this spec rules out.

### 11.3 Dependencies

Every dependency must allow Apache-2.0 distribution. CI enforces this with a license allowlist check (for example `cargo-deny`).

| Crate | License | Source |
|---|---|---|
| wows-toolkit crates | MIT | `wows-toolkit/Cargo.toml`, `[workspace.package]` |
| `pickled` | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/pickled) |
| `gettext` | MIT | [crates.io](https://crates.io/crates/gettext) |
| `image` | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/image) |
| `vfs` | Apache-2.0 | [crates.io](https://crates.io/crates/vfs) |
| `poise` | MIT | [crates.io](https://crates.io/crates/poise) |
| `serenity` | ISC | [crates.io](https://crates.io/crates/serenity) |
| `sqlx` | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/sqlx) |
| `encoding` index crates (via `gettext`) | CC0-1.0 | `cargo deny check licenses` on 2026-09-16 |
| `webpki-roots` (via `serenity`) | CDLA-Permissive-2.0 | `cargo deny check licenses` on 2026-09-16 |
| `tracing`, `tracing-subscriber` | MIT | [crates.io](https://crates.io/crates/tracing), [crates.io](https://crates.io/crates/tracing-subscriber) |

### 11.4 Wargaming's assets and marks

Apache-2.0 covers only Barnacle's own code. The silhouettes and ship names belong to Wargaming, and their use falls under Wargaming's Player Content Policy, not Barnacle's license. That policy forbids commercial use of Wargaming IP apart from listed exceptions, and requires a clear statement that the creator is not affiliated with or endorsed by Wargaming - [Wargaming Player Content Policy, sections 2.1 and 2.5](https://legal.wargaming.net/en/user-documents/content-policies/player-content-policy/view). The policy does not mention bots or software tools specifically, so whether a Discord bot counts as permitted player content is **unverified**.

For Barnacle this means:

- `/about` and the README carry Wargaming's recommended non-affiliation notice.
- The bot has no paid features.
- No Wargaming logo is used as the bot's avatar or embed icon. Track used one (`track/bot/extensions/guess.py:29`).
- Silhouettes are built on the machine running the bot and never committed (Q2).

This section is a reading of the cited sources, not legal advice.
