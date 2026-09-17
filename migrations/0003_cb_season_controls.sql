.bail on

PRAGMA foreign_keys = off;

BEGIN;

CREATE TABLE IF NOT EXISTS schema_migrations (
    name TEXT PRIMARY KEY
) STRICT;

INSERT OR IGNORE INTO schema_migrations (name) VALUES ('0001_guess_solves'), ('0002_cb_attendance');

INSERT INTO schema_migrations (name) VALUES ('0003_cb_season_controls');

CREATE TABLE cb_seasons_rebuilt (
    id INTEGER PRIMARY KEY,
    guild_id INTEGER NOT NULL,
    channel_id INTEGER NOT NULL,
    number INTEGER NOT NULL CHECK (number > 0),
    codename TEXT,
    first_day TEXT NOT NULL,
    last_day TEXT NOT NULL,
    created_by INTEGER NOT NULL,
    created_at_ms INTEGER NOT NULL,
    ping_role_id INTEGER CHECK (ping_role_id IS NULL OR ping_role_id > 0),
    ended_at_ms INTEGER CHECK (ended_at_ms IS NULL OR ended_at_ms >= 0),
    CHECK (first_day <= last_day)
) STRICT;

INSERT INTO cb_seasons_rebuilt (id, guild_id, channel_id, number, codename, first_day, last_day, created_by, created_at_ms)
SELECT id, guild_id, channel_id, number, codename, first_day, last_day, created_by, created_at_ms FROM cb_seasons;

DROP TABLE cb_seasons;

ALTER TABLE cb_seasons_rebuilt RENAME TO cb_seasons;

CREATE UNIQUE INDEX cb_seasons_live_number ON cb_seasons (guild_id, number) WHERE ended_at_ms IS NULL;

COMMIT;

PRAGMA foreign_key_check;

PRAGMA foreign_keys = on;
