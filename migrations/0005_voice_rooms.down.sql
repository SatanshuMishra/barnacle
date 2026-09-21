.bail on

BEGIN;

DROP INDEX vc_rooms_numbered;

DROP TABLE vc_rooms;

DROP TABLE vc_hubs;

DELETE FROM schema_migrations WHERE name = '0005_voice_rooms';

COMMIT;
