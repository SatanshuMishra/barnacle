# event-panel-and-iq: Redesign /event create as a panel wizard and add the IQ Competition
Status: active
Updated: 2026-11-02T13:15:11.392Z
Next ids: c3 D7 R5 S2

## Goal

Ship two pull requests in barnacle: /event create moves dates, weekdays, time and pings into a three-step select-menu panel (fixing the "(Weekday)" date-format error and the guessed-weekday NoSessionsLeft refusal), and a new /iq command group runs a World of Warships IQ Competition with criteria, submissions checked for format, judge-reaction points, a leaderboard, templates and a start/stop/complete/cleanup lifecycle.

## Done when

- [x] c1 The /event pull request is open, reviewed green and assigned to the owner evidence: https://github.com/SatanshuMishra/barnacle/pull/53 (mitosis/event-panel at 8894afe into feat/event-panel-and-iq; round 3 verdict green; 25/25 acceptance checks pass at 8894afe; assigned to SatanshuMishra; review comment posted)
- [x] c2 The IQ Competition pull request is open, reviewed green and assigned to the owner evidence: https://github.com/SatanshuMishra/barnacle/pull/54 (mitosis/iq-competition at 0036a69 into feat/event-panel-and-iq; round 4 verdict green with advisories fixed; regate passes 55/55 at 0036a69; release-check passes; assigned to SatanshuMishra; review comment posted)

## Where things stand

Both pull requests are open and assigned: #53 merged into feat/event-panel-and-iq, and #54 (IQ) awaits the owner's review and merge. Integration waits on the owner saying #54 merged.

## Next step

{"kind": "action", "text": "Run `SHIP=1 ~/.mitosis/barnacle/iq-competition/run.zsh --resume --proven-head iq-competition=0036a6955b5cc53a3ea4a2ec89e58fe0da9b3a8e --gate-classes ~/.mitosis/barnacle/iq-competition/gate-classes.json`, then open the IQ pull request against feat/event-panel-and-iq with mitosis's title and body, assign it to the owner and post review/iq-competition.comment.md."}

Needs: D6, D1, R4.

## Decisions in force

- D1 The run is unattended from the owner's instruction: no questions; gaps become assumptions citing intent lines.
- D2 Lighthouse is skipped; criterion 5.23 (live Discord) stays unproven.

## Open risks

- R3 pipeline 0.12.0 bugs worked around by hand: gate-findings refuses mitosis 0.10.1's null gate reasons; pipeline suite cannot fingerprint Rust compile errors; a restarted drive loses the reviewers' findings; the drive's 14400-second limit is shorter than the mutant gate and no partial gate is kept; fix-check refuses an unproven criterion with no acceptance line. Integration's lighthouse step may meet them again.
- R4 mitosis 0.10.1 samples Rust mutants untraced, so 21 IQ properties needed covered-elsewhere gate classes, and #54's Testing table reads them as not proven; run.zsh still names mitosis/event-panel as the feature branch, so mitosis reports that stale base.

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
