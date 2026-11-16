-- 0020 (attach-engine, the owner's "it should store locally"): what the walks of the reply sources remember of each ITEM beside its excerpts in
-- `source_chunk` (0016): the item's stamp. The folder walk skips a file whose version ("<size>:<modified ms>") is the one stored; the calendar walk skips an
-- event whose version (a hash of its words) is; an item that is known but holds no text (a picture, a scan) has no excerpt row at all, so only this row says
-- "read already". ADDITIVE ONLY: no existing table, row or column changes; nothing is moved or deleted. Idempotent on a database that has it (IF NOT EXISTS).
-- A removed source takes its items, as 0016's trigger takes its excerpts. Reversal: 0020_source_item.down.sql (drops only what this file adds).
CREATE TABLE IF NOT EXISTS source_item (
  source_key TEXT NOT NULL CHECK (source_key LIKE 'local' || char(31) || '_%' OR source_key LIKE 'calendar' || char(31) || '_%'),
  item_key TEXT NOT NULL CHECK (length(trim(item_key)) > 0),
  version TEXT NOT NULL,
  excerpt_count INTEGER NOT NULL CHECK (excerpt_count >= 0),
  PRIMARY KEY (source_key, item_key)
);
CREATE TRIGGER IF NOT EXISTS source_item_source_gone AFTER DELETE ON reply_source
BEGIN DELETE FROM source_item WHERE source_key = OLD.source_key; END;
