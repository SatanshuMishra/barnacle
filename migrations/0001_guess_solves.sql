BEGIN;

CREATE TABLE IF NOT EXISTS guess_solves (
    id INTEGER PRIMARY KEY,
    guild_id INTEGER NOT NULL,
    user_id INTEGER NOT NULL,
    ship_index TEXT NOT NULL,
    elapsed_ms INTEGER NOT NULL CHECK (elapsed_ms >= 0),
    solved_at_ms INTEGER NOT NULL
) STRICT;

CREATE INDEX IF NOT EXISTS guess_solves_by_player
    ON guess_solves (guild_id, user_id, elapsed_ms);

COMMIT;
