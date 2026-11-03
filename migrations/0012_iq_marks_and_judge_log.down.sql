.bail on

PRAGMA foreign_keys = off;

BEGIN;

CREATE TABLE iq_competitions_rebuilt (
    id INTEGER PRIMARY KEY,
    guild_id INTEGER NOT NULL,
    name TEXT NOT NULL CHECK (length(name) BETWEEN 1 AND 100),
    first_day TEXT NOT NULL,
    last_day TEXT NOT NULL,
    time_zone TEXT NOT NULL CHECK (length(time_zone) BETWEEN 1 AND 64),
    status TEXT NOT NULL CHECK (status IN ('running', 'stopped', 'completed', 'archived')),
    category_id INTEGER NOT NULL,
    announcements_id INTEGER NOT NULL,
    submissions_id INTEGER NOT NULL,
    judging_reaction TEXT NOT NULL CHECK (judging_reaction IN ('trophy', 'star', 'hundred', 'crown')),
    rules TEXT NOT NULL CHECK (length(rules) <= 4000),
    last_seen_message INTEGER CHECK (last_seen_message IS NULL OR last_seen_message >= 0),
    created_by INTEGER NOT NULL,
    created_at_ms INTEGER NOT NULL,
    CHECK (first_day <= last_day)
) STRICT;

INSERT INTO iq_competitions_rebuilt (id, guild_id, name, first_day, last_day, time_zone, status, category_id, announcements_id, submissions_id, judging_reaction, rules, last_seen_message, created_by, created_at_ms)
SELECT id, guild_id, name, first_day, last_day, time_zone, status, category_id, announcements_id, submissions_id, CASE awarded_mark WHEN char(0x2B50) THEN 'star' WHEN char(0x1F4AF) THEN 'hundred' WHEN char(0x1F451) THEN 'crown' ELSE 'trophy' END, rules, last_seen_message, created_by, created_at_ms FROM iq_competitions;

DROP TABLE iq_competitions;

ALTER TABLE iq_competitions_rebuilt RENAME TO iq_competitions;

CREATE UNIQUE INDEX iq_competitions_live ON iq_competitions (guild_id) WHERE status <> 'archived';

DROP INDEX iq_submissions_unawarded;

DELETE FROM schema_migrations WHERE name = '0012_iq_marks_and_judge_log';

COMMIT;

PRAGMA foreign_key_check;

PRAGMA foreign_keys = on;
