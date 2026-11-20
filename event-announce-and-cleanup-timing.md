# event-announce-and-cleanup-timing: Event announcement lead time, sign-up clean-up delay, and past dates allowed
Status: done
Updated: 2026-11-20T00:49:07.193Z
Next ids: c6 D2 R2 S1

## Goal

Let an event's creator choose on the notification/ping page when the announcement goes out (1 to 7 days before start, default 1 day) and how long after the end the sign-up stays visible (0 minutes to 4 hours), and allow start dates and announcement times in the past, where a past announcement for a future event goes out immediately.

## Done when

- [x] c1 The notification/ping page offers an announcement lead time from 1 to 7 days, defaulting to 1 day, and the announcement goes out that long before the start. evidence: acceptance checks C1.1 to C13.1 pass 41 of 41 on the FEATURE branch and fail 41 of 41 on main.
- [x] c2 The notification/ping page offers a sign-up clean-up delay from 0 minutes to 4 hours after the end, and the sign-up disappears that long after the end; locking at start time is unchanged and the next occurrence's announcement timing is unaffected. evidence: the same 41 acceptance checks.
- [x] c3 An event can be created with a start date in the past, and an announcement time already past for a future occurrence sends the announcement immediately; past occurrences get no announcement. evidence: the same 41 acceptance checks.
- [x] c4 The change is on its own branch with an open pull request against main and is not merged. evidence: pull request 70 from feat/event-announce-and-cleanup-timing to main was opened and left for the owner, who merged it.
- [x] c5 Formatting, clippy with warnings as errors and the full test suite pass on the branch. evidence: pull request 70's CI test job (cargo fmt --check, clippy -D warnings, cargo test --workspace) passes; the local image build and smoke.sh pass every check, migration 0014 included.

## Where things stand

Pull request 70 is merged into main; the drive ran clean-up, deleted the run's branches and worktrees, and archived the run home to ~/.mitosis/barnacle/archive/event-announce-and-cleanup-timing.

## Next step

The work is done and the thread can close.

## Decisions in force

- D1 hand-off: build the post-timing spec unattended through mitosis: Hand the spec to the drive now: pipeline setup, then drive start, ending in a pull request the owner merges.

## Open risks

- R1 accepted: sign-up posts going up and coming down on each event's timing is unproven in a live Discord server; the owner can check it after deploy.

## Settled, do not reopen

## Where the details live

- Spec: docs/specs/event-announce-and-cleanup-timing.md
- Steps: docs/specs/event-announce-and-cleanup-timing.items.json
- Intent and decisions: docs/specs/event-announce-and-cleanup-timing.decisions.md
- Interview record: docs/specs/event-announce-and-cleanup-timing.record.json
