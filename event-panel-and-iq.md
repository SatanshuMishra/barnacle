# event-panel-and-iq: Redesign /event create as a panel wizard and add the IQ Competition
Status: active
Updated: 2026-11-01T23:58:59.862Z
Next ids: c3 D4 R2 S1

## Goal

Ship two pull requests in barnacle: /event create moves dates, weekdays, time and pings into a three-step select-menu panel (fixing the "(Weekday)" date-format error and the guessed-weekday NoSessionsLeft refusal), and a new /iq command group runs a World of Warships IQ Competition with criteria, submissions checked for format, judge-reaction points, a leaderboard, templates and a start/stop/complete/cleanup lifecycle.

## Done when

- [ ] c1 The /event pull request is open, reviewed green and assigned to the owner
- [ ] c2 The IQ Competition pull request is open, reviewed green and assigned to the owner

## Where things stand

The spec is handed off. Feature branch feat/event-panel-and-iq is cut from main at a77e6f0; the seeding cargo build is running. The run home lacks run.zsh and items.json.

## Next step

Build target/debug in the checkout, cut the feature branch with pipeline feature-branch, commit receipts.config.json on it, write run.zsh and items.json in the run home, run --plan-only, then pipeline drive start.

Needs: D1, D2, R1.

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
