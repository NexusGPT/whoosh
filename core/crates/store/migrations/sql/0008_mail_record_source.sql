-- 0008, contracts delta 14 (2026-11-09): the detection source word 'mx' (DetectionSource::MailRecord: the domain's MX
-- record named the provider; core-autoconfig r2). 0003's CHECK lists did not hold it, and SQLite cannot change a CHECK in
-- place. Owner's go on the card of 09:35. Nothing is lost going forward: every existing value is copied.
-- 1. account.detection_source: a new column with the longer list, the values copied, the old column dropped (its CHECK
--    names only itself, so SQLite drops it), the new one renamed back. The column stays the table's last.
ALTER TABLE account ADD COLUMN detection_source_v8 TEXT CHECK (detection_source_v8 IS NULL OR detection_source_v8 IN ('ispdb','autoconfig','srv','autodiscover','mx','guess','manual'));
UPDATE account SET detection_source_v8 = detection_source;
ALTER TABLE account DROP COLUMN detection_source;
ALTER TABLE account RENAME COLUMN detection_source_v8 TO detection_source;
-- 2. detection_cache: its table CHECK reads `source`, so the table is rebuilt as in 0003 with 'mx' in the list and every row
--    copied. 0003 gave it no index beyond its primary key.
CREATE TABLE detection_cache_v8 (
  domain TEXT PRIMARY KEY NOT NULL CHECK (domain = lower(domain) AND length(domain) > 0),
  outcome TEXT NOT NULL CHECK (outcome IN ('found','imap_only','refused')),
  source TEXT CHECK (source IS NULL OR source IN ('ispdb','autoconfig','srv','autodiscover','mx','guess','manual')),
  config_json TEXT CHECK (config_json IS NULL OR json_valid(config_json)),
  refusal_words TEXT,
  detected_at_ms INTEGER NOT NULL,
  CHECK ((outcome = 'refused') = (config_json IS NULL AND refusal_words IS NOT NULL)),
  CHECK ((outcome = 'refused') = (source IS NULL))
);
INSERT INTO detection_cache_v8 (domain, outcome, source, config_json, refusal_words, detected_at_ms)
  SELECT domain, outcome, source, config_json, refusal_words, detected_at_ms FROM detection_cache;
DROP TABLE detection_cache;
ALTER TABLE detection_cache_v8 RENAME TO detection_cache;
