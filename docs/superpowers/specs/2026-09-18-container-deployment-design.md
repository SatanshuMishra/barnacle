# Container deployment design

Barnacle runs on a self-hosted Dokploy v0.30.7 instance: single-node Docker
Swarm, Arch Linux x86_64, headless, secrets injected as environment variables.
Docker volumes sit on a btrfs subvolume with copy-on-write disabled.

## What blocked deployment

Five things, all verified before this design:

1. No Dockerfile or compose file existed. `.github/workflows/ci.yml` ran `test`
   and `licenses` and built no image.
2. `barnacle.toml` is required at startup and gitignored. `read_config` in
   `crates/barnacle-bot/src/startup.rs` fails hard when it is absent.
3. `data/` is gitignored and holds a catalog, a store and 1155 silhouette PNGs.
   Startup fails with `NoCurrentCatalog` when no catalog is selected and with
   `MissingSilhouettes` when a ship in the pool has no image.
4. The database must already exist with its schema applied. The pool is opened
   with `create_if_missing(false)` and no migration runner exists in the startup
   path.
5. Command registration is pinned to guild IDs inside the gitignored
   `barnacle.toml`.

## Two findings that shaped the design

**The runtime data is 40 MB, not 335 MB.** The bot's only data path is
`data_dir/catalog`. `Config::catalogs()` returns `data_dir.join("catalog")`, and
`CatalogRoot` opens exactly three things beneath it: the `current` pointer,
`<name>/catalog.json` and `<name>/silhouettes/<index>.png`. Nothing in
`barnacle-bot` references a store path; the only `join("store")` in the tree is
in `barnacle-data`. `data/store/common` is 294 MB of downloaded Wargaming game
files consumed only while building a catalog, and never needs to reach the host.

**Catalog adoption is human-gated by design.** `barnacle-data sync` builds a
catalog, `validate` checks it against `curation/ships.toml`, `diff` exists to be
read, and `use` writes the `current` pointer. The bot refuses to start on curation
problems or missing silhouettes. Generating or auto-fetching the newest catalog at
boot would bypass that gate, so it is wrong rather than merely slow.

The question therefore narrows to shipping one human-blessed, immutable 40 MB
bundle to the host.

## Catalog delivery

The catalog is baked into the image from a GitHub Release asset, pinned by a
committed `catalog.version` file.

`catalog.version` does double duty: the workflow uses it to choose the release
asset, and the Dockerfile copies it to `/opt/barnacle/catalog/current`, where it
is the pointer the bot reads. A build-time assertion fails when the archive and
the pointer disagree, so a mismatch fails CI instead of crash-looping the host.

Alternatives weighed and rejected:

| Option | Why not |
|---|---|
| Commit the catalog to git | 40 MB per World of Warships version in history, permanently |
| Have CI run `barnacle-data sync` | CI must pull 294 MB from Wargaming's CDN and compile the heavy crate; reachability from GitHub runners is unverified |
| Pre-populate a host volume by hand | A fresh deploy against an empty volume exits with `NoCurrentCatalog` and Swarm crash-loops until someone copies the catalog in |
| Fetch into the volume at boot | Viable, and it would let data updates skip an image rebuild, but it adds a boot network dependency, a second credential and atomic-unpack logic |

The chosen option puts the blessing where it already happens — a developer
machine, with `diff` on screen — so nothing new has to work for the deploy to be
correct.

## The volume holds only the database

`Config` treats `database` as a path independent of `data_dir`, so the catalog can
come from the image while the database comes from a volume with no code change:

- `data_dir = /opt/barnacle`, catalog at `/opt/barnacle/catalog`, read-only
- `curation = /opt/barnacle/curation/ships.toml`, read-only, already in git
- `database = /var/lib/barnacle/barnacle.sqlite3`, the one volume, about 50 KB

A SQLite file is the whole mutable surface, which suits a btrfs subvolume with
copy-on-write disabled.

## Migrations

Migration 0003 already introduced a `schema_migrations` table and backfills 0001
and 0002 into it. Migrations 0003 and 0004 open with `.bail on`, a sqlite3 CLI
dot-command rather than SQL, and guard themselves with a bare `INSERT` into that
ledger.

Piping all four on every boot therefore fails. Measured on a throwaway database:
the first run applies all four and exits 0 each time; the second run leaves 0001
and 0002 as no-ops but exits 1 on both 0003 and 0004 with `UNIQUE constraint
failed: schema_migrations.name`. The transaction rolls back cleanly, so the
damage is the exit code, not the data — but under Swarm it is a restart loop.

The entrypoint reads the ledger and applies only pending files, in lexical order,
through the `sqlite3` CLI. That preserves the dot-commands and the run-once guard
byte for byte.

`sqlx::migrate!` was rejected. It keeps its own `_sqlx_migrations` table, so it
would read an existing database as unmigrated and die on 0003's ledger insert,
and adopting it would mean rewriting migrations to drop dot-commands.

Behaviour by volume state:

| Volume state | Ledger | Result |
|---|---|---|
| No database file | absent | `sqlite3` creates the file, all four apply |
| Empty database file | absent | All four apply |
| Fully migrated | all four | All skipped |
| Pre-ledger, at 0002 | absent | 0001 and 0002 are `IF NOT EXISTS` no-ops, 0003 creates the ledger and backfills them, 0004 applies |

All four are mandatory. Startup verification demands `cb_seasons.every_day` and
`cb_rehearsal_clock` from 0004 and the `cb_seasons_live_number` index from 0003.

Before applying a pending migration to a database that already holds data, the
entrypoint copies it to `<database>.bak-<timestamp>`, mirroring the `cp` step the
README requires ahead of 0003. Any migration failure stops the entrypoint before
the bot starts.

## Config

The entrypoint renders `barnacle.toml` from environment variables. If a file
already exists at `BARNACLE_CONFIG` it is used untouched, which makes a Dokploy
File Mount an escape hatch.

No Rust changes, so every existing check still runs against the rendered file:
`NoGuilds`, `ZeroGuild`, `GuildsWithGlobal`, `ZeroRehearsalGuild`,
`RehearsalWithGlobal` and the cross-check that each rehearsal guild appears in
`commands.guilds`. Setting `BARNACLE_COMMAND_SCOPE=global` alongside
`BARNACLE_GUILD_IDS` renders both keys on purpose, so the bot's own
`GuildsWithGlobal` error is what the operator sees.

Native environment support inside `Config` was rejected: it would need precedence
rules against a `deny_unknown_fields` struct and would either duplicate or wrap
that cross-validation, in the file the whole startup contract rests on.

The entrypoint rejects a non-numeric guild ID, a scope that is neither `guilds`
nor `global`, an empty guild list under the `guilds` scope, and a path containing
a quote or backslash. Each names the variable and the offending value.

## Image

Builder: `lukemathwalker/cargo-chef:0.1.78-rust-1.97-slim-trixie`, which pins
cargo-chef and Rust 1.97 on the same glibc as the runtime. cargo-chef splits
dependency compilation from source compilation, which matters because BuildKit's
GitHub Actions cache does not persist `RUN --mount=type=cache` directories;
layer splitting is what buys the speedup.

Runtime: `debian:trixie-slim` plus `sqlite3`. The binary is otherwise
self-contained — CA roots are compiled in through `webpki-roots` and SQLite
through `libsqlite3-sys`'s `bundled` feature. The `sqlite3` CLI the migrations
need is the only reason the runtime is not distroless.

`barnacle-bot` does not depend on `barnacle-data`, so building
`--package barnacle-bot` skips `wowsunpack`, `wows-data-mgr`, `pickled` and
`image` entirely.

`tini` runs as pid 1 and forwards `SIGTERM` to the bot, which installs no signal
handler of its own. Measured without it, `docker stop` waited out the full grace
period and exited 137; with it, the container exits 143 immediately. The kernel
does not apply default signal dispositions to pid 1, so an unhandled `SIGTERM`
there is simply ignored.

The container runs as uid 10001. A named volume inherits the image's ownership of
`/var/lib/barnacle` on first use; a bind mount does not, which the deployment
note records.

## CI

`.github/workflows/image.yml` is additive: `test` and `licenses` are untouched
and no Rust source changes, so CI cannot regress. The new workflow reads
`catalog.version`, downloads the matching release asset with `GITHUB_TOKEN`,
builds `linux/amd64`, smoke-tests the image, then pushes to GHCR tagged `main`,
the commit SHA, and the catalog name.

The smoke test starts the image twice against one volume with an invalid token.
Startup order is config, environment, token, catalog, database, Discord, so an
invalid token still exercises config rendering, catalog load, curation
validation, silhouette presence, migrations and database open before failing at
login. It asserts the first start applies all four migrations, the second reports
the schema current and applies nothing, neither start reports a catalog, curation,
silhouette or database error, both reach Discord, and the ledger ends with exactly
four rows. That second start is the regression test for the crash loop above.

## Out of scope

No bot behaviour or command semantics change. No secret, database or data
directory is committed. `data/store` never reaches the host.
