# Design: Barnacle's Discord bot

Date: 2026-09-16
Status: approved by the owner on 2026-09-16 (decision `01M2PJCA9J95PSZ2VKS0YPM77F`). Sections 4.1, 4.3, 5.1, 6.3, 6.6, 7.2, 9, 10, 11 and 12 were then corrected to match what a compiled prototype showed while Plan 3 was written.
Extends: `docs/specs/2026-09-16-silhouette-game-spec.md` section 6 (the bot), and uses the game rules from `docs/superpowers/plans/2026-09-16-barnacle-game-rules.md` (merged as `crates/barnacle-guess`).
Implemented by: `docs/superpowers/plans/2026-09-16-barnacle-bot.md` (Plan 3).

## 1. Scope

In scope:
- A new binary crate, `barnacle-bot`, that runs `/guess`, `/ship info`, `/profile` and `/about` on Discord.
- Moving the read-only catalog folder code out of `barnacle-data` so the bot can use it without the game-data toolkit.
- One SQLite table for solves, with its migration and rollback files, applied by a person.
- The README steps for setting up the Discord application.

Out of scope (later, or owned elsewhere):
- Leaderboards, streaks, and other `/guess` round types (spec 5.4).
- The weekly toolkit-release watcher, deferred until the bot is complete (decision `01M2P9F5BVXHZNYAF8V82C32ZF`).
- Hosting anywhere other than the owner's machine (spec 2.2).

## 2. Decisions

| Question | Decision | Record |
|---|---|---|
| Where are commands registered? | In the servers listed in the config while testing; a config switch registers them globally once the owner invites the bot to their chosen servers | `01M2PDGYS13P27FTPR4V6SXK64` |
| How is a round cancelled? | A Cancel button on the round post; `/guess` stays one command | `01M2PDGYS13P27FTPR4V6SXK64` |
| How does `/ship info` find a ship? | Discord autocomplete over every catalog ship with an English name | `01M2PDGYS13P27FTPR4V6SXK64` |
| Who sees the replies? | `/ship info` privately, `/profile` publicly | `01M2PDGYS13P27FTPR4V6SXK64` |
| What does `/profile` count? | This server only | `01M2PF7A45Y3NH09B4J183SWBM` |
| How is the bot structured? | A Discord-free game core behind a small Discord interface | `01M2PF7A45Y3NH09B4J183SWBM` |
| Which SQLite library? | `sqlx` 0.9 | `01M2PF7A45Y3NH09B4J183SWBM` |
| May CDLA-Permissive-2.0 be used? | Yes, for `webpki-roots` | `01M2PHB4C2D8TG78RRB3TJ9MHG` |
| Sections 1-5 below | Approved as presented | `01M2PFTVD1SAF4AMSBWF8RZ5D1`, `01M2PFXMYFS0Y2FB0R8QM8SKNM`, `01M2PGGGN2VWPPP6SBQAT55DAQ`, `01M2PGTP0ZVPWT4QM7VWXYEWGY`, `01M2PHB4C2D8TG78RRB3TJ9MHG` |

## 3. Dependencies

Checked on 2026-09-16.

| Crate | Version | License | Why |
|---|---|---|---|
| `poise` | 0.7.0, released 2026-09-06, needs Rust 1.82 | MIT | Slash commands on top of serenity - [crates.io](https://crates.io/api/v1/crates/poise) |
| `serenity` | 0.12.5, the newest release, needs Rust 1.74 | ISC | Discord gateway and HTTP; `poise` 0.7.0 requires `^0.12.5` - [crates.io](https://crates.io/api/v1/crates/poise/0.7.0/dependencies) |
| `sqlx` | 0.9.0, needs Rust 1.94 | MIT OR Apache-2.0 | SQLite access - [crates.io](https://crates.io/api/v1/crates/sqlx) |
| `tokio` | 1.x (1.53.1 current) | MIT | Async runtime; `poise`, `serenity` and `sqlx` all require tokio 1 - [crates.io](https://crates.io/api/v1/crates/tokio) |
| `tracing`, `tracing-subscriber` | 0.1.44, 0.3.23 | MIT | Logging to the terminal - [crates.io](https://crates.io/api/v1/crates/tracing), [crates.io](https://crates.io/api/v1/crates/tracing-subscriber) |
| `webpki-roots` (through serenity) | 0.26.11 and 1.0.9 | CDLA-Permissive-2.0 | Mozilla's root certificate list for TLS, pulled in by serenity's `rustls_backend` feature (`serenity-0.12.5/Cargo.toml`, feature `rustls_backend`) |

Facts that shape the design:
- `poise` 0.7.0 breaks the 0.6 API. For example, `Context` fields became methods (`serenity_context()`, `user_data()`), and autocomplete functions return `serenity::CreateAutocompleteResponse` - [poise changelog](https://github.com/serenity-rs/poise/blob/v0.7.0/CHANGELOG.md). Snippets written for 0.6 will not compile.
- `sqlx` runtime queries (`sqlx::query`, as opposed to the `query!` macro) need no `DATABASE_URL` at compile time - [sqlx README](https://github.com/launchbadge/sqlx/blob/v0.9.0/README.md). With only the `sqlite` and `runtime-tokio` features, it compiles SQLite from source and pulls in no TLS crates. This was observed with `cargo tree` in a scratch project, not taken from a document.
- `sqlx` 0.9 and `rusqlite` 0.40 cannot share a build. Both depend on `libsqlite3-sys`, which declares `links = "sqlite3"` - [libsqlite3-sys Cargo.toml](https://github.com/rusqlite/rusqlite/blob/v0.40.2/libsqlite3-sys/Cargo.toml).
- CDLA-Permissive-2.0 lets data be shared if "the text of this agreement" goes with it, and "does not impose any restriction or obligations" on anything built with the data - [SPDX](https://spdx.org/licenses/CDLA-Permissive-2.0.html). `cargo deny check licenses` rejects it until it is added to `deny.toml` (`deny.toml:2-11`).

## 4. Architecture

### 4.1 Crates

`crates/barnacle-bot` is a binary. It depends on `barnacle-catalog`, `barnacle-guess`, `poise`, `tokio`, `sqlx` (`default-features = false`, features `sqlite` and `runtime-tokio`), `rand`, `thiserror`, `serde`, `toml`, `clap`, `tracing` and `tracing-subscriber`. It never depends on `barnacle-data`, so it never compiles the toolkit (spec 2.3).

| Module | Job |
|---|---|
| `config` | Reads `barnacle.toml` |
| `startup` | Runs the checks in 5.2 and returns everything the bot needs |
| `table` | The Discord-free game core: channel state, rounds, timers, judging, recording solves |
| `solves` | SQLite: record a solve, read a player's count and best time, check the schema |
| `text` | Every user-facing sentence, plus the tier, nation and class labels |
| `lookup` | Ranks `/ship info` autocomplete suggestions and resolves typed text to a ship |
| `info` | Builds the `/ship info` card from the catalog, the curation result and the `ShipBook` |
| `wiring` | Turns command options into `RoundOptions`, and encodes and decodes the Cancel button's ID |
| `ids` | `GuildId`, `ChannelId` and `Place` newtypes, so the core never sees serenity's types |
| `discord` | The poise commands, the message and button handlers, and the serenity implementation of the core's Discord interface |

### 4.2 Moving the catalog folder code

The bot needs three things that live today in `barnacle-data`'s `DataDir` (`crates/barnacle-data/src/store.rs:138`): which catalog is current (`store.rs:159`), loading a catalog (`store.rs:263`), and the file names `current` and `catalog.json` (`store.rs:18-19`).

A new module, `barnacle_catalog::store`, takes over these read-only parts, plus the silhouette path `silhouettes/<index>.png`. `barnacle-data` calls the new module, and its behaviour and existing tests stay as they are. The write side (staging, publishing, revision numbering) stays in `barnacle-data`.

### 4.3 What the bot uses from `barnacle-guess`

The bot uses:
- `ShipBook` (`crates/barnacle-guess/src/book.rs:28`), with its `draw` and `answers` methods (`book.rs:116`, `book.rs:99`);
- `Round::judge` and `Round::may_cancel` (`crates/barnacle-guess/src/round.rs:68`, `round.rs:79`);
- `Timing::STANDARD` (`round.rs:17`).

Three additions to `barnacle-guess` are needed. `ShipBook::names` returns a ship's own accepted names and `ShipBook::lookalikes` its look-alike indexes, both for `/ship info`. The third is a conversion from `Snowflake` (`crates/barnacle-guess/src/ids.rs:19`) to Unix milliseconds. It uses Discord's epoch, `1420070400000`, and the formula `(snowflake >> 22) + 1420070400000` - [Discord reference, Snowflakes](https://github.com/discord/discord-api-docs/blob/main/developers/reference.mdx).

## 5. Configuration and startup

### 5.1 Configuration

`barnacle.toml` is ignored by git, and a committed `barnacle.example.toml` shows its format. It sets:

| Key | Meaning | Default |
|---|---|---|
| `data_dir` | The data folder holding `catalog/` | `data` |
| `curation` | The curation file | `curation/ships.toml` |
| `database` | The SQLite file | `data/barnacle.sqlite3` |
| `commands` | Either `{ scope = "guilds", guilds = [<server id>, ...] }` or `{ scope = "global" }`; `guilds` must be left out with `global`, and a server ID of 0 is refused | none, so it must be set |

The bot token comes only from the `DISCORD_TOKEN` environment variable. It is never read from or written to a file.

### 5.2 Startup checks

The bot refuses to start before contacting Discord, printing a message that names the problem, unless all of these hold:

1. `barnacle.toml` parses, and `DISCORD_TOKEN` is set and not empty.
2. `data/catalog/current` names a catalog, and its `catalog.json` loads.
3. `curation/ships.toml` passes `barnacle_catalog::curation::validate` (`crates/barnacle-catalog/src/curation/validate.rs:62`). If it doesn't, every problem is printed. This closes the gap noted in Plan 2: `ShipBook` accepts a file that has never been validated.
4. Every ship in the widest pool (tiers 1-11, not historical) has its silhouette file.
5. The SQLite file exists, and `guess_solves` exists with exactly the columns in 8.1. If either is missing, the bot prints `sqlite3 data/barnacle.sqlite3 < migrations/0001_guess_solves.sql` (with the configured path). The bot opens the file without permission to create it, and never creates or alters tables.

Only then does it connect.
- **Intents** (event subscriptions): it requests `GUILDS`, `GUILD_MESSAGES` and `MESSAGE_CONTENT`. `MESSAGE_CONTENT` is privileged. Without it, message `content` arrives empty, and requesting it before it is enabled closes the connection with code 4014 - [Discord Gateway](https://github.com/discord/discord-api-docs/blob/main/developers/events/gateway.mdx).
- **Commands** are registered according to `commands`. Guild commands update instantly. Discord documents no fixed delay for global commands; it describes a "read-repair" reload instead - [Discord Application Commands](https://github.com/discord/discord-api-docs/blob/main/developers/interactions/application-commands.mdx).

## 6. How a round runs

### 6.1 State

The core keeps one entry per channel, holding the channel's `RecentShips` and its active round, if any. Each entry has its own tokio mutex. That mutex is held across each state change and the Discord call that goes with it, so a channel's changes and messages always happen in order, and channels never wait on each other. An outer map lock is held only long enough to find or create an entry.

An active round holds the `barnacle_guess::Round`, a round number that is unique for the life of the process, whether the hint has been posted, and the round post's message ID.

### 6.2 Starting a round (`/guess`)

Under the channel's lock:
1. If a round is active, the command gets a private "This channel already has a round running."
2. `ShipBook::draw` runs with the channel's recent ships. `GameError::EmptyPool` becomes a private reply (section 7).
3. The `/guess` handler posts the round (section 7) as the interaction reply and returns the post's message ID. If the post fails, nothing is stored and the lock is released.
4. `Draw::start(invoker, posted)` creates the round. The round is stored with a new round number, and the channel's memory becomes `recent.remember(ship)`.
5. A timer task for that round number is spawned.

Messages that arrive during step 3 wait for the lock. Messages sent before the post are rejected by `Round::judge`.

### 6.3 Guess messages

Messages written by bots return immediately, and so do messages in a channel that has never had a round; that check touches only the outer map. Otherwise the channel lock is taken, a channel with no active round drops the message, and `judge` runs. If it returns a `Solve`:
1. The round is removed from the channel.
2. In one transaction, the player's previous best in this server is read and the solve is inserted (section 8).
3. The ending is posted: the win reply and the disabled button.

If the database fails, the error is logged and the win is still announced, without the personal-best line. Wrong guesses change nothing and post nothing.

### 6.4 The timer task

1. The task sleeps for `Timing::STANDARD.before_hint` (20 s).
2. It takes the lock. If its round number is still active, it posts the hint and marks it posted.
3. It sleeps for `after_hint` (10 s).
4. It takes the lock. If its round number is still active, it removes the round and posts the timed-out ending.

A round that ended early leaves nothing for its timer to do, so the task finds nothing and exits.

### 6.5 The Cancel button

The button's custom ID carries the round number. When it is clicked, under the channel lock:
- **The round has already ended:** the clicker gets a private "That round has already ended."
- **`may_cancel` fails:** `Round::may_cancel(clicker, can_manage_messages)` runs first. `can_manage_messages` comes from the member's permissions in the interaction, which Discord defines as the member's total permissions in the channel, including overwrites - [Discord Receiving and Responding](https://github.com/discord/discord-api-docs/blob/main/developers/interactions/receiving-and-responding.mdx). `MANAGE_MESSAGES` is still permission bit 13 - [Discord Permissions](https://github.com/discord/discord-api-docs/blob/main/developers/topics/permissions.mdx). If the check fails, the clicker gets a private refusal.
- **Otherwise:** the round is removed, and the cancelled ending is posted.

A win, a Cancel click and the timeout each remove the round under the same lock, so exactly one of them ends a given round.

### 6.6 The Discord interface

```
trait Announcer {
    async fn post_hint(&self, channel, hint) -> Result<(), AnnounceError>;
    async fn post_ending(&self, channel, round_post, ending) -> Result<(), AnnounceError>;
}
```

`ending` is one of:
- `Solved`: the `Solve`, the reveal, the winning message's ID, and whether it is a personal best (this can be unknown if the database failed);
- `TimedOut`: the reveal;
- `Cancelled`: the reveal and who cancelled.

`post_ending` also disables the round post's button. The round post itself is not part of the trait: the `/guess` handler passes it to the core as a function, because it is the command's reply. The core is generic over `Announcer`, and over `SolveStore`, a one-operation interface (`record`) that `Solves` implements. Its tests use a fake Discord that records every call in order and a fake store. SQLite does its work on a background thread, so with tokio's paused clock the runtime would look idle during a query and jump to the next timer; the fake store keeps the round tests' timing exact, and `Solves` is tested on its own (section 10).

### 6.7 Restarts and logging

- **Restarts:** everything in section 6 lives in memory. After a restart, running rounds are gone, their buttons answer "That round has already ended.", and every channel's recent-ships memory is empty (spec 5.2).
- **Logging:** errors and round events (start, hint, ending) go to the terminal through `tracing`.

## 7. Commands and wording

All wording is Barnacle's own, and nothing is copied from track (spec 11.2). All of it lives in `text`.

### 7.1 `/guess`

- **Options:** `min_tier` and `max_tier` (integers with `min = 1` and `max = 11`, optional), and `historical` (boolean, optional). The command works in servers only.
- **Round post:**
  - An embed titled **Name that ship**, with the description "First correct answer in chat wins."
  - A **Tiers** field reading `VI-XI`, or `VIII` when both bounds are equal.
  - A **Paper ships excluded** field, shown only when `historical` is on.
  - The silhouette as the embed image, attached as `silhouette.png`, and the footer "Hint in 20 seconds".
  - A secondary-style **Cancel** button, and the embed colour `#2E6F6B`.

| Moment | Seen by | Text |
|---|---|---|
| Hint, when the range spans several tiers | channel | Hint: it's tier VIII. |
| Hint, when the range is one tier | channel | Hint: it's from Japan. |
| Win (a reply to the winning message) | channel | Correct: **Yamato**, tier X battleship, Japan. Solved in 3.251 s. |
| Win that is a personal best in this server | channel | The win text followed by " New personal best." |
| Timed out | channel | Nobody named it. It was **Yamato**, tier X battleship, Japan. |
| Cancelled | channel | @user ended the round. It was **Yamato**, tier X battleship, Japan. |
| Round already running | invoker | This channel already has a round running. |
| Empty pool | invoker | No ships fit tiers VI-VIII. With `historical` on: No ships fit tiers VI-VIII with paper ships excluded. |
| Cancel refused | clicker | Only the player who started this round, or someone who can manage messages, can end it. |
| Button on an ended round | clicker | That round has already ended. |
| Any other failure on a pending command | invoker | Something went wrong. Nothing was changed. |

Times always show three decimals, followed by " s".

### 7.2 Labels

- **Tier:** Roman numerals I-XI.
- **Class:**

  | Class | Label |
  |---|---|
  | `destroyer` | destroyer |
  | `cruiser` | cruiser |
  | `battleship` | battleship |
  | `aircraft_carrier` | aircraft carrier |
  | `submarine` | submarine |
  | `other(s)` | `s`, as the game writes it (the 15.8.0 catalog has `Auxiliary`) |
  | `unspecified` | unknown class |

- **Nation:**

  | Nation | Label |
  |---|---|
  | `USA` | U.S.A. |
  | `United_Kingdom` | U.K. |
  | `Russia` | U.S.S.R., the game's own label (`IDS_RUSSIA` in the 15.8.0 English strings) |
  | `Pan_Asia` | Pan-Asia |
  | `Pan_America` | Pan-America |
  | `Events` | Event |
  | `France`, `Germany`, `Italy`, `Japan`, `Netherlands`, `Spain`, `Commonwealth`, `Europe` | as written |
  | anything else | underscores become spaces |

  These are all the nations in the 15.8.0 catalog (`data/catalog/15.8.0_13187581_r4/catalog.json`, counted on 2026-09-16). Two labels differ from the game's own English strings: the game says The Netherlands (`IDS_NETHERLANDS`) and, for `Events`, Alliance of New Earth (`IDS_EVENTS`).

### 7.3 `/ship info <ship>` (private)

- **Suggestions:** as the person types, Discord shows up to 25 suggestions drawn from every catalog ship with an English name, excluded ships included.
  - Matching compares `clean_answer` forms, so "konig" finds König.
  - Names starting with the typed text come first, then names containing it, each group alphabetical by display name.
  - Each suggestion is labelled `Name (X, Nation)`, and its value is the ship index.
  - Discord's documented limit for autocomplete choices is 25 - [Discord Receiving and Responding](https://github.com/discord/discord-api-docs/blob/main/developers/interactions/receiving-and-responding.mdx).
- **Typed text that isn't a suggestion:** Discord lets people submit it anyway. It is tried as a ship index, then as an exact cleaned name. If neither matches, the reply is "No ship matches that."
- **Reply:** an embed titled with the ship's display name (`ShipName::display`, `crates/barnacle-catalog/src/model.rs:163`), with the silhouette when the file exists, and these fields:
  - ID, tier, class, nation, group, and paper ship (yes or no);
  - **In /guess:** either "Yes", or "No: " followed by the curation reason (`Removal`'s `Display`, `crates/barnacle-catalog/src/curation/rules.rs:56`), with the base ship's name added after its index where the reason names one;
  - **Accepted answers:** the ship's own names, its variants' names and its aliases;
  - **Look-alikes:** listed by name.
- **Field length:** any field longer than Discord's 1024-character limit is cut and ends with "and N more" - [Discord Message resource](https://github.com/discord/discord-api-docs/blob/main/developers/resources/message.mdx).

### 7.4 `/profile [user]` (public)

- **Whose profile:** the named user, or the person who ran the command when no user is named.
- **Contents:** for this server only, "Rounds won" and "Best time", or "No rounds won here yet." when there are none.

### 7.5 `/about` (public)

It shows:
- one sentence on what Barnacle is;
- the loaded catalog's game version and build, its catalog name, and its data commit (first 7 characters);
- the `wowsunpack` and `wows-data-mgr` versions from the catalog's provenance;
- the bot version and the repository link `https://github.com/SatanshuMishra/barnacle`;
- the Wargaming notice, word for word from `README.md:30`.

### 7.6 Permissions

Every command is available to everyone by default. Server admins restrict commands per role or channel in Discord's own integration settings (spec 6).

## 8. Storage

### 8.1 Schema

The migration file is `migrations/0001_guess_solves.sql`:

```sql
CREATE TABLE guess_solves (
    id INTEGER PRIMARY KEY,
    guild_id INTEGER NOT NULL,
    user_id INTEGER NOT NULL,
    ship_index TEXT NOT NULL,
    elapsed_ms INTEGER NOT NULL CHECK (elapsed_ms >= 0),
    solved_at_ms INTEGER NOT NULL
) STRICT;
CREATE INDEX guess_solves_by_player ON guess_solves (guild_id, user_id, elapsed_ms);
```

- **`STRICT`:** SQLite rejects a value that can't be stored as the declared type. This needs SQLite 3.37.0 or newer - [sqlite.org](https://www.sqlite.org/stricttables.html). The owner's `sqlite3` is 3.51.0, and the SQLite bundled with `sqlx` is 3.51.3.
- **Rollback:** `migrations/0001_guess_solves.down.sql` drops the index and then the table.
- **IDs:** Discord IDs are stored as SQLite's signed 64-bit integers. The timestamp part of an ID stays below the sign bit until about 2084. A conversion that doesn't fit fails with an error; it never stores a wrong number.
- **`solved_at_ms`:** the winning message's Unix time, derived from its ID (4.3). No clock is read.

### 8.2 Applying the migration

The owner runs `sqlite3 data/barnacle.sqlite3 < migrations/0001_guess_solves.sql`, which also creates the file. The bot connects with `create_if_missing(false)` and a 5-second busy timeout. It changes no database settings.

### 8.3 Queries

| Use | SQL |
|---|---|
| Previous best | `SELECT MIN(elapsed_ms) FROM guess_solves WHERE guild_id = ? AND user_id = ?` |
| Record a win | `INSERT INTO guess_solves (guild_id, user_id, ship_index, elapsed_ms, solved_at_ms) VALUES (?, ?, ?, ?, ?)`, in the same transaction as the previous-best read |
| Profile | `SELECT COUNT(*), MIN(elapsed_ms) FROM guess_solves WHERE guild_id = ? AND user_id = ?` |
| Schema check | `SELECT name, type, "notnull", pk FROM pragma_table_info('guess_solves') ORDER BY cid` |

A win is a personal best when there was no previous best, or when its time is strictly lower. All values are bound as parameters; SQL is never built from strings.

### 8.4 Data handling

The table holds server IDs, user IDs, ship indexes and times, and nothing else: no names and no message text.
- **Deleting a player's data** on request: the owner runs `DELETE FROM guess_solves WHERE user_id = ?`, which the README documents.
- **Backups:** copy the file while the bot is stopped.

## 9. Errors

- **Startup:** each failure in 5.2 is its own `thiserror` variant carrying the path, the problems or the missing column. `main` prints the error with its causes and exits non-zero.
- **Interaction failures:** a Discord call that fails is logged. If an interaction is still waiting for its reply, the invoker gets "Something went wrong. Nothing was changed." through poise's error hook.
- **Posting failures:**
  - A failed round post stores no round.
  - A failed hint or ending post is logged, and the state change already made stands.
- **Database failure during a win:** it is logged, and the win is announced without the personal-best line.
- **Autocomplete failure:** the command returns an empty suggestion list.
- **Refused connection:** if Discord refuses the connection because `MESSAGE_CONTENT` is not enabled, the log names the Developer Portal switch. serenity reports close code 4014 as `GatewayError::DisallowedGatewayIntents` (`serenity-0.12.5/src/gateway/error.rs:52`), and `Client::start` returns it as `Error::Gateway` (`serenity-0.12.5/src/client/mod.rs:905-906`).

## 10. Testing

No test uses the network, a Discord account or a live database.

| Area | Method | Cases |
|---|---|---|
| `table` | A fake `Announcer` and a fake `SolveStore`, with `#[tokio::test(start_paused = true)]` (needs tokio's `test-util`; paused time jumps to the next timer when the runtime is idle - [docs.rs](https://docs.rs/tokio/latest/tokio/time/fn.advance.html)) | See the list below |
| `solves` | In-memory SQLite, created and dropped per test, with the real `0001_guess_solves.sql` applied | record then profile; servers kept separate; strictly faster is a best and a tie is not; the schema check rejects a missing table and wrong columns; an ID above `i64::MAX` is rejected |
| `text` | Unit tests | numerals 1-11; every nation label and the fallback; class labels; `3251 ms` becomes `3.251 s`; truncation at 1024 characters; both empty-pool sentences |
| `lookup` | Unit tests | prefix matches before substring matches; alphabetical order; at most 25 results; "konig" finds König; excluded ships included; ships without an English name left out; typed text resolved as an index, then as an exact name |
| `startup` | Temporary folders | no current catalog; curation problems; a missing silhouette; a missing database file; a missing table |
| `barnacle_catalog::store` | Unit tests moved from `barnacle-data` | the read-side behaviour is unchanged, and `barnacle-data`'s own tests still pass |
| Discord wiring | The owner's checklist on their test server (Plan 3's last task) | a full round with a win; a timed-out round; a cancel refused, then allowed; `/ship info`; `/profile`; `/about` |

The `table` cases:
- the hint arrives at 20 s and the reveal at 30 s;
- a win at 5 s means no hint and no timeout post;
- a win after the hint;
- a busy channel is refused, then allowed once the round ends;
- cancel by the starter, by a member who can manage messages, refused for another member, and a click on an ended round;
- bot messages, messages sent before the post and wrong guesses are ignored;
- two channels run independently;
- a win and a cancel at the same moment give exactly one ending;
- a failed round post leaves the channel free, and an empty pool starts nothing;
- a win is recorded with its server, player, ship and time;
- a storage failure still announces the win, without the personal-best line, and a failing Discord does not stop rounds ending;
- an earlier round's timer neither hints at nor ends the next round in the same channel;
- recent ships are skipped across rounds.

## 11. Discord setup (owner, once)

The README gains these steps:
1. Create an application in the Discord Developer Portal and add a bot user, then copy its token into `DISCORD_TOKEN`.
2. Turn on **Message Content Intent** under Privileged Gateway Intents. Below 10,000 users no review is needed - [Discord Privileged Intent Review](https://github.com/discord/discord-api-docs/blob/main/developers/gateway/getting-started-with-privileged-intent-review.mdx).
3. Turn off **Public Bot**, so only the owner can add the bot to servers. Discord's OAuth2 docs: "If unchecked, only you can add the bot to guilds" - [Discord OAuth2](https://github.com/discord/discord-api-docs/blob/main/developers/topics/oauth2.mdx).
4. Invite the bot with the `bot` and `applications.commands` scopes and the View Channels, Send Messages, Embed Links and Attach Files permissions.
5. Create `barnacle.toml` from the example, listing the test server's ID.
6. Apply the migration (8.2), then run `cargo run --release -p barnacle-bot`.

## 12. Items settled while writing Plan 3

| Item | Result |
|---|---|
| The poise 0.7 and serenity 0.12.5 API for buttons and for editing the round post | `CreateButton`, `CreateActionRow::Buttons`, `ComponentInteraction::create_response` and `ChannelId::edit_message` compile and pass clippy in a scratch copy of the bot |
| The serenity error for close code 4014 | `GatewayError::DisallowedGatewayIntents`, see section 9 |
| The Developer Portal setting that makes a bot private | Public Bot, see section 11 |
| `U.S.S.R.` as the game's label for `Russia` | Confirmed from the game's strings, see 7.2 |
| A command description without a doc comment | poise 0.7's `command` macro reads a command's description only from doc comments, which this project does not write, so each command is built as a value and its `description` field is set in code (`poise-0.7.0` `Command::description`, `poise_macros-0.7.0/src/command/mod.rs:209-210`) |
