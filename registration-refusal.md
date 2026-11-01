# registration-refusal: Bot survives a server refusing command registration
Status: active
Updated: 2026-11-01T20:52:45.492Z
Next ids: c6 D1 R1 S1

## Goal

Stop one refusing Discord server from taking Barnacle down: when slash-command registration is refused for a server listed in BARNACLE_GUILD_IDS (on 2026-09-29 the owner had kicked the bot from one, and production refused to start with only "Discord refused to register the commands / caused by: Missing Access"), log that server by ID with Discord's reason, skip it, and keep serving every other server.

## Done when

- [ ] c1 When Discord refuses command registration for one listed server, Barnacle logs that server's ID and Discord's reason, skips it, and still starts and serves every other listed server.
- [ ] c2 The owner's answers on three behaviours are recorded: every listed server refusing, global registration failing, and whether a refused server is retried.
- [ ] c3 A live start of the dev bot with a server ID it is not in added to its config logs the named refusal and still connects.
- [ ] c4 The fix is on its own branch with an open pull request against main and is not merged.
- [ ] c5 Formatting, clippy with warnings as errors and the full test suite pass on the branch.

## Where things stand

Converted from the old ledger.

## Next step

Run /interphase:bug on the startup crash in crates/barnacle-bot/src/discord.rs: register() loops over CommandScope::Guilds and returns on the first poise::builtins::register_in_guild error, and run() maps that to RunError::Registration and aborts before client.start(); settle with the owner what happens when every listed server refuses, when global registration fails, and whether a refused server is retried, then build the fix on a branch without merging to main.

## Decisions in force

## Open risks

## Settled, do not reopen

## Where the details live

- Old ledger record: thread 01M3N9N80EPJFJ87C3HY53GXQK in refs/logbook/archive/ledger
