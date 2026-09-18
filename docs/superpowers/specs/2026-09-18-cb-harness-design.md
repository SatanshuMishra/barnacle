# The Clan Battle rehearsal harness

## 1. What this replaces and why

The rehearsal shipped on 2026-09-17 was the wrong shape. It read "the clock blocks testing" as "let a developer drive the clock by hand", and grew a second way to run the bot: `advance to:post|close|remove`, a moment picker, and a background beat that skipped rehearsal servers entirely. The owner's intent was the opposite. A rehearsal is the real feature running on the real clock in a server that does not matter, with exactly one rule lifted.

It was also wrong in fact. `advance to:post` chose the earliest night in range with no post row and never required that night to be in the future, so on a season whose range began in the past it posted a night that had already started and closed it on the next beat. That is the expired sign-up the owner saw.

This document replaces it.

## 2. The rule

**A rehearsal season is an ordinary season whose nights fall on every calendar day.** Nothing else differs. Nights still start at 23:30 UTC, still run four hours, still post 24 hours ahead, still close at their start and still vanish 30 minutes after they end.

**The harness may ask the beat questions, and may move the clock. It may never do the beat's work.** Every transition a rehearsal exercises is performed by the same `beat` that serves the real server. The harness creates a season, asks the beat where it is going next, and moves a per-server clock offset to that instant.

These two sentences are the architecture. Any future addition that has the harness computing a moment, rendering a post, or deciding what a night is has broken it.

## 3. What this buys

Lifting the weekday rule makes every night adjacent to the next, so **every rehearsal night boundary is a Wednesday-to-Thursday**: at 23:30 the current night's sign-ups close and the following night is exactly 24 hours out, so its post goes up in the same beat. Successive posting needs no special support and no waiting for a particular weekday.

The clock offset is what removes the waiting. Created on a Thursday, a rehearsal's real-time chain runs about twenty-eight hours; stepping the clock walks the same chain, in the same order, through the same code, in under a minute.

## 4. What is deleted, not patched

- `Moment`, `moments`, `pending_moment` and the `/rehearse advance` command in `discord/commands.rs`.
- `text::advanced` and `text::NOTHING_TO_ADVANCE`.
- The rehearsal filter in `Signups::tick`. A rehearsal server's seasons are beaten like any other; only the instant they are beaten at differs.
- Spec `docs/superpowers/specs/2026-09-17-cb-rehearsal-design.md` is superseded in whole. Leave the file; add a first line saying so and naming this document.

`tick_in`, `purge`, `purge_season`, `seasons_in_any_state`, the `[rehearsal]` config section, the per-guild command registration and `command_list_for` all survive unchanged in purpose.

## 5. The schedule

The weekday rule currently lives on `Night::new`, which means `Night::parse` rejects a Friday. Every night label read back from `cb_posts` and every button id goes through `Night::parse`, so a rehearsal night on a non-CB day would be unreadable and its clicks would fail. **The rule moves off `Night` and onto `Range`.**

In `schedule.rs`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Days {
    ClanBattle,
    Every,
}

impl Night {
    pub fn on(date: Date) -> Option<Self>
    pub fn new(date: Date) -> Option<Self>
}
```

`Night::on` builds a night for any date, with the same 23:30 start. `Night::new` keeps its current behaviour exactly - `Night::on` filtered by `CB_WEEKDAYS` - so every existing caller is unchanged. `Night::parse` now goes through `Night::on`, because a label must round-trip whatever produced it.

`Range` gains a `days: Days` field:

```rust
impl Range {
    pub fn new(first_day: Date, last_day: Date) -> Option<Self>
    pub fn every_day(first_day: Date, last_day: Date) -> Option<Self>
    pub fn days(self) -> Days
}
```

`new` keeps its signature and yields `Days::ClanBattle`, so no existing caller changes. `nights()` filters by `self.days`: `Night::new` for `ClanBattle`, `Night::on` for `Every`. `holds(night)` must also respect the mode, because it is the click guard - a Friday label must not be held by a `ClanBattle` range even when its date falls inside.

`night_count`, `nights_left`, `next_night`, `due_night` and `last_moment_unix` are unchanged in body; they inherit the mode through `nights()`.

## 6. The store

### 6.1 Migration 0004

Guarded and paired exactly as 0003 is: `.bail on`, a `schema_migrations` row that makes a second run abort before anything changes, and a `.down.sql`. Neither statement rebuilds a table.

```sql
ALTER TABLE cb_seasons ADD COLUMN every_day INTEGER NOT NULL DEFAULT 0 CHECK (every_day IN (0, 1));

CREATE TABLE cb_rehearsal_clock (
    guild_id INTEGER PRIMARY KEY,
    offset_s INTEGER NOT NULL
) STRICT;
```

`every_day` is a column rather than a lookup against the configured rehearsal servers because the beat reads seasons from the database and must not need the config to know what a season's nights are. The clock is a table rather than memory so a restart mid-rehearsal does not strand a half-advanced season.

`EXPECTED_COLUMNS` gains `every_day` as the twelfth `cb_seasons` column in migration order, and a fourth table entry for `cb_rehearsal_clock`.

### 6.2 Reading and writing

`NewSeason` gains `pub days: Days`. Every existing construction site passes `Days::ClanBattle`. `create_season` writes `every_day` from it; `to_season` builds the `Range` with `Range::every_day` when the column is 1 and `Range::new` when it is 0, and a row whose dates and mode disagree is impossible because both come from the same row.

Every `SELECT` that builds a `Season` gains `every_day` in the same position. `SeasonChange` does **not** gain it: a season's night rule is fixed when it is created, because changing it under a posted night would orphan that night's post row.

```rust
pub async fn rehearsal_clock(&self, guild: GuildId) -> Result<i64, AttendanceError>
pub async fn set_rehearsal_clock(&self, guild: GuildId, offset_s: i64) -> Result<(), AttendanceError>
pub async fn clear_rehearsal_clock(&self, guild: GuildId) -> Result<(), AttendanceError>
```

`rehearsal_clock` returns 0 when the guild has no row. `set_rehearsal_clock` is an upsert on `guild_id`. `purge_season` is unchanged; clearing the clock is a separate call the core makes during a purge.

## 7. The core

### 7.1 The beat runs everywhere, at a per-server instant

`Signups::tick` no longer filters. It reads the live seasons, groups them by the instant they should be beaten at - the real instant for an ordinary server, the real instant plus that server's stored offset for a rehearsal server - and calls `beat` once per group.

```rust
pub async fn tick(self: &Arc<Self>, now_unix: i64, now_ms: u64) -> TickReport
```

The signature is unchanged and the reports are summed. A server with no offset row is beaten at exactly `now_unix`, so the clan server's behaviour is bit-for-bit what it is today. `tick_in(guild, now_unix, now_ms)` is unchanged: it beats one guild at an instant the caller names.

### 7.2 Asking the beat where it is going

```rust
pub fn next_moment(seasons: &[Season], posts: &[(i64, Vec<Post>)], now_unix: i64) -> Option<i64>
```

The earliest instant strictly after `now_unix` at which `beat` would do something, taken over every season given:

| Step | Moment | From |
|---|---|---|
| remove | `night.remove_at_unix()` | every post row not in state `Removed` |
| close | `night.start_unix()` | every post row in state `Open` |
| post | `night.post_at_unix()` | every night in range with no post row and `start_unix() > now_unix` |

**Strictly after `now_unix` is the whole correctness of this function.** The deleted `advance` omitted it and posted a night that had already started.

This function is the harness's only knowledge of the beat's timetable, and it lives beside the beat rather than in the command. When the beat gains a step, this gains a row, and the harness does not change. Section 9 names the test that holds the two together.

### 7.3 Purging clears the clock

`purge` is unchanged except that, after the rows are deleted, it calls `clear_rehearsal_clock` for the guild. A reset leaves a rehearsal server holding no seasons, no posts, no answers, no messages and no clock offset.

## 8. The commands

`/rehearse` keeps its group, its `guild_only`, its `MANAGE_GUILD` on both `default_member_permissions` and `required_permissions`, its registration only in configured rehearsal servers, and the in-command guard that refuses a server the configured list does not name. Three subcommands.

### 8.1 start

Takes the same options as `/cb season start` except the dates, which it derives:

| Option | |
|---|---|
| `number` | required, min 1 |
| `codename` | optional, max length 100 |
| `ping_role` | optional |
| `nights` | optional, default 3, min 1, max 7 |

The range is `today + 1` through `today + nights`, with `Days::Every`. Tomorrow, so the owner watches a post go up a genuine 24 hours before its night rather than retroactively. Three nights by default, because two adjacent nights are what make successive posting observable and a third leaves room to watch it twice.

The checks run in the order `/cb season start` uses and reuse the same helpers: run in a text channel, the bot holds the sign-up permissions here, then `create_season`, whose number, overlap and range rules apply unchanged. The reply is `text::rehearsal_started`.

### 8.2 next

No options. Reads the guild's offset, computes `now_unix + offset`, loads the guild's live seasons and their posts, and asks `next_moment`.

`None` replies `text::NOTHING_PENDING` and changes nothing. Otherwise it stores `moment - now_unix` as the new offset, calls `tick_in(guild, moment, moment_ms)`, and replies with `text::stepped`.

The offset only ever moves forward, because `next_moment` only returns instants after the current rehearsal now.

### 8.3 reset

Unchanged in behaviour: `purge`, refuse with `text::reset_blocked` when anything could not be removed, otherwise `text::reset_done`. It now also clears the clock, through `purge`.

## 9. Strings

Deleted: `NOTHING_TO_ADVANCE`, `advanced`. Kept: `REHEARSAL_ONLY`, `reset_done`, `reset_blocked`. Added:

```rust
pub const NOTHING_PENDING: &str = "Nothing is waiting to happen in this rehearsal.";

pub fn rehearsal_started(season: &Season, nights_left: usize, post_at_unix: Option<i64>) -> String
pub fn stepped(moment_unix: i64, report: &TickReport) -> String
```

`rehearsal_started` renders a first sentence naming the rehearsal, then the body `season_started` already renders, reusing that function rather than restating it:

```
Rehearsing season 35: Komodo Dragon. 3 CB nights, 3 still ahead. Next sign-up post: <t:1790811000:F> (<t:1790811000:R>). Manage it with number 35.
```

`stepped` renders `Moved the rehearsal clock to <t:MOMENT:F>.` followed by the same count sentences the deleted `advanced` rendered, in the order posted, adopted, closed, removed, then failures, and `Nothing happened.` when every count is zero:

```
Moved the rehearsal clock to <t:1790811000:F>. 1 sign-up post went up.
Moved the rehearsal clock to <t:1790897400:F>. 1 sign-up post closed. 1 sign-up post went up.
Moved the rehearsal clock to <t:1790913600:F>. 1 sign-up post was removed.
```

The second line is the one worth reading: one press shows a night closing and its successor appearing in the same beat.

## 10. The flow a developer runs

1. `/rehearse start number:99` in a channel of the rehearsal server.
2. `/rehearse next` - the first night's post appears, and its role is pinged once.
3. Click the buttons; watch the roster and the tallies.
4. `/cb season edit`, `/cb season move` in another channel - both ordinary commands, unchanged.
5. `/rehearse next` - the first night closes and the second night's post appears in the same beat.
6. `/rehearse next` - the first night's post is deleted.
7. `/rehearse next` twice more to walk the second night through the same two steps.
8. `/rehearse reset` - every season, post, answer, message and the clock offset are gone.

Steps 2, 5 and 6 are the three transitions that previously needed a real Clan Battle night, a real Wednesday and a real 22:00 respectively.

## 11. Tests

`tests/schedule.rs`:

- `a_night_can_fall_on_any_day_when_the_range_says_so`
- `a_clan_battle_range_still_holds_only_clan_battle_nights`
- `a_night_label_round_trips_on_a_day_that_is_not_a_cb_day`

`tests/attendance_store.rs`:

- `a_rehearsal_season_keeps_its_every_day_nights_when_it_is_read_back`
- `the_rehearsal_clock_starts_at_zero_and_upserts`
- `a_purge_clears_the_rehearsal_clock`

`tests/attendance.rs`:

- `the_beat_uses_a_rehearsal_servers_own_clock`
- `an_ordinary_server_is_beaten_at_the_real_instant`
- `next_moment_never_returns_an_instant_that_has_passed`
- `stepping_walks_post_then_close_and_successor_then_removal`
- `every_step_the_beat_takes_was_offered_by_next_moment`

The last two carry the weight. `stepping_walks_post_then_close_and_successor_then_removal` builds a three-night rehearsal season and repeatedly takes `next_moment` and beats at it, asserting the sequence: first night posts; first night closes **and** second night posts in one beat; first night's post is removed. That single assertion is the successive-posting and cleanup coverage that could not be had live.

`every_step_the_beat_takes_was_offered_by_next_moment` is the contract that keeps the harness honest as `/cb` grows: drive a season from creation to exhaustion by stepping, and assert the beat never reports a posted, closed or removed night at an instant `next_moment` did not name. If a later change teaches the beat a new transition and leaves `next_moment` behind, this fails.

`tests/text.rs`:

- `a_rehearsal_start_names_the_season_and_the_next_post`
- `a_step_reports_what_the_beat_did`

## 12. Work split

| Item | Files | After |
|---|---|---|
| `cb4-schedule` | `src/schedule.rs`, `tests/schedule.rs` | |
| `cb4-store` | `migrations/0004_cb_rehearsal_harness.sql`, `.down.sql`, `src/attendance_store.rs`, `tests/attendance_store.rs` | `cb4-schedule` |
| `cb4-core` | `src/attendance.rs`, `tests/attendance.rs` | `cb4-store` |
| `cb4-wiring` | `src/discord.rs`, `src/discord/commands.rs`, `src/text.rs`, `tests/text.rs`, `README.md`, `docs/superpowers/specs/2026-09-17-cb-rehearsal-design.md` | `cb4-core` |

`cb4-schedule` and `cb4-store` each compile and pass the gates alone, because `Range::new`, `Night::new` and `Signups::new` all keep their current signatures and a new sibling carries the new behaviour. `cb4-core` and `cb4-wiring` are the consumers.

`cb4-wiring` owns the supersession line on the old spec and the README rewrite: the README currently describes `advance to:post|close|remove` and a server whose timer does nothing, and both statements become false.

## 13. What the owner checks

Apply 0004 after backing the database up, restart, and in the rehearsal server run section 10 start to finish. The three things worth watching: the post appears with its ping on the first step, the second step shows a close and a new post in one reply, and the third deletes the closed post. Then confirm in the clan server that a real season still posts on its own schedule and that `/rehearse` is absent from the command list.

## 14. Amendments from review

Four fresh-eyes reviews ran against the built branch. Where this section contradicts anything above it, this section is what shipped.

### 14.1 The night rule lives on Range and nowhere else

Section 6.2 said `NewSeason` gains a `days` field. That was wrong: `Range` already carries the rule, so the struct held it twice and `create_season` stored one copy while both callers rendered the other. A caller passing `Range::new` beside `Days::Every` would have stored thirty nights while its own reply said eight, silently. `NewSeason` has no `days` field; `create_season` reads `new.range.days()`.

### 14.2 A new season clears its server's clock, inside the same transaction

A spent rehearsal left its offset behind, and nothing cleared it but `/rehearse reset`. Starting a fresh rehearsal in that server then beat it at the stale instant, skipping the first nights entirely and showing the developer a season that had already half happened.

The clearing is part of `create_season`'s `BEGIN IMMEDIATE` transaction rather than a step the command remembers. A refused create leaves the clock untouched, a successful one resets it, and no future command can forget it. Two tests pin both halves.

Separately, `discord::run` clears the clock of every server the configuration no longer names, at startup. Without it an offset survives a server being delisted and applies again if it is ever re-listed, which would close sign-ups before their night in whatever that server has become.

### 14.3 The clock reaches clicks, not only the beat

`click` resolved the night's start against real time, so after a stepped close the post rendered as closed and still recorded marks. The harness could not demonstrate the one transition it exists to demonstrate. `Signups::rehearsal_instant` resolves a guild's offset and `click` uses it; an unreadable offset falls back to real time rather than refusing the click.

### 14.4 Beats do not overlap

Removing the rehearsal skip made the background beat and `/rehearse next` concurrent writers on one server for the first time. Two beats at the same instant both find no post row and both send, and the upsert on `(season_id, night)` leaves one message with no row: never closed, never deleted, not removed by a reset. `beat` now takes a mutex, so beats are serialised across the process.

### 14.5 The rehearsal range is bounded and always has a future first post

`/rehearse start` applies the same six-month window `/cb season start` applies; neither `create_season` nor the store ever carried that rule, so a rehearsal created one was missing. `nights` is clamped in code rather than trusted from the option declaration, and the first night is pushed forward when its post moment has already passed, which otherwise made the first `next` skip night one for any rehearsal started between 23:30 and midnight UTC.

The codename is clipped to its declared length before it is stored, on both `/cb season start` and `/rehearse start`. Unclipped, an overlong codename overflows the embed title, every send fails, no post row is written, and the beat retries the identical failing send every sixty seconds for the life of the season.

### 14.6 Smaller corrections

- `purge` clears the clock only when nothing failed, matching the rule two branches above it that refuses to delete rows when a board delete failed.
- The rollback rebuilds `cb_seasons` rather than using `ALTER TABLE DROP COLUMN`, which needs SQLite 3.35 and failed with a bare syntax error on anything older. Proven to restore all eleven columns, the partial index, the posts and the marks.
- `Range::holds` had an untested lower bound: flipping it left every schedule test green, while a season whose first day is itself a CB night would refuse every click on its opening night and delete that night's post as out of range. One assertion pins it.
- Nothing asserted the readiness check's new table arm; deleting the check left the suite green.

### 14.7 What section 12 got wrong

Section 12 claimed each of the first two items passes the gates alone. `cb4-store` could not: it added a required field to a struct literal built in files it did not own, and it extended the readiness check without the shared test fixture applying the migration. Two files needed by the change belonged to no item: `tests/common/mod.rs`, which applies the migrations, and `tests/wiring.rs`, which asserted that a non-CB day is rejected by the button-id parser and therefore contradicted section 5 by design. Both are integrator work, and a future split must list every file whose meaning the change alters, not only the files it primarily edits.
