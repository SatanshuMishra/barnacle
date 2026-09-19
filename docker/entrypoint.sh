#!/bin/sh
set -eu

data_dir="${BARNACLE_DATA_DIR:-/opt/barnacle}"
curation="${BARNACLE_CURATION:-/opt/barnacle/curation/ships.toml}"
database="${BARNACLE_DATABASE:-/var/lib/barnacle/barnacle.sqlite3}"
migrations="${BARNACLE_MIGRATIONS:-/opt/barnacle/migrations}"
config="${BARNACLE_CONFIG:-/etc/barnacle/barnacle.toml}"
scope="${BARNACLE_COMMAND_SCOPE:-guilds}"
guild_ids="${BARNACLE_GUILD_IDS:-}"
rehearsal_ids="${BARNACLE_REHEARSAL_GUILD_IDS:-}"

say() {
    printf 'barnacle: %s\n' "$1" >&2
}

die() {
    printf 'barnacle: %s\n' "$1" >&2
    exit 1
}

check_path() {
    case "$2" in
        *'"'*|*'\'*) die "$1 must not contain a quote or a backslash" ;;
        '') die "$1 must not be empty" ;;
    esac
}

check_ids() {
    label="$1"
    shift
    for token in "$@"; do
        case "$token" in
            *[!0-9]*|'')
                die "$label contains \"$token\", which is not a Discord server ID"
                ;;
        esac
    done
}

join_ids() {
    joined=''
    for token in "$@"; do
        if [ -z "$joined" ]; then
            joined="$token"
        else
            joined="$joined, $token"
        fi
    done
    printf '%s' "$joined"
}

case "$scope" in
    guilds|global) ;;
    *) die "BARNACLE_COMMAND_SCOPE is \"$scope\"; it must be \"guilds\" or \"global\"" ;;
esac

check_path BARNACLE_DATA_DIR "$data_dir"
check_path BARNACLE_CURATION "$curation"
check_path BARNACLE_DATABASE "$database"

guild_tokens=$(printf '%s' "$guild_ids" | tr ',' ' ')
rehearsal_tokens=$(printf '%s' "$rehearsal_ids" | tr ',' ' ')
check_ids BARNACLE_GUILD_IDS $guild_tokens
check_ids BARNACLE_REHEARSAL_GUILD_IDS $rehearsal_tokens
guilds=$(join_ids $guild_tokens)
rehearsal=$(join_ids $rehearsal_tokens)

if [ "$scope" = guilds ] && [ -z "$guilds" ]; then
    die 'BARNACLE_GUILD_IDS lists no server; set it, or set BARNACLE_COMMAND_SCOPE=global'
fi

if [ -f "$config" ]; then
    say "using the config at $config"
else
    mkdir -p "$(dirname "$config")"
    {
        printf 'data_dir = "%s"\n' "$data_dir"
        printf 'curation = "%s"\n' "$curation"
        printf 'database = "%s"\n' "$database"
        printf '\n[commands]\n'
        printf 'scope = "%s"\n' "$scope"
        if [ -n "$guilds" ]; then
            printf 'guilds = [%s]\n' "$guilds"
        fi
        if [ -n "$rehearsal" ]; then
            printf '\n[rehearsal]\n'
            printf 'guilds = [%s]\n' "$rehearsal"
        fi
    } > "$config"
    say "wrote $config from the environment"
fi

mkdir -p "$(dirname "$database")"

applied=''
if [ -s "$database" ]; then
    applied=$(sqlite3 "$database" 'SELECT name FROM schema_migrations;' 2>/dev/null || true)
fi

pending=''
for file in "$migrations"/*.sql; do
    [ -e "$file" ] || continue
    case "$file" in
        *.down.sql) continue ;;
    esac
    name=$(basename "$file" .sql)
    if printf '%s\n' "$applied" | grep -qxF "$name"; then
        continue
    fi
    pending="${pending}${pending:+ }$name"
done

if [ -z "$pending" ]; then
    say 'the database schema is current'
else
    if [ -s "$database" ]; then
        backup="$database.bak-$(date -u +%Y%m%dT%H%M%SZ)"
        cp "$database" "$backup"
        say "backed the database up to $backup"
    fi
    for name in $pending; do
        say "applying $name"
        sqlite3 "$database" < "$migrations/$name.sql"
    done
fi

exec barnacle-bot --config "$config" "$@"
