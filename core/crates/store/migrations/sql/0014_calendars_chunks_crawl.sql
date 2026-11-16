-- 0014 (contracts delta 21): calendars pushed by the shell, the chunk with its own id, the body crawl's place. docs/DATA-MAP.md "AI replies";
-- docs/lanes/settingscalendars-mac.md "The seam to the core"; docs/lanes/sync-full-r1.md findings 4 and 5. Additive only; reversal 0014_*.down.sql (written first).
--   folder_sync_state + body_crawl_before_ms, body_crawl_before_id, body_crawl_done_ms   the crawl's place (replaces the body-crawl.tsv file)
--   chunk                one piece of a message part, with its OWN id (AUTOINCREMENT: an id is never reused, so a vector can never point at another message)
--   reply_calendar       the calendars the shell listed (the switch stays in reply_source); calendar_event the events of an enabled one
-- Nothing is backfilled: a folder with no place starts its walk from the newest, a message with no chunk is chunked when the indexer reaches it.

ALTER TABLE folder_sync_state ADD COLUMN body_crawl_before_ms INTEGER;
ALTER TABLE folder_sync_state ADD COLUMN body_crawl_before_id TEXT;
ALTER TABLE folder_sync_state ADD COLUMN body_crawl_done_ms INTEGER;
-- the legal shapes are the contract's `crawl_place_is_legal`: both halves of a place or neither; a finished walk has no place
CREATE TRIGGER folder_crawl_place_legal_insert BEFORE INSERT ON folder_sync_state
WHEN (NEW.body_crawl_before_ms IS NULL) <> (NEW.body_crawl_before_id IS NULL) OR (NEW.body_crawl_done_ms IS NOT NULL AND NEW.body_crawl_before_ms IS NOT NULL)
BEGIN SELECT RAISE(ABORT, 'a crawl place has both halves, and a finished walk has none'); END;
CREATE TRIGGER folder_crawl_place_legal_update BEFORE UPDATE OF body_crawl_before_ms, body_crawl_before_id, body_crawl_done_ms ON folder_sync_state
WHEN (NEW.body_crawl_before_ms IS NULL) <> (NEW.body_crawl_before_id IS NULL) OR (NEW.body_crawl_done_ms IS NOT NULL AND NEW.body_crawl_before_ms IS NOT NULL)
BEGIN SELECT RAISE(ABORT, 'a crawl place has both halves, and a finished walk has none'); END;

CREATE TABLE chunk (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  message_id TEXT NOT NULL REFERENCES message(id) ON DELETE CASCADE,
  part TEXT NOT NULL CHECK (part = 'body' OR (part LIKE 'attachment:%' AND length(part) > 11)),
  chunk_no INTEGER NOT NULL CHECK (chunk_no >= 0),
  received_at_ms INTEGER NOT NULL,
  sender TEXT NOT NULL,
  folder TEXT NOT NULL,
  subject TEXT NOT NULL,
  text TEXT NOT NULL CHECK (length(trim(text)) > 0),
  embedded_at_ms INTEGER,
  UNIQUE (message_id, part, chunk_no)
);
CREATE INDEX chunk_not_embedded ON chunk (id) WHERE embedded_at_ms IS NULL;

CREATE TABLE reply_calendar (
  calendar_id TEXT PRIMARY KEY NOT NULL CHECK (length(trim(calendar_id)) > 0),
  name TEXT NOT NULL CHECK (length(trim(name)) > 0),
  account_label TEXT NOT NULL,
  colour TEXT NOT NULL CHECK (length(colour) = 7 AND colour LIKE '#%'),
  updated_at_ms INTEGER NOT NULL DEFAULT 0
);
-- a calendar the shell no longer lists takes its switch with it
CREATE TRIGGER reply_calendar_gone AFTER DELETE ON reply_calendar
BEGIN DELETE FROM reply_source WHERE source_key = 'calendar' || char(31) || OLD.calendar_id; END;

CREATE TABLE calendar_event (
  calendar_id TEXT NOT NULL REFERENCES reply_calendar(calendar_id) ON DELETE CASCADE,
  event_id TEXT NOT NULL CHECK (length(trim(event_id)) > 0),
  title TEXT NOT NULL,
  start_ms INTEGER NOT NULL,
  end_ms INTEGER NOT NULL CHECK (end_ms >= start_ms),
  attendees_json TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(attendees_json) AND json_type(attendees_json) = 'array'),
  location TEXT,
  PRIMARY KEY (calendar_id, event_id)
);
CREATE INDEX calendar_event_by_start ON calendar_event (start_ms);
