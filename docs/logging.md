# Logs

Barnacle writes one event per line to the console. Every event it writes about
its own work carries a name from the registry below, an outcome, and the Discord
and Barnacle IDs it concerns, so a log system can index the lines as they are,
with no renaming and no custom parsing.

Log schema version 1. The version is written as `barnacle.log.schema` on
`service.started`.

## Formats and settings

Two environment variables control the output. Barnacle reads them from the
process environment only, never from `.env`, because logging starts before
`.env` is read.

| Variable | Default | Values |
|---|---|---|
| `BARNACLE_LOG_FORMAT` | `text` | `text` for lines a person reads, `json` for one JSON object per line |
| `BARNACLE_LOG_LEVEL` | `info` | A level filter: `error`, `warn`, `info`, `debug`, `trace`, or per-module directives such as `info,barnacle_bot=debug` |

Case and surrounding spaces in `BARNACLE_LOG_FORMAT` do not matter, and an empty
value means the default. A value Barnacle cannot use stops the bot before it
does anything else, with a message such as:

```text
BARNACLE_LOG_FORMAT is "yaml"; it must be text or json
```

`BARNACLE_LOG_LEVEL` uses the directive syntax of `tracing-subscriber`'s
`EnvFilter`. A bare word that is not a level, such as `loud`, is read as the
name of a module to show at every level, so it passes the check and hides every
other event, including the one explaining why Barnacle stopped. Name a level
first, as in `info,barnacle_bot=debug`.

Text lines carry colour codes only when the console is a real terminal. JSON
lines never do.

The bot writes its events to stdout. The container entrypoint writes its own
lines, and in text format the bot's final startup error, to stderr. Container
logs merge both streams.

## Examples

A Join to Create room that could not be opened, in text format:

```text
2026-09-22T01:57:10.143408Z  WARN barnacle_bot::failure: a Join to Create room could not be opened event.name="voice.room.open_failed" event.outcome="failure" error.type="discord.missing_permissions" exception.message="Barnacle lacks Manage Channels and Move Members" barnacle.reference="NMV7VTAF" barnacle.permissions.missing="Manage Channels, Move Members" discord.guild.id=1180439871023710208 discord.user.id=298113415562756096 barnacle.hub.id=1180440011427405885
```

The same event in JSON format:

```json
{"timestamp":"2026-09-22T01:57:10.143938Z","level":"WARN","message":"a Join to Create room could not be opened","event.name":"voice.room.open_failed","event.outcome":"failure","error.type":"discord.missing_permissions","exception.message":"Barnacle lacks Manage Channels and Move Members","barnacle.reference":"HXG0ZRG1","barnacle.permissions.missing":"Manage Channels, Move Members","discord.guild.id":"1180439871023710208","discord.user.id":"298113415562756096","barnacle.hub.id":"1180440011427405885","target":"barnacle_bot::failure"}
```

A line from the container entrypoint in JSON format. Its timestamps have whole
seconds and its target is `barnacle::entrypoint`:

```json
{"timestamp":"2026-09-22T01:57:34Z","level":"INFO","target":"barnacle::entrypoint","message":"the database schema is current","event.name":"entrypoint.schema.current","event.outcome":"success"}
```

Every field sits at the top level of the object. Field names contain dots, so a
query addresses them quoted, for example `jq 'select(."event.name" == "voice.room.open_failed")'`.

## Event registry, version 1

Every name is `<area>.<object>.<action>`, in lowercase with dots and
underscores. Barnacle writes no other names for its own events. Events from the
libraries it uses, such as serenity, carry no `event.name`.

| Area | Events |
|---|---|
| Service | `service.started`, `service.failed`, `startup.failed`, `commands.registered` |
| Commands | `command.completed`, `command.refused`, `command.failed` |
| Interactions and gateway events | `interaction.refused`, `interaction.failed`, `interaction.reply_failed`, `event.failed` |
| Clan Battle sign-ups | `signup.post.published`, `signup.post.closed`, `signup.post.removed`, `signup.post.failed`, `signup.tick.completed`, `signup.tick.failed` |
| Rehearsal | `rehearsal.clock.cleared`, `rehearsal.clock.clear_failed` |
| Silhouette game | `guess.round.started`, `guess.round.ended`, `guess.post.failed`, `guess.solve.failed` |
| Join to Create voice | `voice.room.opened`, `voice.room.refused`, `voice.room.open_failed`, `voice.room.abandoned`, `voice.room.closed`, `voice.room.close_failed`, `voice.room.forgotten`, `voice.hub.forgotten`, `voice.notice.failed`, `voice.state.failed`, `voice.sweep.completed`, `voice.sweep.failed` |
| Container entrypoint | `entrypoint.config.written`, `entrypoint.config.found`, `entrypoint.database.backed_up`, `entrypoint.migration.applying`, `entrypoint.schema.current`, `entrypoint.failed` |

`startup.failed` means Barnacle stopped before connecting to Discord, for example
over a missing config or database. `service.failed` means it stopped after
starting to connect, for example because Discord refused the token.

## Field registry, version 1

### On every event

| Field | Type | Source |
|---|---|---|
| `timestamp` | string, RFC 3339 in UTC | the formatter |
| `level` | string: `ERROR`, `WARN`, `INFO`, `DEBUG` or `TRACE` | the formatter |
| `target` | string, the Rust module that wrote the event | the formatter |
| `message` | string, a one-line summary for a person | the formatter |
| `event.name` | string from the event registry | Barnacle |
| `event.outcome` | string: `success`, `failure` or `refused` | Barnacle |

### On failures

| Field | Type | Meaning |
|---|---|---|
| `error.type` | string, one of the values below | The kind of failure. It has few distinct values, so it suits grouping and alerting |
| `exception.message` | string | The whole error chain, outermost first, joined with `: ` |
| `barnacle.reference` | string, 8 characters | The reference quoted to the person who met the failure |
| `http.response.status_code` | number | The HTTP status Discord answered with, when Discord refused a request |
| `discord.error.code` | number | Discord's JSON error code, when Discord refused a request |
| `discord.error.message` | string | Discord's error message, when Discord refused a request |
| `barnacle.permissions.missing` | string, names separated by `, ` | The permissions Barnacle lacks, when they are known |

`startup.failed` and `service.failed` carry `error.type` and
`exception.message`, with the startup description's lines joined by ` | `, but no
reference, since no person in Discord sees them.

The values of `error.type`:

| Value | What happened | Barnacle's side |
|---|---|---|
| `discord.missing_permissions` | Discord refused because Barnacle lacks a permission (Discord code 50013) | no |
| `discord.missing_access` | Barnacle cannot see a channel it needs (50001) | no |
| `discord.unknown_channel` | The channel no longer exists (10003) | no |
| `discord.unknown_message` | The message was deleted (10008) | no |
| `discord.unknown_member` | The member has left the server (10007) | no |
| `discord.unknown_role` | The role no longer exists (10011) | no |
| `discord.unknown_guild` | Barnacle is no longer in the server (10004) | no |
| `discord.unknown_interaction` | Discord stopped waiting for Barnacle's answer (10062) | no |
| `discord.not_in_voice` | The member left voice before Barnacle could move them (40032) | no |
| `discord.channel_limit` | The server has Discord's maximum of 500 channels (30013) | no |
| `discord.invalid_request` | Discord rejected what Barnacle sent (50035) | no |
| `discord.rate_limited` | Discord is limiting how fast Barnacle can act (HTTP 429) | no |
| `discord.unavailable` | Discord answered with a server error (HTTP 500 or above) | no |
| `discord.refused` | Discord refused for another reason | no |
| `network` | Barnacle could not reach Discord | no |
| `database` | Barnacle could not read or save its data | yes |
| `storage` | Barnacle could not read one of its files | yes |
| `timeout` | Discord took too long to answer | no |
| `internal` | Something else went wrong inside Barnacle | yes |

### On refusals

| Field | Type | Meaning |
|---|---|---|
| `barnacle.refusal.reason` | string | Why Barnacle turned the request down |
| `exception.message` | string | The underlying error, when one explains the refusal, such as the text of an option that could not be read |

A command option that fails because Discord refused to look it up, for example a channel Barnacle cannot see, is not a refusal of bad input. It is logged as `command.failed` with the classified `error.type` and a reference, and the reply says what to fix.

### Context, whenever known

| Field | Type |
|---|---|
| `discord.guild.id` | string |
| `discord.channel.id` | string |
| `discord.user.id` | string |
| `discord.interaction.id` | string |
| `discord.message.id` | string |
| `discord.command.name` | string, the full command name without the slash, such as `voice hub create` |
| `barnacle.season.id` | string |
| `barnacle.night` | string |
| `barnacle.hub.id` | string, the Join to Create channel |
| `barnacle.room.id` | string, the room's voice channel |
| `barnacle.room.number` | number |
| `barnacle.round.number` | number |

A context field that is not known is left out of the line, never written as
`null`.

### Measures

| Field | Type |
|---|---|
| `barnacle.duration_ms` | number, milliseconds |
| `barnacle.failures` | number |
| `barnacle.cleared` | number |
| `barnacle.guilds` | number |

### On `service.started`

| Field | Type |
|---|---|
| `service.name` | string |
| `service.version` | string |
| `barnacle.catalog` | string, the ship catalog in use |
| `barnacle.log.schema` | string, the version of this schema |

## Levels

| Level | Used for |
|---|---|
| `ERROR` | Failures on Barnacle's side: its database, its files, or a fault inside Barnacle |
| `WARN` | Failures caused by Discord or by a server's configuration: missing permissions, deleted channels, rate limits, outages |
| `INFO` | Lifecycle events, completed actions, and refusals of what a person asked for |
| `DEBUG` | Routine background beats that did nothing notable |

An `ERROR` line is something whoever runs Barnacle has to fix. A `WARN` line is
usually fixed by a server admin, or fixes itself.

## IDs are strings

Every Discord ID and every Barnacle row ID is written as a JSON string, such as
`"discord.guild.id":"1180439871023710208"`. Discord IDs are 64-bit numbers, and
many log tools store numbers as 64-bit floats, which silently round an ID to a
different one. Counts, durations, Discord error codes and HTTP statuses are
numbers.

## References

Every failure a person sees in Discord ends with a reference, such as:

```text
`/voice hub create` did not finish. Barnacle could not read or save its data. This is a problem on Barnacle's side; tell whoever runs Barnacle and quote the reference. (Reference: HXG0ZRG1)
```

Searching the logs for `barnacle.reference` with that value finds the one event
that describes the failure, with its full error chain and context:

```bash
jq -c 'select(."barnacle.reference" == "HXG0ZRG1")' barnacle.log
grep 'barnacle.reference="HXG0ZRG1"' barnacle.log
```

The first line searches JSON logs and the second text logs. A reference is 8
characters drawn from `0123456789ABCDEFGHJKMNPQRSTVWXYZ`, which leaves out I, L,
O and U so that it reads aloud without confusion. A refusal of bad input, such as
an option out of range, carries no reference, because nothing went wrong.

## Privacy

Barnacle never logs message content, usernames, nicknames or display names. It
logs user IDs only, which are what its database already stores.

## Compatibility

Event names and field names are only ever added. Renaming or removing one bumps
`barnacle.log.schema`, so a dashboard or alert can tell which names a line uses.

## Shipping logs later

Barnacle connects to no log system itself. JSON on stdout is what log shippers
such as Vector, Promtail, Fluent Bit or an OpenTelemetry Collector ingest without
custom parsing: set `BARNACLE_LOG_FORMAT=json` and point the shipper at the
container's output.

The names follow OpenTelemetry semantic conventions where one exists
(`error.type`, `exception.message`, `http.response.status_code`), and everything
specific to Discord or Barnacle lives under `discord.*` or `barnacle.*`, so the
lines map onto OpenTelemetry log attributes one to one.

`service.name` and `service.version` describe the process rather than any one
event, so the shipper attaches them to every line as resource attributes, and
`service.started` records them once for whoever reads the raw console. When a
store indexes some fields as labels, as Loki does, keep labels to fields with few
values, such as `level`, `event.name`, `event.outcome` and `error.type`, and
leave IDs and references as searchable fields.
