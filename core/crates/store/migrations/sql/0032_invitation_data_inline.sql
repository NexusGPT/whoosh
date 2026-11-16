-- 0032: the calendar data of a meeting invitation is not a file of the mail (owner, 13 Nov 2026, picture 212: "fantom attachment").
-- The parser now lists such a part with inline set (core/crates/parse/src/mime.rs `is_invitation_data`); this marks the rows stored before:
-- a calendar part the sender gave no name (the engine named it attachment-N) and that is not a picture referenced from the text.
-- No row is removed and no schema changes. Reversal: 0032_invitation_data_inline.down.sql.
UPDATE attachment SET inline = 1
 WHERE inline = 0 AND mime_type IN ('text/calendar', 'application/ics') AND file_name GLOB 'attachment-[0-9]*';
