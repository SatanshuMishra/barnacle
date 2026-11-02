.bail on

BEGIN;

INSERT INTO schema_migrations (name) VALUES ('0009_iq_competition');

CREATE TABLE iq_competitions (
    id INTEGER PRIMARY KEY,
    guild_id INTEGER NOT NULL,
    name TEXT NOT NULL CHECK (length(name) BETWEEN 1 AND 100),
    first_day TEXT NOT NULL,
    last_day TEXT NOT NULL,
    time_zone TEXT NOT NULL CHECK (length(time_zone) BETWEEN 1 AND 64),
    status TEXT NOT NULL CHECK (status IN ('running', 'stopped', 'completed', 'archived')),
    category_id INTEGER NOT NULL,
    announcements_id INTEGER NOT NULL,
    submissions_id INTEGER NOT NULL,
    judging_reaction TEXT NOT NULL CHECK (judging_reaction IN ('trophy', 'star', 'hundred', 'crown')),
    rules TEXT NOT NULL CHECK (length(rules) <= 4000),
    last_seen_message INTEGER CHECK (last_seen_message IS NULL OR last_seen_message >= 0),
    created_by INTEGER NOT NULL,
    created_at_ms INTEGER NOT NULL,
    CHECK (first_day <= last_day)
) STRICT;

CREATE UNIQUE INDEX iq_competitions_live ON iq_competitions (guild_id) WHERE status <> 'archived';

CREATE TABLE iq_criteria (
    competition_id INTEGER NOT NULL REFERENCES iq_competitions (id),
    position INTEGER NOT NULL CHECK (position BETWEEN 0 AND 24),
    name TEXT NOT NULL CHECK (length(name) BETWEEN 1 AND 50),
    description TEXT NOT NULL CHECK (length(description) <= 300),
    points INTEGER NOT NULL CHECK (points BETWEEN 1 AND 1000),
    PRIMARY KEY (competition_id, position)
) STRICT;

CREATE UNIQUE INDEX iq_criteria_name ON iq_criteria (competition_id, name COLLATE NOCASE);

CREATE TABLE iq_target_roles (
    competition_id INTEGER NOT NULL REFERENCES iq_competitions (id),
    role_id INTEGER NOT NULL,
    PRIMARY KEY (competition_id, role_id)
) STRICT;

CREATE TABLE iq_judges (
    competition_id INTEGER NOT NULL REFERENCES iq_competitions (id),
    user_id INTEGER NOT NULL,
    PRIMARY KEY (competition_id, user_id)
) STRICT;

CREATE TABLE iq_announcements (
    competition_id INTEGER NOT NULL REFERENCES iq_competitions (id),
    position INTEGER NOT NULL CHECK (position >= 0),
    message_id INTEGER NOT NULL,
    PRIMARY KEY (competition_id, position)
) STRICT;

CREATE TABLE iq_submissions (
    message_id INTEGER PRIMARY KEY,
    competition_id INTEGER NOT NULL REFERENCES iq_competitions (id),
    user_id INTEGER NOT NULL,
    criterion TEXT NOT NULL CHECK (length(criterion) BETWEEN 1 AND 50),
    points INTEGER NOT NULL CHECK (points BETWEEN 1 AND 1000),
    posted_at_ms INTEGER NOT NULL,
    awarded_by INTEGER,
    awarded_at_ms INTEGER,
    CHECK ((awarded_by IS NULL) = (awarded_at_ms IS NULL))
) STRICT;

CREATE TABLE iq_adjustments (
    id INTEGER PRIMARY KEY,
    competition_id INTEGER NOT NULL REFERENCES iq_competitions (id),
    user_id INTEGER NOT NULL,
    points INTEGER NOT NULL CHECK (points BETWEEN -1000 AND 1000 AND points <> 0),
    reason TEXT CHECK (reason IS NULL OR length(reason) <= 200),
    judge_id INTEGER NOT NULL,
    made_at_ms INTEGER NOT NULL
) STRICT;

CREATE TABLE iq_servers (
    guild_id INTEGER PRIMARY KEY,
    judge_role_id INTEGER NOT NULL
) STRICT;

COMMIT;
