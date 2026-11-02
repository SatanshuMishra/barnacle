# panel-inputs-and-bot-access: Rework /event and /iq panel inputs, IQ judging and add bot access roles
Status: active
Updated: 2026-11-02T20:04:29.834Z
Next ids: c2 D7 R1 S1

## Goal

Act on the owner's live-probe feedback: typed dates and times instead of short menus, full time-zone choice with a user default, any emoji as the IQ judging reaction, offline judge reactions counted only on well-formed posts, a mark when points are awarded, an optional IQ template, event days limited to the chosen range, and a bot setting naming which roles may use /event and /iq (administrators only by default).

## Done when

- [ ] c1 The pull requests for this request are open, reviewed green and assigned to the owner

## Where things stand

access-roles is built and gated (8 of 8 pass, reconcile clean) on mitosis/access-roles at d461aab, base 425f25a (the lock commit, now on feat/panel-inputs-and-bot-access); its round 1 suite passes scoped (857 passed, 0 failed, every target except the six later-Step locked files, which fail to compile the same way on the base, so the full-run U1 is not a finding); conformance found all of 5.1-5.4 met; 5 LOW candidates (C1 /announce Send re-checks only ownership; C2 and S1 no test for the 0010 down migration; C3 a sixth copy of the table-check helpers; T1 the down migration's DROP TABLE) are with the validator. panel-inputs is resumed a second time: twice it stopped cleaning lane 0's clone because the Worker session's graphify Stop hook wrote graphify-out into it; every run home's dispatch.zsh now exports GRAPHIFY_DENY_ROOTS with ~/.mitosis, which a probe showed stops the hook in a clone. member-zones is built and held; event-inputs and iq-setup-inputs are building. iq-judging waits for mitosis/panel-inputs. access-roles left text::UPDATES_NOT_SET_UP unchanged because tests/text.rs asserts the old text.

## Next step

When the access-roles validator finishes, save its last JSON array as review/round-1/access-roles.verdicts.json, run pipeline record and verdict, then fix every confirmed advisory plus the unfinished UPDATES_NOT_SET_UP text (updating tests/text.rs) on mitosis/access-roles and merge the fixes into mitosis/panel-inputs.

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
- Review helpers: /private/tmp/claude-501/-Users-satanshumishra-Documents-DevLabs-barnacle/54bd6046-bf94-46f7-a742-87b3d50c95da/scratchpad/render_briefs.py renders a round's briefs; ~/.mitosis/runs/barnacle/panel-inputs-and-bot-access/base is the base worktree for pipeline suite
- Security readings for the final report also include /barnacle's server-side required_permissions ADMINISTRATOR (lane 1 assumption).
- Owner feedback items a-h and the access-role request are in the session transcript of 2026-11-02; the live-probe log is /private/tmp/claude-501/-Users-satanshumishra-Documents-DevLabs-barnacle/54bd6046-bf94-46f7-a742-87b3d50c95da/scratchpad/live-probe/bot.log
