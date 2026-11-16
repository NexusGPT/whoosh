-- Reversal of 0014: back to 0013's exact schema. Written first. DATA LOST on reversal: every crawl place (the walk of each folder restarts from the newest, header-only
-- rows are skipped, no body is downloaded twice), every chunk (the indexer rebuilds them; the vectors keyed by chunk id are orphaned and must be dropped with the
-- meaning index), the calendars and events the shell pushed (it pushes them again), and the switches of calendar sources go with their calendar.
DROP TABLE IF EXISTS calendar_event;
DROP TABLE IF EXISTS reply_calendar;
DROP TABLE IF EXISTS chunk;
DROP TRIGGER IF EXISTS folder_crawl_place_legal_update;
DROP TRIGGER IF EXISTS folder_crawl_place_legal_insert;
ALTER TABLE folder_sync_state DROP COLUMN body_crawl_done_ms;
ALTER TABLE folder_sync_state DROP COLUMN body_crawl_before_id;
ALTER TABLE folder_sync_state DROP COLUMN body_crawl_before_ms;
