-- Reversal of 0008: back to 0007's schema. Data lost on reversal (the owner's card of 09:35): the mail-record note on the
-- accounts found that way (their detection_source becomes NULL, as for an account added before 0003) and the detection
-- cache rows found that way (the next Add account detects them again). Every other value is copied.
UPDATE account SET detection_source = NULL WHERE detection_source = 'mx';
ALTER TABLE account ADD COLUMN detection_source_v7 TEXT CHECK (detection_source_v7 IS NULL OR detection_source_v7 IN ('ispdb','autoconfig','srv','autodiscover','guess','manual'));
UPDATE account SET detection_source_v7 = detection_source;
ALTER TABLE account DROP COLUMN detection_source;
ALTER TABLE account RENAME COLUMN detection_source_v7 TO detection_source;
DELETE FROM detection_cache WHERE source = 'mx';
CREATE TABLE detection_cache_v7 (
  domain TEXT PRIMARY KEY NOT NULL CHECK (domain = lower(domain) AND length(domain) > 0),
  outcome TEXT NOT NULL CHECK (outcome IN ('found','imap_only','refused')),
  source TEXT CHECK (source IS NULL OR source IN ('ispdb','autoconfig','srv','autodiscover','guess','manual')),
  config_json TEXT CHECK (config_json IS NULL OR json_valid(config_json)),
  refusal_words TEXT,
  detected_at_ms INTEGER NOT NULL,
  CHECK ((outcome = 'refused') = (config_json IS NULL AND refusal_words IS NOT NULL)),
  CHECK ((outcome = 'refused') = (source IS NULL))
);
INSERT INTO detection_cache_v7 (domain, outcome, source, config_json, refusal_words, detected_at_ms)
  SELECT domain, outcome, source, config_json, refusal_words, detected_at_ms FROM detection_cache;
DROP TABLE detection_cache;
ALTER TABLE detection_cache_v7 RENAME TO detection_cache;
