.bail on

BEGIN;

DROP TABLE organiser_roles;

DELETE FROM schema_migrations WHERE name = '0010_organiser_roles';

COMMIT;
