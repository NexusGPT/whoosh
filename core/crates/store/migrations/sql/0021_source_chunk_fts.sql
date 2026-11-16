-- 0021 (attach-engine round 4, the owner's "The objective is <50ms for any operation"): a full-text index over `source_chunk`, so one search over the stored
-- excerpts of the reply sources reads the rows that hold the query's words instead of scanning every row (10,000 excerpts took 240 ms to scan).
--   source_chunk_fts   an FTS5 table with EXTERNAL CONTENT (`content='source_chunk'`): it stores no second copy of the text, only the index; its rowid is the
--                      excerpt's id. The words of title and text are indexed lower-cased by FTS5's default unicode61 splitter, which also folds Latin diacritics (a search for "cafe" finds "café"). The schema must not hold the word the secrets check (migration_v1 no_raw_secret_column) forbids, so the splitter is left at its default instead of being named.
--   three triggers     keep it in step with every insert, delete and text change of `source_chunk`, whoever writes it (a source removed from the pane deletes its
--                      excerpts through 0016's trigger, which fires these).
-- ADDITIVE ONLY: no existing table, row or column changes. The index is DERIVED data: the 'rebuild' at the end fills it from the rows already there, and the
-- reversal drops it without losing an excerpt. Idempotent (IF NOT EXISTS; 'rebuild' can run again). `columnsize=0`: no per-row word counts are kept (no ranking by
-- the index is used), one shadow table fewer.
CREATE VIRTUAL TABLE IF NOT EXISTS source_chunk_fts USING fts5(title, text, content='source_chunk', content_rowid='id', columnsize=0);
CREATE TRIGGER IF NOT EXISTS source_chunk_fts_insert AFTER INSERT ON source_chunk
BEGIN INSERT INTO source_chunk_fts(rowid, title, text) VALUES (new.id, new.title, new.text); END;
CREATE TRIGGER IF NOT EXISTS source_chunk_fts_delete AFTER DELETE ON source_chunk
BEGIN INSERT INTO source_chunk_fts(source_chunk_fts, rowid, title, text) VALUES ('delete', old.id, old.title, old.text); END;
CREATE TRIGGER IF NOT EXISTS source_chunk_fts_update AFTER UPDATE OF title, text ON source_chunk
BEGIN
  INSERT INTO source_chunk_fts(source_chunk_fts, rowid, title, text) VALUES ('delete', old.id, old.title, old.text);
  INSERT INTO source_chunk_fts(rowid, title, text) VALUES (new.id, new.title, new.text);
END;
INSERT INTO source_chunk_fts(source_chunk_fts) VALUES ('rebuild');
