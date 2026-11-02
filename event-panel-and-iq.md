# event-panel-and-iq: Redesign /event create as a panel wizard and add the IQ Competition
Status: active
Updated: 2026-11-02T01:29:58.397Z
Next ids: c3 D5 R3 S1

## Goal

Ship two pull requests in barnacle: /event create moves dates, weekdays, time and pings into a three-step select-menu panel (fixing the "(Weekday)" date-format error and the guessed-weekday NoSessionsLeft refusal), and a new /iq command group runs a World of Warships IQ Competition with criteria, submissions checked for format, judge-reaction points, a leaderboard, templates and a start/stop/complete/cleanup lifecycle.

## Done when

- [ ] c1 The /event pull request is open, reviewed green and assigned to the owner
- [ ] c2 The IQ Competition pull request is open, reviewed green and assigned to the owner

## Where things stand

Two drives run. Slug event-panel-and-iq is reviewing MSP event-panel on mitosis/event-panel (gate 25 of 25 pass at 276bf27; glossary and ADRs committed at aa274cf). Slug iq-competition is building MSP iq-competition stacked on mitosis/event-panel at aa274cf (5 serial Lanes). The check-author's API readings (back() returns the draft, every IqStore call returns Result, SetupDraft accessors) bind the IQ steps.

## Next step

Watch both drives (slugs event-panel-and-iq and iq-competition); when event-panel's review adds fix commits, merge mitosis/event-panel into the IQ branch before the IQ review; at awaiting-merge for both, report the stacked pull requests.

Needs: D1, D2, R2.

## Decisions in force

- D1 The run is unattended from the owner's instruction: no questions; gaps become assumptions citing intent lines.
- D2 Lighthouse is skipped; criterion 5.23 (live Discord) stays unproven.
- D3 Interphase hand-off: build the reviewed spec: Build the handed-off spec now: cut the feature branch, commit the receipts config, plan, and start the unattended drive.
- D4 IQ run hand-off: start the stacked IQ build now: Start the IQ build now; merge any event-panel fix commits into the IQ branch before its review and ship.

## Open risks

- R2 mitosis fuses Steps sharing a file into one MSP, so the IQ Competition must be a second run stacked on the event-panel MSP branch; a fix round on event-panel after the IQ run starts would need merging into the IQ branch. Bears on c2.

## Settled, do not reopen

## Where the details live

- Approved prototype: https://claude.ai/artifact/LkbTmDty3cBnPSCMGUrtFL
- Prototype sources: /private/tmp/claude-501/-Users-satanshumishra-Documents-DevLabs-barnacle/54bd6046-bf94-46f7-a742-87b3d50c95da/scratchpad/proto/project/
- Spec, decisions, record, items: docs/specs/event-panel-and-iq.{md,decisions.md,record.json,items.json}
- Run homes: ~/.mitosis/barnacle/event-panel-and-iq (the /event run) and ~/.mitosis/barnacle/iq-competition (the stacked IQ run)
