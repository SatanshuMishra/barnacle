# event-scheduler: Generalise CB sign-ups into scheduled events under /event
Status: done
Updated: 2026-11-01T21:06:59.066Z
Next ids: c8 D17 R1 S1

## Goal

Replace the Clan Battle-only season scheduler and the /cb command with one event scheduler and an /event command: each event has its own name, weekdays, start time in its creator's time zone, hour count and ping roles, with Clan Battles as a template of defaults, while existing CB seasons and their posted sign-ups keep working.

## Done when

- [x] c1 /cb is gone and events are created and managed through /event evidence: SatanshuMishra/barnacle#49 merged to main as afa4a57; criteria 7.24 and 7.25 tests pass in the 737-test suite at f381f8a; the local dev bot registered the /event commands in the test server.
- [x] c2 Separate events are separate entities and may overlap in dates and times evidence: criterion 7.10 tests pass at f381f8a, merged in #49.
- [x] c3 An event's start time is entered in its creator's time zone and every viewer sees it in their own zone through Discord timestamps evidence: criteria 7.1, 7.5, 7.6 and 7.36 tests pass at f381f8a, merged in #49.
- [x] c4 The admin chooses each event's length in hours evidence: criteria 7.4, 7.17 and 7.18 tests pass at f381f8a, merged in #49.
- [x] c5 Season numbers no longer identify or title anything; events carry a name evidence: criteria 7.8 and 7.16 tests pass at f381f8a, merged in #49; the migrated local season is titled Clan Battles · Season 99 by name.
- [x] c6 Only the CB command surface changes; voice, guess, updates and announce commands are untouched evidence: criterion 7.32 test passes at f381f8a, merged in #49.
- [x] c7 Existing CB seasons, their answers and their already-posted sign-up buttons keep working after the migration evidence: criteria 7.11, 7.12 and 7.15 tests pass at f381f8a; migrations 0007 and 0008 applied to the local dev database at 0006, its season became an event, and the bot's beat handled its posts; the image workflow's smoke check of 0008 (7.35) was not observed.

## Where things stand

Shipped in #49, merged to main. The run's worktrees, run folders, branches, run home and local spec files are deleted. Follow-ups (main's Cargo.lock pins jiff-tzdb 0.1.8 so main does not build, and the open review advisories) go to a fresh session.

## Next step

None: the thread is done.

## Decisions in force

## Open risks

## Settled, do not reopen

## Where the details live

- barnacle acceptance script (accept.zsh, accept.py, seed.py): /Users/satanshumishra/.mitosis/barnacle/accept.zsh
- review findings ledger shared by barnacle runs: /Users/satanshumishra/.mitosis/barnacle/findings.jsonl
- Old ledger record: thread 01M4YJ7MGKJ7H6XQBXZCFEPR7H in refs/logbook/archive/ledger
