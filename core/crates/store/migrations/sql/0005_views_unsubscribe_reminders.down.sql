-- Reversal of 0005: back to 0004's exact schema objects. Starred, muted, the list headers and the reply reminders are lost.
DROP INDEX IF EXISTS reminder_if_no_reply_due;
DROP INDEX IF EXISTS message_muted;
DROP INDEX IF EXISTS message_starred;
DELETE FROM reminder WHERE if_no_reply = 1;
ALTER TABLE reminder DROP COLUMN fired_at_ms;
ALTER TABLE reminder DROP COLUMN if_no_reply;
ALTER TABLE draft DROP COLUMN remind_if_no_reply_at_ms;
ALTER TABLE message DROP COLUMN unsubscribed_at_ms;
ALTER TABLE message DROP COLUMN list_unsubscribe_one_click;
ALTER TABLE message DROP COLUMN list_unsubscribe_json;
ALTER TABLE message DROP COLUMN muted;
ALTER TABLE message DROP COLUMN starred;
