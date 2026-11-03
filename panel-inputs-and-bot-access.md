# panel-inputs-and-bot-access: Rework /event and /iq panel inputs, IQ judging and add bot access roles
Status: active
Updated: 2026-11-03T00:11:01.930Z
Next ids: c2 D10 R1 S1

## Goal

Act on the owner's live-probe feedback: typed dates and times instead of short menus, full time-zone choice with a user default, any emoji as the IQ judging reaction, offline judge reactions counted only on well-formed posts, a mark when points are awarded, an optional IQ template, event days limited to the chosen range, and a bot setting naming which roles may use /event and /iq (administrators only by default).

## Done when

- [ ] c1 The pull requests for this request are open, reviewed green and assigned to the owner

## Where things stand

access-roles is reviewed green at d1dce61 on mitosis/access-roles (base 425f25a on feat/panel-inputs-and-bot-access), and panel-inputs green at a69fbc7 on mitosis/panel-inputs (base d1dce61); their fix-checks run against records cut to their own criteria, since fix-check refuses 5.23, which has no test rows. The panel-inputs C1 rejection (with_typed_dates counts today in the server's newest-event zone, not the draft's) goes in the final report as a defect main already has. iq-judging (base ad151bc, criteria 5.15-5.23) is reviewed green in round 2 at b7f625f on mitosis/iq-judging, its round 2 advisory (the /iq points reply waiting behind the judge-log line) fixed and fix-check clean; its mitosis reconcile lists discord/commands.rs and tests/iq.rs as declared but unwritten, both harmless. The ship of access-roles has run; panel-inputs and iq-judging follow.

## Next step

Run SHIP=1 <home>/run.zsh --resume --proven-head <msp>=<head> for access-roles (d1dce61), panel-inputs (a69fbc7) and iq-judging (b7f625f) in that order, open each pull request through GitHub MCP with mitosis's title and body, assign each to SatanshuMishra, then park the thread for the owner's merge.

Needs: D5, D8.

## Decisions in force

- D1 The run is unattended from the owner's instruction: no questions or read-back; gaps become assumptions citing intent lines.
- D2 Lighthouse is skipped; criterion 5.23 (live Discord) stays unproven for the owner's dev-bot check.
- D3 An award the startup rescan counts gets the same judge log line as a live award, with no offline tag (owner).
- D4 Interphase hand-off: build the spec as three stacked MSPs: Build now: cut the FEATURE branch, set up the run home, run --plan-only, then build the three MSPs in order access-roles, panel-inputs, iq-judging.
- D5 Build the three MSPs as three stacked mitosis runs: Three stacked runs in order. The first run's check-author writes all eight locked test files; later runs build from the previous MSP branch, and a fix to an earlier MSP is merged into each MSP stacked on it.
- D7 Hand-off of iq-judging: build the third stacked run from mitosis/panel-inputs: Build iq-judging now from mitosis/panel-inputs; run panel-inputs review round 1 alongside it.
- D8 Ship: push the three stacked MSPs and open their pull requests: Ship all three now: SHIP=1 resume of each run with its proven head, pull requests stacked access-roles into feat/panel-inputs-and-bot-access, panel-inputs into mitosis/access-roles, iq-judging into mitosis/panel-inputs, each assigned to the owner; 5.23 stays for the owner's dev-bot check.
- D9 Ship of panel-inputs: push mitosis/panel-inputs at its proven head: Ship panel-inputs now with --proven-head panel-inputs=a69fbc7, opened into mitosis/access-roles and assigned to the owner.

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
