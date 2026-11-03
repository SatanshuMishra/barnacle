.bail on

BEGIN;

INSERT INTO schema_migrations (name) VALUES ('0011_member_zones');

CREATE TABLE member_zones (
    user_id INTEGER PRIMARY KEY,
    time_zone TEXT NOT NULL CHECK (length(time_zone) BETWEEN 1 AND 64)
) STRICT;

COMMIT;
