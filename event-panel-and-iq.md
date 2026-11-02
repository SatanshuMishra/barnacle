# event-panel-and-iq: Redesign /event create as a panel wizard and add the IQ Competition
Status: parked
Updated: 2026-11-02T16:27:17.561Z
Next ids: c3 D7 R6 S2

## Goal

Ship two pull requests in barnacle: /event create moves dates, weekdays, time and pings into a three-step select-menu panel (fixing the "(Weekday)" date-format error and the guessed-weekday NoSessionsLeft refusal), and a new /iq command group runs a World of Warships IQ Competition with criteria, submissions checked for format, judge-reaction points, a leaderboard, templates and a start/stop/complete/cleanup lifecycle.

## Done when

- [x] c1 The /event pull request is open, reviewed green and assigned to the owner evidence: https://github.com/SatanshuMishra/barnacle/pull/53 (mitosis/event-panel at 8894afe into feat/event-panel-and-iq; round 3 verdict green; 25/25 acceptance checks pass at 8894afe; assigned to SatanshuMishra; review comment posted)
- [x] c2 The IQ Competition pull request is open, reviewed green and assigned to the owner evidence: https://github.com/SatanshuMishra/barnacle/pull/54 (mitosis/iq-competition at 0036a69 into feat/event-panel-and-iq; round 4 verdict green with advisories fixed; regate passes 55/55 at 0036a69; release-check passes; assigned to SatanshuMishra; review comment posted)

## Where things stand

Integration is done: the FEATURE pull request https://github.com/SatanshuMishra/barnacle/pull/55 (feat/event-panel-and-iq at b3655be into main) is open, ready for review and assigned to the owner. At b3655be 80 of 80 acceptance checks and 841 suite tests pass; integration-plan chose lighthouse skip and integration-ready passes. Clean-up waits on the owner saying #55 merged.

## Next step

The owner starts Integration by saying #54 (the IQ pull request) merged into feat/event-panel-and-iq; then resume this thread, sync, run lighthouse on feat/event-panel-and-iq (or skip it per D2) and open the FEATURE pull request into main.

Needs: D1, D2, R3, R4.

## Decisions in force

- D1 The run is unattended from the owner's instruction: no questions; gaps become assumptions citing intent lines.
- D2 Lighthouse is skipped; criterion 5.23 (live Discord) stays unproven.

## Open risks

- R5 Live Discord behaviour (criterion 5.23) is unproven in #55: rendering, reaction delivery, channel and role creation and permission overwrites need a dev-bot run against a real server before or after merge.

## Settled, do not reopen

- S1 The IQ pull request targets feat/event-panel-and-iq, since mitosis/event-panel already merged there through #53 (D6).

## Where the details live

- Approved prototype: https://claude.ai/artifact/LkbTmDty3cBnPSCMGUrtFL
- Prototype sources: /private/tmp/claude-501/-Users-satanshumishra-Documents-DevLabs-barnacle/54bd6046-bf94-46f7-a742-87b3d50c95da/scratchpad/proto/project/
- Spec, decisions, record, items: docs/specs/event-panel-and-iq.{md,decisions.md,record.json,items.json}
- Run homes: ~/.mitosis/barnacle/event-panel-and-iq (the /event run) and ~/.mitosis/barnacle/iq-competition (the stacked IQ run)
- Build state: ~/.mitosis/runs/barnacle/<slug>/state.json; review rounds under each run home's review/
- Provable record copy for pipeline fix-check (criterion 5.23 left out, it has no acceptance line): ~/.mitosis/barnacle/event-panel-and-iq/review/record.provable.json
- IQ review rounds, one line each with verdict and fix commits: ~/.mitosis/barnacle/iq-competition/rounds.md
- IQ mutant evidence: the 24 sampled mutants and the full suite's result on each, in ~/.mitosis/barnacle/iq-competition/review/round-1/iq-competition.gate-mutants-{sample,suite}.json
- IQ ship result, PR title and body as mitosis wrote them: ~/.mitosis/barnacle/iq-competition/review/ship.out and iq-competition.pr-{title.txt,body.md}
- FEATURE pull request fields as rendered: /private/tmp/claude-501/-Users-satanshumishra-Documents-DevLabs-barnacle/54bd6046-bf94-46f7-a742-87b3d50c95da/scratchpad/feature-pr.json
