-- Reversal of 0020: back to 0019's exact schema. Drops ONLY what 0020 added. DATA LOST: the item stamps (the next walk reads every item again and writes them
-- anew); `source_chunk` and every other table are untouched.
DROP TRIGGER IF EXISTS source_item_source_gone;
DROP TABLE IF EXISTS source_item;
