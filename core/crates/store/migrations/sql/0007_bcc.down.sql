-- Reversal of 0007: back to 0006's exact schema. Data lost on reversal: every Bcc recipient on messages and drafts, and
-- which model entry filed each label.
ALTER TABLE label_assignment DROP COLUMN decided_entry_id;
ALTER TABLE draft DROP COLUMN bcc_json;
ALTER TABLE message DROP COLUMN bcc_json;
