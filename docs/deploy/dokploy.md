# Deploying Barnacle to Dokploy

Barnacle runs as one container. The image carries the bot, the ship catalog and
the migrations; a volume carries only the SQLite database.

## Before the first deploy

The image build downloads the catalog from a GitHub Release named after
`catalog.version`. That release has to exist first, or the `image` workflow fails
with a missing-release error on its very first run:

```bash
tar -C data/catalog -czf catalog.tar.gz "$(cat data/catalog/current)"
gh release create "catalog-$(cat data/catalog/current)" catalog.tar.gz --notes "Catalog only"
```

Confirm `catalog.version` names the same catalog as `data/catalog/current`, then
merge.

## What lives where

| Path | Source | Writable |
|---|---|---|
| `/usr/local/bin/barnacle-bot` | image | no |
| `/opt/barnacle/catalog` | image, from the catalog archive | no |
| `/opt/barnacle/curation/ships.toml` | image, from git | no |
| `/opt/barnacle/migrations` | image, from git | no |
| `/etc/barnacle/barnacle.toml` | rendered at start, or mounted | yes |
| `/var/lib/barnacle/barnacle.sqlite3` | volume | yes |

## Environment variables

| Name | Required | Default | Meaning |
|---|---|---|---|
| `DISCORD_TOKEN` | yes | none | The bot token from the Discord Developer Portal |
| `BARNACLE_GUILD_IDS` | when the scope is `guilds` | none | Comma-separated server IDs to register commands in |
| `BARNACLE_COMMAND_SCOPE` | no | `guilds` | `guilds` or `global` |
| `BARNACLE_REHEARSAL_GUILD_IDS` | no | none | Servers `/rehearse reset` may wipe; each must appear in `BARNACLE_GUILD_IDS` |
| `BARNACLE_DATABASE` | no | `/var/lib/barnacle/barnacle.sqlite3` | Database path, inside the volume |
| `BARNACLE_DATA_DIR` | no | `/opt/barnacle` | Parent of the `catalog` directory |
| `BARNACLE_CURATION` | no | `/opt/barnacle/curation/ships.toml` | Curation file path |
| `BARNACLE_CONFIG` | no | `/etc/barnacle/barnacle.toml` | Config path; an existing file is used as-is |

`RUST_LOG` has no effect. `tracing-subscriber` is built without its
`env-filter` feature, so logging is fixed at INFO on stderr.

## Setting the application up

1. Create an Application in Dokploy and set its source to **Docker**.
2. Fill in the registry fields:
   - Docker Image: `ghcr.io/satanshumishra/barnacle:main`
   - Registry URL: `ghcr.io`
   - Username: your GitHub username
   - Password: a personal access token with `read:packages`

   The repository is private, so its package is too and these credentials are
   required.
3. Under Volumes, add a **Volume Mount**:
   - Volume Name: `barnacle-data`
   - Mount Path: `/var/lib/barnacle`
4. Under Environment, set `DISCORD_TOKEN` and `BARNACLE_GUILD_IDS`. Add
   `BARNACLE_REHEARSAL_GUILD_IDS` only if you rehearse in a throwaway server.
5. Publish no ports and set no domain. Barnacle is an outbound gateway client,
   so it listens on nothing and has no HTTP health check.
6. Deploy.

## First start against an empty volume

Nothing to do by hand. The entrypoint creates the database, applies all four
migrations in order and starts the bot. The logs read:

```text
barnacle: wrote /etc/barnacle/barnacle.toml from the environment
barnacle: applying 0001_guess_solves
barnacle: applying 0002_cb_attendance
barnacle: applying 0003_cb_season_controls
barnacle: applying 0004_cb_rehearsal_harness
```

On every later start the entrypoint reads the `schema_migrations` ledger, finds
nothing pending and reports `the database schema is current`. Before applying a
pending migration to a database that already holds data it copies the file to
`<database>.bak-<timestamp>`, which is the backup step the README requires ahead
of migration 0003.

A migration that fails stops the entrypoint before the bot starts, so the
database is never left half-migrated. Swarm restarts the task, it fails the same
way, and the logs name the file.

## Adopting a new World of Warships version

The catalog is deliberately not built on the host. Adopting one requires
`barnacle-data diff` and `validate` to pass under your eye, so it stays a
decision you make on your own machine.

1. Build and bless the catalog locally, as `README.md` describes:

   ```bash
   cargo run --release -p barnacle-data -- sync
   cargo run --release -p barnacle-data -- diff
   cargo run --release -p barnacle-data -- use <version>_<build>_r<n>
   ```

2. Archive the catalog that `use` selected:

   ```bash
   tar -C data/catalog -czf catalog.tar.gz "$(cat data/catalog/current)"
   ```

3. Publish it as a release asset named after the catalog:

   ```bash
   gh release create "catalog-$(cat data/catalog/current)" catalog.tar.gz --notes "Catalog only"
   ```

4. Point the repository at it and push:

   ```bash
   cp data/catalog/current catalog.version
   git commit -am "chore(data): adopt catalog $(cat catalog.version)"
   git push
   ```

The push builds a new image, smoke-tests it and pushes it to GHCR. Redeploy in
Dokploy to pick it up. `catalog.version` becomes the `current` pointer inside the
image, so git history records which catalog every deploy ran, and the build fails
if the archive and the version file disagree.

## Running the image locally

`compose.yaml` mirrors the Dokploy setup. Build the image first, because the
published one is private:

```bash
tar -C data/catalog -czf catalog.tar.gz "$(cat data/catalog/current)"
docker build -t ghcr.io/satanshumishra/barnacle:main .
DISCORD_TOKEN=... BARNACLE_GUILD_IDS=... docker compose up
```

To check an image without a real token, run the smoke test:

```bash
./.github/scripts/smoke.sh ghcr.io/satanshumishra/barnacle:main
```

## Caveats

A **Bind Mount** in place of the Volume Mount will not work without preparation.
The container runs as uid 10001, and a named volume inherits the image's
ownership of `/var/lib/barnacle` on first use, while a host directory does not —
`chown 10001:10001` it first.

Logs carry ANSI escape codes. `tracing-subscriber`'s formatter does not detect a
non-terminal, and the bot does not disable colour.
