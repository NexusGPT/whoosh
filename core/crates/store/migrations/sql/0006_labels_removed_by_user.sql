-- 0006, contracts delta 11 (2026-11-08): the labels the owner took off a thread. A rule or the decision model never files
-- one of them on that thread again; the owner can, which lifts it. A JSON array of label ids on the thread row: no new
-- table (every table traces to a docs/DATA-MAP.md row).
ALTER TABLE thread ADD COLUMN labels_removed_json TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(labels_removed_json));
