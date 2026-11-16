-- 0007, contracts delta 12 (2026-11-09): Bcc recipients. On a message they exist only for the owner's own sent copies
-- (Message.bcc); on a draft they go out with it (Command::Send). A JSON array of addresses beside the cc_json each
-- table already has: no new table.
ALTER TABLE message ADD COLUMN bcc_json TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(bcc_json));
ALTER TABLE draft ADD COLUMN bcc_json TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(bcc_json));
-- And the decision model's page entry that filed a label (contracts delta 12, decided_entry_id beside decided_by; NULL for
-- the owner, a rule, the default, or a filing written before it).
ALTER TABLE label_assignment ADD COLUMN decided_entry_id TEXT;
