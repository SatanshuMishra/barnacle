.bail on

BEGIN;

DROP TABLE iq_servers;

DROP TABLE iq_adjustments;

DROP TABLE iq_submissions;

DROP TABLE iq_announcements;

DROP TABLE iq_judges;

DROP TABLE iq_target_roles;

DROP TABLE iq_criteria;

DROP TABLE iq_competitions;

DELETE FROM schema_migrations WHERE name = '0009_iq_competition';

COMMIT;
