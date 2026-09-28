# Release notes

Barnacle's players are Discord members with no technical background. Every
version they can notice comes with short notes in plain language, which the bot
owner posts to a server with `/announce`. The notes live in
`crates/barnacle-bot/release-notes.toml` and are built into the bot, and the
test suite refuses a build whose version has no notes or whose notes break the
rules below.

## When a change needs notes

Every change players can notice raises the version and adds notes in the same
pull request. A change players cannot notice, such as a refactor or a test,
needs neither.

1. Raise `version` under `[workspace.package]` in the root `Cargo.toml`, then
   run `cargo build` so `Cargo.lock` follows it.
   - New features or changed behaviour raise the middle number and reset the
     last: 0.1.0 becomes 0.2.0.
   - Fixes alone raise the last number: 0.2.0 becomes 0.2.1.
2. Add a `[[release]]` entry for the new version to
   `crates/barnacle-bot/release-notes.toml`. Keep the older entries.

An entry has a `version` and three lists, any of which may be left out:

- `new`: one `[[release.new]]` table per feature, with `what` saying what
  players can now do and `how` saying how to use it.
- `changed`: one line per thing that now behaves differently.
- `fixed`: one line per problem that no longer happens.

```toml
[[release]]
version = "0.2.1"
changed = ["Hints now appear 5 seconds sooner."]
fixed = ["The standings no longer list the same player twice."]
```

## How to write a line

Write for a player, not for a developer. Say what they can now do, or what
they will notice, and how to use it. One short sentence per line. Never name
code, files, commands' internals, error messages or anything a player cannot
see; name a command only as a player types it, such as /guess-series.

## Rules the tests enforce

`cargo test -p barnacle-bot --test release_notes` fails, naming the version and
the line, when:

- the running version has no entry;
- two entries share a version;
- a version is not three whole numbers separated by dots, such as 0.2.0;
- an entry has no `new`, `changed` or `fixed` line at all;
- a `new` item has no `how`, or an empty one;
- a line is empty or longer than 120 characters;
- a line does not end in ".", "!" or "?";
- a line holds more than one sentence: a ".", "!" or "?" followed by a space
  before the end;
- a line contains a backtick, `**`, `__`, `::`, `->`, `http`, `.rs`, `.toml`,
  `.sql`, `{` or `}`;
- the announcement would not fit in one Discord embed: a section over 1024
  characters, or the whole post over 6000.

## Examples

Accepted, because a player knows what changed and what to do:

```toml
[[release.new]]
what = "You can now play a series of 2 to 20 silhouette rounds in a row without starting each one yourself."
how = "Type /guess-series, choose how many rounds you want (10 if you leave it out), and answer in chat as usual."
```

Rejected, because it names code with backticks and `::`, and tells a player
nothing they can use:

```toml
fixed = ["Fixed the `Table::start` race."]
```

Write it instead as what the player saw stop happening:

```toml
fixed = ["Two rounds no longer start at once in the same channel."]
```
