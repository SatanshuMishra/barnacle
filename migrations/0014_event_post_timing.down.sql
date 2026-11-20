.bail on

BEGIN;

ALTER TABLE cb_seasons DROP COLUMN clean_up_minutes;

ALTER TABLE cb_seasons DROP COLUMN post_lead_days;

DELETE FROM schema_migrations WHERE name = '0014_event_post_timing';

COMMIT;
