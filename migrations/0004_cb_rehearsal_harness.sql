.bail on

BEGIN;

INSERT INTO schema_migrations (name) VALUES ('0004_cb_rehearsal_harness');

ALTER TABLE cb_seasons ADD COLUMN every_day INTEGER NOT NULL DEFAULT 0 CHECK (every_day IN (0, 1));

CREATE TABLE cb_rehearsal_clock (
    guild_id INTEGER PRIMARY KEY,
    offset_s INTEGER NOT NULL
) STRICT;

COMMIT;
