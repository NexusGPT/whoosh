-- Reversal of 0010: back to 0009's exact schema objects. Written first. Data lost on reversal: which messages were imported and from
-- which source (the messages themselves stay, as ordinary rows; Settings > Accounts can no longer count or re-run per source).
DROP TRIGGER IF EXISTS message_import_mark_is_whole_update;
DROP TRIGGER IF EXISTS message_import_mark_is_whole_insert;
DROP INDEX IF EXISTS message_by_import_source;
ALTER TABLE message DROP COLUMN imported_at_ms;
ALTER TABLE message DROP COLUMN import_source_id;
ALTER TABLE message DROP COLUMN source;
