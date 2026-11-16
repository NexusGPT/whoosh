-- 0019 (contracts delta 26, body-wire): the mail content engine's result for a fetched body, kept beside it so the reader never runs the engine.
--   message_prepared: one row per message whose body the fetch prepared: the engine's result as JSON (segments, fold_from, layout,
--   layout_reasons, display_html, written_html, warnings), written in the same transaction as message.body_text and body_html. A table of its
--   own, not a column of message: no existing table changes shape (the earlier migrations' tests pin message's columns). A body stored
--   before 0019 has no row (the read gives empty fields, never an error); the row goes with its message (ON DELETE CASCADE). Reversal: 0019_*.down.sql.
CREATE TABLE message_prepared (
  message_id TEXT PRIMARY KEY NOT NULL REFERENCES message(id) ON DELETE CASCADE,
  prepared TEXT NOT NULL
);
