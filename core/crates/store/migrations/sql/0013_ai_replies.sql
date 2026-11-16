-- 0013 (contracts delta 20): AI replies. docs/DATA-MAP.md "AI replies"; docs/JOURNEYS.md J10; core/crates/contracts/src/reply_ai*.rs.
-- Additive only: eleven new tables and one label column (ai_reads_thread, default 0: every label behaves as before). Reversal: 0013_ai_replies.down.sql (written first).
--   reply_use            two rows (auto, on_demand): master switch, tone id, match-thread switch, prompt id
--   reply_filter         the auto use's three cards (one row), reply_filter_label the ticked labels (no row = every label while scope_all = 1)
--   reply_on_demand      the two switches of Sur demande (one row)
--   reply_pass           one row per message the auto use looked at; the counts of the last 7 days are DERIVED from these rows
--   reply_source         one source of context (a folder of an account, a folder of this Mac, a calendar): switch, count, sync progress
--   retrieval_settings   how the context is found (one row)
--   reply_voice          the tone text and the analysis (one row); reply_tone the other tones
--   reply_prompt         prompt versions: append-only, one current per prompt; fixed_note the notes always joined to it
--   reply_draft          the AI's draft of a reply: one LIVE draft per thread, a sent or discarded row never changes
-- The chunk table and its vectors are NOT here: the background indexer's lane owns them. Defaults are the contract's (reply_ai_logic.rs
-- default_*), pinned equal by contracts/tests/replies_d20.rs `the_defaults_equal_the_migrations_seed`. The master switch of the auto use
-- starts OFF: nothing is drafted, and no mail text goes to any model, until the owner turns it on.

-- the label AI and the AI replies share ONE reads record (contracts delta 20: `AiReads.rest_of_thread`); the label's three existing ticks are untouched,
-- its trigger label_ai_reads_something_* still counts subject, body, attachments only (a thread alone is meaningless for a label)
ALTER TABLE label ADD COLUMN ai_reads_thread INTEGER NOT NULL DEFAULT 0 CHECK (ai_reads_thread IN (0,1));

CREATE TABLE reply_prompt (
  prompt_id TEXT NOT NULL CHECK (length(trim(prompt_id)) > 0),
  version INTEGER NOT NULL CHECK (version >= 1),
  text TEXT NOT NULL CHECK (length(trim(text)) > 0),
  created_at_ms INTEGER NOT NULL,
  current INTEGER NOT NULL DEFAULT 0 CHECK (current IN (0,1)),
  PRIMARY KEY (prompt_id, version)
);
CREATE UNIQUE INDEX reply_prompt_one_current ON reply_prompt (prompt_id) WHERE current = 1;
-- versions only grow: only the `current` flag may change, and no version is deleted
CREATE TRIGGER reply_prompt_text_is_sealed BEFORE UPDATE OF prompt_id, version, text, created_at_ms ON reply_prompt
BEGIN SELECT RAISE(ABORT, 'a prompt version never changes: save a new version'); END;
CREATE TRIGGER reply_prompt_versions_stay BEFORE DELETE ON reply_prompt
BEGIN SELECT RAISE(ABORT, 'a prompt version is never deleted'); END;
INSERT INTO reply_prompt (prompt_id, version, text, created_at_ms, current) VALUES ('replies', 1, 'You are replying to {{message}}. You have read {{thread}} and {{excerpts}}. My availability is in {{calendar}}.
Write the reply in this tone: {{tone}}. Follow {{instruction}} when there is one. Always keep in mind: {{notes}}.
Answer the question first. Never promise a price or a deadline.
// One sentence per rule. This line is a note: the model does not read it.', 0, 1);

CREATE TABLE reply_tone (
  id TEXT PRIMARY KEY NOT NULL CHECK (length(trim(id)) > 0 AND id <> 'voice'),
  name TEXT NOT NULL CHECK (length(trim(name)) > 0),
  text TEXT NOT NULL CHECK (length(trim(text)) > 0),
  sort_order INTEGER NOT NULL DEFAULT 0
);
CREATE UNIQUE INDEX reply_tone_one_name ON reply_tone (lower(trim(name)));

CREATE TABLE reply_use (
  kind TEXT PRIMARY KEY NOT NULL CHECK (kind IN ('auto','on_demand')),
  enabled INTEGER NOT NULL CHECK (enabled IN (0,1)),
  tone_id TEXT NOT NULL CHECK (length(trim(tone_id)) > 0),
  match_thread INTEGER NOT NULL CHECK (match_thread IN (0,1)),
  prompt_id TEXT NOT NULL CHECK (length(trim(prompt_id)) > 0),
  updated_at_ms INTEGER NOT NULL DEFAULT 0
);
-- a use reads the voice or an existing tone, and an existing prompt (the words are the contract's: reply_ai_logic.rs)
CREATE TRIGGER reply_use_tone_exists_insert BEFORE INSERT ON reply_use
WHEN NEW.tone_id <> 'voice' AND NOT EXISTS (SELECT 1 FROM reply_tone WHERE id = NEW.tone_id)
BEGIN SELECT RAISE(ABORT, 'that tone does not exist'); END;
CREATE TRIGGER reply_use_tone_exists_update BEFORE UPDATE OF tone_id ON reply_use
WHEN NEW.tone_id <> 'voice' AND NOT EXISTS (SELECT 1 FROM reply_tone WHERE id = NEW.tone_id)
BEGIN SELECT RAISE(ABORT, 'that tone does not exist'); END;
CREATE TRIGGER reply_use_prompt_exists_insert BEFORE INSERT ON reply_use
WHEN NOT EXISTS (SELECT 1 FROM reply_prompt WHERE prompt_id = NEW.prompt_id)
BEGIN SELECT RAISE(ABORT, 'that prompt does not exist'); END;
CREATE TRIGGER reply_use_prompt_exists_update BEFORE UPDATE OF prompt_id ON reply_use
WHEN NOT EXISTS (SELECT 1 FROM reply_prompt WHERE prompt_id = NEW.prompt_id)
BEGIN SELECT RAISE(ABORT, 'that prompt does not exist'); END;
-- a removed tone: the uses that chose it fall back to the voice
CREATE TRIGGER reply_tone_removed AFTER DELETE ON reply_tone
BEGIN UPDATE reply_use SET tone_id = 'voice' WHERE tone_id = OLD.id; END;
INSERT INTO reply_use (kind, enabled, tone_id, match_thread, prompt_id) VALUES ('auto', 0, 'voice', 1, 'replies'), ('on_demand', 1, 'voice', 1, 'replies');

CREATE TABLE reply_filter (
  id INTEGER PRIMARY KEY NOT NULL CHECK (id = 1),
  scope_all INTEGER NOT NULL DEFAULT 1 CHECK (scope_all IN (0,1)),
  entry_enabled INTEGER NOT NULL DEFAULT 1 CHECK (entry_enabled IN (0,1)),
  entry_match TEXT NOT NULL DEFAULT 'all' CHECK (entry_match IN ('any','all')),
  entry_conditions_json TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(entry_conditions_json) AND json_type(entry_conditions_json) = 'array'),
  ai_enabled INTEGER NOT NULL DEFAULT 1 CHECK (ai_enabled IN (0,1)),
  ai_reads_subject INTEGER NOT NULL DEFAULT 1 CHECK (ai_reads_subject IN (0,1)),
  ai_reads_body INTEGER NOT NULL DEFAULT 1 CHECK (ai_reads_body IN (0,1)),
  ai_reads_thread INTEGER NOT NULL DEFAULT 1 CHECK (ai_reads_thread IN (0,1)),
  ai_reads_attachments INTEGER NOT NULL DEFAULT 0 CHECK (ai_reads_attachments IN (0,1)),
  ai_description TEXT NOT NULL DEFAULT '',
  exit_enabled INTEGER NOT NULL DEFAULT 1 CHECK (exit_enabled IN (0,1)),
  exit_match TEXT NOT NULL DEFAULT 'all' CHECK (exit_match IN ('any','all')),
  exit_conditions_json TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(exit_conditions_json) AND json_type(exit_conditions_json) = 'array'),
  updated_at_ms INTEGER NOT NULL DEFAULT 0,
  CHECK (ai_reads_subject + ai_reads_body + ai_reads_thread + ai_reads_attachments >= 1),
  CHECK (ai_enabled = 0 OR length(trim(ai_description)) > 0)
);
INSERT INTO reply_filter (id, ai_description) VALUES (1, 'Emails that wait for a reply from me: a question, a request, a decision that someone is waiting for.');
-- the ticked labels: a deleted label leaves the list (an emptied list with scope_all = 0 admits nothing, as the contract says)
CREATE TABLE reply_filter_label (label_id TEXT PRIMARY KEY NOT NULL REFERENCES label(id) ON DELETE CASCADE);

CREATE TABLE reply_on_demand (
  id INTEGER PRIMARY KEY NOT NULL CHECK (id = 1),
  instruction_before INTEGER NOT NULL DEFAULT 1 CHECK (instruction_before IN (0,1)),
  change_after INTEGER NOT NULL DEFAULT 1 CHECK (change_after IN (0,1)),
  updated_at_ms INTEGER NOT NULL DEFAULT 0
);
INSERT INTO reply_on_demand (id) VALUES (1);

CREATE TABLE reply_draft (
  id TEXT PRIMARY KEY NOT NULL,
  thread_id TEXT NOT NULL REFERENCES thread(id) ON DELETE CASCADE,
  message_id TEXT NOT NULL REFERENCES message(id) ON DELETE CASCADE,
  text TEXT NOT NULL CHECK (length(trim(text)) > 0),
  made_by TEXT NOT NULL CHECK (made_by IN ('auto','on_demand')),
  excerpt_ids_json TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(excerpt_ids_json) AND json_type(excerpt_ids_json) = 'array'),
  instruction TEXT,
  state TEXT NOT NULL DEFAULT 'proposed' CHECK (state IN ('proposed','edited','sent','discarded')),
  revision INTEGER NOT NULL DEFAULT 0 CHECK (revision >= 0),
  created_at_ms INTEGER NOT NULL,
  updated_at_ms INTEGER NOT NULL
);
CREATE UNIQUE INDEX reply_draft_one_live_per_thread ON reply_draft (thread_id) WHERE state IN ('proposed','edited');
CREATE INDEX reply_draft_by_message ON reply_draft (message_id);
-- sealed: a sent or discarded draft changes no more (a cascade delete with its message is not an update); the moves are the contract's `reply_draft_can_move`
CREATE TRIGGER reply_draft_is_sealed BEFORE UPDATE ON reply_draft
WHEN OLD.state IN ('sent','discarded')
BEGIN SELECT RAISE(ABORT, 'this draft is closed: a sent or discarded draft does not change'); END;
CREATE TRIGGER reply_draft_never_goes_back BEFORE UPDATE OF state ON reply_draft
WHEN OLD.state = 'edited' AND NEW.state = 'proposed'
BEGIN SELECT RAISE(ABORT, 'a draft taken into the composer is not proposed again'); END;

-- one row per message the auto use looked at; a looked-at-again message REPLACES its row (the key is the message), so a count cannot count it twice.
-- The legal shapes are the contract's `reply_pass_is_legal`: the entry step is always reached; a stop leaves the later steps not_reached; a draft needs all three through.
CREATE TABLE reply_pass (
  message_id TEXT PRIMARY KEY NOT NULL REFERENCES message(id) ON DELETE CASCADE,
  received_at_ms INTEGER NOT NULL,
  entry_result TEXT NOT NULL CHECK (entry_result IN ('passed','skipped','stopped')),
  ai_result TEXT NOT NULL CHECK (ai_result IN ('passed','skipped','stopped','not_reached')),
  exit_result TEXT NOT NULL CHECK (exit_result IN ('passed','skipped','stopped','not_reached')),
  draft_id TEXT REFERENCES reply_draft(id) ON DELETE SET NULL,
  CHECK (entry_result IN ('passed','skipped') OR (ai_result = 'not_reached' AND exit_result = 'not_reached')),
  CHECK (ai_result IN ('passed','skipped') OR exit_result = 'not_reached'),
  CHECK (draft_id IS NULL OR (entry_result IN ('passed','skipped') AND ai_result IN ('passed','skipped') AND exit_result IN ('passed','skipped')))
);
CREATE INDEX reply_pass_by_time ON reply_pass (received_at_ms);

CREATE TABLE reply_source (
  source_key TEXT PRIMARY KEY NOT NULL,
  kind TEXT NOT NULL CHECK (kind IN ('mail_folder','local_folder','calendar')),
  account_id TEXT REFERENCES account(id) ON DELETE CASCADE,
  enabled INTEGER NOT NULL DEFAULT 0 CHECK (enabled IN (0,1)),
  item_count INTEGER NOT NULL DEFAULT 0 CHECK (item_count >= 0),
  progress_state TEXT NOT NULL DEFAULT 'not_started' CHECK (progress_state IN ('not_started','syncing','up_to_date','failed')),
  progress_percent INTEGER CHECK (progress_percent IS NULL OR (progress_percent >= 0 AND progress_percent <= 100)),
  progress_words TEXT,
  updated_at_ms INTEGER NOT NULL DEFAULT 0,
  -- the key is `reply_source_key`: the kind's word, U+001F, then its parts
  CHECK (source_key LIKE (CASE kind WHEN 'mail_folder' THEN 'mail' WHEN 'local_folder' THEN 'local' ELSE 'calendar' END) || char(31) || '%'),
  CHECK ((kind = 'mail_folder') = (account_id IS NOT NULL)),
  CHECK ((progress_state = 'syncing') = (progress_percent IS NOT NULL)),
  CHECK ((progress_state = 'failed') = (progress_words IS NOT NULL))
);
CREATE INDEX reply_source_by_kind ON reply_source (kind, enabled);

CREATE TABLE retrieval_settings (
  id INTEGER PRIMARY KEY NOT NULL CHECK (id = 1),
  retrieval_prompt TEXT NOT NULL CHECK (length(trim(retrieval_prompt)) > 0),
  questions_max INTEGER NOT NULL CHECK (questions_max >= 1),
  excerpts_per_question INTEGER NOT NULL CHECK (excerpts_per_question >= 1),
  excerpt_size TEXT NOT NULL CHECK (excerpt_size IN ('sentence','paragraph','section')),
  attach_date INTEGER NOT NULL CHECK (attach_date IN (0,1)),
  attach_sender INTEGER NOT NULL CHECK (attach_sender IN (0,1)),
  attach_folder INTEGER NOT NULL CHECK (attach_folder IN (0,1)),
  attach_subject INTEGER NOT NULL CHECK (attach_subject IN (0,1)),
  keep_n INTEGER NOT NULL CHECK (keep_n >= 1),
  clash_rule TEXT NOT NULL CHECK (clash_rule IN ('keep_both_dated','keep_newest','keep_oldest')),
  ai_filter_question TEXT CHECK (ai_filter_question IS NULL OR length(trim(ai_filter_question)) > 0),
  scope TEXT NOT NULL CHECK (scope IN ('this_account','all_accounts')),
  updated_at_ms INTEGER NOT NULL DEFAULT 0
);
INSERT INTO retrieval_settings (id, retrieval_prompt, questions_max, excerpts_per_question, excerpt_size, attach_date, attach_sender, attach_folder, attach_subject, keep_n, clash_rule, ai_filter_question, scope)
VALUES (1, 'From {{message}} and {{instruction}}, write up to 3 short questions to find what I need to know before I reply.', 3, 5, 'paragraph', 1, 1, 1, 1, 6, 'keep_both_dated', 'Does this excerpt help to answer?', 'this_account');

CREATE TABLE reply_voice (
  id INTEGER PRIMARY KEY NOT NULL CHECK (id = 1),
  tone_text TEXT NOT NULL DEFAULT '',
  sample_size INTEGER NOT NULL CHECK (sample_size >= 1),
  analysed_at_ms INTEGER,
  same_person_examples INTEGER NOT NULL CHECK (same_person_examples >= 0),
  updated_at_ms INTEGER NOT NULL DEFAULT 0
);
INSERT INTO reply_voice (id, tone_text, sample_size, analysed_at_ms, same_person_examples) VALUES (1, '', 100, NULL, 3);

CREATE TABLE fixed_note (
  id TEXT PRIMARY KEY NOT NULL CHECK (length(trim(id)) > 0),
  title TEXT NOT NULL CHECK (length(trim(title)) > 0),
  text TEXT NOT NULL CHECK (length(trim(text)) > 0),
  sort_order INTEGER NOT NULL DEFAULT 0
);
