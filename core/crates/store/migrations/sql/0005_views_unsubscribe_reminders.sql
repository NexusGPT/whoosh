-- 0005, contracts delta 10 (2026-11-08): the state the delta's commands write in the real core.
--   message  + starred, muted (the Starred and Muted views, the starred filter; delta 8's Star and Mute had no column)
--            + list_unsubscribe_json, list_unsubscribe_one_click (the List-Unsubscribe header and RFC 8058's one-click offer)
--            + unsubscribed_at_ms (Unsubscribe was done for this message's list)
--   draft    + remind_if_no_reply_at_ms (RemindIfNoReply set while the message is still a draft)
--   reminder + if_no_reply, fired_at_ms (after the send, the reminder rides the sent message: it fires at when_ms when no
--              reply arrived in the thread). No new table: every table traces to a docs/DATA-MAP.md row.
ALTER TABLE message ADD COLUMN starred INTEGER NOT NULL DEFAULT 0 CHECK (starred IN (0,1));
ALTER TABLE message ADD COLUMN muted INTEGER NOT NULL DEFAULT 0 CHECK (muted IN (0,1));
ALTER TABLE message ADD COLUMN list_unsubscribe_json TEXT CHECK (list_unsubscribe_json IS NULL OR json_valid(list_unsubscribe_json));
ALTER TABLE message ADD COLUMN list_unsubscribe_one_click INTEGER NOT NULL DEFAULT 0 CHECK (list_unsubscribe_one_click IN (0,1));
ALTER TABLE message ADD COLUMN unsubscribed_at_ms INTEGER;
ALTER TABLE draft ADD COLUMN remind_if_no_reply_at_ms INTEGER;
ALTER TABLE reminder ADD COLUMN if_no_reply INTEGER NOT NULL DEFAULT 0 CHECK (if_no_reply IN (0,1));
ALTER TABLE reminder ADD COLUMN fired_at_ms INTEGER;
CREATE INDEX message_starred ON message (starred) WHERE starred = 1;
CREATE INDEX message_muted ON message (muted) WHERE muted = 1;
CREATE INDEX reminder_if_no_reply_due ON reminder (when_ms) WHERE if_no_reply = 1 AND fired_at_ms IS NULL;
