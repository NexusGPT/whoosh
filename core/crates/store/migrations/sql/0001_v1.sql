-- Whoosh migration v1. One table per docs/DATA-MAP.md record whose home is SQLite.
-- Times are milliseconds since the Unix epoch (UTC). Booleans are 0 or 1. *_json columns hold a JSON value.
-- A credential is never stored: a keychain_ref column holds the Keychain service and account joined by the
-- unit separator char(31), nothing else. Synced rows carry updated_at_ms so the latest write wins.

CREATE TABLE account (
  id TEXT PRIMARY KEY NOT NULL,
  address TEXT NOT NULL UNIQUE,
  provider TEXT NOT NULL CHECK (provider IN ('google','microsoft','icloud','fastmail','imap')),
  display_name TEXT NOT NULL,
  colour TEXT NOT NULL CHECK (length(colour) = 7 AND substr(colour, 1, 1) = '#'),
  sort_order INTEGER NOT NULL,
  paused INTEGER NOT NULL DEFAULT 0 CHECK (paused IN (0,1)),
  group_name TEXT,
  icon_id TEXT,
  credential_kind TEXT NOT NULL CHECK (credential_kind IN ('manual','oauth')),
  keychain_ref TEXT NOT NULL CHECK (instr(keychain_ref, char(31)) > 1),
  updated_at_ms INTEGER NOT NULL
);

CREATE TABLE account_icon_file (
  account_id TEXT PRIMARY KEY NOT NULL REFERENCES account(id) ON DELETE CASCADE,
  file_name TEXT NOT NULL,
  mime_type TEXT NOT NULL,
  size_bytes INTEGER NOT NULL CHECK (size_bytes > 0),
  local_path TEXT NOT NULL,
  updated_at_ms INTEGER NOT NULL
);

CREATE TABLE account_sync_state (
  account_id TEXT PRIMARY KEY NOT NULL REFERENCES account(id) ON DELETE CASCADE,
  history_fetched INTEGER NOT NULL DEFAULT 0 CHECK (history_fetched IN (0,1)),
  messages_indexed INTEGER NOT NULL DEFAULT 0 CHECK (messages_indexed >= 0),
  last_error_json TEXT CHECK (last_error_json IS NULL OR json_valid(last_error_json)),
  provider_status_words TEXT,
  last_synced_ms INTEGER
);

CREATE TABLE folder_sync_state (
  account_id TEXT NOT NULL REFERENCES account(id) ON DELETE CASCADE,
  folder TEXT NOT NULL,
  cursor_kind TEXT CHECK (cursor_kind IS NULL OR cursor_kind IN ('imap','graph_delta','gmail_history','jmap')),
  imap_uid_validity INTEGER,
  imap_highest_mod_seq INTEGER,
  cursor_value TEXT,
  last_uid INTEGER,
  last_fetched_ms INTEGER,
  last_full_pass_ms INTEGER,
  PRIMARY KEY (account_id, folder),
  CHECK (
    (cursor_kind IS NULL AND imap_uid_validity IS NULL AND imap_highest_mod_seq IS NULL AND cursor_value IS NULL)
    OR (cursor_kind = 'imap' AND imap_uid_validity IS NOT NULL AND imap_highest_mod_seq IS NOT NULL AND cursor_value IS NULL)
    OR (cursor_kind IN ('graph_delta','gmail_history','jmap') AND cursor_value IS NOT NULL AND imap_uid_validity IS NULL AND imap_highest_mod_seq IS NULL)
  )
);

CREATE TABLE thread (
  id TEXT PRIMARY KEY NOT NULL,
  account_id TEXT NOT NULL REFERENCES account(id) ON DELETE CASCADE,
  remote_id TEXT,
  subject TEXT NOT NULL,
  participants_json TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(participants_json)),
  last_message_at_ms INTEGER
);
CREATE INDEX thread_by_account ON thread (account_id, last_message_at_ms DESC);

CREATE TABLE message (
  id TEXT PRIMARY KEY NOT NULL,
  thread_id TEXT NOT NULL REFERENCES thread(id) ON DELETE CASCADE DEFERRABLE INITIALLY DEFERRED,
  account_id TEXT NOT NULL REFERENCES account(id) ON DELETE CASCADE,
  remote_id TEXT,
  sender_name TEXT NOT NULL,
  from_address TEXT NOT NULL,
  to_json TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(to_json)),
  cc_json TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(cc_json)),
  subject TEXT NOT NULL,
  preview TEXT NOT NULL DEFAULT '',
  body_text TEXT,
  body_html TEXT,
  received_at_ms INTEGER NOT NULL,
  read INTEGER NOT NULL DEFAULT 0 CHECK (read IN (0,1)),
  archived INTEGER NOT NULL DEFAULT 0 CHECK (archived IN (0,1)),
  folder TEXT NOT NULL,
  has_list_header INTEGER NOT NULL DEFAULT 0 CHECK (has_list_header IN (0,1)),
  size_bytes INTEGER,
  login_code TEXT,
  trackers_blocked INTEGER NOT NULL DEFAULT 0 CHECK (trackers_blocked >= 0),
  UNIQUE (account_id, folder, remote_id)
);
CREATE INDEX message_by_folder ON message (account_id, folder, received_at_ms DESC);
CREATE INDEX message_by_thread ON message (thread_id, received_at_ms);
CREATE INDEX message_by_sender ON message (from_address);

CREATE TABLE attachment (
  id TEXT PRIMARY KEY NOT NULL,
  message_id TEXT NOT NULL REFERENCES message(id) ON DELETE CASCADE,
  remote_id TEXT,
  file_name TEXT NOT NULL,
  mime_type TEXT NOT NULL,
  size_bytes INTEGER NOT NULL CHECK (size_bytes >= 0),
  local_path TEXT,
  extracted_text TEXT
);
CREATE INDEX attachment_by_message ON attachment (message_id);

CREATE TABLE contact (
  id TEXT PRIMARY KEY NOT NULL,
  name TEXT NOT NULL,
  addresses_json TEXT NOT NULL CHECK (json_valid(addresses_json)),
  logo_id TEXT,
  mail_count_json TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(mail_count_json)),
  updated_at_ms INTEGER NOT NULL
);

CREATE TABLE draft (
  id TEXT PRIMARY KEY NOT NULL,
  from_account_id TEXT NOT NULL REFERENCES account(id) ON DELETE CASCADE,
  to_json TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(to_json)),
  cc_json TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(cc_json)),
  subject TEXT NOT NULL DEFAULT '',
  body TEXT NOT NULL DEFAULT '',
  attachment_ids_json TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(attachment_ids_json)),
  ai_marks_json TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(ai_marks_json)),
  reply_to_thread_id TEXT,
  updated_at_ms INTEGER NOT NULL
);

CREATE TABLE outbox_entry (
  id TEXT PRIMARY KEY NOT NULL,
  draft_json TEXT NOT NULL CHECK (json_valid(draft_json)),
  state TEXT NOT NULL CHECK (state IN ('undoable','scheduled','sending','sent','failed','cancelled')),
  due_at_ms INTEGER,
  sent_at_ms INTEGER,
  failure_json TEXT CHECK (failure_json IS NULL OR json_valid(failure_json)),
  attempts INTEGER NOT NULL DEFAULT 0 CHECK (attempts >= 0),
  tracking_id TEXT,
  created_at_ms INTEGER NOT NULL,
  updated_at_ms INTEGER NOT NULL,
  CHECK ((state = 'failed') = (failure_json IS NOT NULL)),
  CHECK ((state = 'sent') = (sent_at_ms IS NOT NULL)),
  CHECK (state NOT IN ('undoable','scheduled') OR due_at_ms IS NOT NULL)
);
CREATE INDEX outbox_by_state ON outbox_entry (state, due_at_ms);

CREATE TABLE label (
  id TEXT PRIMARY KEY NOT NULL,
  name TEXT NOT NULL,
  parent_id TEXT REFERENCES label(id) ON DELETE SET NULL,
  colour TEXT,
  account_ids_json TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(account_ids_json)),
  updated_at_ms INTEGER NOT NULL
);

CREATE TABLE label_assignment (
  label_id TEXT NOT NULL REFERENCES label(id) ON DELETE CASCADE,
  message_id TEXT NOT NULL REFERENCES message(id) ON DELETE CASCADE,
  updated_at_ms INTEGER NOT NULL,
  PRIMARY KEY (label_id, message_id)
);
CREATE INDEX label_assignment_by_message ON label_assignment (message_id);

CREATE TABLE pin (
  message_id TEXT PRIMARY KEY NOT NULL REFERENCES message(id) ON DELETE CASCADE,
  pinned_at_ms INTEGER NOT NULL
);

CREATE TABLE snooze (
  message_id TEXT PRIMARY KEY NOT NULL REFERENCES message(id) ON DELETE CASCADE,
  return_at_ms INTEGER NOT NULL,
  updated_at_ms INTEGER NOT NULL
);
CREATE INDEX snooze_by_return ON snooze (return_at_ms);

CREATE TABLE undo_entry (
  id TEXT PRIMARY KEY NOT NULL,
  action_json TEXT NOT NULL CHECK (json_valid(action_json)),
  at_ms INTEGER NOT NULL,
  consumed INTEGER NOT NULL DEFAULT 0 CHECK (consumed IN (0,1))
);
CREATE INDEX undo_by_time ON undo_entry (consumed, at_ms DESC);

CREATE TABLE split_tab (
  id TEXT PRIMARY KEY NOT NULL,
  name TEXT NOT NULL,
  sort_order INTEGER NOT NULL,
  rule_kind TEXT NOT NULL CHECK (rule_kind IN ('sender','domain','account','label','decision_category','builtin')),
  rule_value TEXT NOT NULL,
  enabled INTEGER NOT NULL DEFAULT 1 CHECK (enabled IN (0,1)),
  updated_at_ms INTEGER NOT NULL
);

CREATE TABLE rule (
  id TEXT PRIMARY KEY NOT NULL,
  position INTEGER NOT NULL,
  conditions_json TEXT NOT NULL CHECK (json_valid(conditions_json)),
  tab_id TEXT NOT NULL,
  enabled INTEGER NOT NULL DEFAULT 1 CHECK (enabled IN (0,1)),
  updated_at_ms INTEGER NOT NULL
);
CREATE INDEX rule_by_position ON rule (position);

CREATE TABLE message_category (
  message_id TEXT PRIMARY KEY NOT NULL REFERENCES message(id) ON DELETE CASCADE,
  tab_id TEXT NOT NULL,
  decided_by TEXT NOT NULL CHECK (decided_by IN ('rule','laya','jev','owner','default')),
  rule_id TEXT,
  probability REAL CHECK (probability IS NULL OR (probability >= 0.0 AND probability <= 1.0)),
  decided_at_ms INTEGER NOT NULL,
  updated_at_ms INTEGER NOT NULL,
  CHECK ((decided_by = 'rule') = (rule_id IS NOT NULL))
);
CREATE INDEX message_category_by_tab ON message_category (tab_id);

CREATE TABLE ai_provider (
  id TEXT PRIMARY KEY NOT NULL,
  name TEXT NOT NULL,
  models_json TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(models_json)),
  connected INTEGER NOT NULL DEFAULT 0 CHECK (connected IN (0,1)),
  keychain_ref TEXT CHECK (keychain_ref IS NULL OR instr(keychain_ref, char(31)) > 1),
  updated_at_ms INTEGER NOT NULL
);

CREATE TABLE decision_model (
  id INTEGER PRIMARY KEY NOT NULL CHECK (id = 1),
  provider TEXT NOT NULL CHECK (provider IN ('local','jev','rules')),
  model TEXT,
  jev_api_key_ref TEXT CHECK (jev_api_key_ref IS NULL OR instr(jev_api_key_ref, char(31)) > 1),
  decisions_enabled_json TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(decisions_enabled_json)),
  question_words_json TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(question_words_json)),
  last_run_ms INTEGER,
  updated_at_ms INTEGER NOT NULL
);

CREATE TABLE speech_engine (
  id INTEGER PRIMARY KEY NOT NULL CHECK (id = 1),
  kind TEXT NOT NULL CHECK (kind IN ('local','cloud')),
  cloud_provider_id TEXT REFERENCES ai_provider(id) ON DELETE SET NULL,
  ready INTEGER NOT NULL DEFAULT 0 CHECK (ready IN (0,1)),
  model_path TEXT
);

CREATE TABLE agent_config (
  id TEXT PRIMARY KEY NOT NULL,
  name TEXT NOT NULL,
  purpose TEXT NOT NULL DEFAULT '',
  provider_id TEXT NOT NULL,
  model TEXT NOT NULL,
  instructions TEXT NOT NULL DEFAULT '',
  allowed_json TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(allowed_json)),
  built_in INTEGER NOT NULL DEFAULT 0 CHECK (built_in IN (0,1)),
  updated_at_ms INTEGER NOT NULL
);

CREATE TABLE agent_conversation (
  id TEXT PRIMARY KEY NOT NULL,
  agent_id TEXT NOT NULL REFERENCES agent_config(id) ON DELETE CASCADE,
  turns_json TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(turns_json)),
  started_at_ms INTEGER NOT NULL,
  updated_at_ms INTEGER NOT NULL
);
CREATE INDEX agent_conversation_by_agent ON agent_conversation (agent_id, updated_at_ms DESC);

CREATE TABLE agent_proposal (
  id TEXT PRIMARY KEY NOT NULL,
  agent_id TEXT NOT NULL REFERENCES agent_config(id) ON DELETE CASCADE,
  action_words TEXT NOT NULL,
  message_ids_json TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(message_ids_json)),
  label TEXT,
  decision TEXT NOT NULL DEFAULT 'pending' CHECK (decision IN ('pending','approved','rejected')),
  decided_at_ms INTEGER,
  created_at_ms INTEGER NOT NULL,
  CHECK ((decision = 'pending') = (decided_at_ms IS NULL))
);

CREATE TABLE mcp_grant (
  app_name TEXT PRIMARY KEY NOT NULL,
  allowed INTEGER NOT NULL DEFAULT 0 CHECK (allowed IN (0,1)),
  granted_at_ms INTEGER
);

CREATE TABLE notch_setting (
  id INTEGER PRIMARY KEY NOT NULL CHECK (id = 1),
  show INTEGER NOT NULL DEFAULT 1 CHECK (show IN (0,1)),
  buttons_json TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(buttons_json)),
  hidden_in_screen_share INTEGER NOT NULL DEFAULT 1 CHECK (hidden_in_screen_share IN (0,1)),
  follow_focus INTEGER NOT NULL DEFAULT 0 CHECK (follow_focus IN (0,1))
);

CREATE TABLE notification_setting (
  account_id TEXT PRIMARY KEY NOT NULL REFERENCES account(id) ON DELETE CASCADE,
  enabled INTEGER NOT NULL DEFAULT 1 CHECK (enabled IN (0,1)),
  updated_at_ms INTEGER NOT NULL
);

CREATE TABLE search_history (
  query TEXT PRIMARY KEY NOT NULL,
  at_ms INTEGER NOT NULL
);
CREATE INDEX search_history_by_time ON search_history (at_ms DESC);

CREATE TABLE settings_sync (
  id INTEGER PRIMARY KEY NOT NULL CHECK (id = 1),
  devices_json TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(devices_json)),
  last_sync_ms INTEGER,
  records_carried_json TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(records_carried_json))
);

CREATE TABLE shortcut_preset (
  id INTEGER PRIMARY KEY NOT NULL CHECK (id = 1),
  preset TEXT NOT NULL,
  overrides_json TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(overrides_json)),
  updated_at_ms INTEGER NOT NULL
);

CREATE TABLE shortcut_usage (
  action TEXT PRIMARY KEY NOT NULL,
  mouse_uses INTEGER NOT NULL DEFAULT 0 CHECK (mouse_uses >= 0),
  key_uses INTEGER NOT NULL DEFAULT 0 CHECK (key_uses >= 0)
);

CREATE TABLE signature (
  address TEXT PRIMARY KEY NOT NULL,
  text TEXT NOT NULL,
  updated_at_ms INTEGER NOT NULL
);

CREATE TABLE smart_folder (
  id TEXT PRIMARY KEY NOT NULL,
  name TEXT NOT NULL,
  rule_words TEXT NOT NULL,
  parent_id TEXT REFERENCES smart_folder(id) ON DELETE SET NULL,
  updated_at_ms INTEGER NOT NULL
);

CREATE TABLE voice_profile (
  address TEXT PRIMARY KEY NOT NULL,
  summary_words TEXT NOT NULL,
  built_at_ms INTEGER NOT NULL,
  updated_at_ms INTEGER NOT NULL
);

CREATE TABLE device_link (
  id TEXT PRIMARY KEY NOT NULL,
  keychain_ref TEXT NOT NULL CHECK (instr(keychain_ref, char(31)) > 1),
  created_at_ms INTEGER NOT NULL,
  expires_at_ms INTEGER NOT NULL,
  used_at_ms INTEGER
);

CREATE TABLE index_manifest (
  index_kind TEXT PRIMARY KEY NOT NULL CHECK (index_kind IN ('text','vector')),
  folder TEXT NOT NULL,
  documents INTEGER NOT NULL DEFAULT 0 CHECK (documents >= 0),
  dimension INTEGER,
  built_at_ms INTEGER,
  CHECK ((index_kind = 'vector') = (dimension IS NOT NULL))
);

CREATE TABLE vip (
  kind TEXT NOT NULL CHECK (kind IN ('address','domain')),
  value TEXT NOT NULL,
  updated_at_ms INTEGER NOT NULL,
  PRIMARY KEY (kind, value)
);

CREATE TABLE template (
  id TEXT PRIMARY KEY NOT NULL,
  name TEXT NOT NULL,
  body TEXT NOT NULL,
  from_account_id TEXT REFERENCES account(id) ON DELETE SET NULL,
  updated_at_ms INTEGER NOT NULL
);

CREATE TABLE tracking_event (
  id INTEGER PRIMARY KEY NOT NULL,
  tracking_id TEXT NOT NULL,
  kind TEXT NOT NULL CHECK (kind IN ('open','click')),
  at_ms INTEGER NOT NULL,
  UNIQUE (tracking_id, kind, at_ms)
);
CREATE INDEX tracking_event_by_id ON tracking_event (tracking_id);

CREATE TABLE layout_state (
  window_id TEXT PRIMARY KEY NOT NULL,
  pane_widths_json TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(pane_widths_json)),
  collapsed_json TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(collapsed_json)),
  selected_mailboxes_json TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(selected_mailboxes_json))
);

CREATE TABLE tracker_rule (
  pattern TEXT PRIMARY KEY NOT NULL,
  blocked INTEGER NOT NULL DEFAULT 1 CHECK (blocked IN (0,1)),
  open_and_click_tracking_on_sent INTEGER NOT NULL DEFAULT 0 CHECK (open_and_click_tracking_on_sent IN (0,1))
);

CREATE TABLE note (
  id TEXT PRIMARY KEY NOT NULL,
  thread_id TEXT NOT NULL REFERENCES thread(id) ON DELETE CASCADE,
  message_id TEXT REFERENCES message(id) ON DELETE CASCADE,
  body TEXT NOT NULL,
  created_at_ms INTEGER NOT NULL,
  updated_at_ms INTEGER NOT NULL
);
CREATE INDEX note_by_thread ON note (thread_id);

CREATE TABLE reminder (
  id TEXT PRIMARY KEY NOT NULL,
  message_id TEXT NOT NULL REFERENCES message(id) ON DELETE CASCADE,
  when_ms INTEGER NOT NULL,
  note TEXT NOT NULL DEFAULT '',
  updated_at_ms INTEGER NOT NULL
);
CREATE INDEX reminder_by_time ON reminder (when_ms);
