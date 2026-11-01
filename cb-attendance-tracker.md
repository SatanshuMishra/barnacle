# cb-attendance-tracker: CB attendance tracker: feasibility and architecture
Status: done
Updated: 2026-11-01T20:52:45.492Z
Next ids: c3 D6 R9 S1

## Goal

Decide whether a Clan Battle attendance tracker (per-season schedule, four one-hour slots on Wed/Thu/Sat/Sun 17:30-21:30 America/Edmonton, a sign-up post 24h before each CB night, Discord-localised timestamps) is feasible in barnacle without disproportionate complexity, and propose the simplest architecture that fits the existing Rust bot.

## Done when

- [x] c1 Feasibility verdict delivered in which every mechanism (scheduling, DST-correct times, button interactions, persistence, late-setup 'next CB day' rule) is explained with how it would be implemented, and no part is left unexplained evidence: Feasibility verdict delivered, with each mechanism named against a concrete code path or library call. Scheduling: a 60 s tick calling schedule::due_night, proven with a scratch jiff build that reproduced Season 35's 30 nights and every due-night boundary. Times: a fixed 23:30-03:30 UTC range plus Discord U+003Ct:unix:style> tags; the scratch build runs with jiff default-features=false, features=["std"] and fails loudly on any zone lookup. Buttons: the existing custom_id prefix routing in crates/barnacle-bot/src/discord/events.rs, with serenity's ButtonKind::NonLink custom_id read back for the channel check. Persistence: sqlx on the same SQLite file, migrations/0002 applied by hand like 0001, schema check generalised from solves.rs. Late setup: falls out of the due-night rule, verified at 2026-09-22T12:00Z, 2026-09-23T00:00Z, 23:29Z and 23:30Z. Not observed: nothing was run against Discord itself; button-label timestamp rendering stays unverified, which is why times sit in the embed text.
- [x] c2 Architecture proposal delivered in which every component carries a justification, and anything that cannot be justified is removed evidence: Architecture proposed with a justification per component, accepted by the owner, then built and merged. The owner ruled on every open decision (fixed UTC range, mentions for names, post lifecycle, edit limits) and said go. The four fanout PRs are merged into feat/cb-attendance, and PR #12 opens that branch against main. On the merged branch: cargo fmt --all --check clean, cargo clippy --workspace --all-targets -- -D warnings clean, cargo test --workspace 271 passed across 44 suites 0 failed; both migrations applied to a fresh SQLite database whose tables then refused a mark with no post, an unknown post state and a duplicate season number. Not observed: the bot has never run against Discord, so the post, its buttons, the close and the removal are unverified in the real client.

## Where things stand

The Clan Battle attendance tracker is on main: crates/barnacle-bot/src/schedule.rs, attendance.rs, attendance_store.rs, discord/board.rs and the /cb season commands, with migrations/0002_cb_attendance.sql applied by hand. Verified by fmt, clippy and 271 workspace tests on the merged branch, plus a fresh-database migration check. Unverified against Discord itself.

## Next step

Owner runs the live check from spec section 15 in Discord: apply migrations/0002_cb_attendance.sql to the real data/barnacle.sqlite3, start the bot, run /cb season start in the sign-up channel with a range covering today, and confirm the post appears within a minute, a click updates the roster, a restart makes no duplicate, the buttons grey out at 23:30 UTC and the post is deleted at 04:00 UTC.

## Decisions in force

- D1 CB times are stored as a fixed UTC range, 23:30-03:30: Fixed UTC. The owner decided: "You are NEVER storing my MDT range. You are storing the universal time range as the default." Each CB date D starts at D 23:30 UTC with four one-hour slots ending 03:30 UTC on D+1; the post goes out exactly 24 hours before the start. No time-zone database is needed for the times; Discord timestamps localise them per viewer, so Mountain viewers see 16:30 MST in winter by design.
- D2 Roster shows players as mentions; nothing about names is stored: Mentions. The owner: "Why are we over-complicating this? /profile and /leaderboard already use and display user's names. Just use @user-name or name. Keep it simple." No storage, no lookups, no README privacy change. Known trade-off told to the owner: a viewer whose client has not loaded a member sees that line as a raw mention; the fallback is the existing member_names helper.
- D3 Sign-up post lifecycle: post any time before start, close at start, remove 30 minutes after end: Each night's post moves open -> closed -> removed, stored per post with its message id. The bot posts any time between 24 hours before start and the start; before sending it checks the database and then the channel's last 50 messages for its own post for that night, so a crash between send and record never duplicates or loses a post. Sign-ups close at the night's start (23:30 UTC): the message is redrawn with buttons disabled. The message is deleted at 04:00 UTC (30 minutes after the 03:30 UTC end, 22:00 MDT), or on the first minute the bot is back after that. The owner confirmed close time: "Close at the start of CBs (i.e., 17:30.) ... don't delete the message till ~22:00".
- D4 No design constraint from Discord edit limits; redraws still coalesced: The owner: "There is no limits to edits ... Do NOT take this into-account. However, keep any optimizations to reduce edits overall. However, don't hesitate to edit when needed." The redraw combiner stays, justified by correctness (no stale roster from racing clicks) and fewer edits.
- D5 The build is dispatched by fanout as four MSPs off one feature branch: Four items, each its own MSP, branch and PR into feat/cb-attendance: cb-core (schedule, store, migrations, Signups core, lib.rs, Cargo.toml), cb-readme, cb-strings (text.rs, wiring.rs) after cb-core, cb-discord (board, commands, events, discord.rs, startup, main) after cb-core and cb-strings. cb-core owns lib.rs alone, which is what keeps the other three file-disjoint. The planner confirmed the shape: msps [[cb-core],[cb-readme],[cb-strings],[cb-discord]], cluster_after {2:[0], 3:[0,2]}, tiers top/cheap/top/top, coupling_review empty, nothing underspecified or unpredicted. The third option was rejected because a stub file listed by two items fuses them, and hiding it from the file list would make reconcile report a false cross-MSP collision.

## Open risks

- R1 The bot runs on the owner's machine; if it is off for the whole window between the day-before post time and the night's start, that night gets no sign-up post.
- R2 Worker agents run with --permission-mode bypassPermissions, so each one can edit files and run commands in its worktree without a prompt; the containment is the per-MSP worktree and the file list in its brief, not the permission gate.
- R3 fanout merges a producer's branch into a consumer's worktree, so cb-strings' and cb-discord's PR diffs include their producers' commits until those PRs merge.
- R4 Editing the sign-up post on every roster click re-parses mentions with DEFAULT allowances when allowed_mentions is omitted from the PATCH. Discord docs: on edit, mentions/mention_roles/mention_everyone are 'reconstructed from scratch', and 'if there is no explicit allowed_mentions in the edit request, the content will be parsed with default allowances, that is, without regard to whether or not an allowed_mentions was present in the request that originally created the message.' If the post body carries a role mention, every roster-update edit must repeat allowed_mentions or risk an unintended re-parse.
- R5 Relying on an edit to DELIVER a role ping is unsupported. Discord documents only that the message object's mention arrays are reconstructed on edit; it says nothing about whether a push notification or badge fires on MESSAGE_UPDATE. Community reports conflict (discord-api-docs #1419 and #2474 report unwanted pings on edit; support-forum users describe edits as a silent-ping trick). If the roster post must notify, send a new message rather than edit an existing one.
- R6 A role mention placed in an embed never notifies anyone. Discord scopes allowed_mentions to 'the message content, or the content of components attached to that message' and never includes embeds; discord.js guide states 'Mentions in embeds will not trigger a notification'. If the sign-up post is an embed, the role ping is silently lost. Fix is content, or a Components V2 Text Display (type 10, requires IS_COMPONENTS_V2 flag 1U+003CU+003C15), which officially does ping.
- R7 Posting the sign-up message as an interaction response or via a webhook defaults allowed_mentions to {"parse": ["users"]}, not the permissive regular-message default, so a role mention will NOT ping unless allowed_mentions is passed explicitly. /cb season start is a slash command, so this is the live path.
- R8 If the CB role is not set 'Allow anyone to @mention this role' (mentionable=false), the bot needs the MENTION_EVERYONE permission or the mention renders as a coloured pill with nobody notified. Docs: 'To mention roles and notify their members, the role's mentionable field must be set to true, or the bot must have the MENTION_EVERYONE permission.' Since 2022-08-03, for interaction responses and bot-created webhooks MENTION_EVERYONE is computed including channel overwrites, so a channel-level deny kills the ping.

## Settled, do not reopen

## Where the details live

- CB attendance design spec: docs/superpowers/specs/2026-09-17-cb-attendance-design.md
- Throwaway schedule probe (session scratchpad): /private/tmp/claude-501/-Users-satanshumishra-Documents-DevLabs-barnacle/4036634e-e540-4469-a79a-14c2517526bb/scratchpad/cb-spike
- fanout install (clone): U+007E/.claude/tools/fanout
- fanout run inputs: items, prompt template, PR bridge, plan: /private/tmp/claude-501/-Users-satanshumishra-Documents-DevLabs-barnacle/4036634e-e540-4469-a79a-14c2517526bb/scratchpad/fanout
- feature branch: feat/cb-attendance
- Old ledger record: thread 01M2R6RSSPA72GZHHK2881S5W9 in refs/logbook/archive/ledger
