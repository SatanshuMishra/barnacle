.bail on

BEGIN;

DROP TABLE member_zones;

DELETE FROM schema_migrations WHERE name = '0011_member_zones';

COMMIT;
