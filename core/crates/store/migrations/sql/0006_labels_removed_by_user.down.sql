-- Reversal of 0006: back to 0005's exact schema. The owner's removals are forgotten.
ALTER TABLE thread DROP COLUMN labels_removed_json;
