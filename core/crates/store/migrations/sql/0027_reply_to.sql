-- 0027 (lane real-replyto): each mail's Reply-To header kept on this Mac, so a reply goes to the address the sender asked replies to go to
-- (a mailing list, a no-reply sender) and never waits on the mail server (owner: "Also it should store locally"; "<50ms for any operation").
--   message.reply_to_json: every address of the header with its display name, in header order, a JSON array of {"name": ..., "address": ...}
--     (name '' when the header gave none). NULL = not read yet (a row stored before 0027): it reads as "no Reply-To" until its headers are
--     read again; '[]' = read, and the mail has no Reply-To. Checked as JSON when not NULL.
-- Additive: one nullable column; every existing row reads NULL. Reversal: 0027_reply_to.down.sql.
ALTER TABLE message ADD COLUMN reply_to_json TEXT CHECK (reply_to_json IS NULL OR json_valid(reply_to_json));
