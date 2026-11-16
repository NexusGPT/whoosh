-- 0029 (lane fix-nesting, J2.6 "nests 'Supplier quotes' under a 'Work' label"): a smart folder may sit under a LABEL as well as under another
-- smart folder. ADDITIVE, ONE COLUMN (no table is added, so the table lists of the older migration tests stay exact).
--   smart_folder.parent_label_id   the label the folder sits under; NULL = none. At most one of parent_id (a smart folder) and parent_label_id
--                                  (a label) is set; the store's reads answer whichever is set as the contract's one SmartFolder.parent_id.
-- parent_id cannot hold a label id: 0001 gives it a foreign key to smart_folder(id). A deleted label sends its smart folders to the top level.
-- Reversal: 0029_smart_folder_label_parent.down.sql.
ALTER TABLE smart_folder ADD COLUMN parent_label_id TEXT REFERENCES label(id) ON DELETE SET NULL;
