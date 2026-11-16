-- Reversal of 0015: back to 0014's exact schema. Written first. DATA LOST: every file attached to a draft or a queued message (the drafts and the queued messages stay,
-- without their files: a message queued with files would go out without them, so revert only with an empty outbox).
DROP TRIGGER IF EXISTS outgoing_attachment_outbox_gone;
DROP TRIGGER IF EXISTS outgoing_attachment_draft_gone;
DROP TABLE IF EXISTS outgoing_attachment;
