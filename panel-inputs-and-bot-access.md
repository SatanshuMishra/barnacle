# panel-inputs-and-bot-access: Rework /event and /iq panel inputs, IQ judging and add bot access roles
Status: parked
Updated: 2026-11-03T00:55:18.027Z
Next ids: c2 D11 R1 S1

## Goal

Act on the owner's live-probe feedback: typed dates and times instead of short menus, full time-zone choice with a user default, any emoji as the IQ judging reaction, offline judge reactions counted only on well-formed posts, a mark when points are awarded, an optional IQ template, event days limited to the chosen range, and a bot setting naming which roles may use /event and /iq (administrators only by default).

## Done when

- [x] c1 The pull requests for this request are open, reviewed green and assigned to the owner evidence: pull requests 56 (access-roles into feat/panel-inputs-and-bot-access), 57 (panel-inputs into mitosis/access-roles) and 58 (iq-judging into mitosis/panel-inputs) are open, ready for review and assigned to SatanshuMishra; all three reviewed green, with access-roles and panel-inputs in round 1 and iq-judging in round 2

## Where things stand

Pull request 59 is merged and main is at 1ccc2b7. pipeline clean removed and archived the panel-inputs-and-bot-access and iq-judging runs. It refused the panel-inputs run because its feature branch mitosis/access-roles was already deleted, so it cannot find a merge into main. Its work, a69fbc7, is in main. Still on disk: branch mitosis/panel-inputs, worktrees ~/.mitosis/runs/barnacle/panel-inputs/{base,trees/panel-inputs}, the run folder ~/.mitosis/runs/barnacle/panel-inputs (about 16 GB), and the unarchived home ~/.mitosis/barnacle/panel-inputs. ~/.mitosis/barnacle/iq-judging is kept only because mitosis/panel-inputs was in use. Criterion 5.23 (live Discord) is unproven, for the owner's dev-bot check. with_typed_dates counts today in the server's newest-event zone, not the draft's, a defect main already has.

## Next step

With the owner's go-ahead, copy ~/.mitosis/barnacle/panel-inputs to ~/.mitosis/barnacle/archive/panel-inputs, remove the worktrees ~/.mitosis/runs/barnacle/panel-inputs/{base,trees/panel-inputs}, delete branch mitosis/panel-inputs and the folders ~/.mitosis/runs/barnacle/panel-inputs, ~/.mitosis/barnacle/panel-inputs and ~/.mitosis/barnacle/iq-judging, then close the thread.

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
