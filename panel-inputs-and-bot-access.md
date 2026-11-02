# panel-inputs-and-bot-access: Rework /event and /iq panel inputs, IQ judging and add bot access roles
Status: active
Updated: 2026-11-02T19:46:01.136Z
Next ids: c2 D7 R1 S1

## Goal

Act on the owner's live-probe feedback: typed dates and times instead of short menus, full time-zone choice with a user default, any emoji as the IQ judging reaction, offline judge reactions counted only on well-formed posts, a mark when points are awarded, an optional IQ template, event days limited to the chosen range, and a bot setting naming which roles may use /event and /iq (administrators only by default).

## Done when

- [ ] c1 The pull requests for this request are open, reviewed green and assigned to the owner

## Where things stand

Stage 2, building run 1 of 3 (access-roles) on FEATURE branch feat/panel-inputs-and-bot-access. Interphase is handed off: 23 criteria (5.23 live Discord unproven), 60 locked acceptance tests in 8 files, 7 Steps; the spec review's 17 findings are fixed. The check-author lock commit (425f25a) and organiser-access (c10a7fc) are on mitosis/access-roles; organiser-commands is being built in lane 1. Runs panel-inputs and iq-judging are set up but not planned, since their base branches do not exist yet.

## Next step

While ~/.mitosis/barnacle/panel-inputs/run.zsh builds, run access-roles review round 1 in its worktree ~/.mitosis/runs/barnacle/panel-inputs-and-bot-access/trees/access-roles (base feat/panel-inputs-and-bot-access): review-diff, obligations, suite, standard-checks, then code-reviewer, conformance-auditor and security-reviewer through pipeline agent.

Needs: D5, D6.

## Decisions in force

- D1 The run is unattended from the owner's instruction: no questions or read-back; gaps become assumptions citing intent lines.
- D2 Lighthouse is skipped; criterion 5.23 (live Discord) stays unproven for the owner's dev-bot check.
- D3 An award the startup rescan counts gets the same judge log line as a live award, with no offline tag (owner).
- D4 Interphase hand-off: build the spec as three stacked MSPs: Build now: cut the FEATURE branch, set up the run home, run --plan-only, then build the three MSPs in order access-roles, panel-inputs, iq-judging.
- D5 Build the three MSPs as three stacked mitosis runs: Three stacked runs in order. The first run's check-author writes all eight locked test files; later runs build from the previous MSP branch, and a fix to an earlier MSP is merged into each MSP stacked on it.
- D6 Hand-off of panel-inputs: build the second stacked run from mitosis/access-roles: Build panel-inputs now from mitosis/access-roles; start access-roles review round 1 alongside it.

## Open risks

## Settled, do not reopen

## Where the details live

- Spec workspace: docs/specs/panel-inputs-and-bot-access.{md,decisions.md,record.json,items.json}
- Build state: ~/.mitosis/runs/barnacle/<slug>/state.json, with lane logs under lanes/ and work in progress under clones/
- Record builder scripts (rerunning build_record.py wipes the probes): /private/tmp/claude-501/-Users-satanshumishra-Documents-DevLabs-barnacle/54bd6046-bf94-46f7-a742-87b3d50c95da/scratchpad/{build_record.py,add_tasks.py}
- Security readings for the final report, not the PR: /announce keeps owner-only plus the organiser check; panel final buttons re-check access.
- Run homes: ~/.mitosis/barnacle/{panel-inputs-and-bot-access,panel-inputs,iq-judging}, each with CHARTER.md, items.json, run.zsh, dispatch.zsh, worker-rules.txt and feature.json; items.all.json in the first holds all seven Steps
- Owner feedback items a-h and the access-role request are in the session transcript of 2026-11-02; the live-probe log is /private/tmp/claude-501/-Users-satanshumishra-Documents-DevLabs-barnacle/54bd6046-bf94-46f7-a742-87b3d50c95da/scratchpad/live-probe/bot.log
