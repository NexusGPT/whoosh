-- 0022 (lane flow3-core, SPEC A140 part 3): the AI's NO, kept. One row per (message, label): the label AI ruled on the message (step 1 of the label
-- had picked it) and did not accept it. A "yes" needs no row: the filing (label_assignment) records it. Read by the counts' `waiting_for_ai` (a picked
-- mail with a filing or a row here is not waiting) and by the ask-once rule (a mail with a row here is not asked again for that label when a fact
-- flip makes it pass step 1 again, after a restart too). Cleared for a label by its Re-run and by a change of its description or its AI-reads
-- ticks (the store's code); for a message or a label by their deletion (the foreign keys). Internal to the store: it never crosses to Swift.
-- Additive; no existing table changes. Reversal: 0022_ai_label_no.down.sql.
CREATE TABLE IF NOT EXISTS ai_label_no (
  message_id TEXT NOT NULL REFERENCES message(id) ON DELETE CASCADE,
  label_id TEXT NOT NULL REFERENCES label(id) ON DELETE CASCADE,
  decided_by TEXT NOT NULL CHECK (decided_by IN ('laya', 'jev')),
  decided_at_ms INTEGER NOT NULL,
  PRIMARY KEY (message_id, label_id)
);
CREATE INDEX IF NOT EXISTS ai_label_no_by_label ON ai_label_no (label_id);
