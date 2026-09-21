.bail on

BEGIN;

INSERT INTO schema_migrations (name) VALUES ('0005_voice_rooms');

CREATE TABLE vc_hubs (
    channel_id INTEGER PRIMARY KEY,
    guild_id INTEGER NOT NULL,
    room_name TEXT NOT NULL CHECK (length(room_name) BETWEEN 1 AND 90),
    created_by INTEGER NOT NULL,
    created_at_ms INTEGER NOT NULL
) STRICT;

CREATE TABLE vc_rooms (
    channel_id INTEGER PRIMARY KEY,
    guild_id INTEGER NOT NULL,
    hub_id INTEGER NOT NULL,
    room_name TEXT NOT NULL,
    number INTEGER NOT NULL CHECK (number >= 1),
    owner_id INTEGER NOT NULL,
    created_at_ms INTEGER NOT NULL,
    empty_since_ms INTEGER
) STRICT;

CREATE UNIQUE INDEX vc_rooms_numbered ON vc_rooms (guild_id, room_name, number);

COMMIT;
