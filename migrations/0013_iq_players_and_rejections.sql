.bail on

BEGIN;

INSERT INTO schema_migrations (name) VALUES ('0013_iq_players_and_rejections');

ALTER TABLE iq_criteria ADD COLUMN kind TEXT NOT NULL DEFAULT 'single' CHECK (kind IN ('single', 'multi'));

CREATE TABLE iq_submission_partners (
    message_id INTEGER NOT NULL REFERENCES iq_submissions (message_id) ON DELETE CASCADE,
    user_id INTEGER NOT NULL,
    PRIMARY KEY (message_id, user_id)
) STRICT;

CREATE TABLE iq_rejections (
    message_id INTEGER PRIMARY KEY,
    competition_id INTEGER NOT NULL REFERENCES iq_competitions (id),
    user_id INTEGER NOT NULL,
    reason TEXT NOT NULL CHECK (length(reason) BETWEEN 1 AND 200),
    rejected_by INTEGER NOT NULL,
    rejected_at_ms INTEGER NOT NULL
) STRICT;

CREATE TABLE iq_board_posts (
    competition_id INTEGER PRIMARY KEY REFERENCES iq_competitions (id),
    mark INTEGER NOT NULL CHECK (mark >= 0),
    digest TEXT NOT NULL DEFAULT '',
    message_id INTEGER
) STRICT;

COMMIT;
