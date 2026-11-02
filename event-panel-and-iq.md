# event-panel-and-iq: Redesign /event create as a panel wizard and add the IQ Competition
Status: active
Updated: 2026-11-02T10:16:50.589Z
Next ids: c3 D6 R5 S1

## Goal

Ship two pull requests in barnacle: /event create moves dates, weekdays, time and pings into a three-step select-menu panel (fixing the "(Weekday)" date-format error and the guessed-weekday NoSessionsLeft refusal), and a new /iq command group runs a World of Warships IQ Competition with criteria, submissions checked for format, judge-reaction points, a leaderboard, templates and a start/stop/complete/cleanup lifecycle.

## Done when

- [x] c1 The /event pull request is open, reviewed green and assigned to the owner evidence: https://github.com/SatanshuMishra/barnacle/pull/53 (mitosis/event-panel at 8894afe into feat/event-panel-and-iq; round 3 verdict green; 25/25 acceptance checks pass at 8894afe; assigned to SatanshuMishra; review comment posted)
- [ ] c2 The IQ Competition pull request is open, reviewed green and assigned to the owner

## Where things stand

The owner merged #53 into feat/event-panel-and-iq (eef145a), which holds the C1 fix that mitosis/iq-competition (6b96b2d, cut at aa274cf) lacks. MSP iq-competition is built and committed but held back gate-failed: 21 of 55 properties judged weak (R4). IQ review round 1 runs by hand (the drive is stopped): diff, obligations, standards (S1 LOW), gate findings (G1-G21 LOW) and the three reviewer briefs are written under review/round-1; the suite against the base checkout bases/iq-competition and a full-suite run over the 24 sampled mutants run in the background.

## Next step

When the round-1 suite ends, dispatch code-reviewer, conformance-auditor and security-reviewer on review/round-1, then candidates, validator, record and verdict; judge G1-G21 with the mutant suite evidence and, where every survivor is killed elsewhere, write --gate-classes covered-elsewhere for the ship run. Then merge origin/feat/event-panel-and-iq into mitosis/iq-competition, review it as round 2, drive to green and open the pull request against feat/event-panel-and-iq.

Needs: D1, D4, R2, R3, R4.

## Decisions in force

- D1 The run is unattended from the owner's instruction: no questions; gaps become assumptions citing intent lines.
- D2 Lighthouse is skipped; criterion 5.23 (live Discord) stays unproven.
- D3 Interphase hand-off: build the reviewed spec: Build the handed-off spec now: cut the feature branch, commit the receipts config, plan, and start the unattended drive.
- D4 IQ run hand-off: start the stacked IQ build now: Start the IQ build now; merge any event-panel fix commits into the IQ branch before its review and ship.

## Open risks

- R2 mitosis fuses Steps sharing a file into one MSP, so the IQ Competition must be a second run stacked on the event-panel MSP branch; a fix round on event-panel after the IQ run starts would need merging into the IQ branch. Bears on c2.
- R3 pipeline 0.12.0 bugs worked around by hand: gate-findings refuses mitosis 0.10.1's null gate reasons, and pipeline suite cannot fingerprint Rust compile errors, so a failure the base shares reads as new (event-panel's U1 was dropped on per-target evidence in review/round-1/event-panel.suite-base-comparison.txt); and a drive restarted after a review-step halt loses the reviewers' findings; the drive's 14400-second run limit is shorter than mitosis's mutant gate on 55 Rust tests, and mitosis keeps no partial gate, so a killed gate restarts from zero; pipeline fix-check refuses a record with an unproven criterion that has no acceptance line. All belong in the final report as plugin bugs.
- R4 mitosis 0.10.1 samples Rust mutants untraced, taking the first 12 added lines in file order (top-of-file constants, an SQLite busy timeout, test-file generics), so 21 IQ properties read weak without a mutant in the code they exercise; the held-back MSP ships only with --gate-classes, and BUSY_TIMEOUT ±1 is a mutant no test can observe. Bears on c2; also a plugin bug for the final report.

## Settled, do not reopen

## Where the details live

- Approved prototype: https://claude.ai/artifact/LkbTmDty3cBnPSCMGUrtFL
- Prototype sources: /private/tmp/claude-501/-Users-satanshumishra-Documents-DevLabs-barnacle/54bd6046-bf94-46f7-a742-87b3d50c95da/scratchpad/proto/project/
- Spec, decisions, record, items: docs/specs/event-panel-and-iq.{md,decisions.md,record.json,items.json}
- Run homes: ~/.mitosis/barnacle/event-panel-and-iq (the /event run) and ~/.mitosis/barnacle/iq-competition (the stacked IQ run)
- Build state: ~/.mitosis/runs/barnacle/<slug>/state.json; review rounds under each run home's review/
- Provable record copy for pipeline fix-check (criterion 5.23 left out, it has no acceptance line): ~/.mitosis/barnacle/event-panel-and-iq/review/record.provable.json
- IQ mutant evidence: the 24 sampled mutants and the full suite's result on each, in the session scratchpad's iq_mutants.json and suite_mutants.json
