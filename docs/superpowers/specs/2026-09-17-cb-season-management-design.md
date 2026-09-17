# Design: Clan Battle season management

Date: 2026-09-17
Status: approved by the owner on 2026-09-17; implementation spec.
Extends: `docs/superpowers/specs/2026-09-17-cb-attendance-design.md`, which built the tracker this changes. Every rule in that document still holds unless this one replaces it by name.
Logbook thread: `01M2RP0CQE2NR9FFKGSVH1PCPW` (`cb-season-management`), successor to `01M2R6RSSPA72GZHHK2881S5W9`.
Built by: a `fanout` run, one agent per work item; the item table is section 13.

This document is the only briefing the implementing agents get. Anything not written here is not decided, and an agent that finds a gap stops and reports it rather than inventing an answer.

## 1. Scope

The tracker is live and works. It is missing three controls the owner found in use.

In scope:
- `/cb season end` ends a season **immediately** and takes down every sign-up post it still has up.
- `/cb season edit` changes a season's number, dates, codename and ping role in place.
- `/cb season move` moves a season's posts to another channel.
- A season may name a role to ping when a sign-up post goes up.
- Migration `0003`, applied by hand like the others.

Out of scope:
- Reading attendance back (a command that answers "who was in for Wednesday"). The rows are kept for it; nothing reads them yet.
- Reminders for people who have not answered.
- A manual re-post or re-ping command. Moving a season is the only action that re-posts.
- Any change to how nights, hours or times are computed.

## 2. Decisions

| Question | Decision | Why |
|---|---|---|
| What does ending a season do? | Ends it at once: every post not already removed is deleted from Discord, the season is marked ended, and nothing more posts. The answers stay. | The owner: "ending should immediately end the season and clear any sign-ups interfaces". A season that reaches its last day still ends by itself, and its last post is still swept 30 minutes after that night ends. |
| Does an edit re-post the sign-up? | No. An edit redraws the existing message in place, keeping its position in the channel and every answer on it. | The owner: "Only `re-print` when the scenario requires it (e.g., changing the post channel)." |
| When is a post re-posted, then? | Only when the season moves to another channel. A message cannot change channel. | Same decision. |
| Where does a ping go? | In the message's content, never in the embed, and only on the first post of that night. | A role mention inside an embed notifies nobody. Discord does not document whether an edit notifies, so a redraw must not be relied on to ping and must not be allowed to. |
| Can an ended season's number be used again? | Yes. Numbers are unique among live seasons only. | Ending is now destructive on Discord, so ending the wrong season must be recoverable by starting it again. |

## 3. Principles every implementer follows

The principles in section 3 of the attendance spec apply unchanged, and they are absolute:

1. **Never write a comment, docstring or doc comment**, in Rust, SQL, TOML or Markdown.
2. **No emojis anywhere.**
3. **Create new values; never mutate a shared one in place.**
4. **Follow the file you are editing**: `thiserror` enums with lowercase messages, user-facing sentences in `text.rs`, `ids.rs` newtypes across module boundaries.
5. **The core stays Discord-free.** `schedule`, `attendance_store` and `attendance` import neither `poise` nor `serenity`.
6. **Tests come with the code**, under the names section 12 gives.
7. **Before you finish**: `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`. All three green, no lint silenced, no test deleted that you did not write.
8. **Stay inside your file list.** Another agent owns every other file right now.
9. **Do not run git.**
10. **Every value here is exact.** Strings, column names, SQL and replies are copied as written.

## 4. Migration 0003

`migrations/0003_cb_season_controls.sql`. SQLite cannot add a partial index in place of a table constraint, so `cb_seasons` is rebuilt. The file is **not re-runnable**: it is guarded so a second run aborts before touching anything.

```sql
.bail on

PRAGMA foreign_keys = off;

BEGIN;

CREATE TABLE IF NOT EXISTS schema_migrations (
    name TEXT PRIMARY KEY
) STRICT;

INSERT OR IGNORE INTO schema_migrations (name) VALUES ('0001_guess_solves'), ('0002_cb_attendance');

INSERT INTO schema_migrations (name) VALUES ('0003_cb_season_controls');

CREATE TABLE cb_seasons_rebuilt (
    id INTEGER PRIMARY KEY,
    guild_id INTEGER NOT NULL,
    channel_id INTEGER NOT NULL,
    number INTEGER NOT NULL CHECK (number > 0),
    codename TEXT,
    first_day TEXT NOT NULL,
    last_day TEXT NOT NULL,
    created_by INTEGER NOT NULL,
    created_at_ms INTEGER NOT NULL,
    ping_role_id INTEGER CHECK (ping_role_id IS NULL OR ping_role_id > 0),
    ended_at_ms INTEGER CHECK (ended_at_ms IS NULL OR ended_at_ms >= 0),
    CHECK (first_day <= last_day)
) STRICT;

INSERT INTO cb_seasons_rebuilt (id, guild_id, channel_id, number, codename, first_day, last_day, created_by, created_at_ms)
SELECT id, guild_id, channel_id, number, codename, first_day, last_day, created_by, created_at_ms FROM cb_seasons;

DROP TABLE cb_seasons;

ALTER TABLE cb_seasons_rebuilt RENAME TO cb_seasons;

CREATE UNIQUE INDEX cb_seasons_live_number ON cb_seasons (guild_id, number) WHERE ended_at_ms IS NULL;

COMMIT;

PRAGMA foreign_key_check;

PRAGMA foreign_keys = on;
```

`migrations/0003_cb_season_controls.down.sql` rebuilds the old shape and gives the number back its unconditional uniqueness:

```sql
.bail on

PRAGMA foreign_keys = off;

BEGIN;

CREATE TABLE cb_seasons_rolled_back (
    id INTEGER PRIMARY KEY,
    guild_id INTEGER NOT NULL,
    channel_id INTEGER NOT NULL,
    number INTEGER NOT NULL CHECK (number > 0),
    codename TEXT,
    first_day TEXT NOT NULL,
    last_day TEXT NOT NULL,
    created_by INTEGER NOT NULL,
    created_at_ms INTEGER NOT NULL,
    UNIQUE (guild_id, number),
    CHECK (first_day <= last_day)
) STRICT;

INSERT INTO cb_seasons_rolled_back (id, guild_id, channel_id, number, codename, first_day, last_day, created_by, created_at_ms)
SELECT id, guild_id, channel_id, number, codename, first_day, last_day, created_by, created_at_ms FROM cb_seasons;

DROP INDEX IF EXISTS cb_seasons_live_number;

DROP TABLE cb_seasons;

ALTER TABLE cb_seasons_rolled_back RENAME TO cb_seasons;

DELETE FROM schema_migrations WHERE name = '0003_cb_season_controls';

COMMIT;

PRAGMA foreign_key_check;

PRAGMA foreign_keys = on;
```

What a person needs to know before running either, and what belongs in the README:
- **Back the database up first**: `cp data/barnacle.sqlite3 data/barnacle.sqlite3.bak`. Both files drop and recreate a table.
- **Apply with the bot stopped**: `sqlite3 data/barnacle.sqlite3 < migrations/0003_cb_season_controls.sql`.
- **The forward file runs once.** A second run aborts on the `schema_migrations` insert with nothing else executed, which is what the `.bail on` line is for.
- **The rollback loses data**: every ping role and every ended marker. It also fails, cleanly, if two ended seasons in one server share a number, because the old constraint cannot hold that; the human resolves those rows first.

The startup schema check in `attendance_store.rs` must be updated to the eleven columns above, in that order. `schema_migrations` is not checked: the bot never reads it.

## 5. Storage

`attendance_store.rs` changes.

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RoleId(u64);
```

`RoleId` lives in `ids.rs` beside `GuildId` and `ChannelId`, with the same `new`, `get` and `Display` shape.

```rust
pub struct Season {
    pub id: i64,
    pub guild: GuildId,
    pub channel: ChannelId,
    pub number: u32,
    pub codename: Option<String>,
    pub range: Range,
    pub created_by: UserId,
    pub created_at_ms: u64,
    pub ping_role: Option<RoleId>,
    pub ended_at_ms: Option<u64>,
}

pub struct NewSeason {
    pub guild: GuildId,
    pub channel: ChannelId,
    pub number: u32,
    pub codename: Option<String>,
    pub range: Range,
    pub created_by: UserId,
    pub created_at_ms: u64,
    pub ping_role: Option<RoleId>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SeasonChange {
    pub number: Option<u32>,
    pub first_day: Option<Date>,
    pub last_day: Option<Date>,
    pub codename: Option<Option<String>>,
    pub ping_role: Option<Option<RoleId>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditOutcome {
    Edited { before: Season, after: Season },
    NumberTaken,
    Overlaps(Season),
    BadRange,
    NotFound,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EndOutcome {
    Removed,
    Ended { season: Season },
    NotFound,
}

impl Attendance {
    pub async fn live_season(&self, guild: GuildId, number: u32) -> Result<Option<Season>, AttendanceError>;
    pub async fn edit_season(&self, guild: GuildId, number: u32, change: &SeasonChange) -> Result<EditOutcome, AttendanceError>;
    pub async fn move_season(&self, id: i64, channel: ChannelId) -> Result<(), AttendanceError>;
    pub async fn end_season(&self, guild: GuildId, number: u32, now_ms: u64) -> Result<EndOutcome, AttendanceError>;
}
```

Behaviour, exactly:

- **Every existing query that finds or lists seasons ignores ended ones.** `seasons_in`, `live_seasons`, the number check in `create_season` and its overlap check all add `ended_at_ms IS NULL`. `season(id)` still returns an ended season, because a click on a stale message must still resolve its season to refuse it.
- `SeasonChange` uses `Option<Option<T>>` for the two clearable fields: `None` leaves the value alone, `Some(None)` clears it, `Some(value)` sets it.
- `edit_season` runs in one `BEGIN IMMEDIATE` transaction: it finds the live season, applies the change to a copy, and returns `BadRange` when the resulting `Range::new` fails, `NumberTaken` when another live season in that guild holds the new number, `Overlaps(other)` when the new range overlaps another live season's, and otherwise writes the row and returns `Edited` with the season before and after.
- `end_season` finds the live season. With no `cb_posts` row at all it deletes the row and returns `Removed`. Otherwise it sets `ended_at_ms` and returns `Ended` with the season as it was, so the caller can clear that season's posts from the channel it was posting in.
- `move_season` writes the new channel and nothing else. The caller clears the old channel's posts **before** calling it, because a post's channel comes from its season and there is nowhere else to read the old one from.
- `record_post` becomes an upsert: `INSERT INTO cb_posts (...) VALUES (...) ON CONFLICT (season_id, night) DO UPDATE SET message_id = excluded.message_id, state = 'open', posted_at_ms = excluded.posted_at_ms`. A night whose message was cleared can then be posted again without disturbing its marks, which reference that row.
- Reading a row maps `ping_role_id` through `to_role`, which refuses zero and negatives with `AttendanceError::InvalidRole { value: i64 }`, whose message is `the attendance database holds {value}, which is not a role ID`.

## 6. The date rules

These are the whole of what an edit does to existing posts. They follow from one invariant already in the tracker: a night is posted only while its start is in the future, and a post exists only for a night inside its season's range.

| Change | Effect |
|---|---|
| `last_day` moves later | The nights it adds post when they come due. Nothing else happens. |
| `last_day` moves earlier | Every night after it stops being a CB night. Any post one of them still has is deleted from Discord now and its row marked `removed`. Its marks stay. |
| `first_day` moves later | The same, for the nights before it. |
| `first_day` moves earlier | Nothing is posted retroactively. The nights it adds are in the past, and a night whose start has passed is never posted. |
| The new range leaves no nights ahead | Allowed. The reply says nothing further will post. This is not an error: truncating a season deliberately is what it looks like. |
| The new range overlaps another live season | Refused, naming that season. |
| `last_day` earlier than `first_day` | Refused. |

A night whose post is swept this way keeps its answers, so re-extending the range and posting that night again shows the roster it already had.

## 7. The core

`attendance.rs` changes.

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Delivery {
    New,
    Redraw,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClearScope {
    All,
    OutsideRange,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PostsReport {
    pub touched: usize,
    pub failures: usize,
}

pub struct SignupView {
    pub season: i64,
    pub number: u32,
    pub codename: Option<String>,
    pub night: Night,
    pub open: bool,
    pub ping: Option<RoleId>,
    pub hours: [HourTally; 4],
    pub rows: Vec<RosterRow>,
    pub hidden: usize,
}

impl<B: Board> Signups<B> {
    pub async fn clear_posts(self: &Arc<Self>, season: &Season, scope: ClearScope, now_unix: i64) -> PostsReport;
    pub async fn refresh_posts(self: &Arc<Self>, season: &Season, now_unix: i64) -> PostsReport;
}
```

- `clear_posts` walks every post of the season whose state is not `removed`. With `ClearScope::All` it takes all of them; with `ClearScope::OutsideRange` only those whose night the season's range no longer holds. Each one is deleted through the board, marked `removed`, and its redraw slot dropped. A board failure counts in `failures` and leaves the row alone, so the next tick retries it.
- `refresh_posts` redraws every post of the season whose state is not `removed`, so an edited codename, number or ping role appears at once. It never sends a post.
- `tick` changes in two places. `live_seasons` no longer returns ended seasons, so an ended season is neither posted nor swept again. And its posting step now treats a post row in state `removed` as **not posted**, so a night whose message was cleared can be posted again. `store.record_post` becomes an upsert on `(season_id, night)`, setting the new message, `state = 'open'` and the new time, which keeps that night's marks pointing at the same row.

That pair is what makes a move work: the old channel's message is deleted and its row marked `removed`, and the next tick posts the same night in the new channel with its roster intact. It cannot make anything else re-post, because a night that was swept on its normal schedule has already started, and a night dropped by an edit is no longer in the season's range.
- `view` carries `season.ping_role` into `SignupView.ping`.
- `click` already refuses a night outside its season's range, which is what a click on a message that was swept by an edit will hit if the message somehow survives.

## 8. The board and the ping

`Board` gains the delivery distinction, because the two calls differ in exactly one way that matters:

```rust
pub trait Board: Send + Sync + 'static {
    fn send_post(&self, channel: ChannelId, view: &SignupView) -> impl Future<Output = Result<Snowflake, BoardError>> + Send;
    fn edit_post(&self, channel: ChannelId, message: Snowflake, view: &SignupView) -> impl Future<Output = Result<(), BoardError>> + Send;
    // find_post and delete_post are unchanged
}
```

The rule lives in two pure functions in `wiring.rs`, so it is testable without Discord:

```rust
pub fn ping_content(ping: Option<RoleId>) -> String;
pub fn ping_allowance(delivery: Delivery, ping: Option<RoleId>) -> Vec<RoleId>;
```

- `ping_content(Some(role))` is `<@&123>`; `ping_content(None)` is the empty string.
- `ping_allowance(Delivery::New, Some(role))` is that one role. **Every other case is empty**, including `Delivery::Redraw` with a role set.

`DiscordBoard` uses them:
- `send_post` sets the message content to `ping_content(view.ping)` and its allowed mentions to exactly `ping_allowance(Delivery::New, view.ping)`, with users and everyone still suppressed.
- `edit_post` sets the content to `ping_content(view.ping)` as well, so clearing or changing the role shows immediately, and sets allowed mentions to `ping_allowance(Delivery::Redraw, view.ping)`, which is empty. The mention still renders as a pill; nobody is notified.

Why this is written down rather than left to judgement: the roster is made of user mentions, and Discord re-parses a message's mentions on every edit using the allowances that edit carries. An edit that omitted them would ping every person on the roster, on every click.

Two facts about the ping that belong in the README rather than in code: a role that is not marked mentionable needs the bot to hold Mention @everyone, @here and All Roles in that channel, and a channel-level denial of that permission is enough to silence it.

## 9. The commands

All of `/cb` stays server-only and Manage Server by default, and every reply stays ephemeral.

### 9.1 `/cb season start`

Gains one option: `ping_role`, a Discord role, optional, described as `Role to ping when a sign-up is posted`. It is stored on the season. Every existing check is unchanged, except that "already exists" and "overlaps" now consider live seasons only.

### 9.2 `/cb season edit`

| Option | Type | Description |
|---|---|---|
| `number` | integer, required, min 1 | `Which season to change, for example 35` |
| `new_number` | integer, optional, min 1 | `Change the season number` |
| `first_day` | string, optional | `New first CB day, as 2026-09-16` |
| `last_day` | string, optional | `New last CB day, as 2026-11-05` |
| `codename` | string, optional, max 100 | `New codename, for example Komodo Dragon` |
| `ping_role` | role, optional | `Role to ping when a sign-up is posted` |
| `clear_codename` | boolean, optional | `Remove the codename` |
| `clear_ping_role` | boolean, optional | `Stop pinging a role` |

Checks, in order, each replying and stopping:
1. At least one change is named, else `text::NOTHING_TO_CHANGE`.
2. `codename` and `clear_codename` are not both given, else `text::CODENAME_BOTH_WAYS`; same for the ping pair with `text::PING_BOTH_WAYS`.
3. Any date given parses, else `text::DATE_FORMAT`.
4. `edit_season` returns `Edited`, else `text::season_not_found`, `text::LAST_DAY_BEFORE_FIRST`, `text::season_number_taken` or `text::season_overlaps`.

On success, in this order: `clear_posts(&after, ClearScope::OutsideRange, now)`, then `refresh_posts(&after, now)`, then the reply `text::season_edited(&after, nights_left, next_post_at, cleared)`.

### 9.3 `/cb season move`

Takes `number` only, and is **run in the channel the season should post in**, exactly as `start` is, so the bot's permissions there can be checked the same way.

Checks: the channel is a text channel; the bot has the four sign-up permissions here; a live season with that number exists; that season does not already post here, else `text::season_already_here`.

Then, in this order: `clear_posts(&season, ClearScope::All, now)` against the **old** channel, `move_season(season.id, here)`, and the reply `text::season_moved(&after, cleared, next_post_at)`. The tick re-posts the due night here within a minute if its start is still ahead, which is the one case where the room is pinged again.

### 9.4 `/cb season end`

Takes `number`. On `Removed`, replies `text::season_removed`. On `Ended`, it clears that season's posts with `clear_posts(&season, ClearScope::All, now)` and replies `text::season_ended(number, cleared)`. On `NotFound`, `text::season_not_found`.

Ending is immediate and complete: no further posts, and every sign-up it had in the channel is gone. The answers stay in the database, and the season number is free to use again.

### 9.5 `/cb season show`

Lists live seasons only. A season with a ping role adds ` Pings <@&123>.` to its line. Nothing is pinged by that reply: it is an interaction response, which parses user mentions only unless told otherwise, and it is told nothing.

## 10. Strings

New in `text.rs`, exactly:

```rust
pub const NOTHING_TO_CHANGE: &str = "Name at least one thing to change.";
pub const CODENAME_BOTH_WAYS: &str = "Pass a codename or clear it, not both.";
pub const PING_BOTH_WAYS: &str = "Pass a ping role or clear it, not both.";

pub fn season_already_here(number: u32) -> String;
pub fn season_edited(season: &Season, nights_left: usize, post_at_unix: Option<i64>, cleared: usize) -> String;
pub fn season_moved(season: &Season, cleared: usize, post_at_unix: Option<i64>) -> String;
pub fn season_ended(number: u32, cleared: usize) -> String;
pub fn pings_line(ping: Option<RoleId>) -> String;
```

Rendered output, exactly:

- `season_already_here(35)` is `Season 35 already posts in this channel.`
- `season_edited` is `Season 35: Komodo Dragon updated. 21 CB nights, 18 still ahead. Next sign-up post: <t:…:F> (<t:…:R>). Manage it with number 35.` When `cleared` is above zero, ` 2 sign-up posts outside the new dates were cleared.` is appended before the last sentence, and `1 sign-up post` is used at one. When nothing is ahead, the post sentence is `Nothing further will post.`
- `season_moved` is `Season 35: Komodo Dragon now posts in this channel. 1 sign-up post was cleared from the old channel. Next sign-up post: <t:…:F> (<t:…:R>).` With nothing cleared that middle sentence is left out, and the post sentence follows the same rules as above, including `within a minute` when one is already due.
- `season_ended(35, 2)` is `Season 35 has ended. 2 sign-up posts were cleared, and the answers are kept.` With nothing cleared it is `Season 35 has ended. The answers are kept.`
- `pings_line(Some(role))` is ` Pings <@&123>.`; `pings_line(None)` is the empty string. `season_line` appends it.
- The count words follow the existing helpers: one night is `1 CB night` and `1 still ahead`.

## 11. What this does not change

The night arithmetic, the fixed 23:30-03:30 UTC range, the posting and closing schedule, the redraw combining, the roster rendering and the click path are all untouched. An edit reaches the screen through the same redraw a click uses.

## 12. Tests

`tests/attendance_store.rs`
- `an_ended_season_frees_its_number` covering start, end, start again with the same number.
- `an_ended_season_is_not_live_and_not_listed`, checking `live_seasons` and `seasons_in`.
- `a_click_can_still_resolve_an_ended_season`, through `season(id)`.
- `edit_season_rejects_a_number_another_live_season_holds`.
- `edit_season_ignores_an_ended_season_when_checking_overlap`.
- `edit_season_rejects_a_backwards_range`.
- `edit_season_changes_only_what_it_is_given`, asserting each untouched field.
- `edit_season_clears_a_codename_and_a_ping_role`.
- `end_season_keeps_the_marks`.
- `a_bad_role_id_is_named`.

`tests/attendance.rs`
- `a_moved_season_reposts_the_same_night_with_its_roster`, clearing the post, changing the channel and ticking, then asserting one new message in the new channel and the same marks on it.
- `a_night_swept_on_schedule_is_never_reposted`.
- `clearing_all_posts_deletes_every_live_message` for an open and a closed post together.
- `clearing_outside_the_range_leaves_the_nights_that_remain`.
- `a_cleared_post_keeps_its_marks`.
- `refresh_posts_edits_and_never_sends`.
- `a_failed_delete_is_counted_and_retried_next_tick`.
- `an_ended_season_is_never_posted_again`.
- `the_view_carries_the_ping_role`.

`tests/wiring.rs`
- `a_ping_role_renders_as_a_role_mention`.
- `only_a_new_post_may_ping`, asserting `Delivery::Redraw` allows nothing even with a role set.

`tests/text.rs`
- `an_edited_season_reports_what_is_left_and_what_was_cleared`, including the singular and the nothing-ahead forms.
- `a_moved_season_names_the_new_channel_and_the_next_post`.
- `an_ended_season_says_the_answers_are_kept`.
- `a_season_line_names_its_ping_role`.

Live check for the owner, after merge: on a test season, set a ping role and confirm the role is notified once when the post appears and not again when someone clicks; edit the codename and watch the post's title change in place; shrink the dates past the posted night and watch that post disappear; move the season to another channel and watch it re-post; end the season and watch everything go.

## 13. Work split

One agent owns each item and edits only its files.

| Item | Files | Depends on |
|---|---|---|
| `cb2-store` | `crates/barnacle-bot/src/ids.rs`, `src/attendance_store.rs`, `tests/attendance_store.rs`, `migrations/0003_cb_season_controls.sql`, `migrations/0003_cb_season_controls.down.sql` | nothing |
| `cb2-readme` | `README.md` | nothing |
| `cb2-core` | `crates/barnacle-bot/src/attendance.rs`, `tests/attendance.rs`, `tests/common/fakes.rs` | `cb2-store` |
| `cb2-strings` | `crates/barnacle-bot/src/wiring.rs`, `src/text.rs`, `tests/wiring.rs`, `tests/text.rs` | `cb2-core` |
| `cb2-discord` | `crates/barnacle-bot/src/discord/board.rs`, `src/discord/commands.rs` | `cb2-strings` |

No item adds a module, so `lib.rs` is untouched. `cb2-readme` documents the `0003` step with its backup line, the ping role and the permission it needs, and the new meaning of ending a season.

## 14. Amendments from review

Five fresh-eyes reviews ran against the built branch. Where this section contradicts anything above it, this section is what shipped and the earlier text is the superseded design.

### 14.1 A night pings once, ever

Sections 7 and 8 keyed the ping to `Delivery::New` and assumed a night is delivered new exactly once. It is not. `clear_posts` marks a post row `removed`, and the tick treats a removed row as not posted so that a moved night can post again, which makes `Delivery::New` reachable as often as an operator runs `move` or an out-of-range `edit` and then undoes it. Every one of those repeats fired the role ping, so a server manager who cannot mention a role themselves could make the bot mention it without limit.

The tick now derives the delivery from whether a post row has ever existed for that night: absent means `Delivery::New`, present in any state means `Delivery::Redraw`. Rows are never deleted, only marked removed, so the row's existence is a durable record that the night has already been announced. `Board::send_post` takes the delivery rather than hard-coding `Delivery::New`. A move therefore re-posts without pinging, which the README states.

### 14.2 A failed clear stops the command

Sections 9.3 and 9.4 let `move` and `end` proceed after `clear_posts`, discarding its failure count. A refused delete then stranded a live sign-up message: on `end` the season was already marked ended, so the tick would never sweep it again and no command could reach it, while its buttons kept recording answers; on `move` the row still pointed at the old channel, so the night never posted in the new one.

`/cb season move` now clears first and returns `CLEAR_FAILED_MOVE` without writing the new channel when anything failed. `/cb season end` now looks the season up, clears, and returns `CLEAR_FAILED_END` without marking it ended when anything failed. Both leave the season exactly as it was, which is the state an operator can retry from. `/cb season edit` appends `REFRESH_FAILED` to its reply when a redraw did not land, because the tick only redraws nights already underway and will not repair a future night's post on its own. All three log the failure count at `warn`.

### 14.3 A redraw reads the season it is drawing

The redraw combiner collapses queued redraws of one message on the assumption that they would all render the same bytes. That held while a season's fields were immutable. Editing breaks it: a member's click and an admin's edit can interleave so that the click's redraw, holding the pre-edit season, satisfies the edit's slot, leaving the old codename on screen while the edit reports success. `redraw` now re-reads the season by id inside the lock and renders from that, which restores the assumption the combiner rests on.

The tick's posting step re-reads the season for the same reason, and skips it when it has ended. A sub-second window remains between that read and `record_post` in which a concurrent move could still place a post in the old channel; the night self-heals at its removal time and re-running the move recovers it. Closing that window entirely means storing each post's own channel on its row, which is recorded here as the durable fix rather than done now.

### 14.4 A click on an ended season is refused

`click` resolved a season by id without the ended filter, deliberately, so that a stale click could be refused rather than error. It then checked only the guild and the range, both of which an ended season still satisfies, so answers kept landing. `click` now returns `UnknownSeason` when the season carries `ended_at_ms`.

### 14.5 A season sits within six months of today

`Range::nights()` walks one calendar day at a time, and `parse_day` accepts any ISO date, so `0001-01-01` to `9999-12-31` made every tick and every command walk millions of days on the shared executor. The owner set the rule: a season's first and last day must both lie within six months either side of today. Seasons run two to four months and are announced a week or two ahead, so nothing legitimate is excluded.

`Range::near(today)` is the check, `schedule::today(now_unix)` supplies the day, `/cb season start` refuses with `RANGE_TOO_FAR` before creating, and `edit_season` takes `today` and returns the new `EditOutcome::TooFar`, because only the store knows the range an edit produces.

### 14.6 Smaller corrections

- `season_moved` takes `nights_left` and renders its last sentence through `next_post`, so moving a season with no nights left says `Nothing further will post.` instead of promising a post within a minute. Section 10's signature omitted the argument; the code followed the spec and the spec was wrong.
- `move_season` takes the guild, filters on `ended_at_ms IS NULL`, and returns whether a row changed, so a season ended between the command's lookup and its write no longer reports a successful move.
- The startup readiness check verifies that `cb_seasons_live_number` exists, not only that the eleven columns do, and `StartupError::SeasonControls` names `migrations/0003_cb_season_controls.sql`. The single previous message named `0002_cb_attendance.sql`, which is `CREATE TABLE IF NOT EXISTS` throughout and would have told every existing deployment to run a file that changes nothing.
- `cb` carries `required_permissions = "MANAGE_GUILD"` as well as `default_member_permissions`, because poise applies the latter once at registration and a guild owner can override it per role under Integrations.
- `private` sets an explicitly empty allowed-mentions allowance. Nothing was notified before, but only because the reply is ephemeral, which is not the reason section 9.5 gave.
- `season_name` escapes the codename, as every other user-supplied string in `text.rs` already did.
- `forget` drops its slot unconditionally. Requiring a strong count of one silently skipped the removal whenever a redraw was in flight, and nothing revisited a removed post, so the entry lived for the life of the process.
- The shared test fixture in `tests/common/mod.rs` applies `0003`, and the duplicate private copies in the two test files are gone. Both files had shadowed the shared helper, leaving it building a schema every store query fails against.
- `FakeBoard` records each send's delivery and exposes `deletes_in`, so a test can tell which channel a delete went to. Without it, the move test passed whether or not the delete reached the right channel.
