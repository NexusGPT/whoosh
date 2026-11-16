-- Reversal of 0032: the marked calendar parts read as files again (a calendar part is never referenced by a content id, so these rows are
-- exactly the ones 0032 marked, plus any the parser marked since: those read as files again too, as before 0032).
UPDATE attachment SET inline = 0
 WHERE inline = 1 AND mime_type IN ('text/calendar', 'application/ics') AND file_name GLOB 'attachment-[0-9]*';
