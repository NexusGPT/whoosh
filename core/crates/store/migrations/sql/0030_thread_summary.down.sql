-- Reversal of 0030: back to the schema before it. DATA LOST: every kept thread summary (each is made again on the next open).
ALTER TABLE thread DROP COLUMN summary_of;
ALTER TABLE thread DROP COLUMN summary_text;
