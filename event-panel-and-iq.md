# event-panel-and-iq: Redesign /event create as a panel wizard and add the IQ Competition
Status: active
Updated: 2026-11-01T23:58:45.224Z
Next ids: c3 D4 R2 S1

## Goal

Ship two pull requests in barnacle: /event create moves dates, weekdays, time and pings into a three-step select-menu panel (fixing the "(Weekday)" date-format error and the guessed-weekday NoSessionsLeft refusal), and a new /iq command group runs a World of Warships IQ Competition with criteria, submissions checked for format, judge-reaction points, a leaderboard, templates and a start/stop/complete/cleanup lifecycle.

## Done when

- [ ] c1 The /event pull request is open, reviewed green and assigned to the owner
- [ ] c2 The IQ Competition pull request is open, reviewed green and assigned to the owner

## Where things stand

Interphase (unattended) is in Phase 9: the spec, record and items pass their checks and pipeline guarantees; the glossary and two ADRs are written; the one spec review is running. The run home holds CHARTER.md, dispatch.zsh, worker-rules.txt and receipts-answers.json; run.zsh and items.json are not written yet.

## Next step

Fix the spec reviewer's real issues, rerun render and checks, verify the guard and hand off; then build target/debug in the checkout (seed.py clones it per worktree), cut the feature branch, commit the receipts config, write run.zsh, plan, and start the drive.

## Decisions in force

- D1 The run is unattended from the owner's instruction: no questions; gaps become assumptions citing intent lines.
- D2 Lighthouse is skipped; criterion 5.23 (live Discord) stays unproven.
- D3 Interphase hand-off: build the reviewed spec: Build the handed-off spec now: cut the feature branch, commit the receipts config, plan, and start the unattended drive.

## Open risks

- R1 The checkout has no target/debug, so seed.py fails and every acceptance run is inconclusive until cargo builds it.

## Settled, do not reopen

## Where the details live

- Approved prototype: https://claude.ai/artifact/LkbTmDty3cBnPSCMGUrtFL
- Prototype sources: /private/tmp/claude-501/-Users-satanshumishra-Documents-DevLabs-barnacle/54bd6046-bf94-46f7-a742-87b3d50c95da/scratchpad/proto/project/
- Spec, decisions, record, items: docs/specs/event-panel-and-iq.{md,decisions.md,record.json,items.json}
- Run home: ~/.mitosis/barnacle/event-panel-and-iq (receipts answers, charter, dispatch, worker rules)
- Glossary and ADRs: GLOSSARY.md, docs/adr/0001-competitions-are-archived-and-reused-as-templates.md, docs/adr/0002-iq-scoring-trusts-judges.md
