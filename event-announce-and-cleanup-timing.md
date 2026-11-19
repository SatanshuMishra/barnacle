# event-announce-and-cleanup-timing: Event announcement lead time, sign-up clean-up delay, and past dates allowed
Status: active
Updated: 2026-11-19T23:00:15.662Z
Next ids: c6 D1 R1 S1

## Goal

Let an event's creator choose on the notification/ping page when the announcement goes out (1 to 7 days before start, default 1 day) and how long after the end the sign-up stays visible (0 minutes to 4 hours), and allow start dates and announcement times in the past, where a past announcement for a future event goes out immediately.

## Done when

- [ ] c1 The notification/ping page offers an announcement lead time from 1 to 7 days, defaulting to 1 day, and the announcement goes out that long before the start.
- [ ] c2 The notification/ping page offers a sign-up clean-up delay from 0 minutes to 4 hours after the end, and the sign-up disappears that long after the end; locking at start time is unchanged and the next occurrence's announcement timing is unaffected.
- [ ] c3 An event can be created with a start date in the past, and an announcement time already past for a future occurrence sends the announcement immediately; past occurrences get no announcement.
- [ ] c4 The change is on its own branch with an open pull request against main and is not merged.
- [ ] c5 Formatting, clippy with warnings as errors and the full test suite pass on the branch.

## Where things stand

Not started.

## Next step

Run interphase:feature in the standard lane with "Work autonomously." on the three event changes, then pipeline setup and drive start.

## Decisions in force

## Open risks

## Settled, do not reopen

## Where the details live
