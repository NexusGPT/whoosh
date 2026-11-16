-- 0030 (lane fix-aicore): the reader's thread summary, made by the owner's writing model and kept on this Mac. ADDITIVE, COLUMNS ONLY (no table is
-- added, so the table lists of the older migration tests stay exact; 0029 belongs to another lane).
--   thread.summary_text   the summary of the thread, as the writing model wrote it; NULL = none made yet
--   thread.summary_of     the id of the newest message of the thread when the summary was made; a newer message makes it stale (made again)
-- Reversal: 0030_thread_summary.down.sql.
ALTER TABLE thread ADD COLUMN summary_text TEXT;
ALTER TABLE thread ADD COLUMN summary_of TEXT;
