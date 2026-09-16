# Audit: track's silhouette guessing game, and what wows-toolkit offers in its place

Date: 2026-09-16
The bot was renamed from shrimpy to Barnacle on 2026-09-16; this report uses the new name.
Sources audited:
- padtrack/track at `4ce606c` (2026-02-17, last commit). License AGPL-3.0.
- landaire/wows-toolkit at `1e47b9fe` (2026-08-19, version 1.0.2-beta2). License MIT.
- landaire/wows-replay-data, `builds.toml` and `15.8.0_13187581/metadata.toml` as of 2026-09-16 (repo pushed 2026-09-10). No license file.

Every `track/...` path below is relative to the track repository root; every `wows-toolkit/...` path is relative to that repository.

## Verdict

The game can be rebuilt on wows-toolkit with no local World of Warships install and no manual unpacking step, because wows-toolkit's companion repository already publishes every game build's silhouettes, GameParams (the game's master data file) and translation catalogs. Two pieces are not covered by the toolkit and stay the bot's responsibility: the paper-ship flag, which wowsunpack does not parse into its typed model, and the curation of which ships are carbon copies. Track's curation data is also stale by 208 silhouettes and carries three defects, one of which crashes the game whenever Goliath is drawn.

## 1. How track's game worked

### 1.1 Entry point

`/guess` is a discord.py app command in `track/bot/extensions/guess.py:336-383`. It takes four options:

| Option | Type | Default | Meaning |
|---|---|---|---|
| `difficulty` | `easy` / `normal` / `hard` | `normal` | How many alternative names are accepted |
| `min_level` | int 1-11 | 6 | Lowest tier drawn |
| `max_level` | int 1-11 | 11 | Highest tier drawn |
| `historical` | bool | false | Excludes paper ships (designs never built) |

The command rejects `min_level > max_level` ephemerally (`guess.py:355-359`) and refuses to start a second game in the same channel by scanning asyncio tasks for one named `guess_<channel_id>` (`guess.py:361-366`). It then defers, draws a ship, and runs the game as a task with that name (`guess.py:368-383`). Because the lock is an in-process task name, it only works while the bot runs as a single process.

### 1.2 The eligible ship pool, and how test ships and carbon copies were excluded

Exclusion happens in two layers.

The first layer is at load time. `track/bot/utils/wows.py:132-160` reads a pre-generated `ships.json` and keeps only ships whose `group` is in `GROUPS` (`wows.py:70-83`). A `group` is the category string WG assigns to every ship in GameParams, such as `upgradeable` for tech-tree ships or `demoWithoutStats` for ships in closed testing. That list still includes the three `demo*` test groups and `earlyAccess`, because `/inspect` needs to find those ships.

The second layer is the game's own filter, `GuessCog.is_allowed` (`guess.py:258-262`). A ship is eligible only when its group is in the `groups` allowlist of `track/bot/assets/public/guess.toml:1-10` and its index is not in that file's `forbidden` list (`guess.toml:660-776`). A ship's index is WG's stable 7-character ID, such as `PASB008` for Colorado.

- **Test ships** are excluded by the group allowlist: `start`, `special`, `specialUnsellable`, `ultimate`, `upgradeable`, `upgradeableExclusive`, `upgradeableUltimate`, `superShip`. Every `demo*` group, plus `earlyAccess`, `disabled`, `preserved`, `event`, `clan` and `unavailable`, falls outside it.
- **Carbon copies** are excluded by the hand-maintained `forbidden` list of 116 entries. These are reskins that share a hull with a ship already in the pool: ARP, Azur Lane, Blue Archive, High School Fleet and STAR collaboration ships, "B" (Black Friday) variants, "Golden" variants, CLR variants, and a few event ships. The list also carries six ships excluded only because their silhouette image is bad (`guess.toml:771-776`), and some entries were added only because their silhouette file was missing (commit `c367a32`, "exclude missing Black Friday ships in guess").

`random_ship` (`guess.py:264-283`) then keeps ships in the tier range, drops paper ships when `historical` is set, and picks one uniformly. At the last data snapshot (13.11.0) this yields 672 eligible ships, 291 of them paper ships, and every one of them has a silhouette file.

| Class | Eligible ships |
|---|---|
| Cruiser | 259 |
| Battleship | 177 |
| Destroyer | 177 |
| Aircraft carrier | 43 |
| Submarine | 16 |

### 1.3 Similar-ship groups

`guess.toml:12-579` holds 133 `similar` groups: ships that are distinct in the game but whose silhouettes are hard or impossible to tell apart, such as Warspite and Queen Elizabeth. `get_similar` (`guess.py:285-291`) returns the other members of any group the drawn ship belongs to. These groups do not remove ships from the pool; they widen the set of accepted answers (section 1.5).

### 1.4 The silhouette image

`get_silhouette` (`guess.py:323-334`) opens `ships_silhouettes/ship_background.png`, pastes `ships_silhouettes/<index>.png` onto it using the silhouette's alpha channel as the mask, and attaches the result as `ship.png`. Both images are 404x155. They are WG's own in-game GUI assets from `gui/ships_silhouettes/` in the game client, so no rendering is involved.

### 1.5 Answer matching

`get_accepted` (`guess.py:293-321`) builds the set of accepted strings:

1. The drawn ship's short and full names in the invoker's language, cleaned.
2. The ship's romanizations: an ASCII transliteration of the English name, plus a nation-specific table for Japanese (`o` with macron becomes `ou`) and German (umlaut `a` becomes `ae`) (`wows.py:85-88`, `wows.py:143-153`). One alias is hard-coded: `kreml` for Kremlin (`wows.py:158`).
3. Unless the difficulty is `hard`, the same strings for each similar ship. On `normal`, similar ships outside the tier range are meant to be skipped. On both `easy` and `normal`, paper ships among the similar ones are skipped when `historical` is set.

"Cleaned" (`Ship.clean`, `wows.py:115-120`) means lowercased, with all whitespace and the characters `- . ' , ·` removed. Every chat message in the channel is transliterated to ASCII, cleaned the same way, and checked against the set (`guess.py:203-205`). Anyone in the channel can answer, not only the invoker.

### 1.6 Game flow and timing

```
/guess -> defer -> draw ship -> post embed + silhouette + Cancel button
       -> wait 20 s for a correct message
            timeout -> post hint -> wait 10 s more
                          timeout -> disable button, "Time's up. The answer was X."
       -> correct message -> disable button, reply "Well done! Time taken: N s"
                             update the winner's guess_count and best time
```

- The hint (`guess.py:176-180`) gives the tier when the range spans several tiers, otherwise the nation.
- Elapsed time is measured between the Discord message IDs of the game post and the winning message (`guess.py:192`, `guess.py:228`), so the bot's own latency does not count against the player.
- Only the invoker can press Cancel (`guess.py:141-147`). Cancelling kills the task and reveals the answer (`guess.py:121-129`).
- The winner's `guess_count` is incremented and `guess_record` (best time in seconds) is lowered if beaten (`guess.py:230-247`, columns at `track/bot/utils/db.py:68-69`). `/profile` shows both (`track/bot/extensions/general.py:24-38`).
- Server admins can disable `guess` per server or per channel through track's general disable mechanism (`track/bot/extensions/settings.py:13-31`, `track/bot/track.py:46-74`).

### 1.7 The companion command, `/inspect`

`/inspect <ship>` (`guess.py:385-405`) shows a ship's ID, paper flag, group, tier, class, nation, silhouette, every accepted answer string, whether the ship is eligible for `/guess`, and its similar ships. It is the curation tool: it is how a maintainer checks what the game will accept. Ship lookup uses substring and autocomplete matching over names and romanizations (`wows.py:166-235`).

### 1.8 How track was updated per game patch

`track/docs/UPDATING.md` describes a six-step manual pipeline:

1. On a Windows machine with the game installed, run WG's community unpacker through `track/scripts/extract.py`, which pulls `content/GameParams.data`, `gui/ships_silhouettes/*` and the `texts` translation catalogs.
2. Copy those into `resources/` and `bot/assets/public/`.
3. Run `track/scripts/ships/generate.py`, which reverses, decompresses and unpickles GameParams, keeps every entity whose type is `Ship`, and writes `id, index, isPaperShip, group, level, name, species, nation` plus short and full names for every language (`generate.py:11-20`, `generate.py:45-78`).
4. Run `track/scripts/ships/compare.py`, which prints new ships and ships whose group changed.
5. Edit `guess.toml` by hand for each new ship.
6. Commit the new `ships.json` and silhouettes.

The steps that took human judgment were step 1, which needs a game install, and step 5.

## 2. Defects found in track

| # | Where | What happens | Consequence |
|---|---|---|---|
| D1 | `guess.toml:559` | The similar group for Goliath lists `PBSC710` (Monmouth), which is not in `ships.json`. `get_similar` indexes `wows.ships[index]` and raises `KeyError`. | Goliath (`PBSC210`, tier 10 tech tree, eligible) crashes the game after its silhouette is posted, so no answer can ever be accepted. `/inspect Goliath` also fails. Checked against the committed data; not run live. |
| D2 | `guess.toml:754-755` | `PASA898` is listed twice; the second entry is commented "AL Agir", whose real index is `PGSC899`. | AL Agir stays in the pool as a carbon copy of Agir. The silhouette hashes confirm the two are byte-identical. |
| D3 | `guess.py:310-314` | `difficulty == "normal" and similar.level < min_level or similar.level > max_level` parses as `(normal and below) or above`. | On `easy`, similar ships above the tier range are rejected, so easy is stricter than intended. |
| D4 | `wows.py:125-126` with `settings.py:67` and `db.py:74` | `/setlanguage` writes `user.wows_locale`, which is not a column, so nothing is saved. If a locale were ever stored, `tl()` would return the locale string instead of the translation table and crash. | Per-user language never worked. It is latent rather than crashing only because the write is lost. |
| D5 | `guess.py:203-205` | The answer check does not ignore messages from bots. | Another bot echoing text could win. Minor. |
| D6 | `guess.py:361-366` | The single-game-per-channel lock is an asyncio task name. | It does not hold across shards or processes. |

## 3. What wows-toolkit provides

### 3.1 Crates relevant to this game

All four are published on crates.io (checked 2026-09-16), so the bot can depend on released versions instead of git revisions.

| Crate | Version | Released | Role for Barnacle |
|---|---|---|---|
| `wowsunpack` | 0.45.0 | 2026-08-12 | Parses GameParams into typed `Param` and `Vehicle` values; exposes the raw unpickled tree; reads translation catalogs; VFS over game archives. |
| `wows-data-mgr` | 0.21.0 | 2026-08-12 | Downloads published per-build dumps from landaire/wows-replay-data into a local content-addressed store, checks for updates, validates the cache. |
| `wows-core` | 0.13.0 | 2026-08-12 | Shared domain types (`GameParamId`, `Version`, `Recognized<T>`). |
| `wt-translations` | 0.16.0 | 2026-08-12 | UI translation keys for the toolkit itself. Not needed for ship names. |

### 3.2 The data source: landaire/wows-replay-data

`wows-toolkit/crates/wows-data-mgr/src/download_repo.rs:23` points at `https://raw.githubusercontent.com/landaire/wows-replay-data/main`. The repository has one directory per game build from 0.6.13 to 15.8.0, a `builds.toml` index, and a shared `common/` store keyed by a truncated SHA-256 of each file (`wows-toolkit/crates/wows-data-mgr/src/cas.rs:35-68`).

Each dump always includes `gui/ships_silhouettes` (`wows-toolkit/crates/wows-data-mgr/src/dump.rs:26-40`), `content/GameParams.data` (`dump.rs:48`), and every language's `global.mo` translation catalog (`dump.rs:196`). The 15.8.0 dump lists 1,194 ship silhouettes plus `ship_background.png`, and 22 translation catalogs. So every input track's pipeline pulled from a Windows game install is already published per build.

`check_for_updates` (`download_repo.rs:129-172`) compares the repository's latest commit and each local build's metadata against the remote copy. `download_build` (`download_repo.rs:643`) fetches one build, skipping content already stored locally. Both are behind the crate's `download` feature. The crate's default `constants` feature pulls in a GitHub API client that Barnacle does not need.

### 3.3 Mapping track's fields onto wowsunpack

| track field (`generate.py:11-20`) | wowsunpack source | Status |
|---|---|---|
| `id` | `Param::id()` returning `GameParamId` | Covered |
| `index` | `Param::index()` | Covered |
| `name` | `Param::name()` | Covered |
| `typeinfo.species` | `Param::species()` returning `Option<Recognized<Species>>`; `Species` has `Battleship`, `Cruiser`, `Destroyer`, `AirCarrier`, `Submarine` | Covered |
| `typeinfo.nation` | `Param::nation()` | Covered |
| `level` | `Vehicle::level()` | Covered |
| `group` | `Vehicle::group()`; `Vehicle::is_test_ship()` is true for any `demo*` group (`types.rs:1456-1460`) | Covered |
| `isPaperShip` | Not parsed. No field on `Vehicle` or `Param`. | **Gap.** Readable from the raw tree via `game_params::convert::game_params_to_pickle` (`convert/mod.rs:22`). |
| Short name `IDS_<index>` | The provider maps every param to `IDS_<index>` (`provider.rs:2130`) and `localized_name_from_param` resolves it | Covered, but only for the one catalog loaded into the provider |
| Full name `IDS_<index>_FULL` | No helper. Needs a direct `gettext::Catalog` lookup of the key. | Small gap, trivial to fill |
| Silhouette PNG | File in the dump at `gui/ships_silhouettes/<index>.png`, reached through `wows_data_mgr::Dump::vfs()` | Covered |

The toolkit's own replay renderer loads silhouettes by exactly this path (`wows-toolkit/crates/wows-toolkit/src/replay/renderer/video_export.rs:643`), so the path convention is exercised upstream and a change to it would surface there first.

### 3.4 Risks in depending on wows-toolkit

- **Breaking changes every minor version.** The crates are 0.x (wowsunpack 0.45), and the workspace bumps versions together. Every upgrade should be treated as potentially breaking.
- **Moving toolchain.** The workspace pins Rust 1.97 and edition 2024 (`wows-toolkit/Cargo.toml`). Barnacle must track at least the toolkit's minimum Rust version.
- **Upstream focus.** The repository's stated active effort is a replay-simulation rewrite (`wows-toolkit/AGENTS.md`, "Active major effort"). Ship-catalog parsing is a side path of that work, so a regression there may not be caught quickly.
- **Data repository terms.** landaire/wows-replay-data has no license file, and its contents are WG's game assets. Pulling them is what the toolkit does for every user, but redistributing silhouettes inside Barnacle's own repository is a separate decision (see spec, open questions).
- **Single maintainer.** Both the crates and the data repository are published from one account. If publication stops, the fallback is `wows-data-mgr`'s own `dump-renderer-data` command against a local game install, which produces the same layout.

## 4. Automatic carbon-copy detection: measured

In the 15.8.0 dump, the 1,194 silhouettes collapse to 1,000 distinct file hashes: 157 hashes are shared by 351 ships. Measured against track's curated lists:

- Of the 115 unique `forbidden` indexes, 65 share a byte-identical silhouette with another ship and would be caught automatically. The other 50 differ at the byte level. Many are visually the same hull (ARP Takao, HSF Hiei, the AL ships); a pixel-level comparison should catch them, but that is **unverified**, because the images were not downloaded in this audit.
- Among track's eligible pool, 8 identical-hash groups still contain more than one ship. Two are already in `similar`. The other six are Clemson / DD 214, Lyon / Lugdunum, Agir / AL Agir (defect D2), Georg Hoffmann / Georg Hoffmann Golden, Prins van Oranje / Prins van Oranje Golden, and Sinop / Teng She. These are carbon copies track's curation missed.

Identical-hash detection is therefore a cheap, reliable first pass that also audits the hand-kept lists. It is not a replacement for them.

## 5. Drift since track's last data update

Track's snapshot is game version 13.11.0 (commit `4f2f9fe`, 2024-12-03). The 15.8.0 dump has 208 silhouettes that track does not, and 2 of track's are gone. Every one of the 208 needs a curation decision (eligible, carbon copy, or similar to something), so the ported `forbidden` and `similar` lists are a starting point, not a finished dataset.

## 6. Core requirements extracted

R1-R10 are what track did and Barnacle must keep. R11-R14 fix what track got wrong or could not do.

| ID | Requirement | track reference |
|---|---|---|
| R1 | `/guess` with difficulty, tier range 1-11 (defaults 6-11), and a historical option | `guess.py:336-354` |
| R2 | The pool excludes test ships by group allowlist and carbon copies by exclusion list | `guess.py:258-283`, `guess.toml` |
| R3 | One active game per channel; a second attempt is refused ephemerally | `guess.py:361-366` |
| R4 | The silhouette is composited on WG's background image | `guess.py:323-334` |
| R5 | Answers are matched after cleaning, plus romanizations and aliases. Barnacle matches English names only (spec Q5); track used the invoker's language. | `wows.py:115-160` |
| R6 | Similar ships are accepted on easy and normal; normal limits them to the tier range; historical removes paper ones | `guess.py:293-321` (intent, not the buggy code) |
| R7 | A 20 s answer window, then a hint, then 10 s more, then the answer is revealed | `guess.py:154-223` |
| R8 | Time is measured between message snowflakes | `guess.py:192`, `guess.py:228` |
| R9 | An invoker-only Cancel button reveals the answer | `guess.py:113-151` |
| R10 | Per-user solve count and best time, shown on a profile | `guess.py:230-247`, `general.py:24-38` |
| R11 | Curation config is validated against the current catalog at build time; an unknown index fails the build | fixes D1, D2 |
| R12 | Game-version updates need no game install and no hand-copied files | replaces `UPDATING.md` steps 1-3 |
| R13 | A per-build diff lists new, removed and regrouped ships, plus identical-silhouette groups, for curation | extends `compare.py` |
| R14 | An `/inspect`-equivalent curation view | `guess.py:385-405` |

Out of scope for the first port: track's other features (renders, stats, builds, clans), per-server command disabling beyond Discord's built-in command permissions, and track's OAuth linking.
