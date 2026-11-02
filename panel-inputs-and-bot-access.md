# panel-inputs-and-bot-access: Rework /event and /iq panel inputs, IQ judging and add bot access roles
Status: active
Updated: 2026-11-02T21:55:43.304Z
Next ids: c2 D8 R1 S1

## Goal

Act on the owner's live-probe feedback: typed dates and times instead of short menus, full time-zone choice with a user default, any emoji as the IQ judging reaction, offline judge reactions counted only on well-formed posts, a mark when points are awarded, an optional IQ template, event days limited to the chosen range, and a bot setting naming which roles may use /event and /iq (administrators only by default).

## Done when

- [ ] c1 The pull requests for this request are open, reviewed green and assigned to the owner

## Where things stand

access-roles is reviewed green in round 1 (base 425f25a, the lock commit on feat/panel-inputs-and-bot-access) with its fixes at d1dce61 on mitosis/access-roles; fix-check is clean against review/access-roles.record.json (criteria 5.1-5.4 only, since fix-check refuses 5.23, which has no test rows). panel-inputs is gated (30 of 30 pass, reconcile clean, gated commit d2a626a) and at 1db7c8f on mitosis/panel-inputs, holding the merged access-roles fixes and time_zone added to the three tests that pinned the old option lists; its review round 1 (base d1dce61, criteria 5.5-5.14, 26 files) has its scoped suite passing (893 passed, 0 failed; the full run fails only on iq-judging's locked files, as the base does) and its three reviewers running, with one standards candidate K1 (the 0011 down migration's DROP TABLE). iq-judging is planned clean (lanes iq-marks-store and iq-judging-discord) and building from mitosis/panel-inputs.

## Next step

Plan and build iq-judging with ~/.mitosis/barnacle/iq-judging/run.zsh (--plan-only first); meanwhile finish panel-inputs review round 1 (suite, reviewers, validator, record, verdict, fixes) and merge its fixes into mitosis/iq-judging.

Needs: D5, D7.

## Decisions in force

- D1 The run is unattended from the owner's instruction: no questions or read-back; gaps become assumptions citing intent lines.
- D2 Lighthouse is skipped; criterion 5.23 (live Discord) stays unproven for the owner's dev-bot check.
- D3 An award the startup rescan counts gets the same judge log line as a live award, with no offline tag (owner).
- D4 Interphase hand-off: build the spec as three stacked MSPs: Build now: cut the FEATURE branch, set up the run home, run --plan-only, then build the three MSPs in order access-roles, panel-inputs, iq-judging.
- D5 Build the three MSPs as three stacked mitosis runs: Three stacked runs in order. The first run's check-author writes all eight locked test files; later runs build from the previous MSP branch, and a fix to an earlier MSP is merged into each MSP stacked on it.
- D7 Hand-off of iq-judging: build the third stacked run from mitosis/panel-inputs: Build iq-judging now from mitosis/panel-inputs; run panel-inputs review round 1 alongside it.

## Open risks

## Settled, do not reopen

## Where the details live

- Spec workspace: docs/specs/panel-inputs-and-bot-access.{md,decisions.md,record.json,items.json}
- Build state: ~/.mitosis/runs/barnacle/<slug>/state.json, with lane logs under lanes/ and work in progress under clones/
- Record builder scripts (rerunning build_record.py wipes the probes): /private/tmp/claude-501/-Users-satanshumishra-Documents-DevLabs-barnacle/54bd6046-bf94-46f7-a742-87b3d50c95da/scratchpad/{build_record.py,add_tasks.py}
- Security readings for the final report, not the PR: /announce keeps owner-only plus the organiser check; panel final buttons re-check access.
- Run homes: ~/.mitosis/barnacle/{panel-inputs-and-bot-access,panel-inputs,iq-judging}, each with CHARTER.md, items.json, run.zsh, dispatch.zsh, worker-rules.txt and feature.json; items.all.json in the first holds all seven Steps
- Review helpers: /private/tmp/claude-501/-Users-satanshumishra-Documents-DevLabs-barnacle/54bd6046-bf94-46f7-a742-87b3d50c95da/scratchpad/render_briefs.py renders a round's briefs; ~/.mitosis/runs/barnacle/panel-inputs-and-bot-access/base is the base worktree for pipeline suite
- Security readings for the final report also include /barnacle's server-side required_permissions ADMINISTRATOR (lane 1 assumption).
- Owner feedback items a-h and the access-role request are in the session transcript of 2026-11-02; the live-probe log is /private/tmp/claude-501/-Users-satanshumishra-Documents-DevLabs-barnacle/54bd6046-bf94-46f7-a742-87b3d50c95da/scratchpad/live-probe/bot.log
- Review state per run home: review/round-<n>/ holds each round's findings, verdicts and verdict; rounds.md holds one line per round; review/<msp>.record.json is the record cut to that MSP's criteria for pipeline fix-check
