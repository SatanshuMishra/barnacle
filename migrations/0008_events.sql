.bail on

PRAGMA foreign_keys = off;

BEGIN;

INSERT INTO schema_migrations (name) VALUES ('0008_events');

CREATE TABLE cb_seasons_rebuilt (
    id INTEGER PRIMARY KEY,
    guild_id INTEGER NOT NULL,
    channel_id INTEGER NOT NULL,
    name TEXT NOT NULL CHECK (length(name) BETWEEN 1 AND 200),
    first_day TEXT NOT NULL,
    last_day TEXT NOT NULL,
    weekdays INTEGER NOT NULL CHECK (weekdays BETWEEN 1 AND 127),
    start_minute INTEGER NOT NULL CHECK (start_minute BETWEEN 0 AND 1439),
    time_zone TEXT NOT NULL CHECK (length(time_zone) BETWEEN 1 AND 64),
    hours INTEGER NOT NULL CHECK (hours BETWEEN 1 AND 24),
    created_by INTEGER NOT NULL,
    created_at_ms INTEGER NOT NULL,
    ended_at_ms INTEGER CHECK (ended_at_ms IS NULL OR ended_at_ms >= 0),
    CHECK (first_day <= last_day)
) STRICT;

INSERT INTO cb_seasons_rebuilt (id, guild_id, channel_id, name, first_day, last_day, weekdays, start_minute, time_zone, hours, created_by, created_at_ms, ended_at_ms)
SELECT id, guild_id, channel_id, 'Clan Battles · Season ' || number || CASE WHEN codename IS NULL THEN '' ELSE ': ' || codename END, first_day, last_day, CASE WHEN every_day = 1 THEN 127 ELSE 108 END, 1410, 'UTC', 4, created_by, created_at_ms, ended_at_ms FROM cb_seasons;

DROP TABLE cb_seasons;

ALTER TABLE cb_seasons_rebuilt RENAME TO cb_seasons;

CREATE UNIQUE INDEX cb_seasons_live_name ON cb_seasons (guild_id, name COLLATE NOCASE) WHERE ended_at_ms IS NULL;

CREATE TABLE cb_marks_rebuilt (
    season_id INTEGER NOT NULL,
    night TEXT NOT NULL,
    user_id INTEGER NOT NULL,
    hour INTEGER NOT NULL CHECK (hour BETWEEN 1 AND 24),
    attending INTEGER NOT NULL CHECK (attending IN (0, 1)),
    answered_at_ms INTEGER NOT NULL,
    changed_at_ms INTEGER NOT NULL,
    PRIMARY KEY (season_id, night, user_id, hour),
    FOREIGN KEY (season_id, night) REFERENCES cb_posts (season_id, night)
) STRICT;

INSERT INTO cb_marks_rebuilt (season_id, night, user_id, hour, attending, answered_at_ms, changed_at_ms)
SELECT season_id, night, user_id, hour, attending, answered_at_ms, changed_at_ms FROM cb_marks;

DROP TABLE cb_marks;

ALTER TABLE cb_marks_rebuilt RENAME TO cb_marks;

COMMIT;

PRAGMA foreign_key_check;

PRAGMA foreign_keys = on;
