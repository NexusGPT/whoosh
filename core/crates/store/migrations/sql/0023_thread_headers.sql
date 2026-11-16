-- 0023 (lane reply-headers, SPEC A144): each mail's threading headers kept on this Mac, so a reply is threaded from the store and never
-- waits on the mail server (owner: "Also it should store locally"; "<50ms for any operation").
--   message.rfc_message_id: the mail's own Message-ID with its angle brackets. NULL = not known yet (a row stored before 0023, or a fetch that
--     could not read the header); the fill-in pass after a folder sync asks the server for it. '' (empty) = the server was asked and gave no
--     Message-ID for the mail's UID (gone from the folder, or a mail with no Message-ID): the pass does not ask again; a reply never picks it.
--   message.rfc_references_json: the mail's chain, oldest first (its References ids, else its In-Reply-To), a JSON array of ids with their
--     angle brackets. NULL = not known yet; '[]' = known and empty. Checked as JSON when not NULL.
-- message_headers_unknown: the fill-in pass reads only the rows still unknown, newest first, per account and folder (a partial index: the
-- rows already known cost nothing).
-- Additive: two nullable columns and one index; every existing row reads NULL (unknown). Reversal: 0023_thread_headers.down.sql.
ALTER TABLE message ADD COLUMN rfc_message_id TEXT;
ALTER TABLE message ADD COLUMN rfc_references_json TEXT CHECK (rfc_references_json IS NULL OR json_valid(rfc_references_json));
CREATE INDEX IF NOT EXISTS message_headers_unknown ON message (account_id, folder, received_at_ms DESC) WHERE rfc_message_id IS NULL;
