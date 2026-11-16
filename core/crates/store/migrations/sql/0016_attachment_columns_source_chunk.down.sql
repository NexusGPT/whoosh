-- Reversal of 0016: back to 0015's exact schema. Written first. The four attachment columns go back into `remote_id` as the JSON body-r2 wrote ({"part","enc","cid","inline"}),
-- so nothing a reader needs is lost. DATA LOST: every source chunk (the indexer cuts them again) and their vectors' meaning (the meaning index must drop keys at or above 2^62).
UPDATE attachment SET remote_id = json_object('part', part_path, 'enc', transfer_encoding, 'cid', content_id, 'inline', json(CASE inline WHEN 1 THEN 'true' ELSE 'false' END))
  WHERE part_path IS NOT NULL AND remote_id IS NULL;
DROP TRIGGER IF EXISTS source_chunk_source_gone;
DROP TABLE IF EXISTS source_chunk;
ALTER TABLE attachment DROP COLUMN inline;
ALTER TABLE attachment DROP COLUMN content_id;
ALTER TABLE attachment DROP COLUMN transfer_encoding;
ALTER TABLE attachment DROP COLUMN part_path;
