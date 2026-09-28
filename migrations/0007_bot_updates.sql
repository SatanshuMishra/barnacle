.bail on

BEGIN;

INSERT INTO schema_migrations (name) VALUES ('0007_bot_updates');

CREATE TABLE bot_updates (
    guild_id INTEGER PRIMARY KEY,
    channel_id INTEGER NOT NULL,
    ping_id INTEGER,
    last_version TEXT CHECK (last_version IS NULL OR length(last_version) BETWEEN 5 AND 32)
) STRICT;

COMMIT;
