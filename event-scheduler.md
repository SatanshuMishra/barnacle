# event-scheduler: Generalise CB sign-ups into scheduled events under /event
Status: parked
Updated: 2026-11-01T20:53:17.372Z
Next ids: c8 D17 R1 S1

## Goal

Replace the Clan Battle-only season scheduler and the /cb command with one event scheduler and an /event command: each event has its own name, weekdays, start time in its creator's time zone, hour count and ping roles, with Clan Battles as a template of defaults, while existing CB seasons and their posted sign-ups keep working.

## Done when

- [ ] c1 /cb is gone and events are created and managed through /event
- [ ] c2 Separate events are separate entities and may overlap in dates and times
- [ ] c3 An event's start time is entered in its creator's time zone and every viewer sees it in their own zone through Discord timestamps
- [ ] c4 The admin chooses each event's length in hours
- [ ] c5 Season numbers no longer identify or title anything; events carry a name
- [ ] c6 Only the CB command surface changes; voice, guess, updates and announce commands are untouched
- [ ] c7 Existing CB seasons, their answers and their already-posted sign-up buttons keep working after the migration

## Where things stand

Converted from the old ledger.

## Next step

Decide how to clean the event-scheduler run, which predates pipeline feature branches: pipeline clean --repo /Users/satanshumishra/Documents/DevLabs/barnacle --slug event-scheduler exits 2 with "no feature.json in /Users/satanshumishra/.mitosis/barnacle/event-scheduler", since the run built on main and wrote no feature.json. Either the owner removes the run home, the run dirs under ~/.mitosis/runs/barnacle/event-scheduler*, the mitosis/event-scheduler* local branches and docs/specs/event-scheduler.* by hand, or pipeline clean gains a path for pre-feature-branch runs; then close the thread.

## Decisions in force

- D1 Interphase hand-off: build with mitosis: Start stage 2. The run home ~/.mitosis/barnacle/event-scheduler/ is created with CHARTER.md, worker-rules.txt, dispatch.zsh and run.zsh, the items file and graph are copied in, and run.zsh --plan-only runs first, followed by the build on feature branch main.
- D2 mitosis build setup: one Lane, 12-hour Worker timeout, missing test file fails: Chose 43200 seconds and a missing file as a failure. run.zsh passes --timeout 43200; plan id stayed 36661fb6d07e after the change. accept.zsh was checked on main: an existing passing test exits 0, a missing test file exits 1, an unknown test name exits 2.
- D3 mitosis run 1 result: committed, gate inconclusive from the acceptance script; fix it and resume: Fixed accept.py (errors are classified by the compiler-message target kind and name) and verified it as described; next run.zsh --resume re-gates MSP event-scheduler on ae77941.
- D4 mitosis run 2 (resume) result: gate not re-run; clear the MSP record and resume to re-gate: Back up state.json to state.before-regate.json, clear msps.0, and run run.zsh --resume into resume-2.log; read its last JSON line for the gate outcome.
- D5 mitosis run 3 (re-gate) result: exit 0, gate 170 pass; start review round 1: Review round 1 starts: full suite at ae77941 into review/round-1/event-scheduler.suite.txt, then the three reviewers with briefs from pipeline.mjs brief, criteria 7.1 to 7.36.
- D6 Review round 1 verdict: loop on four confirmed blockers; amend the spec through interphase: Amend through interphase:feature, autonomously, then build amend-1 with mitosis on mitosis/event-scheduler and review round 2.
- D7 Interphase amendment 1 hand-off: build event-scheduler-amend-1 with mitosis on mitosis/event-scheduler: Build amendment 1 with amend-1.zsh (always --no-push), read its JSON result, merge its branch into mitosis/event-scheduler in the MSP worktree, then run review round 2 of MSP event-scheduler with previous_head ae77941.
- D8 mitosis amend-1 result: committed, gate 11 pass; fast-forwarded into mitosis/event-scheduler; start review round 2: Run review round 2 of MSP event-scheduler: diff main...mitosis/event-scheduler, full suite at 1231c39, code-reviewer, conformance-auditor and security-reviewer, then candidates, validator, record and verdict.
- D9 Review round 2 verdict: loop on C1 and A1; amend the spec a second time through interphase: Amend through interphase:feature as amendment 2, autonomously, closing stale post state by one rule for every stored post-state write and incomplete criterion test by one rule that every row's expected is asserted; then build amend-2 with mitosis on mitosis/event-scheduler and review round 3 with previous_head 1231c39.
- D10 Interphase amendment 2 hand-off: build event-scheduler-amend-2 with mitosis on mitosis/event-scheduler: Build amendment 2 with amend-2.zsh (always --no-push): --plan-only first, then the build; merge its branch into mitosis/event-scheduler in the MSP worktree and run review round 3 of MSP event-scheduler with previous_head 1231c39. The five open_issues go to the final report as top risks.
- D11 mitosis amend-2 result: committed, gate 34 pass, reconcile unwritten text.rs; fast-forward into mitosis/event-scheduler and start review round 3: Fast-forward mitosis/event-scheduler to ee74853 in the event-scheduler worktree and run review round 3 with previous_head 1231c39; the reconcile finding enters round 3 as M1.
- D12 Review round 3 verdict: loop on M1 (reconcile unwritten text.rs); amend the spec a third time through interphase: Amend through interphase:feature as amendment 3, autonomously, then build amend-3 with mitosis on mitosis/event-scheduler (reconcile must be clean) and review round 4 with previous_head ee74853.
- D13 Interphase amendment 3 hand-off: build event-scheduler-amend-3 with mitosis on mitosis/event-scheduler: Copy docs/specs/event-scheduler.amend-3.items.json to ~/.mitosis/barnacle/event-scheduler/amend-3.items.json, run amend-3.zsh --plan-only, then build. The build must exit 0 with reconcile clean; then fast-forward mitosis/event-scheduler in the event-scheduler worktree and review round 4.
- D14 mitosis amend-3 result: committed, gate 1 pass, reconcile clean; fast-forward into mitosis/event-scheduler and start review round 4: Fast-forward mitosis/event-scheduler to f381f8a in /Users/satanshumishra/.mitosis/runs/barnacle/event-scheduler/trees/event-scheduler and run review round 4 (previous_head ee74853, earlier findings rounds 1 to 3).
- D15 Review round 4 verdict: green; ship event-scheduler as a draft pull request: Ship: run SHIP=1 /Users/satanshumishra/.mitosis/barnacle/event-scheduler/run.zsh --resume, read the MSP's title, body, branch and base from the JSON result, open a draft pull request through the GitHub MCP, assign it to the signed-in user and post review/event-scheduler.comment.md.
- D16 Ship: draft pull request SatanshuMishra/barnacle#49 opened for event-scheduler; park for the owner's review: Park the thread: the owner reviews draft #49, its CI and the image workflow's smoke check of migration 0008, then merges or sends changes back.

## Open risks

## Settled, do not reopen

## Where the details live

- event-scheduler spec (local, excluded from git): /Users/satanshumishra/Documents/DevLabs/barnacle/docs/specs/event-scheduler.md
- mitosis run home: CHARTER.md, run.zsh, dispatch.zsh, worker-rules.txt, items.json, graph.json, rounds.md, build.log: /Users/satanshumishra/.mitosis/barnacle/event-scheduler/
- mitosis run directory and worktree for MSP event-scheduler, branch mitosis/event-scheduler: /Users/satanshumishra/.mitosis/runs/barnacle/event-scheduler/
- barnacle acceptance script (accept.zsh, accept.py, seed.py): /Users/satanshumishra/.mitosis/barnacle/accept.zsh
- baseline full suite on main at 4f5aa70: 557 passed, 0 failed: /private/tmp/claude-501/-Users-satanshumishra-Documents-DevLabs-barnacle/338a0ed4-b2b4-46b7-953c-bea96773198e/scratchpad/baseline/suite.txt
- round 1 conformance audit findings: 34 MET, 1 NOT MET (7.16, A1 HIGH: no_event_text_says_night_or_season omits six event refusals), 7.35 not applicable (image-only); A2 LOW: no test checks the panel is ephemeral: /Users/satanshumishra/.mitosis/barnacle/event-scheduler/review/round-1/conformance-auditor.json
- round 1 security review findings: 4 LOW, BLOCKING none (S1 panel presses do not re-check Manage Server; S2 edit channel not checked against the admin's own view; S3 no cap on running events per server; S4 created_at_ms whole seconds); passed on: Signups::edit_event deletes old-channel posts before the store edit can refuse: /Users/satanshumishra/.mitosis/barnacle/event-scheduler/review/round-1/security-reviewer.json
- round 1 code review findings: C1-C3 MEDIUM reproduced (edit move deletes posts before a refusal; spring-forward posts the next session 23 h late; reopened session never closes), C4-C5 LOW; repro tests in scratchpad es/crates/barnacle-bot/tests/review_repro.rs: /Users/satanshumishra/.mitosis/barnacle/event-scheduler/review/round-1/code-reviewer.json
- round 1 merged findings (11) and candidates (4: C1, C2, C3, A1) and readable record: /Users/satanshumishra/.mitosis/barnacle/event-scheduler/review/round-1/event-scheduler.md
- Old ledger record: thread 01M4YJ7MGKJ7H6XQBXZCFEPR7H in refs/logbook/archive/ledger
