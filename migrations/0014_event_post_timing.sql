.bail on

BEGIN;

INSERT INTO schema_migrations (name) VALUES ('0014_event_post_timing');

ALTER TABLE cb_seasons ADD COLUMN post_lead_days INTEGER NOT NULL DEFAULT 1 CHECK (post_lead_days BETWEEN 1 AND 7);

ALTER TABLE cb_seasons ADD COLUMN clean_up_minutes INTEGER NOT NULL DEFAULT 30 CHECK (clean_up_minutes BETWEEN 0 AND 240 AND clean_up_minutes % 30 = 0);

COMMIT;
