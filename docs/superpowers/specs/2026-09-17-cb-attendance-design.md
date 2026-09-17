# Design: Clan Battle attendance sign-ups

Date: 2026-09-17
Status: approved by the owner on 2026-09-17; implementation spec.
Extends: `docs/superpowers/specs/2026-09-16-barnacle-bot-design.md` (the bot as built in `crates/barnacle-bot`).
Logbook thread: `01M2R6RSSPA72GZHHK2881S5W9` (`cb-attendance-tracker`).
Built by: a `fanout` run, one agent per work item; the item table is section 16.

This document is the only briefing the implementing agents get. Anything not written here is not decided, and an agent that finds a gap stops and reports it rather than inventing an answer.

## 1. Scope

In scope:
- A sign-up post for every Clan Battle (CB) night in a season, posted in one channel 24 hours before the night starts.
- Four one-hour slots per night. A player marks each hour Attending or Nope, or all four at once.
- `/cb season start`, `/cb season show` and `/cb season end` for server managers.
- Three SQLite tables, with migration and rollback files a person applies.
- README steps for the new migration, the sign-up channel and the stored data.

Out of scope:
- Reminders or pings, lineup picking, attendance stats or history commands.
- Editing a season in place. A wrong season is ended and started again.
- Different times per season, or times in the config file.
- Discord's Components V2 layout, which serenity 0.12.5 does not support (no `TextDisplay` or `IS_COMPONENTS_V2` in its source).
- Hosting anywhere other than the owner's machine.

## 2. Decisions

| Question | Decision | Record |
|---|---|---|
| How are CB times stored? | One fixed UTC range, 23:30-03:30, derived once from 17:30-21:30 MDT. The MDT range is never stored, never configured and never re-derived. Discord timestamps show each viewer their own local time. | `01M2R7PXSVX4DK6R2SMVNAGY35` |
| How are players named on the roster? | Discord mentions, `<@id>`, with pings suppressed. No names are stored or fetched. | `01M2R8JR21RCFQDDB2AJMWNTXH` |
| When is a night posted, closed and removed? | Posted any time from 24 hours before the start until the start. Closed at the start. Deleted 30 minutes after the end. Before sending, the bot checks its records and then the channel for an existing post. | `01M2R8JXD4XSTR4EBFSTRB7WC8` |
| Do Discord edit limits shape the design? | No. Redraws are still combined, to avoid stale rosters and needless edits. | `01M2R8K0NEVK0J0NGSEW18PQX3` |

Intent behind those decisions, so a reader can tell a bug from a choice:
- **Time is an instant, never a wall clock.** Every moment in this feature is a Unix second. No time zone is ever looked up, no `America/*` identifier appears anywhere in the code, and the machine's own clock setting changes nothing. A viewer's local time is Discord's job.
- **The sign-up post is disposable; the answers are not.** Messages are posted, closed and deleted on a schedule. `cb_marks` rows outlive them.
- **The bot is offline half the time.** Every step is driven by comparing stored state against the current clock, so any step missed while the bot was down runs on the first tick after it returns, and never runs twice.
- **The core knows nothing about Discord.** It is tested with a fake board and chosen times, which is where the restart, crash and race behaviour is proven.

## 3. Principles every implementer follows

1. **Never write a comment, docstring or doc comment.** Not in Rust, not in SQL, not in TOML. Name things so the code reads without them. This overrides any habit and any example.
2. **No emojis anywhere**, including button labels and message text.
3. **Create new values; never mutate a shared one in place.** Return a new struct with the change.
4. **Follow the file you are editing.** Error types are `thiserror` enums with lowercase messages. User-facing sentences live in `text.rs`. Newtypes from `ids.rs` cross module boundaries, never serenity types.
5. **The core is Discord-free.** `schedule`, `attendance_store` and `attendance` must not import `poise` or `serenity`. `attendance` talks to Discord only through the `Board` trait.
6. **Tests come with the code**, in `crates/barnacle-bot/tests/`, following the existing files. Section 15 lists the tests each item owes.
7. **Before you finish**: `cargo fmt --all`, then `cargo clippy --workspace --all-targets -- -D warnings`, then `cargo test --workspace`. All three must pass. Do not weaken a lint, do not `#[allow]` your way out, do not delete a failing test you did not write.
8. **Stay inside your file list.** Another agent owns every other file in this repo right now. If your work seems to need a file you do not own, stop and report it instead of editing it.
9. **Do not run git.** No commit, no branch, no push. The runner commits your worktree.
10. **Every value in this spec is exact.** Strings, table names, column names, button IDs, limits and error sentences are copied as written, not paraphrased.

## 4. Dependencies

| Crate | Version | License | Why |
|---|---|---|---|
| `jiff` | 0.2 (0.2.37 already in `Cargo.lock` through `wows-data-mgr`) | Unlicense OR MIT | Parses `2026-09-16`, gives the weekday, walks a date range, and converts a UTC date and time to a Unix second |

Workspace `Cargo.toml` gains, in the existing alphabetical position:

```toml
jiff = { version = "0.2", default-features = false, features = ["std"] }
```

`crates/barnacle-bot/Cargo.toml` gains `jiff.workspace = true` in its dependency list, alphabetically.

Turning the default features off is load-bearing: it removes the time zone database, so no code can accidentally depend on the machine's time zone rules. A build with these features was run against this schedule; a stray `in_tz("America/Denver")` call fails loudly with "no time zone database configured" rather than working by accident. `deny.toml` already allows MIT, which satisfies the `OR` expression.

## 5. Time rules and the `schedule` module

### 5.1 The rules

All moments are Unix seconds. A CB night is named by its date D, the UTC date on which its first hour starts.

| Moment | Value | Season 35's Sunday 2026-11-01 |
|---|---|---|
| Sign-up post due | start - 86400 | 1793489400 |
| Night starts, sign-ups close | D at 23:30 UTC | 1793575800 |
| Hour h (1 to 4) starts | start + (h-1) * 3600 | hour 3: 1793583000 |
| Night ends | start + 14400 | 1793590200 |
| Post deleted | end + 1800 | 1793592000 |

A season's nights are every Wednesday, Thursday, Saturday and Sunday from `first_day` to `last_day`, both included, by UTC date. Season 35, 2026-09-16 to 2026-11-05, has 30 nights, the first on 2026-09-16 and the last on 2026-11-05.

The due night is the earliest night in the season whose start is still in the future, and it is due once its post time has passed. That single rule produces the late-setup behaviour with no stored cursor:

| Now (UTC) | Now (MDT) | Due night |
|---|---|---|
| 2026-09-22 12:00 | Tue 06:00 | none; the next night is 2026-09-23 |
| 2026-09-23 00:00 | Tue 18:00 | 2026-09-23 |
| 2026-09-23 23:29 | Wed 17:29 | 2026-09-23 |
| 2026-09-23 23:30 | Wed 17:30 | 2026-09-24 |
| 2026-11-05 23:30 | Thu 17:30 | none; the season is over |

### 5.2 `crates/barnacle-bot/src/schedule.rs`

No dependency but `jiff`. Every public item below exists exactly as written.

```rust
use jiff::ToSpan;
use jiff::civil::Date;
use jiff::civil::Weekday;
use jiff::civil::time;
use jiff::tz::Offset;

pub const CB_WEEKDAYS: [Weekday; 4] = [
    Weekday::Wednesday,
    Weekday::Thursday,
    Weekday::Saturday,
    Weekday::Sunday,
];
pub const START_HOUR: i8 = 23;
pub const START_MINUTE: i8 = 30;
pub const HOURS_PER_NIGHT: u8 = 4;
pub const SECONDS_PER_HOUR: i64 = 3600;
pub const POST_LEAD_SECONDS: i64 = 24 * SECONDS_PER_HOUR;
pub const REMOVE_AFTER_END_SECONDS: i64 = 30 * 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Hour(u8);

impl Hour {
    pub const ALL: [Hour; 4] = [Hour(1), Hour(2), Hour(3), Hour(4)];
    pub fn new(value: u8) -> Option<Self>;
    pub fn get(self) -> u8;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Night {
    date: Date,
    start_unix: i64,
}

impl Night {
    pub fn new(date: Date) -> Option<Self>;
    pub fn parse(text: &str) -> Option<Self>;
    pub fn date(self) -> Date;
    pub fn label(self) -> String;
    pub fn start_unix(self) -> i64;
    pub fn hour_start_unix(self, hour: Hour) -> i64;
    pub fn hour_end_unix(self, hour: Hour) -> i64;
    pub fn end_unix(self) -> i64;
    pub fn post_at_unix(self) -> i64;
    pub fn remove_at_unix(self) -> i64;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Range {
    first_day: Date,
    last_day: Date,
}

impl Range {
    pub fn new(first_day: Date, last_day: Date) -> Option<Self>;
    pub fn first_day(self) -> Date;
    pub fn last_day(self) -> Date;
    pub fn nights(self) -> impl Iterator<Item = Night>;
    pub fn night_count(self) -> usize;
    pub fn nights_left(self, now_unix: i64) -> usize;
    pub fn holds(self, night: Night) -> bool;
    pub fn next_night(self, now_unix: i64) -> Option<Night>;
    pub fn due_night(self, now_unix: i64) -> Option<Night>;
    pub fn overlaps(self, other: Range) -> bool;
    pub fn last_moment_unix(self) -> Option<i64>;
}

pub fn parse_day(text: &str) -> Option<Date>;
```

Behaviour, exactly:

- `Hour::new` returns `Some` for 1 to 4 and `None` otherwise.
- `Night::new` returns `None` unless `date.weekday()` is in `CB_WEEKDAYS`; it computes the start as `Offset::UTC.to_timestamp(date.to_datetime(time(START_HOUR, START_MINUTE, 0, 0))).ok()?.as_second()`.
- `Night::parse` is `Night::new(parse_day(text)?)`.
- `Night::label` is the date as `YYYY-MM-DD`; this is what goes in a button ID and in the `night` column.
- `hour_start_unix` is `start_unix + (hour.get() - 1) * SECONDS_PER_HOUR`; `hour_end_unix` is that plus `SECONDS_PER_HOUR`; `end_unix` is `start_unix + 4 * SECONDS_PER_HOUR`; `post_at_unix` is `start_unix - POST_LEAD_SECONDS`; `remove_at_unix` is `end_unix() + REMOVE_AFTER_END_SECONDS`.
- `parse_day` accepts only a 10-character `YYYY-MM-DD` string: it returns `None` unless the text is 10 bytes long and `text.parse::<Date>()` succeeds. `2026-9-16`, `2026/09/16` and `2026-09-31` all give `None`.
- `Range::new` returns `None` when `last_day < first_day`.
- `nights` walks `first_day.series(1.day())` while the date is at most `last_day` and keeps the dates `Night::new` accepts.
- `nights_left(now)` counts nights whose `start_unix() > now`.
- `next_night(now)` is the first night with `start_unix() > now`.
- `due_night(now)` is `next_night(now)` when its `post_at_unix() <= now`, otherwise `None`.
- `overlaps` is `self.first_day <= other.last_day && other.first_day <= self.last_day`.
- `last_moment_unix` is the `remove_at_unix()` of the last night in the range, or `None` when the range holds no night.

## 6. Storage

### 6.1 `migrations/0002_cb_attendance.sql`

```sql
BEGIN;

CREATE TABLE IF NOT EXISTS cb_seasons (
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

CREATE TABLE IF NOT EXISTS cb_posts (
    season_id INTEGER NOT NULL REFERENCES cb_seasons (id),
    night TEXT NOT NULL,
    message_id INTEGER NOT NULL,
    state TEXT NOT NULL CHECK (state IN ('open', 'closed', 'removed')),
    posted_at_ms INTEGER NOT NULL,
    PRIMARY KEY (season_id, night)
) STRICT;

CREATE TABLE IF NOT EXISTS cb_marks (
    season_id INTEGER NOT NULL,
    night TEXT NOT NULL,
    user_id INTEGER NOT NULL,
    hour INTEGER NOT NULL CHECK (hour BETWEEN 1 AND 4),
    attending INTEGER NOT NULL CHECK (attending IN (0, 1)),
    answered_at_ms INTEGER NOT NULL,
    changed_at_ms INTEGER NOT NULL,
    PRIMARY KEY (season_id, night, user_id, hour),
    FOREIGN KEY (season_id, night) REFERENCES cb_posts (season_id, night)
) STRICT;

COMMIT;
```

`migrations/0002_cb_attendance.down.sql`:

```sql
BEGIN;

DROP TABLE IF EXISTS cb_marks;
DROP TABLE IF EXISTS cb_posts;
DROP TABLE IF EXISTS cb_seasons;

COMMIT;
```

Why each table exists:
- `cb_seasons` is the season you set up: where to post, which season it is, and which dates it covers.
- `cb_posts` is one row per posted night. It stops a restart from posting twice, holds the message ID needed to close and delete the post, and tracks which lifecycle steps have run.
- `cb_marks` is the answers, one row per player per hour. `answered_at_ms` is set once, on the first answer for that hour, and never changed; `changed_at_ms` moves on every write.

Dates are `YYYY-MM-DD` text, which SQLite orders correctly as text. sqlx turns foreign keys on by default.

### 6.2 `crates/barnacle-bot/src/attendance_store.rs`

Mirrors `solves.rs`: a `SqlitePool`, a startup schema check, and `i64` conversion helpers that fail rather than truncate.

```rust
#[derive(Debug, thiserror::Error)]
pub enum AttendanceError {
    #[error("the attendance database could not be used")]
    Database(#[from] sqlx::Error),
    #[error("the attendance database has no {table} table")]
    MissingTable { table: &'static str },
    #[error("{table} does not have the expected columns; found {found:?}")]
    UnexpectedColumns { table: &'static str, found: Vec<Column> },
    #[error("{value} does not fit in a SQLite integer")]
    OutOfRange { value: u128 },
    #[error("the attendance database holds a negative value, {value}")]
    Negative { value: i64 },
    #[error("the attendance database holds a player ID below 1, {value}")]
    InvalidPlayer { value: i64 },
    #[error("the attendance database holds {value}, which is not a CB night")]
    BadNight { value: String },
    #[error("the attendance database holds {value}, which is not a post state")]
    BadState { value: String },
    #[error("the attendance database holds {value}, which is not an hour")]
    BadHour { value: i64 },
    #[error("the attendance database holds {value}, which is not a season number")]
    BadSeasonNumber { value: i64 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Season {
    pub id: i64,
    pub guild: GuildId,
    pub channel: ChannelId,
    pub number: u32,
    pub codename: Option<String>,
    pub range: Range,
    pub created_by: UserId,
    pub created_at_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewSeason {
    pub guild: GuildId,
    pub channel: ChannelId,
    pub number: u32,
    pub codename: Option<String>,
    pub range: Range,
    pub created_by: UserId,
    pub created_at_ms: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PostState {
    Open,
    Closed,
    Removed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Post {
    pub night: Night,
    pub message: Snowflake,
    pub state: PostState,
    pub posted_at_ms: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Mark {
    pub user: UserId,
    pub hour: Hour,
    pub attending: bool,
    pub answered_at_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CreateOutcome {
    Created(Season),
    NumberTaken,
    Overlaps(Season),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EndOutcome {
    Removed,
    Shortened { last_day: Date },
    NotFound,
}

#[derive(Debug, Clone)]
pub struct Attendance { /* pool */ }

impl Attendance {
    pub async fn open(path: &Path) -> Result<Self, AttendanceError>;
    pub async fn with_pool(pool: SqlitePool) -> Result<Self, AttendanceError>;
    pub async fn create_season(&self, new: &NewSeason) -> Result<CreateOutcome, AttendanceError>;
    pub async fn season(&self, id: i64) -> Result<Option<Season>, AttendanceError>;
    pub async fn seasons_in(&self, guild: GuildId) -> Result<Vec<Season>, AttendanceError>;
    pub async fn live_seasons(&self, now_unix: i64) -> Result<Vec<Season>, AttendanceError>;
    pub async fn end_season(&self, guild: GuildId, number: u32) -> Result<EndOutcome, AttendanceError>;
    pub async fn post(&self, season: i64, night: Night) -> Result<Option<Post>, AttendanceError>;
    pub async fn posts(&self, season: i64) -> Result<Vec<Post>, AttendanceError>;
    pub async fn record_post(&self, season: i64, night: Night, message: Snowflake, at_ms: u64) -> Result<(), AttendanceError>;
    pub async fn set_post_state(&self, season: i64, night: Night, state: PostState) -> Result<(), AttendanceError>;
    pub async fn mark(&self, season: i64, night: Night, user: UserId, hours: &[Hour], attending: bool, at_ms: u64) -> Result<(), AttendanceError>;
    pub async fn roster(&self, season: i64, night: Night) -> Result<Vec<Mark>, AttendanceError>;
}
```

Behaviour, exactly:

- `with_pool` checks `cb_seasons`, `cb_posts` and `cb_marks` with the same `pragma_table_info` query `solves.rs` uses, in the column order of the migration above. A missing table is `MissingTable`; wrong columns are `UnexpectedColumns`.
- `create_season` runs in one transaction: `NumberTaken` when this guild already has that season number; `Overlaps(existing)` when another season in this guild has `first_day <= new.last_day AND last_day >= new.first_day`, returning the first such season ordered by `first_day`; otherwise it inserts and returns `Created` with the assigned `id`.
- `seasons_in` returns every season in that guild ordered by `first_day`.
- `live_seasons(now)` returns seasons from every guild that still have work: either `Range::last_moment_unix()` is greater than `now`, or the season holds a `cb_posts` row whose `state` is not `removed`. The second half is load-bearing. A season's last removal moment is exactly its `last_moment_unix()`, so a liveness test on that alone drops the season one instant before the tick would sweep its final post, and that post is then never deleted. The SQL selects `last_day >= ?`, passing the UTC date two days before `now`, or `EXISTS (SELECT 1 FROM cb_posts WHERE season_id = cb_seasons.id AND state <> 'removed')`, and returns that existence as a column so the exact test can be done in Rust.
- `end_season` finds the season by guild and number: `NotFound` when there is none; `Removed` when it has no `cb_posts` row, deleting it; otherwise `Shortened` with `last_day` set to the latest `night` in `cb_posts` for that season, updating the row.
- `record_post` inserts with `state = 'open'`; it is an error for the row to already exist, so callers check `post` first.
- `set_post_state` updates one row's `state`.
- `mark` writes one row per hour in `hours` in one transaction, each with `INSERT INTO cb_marks (...) VALUES (...) ON CONFLICT (season_id, night, user_id, hour) DO UPDATE SET attending = excluded.attending, changed_at_ms = excluded.changed_at_ms`, so `answered_at_ms` keeps its first value.
- `roster` returns every mark for that night ordered by `answered_at_ms`, then `user_id`, then `hour`.
- `create_season` and `mark` open their transaction with `BEGIN IMMEDIATE`, taking the write lock before the read. `create_season`'s overlap rule has no constraint behind it, so a deferred transaction lets two overlapping seasons with different numbers both pass the check and both insert.
- Every `u64` to `i64` conversion goes through a checked helper, as `solves.rs` does. A stored value that is out of range, negative, not a CB night, not a state, not an hour or not a season number is an error, never a silent default. A season number above `u32::MAX` is `BadSeasonNumber`: the column already forbids negatives, so reporting it as one would be a false sentence.

### 6.3 One pool

`startup.rs` opens the SQLite pool once and hands it to both stores. `Solves::open` keeps working for its tests; the binary path builds the pool, then `Solves::with_pool` and `Attendance::with_pool`, both of which already take one. `solves.rs` is not edited: `attendance_store.rs` carries its own `columns` helper over the three new tables rather than reaching into another item's file for fifteen lines.

`StartupError` gains:

```rust
#[error(
    "the attendance tables are missing from {path}; create them with `sqlite3 {path} < migrations/0002_cb_attendance.sql`"
)]
Attendance {
    path: PathBuf,
    #[source]
    source: AttendanceError,
},
```

## 7. The core: `crates/barnacle-bot/src/attendance.rs`

Discord-free. It owns the lifecycle, the click handling and the redraw combining, and it reaches Discord only through `Board`.

```rust
#[derive(Debug, thiserror::Error)]
#[error("the Discord call failed")]
pub struct BoardError(#[source] pub Box<dyn std::error::Error + Send + Sync>);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Removal {
    Deleted,
    Gone,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PostTag {
    pub season: i64,
    pub night: Night,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cell {
    In,
    Out,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HourTally {
    pub hour: Hour,
    pub attending: u32,
    pub nope: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RosterRow {
    pub user: UserId,
    pub cells: [Cell; 4],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignupView {
    pub season: i64,
    pub number: u32,
    pub codename: Option<String>,
    pub night: Night,
    pub open: bool,
    pub hours: [HourTally; 4],
    pub rows: Vec<RosterRow>,
    pub hidden: usize,
}

pub trait Board: Send + Sync + 'static {
    fn send_post(
        &self,
        channel: ChannelId,
        view: &SignupView,
    ) -> impl Future<Output = Result<Snowflake, BoardError>> + Send;

    fn find_post(
        &self,
        channel: ChannelId,
        tag: PostTag,
    ) -> impl Future<Output = Result<Option<Snowflake>, BoardError>> + Send;

    fn edit_post(
        &self,
        channel: ChannelId,
        message: Snowflake,
        view: &SignupView,
    ) -> impl Future<Output = Result<(), BoardError>> + Send;

    fn delete_post(
        &self,
        channel: ChannelId,
        message: Snowflake,
    ) -> impl Future<Output = Result<Removal, BoardError>> + Send;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    All,
    One(Hour),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Click {
    pub guild: GuildId,
    pub channel: ChannelId,
    pub message: Snowflake,
    pub user: UserId,
    pub season: i64,
    pub night: Night,
    pub target: Target,
    pub attending: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClickOutcome {
    Recorded,
    Closed { start_unix: i64 },
    UnknownSeason,
    Failed,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TickReport {
    pub posted: Vec<PostTag>,
    pub adopted: Vec<PostTag>,
    pub closed: Vec<PostTag>,
    pub removed: Vec<PostTag>,
    pub failures: usize,
}

pub const ROSTER_LIMIT: usize = 60;

pub struct Signups<B: Board> { /* board, store, redraw slots */ }

impl<B: Board> Signups<B> {
    pub fn new(board: B, store: Attendance) -> Arc<Self>;
    pub fn store(&self) -> &Attendance;
    pub async fn tick(self: &Arc<Self>, now_unix: i64, now_ms: u64) -> TickReport;
    pub async fn click(self: &Arc<Self>, click: Click, now_unix: i64, now_ms: u64) -> ClickOutcome;
    pub async fn view(&self, season: &Season, night: Night, now_unix: i64) -> Result<SignupView, AttendanceError>;
}
```

### 7.1 `tick`

For every season from `live_seasons(now_unix)`, in this order, and never stopping the whole tick because one step failed:

1. **Remove.** For every post from `posts(season.id)` whose `state` is not `Removed` and whose `night.remove_at_unix() <= now_unix`: call `delete_post`. On `Deleted` or `Gone`, set the state to `Removed` and add the tag to `removed`. On `Err`, count a failure and leave the row alone.
2. **Close.** For every post whose `state` is `Open` and whose `night.start_unix() <= now_unix`: redraw it (section 7.3). On success set the state to `Closed` and add the tag to `closed`. On `Err`, count a failure.
3. **Post.** Take `season.range.due_night(now_unix)`. If there is one and `post(season.id, night)` is `None`:
   1. `find_post(season.channel, PostTag { season: season.id, night })`.
   2. `Some(message)` means a previous run sent it and died before recording it: record it and add the tag to `adopted`.
   3. `None` means send it. Build the view with an empty roster, `send_post`, record the returned message ID, and add the tag to `posted`.
   4. Record with `record_post(..., now_ms)`. Sending before recording is deliberate: a crash in between leaves a message that the next tick adopts, where recording first would leave a night that is never posted.
   5. On `Err` from either call, count a failure and try again on the next tick.

A failure in any step is logged by the caller through `TickReport.failures` plus a `tracing::warn!` at the call site in `discord.rs`; the core itself does not log.

### 7.2 `click`

1. `store.season(click.season)`. `None`, or a season whose `guild` is not `click.guild`, gives `UnknownSeason`. So does a night the season's range does not hold.
2. `click.night.start_unix() <= now_unix` gives `Closed { start_unix }`, and nothing is written.
3. `store.mark(...)` with `Target::All` expanding to `Hour::ALL` and `Target::One(h)` to `[h]`. An error gives `Failed`.
4. Redraw the message (section 7.3). A redraw error does not change the outcome, which is `Recorded`.

### 7.3 Redraw combining

Each message has a slot: a `tokio::sync::Mutex<()>`, a `wanted: AtomicU64` and a `drawn: AtomicU64`, kept in a `std::sync::Mutex<HashMap<Snowflake, Arc<Slot>>>`.

A redraw request for a message:
1. Take or create that message's slot, then `let mine = slot.wanted.fetch_add(1, SeqCst) + 1;`.
2. Await the slot's mutex.
3. If `slot.drawn.load(SeqCst) >= mine`, a later redraw already showed this change: return `Ok(())` without an edit.
4. Otherwise read `let target = slot.wanted.load(SeqCst);`, build the view from the store, call `edit_post`, and on success `slot.drawn.store(target, SeqCst)`.

The guarantees this buys, which the tests in section 15 pin:
- **No stale roster.** Every write that happened before a request's `fetch_add` is included in the edit that satisfies it, and edits run one at a time per message.
- **Bursts collapse.** Twenty clicks arriving during one slow edit produce at most two further edits.
- **The tick can trust it.** `tick` marks a post `Closed` only when its redraw returned `Ok`.

A slot is removed from the map when its post is removed, and only when the entry still in the map is the same `Arc` the remover looked up. Removing by key alone would let a later redraw build a fresh slot, with its own mutex and zeroed counters, while a redraw on the old slot is still in flight, which is two concurrent edits of one message.

### 7.4 `view`

`view` reads `roster(season, night)` and folds it into a `SignupView`:
- `open` is `now_unix < night.start_unix()`.
- `hours[i]` counts the marks for that hour: `attending` where the mark is true, `nope` where it is false.
- `rows` holds one row per player in roster order (first answer first), with `cells[h-1]` set from that player's mark for hour h, or `Cell::None` when they have no mark for it.
- At most `ROSTER_LIMIT` rows are kept; `hidden` is how many players were left out.
- The fold runs once over the marks, keeping an index from player to row, because it runs inside the redraw lock. Scanning the rows per mark is quadratic in the number of players and would block that message's edits while it ran.

## 8. Strings and IDs

### 8.1 `crates/barnacle-bot/src/wiring.rs`

```rust
pub const SIGNUP_PREFIX: &str = "barnacle-cb:";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SignupClick {
    pub season: i64,
    pub night: Night,
    pub target: Target,
    pub attending: bool,
}

pub fn signup_button_id(season: i64, night: Night, target: Target, attending: bool) -> String;
pub fn signup_tag_prefix(season: i64, night: Night) -> String;
pub fn signup_click(custom_id: &str) -> Option<SignupClick>;
pub fn missing_signup_permissions(access: ChannelAccess) -> Vec<&'static str>;
```

`missing_signup_permissions` is the sign-up channel's permission set: View Channel, Send Messages, Embed Links and Read Message History. It sits beside the existing `missing_permissions`, which keeps its own set for `/guess`, and neither changes the other.

The grammar is `barnacle-cb:{season}:{night}:{target}:{choice}` where `target` is `all`, `1`, `2`, `3` or `4` and `choice` is `in` or `out`; `night` is `Night::label`. `barnacle-cb:3:2026-09-23:all:in` is an example. `signup_tag_prefix` returns everything through the night and its trailing colon. `signup_click` accepts only what `signup_button_id` can produce and returns `None` for everything else, including an unknown target, an unknown choice, a date that is not a CB night, and a season that is not a plain positive integer: `+3` and `003` are rejected, because `i64::from_str` accepts them and the bot can never write them.

### 8.2 `crates/barnacle-bot/src/text.rs`

New constants and functions; every sentence is exactly as written here.

```rust
pub const ATTEND_ALL_LABEL: &str = "Attend all";
pub const NOPE_ALL_LABEL: &str = "Nope all";
pub const NO_ANSWERS_YET: &str = "No one has answered yet.";
pub const SIGNUPS_CLOSE_AT_START: &str = "Sign-ups close when the night starts.";
pub const SIGNUPS_CLOSED: &str = "Sign-ups closed";
pub const SEASON_GONE: &str = "This season no longer exists.";
pub const RUN_IN_TEXT_CHANNEL: &str = "Run this in a text channel.";
pub const DATE_FORMAT: &str = "Dates look like 2026-09-16.";
pub const LAST_DAY_BEFORE_FIRST: &str = "The last day is before the first day.";
pub const NO_NIGHTS_LEFT: &str = "That range has no CB nights left.";
pub const NO_SEASON_HERE: &str = "No CB season is set up here.";
pub const ROSTER_HEADER: &str = "1    2    3    4    ";

pub fn signup_title(number: u32, codename: Option<&str>) -> String;
pub fn signup_description(view: &SignupView) -> String;
pub fn hour_button_label(hour: Hour, attending: bool) -> String;
pub fn signups_closed_at(start_unix: i64) -> String;
pub fn season_started(season: &Season, nights_left: usize, post_at_unix: Option<i64>) -> String;
pub fn season_line(season: &Season, nights_left: usize, post_at_unix: Option<i64>) -> String;
pub fn season_number_taken(number: u32) -> String;
pub fn season_overlaps(other: &Season) -> String;
pub fn season_removed(number: u32) -> String;
pub fn season_shortened(number: u32, last_day: Date) -> String;
pub fn season_not_found(number: u32) -> String;
pub fn season_list(lines: &[String]) -> String;
```

`season_list` joins the lines with newlines while the result stays inside `MESSAGE_CONTENT_LIMIT`, which is 2000, and ends with `and 3 more.` when it had to stop. It lives here rather than in the command so it can be tested without Discord.

Rendered output, exactly:

- `signup_title(35, None)` is `Clan Battles · Season 35`; with a codename it is `Clan Battles · Season 35: Komodo Dragon`.
- `hour_button_label` is `Hour 1: Attending` or `Hour 1: Nope`.
- `signups_closed_at(s)` is ``Sign-ups for this night closed at <t:{s}:t>.``
- `season_number_taken(35)` is `Season 35 already exists. End it first with /cb season end.`
- `season_overlaps(other)` is `That range overlaps Season 34 (2026-06-10 to 2026-08-02).`
- `season_removed(35)` is `Season 35 was removed. Nothing had been posted.`
- `season_shortened(35, d)` is `Season 35 ends after 2026-10-01. No more sign-up posts.`
- `season_not_found(35)` is `No Season 35 is set up here.`
- `season_started` is `Season 35: Komodo Dragon will post here. 30 CB nights, 23 still ahead. First sign-up post: <t:1790811000:F> (<t:1790811000:R>).` Without a codename the first clause is `Season 35 will post here.`; when `post_at_unix` is `None`, because the next post is already due, the last sentence is `First sign-up post: within a minute.`
- `season_line` is the same shape as `season_started` for `/cb season show`, prefixed with the channel mention: `<#123> Season 35: Komodo Dragon, 2026-09-16 to 2026-11-05. 23 nights ahead. Next sign-up post: <t:…:F> (<t:…:R>).`
- Both count sentences go singular at one: `1 CB night, 1 still ahead` and `1 night ahead`. Every season reaches one night ahead on its last night, so the plural form is not an edge case. `standing_line` in the same file already has the shape to follow.

`signup_description(view)` builds these lines, joined with `\n`:

1. Open: ``<t:{start}:F> · starts <t:{start}:R>`` then `SIGNUPS_CLOSE_AT_START`. Closed: ``<t:{start}:F> · Sign-ups closed`` and no second line.
2. An empty line.
3. One line per hour: ``**Hour 1** · <t:{hour_start}:t> – <t:{hour_end}:t> · 6 in · 1 out``, using an en dash between the times and the tally's own numbers.
4. An empty line.
5. The roster. With no rows, `NO_ANSWERS_YET`. Otherwise `` `1    2    3    4    ` `` followed by one line per row: a backtick, the four cells, a backtick, a space, and `<@{user}>`. A cell is `in`, `out` or `-`, each padded on the right to five characters, so the code span is always 20 characters. When `hidden` is above zero, a last line reads `and 7 more.`

The em dash, the middle dot and the en dash in these strings are exact. There are no emojis.

## 9. The Discord layer

### 9.1 `crates/barnacle-bot/src/discord/board.rs`

`DiscordBoard { http: Arc<serenity::Http>, bot: serenity::UserId }` implements `Board`.

- **The message.** One embed, `text::EMBED_COLOUR`, title from `text::signup_title`, description from `text::signup_description`, and five action rows. Row one is `ATTEND_ALL_LABEL` as a green (`ButtonStyle::Success`) button and `NOPE_ALL_LABEL` as a red (`ButtonStyle::Danger`) one. Rows two to five hold `Hour n: Attending` in green and `Hour n: Nope` in red. Button IDs come from `wiring::signup_button_id`. Every button is `disabled(true)` when `view.open` is false.
- **`send_post`** sends that message with `CreateAllowedMentions::new()` so nothing is pinged, and returns the new message ID.
- **`edit_post`** edits the message with the same embed and components.
- **`find_post`** reads the channel's last 50 messages with `GetMessages::new().limit(50)` and returns the first message whose author is `self.bot` and which has a button whose `custom_id` starts with `wiring::signup_tag_prefix(tag.season, tag.night)`. A button's ID lives in `serenity::ButtonKind::NonLink { custom_id, .. }`.
- **`delete_post`** deletes the message, returning `Removal::Gone` for Discord error codes 10008 (Unknown Message) and 10003 (Unknown Channel), matched the way `departed` in `commands.rs` matches 10007, and `Removal::Deleted` otherwise.
- **Every ID crosses into serenity fallibly.** `serenity::ChannelId::new` and `serenity::MessageId::new` panic on zero, and this crate's `ChannelId` and `Snowflake` are plain `u64` with no non-zero invariant, so a zeroed row in the database would panic inside the tick task and stop every future post for every server. Each of the four methods converts through `NonZeroU64` and returns a `BoardError` instead.
- Every other failure is a `BoardError`.

### 9.2 `crates/barnacle-bot/src/discord/commands.rs`

A new top-level `cb` command, `guild_only`, with `default_member_permissions = "MANAGE_GUILD"`, holding a `season` subcommand group with `start`, `show` and `end`. It is added to `all()` with a description, following the existing style. Every reply is ephemeral.

**`/cb season start`**

| Option | Type | Description shown to the user |
|---|---|---|
| `number` | integer, required, min 1 | `Which CB season this is, for example 35` |
| `first_day` | string, required | `First CB day, as 2026-09-16` |
| `last_day` | string, required | `Last CB day, as 2026-11-05` |
| `codename` | string, optional, max length 100 | `The season's codename, for example Komodo Dragon` |

Checks in this order, each replying with the named string and stopping:
1. The channel is `ChannelType::Text`, else `text::RUN_IN_TEXT_CHANNEL`.
2. `wiring::missing_signup_permissions(channel_access(ctx))` is empty, else `text::missing_permissions(&missing)`.
3. Both days parse with `schedule::parse_day`, else `text::DATE_FORMAT`.
4. `Range::new` succeeds, else `text::LAST_DAY_BEFORE_FIRST`.
5. The range has at least one night with `start_unix() > now`, else `text::NO_NIGHTS_LEFT`.
6. `create_season` returns `Created`, else `text::season_number_taken` or `text::season_overlaps`.

On success it replies with `text::season_started`, passing `due_night(now).is_none().then(...)`: the post time when the next post is still in the future, and `None` when it is already due.

**`/cb season show`** replies with one `text::season_line` per season from `seasons_in(guild)` whose `last_moment_unix()` is still ahead, newest last, or `text::NO_SEASON_HERE`. The reply is capped the way the roster is: lines are taken while the total stays inside Discord's 2,000-character message limit, and a last line counts the rest. Nothing stops a manager creating a dozen one-night seasons, and an over-long reply fails the whole command rather than truncating itself.

Descriptions shown in Discord's command picker: `cb` is `Clan Battle sign-ups`, the `season` group is `Clan Battle seasons for this server`, `start` is `Set up a season and post its sign-ups in this channel`, `show` is `List this server's seasons and when each posts next`, and `end` is `Stop posting sign-ups for a season`. `end`'s `number` option is `Which season to stop posting, for example 35`. A description that names the wrong scope is worse than none: `show` lists the whole server, not the channel it was run in.

**`/cb season end`** takes `number` and replies with `text::season_removed`, `text::season_shortened` or `text::season_not_found` from the `EndOutcome`.

### 9.3 `crates/barnacle-bot/src/discord/events.rs`

The component branch gains a sign-up path before the existing cancel path. For a `custom_id` that `wiring::signup_click` accepts, and a component with a `guild_id`:

1. If `click.night.start_unix() <= now`, reply with `CreateInteractionResponse::Message`, ephemeral, `text::signups_closed_at(start)`. Nothing else runs.
2. Otherwise `component.defer(serenity_context).await?`, which sends `Acknowledge`, Discord's deferred message update, exactly as the cancel path does today.
3. Call `data.signups.click(...)` with the interaction's guild, channel, message ID and user.
4. `Recorded` sends nothing more. `Closed` sends `text::signups_closed_at` as an ephemeral followup, `UnknownSeason` sends `text::SEASON_GONE`, and `Failed` sends `text::SOMETHING_WENT_WRONG`, all through the existing `private_followup`.

A `custom_id` that starts with `wiring::SIGNUP_PREFIX` but does not parse is ignored, as unknown IDs are today.

### 9.4 `crates/barnacle-bot/src/discord.rs`

- `Data` gains `pub signups: Arc<Signups<DiscordBoard>>`.
- `run` takes the `Attendance` store alongside `Solves` and builds the board in `setup` from `Arc::clone(&ctx.http)` and `ready.user.id`.
- `setup` spawns the tick task, which poise runs once because its setup closure is `FnOnce`:

```rust
let ticker = Arc::clone(&signups);
tokio::spawn(async move {
    let mut beat = tokio::time::interval(TICK);
    beat.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    loop {
        beat.tick().await;
        let report = ticker.tick(now_unix(), now_ms()).await;
        if report.failures > 0 {
            tracing::warn!(failures = report.failures, "a sign-up step failed and will be retried");
        }
    }
});
```

`TICK` is 60 seconds. `now_unix` is `jiff::Timestamp::now().as_second()` and `now_ms` its millisecond equivalent; both live in `discord.rs`.

This one task drives the whole feature, so its death must not be silent:
- The body runs inside `catch_unwind` over an `AssertUnwindSafe` future, and a panic is logged at error and the loop continues. Without it, one panic anywhere under `tick` leaves a bot that still answers commands and clicks but never posts, closes or deletes another night, with nothing in the log.
- `now_ms` returns an `Option`. A clock reading before 1970 fails the conversion, and the beat is skipped with a warning rather than writing zero into `answered_at_ms`, which is the column the roster orders by.

## 10. The sign-up post

```text
Clan Battles · Season 35: Komodo Dragon
Wednesday, September 23, 2026 5:30 PM · starts in 23 hours
Sign-ups close when the night starts.

Hour 1 · 5:30 PM – 6:30 PM · 6 in · 1 out
Hour 2 · 6:30 PM – 7:30 PM · 7 in · 1 out
Hour 3 · 7:30 PM – 8:30 PM · 5 in · 2 out
Hour 4 · 8:30 PM – 9:30 PM · 3 in · 3 out

`1    2    3    4    `
`in   in   in   -    ` @Aki
`in   in   out  out  ` @Borealis

[ Attend all ]         [ Nope all ]
[ Hour 1: Attending ]  [ Hour 1: Nope ]
[ Hour 2: Attending ]  [ Hour 2: Nope ]
[ Hour 3: Attending ]  [ Hour 3: Nope ]
[ Hour 4: Attending ]  [ Hour 4: Nope ]
```

Every time in it is a Discord timestamp tag, so each viewer sees their own local time. Times are never written into a button label, because nothing confirms Discord renders them there.

Limits this layout respects: five action rows with at most five buttons each; 100 characters for a button ID, against about 55 for the longest here; 4,096 for the description, against about 600 for the header and 46 per roster row, which is why `ROSTER_LIMIT` is 60.

## 11. Errors

| Situation | Behaviour |
|---|---|
| A table is missing at startup | Startup stops with the `sqlite3` command for `migrations/0002_cb_attendance.sql`, as it already does for `guess_solves` |
| A send, edit or delete fails | Counted in `TickReport.failures`, logged at warn by the tick task, and retried on the next tick |
| The channel was deleted | Sends keep failing until the night starts; deletes count as done |
| A command fails | The existing ephemeral `text::SOMETHING_WENT_WRONG` |
| A click arrives for a season that was ended | `text::SEASON_GONE`, ephemeral |

## 12. README changes

- Setup step after the existing one: `sqlite3 data/barnacle.sqlite3 < migrations/0002_cb_attendance.sql`.
- A "Clan Battle sign-ups" section covering: make a channel only the bot can post in; run `/cb season start` in it; the bot needs View Channel, Send Messages, Embed Links and Read Message History there, all of which the current invite link already grants; nights run 23:30-03:30 UTC on Wednesday, Thursday, Saturday and Sunday; each post goes up 24 hours ahead, closes at the start and is deleted 30 minutes after the end; the bot must be running during those windows, and a night whose start passes while it is down gets no post.
- "Player data" gains: the database also stores each player's Discord user ID with their Attending or Nope answer per hour and night, and still stores no names; the `DELETE` command for one player's answers; and that `migrations/0002_cb_attendance.down.sql` removes every season and answer.

## 13. What the owner reviews, not the bot

Nothing in this feature merges, gates or verifies anything. The tick posts, closes and deletes; the buttons record answers. Choosing a lineup from the roster stays with the people reading it.

## 14. Known limits

- **Uptime.** The bot must run at some point between a night's post time and its start for that night to be posted.
- **Mentions may not resolve.** A viewer whose Discord app has not loaded a member sees that roster row as a raw mention. The fallback, if it shows up in practice, is the name lookup `/leaderboard` already uses.
- **`find_post` reads 50 messages.** The sign-up channel is expected to hold nothing else.

## 15. Tests

Each test file is owned by the item that owns its subject (section 16). Names are exact; add more if a behaviour here is untested by them.

`tests/schedule.rs`
- `season_35_has_thirty_nights` covering first and last night.
- `night_moments_match_the_spec` for 2026-11-01: start 1793575800, end 1793590200, post 1793489400, remove 1793592000, hour 3 start 1793583000.
- `only_cb_weekdays_are_nights` for a Monday, Tuesday and Friday.
- `parse_day_is_strict` for `2026-9-16`, `2026/09/16`, `2026-09-31`, `tomorrow` and an empty string.
- `due_night_walks_the_boundaries` for every row of the table in section 5.1.
- `a_season_set_up_late_starts_from_the_next_night`.
- `range_rejects_a_backwards_range` and `overlaps_is_symmetric`.

`tests/attendance_store.rs`, each against a temporary database built from the migration file
- `create_season_rejects_a_duplicate_number`.
- `create_season_rejects_an_overlapping_range` and names the season it overlaps.
- `end_season_removes_a_season_with_no_posts`.
- `end_season_shortens_a_season_that_has_posted`.
- `marks_upsert_and_keep_the_first_answer_time`.
- `roster_is_ordered_by_first_answer`.
- `live_seasons_keeps_a_season_until_its_last_post_is_removed`, which fails if liveness is tested on the range alone.
- `live_seasons_drops_a_season_whose_posts_are_all_removed`.
- `a_missing_table_is_named` and `unexpected_columns_are_reported`.
- `a_season_number_above_the_ceiling_is_named_as_one`.

`tests/attendance.rs`, with a fake board in `tests/common/fakes.rs` and chosen times
- `posts_the_due_night_once_across_repeated_ticks`.
- `a_rebuilt_signups_does_not_post_twice` (the restart case).
- `a_post_found_in_the_channel_is_adopted_not_resent`.
- `a_failed_send_is_retried_next_tick`.
- `a_night_that_started_while_offline_is_never_posted`.
- `closes_at_the_start` and `marks_the_post_closed_only_after_the_edit_succeeds`.
- `removes_thirty_minutes_after_the_end` and `a_gone_message_counts_as_removed`.
- `a_long_gap_removes_and_posts_in_one_tick`.
- `a_click_after_the_start_writes_nothing`.
- `a_click_from_another_guild_is_refused`.
- `attend_all_writes_four_marks`.
- `twenty_clicks_during_one_slow_edit_make_at_most_three_edits`.
- `the_last_edit_shows_the_last_mark`.
- `removes_the_last_night_of_a_season`, which fails if a season stops being live at the moment its last post is due for removal.
- `the_roster_stops_at_the_limit_and_counts_the_rest`, marking more players than `ROSTER_LIMIT` and asserting the row count, the hidden count and the order.

`tests/text.rs`
- `roster_rows_are_aligned_and_mention_the_player`.
- `an_empty_roster_says_so`.
- `a_closed_view_says_closed_and_drops_the_notice`.
- `hidden_players_are_counted`.
- `a_title_without_a_codename_omits_the_colon`.
- `a_full_roster_fits_discords_description_limit`, building `ROSTER_LIMIT` rows with the longest possible user IDs and asserting the rendered description stays inside `EMBED_DESCRIPTION_LIMIT`. Without it, raising `ROSTER_LIMIT` later makes Discord reject every edit while the suite stays green.
- `one_night_reads_as_singular` over both count sentences.
- `a_long_season_list_is_capped_and_counted`.

`tests/wiring.rs`
- `signup_ids_round_trip` for every target and both choices.
- `a_malformed_signup_id_is_rejected`, including `+3` and `003` as season numbers.
- `the_longest_signup_id_fits_discords_limit`.

`tests/startup.rs`
- `a_missing_attendance_table_is_named_with_its_migration`.

Live check, by the owner after merge, in a test server: start a season whose `first_day` is today before 23:30 UTC; the post appears within a minute; clicks update the roster; restarting the bot makes no duplicate; at 23:30 UTC the buttons are disabled; at 04:00 UTC the post is gone.

## 16. Work split

One agent owns each item and edits only its files.

| Item | Files | Depends on |
|---|---|---|
| `cb-core` | `Cargo.toml`, `crates/barnacle-bot/Cargo.toml`, `crates/barnacle-bot/src/lib.rs`, `src/schedule.rs`, `src/attendance.rs`, `src/attendance_store.rs`, `tests/schedule.rs`, `tests/attendance.rs`, `tests/attendance_store.rs`, `tests/common/fakes.rs`, `tests/common/mod.rs`, `migrations/0002_cb_attendance.sql`, `migrations/0002_cb_attendance.down.sql` | nothing |
| `cb-readme` | `README.md` | nothing |
| `cb-strings` | `crates/barnacle-bot/src/text.rs`, `src/wiring.rs`, `tests/text.rs`, `tests/wiring.rs` | `cb-core` |
| `cb-discord` | `crates/barnacle-bot/src/discord.rs`, `src/discord/board.rs`, `src/discord/commands.rs`, `src/discord/events.rs`, `src/startup.rs`, `src/main.rs`, `tests/startup.rs` | `cb-core`, `cb-strings` |

`cb-core` declares `pub mod attendance;`, `pub mod attendance_store;` and `pub mod schedule;` in `lib.rs`, in alphabetical order, and is the only item that edits `lib.rs`.

Each item must leave the workspace compiling, linted and tested green on its own branch. `cb-core` adds three modules nothing calls yet, which is expected: nothing is wired into Discord until `cb-discord`.
