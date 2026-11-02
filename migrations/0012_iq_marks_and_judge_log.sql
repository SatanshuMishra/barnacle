.bail on

PRAGMA foreign_keys = off;

BEGIN;

INSERT INTO schema_migrations (name) VALUES ('0012_iq_marks_and_judge_log');

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
    judge_log_channel_id INTEGER NULL,
    denied_mark TEXT NOT NULL CHECK (length(denied_mark) BETWEEN 1 AND 100),
    awarded_mark TEXT NOT NULL CHECK (length(awarded_mark) BETWEEN 1 AND 100),
    confirm_mark TEXT NOT NULL CHECK (length(confirm_mark) BETWEEN 1 AND 100),
    rules TEXT NOT NULL CHECK (length(rules) <= 4000),
    last_seen_message INTEGER CHECK (last_seen_message IS NULL OR last_seen_message >= 0),
    created_by INTEGER NOT NULL,
    created_at_ms INTEGER NOT NULL,
    CHECK (first_day <= last_day),
    CHECK (denied_mark <> awarded_mark AND denied_mark <> confirm_mark AND awarded_mark <> confirm_mark)
) STRICT;

INSERT INTO iq_competitions_rebuilt (id, guild_id, name, first_day, last_day, time_zone, status, category_id, announcements_id, submissions_id, judge_log_channel_id, denied_mark, awarded_mark, confirm_mark, rules, last_seen_message, created_by, created_at_ms)
SELECT id, guild_id, name, first_day, last_day, time_zone, status, category_id, announcements_id, submissions_id, NULL, char(0x274C), CASE judging_reaction WHEN 'trophy' THEN char(0x1F3C6) WHEN 'star' THEN char(0x2B50) WHEN 'hundred' THEN char(0x1F4AF) WHEN 'crown' THEN char(0x1F451) END, char(0x2705), rules, last_seen_message, created_by, created_at_ms FROM iq_competitions;

DROP TABLE iq_competitions;

ALTER TABLE iq_competitions_rebuilt RENAME TO iq_competitions;

CREATE UNIQUE INDEX iq_competitions_live ON iq_competitions (guild_id) WHERE status <> 'archived';

CREATE INDEX iq_submissions_unawarded ON iq_submissions (competition_id, message_id) WHERE awarded_by IS NULL;

COMMIT;

PRAGMA foreign_key_check;

PRAGMA foreign_keys = on;
