# event-panel-and-iq: Redesign /event create as a panel wizard and add the IQ Competition
Status: active
Updated: 2026-11-02T00:02:10.230Z
Next ids: c3 D4 R3 S1

## Goal

Ship two pull requests in barnacle: /event create moves dates, weekdays, time and pings into a three-step select-menu panel (fixing the "(Weekday)" date-format error and the guessed-weekday NoSessionsLeft refusal), and a new /iq command group runs a World of Warships IQ Competition with criteria, submissions checked for format, judge-reaction points, a leaderboard, templates and a start/stop/complete/cleanup lifecycle.

## Done when

- [ ] c1 The /event pull request is open, reviewed green and assigned to the owner
- [ ] c2 The IQ Competition pull request is open, reviewed green and assigned to the owner

## Where things stand

The drive for slug event-panel-and-iq is running (plan 2e92988afdef: the check-author locks all seven test files on feat/event-panel-and-iq, then one Lane builds event-wizard as MSP event-panel). Its run home holds only the check-author and event-wizard items. The IQ run home is prepared except feature.json and the EVENT_PANEL_BRANCH placeholder in its run.zsh.

## Next step

Watch pipeline drive status for slug event-panel-and-iq until awaiting-merge; then write the IQ run home's feature.json (feature: the event-panel MSP branch, commit: its head, core main, remote origin), replace EVENT_PANEL_BRANCH in its run.zsh, plan it, record its build boundary and drive slug iq-competition.

Needs: D1, D2, D3.

## Decisions in force

- D1 The run is unattended from the owner's instruction: no questions; gaps become assumptions citing intent lines.
- D2 Lighthouse is skipped; criterion 5.23 (live Discord) stays unproven.
- D3 Interphase hand-off: build the reviewed spec: Build the handed-off spec now: cut the feature branch, commit the receipts config, plan, and start the unattended drive.

## Open risks

- R2 mitosis fuses Steps sharing a file into one MSP, so the IQ Competition must be a second run stacked on the event-panel MSP branch; a fix round on event-panel after the IQ run starts would need merging into the IQ branch. Bears on c2.

## Settled, do not reopen

## Where the details live

- Approved prototype: https://claude.ai/artifact/LkbTmDty3cBnPSCMGUrtFL
- Prototype sources: /private/tmp/claude-501/-Users-satanshumishra-Documents-DevLabs-barnacle/54bd6046-bf94-46f7-a742-87b3d50c95da/scratchpad/proto/project/
- Spec, decisions, record, items: docs/specs/event-panel-and-iq.{md,decisions.md,record.json,items.json}
- Run homes: ~/.mitosis/barnacle/event-panel-and-iq (the /event run) and ~/.mitosis/barnacle/iq-competition (the stacked IQ run)
