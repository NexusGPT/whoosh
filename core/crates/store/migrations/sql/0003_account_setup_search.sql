-- 0003: the server configuration of an account (Add account by IMAP), the last detection per domain, the Gmail
-- message-in-several-label-folders table, and the text read out of attachments (One search). Additive only:
-- auth_method is 'basic' (a mail or app password, kept in the Keychain) or 'oauth2'; every new column is nullable (an account added before 0003, or by Graph or JMAP, has no IMAP server config).
-- docs/DATA-MAP.md "Add account by IMAP" and "One search". The reversal is 0003_account_setup_search.down.sql.
ALTER TABLE account ADD COLUMN imap_host TEXT;
ALTER TABLE account ADD COLUMN imap_port INTEGER CHECK (imap_port IS NULL OR (imap_port BETWEEN 1 AND 65535));
ALTER TABLE account ADD COLUMN imap_security TEXT CHECK (imap_security IS NULL OR imap_security IN ('ssl','starttls'));
ALTER TABLE account ADD COLUMN smtp_host TEXT;
ALTER TABLE account ADD COLUMN smtp_port INTEGER CHECK (smtp_port IS NULL OR (smtp_port BETWEEN 1 AND 65535));
ALTER TABLE account ADD COLUMN smtp_security TEXT CHECK (smtp_security IS NULL OR smtp_security IN ('ssl','starttls'));
ALTER TABLE account ADD COLUMN username TEXT;
ALTER TABLE account ADD COLUMN smtp_username TEXT;
ALTER TABLE account ADD COLUMN auth_method TEXT CHECK (auth_method IS NULL OR auth_method IN ('basic','oauth2'));
ALTER TABLE account ADD COLUMN detection_source TEXT CHECK (detection_source IS NULL OR detection_source IN ('ispdb','autoconfig','srv','autodiscover','guess','manual'));

-- The last detection result per domain, read before a new lookup and cleared by Manual setup.
-- A found result carries its config as JSON (the AccountConfig fields); a refusal carries its words.
CREATE TABLE detection_cache (
  domain TEXT PRIMARY KEY NOT NULL CHECK (domain = lower(domain) AND length(domain) > 0),
  outcome TEXT NOT NULL CHECK (outcome IN ('found','imap_only','refused')),
  source TEXT CHECK (source IS NULL OR source IN ('ispdb','autoconfig','srv','autodiscover','guess','manual')),
  config_json TEXT CHECK (config_json IS NULL OR json_valid(config_json)),
  refusal_words TEXT,
  detected_at_ms INTEGER NOT NULL,
  CHECK ((outcome = 'refused') = (config_json IS NULL AND refusal_words IS NOT NULL)),
  CHECK ((outcome = 'refused') = (source IS NULL))
);

-- Gmail over IMAP: one message row, one row per label-folder copy (docs/lanes/core-sync-imap.md "Round 2").
CREATE TABLE message_location (
  account_id TEXT NOT NULL REFERENCES account(id) ON DELETE CASCADE,
  folder TEXT NOT NULL,
  uid_validity INTEGER NOT NULL,
  uid INTEGER NOT NULL,
  message_id TEXT NOT NULL REFERENCES message(id) ON DELETE CASCADE,
  PRIMARY KEY (account_id, folder, uid_validity, uid)
);
CREATE INDEX message_location_by_message ON message_location (message_id);

-- What was read out of an attachment on this device, once it was downloaded: extracted text, OCR text and scene
-- labels of an image, the transcript of speech. `embedded` = its vector is in the vector index (the vector itself is not here).
CREATE TABLE attachment_content (
  attachment_id TEXT PRIMARY KEY NOT NULL REFERENCES attachment(id) ON DELETE CASCADE,
  text TEXT,
  ocr_text TEXT,
  labels_json TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(labels_json)),
  transcript TEXT,
  embedded INTEGER NOT NULL DEFAULT 0 CHECK (embedded IN (0,1)),
  updated_at_ms INTEGER NOT NULL
);
