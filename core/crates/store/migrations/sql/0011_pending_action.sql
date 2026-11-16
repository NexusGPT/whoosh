-- 0011, engine-act (2026-11-10): the server write-back of a mail action, kept until the server took it. One row per message the
-- action touched, written in the SAME transaction as the local change (actions::WriteBackLedger), so a crash between the two never
-- keeps the local change and loses the server's. A worker applies the rows over IMAP, deletes a row the server took, and on a
-- refusal or no network keeps it with the server's words and the time of the next try (the wait doubles, SYNC_BACKOFF_MAX_MS caps it).
--   account_id  the account whose server is asked (a cross-account move: the SOURCE account)
--   message_id  the local message (NULL when it is gone; the row still names its folder and UID)
--   folder/uid  where the message sat on the server BEFORE the action (the UID is NULL when the sync never stored one)
--   kind        what the server is asked: set_archived, set_read, label, move, move_to_folder
--   payload_json  the kind's arguments, e.g. {"archived":true}, {"label":"Starred","applied":true}, {"to_account_id":"a2"}
--   tries / next_at_ms / last_error  how often it was tried, when it is tried next, the server's words of the last refusal
CREATE TABLE pending_action (
    id           INTEGER PRIMARY KEY,
    account_id   TEXT NOT NULL REFERENCES account(id) ON DELETE CASCADE,
    message_id   TEXT,
    folder       TEXT NOT NULL CHECK (length(folder) > 0),
    uid          INTEGER CHECK (uid IS NULL OR uid > 0),
    kind         TEXT NOT NULL CHECK (kind IN ('set_archived','set_read','label','move','move_to_folder')),
    payload_json TEXT NOT NULL CHECK (json_valid(payload_json)),
    created_ms   INTEGER NOT NULL,
    tries        INTEGER NOT NULL DEFAULT 0 CHECK (tries >= 0),
    next_at_ms   INTEGER NOT NULL,
    last_error   TEXT
);
CREATE INDEX pending_action_due ON pending_action (next_at_ms, id);
