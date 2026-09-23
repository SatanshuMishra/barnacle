.bail on

PRAGMA foreign_keys = off;

BEGIN;

INSERT INTO schema_migrations (name) VALUES ('0006_cb_ping_roles');

CREATE TABLE cb_season_ping_roles (
    season_id INTEGER NOT NULL REFERENCES cb_seasons (id),
    position INTEGER NOT NULL CHECK (position BETWEEN 1 AND 5),
    role_id INTEGER NOT NULL CHECK (role_id > 0),
    PRIMARY KEY (season_id, position),
    UNIQUE (season_id, role_id)
) STRICT;

INSERT INTO cb_season_ping_roles (season_id, position, role_id)
SELECT id, 1, ping_role_id FROM cb_seasons WHERE ping_role_id IS NOT NULL;

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
    ended_at_ms INTEGER CHECK (ended_at_ms IS NULL OR ended_at_ms >= 0),
    every_day INTEGER NOT NULL DEFAULT 0 CHECK (every_day IN (0, 1)),
    CHECK (first_day <= last_day)
) STRICT;

INSERT INTO cb_seasons_rebuilt (id, guild_id, channel_id, number, codename, first_day, last_day, created_by, created_at_ms, ended_at_ms, every_day)
SELECT id, guild_id, channel_id, number, codename, first_day, last_day, created_by, created_at_ms, ended_at_ms, every_day FROM cb_seasons;

DROP TABLE cb_seasons;

ALTER TABLE cb_seasons_rebuilt RENAME TO cb_seasons;

CREATE UNIQUE INDEX cb_seasons_live_number ON cb_seasons (guild_id, number) WHERE ended_at_ms IS NULL;

COMMIT;

PRAGMA foreign_key_check;

PRAGMA foreign_keys = on;
