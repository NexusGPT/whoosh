-- 0010, contracts delta 18 (2026-11-09): imported mail says where it came from. No new table: a source's imported count and its last run
-- are DERIVED from the message rows (COUNT and MAX(imported_at_ms) over import_source_id), stored once, never twice. The security-scoped
-- bookmark of the file or folder lives in the Keychain-backed bookmark store, not here.
--   message + source          'synced' (every row that existed) or 'imported' (a row the import crate wrote)
--            + import_source_id  the contract's `import_source_id(kind, location)`, e.g. 'apple-mail:~/Library/Mail'; NULL for synced mail
--            + imported_at_ms    when the run stored it; NULL for synced mail
-- A row that is imported has both; a synced row has neither (`imported_row_is_legal` in the contract, repeated by two triggers).
ALTER TABLE message ADD COLUMN source TEXT NOT NULL DEFAULT 'synced' CHECK (source IN ('synced','imported'));
ALTER TABLE message ADD COLUMN import_source_id TEXT CHECK (import_source_id IS NULL OR length(trim(import_source_id)) > 0);
ALTER TABLE message ADD COLUMN imported_at_ms INTEGER;
-- Rows the import crate wrote before this migration (their ids start 'imp:') get the honest marker 'earlier-import' and their own
-- received time as the import time (the run time was not kept): they stay one source in the counts.
UPDATE message SET source = 'imported', import_source_id = 'earlier-import', imported_at_ms = received_at_ms WHERE id LIKE 'imp:%';
CREATE INDEX message_by_import_source ON message (import_source_id) WHERE source = 'imported';
CREATE TRIGGER message_import_mark_is_whole_insert BEFORE INSERT ON message
WHEN NOT ((NEW.source = 'imported' AND NEW.import_source_id IS NOT NULL AND NEW.imported_at_ms IS NOT NULL)
       OR (NEW.source = 'synced' AND NEW.import_source_id IS NULL AND NEW.imported_at_ms IS NULL))
BEGIN SELECT RAISE(ABORT, 'imported mail names its source and its time; synced mail names neither'); END;
CREATE TRIGGER message_import_mark_is_whole_update BEFORE UPDATE OF source, import_source_id, imported_at_ms ON message
WHEN NOT ((NEW.source = 'imported' AND NEW.import_source_id IS NOT NULL AND NEW.imported_at_ms IS NOT NULL)
       OR (NEW.source = 'synced' AND NEW.import_source_id IS NULL AND NEW.imported_at_ms IS NULL))
BEGIN SELECT RAISE(ABORT, 'imported mail names its source and its time; synced mail names neither'); END;
