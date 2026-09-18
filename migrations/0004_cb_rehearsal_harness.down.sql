.bail on

BEGIN;

DROP TABLE cb_rehearsal_clock;

ALTER TABLE cb_seasons DROP COLUMN every_day;

DELETE FROM schema_migrations WHERE name = '0004_cb_rehearsal_harness';

COMMIT;
