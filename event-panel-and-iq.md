# event-panel-and-iq: Redesign /event create as a panel wizard and add the IQ Competition
Status: active
Updated: 2026-11-02T10:47:43.675Z
Next ids: c3 D6 R5 S1

## Goal

Ship two pull requests in barnacle: /event create moves dates, weekdays, time and pings into a three-step select-menu panel (fixing the "(Weekday)" date-format error and the guessed-weekday NoSessionsLeft refusal), and a new /iq command group runs a World of Warships IQ Competition with criteria, submissions checked for format, judge-reaction points, a leaderboard, templates and a start/stop/complete/cleanup lifecycle.

## Done when

- [x] c1 The /event pull request is open, reviewed green and assigned to the owner evidence: https://github.com/SatanshuMishra/barnacle/pull/53 (mitosis/event-panel at 8894afe into feat/event-panel-and-iq; round 3 verdict green; 25/25 acceptance checks pass at 8894afe; assigned to SatanshuMishra; review comment posted)
- [ ] c2 The IQ Competition pull request is open, reviewed green and assigned to the owner

## Where things stand

mitosis/iq-competition is at 29f9d49, which merges feat/event-panel-and-iq (eef145a, holding #53) into the round-1 fixes. MSP iq-competition is built and committed but held back gate-failed: 21 of 55 properties judged weak (R4). IQ review runs by hand, because the drive is stopped; round 1 (verdict diagnose) is fixed at ef403f2: 832 tests pass, clippy and fmt clean, pipeline fix-check passes, and both diagnoses' selecting checks print nothing. S4 and G1-G21 were rejected. The new limit tests kill every sampled mutant except iq_store.rs:22 BUSY_TIMEOUT ±1, which no test can observe.

## Next step

Review 6b96b2d..29f9d49 as round 2 (code-reviewer and security-reviewer on the fix diff, with the round-1 diagnoses); drive it to green; write --gate-classes covered-elsewhere for the 21 weak properties, naming BUSY_TIMEOUT as unobservable, for the ship run; open the pull request against feat/event-panel-and-iq.

Needs: D1, D4, R3, R4.

## Decisions in force

- D1 The run is unattended from the owner's instruction: no questions; gaps become assumptions citing intent lines.
- D2 Lighthouse is skipped; criterion 5.23 (live Discord) stays unproven.
- D3 Interphase hand-off: build the reviewed spec: Build the handed-off spec now: cut the feature branch, commit the receipts config, plan, and start the unattended drive.
- D4 IQ run hand-off: start the stacked IQ build now: Start the IQ build now; merge any event-panel fix commits into the IQ branch before its review and ship.

## Open risks

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
- IQ mutant evidence: the 24 sampled mutants and the full suite's result on each, in ~/.mitosis/barnacle/iq-competition/review/round-1/iq-competition.gate-mutants-{sample,suite}.json
