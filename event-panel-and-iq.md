# event-panel-and-iq: Redesign /event create as a panel wizard and add the IQ Competition
Status: active
Updated: 2026-11-02T02:06:33.167Z
Next ids: c3 D6 R4 S1

## Goal

Ship two pull requests in barnacle: /event create moves dates, weekdays, time and pings into a three-step select-menu panel (fixing the "(Weekday)" date-format error and the guessed-weekday NoSessionsLeft refusal), and a new /iq command group runs a World of Warships IQ Competition with criteria, submissions checked for format, judge-reaction points, a leaderboard, templates and a start/stop/complete/cleanup lifecycle.

## Done when

- [ ] c1 The /event pull request is open, reviewed green and assigned to the owner
- [ ] c2 The IQ Competition pull request is open, reviewed green and assigned to the owner

## Where things stand

The event-panel-and-iq drive is stopped at ship (no ship boundary recorded). MSP event-panel is at 8894afe on mitosis/event-panel (local, unpushed): round 2's confirmed C1 (typed-dates window zone mismatch) is fixed there with receipt 6d6dba9, fix-check clean. Review round 3: code and security reviewers found nothing, the suite's U1 is dropped as the base-shared IQ compile failure, release-check passes, and a validator is judging carried obligation P1 (C1) before pipeline record. Slug iq-competition is building MSP iq-competition stacked on mitosis/event-panel at aa274cf; lanes iq-rules, iq-text and iq-store are done, iq-setup and iq-discord remain. The check-author's API readings (back() returns the draft, every IqStore call returns Result, SetupDraft accessors) bind the IQ steps.

## Next step

Run SHIP=1 ~/.mitosis/barnacle/event-panel-and-iq/run.zsh --resume --proven-head event-panel=8894afe, open the /event pull request with mitosis's title and body, assign it to the owner and post review/event-panel.comment.md; then merge mitosis/event-panel into mitosis/iq-competition before the IQ review.

Needs: D5, D1, R2.

## Decisions in force

- D1 The run is unattended from the owner's instruction: no questions; gaps become assumptions citing intent lines.
- D2 Lighthouse is skipped; criterion 5.23 (live Discord) stays unproven.
- D3 Interphase hand-off: build the reviewed spec: Build the handed-off spec now: cut the feature branch, commit the receipts config, plan, and start the unattended drive.
- D4 IQ run hand-off: start the stacked IQ build now: Start the IQ build now; merge any event-panel fix commits into the IQ branch before its review and ship.
- D5 event-panel ship: open the /event pull request from the green head: Ship event-panel now: SHIP=1 run.zsh --resume --proven-head event-panel=8894afe, then open its pull request assigned to the owner and post the review comment.

## Open risks

- R2 mitosis fuses Steps sharing a file into one MSP, so the IQ Competition must be a second run stacked on the event-panel MSP branch; a fix round on event-panel after the IQ run starts would need merging into the IQ branch. Bears on c2.
- R3 pipeline 0.12.0 bugs worked around by hand: gate-findings refuses mitosis 0.10.1's null gate reasons, and pipeline suite cannot fingerprint Rust compile errors, so a failure the base shares reads as new (event-panel's U1 was dropped on per-target evidence in review/round-1/event-panel.suite-base-comparison.txt); and a drive restarted after a review-step halt loses the reviewers' findings. All belong in the final report as plugin bugs.

## Settled, do not reopen

## Where the details live

- Approved prototype: https://claude.ai/artifact/LkbTmDty3cBnPSCMGUrtFL
- Prototype sources: /private/tmp/claude-501/-Users-satanshumishra-Documents-DevLabs-barnacle/54bd6046-bf94-46f7-a742-87b3d50c95da/scratchpad/proto/project/
- Spec, decisions, record, items: docs/specs/event-panel-and-iq.{md,decisions.md,record.json,items.json}
- Run homes: ~/.mitosis/barnacle/event-panel-and-iq (the /event run) and ~/.mitosis/barnacle/iq-competition (the stacked IQ run)
- Build state: ~/.mitosis/runs/barnacle/<slug>/state.json; review rounds under each run home's review/
- Provable record copy for pipeline fix-check (criterion 5.23 left out, it has no acceptance line): ~/.mitosis/barnacle/event-panel-and-iq/review/record.provable.json
