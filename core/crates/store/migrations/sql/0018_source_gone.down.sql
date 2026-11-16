-- Reversal of 0018: back to 0017's exact schema. DATA LOST: which folders were gone and the owner's remembered switch (a gone folder reads present and off).
DROP TRIGGER IF EXISTS reply_source_presence_legal_update;
DROP TRIGGER IF EXISTS reply_source_presence_legal_insert;
ALTER TABLE reply_source DROP COLUMN owner_enabled;
ALTER TABLE reply_source DROP COLUMN gone;
