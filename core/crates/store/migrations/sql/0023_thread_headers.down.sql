-- Reversal of 0023: back to 0022's exact schema. DATA LOST: every stored Message-ID and References chain (the fill-in pass of a later
-- 0023 asks the server again; until then a reply asks the server, as before 0023).
DROP INDEX IF EXISTS message_headers_unknown;
ALTER TABLE message DROP COLUMN rfc_references_json;
ALTER TABLE message DROP COLUMN rfc_message_id;
