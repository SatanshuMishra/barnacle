.bail on

BEGIN;

INSERT INTO schema_migrations (name) VALUES ('0010_organiser_roles');

CREATE TABLE organiser_roles (
    guild_id INTEGER NOT NULL,
    role_id INTEGER NOT NULL,
    PRIMARY KEY (guild_id, role_id)
) STRICT;

COMMIT;
