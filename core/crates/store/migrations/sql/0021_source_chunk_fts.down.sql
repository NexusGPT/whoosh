-- Reversal of 0021: back to 0020's exact schema. Drops ONLY what 0021 added (the triggers, the FTS5 table and its shadow tables). No excerpt is lost: the index
-- is derived from `source_chunk`, which is untouched; 0021 on again rebuilds it.
DROP TRIGGER IF EXISTS source_chunk_fts_update;
DROP TRIGGER IF EXISTS source_chunk_fts_delete;
DROP TRIGGER IF EXISTS source_chunk_fts_insert;
DROP TABLE IF EXISTS source_chunk_fts;
