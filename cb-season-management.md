# cb-season-management: CB season management: end now, edit, move, ping role
Status: done
Updated: 2026-11-01T20:52:45.492Z
Next ids: c4 D2 R6 S1

## Goal

Add the season controls the live bot is missing: ending a season immediately and clearing every sign-up post it has up, editing a season's number, dates, codename and ping role in place, moving it to another channel, and pinging a role when a sign-up is posted. Redraw in place for everything cosmetic; re-post only when the channel changes.

## Done when

- [x] c1 /cb season end stops the season immediately and removes every sign-up post it still has in the channel, keeping the recorded answers evidence: cargo test --workspace passes 345 of 345. clearing_all_posts_deletes_every_live_message asserts every live message is deleted and every row marked removed, and asserts a second call is a no-op; end_season_keeps_the_marks asserts the answers survive; an_ended_season_is_not_live_and_not_listed asserts the season drops out of live_seasons, seasons_in and live_season together. Review added the failure half the criterion did not name: end now clears first and refuses without marking the season ended when any delete fails, so a refused delete cannot strand a live sign-up board no command can reach. The owner has run /cb season end against Discord and reported the commands working.
- [x] c2 A season's number, dates, codename and ping role can be edited in place, and the live sign-up post redraws rather than being re-posted evidence: cargo test --workspace passes 345 of 345. edit_season_changes_only_what_it_is_given and edit_season_clears_a_codename_and_a_ping_role cover each editable field including the clear-versus-set distinction, asserting against re-read rows; refresh_posts_edits_and_never_sends asserts an edit reaches the screen through an edit of the existing message and never a send. a_redraw_renders_the_season_as_it_is_now, added at review, pins the case the criterion missed: the redraw combiner could collapse a click's draw into an edit's slot and leave the old codename on screen while reporting success, so a redraw now re-reads the season.
- [x] c3 A sign-up post pings a configured role when it is first posted, and never re-pings on a redraw evidence: cargo test -p barnacle-bot --test wiring passes 11 of 11, including only_a_new_post_may_ping, which covers all four combinations of delivery and role and fails if a redraw ever returns a role. only_the_first_post_of_a_night_pings, added at review, is the stronger statement: the security pass found that clearing a post and letting it re-post made the ping repeatable without limit, so the ping is now derived from whether a post row has ever existed for that night rather than from the delivery alone. A move therefore re-posts silently, which the README states. The owner has not itemised observing the ping fire exactly once in Discord; that half rests on their general report that everything works.

## Where things stand

Converted from the old ledger.

## Next step

Nothing outstanding on this thread. The open risks carry forward on their own: the 0003 rollback is a one-way door once a season number has been reused, and a sub-second race can still leave a moved night's post in the old channel until its removal time.

## Decisions in force

- D1 Ship the season management stack as one PR from the tip branch: Ship fanout/cb2-discord as one PR carrying all four code layers, plus fanout/cb2-readme separately. The stack is already correct: fanout merged each producer into its consumer's worktree, so the tip branch contains store, core, strings and discord as four reviewable commits, with fmt, clippy and 298 tests green. Reshaping the split would have made the store item edit files another agent owned, which is the collision the split exists to prevent. Weakening the gate would have opened PRs that do not build. The lesson for future splits: when a layer changes a shared type, its consumers are part of that layer's blast radius, and only the tip branch can be gated.

## Open risks

- R1 Migration 0003 rebuilds cb_seasons and is not re-runnable; it relies on .bail on plus a schema_migrations row to refuse a second run, and the owner must copy the database first.
- R2 The tick now treats a post row in state removed as not posted so a moved night can be posted again; if a night that was swept on schedule ever became due again, that rule would re-post it.
- R3 Do not edit anything inside a fanout worktree while the run is live: last round an uncommitted .gitignore change there made git refuse the producer merge and silently built one item against the wrong base.
- R4 The 0003 rollback is a one-way door once a season number has been ended and reused: the old schema's unconditional UNIQUE (guild_id, number) cannot hold both rows, so the down migration refuses. It refuses cleanly with data intact, but the README currently describes the refusal case as two ended seasons sharing a number, when the pair can equally be one ended and one live.
- R5 A sub-second race remains between the tick re-reading a season and recording its post: a move landing inside that window can still leave a post in the old channel. The night self-heals at its removal time and re-running the move recovers it. The durable fix is storing each post's own channel on its cb_posts row, recorded in spec section 14.3 and not done.

## Settled, do not reopen

## Where the details live

- Season management design spec: docs/superpowers/specs/2026-09-17-cb-season-management-design.md
- fanout round two inputs: /private/tmp/claude-501/-Users-satanshumishra-Documents-DevLabs-barnacle/0e32eda0-2cc7-401e-9892-fc3b4f0be17b/scratchpad/fanout
- feature branch: feat/cb-season-management
- Old ledger record: thread 01M2RP0CQE2NR9FFKGSVH1PCPW in refs/logbook/archive/ledger
