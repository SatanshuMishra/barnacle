.bail on

PRAGMA foreign_keys = off;

BEGIN;

CREATE TABLE cb_seasons_rolled_back (
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
    every_day INTEGER NOT NULL DEFAULT 0 CHECK (every_day IN (0, 1)),
    CHECK (first_day <= last_day)
) STRICT;

INSERT INTO cb_seasons_rolled_back (id, guild_id, channel_id, number, codename, first_day, last_day, created_by, created_at_ms, ping_role_id, ended_at_ms, every_day)
SELECT id, guild_id, channel_id, number, codename, first_day, last_day, created_by, created_at_ms, (SELECT role_id FROM cb_season_ping_roles WHERE cb_season_ping_roles.season_id = cb_seasons.id AND position = 1), ended_at_ms, every_day FROM cb_seasons;

DROP TABLE cb_season_ping_roles;

DROP INDEX IF EXISTS cb_seasons_live_number;

DROP TABLE cb_seasons;

ALTER TABLE cb_seasons_rolled_back RENAME TO cb_seasons;

CREATE UNIQUE INDEX cb_seasons_live_number ON cb_seasons (guild_id, number) WHERE ended_at_ms IS NULL;

DELETE FROM schema_migrations WHERE name = '0006_cb_ping_roles';

COMMIT;

PRAGMA foreign_key_check;

PRAGMA foreign_keys = on;
