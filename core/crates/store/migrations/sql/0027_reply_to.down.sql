-- Reversal of 0027: back to 0023's schema (the version before it in this tree). DATA LOST: every stored Reply-To (a reply goes to the
-- sender again until the headers are read again).
ALTER TABLE message DROP COLUMN reply_to_json;
