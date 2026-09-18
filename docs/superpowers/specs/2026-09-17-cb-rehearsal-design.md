# Clan Battle rehearsals

## 1. The problem

Clan Battle nights fall on Wednesday, Thursday, Saturday and Sunday, a night's sign-up post goes up 24 hours before its start, closes at its start and is deleted 30 minutes after its end. Every one of those transitions is driven by wall-clock time, so the soonest a developer can watch a post appear is the next Friday, Saturday, Tuesday or Wednesday at 23:30 UTC. That makes a change to `/cb` untestable on the day it is written.

This document specifies a rehearsal: a run of the whole sign-up flow, driven by hand, in a server that holds nothing anyone cares about, leaving nothing behind when it ends.

## 2. The rule

**A rehearsal server has no clock of its own.** The background beat skips it entirely, so a season there advances only when a developer advances it, and a rehearsal run twice produces the same sequence both times.

**Everything in a rehearsal server is disposable.** That is the whole isolation story: a season there is an ordinary season in every respect, and what makes it safe to delete is where it lives. The reset command deletes every Clan Battle season, post and answer in that server without asking which of them mattered.

These two sentences are the architecture. Everything below is how they are enforced.

## 3. What is not built

- No flag on a season marking it as a rehearsal, and therefore no migration. The guild is already a clean boundary and a column would be a second lock on the same door. The cost is the rule in section 2: a real season started in a rehearsal server will be deleted by a reset. That is accepted.
- No simulated clock in the production beat. The beat is driven entirely by one `now_unix`, so a wrong value there is indistinguishable from time passing and would close sign-ups early and delete live posts.
- No second renderer. A rehearsal drives the real `tick`, not a parallel path that could drift from it.
- Nothing in this document changes the schedule, the click path, the redraw combiner, the ping rule or any `/cb season` command.

## 4. The configuration

`barnacle.toml` gains one section:

```toml
[commands]
scope = "guilds"
guilds = [111111111111111111, 222222222222222222]

[rehearsal]
guilds = [222222222222222222]
```

`Config` gains `pub rehearsal: Vec<u64>`, empty when the section is absent. `ConfigFile` gains `rehearsal: Option<RehearsalFile>` with `RehearsalFile { guilds: Vec<u64> }`, both carrying `#[serde(deny_unknown_fields)]` as the rest of the file already does, so `[rehearsals]` or `guild = ...` is refused at startup rather than silently leaving the guard off.

Three new `ConfigError` variants, with these exact messages:

| Variant | Message | When |
|---|---|---|
| `ZeroRehearsalGuild` | `rehearsal.guilds contains 0, which is not a Discord server ID` | any entry is 0 |
| `RehearsalWithGlobal` | `commands.scope is "global", so rehearsal.guilds must be left out` | the section is present and scope is global |
| `RehearsalNotRegistered { guild }` | `rehearsal.guilds lists {guild}, which is not in commands.guilds` | any entry is absent from `commands.guilds` |

An empty `[rehearsal] guilds = []` is accepted and means no rehearsal server, identical to leaving the section out.

`barnacle.example.toml` gains the section, commented out, with a line saying a server listed there is wiped by `/rehearse reset`.

## 5. Registration

`register` currently sends one command list to every guild. It now sends a different list per guild: `commands::all()` everywhere, and `commands::all()` plus `commands::rehearsal()` in a guild that appears in `rehearsal`.

This is what makes the guarantee structural. In a server that is not listed, the rehearsal commands are not registered, so they do not appear in Discord's command list at all: not hidden, not permission-denied, absent.

`register` takes `rehearsal: &[u64]` alongside the scope. Under `CommandScope::Global` the rehearsal list is empty by section 4's validation, so the global branch is unchanged.

`discord::run` takes the rehearsal guild list and passes it both to `register` and to the `Signups` it builds. `main.rs` passes `config.rehearsal`.

## 6. The core

`Signups` gains one field, `rehearsal: Vec<GuildId>`, and one constructor:

```rust
pub fn new(board: B, store: Attendance) -> Arc<Self>
pub fn rehearsing(board: B, store: Attendance, rehearsal: Vec<GuildId>) -> Arc<Self>
```

`new` keeps its signature and builds with an empty list, so every existing caller and test is untouched and the safe default is the one you get by saying nothing.

### 6.1 The background beat skips rehearsal servers

`tick` keeps its signature and its body. After `live_seasons` returns, it drops every season whose guild is in `rehearsal` before the loop. The filter is in Rust rather than in SQL because the list is tiny and the query stays as it is.

### 6.2 Advancing by hand

```rust
pub async fn tick_in(self: &Arc<Self>, guild: GuildId, now_unix: i64, now_ms: u64) -> TickReport
```

Identical to `tick` except that it keeps only seasons whose guild equals `guild`. It does not check whether the guild is a rehearsal server; the command does that, and a scoped tick is meaningful on its own.

`tick` and `tick_in` must share their body rather than being two copies. Extract the existing loop into a private `beat(seasons: Vec<Season>, now_unix, now_ms) -> TickReport` and have both select the seasons and call it. A rehearsal that runs a copy of the beat proves nothing about the beat.

### 6.3 Purging

```rust
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PurgeReport {
    pub seasons: usize,
    pub messages: usize,
    pub answers: usize,
    pub failures: usize,
}

pub async fn purge(self: &Arc<Self>, guild: GuildId, now_unix: i64) -> PurgeReport
```

Two passes, in this order, and the order is the whole point:

1. For every season the guild holds in any state, call `clear_posts(&season, ClearScope::All, now_unix)` and add its counts. **If the total failures are not zero, return immediately with `seasons: 0` and `answers: 0`.** Nothing is deleted from the database while a message it describes may still be on screen.
2. Only then, for each season, call `store.purge_season`, adding the answers it reports and counting the season.

`messages` is the number of posts actually removed from Discord, which is `clear_posts`' `touched`. A post already gone counts as removed, because `delete_post` maps Discord's unknown-message answer to `Removal::Gone`.

This is the same rule `/cb season end` and `/cb season move` already follow: a change that cannot remove a post it needs to remove refuses and leaves everything as it was.

## 7. The store

```rust
pub async fn seasons_in_any_state(&self, guild: GuildId) -> Result<Vec<Season>, AttendanceError>
pub async fn purge_season(&self, guild: GuildId, id: i64) -> Result<u64, AttendanceError>
```

`seasons_in_any_state` is `SEASONS_IN_GUILD` without the `ended_at_ms IS NULL` filter, ordered by `first_day`. The existing `seasons_in` is untouched, because every other caller wants live seasons only.

`purge_season` runs in one `BEGIN IMMEDIATE` transaction and deletes in foreign-key order, since `cb_marks` references `cb_posts` and `cb_posts` references `cb_seasons`:

```sql
DELETE FROM cb_marks WHERE season_id = ?
DELETE FROM cb_posts WHERE season_id = ?
DELETE FROM cb_seasons WHERE id = ? AND guild_id = ?
```

It returns the number of `cb_marks` rows deleted. The season delete carries `guild_id` so a mistaken id cannot reach another server's season; if it matches no row the transaction still commits, having deleted nothing, because the marks and posts deletes are keyed on the same id and a caller that passed a foreign id has already been told nothing by `seasons_in_any_state`.

No migration. Nothing in this section changes a table.

## 8. The commands

A new top-level group, registered only in rehearsal servers:

```
/rehearse advance to:<post|close|remove>
/rehearse reset
```

Both are `guild_only` with `default_member_permissions = "MANAGE_GUILD"` and `required_permissions = "MANAGE_GUILD"`, matching `cb`. Both reply ephemerally through the existing `private` helper, and both defer with `defer_ephemeral` before doing Discord work.

Both begin with the same guard: if the invoking guild is not in the rehearsal list, reply `REHEARSAL_ONLY`. The list reaches the commands through `Data`, which gains `rehearsal: Vec<GuildId>`. Registration already makes this unreachable in a normal server; the guard is what makes the code correct on its own rather than correct by configuration.

### 8.1 advance

`to` is a poise choice parameter with three values, `post`, `close` and `remove`, described as the three moments of a night.

The moment is derived from the guild's own data, never from the real clock, taking the earliest pending one across every live season in the guild:

| Choice | Moment | Derived from |
|---|---|---|
| `post` | `night.post_at_unix()` | the earliest night in range with no `cb_posts` row |
| `close` | `night.start_unix()` | the earliest post row in state `Open` |
| `remove` | `night.remove_at_unix()` | the earliest post row not in state `Removed` |

If there is no such moment, reply `NOTHING_TO_ADVANCE` and do nothing.

Otherwise call `tick_in(guild, moment, moment_ms)` where `moment_ms` is `moment * 1000` as an unsigned value, and reply with `text::advanced`. Running the real beat at that instant means the step under test also performs every earlier step that instant implies, which is exactly what production would do.

### 8.2 reset

Calls `purge(guild, now_unix)` with the real clock, since nothing in a purge depends on the time.

If `failures` is not zero, reply `RESET_BLOCKED`. Otherwise reply with `text::reset_done`.

Reset removes Clan Battle seasons, posts and answers. It does not touch `guess_solves`, because the silhouette game is a different feature.

## 9. Strings

Exactly these, in `text.rs`:

```rust
pub const REHEARSAL_ONLY: &str = "This server is not set up for rehearsals.";
pub const NOTHING_TO_ADVANCE: &str = "Nothing is waiting for that step.";
pub const RESET_BLOCKED: &str = "Some sign-up posts could not be removed, so nothing was deleted. Check that the bot can manage messages here, then run this again.";
```

```rust
pub fn advanced(moment_unix: i64, report: &TickReport) -> String
pub fn reset_done(purge: &PurgeReport) -> String
```

`advanced` renders `Ran the beat at <t:MOMENT:F>.` followed by one sentence per non-empty count, in the order posted, adopted, closed, removed, then failures, each using the existing singular and plural helpers:

```
Ran the beat at <t:1790811000:F>. 1 sign-up post went up.
Ran the beat at <t:1790897400:F>. 1 sign-up post closed.
Ran the beat at <t:1790913600:F>. 2 sign-up posts were removed.
Ran the beat at <t:1790811000:F>. Nothing happened.
```

`Nothing happened.` is the sentence when every count is zero. A non-zero `failures` appends `1 step failed.` or `2 steps failed.`

`reset_done` renders, omitting any zero count and using `Nothing was there to clear.` when all are zero:

```
Cleared 1 season, 2 sign-up posts and 8 answers.
Cleared 1 season and 1 sign-up post.
Nothing was there to clear.
```

Both functions go through the existing `sentences` helper so spacing matches every other reply, and reuse the existing pluralising helpers rather than adding new ones.

## 10. The flow a developer runs

1. `/cb season start` in a channel of the rehearsal server, with a range covering the next CB nights.
2. `/rehearse advance to:post` — the post appears, the role is pinged once.
3. Click the buttons as an ordinary member; watch the roster and tallies update.
4. `/cb season edit codename:...` — the post redraws where it stands, with no ping.
5. `/cb season move` in another channel of the same server — the night re-posts there with its roster, silently.
6. `/rehearse advance to:close` — the buttons grey out.
7. `/rehearse advance to:remove` — the post is deleted.
8. `/cb season end` — anything still up is cleared and the season is marked ended.
9. `/rehearse reset` — every season, post, answer and message in the server is gone.

After step 9 the rehearsal server holds no Clan Battle data at all.

## 11. Tests

Named exactly. `tests/config.rs`:

- `a_rehearsal_server_is_read_from_the_config`
- `a_rehearsal_server_outside_the_command_guilds_is_refused`
- `a_rehearsal_server_with_global_scope_is_refused`
- `a_zero_rehearsal_server_is_refused`
- `an_empty_rehearsal_list_is_no_rehearsal_server`

`tests/attendance_store.rs`:

- `purge_season_deletes_its_marks_posts_and_season`
- `purge_season_leaves_another_guilds_season_alone`
- `seasons_in_any_state_lists_an_ended_season`

`tests/attendance.rs`:

- `the_beat_skips_a_rehearsal_server`
- `tick_in_runs_only_the_guild_it_is_given`
- `advancing_to_the_post_moment_posts_the_night`
- `a_purge_deletes_every_message_row_and_answer`
- `a_purge_whose_delete_fails_deletes_no_rows`

The last one is the important one: with `FakeBoard::fail_deletes` set, assert `failures` is non-zero, `seasons` and `answers` are zero, and that the season, its post row and its marks are all still readable afterwards.

`tests/text.rs`:

- `an_advance_reports_what_the_beat_did`
- `a_reset_reports_what_it_cleared`

## 12. Work split

Four items. Each of the first three compiles and passes the gates on its own, because none of them changes a signature an earlier one's callers depend on.

| Item | Files | After |
|---|---|---|
| `cb3-config` | `src/config.rs`, `tests/config.rs`, `tests/startup.rs`, `barnacle.example.toml` | |
| `cb3-store` | `src/attendance_store.rs`, `tests/attendance_store.rs` | |
| `cb3-core` | `src/attendance.rs`, `tests/attendance.rs` | `cb3-store` |
| `cb3-wiring` | `src/discord.rs`, `src/discord/commands.rs`, `src/text.rs`, `src/main.rs`, `tests/text.rs`, `README.md` | `cb3-config`, `cb3-core` |

`tests/startup.rs` belongs to `cb3-config` only because it builds a `Config` literal that gains a field.

`cb3-wiring` owns `Data`, the new command group, `commands::rehearsal()`, the per-guild registration, the strings and the README section describing the rehearsal server and its reset.

## 13. What the owner checks

Add a rehearsal server to `barnacle.toml`, restart, and confirm `/rehearse` does not appear in the clan server's command list and does appear in the rehearsal one. Then run section 10 start to finish, and confirm after the reset that `/cb season show` in the rehearsal server reports no season and the channel holds no sign-up messages.
