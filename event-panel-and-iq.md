# event-panel-and-iq: Redesign /event create as a panel wizard and add the IQ Competition
Status: active
Updated: 2026-11-01T23:47:21.120Z
Next ids: c3 D3 R1 S1

## Goal

Ship two pull requests in barnacle: /event create moves dates, weekdays, time and pings into a three-step select-menu panel (fixing the "(Weekday)" date-format error and the guessed-weekday NoSessionsLeft refusal), and a new /iq command group runs a World of Warships IQ Competition with criteria, submissions checked for format, judge-reaction points, a leaderboard, templates and a start/stop/complete/cleanup lifecycle.

## Done when

- [ ] c1 The /event pull request is open, reviewed green and assigned to the owner
- [ ] c2 The IQ Competition pull request is open, reviewed green and assigned to the owner

## Where things stand

Interphase (unattended) is in Phase 9: the spec, record and items are written and pass their checks; the one spec review is running.

## Next step

Fix the spec reviewer's real issues, rerun render and checks, then Phase 10 hand-off (glossary, ADRs, guard verify), then pipeline guarantees and the feature branch.

## Decisions in force

- D1 The run is unattended from the owner's instruction: no questions; gaps become assumptions citing intent lines.
- D2 Lighthouse is skipped; criterion 5.23 (live Discord) stays unproven.

## Open risks

## Settled, do not reopen

## Where the details live

- Approved prototype: https://claude.ai/artifact/LkbTmDty3cBnPSCMGUrtFL
- Prototype sources: /private/tmp/claude-501/-Users-satanshumishra-Documents-DevLabs-barnacle/54bd6046-bf94-46f7-a742-87b3d50c95da/scratchpad/proto/project/
- Spec, decisions, record, items: docs/specs/event-panel-and-iq.{md,decisions.md,record.json,items.json}
- Drafted receipts answers: /var/folders/tz/b798spt96sjfyy924nf0cw4c0000gn/T/tmp.eIm8UoQZ5S/answers.json
