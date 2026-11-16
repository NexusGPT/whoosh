-- 0018 (contracts delta 24): a mail folder removed on the server stays as a row, switched off, with its last count (replies-settings round 4).
--   reply_source + gone (0/1), owner_enabled (the switch the owner last chose, kept apart from the forced-off `enabled`; NULL while the folder is present)
-- Legal shapes are the contract's `source_presence_is_legal`: a gone row is off; owner_enabled exists only while gone. progress_state is untouched (a gone row's progress is
-- DERIVED as `Gone` on read, so the CHECK on progress_state needs no rebuild). Existing rows: not gone. Reversal: 0018_*.down.sql.
ALTER TABLE reply_source ADD COLUMN gone INTEGER NOT NULL DEFAULT 0 CHECK (gone IN (0,1));
ALTER TABLE reply_source ADD COLUMN owner_enabled INTEGER CHECK (owner_enabled IS NULL OR owner_enabled IN (0,1));
CREATE TRIGGER reply_source_presence_legal_insert BEFORE INSERT ON reply_source
WHEN (NEW.gone = 1 AND NEW.enabled = 1) OR (NEW.gone = 0 AND NEW.owner_enabled IS NOT NULL)
BEGIN SELECT RAISE(ABORT, 'a gone source is off, and the owner''s remembered choice exists only while it is gone'); END;
CREATE TRIGGER reply_source_presence_legal_update BEFORE UPDATE OF enabled, gone, owner_enabled ON reply_source
WHEN (NEW.gone = 1 AND NEW.enabled = 1) OR (NEW.gone = 0 AND NEW.owner_enabled IS NOT NULL)
BEGIN SELECT RAISE(ABORT, 'a gone source is off, and the owner''s remembered choice exists only while it is gone'); END;
