.bail on

PRAGMA foreign_keys = off;

BEGIN;

DELETE FROM schema_migrations WHERE name = '0008_events';

CREATE TEMP TABLE cb_events_rollback_check (
    ok INTEGER NOT NULL CHECK (ok = 1)
) STRICT;

INSERT INTO cb_events_rollback_check (ok)
SELECT weekdays IN (108, 127) AND start_minute = 1410 AND time_zone = 'UTC' AND hours = 4 FROM cb_seasons;

DROP TABLE cb_events_rollback_check;

CREATE TABLE cb_marks_rolled_back (
    season_id INTEGER NOT NULL,
    night TEXT NOT NULL,
    user_id INTEGER NOT NULL,
    hour INTEGER NOT NULL CHECK (hour BETWEEN 1 AND 4),
    attending INTEGER NOT NULL CHECK (attending IN (0, 1)),
    answered_at_ms INTEGER NOT NULL,
    changed_at_ms INTEGER NOT NULL,
    PRIMARY KEY (season_id, night, user_id, hour),
    FOREIGN KEY (season_id, night) REFERENCES cb_posts (season_id, night)
) STRICT;

INSERT INTO cb_marks_rolled_back (season_id, night, user_id, hour, attending, answered_at_ms, changed_at_ms)
SELECT season_id, night, user_id, hour, attending, answered_at_ms, changed_at_ms FROM cb_marks;

DROP TABLE cb_marks;

ALTER TABLE cb_marks_rolled_back RENAME TO cb_marks;

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
    ended_at_ms INTEGER CHECK (ended_at_ms IS NULL OR ended_at_ms >= 0),
    every_day INTEGER NOT NULL DEFAULT 0 CHECK (every_day IN (0, 1)),
    CHECK (first_day <= last_day)
) STRICT;

INSERT INTO cb_seasons_rolled_back (id, guild_id, channel_id, number, codename, first_day, last_day, created_by, created_at_ms, ended_at_ms, every_day)
SELECT id, guild_id, channel_id, id, name, first_day, last_day, created_by, created_at_ms, ended_at_ms, CASE WHEN weekdays = 127 THEN 1 ELSE 0 END FROM cb_seasons;

DROP INDEX IF EXISTS cb_seasons_live_name;

DROP TABLE cb_seasons;

ALTER TABLE cb_seasons_rolled_back RENAME TO cb_seasons;

CREATE UNIQUE INDEX cb_seasons_live_number ON cb_seasons (guild_id, number) WHERE ended_at_ms IS NULL;

COMMIT;

PRAGMA foreign_key_check;

PRAGMA foreign_keys = on;
