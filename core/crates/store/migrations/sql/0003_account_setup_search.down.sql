-- Reversal of 0003_account_setup_search. Written first. Drops only what 0003 added.
-- Data lost on reversal: the server config of each account (re-detected by the next Add account), the detection cache,
-- the Gmail label-folder rows (rebuilt by the next sync) and the text read out of attachments (rebuilt by re-indexing).
DROP INDEX IF EXISTS message_location_by_message;
DROP TABLE IF EXISTS message_location;
DROP TABLE IF EXISTS attachment_content;
DROP TABLE IF EXISTS detection_cache;
ALTER TABLE account DROP COLUMN detection_source;
ALTER TABLE account DROP COLUMN auth_method;
ALTER TABLE account DROP COLUMN smtp_username;
ALTER TABLE account DROP COLUMN username;
ALTER TABLE account DROP COLUMN smtp_security;
ALTER TABLE account DROP COLUMN smtp_port;
ALTER TABLE account DROP COLUMN smtp_host;
ALTER TABLE account DROP COLUMN imap_security;
ALTER TABLE account DROP COLUMN imap_port;
ALTER TABLE account DROP COLUMN imap_host;
