# panel-inputs-and-bot-access: Rework /event and /iq panel inputs, IQ judging and add bot access roles
Status: active
Updated: 2026-11-02T17:45:52.237Z
Next ids: c2 D1 R1 S1

## Goal

Act on the owner's live-probe feedback: typed dates and times instead of short menus, full time-zone choice with a user default, any emoji as the IQ judging reaction, offline judge reactions counted only on well-formed posts, a mark when points are awarded, an optional IQ template, event days limited to the chosen range, and a bot setting naming which roles may use /event and /iq (administrators only by default).

## Done when

- [ ] c1 The pull requests for this request are open, reviewed green and assigned to the owner

## Where things stand

Interphase interview, slug panel-inputs-and-bot-access: the intent round and frontier round 1 (Q1-Q15, including the lighthouse skip as Q14) are with the owner. Settled so far: three IQ emoji fields (Denied, Awarded, Confirm awarded) that must all differ. Grounded facts: serenity 0.12.5 modals hold text inputs only; interactions carry no time zone; catch-up skips judge reactions on posts the bot already marked (discord/iq.rs catch_up_one); event length is stored in whole hours; judging_reaction has a CHECK of 4 values in migration 0009.

## Next step

Record the owner's answers to the intent round and round 1 in docs/specs/panel-inputs-and-bot-access.decisions.md and the record, then ask the next frontier round or hold the read-back.

## Decisions in force

## Open risks

## Settled, do not reopen

## Where the details live

- Owner feedback items a-h and the access-role request are in the session transcript of 2026-11-02; the live-probe log is /private/tmp/claude-501/-Users-satanshumishra-Documents-DevLabs-barnacle/54bd6046-bf94-46f7-a742-87b3d50c95da/scratchpad/live-probe/bot.log
