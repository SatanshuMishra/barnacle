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
    UNIQUE (guild_id, number),
    CHECK (first_day <= last_day)
) STRICT;

INSERT INTO cb_seasons_rolled_back (id, guild_id, channel_id, number, codename, first_day, last_day, created_by, created_at_ms)
SELECT id, guild_id, channel_id, number, codename, first_day, last_day, created_by, created_at_ms FROM cb_seasons;

DROP INDEX IF EXISTS cb_seasons_live_number;

DROP TABLE cb_seasons;

ALTER TABLE cb_seasons_rolled_back RENAME TO cb_seasons;

DELETE FROM schema_migrations WHERE name = '0003_cb_season_controls';

COMMIT;

PRAGMA foreign_key_check;

PRAGMA foreign_keys = on;
