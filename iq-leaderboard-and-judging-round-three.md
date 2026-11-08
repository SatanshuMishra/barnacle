# iq-leaderboard-and-judging-round-three: Guess leaderboard rename, IQ leaderboard rework, multi-player criteria, private rejections, /iq reject, 12-hour leaderboard
Status: active
Updated: 2026-11-08T22:17:00.295Z
Next ids: c9 D3 R1 S1

## Goal

Ship the owner's eight changes (a to h) to the barnacle Discord bot: /guess leaderboard, the IQ leaderboard in a container with a new header and no indent and private to the caller, no 15-minute limit on /iq setup, single or multi-player criteria, rejection reasons private to the poster, /iq reject with a companion log, and a 12-hour automatic leaderboard skipped when scores have not changed.

## Done when

- [ ] c1 /leaderboard is replaced by /guess leaderboard
- [ ] c2 /iq leaderboard renders in a container like the guess leaderboard, with the new header, no indented rows, the caller's row highlighted, and only the caller can see it
- [ ] c3 No /iq setup or edit flow times out at 15 minutes
- [ ] c4 A criterion can be single or multi-player, chosen at creation and defaulting to single; multi-player submissions credit the poster and pinged players and are rejected when pings are missing
- [ ] c5 Rejection reasons follow D2: a reply to the post in the channel, and the post gets the X
- [ ] c6 /iq reject takes a message id and mandatory reason, adds the X, blocks later awards and writes a companion log
- [ ] c7 The bot prints the leaderboard every 12 hours and skips the print when no scores changed
- [ ] c8 Reviewed independently and opened as pull requests that a human merges

## Where things stand

Not started.

## Next step

The drive entered build and runs it next.

## Decisions in force

- D1 hand-off: build the unattended spec for the owner's eight changes and one command structure: Build the spec through the drive; every later gap is filled from the recorded intent, and a human merges.
- D2 Rejection reasons stay a channel reply: Rejection reasons, including those from /iq reject, stay as a reply to the post in the channel, and the post gets the X.

## Open risks

## Settled, do not reopen

## Where the details live
