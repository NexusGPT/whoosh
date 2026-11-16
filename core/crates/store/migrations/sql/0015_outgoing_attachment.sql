-- 0015 (contracts delta 22): the files of a draft and of a queued message. docs/DATA-MAP.md `outgoing-attachment`; core/crates/contracts/src/reply_ai_d22.rs.
-- One new table, no existing table changes. `Draft.attachment_ids` (already there) lists the ids of a draft's rows. A row is owned by a draft; `Send` re-owns the rows to
-- the outbox entry (so the draft can go, and an undo of the send gives them back); a removed owner takes its rows. The caps are the contract's (ledger proposals), not SQL.
CREATE TABLE outgoing_attachment (
  id TEXT PRIMARY KEY NOT NULL CHECK (length(trim(id)) > 0),
  owner_kind TEXT NOT NULL CHECK (owner_kind IN ('draft','outbox')),
  owner_id TEXT NOT NULL CHECK (length(trim(owner_id)) > 0),
  source_kind TEXT NOT NULL CHECK (source_kind IN ('local_file','stored_message')),
  path TEXT,
  message_id TEXT,
  attachment_id TEXT,
  file_name TEXT NOT NULL CHECK (length(trim(file_name)) > 0),
  media_type TEXT NOT NULL,
  size_bytes INTEGER NOT NULL CHECK (size_bytes >= 0),
  state TEXT NOT NULL CHECK (state IN ('ready','will_download','gone','too_large')),
  sort_order INTEGER NOT NULL DEFAULT 0,
  CHECK ((source_kind = 'local_file' AND path IS NOT NULL AND length(trim(path)) > 0 AND message_id IS NULL AND attachment_id IS NULL)
      OR (source_kind = 'stored_message' AND path IS NULL AND message_id IS NOT NULL AND length(trim(message_id)) > 0 AND attachment_id IS NOT NULL AND length(trim(attachment_id)) > 0))
);
-- the same source is one row per owner (adding is idempotent)
CREATE UNIQUE INDEX outgoing_attachment_one_source ON outgoing_attachment (owner_kind, owner_id, source_kind, coalesce(path, ''), coalesce(message_id, ''), coalesce(attachment_id, ''));
CREATE INDEX outgoing_attachment_by_owner ON outgoing_attachment (owner_kind, owner_id, sort_order);
CREATE TRIGGER outgoing_attachment_draft_gone AFTER DELETE ON draft
BEGIN DELETE FROM outgoing_attachment WHERE owner_kind = 'draft' AND owner_id = OLD.id; END;
CREATE TRIGGER outgoing_attachment_outbox_gone AFTER DELETE ON outbox_entry
BEGIN DELETE FROM outgoing_attachment WHERE owner_kind = 'outbox' AND owner_id = OLD.id; END;
