BEGIN;

CREATE TABLE IF NOT EXISTS cb_seasons (
    id INTEGER PRIMARY KEY,
    guild_id INTEGER NOT NULL,
    channel_id INTEGER NOT NULL,
    number INTEGER NOT NULL CHECK (number > 0),
    codename TEXT,
    first_day TEXT NOT NULL,
    last_day TEXT NOT NULL,
    created_by INTEGER NOT NULL,
    created_at_ms INTEGER NOT NULL,
    UNIQUE (guild_id, number),
    CHECK (first_day <= last_day)
) STRICT;

CREATE TABLE IF NOT EXISTS cb_posts (
    season_id INTEGER NOT NULL REFERENCES cb_seasons (id),
    night TEXT NOT NULL,
    message_id INTEGER NOT NULL,
    state TEXT NOT NULL CHECK (state IN ('open', 'closed', 'removed')),
    posted_at_ms INTEGER NOT NULL,
    PRIMARY KEY (season_id, night)
) STRICT;

CREATE TABLE IF NOT EXISTS cb_marks (
    season_id INTEGER NOT NULL,
    night TEXT NOT NULL,
    user_id INTEGER NOT NULL,
    hour INTEGER NOT NULL CHECK (hour BETWEEN 1 AND 4),
    attending INTEGER NOT NULL CHECK (attending IN (0, 1)),
    answered_at_ms INTEGER NOT NULL,
    changed_at_ms INTEGER NOT NULL,
    PRIMARY KEY (season_id, night, user_id, hour),
    FOREIGN KEY (season_id, night) REFERENCES cb_posts (season_id, night)
) STRICT;

COMMIT;
