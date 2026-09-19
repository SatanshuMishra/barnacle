#!/usr/bin/env bash
set -euo pipefail

image="${1:?usage: smoke.sh <image>}"
volume="barnacle-smoke-$$"
failures=0

cleanup() {
    docker volume rm -f "$volume" >/dev/null 2>&1 || true
}
trap cleanup EXIT

report() {
    if [ "$1" = pass ]; then
        printf 'pass  %s\n' "$2"
    else
        printf 'FAIL  %s\n' "$2"
        failures=$((failures + 1))
    fi
}

expect_absent() {
    if printf '%s' "$2" | grep -qF "$3"; then
        report fail "$1"
    else
        report pass "$1"
    fi
}

expect_present() {
    if printf '%s' "$2" | grep -qE "$3"; then
        report pass "$1"
    else
        report fail "$1"
    fi
}

start() {
    docker run --rm \
        --volume "$volume:/var/lib/barnacle" \
        --env DISCORD_TOKEN=smoke.invalid.token \
        --env BARNACLE_GUILD_IDS=1 \
        "$image" 2>&1 || true
}

docker volume create "$volume" >/dev/null

printf '\n=== first start against an empty volume ===\n'
first="$(start)"
printf '%s\n' "$first"

printf '\n=== second start against the same volume ===\n'
second="$(start)"
printf '%s\n' "$second"

ledger="$(docker run --rm \
    --volume "$volume:/var/lib/barnacle" \
    --entrypoint sqlite3 \
    "$image" /var/lib/barnacle/barnacle.sqlite3 \
    'SELECT name FROM schema_migrations ORDER BY name;')"

printf '\n=== checks ===\n'

for migration in 0001_guess_solves 0002_cb_attendance 0003_cb_season_controls 0004_cb_rehearsal_harness; do
    expect_present "first start applies $migration" "$first" "applying $migration"
done
expect_present 'first start renders the config from the environment' "$first" \
    'wrote /etc/barnacle/barnacle.toml from the environment'
expect_present 'second start finds the schema current' "$second" \
    'the database schema is current'
expect_absent 'second start applies nothing' "$second" 'applying 0'

for stage in "$first" "$second"; do
    expect_present 'startup reaches Discord' "$stage" 'Discord'
    expect_absent 'no catalog is missing' "$stage" 'no catalog is selected'
    expect_absent 'the catalog loads' "$stage" 'current catalog could not be loaded'
    expect_absent 'curation validates' "$stage" 'curation has'
    expect_absent 'every silhouette is present' "$stage" 'have no silhouette file'
    expect_absent 'the config is valid' "$stage" 'is not a valid Barnacle config'
    expect_absent 'the token is found' "$stage" 'DISCORD_TOKEN is not set'
    expect_absent 'the solves database is ready' "$stage" 'is not ready'
    expect_absent 'the attendance tables are present' "$stage" 'tables are missing'
    expect_absent 'the season controls are present' "$stage" 'season controls are missing'
done

expected_ledger='0001_guess_solves
0002_cb_attendance
0003_cb_season_controls
0004_cb_rehearsal_harness'

if [ "$ledger" = "$expected_ledger" ]; then
    report pass 'the migration ledger holds every migration'
else
    report fail "the migration ledger holds every migration (found: $ledger)"
fi

printf '\n'
if [ "$failures" -ne 0 ]; then
    printf '%s check(s) failed\n' "$failures"
    exit 1
fi
printf 'every check passed\n'
