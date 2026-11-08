.bail on

BEGIN;

DROP TABLE iq_board_posts;

DROP TABLE iq_rejections;

DROP TABLE iq_submission_partners;

ALTER TABLE iq_criteria DROP COLUMN kind;

DELETE FROM schema_migrations WHERE name = '0013_iq_players_and_rejections';

COMMIT;
