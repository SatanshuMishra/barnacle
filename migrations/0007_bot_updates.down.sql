.bail on

BEGIN;

DROP TABLE bot_updates;

DELETE FROM schema_migrations WHERE name = '0007_bot_updates';

COMMIT;
