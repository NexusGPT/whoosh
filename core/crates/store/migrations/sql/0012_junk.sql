-- 0012, junk-r2 (2026-11-10): the local spam filter's state (core/crates/junk), as core-junk's "### Store text" names it. Additive only:
-- three new tables and two nullable message columns; no existing row changes.
--   junk_model      the learned word table (junk::Model::to_bytes), one per profile (id 1, like decision_model), with the message
--                   counts per class it was learned from
--   junk_bootstrap  one row per account once its first-run training (the server's Junk folder as junk, its inbox as not junk) ran
--   junk_learned    the owner's lesson per message (MarkSpam = 'junk', the not-junk action = 'not_junk'); written BEFORE the model
--                   trains, so a repeated command never trains twice
--   message.junk_score  the filter's score of a live arrival in [0, 1] (NULL: never scored: history, the first screen, a reset page)
--   message.junk_state  'suspect' (kept, marked for review) or 'moved' (the filter moved it to Junk); NULL otherwise
CREATE TABLE junk_model (
  id INTEGER PRIMARY KEY NOT NULL CHECK (id = 1),
  model_bytes BLOB NOT NULL,
  trained_junk INTEGER NOT NULL CHECK (trained_junk >= 0),
  trained_good INTEGER NOT NULL CHECK (trained_good >= 0),
  updated_at_ms INTEGER NOT NULL
);
CREATE TABLE junk_bootstrap (
  account_id TEXT PRIMARY KEY NOT NULL REFERENCES account(id) ON DELETE CASCADE,
  done_at_ms INTEGER NOT NULL,
  junk_learned INTEGER NOT NULL CHECK (junk_learned >= 0),
  good_learned INTEGER NOT NULL CHECK (good_learned >= 0)
);
CREATE TABLE junk_learned (
  message_id TEXT PRIMARY KEY NOT NULL REFERENCES message(id) ON DELETE CASCADE,
  label TEXT NOT NULL CHECK (label IN ('junk','not_junk'))
);
ALTER TABLE message ADD COLUMN junk_score REAL CHECK (junk_score IS NULL OR (junk_score >= 0 AND junk_score <= 1));
ALTER TABLE message ADD COLUMN junk_state TEXT CHECK (junk_state IS NULL OR junk_state IN ('suspect','moved'));
