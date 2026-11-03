# panel-inputs-and-bot-access: Rework /event and /iq panel inputs, IQ judging and add bot access roles
Status: active
Updated: 2026-11-03T00:12:05.011Z
Next ids: c2 D11 R1 S1

## Goal

Act on the owner's live-probe feedback: typed dates and times instead of short menus, full time-zone choice with a user default, any emoji as the IQ judging reaction, offline judge reactions counted only on well-formed posts, a mark when points are awarded, an optional IQ template, event days limited to the chosen range, and a bot setting naming which roles may use /event and /iq (administrators only by default).

## Done when

- [x] c1 The pull requests for this request are open, reviewed green and assigned to the owner evidence: pull requests 56 (access-roles into feat/panel-inputs-and-bot-access), 57 (panel-inputs into mitosis/access-roles) and 58 (iq-judging into mitosis/panel-inputs) are open, ready for review and assigned to SatanshuMishra; all three reviewed green, with access-roles and panel-inputs in round 1 and iq-judging in round 2

## Where things stand

All three MSPs are shipped as stacked pull requests waiting for the owner's merge: 56 access-roles (d1dce61, carrying the lock commit 425f25a), 57 panel-inputs (a69fbc7) and 58 iq-judging (b7f625f). Each stacked pull request must merge into its predecessor's branch before that predecessor merges. Criterion 5.23 (live Discord) is unproven, for the owner's dev-bot check. with_typed_dates counts today in the server's newest-event zone, not the draft's, a defect main already has. CI clippy all-targets fails on 56 and 57 alone, since the later locked test files only compile with iq-judging.

## Next step

Run SHIP=1 ~/.mitosis/barnacle/iq-judging/run.zsh --resume --proven-head iq-judging=b7f625f, open its pull request into mitosis/panel-inputs with mitosis's title and body, assign it, post review/iq-judging.comment.md, then park the thread for the owner's merge.

Needs: D8.

## Decisions in force

- D1 The run is unattended from the owner's instruction: no questions or read-back; gaps become assumptions citing intent lines.
- D2 Lighthouse is skipped; criterion 5.23 (live Discord) stays unproven for the owner's dev-bot check.
- D3 An award the startup rescan counts gets the same judge log line as a live award, with no offline tag (owner).
- D4 Interphase hand-off: build the spec as three stacked MSPs: Build now: cut the FEATURE branch, set up the run home, run --plan-only, then build the three MSPs in order access-roles, panel-inputs, iq-judging.
- D5 Build the three MSPs as three stacked mitosis runs: Three stacked runs in order. The first run's check-author writes all eight locked test files; later runs build from the previous MSP branch, and a fix to an earlier MSP is merged into each MSP stacked on it.
- D7 Hand-off of iq-judging: build the third stacked run from mitosis/panel-inputs: Build iq-judging now from mitosis/panel-inputs; run panel-inputs review round 1 alongside it.
- D8 Ship: push the three stacked MSPs and open their pull requests: Ship all three now: SHIP=1 resume of each run with its proven head, pull requests stacked access-roles into feat/panel-inputs-and-bot-access, panel-inputs into mitosis/access-roles, iq-judging into mitosis/panel-inputs, each assigned to the owner; 5.23 stays for the owner's dev-bot check.
- D9 Ship of panel-inputs: push mitosis/panel-inputs at its proven head: Ship panel-inputs now with --proven-head panel-inputs=a69fbc7, opened into mitosis/access-roles and assigned to the owner.
- D10 Ship of iq-judging: push mitosis/iq-judging at its proven head: Ship iq-judging now with --proven-head iq-judging=b7f625f, opened into mitosis/panel-inputs and assigned to the owner.

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
- Run watcher: /private/tmp/claude-501/-Users-satanshumishra-Documents-DevLabs-barnacle/54bd6046-bf94-46f7-a742-87b3d50c95da/scratchpad/watch_runs.py prints each lane and MSP state change of the three runs; run it under Monitor and re-arm it every 30 minutes
- Review state per run home: review/round-<n>/ holds each round's findings, verdicts and verdict; rounds.md holds one line per round; review/<msp>.record.json is the record cut to that MSP's criteria for pipeline fix-check
