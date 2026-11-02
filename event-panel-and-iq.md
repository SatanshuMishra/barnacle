# event-panel-and-iq: Redesign /event create as a panel wizard and add the IQ Competition
Status: active
Updated: 2026-11-02T00:01:28.424Z
Next ids: c3 D4 R3 S1

## Goal

Ship two pull requests in barnacle: /event create moves dates, weekdays, time and pings into a three-step select-menu panel (fixing the "(Weekday)" date-format error and the guessed-weekday NoSessionsLeft refusal), and a new /iq command group runs a World of Warships IQ Competition with criteria, submissions checked for format, judge-reaction points, a leaderboard, templates and a start/stop/complete/cleanup lifecycle.

## Done when

- [ ] c1 The /event pull request is open, reviewed green and assigned to the owner
- [ ] c2 The IQ Competition pull request is open, reviewed green and assigned to the owner

## Where things stand

The spec is handed off. Feature branch feat/event-panel-and-iq is cut from main at a77e6f0 and holds the receipts config (9470ff7, pushed); the seeding cargo build is running. The run home lacks run.zsh and items.json.

## Next step

Watch pipeline drive status for slug event-panel-and-iq until awaiting-merge; then build the IQ run home ~/.mitosis/barnacle/iq-competition (items: the five iq steps with check-author and event-wizard stripped from after; feature.json pointing at the event-panel MSP branch and head), plan it, record its build boundary and drive it.

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
- Run home: ~/.mitosis/barnacle/event-panel-and-iq (receipts answers, charter, dispatch, worker rules)
- Glossary and ADRs: GLOSSARY.md, docs/adr/0001-competitions-are-archived-and-reused-as-templates.md, docs/adr/0002-iq-scoring-trusts-judges.md
