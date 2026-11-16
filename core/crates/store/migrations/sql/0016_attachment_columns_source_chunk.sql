-- 0016 (contracts delta 23): the four columns of a received attachment, and the excerpts of files and events. docs/lanes/body-r2.md "Where the schema and the contract
-- fall short" 1 and 2; docs/lanes/reply-engine.md gap 6. Reversal: 0016_*.down.sql (written first).
--   attachment + part_path, transfer_encoding, content_id, inline: body-r2 wrote them as JSON in `remote_id` ({"part":"2","enc":"base64","cid":"x@y","inline":true});
--                every such row is MOVED to the columns and its remote_id is set to NULL; any other remote_id (a server's own id) is left alone
--   source_chunk  one piece of a file of a source folder or of a calendar event, beside `chunk` (which is not touched: its rows, its ids and `chunk_vector_key` stay)
ALTER TABLE attachment ADD COLUMN part_path TEXT CHECK (part_path IS NULL OR length(trim(part_path)) > 0);
ALTER TABLE attachment ADD COLUMN transfer_encoding TEXT CHECK (transfer_encoding IS NULL OR length(trim(transfer_encoding)) > 0);
ALTER TABLE attachment ADD COLUMN content_id TEXT CHECK (content_id IS NULL OR length(trim(content_id)) > 0);
ALTER TABLE attachment ADD COLUMN inline INTEGER NOT NULL DEFAULT 0 CHECK (inline IN (0,1));
UPDATE attachment SET
  part_path = nullif(trim(coalesce(json_extract(remote_id, '$.part'), '')), ''),
  transfer_encoding = nullif(trim(coalesce(json_extract(remote_id, '$.enc'), '')), ''),
  content_id = nullif(trim(coalesce(json_extract(remote_id, '$.cid'), '')), ''),
  inline = CASE WHEN json_extract(remote_id, '$.inline') IN (1, '1', 'true') THEN 1 ELSE 0 END,
  remote_id = NULL
WHERE remote_id IS NOT NULL AND json_valid(remote_id) AND json_type(remote_id) = 'object' AND json_extract(remote_id, '$.part') IS NOT NULL;

CREATE TABLE source_chunk (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  source_key TEXT NOT NULL CHECK (source_key LIKE 'local' || char(31) || '_%' OR source_key LIKE 'calendar' || char(31) || '_%'),
  item_key TEXT NOT NULL CHECK (length(trim(item_key)) > 0),
  chunk_no INTEGER NOT NULL CHECK (chunk_no >= 0),
  date_ms INTEGER,
  title TEXT NOT NULL,
  text TEXT NOT NULL CHECK (length(trim(text)) > 0),
  embedded_at_ms INTEGER,
  UNIQUE (source_key, item_key, chunk_no)
);
CREATE INDEX source_chunk_not_embedded ON source_chunk (id) WHERE embedded_at_ms IS NULL;
-- a removed source takes its excerpts (the files on the disk are not touched)
CREATE TRIGGER source_chunk_source_gone AFTER DELETE ON reply_source
BEGIN DELETE FROM source_chunk WHERE source_key = OLD.source_key; END;
