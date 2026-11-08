# iq-leaderboard-and-judging-round-three: Guess leaderboard rename, IQ leaderboard rework, multi-player criteria, private rejections, /iq reject, 12-hour leaderboard
Status: active
Updated: 2026-11-08T23:34:23.134Z
Next ids: c9 D4 R4 S1

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

The spec is handed off and `pipeline drive` is building it unattended on feat/iq-leaderboard-and-judging-round-three as one MSP, iq-round-three. Two of 7 Steps are committed on the build branch and iq-board-view is building; no pull request is open.

## Next step

The drive entered build and runs it next.

## Decisions in force

- D1 hand-off: build the unattended spec for the owner's eight changes and one command structure: Build the spec through the drive; every later gap is filled from the recorded intent, and a human merges.
- D2 Rejection reasons stay a channel reply: Rejection reasons, including those from /iq reject, stay as a reply to the post in the channel, and the post gets the X.

- D3 One command structure: every feature has one root command and each action is a subcommand, as the owner asked; /about stays top-level because /barnacle is hidden from non-administrators.

## Open risks

- R1 How the embed heading renders and the gateway handlers behave is unproven until the bot runs on a real Discord server. Bears on c2.
- R2 Migration 0013 must be applied before the new binary starts, or startup refuses.
- R3 The watcher's restart of the live run is untested, and the drive's build stage resumes only because this run's run.zsh adds the resume flag itself.

## Settled, do not reopen

## Where the details live

- docs/specs/iq-leaderboard-and-judging-round-three.md: the spec, with its decisions and record beside it under the same name.
- ~/.mitosis/barnacle/iq-leaderboard-and-judging-round-three/: the run home, with CHARTER.md, run.zsh, the review rounds, and watch/ holding the watcher's log and status.
- ~/.mitosis/barnacle/watch-drive.py: the token-free watcher that notifies on progress and restarts a stalled or dead drive.
