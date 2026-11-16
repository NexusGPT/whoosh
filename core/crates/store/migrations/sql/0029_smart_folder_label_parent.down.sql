-- Reversal of 0029: back to 0028's exact schema. DATA LOST: a smart folder under a label goes back to the top level.
UPDATE smart_folder SET parent_label_id = NULL;
ALTER TABLE smart_folder DROP COLUMN parent_label_id;
