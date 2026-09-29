# Release notes

Barnacle's players are Discord members with no technical background. Every
version they can notice comes with notes in plain language, which the bot owner
posts to a server with `/announce`. The notes live in
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

`/announce` only ever posts the running version's notes, so an entry must stand
on its own: a player who missed every earlier post still has to understand it.

## The pattern: tell the story

Before writing, picture a reader who has never used Barnacle and sees only this
post. After reading it, they should be able to say what changed, who it is
for, how to use it and what will happen when they do. Every rule below serves
that reader.

1. **Open with the whole update in one sentence.** The `summary` says what the
   update brings, in the reader's terms: "This update lets you play several
   silhouette rounds in a row and lets servers choose where Barnacle posts
   news."
2. **Name sections for what is inside them.** The post uses "New features",
   "Changes to existing features" and "Fixes", never a bare "New" or
   "Changed".
3. **One feature, one story.** Everything a player meets while using one
   feature belongs to that feature. A countdown, a scoreboard or a check that
   only exists inside a series is part of the series, described under it, not
   a feature of its own. Ask: "Would a player ever use this without the other
   thing?" If not, it is one feature.
4. **Tell each feature in the order a player lives it.**
   - `story`: one or two sentences on what the feature is and why it helps.
     The strongest form says what was true before and what is true now:
     "Until now, each round had to be started by hand. A series plays up to 20
     rounds back to back."
   - `steps`: how to use it, as numbered steps in the order the player does
     them, each naming the exact command they type.
   - `expect`: what happens next, in the order it happens, including anything
     that runs on its own (a countdown, a cleanup, a check, an ending).
5. **Say who it is for** when it is not every player: `audience = "server
   managers"` prints "For server managers" under the feature's name.
6. **Tell a change as before and now.** A `[[release.change]]` gives the
   feature's name, what it did `before` and what it does `now`.
7. **Tell a fix as what the player will notice.** Say what stopped happening,
   not what was wrong in the code: "Two rounds no longer start at once in the
   same channel."

## The entry

```toml
[[release]]
version = "0.3.0"
summary = "One sentence on what this update brings."
fixes = ["What the player will notice no longer goes wrong."]

[[release.feature]]
title = "Feature name as a player would say it"
audience = "server managers"
story = [
    "What was true before, or what this feature is.",
    "What is true now, and why it helps.",
]
steps = [
    "The first thing the player does, with the command they type.",
    "The next thing they do.",
]
expect = [
    "The first thing that happens.",
    "The next thing that happens, in order.",
]

[[release.change]]
title = "Existing feature's name"
before = "What it used to do."
now = "What it does now."
```

`audience`, `expect`, `[[release.change]]` and `fixes` may be left out; a
section with nothing in it does not appear in the post. `fixes` sits on the
`[[release]]` table itself, above its first `[[release.feature]]`, because
TOML cannot return to a table once a sub-table has started.

## How the post looks

`/announce` sends the notes as a normal Discord message, not an embed. Any role
ping comes first on its own line, then:

```text
# Barnacle Update 0.2.0
This update lets you play several silhouette rounds in a row and lets servers choose where Barnacle posts news.

## New features

### Silhouette series
Until now, each round of the ship silhouette game (/guess) had to be started by hand, one at a time. A series plays up to 20 rounds back to back and keeps score for everyone taking part.
**How to use it**
1. Type /guess-series in any channel where /guess works.
2. Choose how many rounds to play, from 2 to 20; if you leave it out, you get 10.
3. Name each ship in chat as it appears, just like /guess.
**What to expect**
- A 5-second countdown appears before every ship, so you are ready to answer.
- After each round, the standings show who has won the most rounds and who was fastest so far.
- The standings clear away when the next ship appears, so the channel stays tidy.
- If nobody talks for three rounds, Barnacle asks if you are still playing; press Yes within 10 seconds to keep going.
- When the series ends, the final standings stay in the channel for everyone to see.

### Update news channel
-# For server managers
Barnacle can now post news like this in a channel your server picks, so members hear about changes as they happen.
**How to use it**
1. Someone with Manage Server types /updates setup.
2. They pick the channel for the news and, if they like, a role to notify.

-# Questions or feedback? Reach out to a server administrator.
```

A change renders under "## Changes to existing features" as its `###` name
followed by "**Before:**" and "**Now:**" lines, and fixes render under
"## Fixes" as bullets.

## How to write a line

Write for a player, not for a developer. One short sentence per line. Never
name code, files, error messages or anything a player cannot see; name a
command only as a player types it, such as /guess-series.

A `title` is the feature's short name as a player would say it, such as
Silhouette series: 1 to 40 characters, with no ".", "!" or "?" at the end. An
`audience` follows the same rule.

## Rules the tests enforce

`cargo test -p barnacle-bot --test release_notes` fails, naming the version and
the line, when:

- the running version has no entry;
- two entries share a version;
- a version is not three whole numbers separated by dots, such as 0.2.0;
- an entry has no `summary`;
- an entry has no feature, change or fix at all;
- a feature or change has no `title`, or an empty one;
- a feature has no `story`, a `story` of more than two sentences, or no
  `steps`;
- a `title` or `audience` is empty, longer than 40 characters, ends in ".",
  "!" or "?", or contains any of the sequences listed below for lines;
- a line (the summary, a story sentence, a step, an expectation, a before, a
  now or a fix) is empty or longer than 120 characters;
- a line does not end in ".", "!" or "?";
- a line holds more than one sentence: a ".", "!" or "?" followed by a space
  before the end;
- a line contains a backtick, `**`, `__`, `::`, `->`, `http`, `.rs`, `.toml`,
  `.sql`, `{` or `}`;
- the post plus the longest ping line is over 2000 characters.

The tests check form, not story. Whether a zero-context reader can retell the
update is checked by reading the rendered post before it merges.

## Examples

Split, so a reader sees three features where there is one:

```toml
[[release.feature]]
title = "Guess series"
story = ["You can now play several rounds in a row."]
steps = ["Type /guess-series."]

[[release.feature]]
title = "Series standings"
story = ["After every round, Barnacle shows the standings."]
steps = ["Keep playing."]
```

Told as one story instead, with the standings as something the player can
expect while playing a series:

```toml
[[release.feature]]
title = "Silhouette series"
story = ["A series plays up to 20 rounds back to back and keeps score for everyone taking part."]
steps = ["Type /guess-series in any channel where /guess works."]
expect = ["After each round, the standings show who has won the most rounds and who was fastest so far."]
```

Rejected, because it names code and tells a player nothing they can use:

```toml
fixes = ["Fixed the `Table::start` race."]
```

Written as what the player saw stop happening:

```toml
fixes = ["Two rounds no longer start at once in the same channel."]
```
