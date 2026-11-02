# panel-inputs-and-bot-access: Rework /event and /iq panel inputs, IQ judging and add bot access roles
Status: active
Updated: 2026-11-02T18:28:23.180Z
Next ids: c2 D6 R1 S1

## Goal

Act on the owner's live-probe feedback: typed dates and times instead of short menus, full time-zone choice with a user default, any emoji as the IQ judging reaction, offline judge reactions counted only on well-formed posts, a mark when points are awarded, an optional IQ template, event days limited to the chosen range, and a bot setting naming which roles may use /event and /iq (administrators only by default).

## Done when

- [ ] c1 The pull requests for this request are open, reviewed green and assigned to the owner

## Where things stand

Interphase Phase 9, unattended: the spec, record and items are written and pass check_record --complete, check_spec and check_items; 23 criteria (5.23 live Discord unproven), 57 locked acceptance tests in 7 new test files, 7 Steps in 3 MSPs (access-roles: organiser-access, organiser-commands; panel-inputs: member-zones, event-inputs, iq-setup-inputs; iq-judging: iq-marks-store, iq-judging-discord). The glossary (Event days, Organiser, Member's zone, Judging marks, Judge log) and ADRs 0003-0005 are written. The record scripts hold, not yet applied to the record: tests of 5.3-5.4 moved to organiser_commands.rs and the points-reason test to iq_judging.rs (so each locked file compiles at its own Step's gate), plus the schema_migrations and run_script facts in the tasks. The one spec review is running. The run home holds dispatch.zsh and worker-rules.base.txt.

## Next step

Run pipeline feature-branch --type feat, then write CHARTER.md, worker-rules.txt (base plus pipeline prevent output), items.json, graph.json and run.zsh in ~/.mitosis/barnacle/panel-inputs-and-bot-access, run run.zsh --plan-only, then build.

Needs: D1, D2, D4.

## Decisions in force

- D1 The run is unattended from the owner's instruction: no questions or read-back; gaps become assumptions citing intent lines.
- D2 Lighthouse is skipped; criterion 5.23 (live Discord) stays unproven for the owner's dev-bot check.
- D3 An award the startup rescan counts gets the same judge log line as a live award, with no offline tag (owner).
- D4 Interphase hand-off: build the spec as three stacked MSPs: Build now: cut the FEATURE branch, set up the run home, run --plan-only, then build the three MSPs in order access-roles, panel-inputs, iq-judging.
- D5 Build the three MSPs as three stacked mitosis runs: Three stacked runs in order. The first run's check-author writes all eight locked test files; later runs build from the previous MSP branch, and a fix to an earlier MSP is merged into each MSP stacked on it.

## Open risks

## Settled, do not reopen

## Where the details live

- Spec workspace: docs/specs/panel-inputs-and-bot-access.{md,decisions.md,record.json,items.json,guard.json}
- Record builder scripts (rerunning build_record.py wipes the probes): /private/tmp/claude-501/-Users-satanshumishra-Documents-DevLabs-barnacle/54bd6046-bf94-46f7-a742-87b3d50c95da/scratchpad/{build_record.py,add_tasks.py}
- Security readings for the final report, not the PR: /announce keeps owner-only plus the organiser check; panel final buttons re-check access.
- Run home: ~/.mitosis/barnacle/panel-inputs-and-bot-access (earlier runs' scripts to copy are in ~/.mitosis/barnacle/archive/)
- Owner feedback items a-h and the access-role request are in the session transcript of 2026-11-02; the live-probe log is /private/tmp/claude-501/-Users-satanshumishra-Documents-DevLabs-barnacle/54bd6046-bf94-46f7-a742-87b3d50c95da/scratchpad/live-probe/bot.log
