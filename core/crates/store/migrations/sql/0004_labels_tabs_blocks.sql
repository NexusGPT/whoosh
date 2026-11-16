-- 0004 (delta 9): labels are the tabs. docs/DATA-MAP.md "Labels as the tabs, scope, suggestions" and the main-table rows suggestion and block.
-- Additive only; reversal: 0004_labels_tabs_blocks.down.sql (written first).
--   label             + description, pinned, scope_account_id (NULL = every mailbox), kind (builtin | owner), sort_order (among siblings)
--   label_assignment  + decided_by, rule_id, probability, decided_at_ms  (message_category folds into it)
--   rule              + action_kind, action_label_id, match_mode          (a rule can target a label)
--   block, suggestion   new tables
--   attachment_content + vector_key
-- The old tables split_tab and message_category stay as read-only history: one writer (label, label_assignment) from here on.
-- The five built-in names are the SPLIT_TABS_DEFAULT row of docs/CONSTANTS.md minus the mailbox VIPs; a shipped migration is a
-- snapshot, so the literals stay here and tests/migration_v4.rs goes red if the ledger differs: a change then needs a new migration.
-- No foreign key is declared on an ALTERed column (SQLite cannot drop such a column on reversal); triggers keep the references true.

ALTER TABLE label ADD COLUMN description TEXT NOT NULL DEFAULT '';
ALTER TABLE label ADD COLUMN pinned INTEGER NOT NULL DEFAULT 0 CHECK (pinned IN (0,1));
ALTER TABLE label ADD COLUMN scope_account_id TEXT;
ALTER TABLE label ADD COLUMN kind TEXT NOT NULL DEFAULT 'owner' CHECK (kind IN ('builtin','owner'));
ALTER TABLE label ADD COLUMN sort_order INTEGER NOT NULL DEFAULT 0 CHECK (sort_order >= 0);

ALTER TABLE label_assignment ADD COLUMN decided_by TEXT NOT NULL DEFAULT 'owner' CHECK (decided_by IN ('rule','laya','jev','owner','default'));
ALTER TABLE label_assignment ADD COLUMN rule_id TEXT;
ALTER TABLE label_assignment ADD COLUMN probability REAL CHECK (probability IS NULL OR (probability >= 0.0 AND probability <= 1.0));
ALTER TABLE label_assignment ADD COLUMN decided_at_ms INTEGER;

ALTER TABLE rule ADD COLUMN action_kind TEXT NOT NULL DEFAULT 'tab' CHECK (action_kind IN ('tab','needs_reply','vip','mute','label'));
ALTER TABLE rule ADD COLUMN action_label_id TEXT;
ALTER TABLE rule ADD COLUMN match_mode TEXT NOT NULL DEFAULT 'any' CHECK (match_mode IN ('any','all'));

ALTER TABLE attachment_content ADD COLUMN vector_key TEXT;
CREATE UNIQUE INDEX attachment_content_by_vector_key ON attachment_content (vector_key) WHERE vector_key IS NOT NULL;

-- 1 backfill: the order of the labels that exist (by name within a parent, ties by id), then room for the built-ins at the top.
UPDATE label SET sort_order = (SELECT count(*) FROM label o WHERE o.parent_id IS label.parent_id AND (o.name < label.name OR (o.name = label.name AND o.id < label.id)));
UPDATE label SET sort_order = sort_order + 5 WHERE parent_id IS NULL;
-- an owner label at the top that already has a built-in's name keeps living, renamed (the name is a sibling rule of the contract)
UPDATE label SET name = name || ' (mine)' WHERE parent_id IS NULL AND lower(name) IN ('primary','needs reply','updates','promotions','social');

INSERT OR IGNORE INTO label (id, name, parent_id, colour, account_ids_json, updated_at_ms, description, pinned, kind, sort_order) VALUES
  ('primary',    'Primary',     NULL, NULL, '[]', 0, '', 1, 'builtin', 0),
  ('needs-reply','Needs Reply', NULL, NULL, '[]', 0, '', 1, 'builtin', 1),
  ('updates',    'Updates',     NULL, NULL, '[]', 0, '', 1, 'builtin', 2),
  ('promotions', 'Promotions',  NULL, NULL, '[]', 0, '', 1, 'builtin', 3),
  ('social',     'Social',      NULL, NULL, '[]', 0, '', 0, 'builtin', 4);

-- 2 fold split_tab into the labels: a built-in tab keeps its on/off and its place; a tab on a label pins it; a tab on a sender,
-- a domain or an account becomes an owner label with one rule (the label rule format: conditions_json is an array of {"kind":..}).
UPDATE label SET pinned = (SELECT s.enabled FROM split_tab s WHERE s.id = label.id AND s.rule_kind IN ('builtin','decision_category'))
  WHERE kind = 'builtin' AND id IN (SELECT id FROM split_tab WHERE rule_kind IN ('builtin','decision_category'));
UPDATE label SET sort_order = (
    SELECT count(*) FROM split_tab o, split_tab me WHERE me.id = label.id AND o.rule_kind IN ('builtin','decision_category')
      AND o.id IN ('primary','needs-reply','updates','promotions','social')
      AND (o.sort_order < me.sort_order OR (o.sort_order = me.sort_order AND o.id < me.id)))
  WHERE kind = 'builtin' AND id IN (SELECT id FROM split_tab WHERE rule_kind IN ('builtin','decision_category'));
UPDATE label SET pinned = 1 WHERE id IN (SELECT rule_value FROM split_tab WHERE rule_kind = 'label' AND enabled = 1);

CREATE TEMP TABLE fold_base AS SELECT coalesce(max(position), -1) + 1 AS base FROM rule;
INSERT INTO label (id, name, parent_id, colour, account_ids_json, updated_at_ms, description, pinned, kind, sort_order)
  SELECT 'tab-' || s.id,
         CASE WHEN EXISTS (SELECT 1 FROM label l WHERE l.parent_id IS NULL AND lower(l.name) = lower(trim(replace(s.name, '>', '-'))))
              THEN trim(replace(s.name, '>', '-')) || ' (tab)'
              WHEN trim(replace(s.name, '>', '-')) = '' THEN 'Tab ' || s.id
              ELSE trim(replace(s.name, '>', '-')) END,
         NULL, NULL, '[]', s.updated_at_ms, '', s.enabled, 'owner',
         (SELECT count(*) FROM label t WHERE t.parent_id IS NULL) + s.sort_order
  FROM split_tab s WHERE s.rule_kind IN ('sender','domain','account');
INSERT INTO rule (id, position, conditions_json, tab_id, enabled, updated_at_ms, action_kind, action_label_id, match_mode)
  SELECT 'rule-tab-' || s.id, (SELECT base FROM fold_base) + s.sort_order,
         CASE s.rule_kind WHEN 'sender' THEN json_array(json_object('kind','from','words',s.rule_value))
                          WHEN 'domain' THEN json_array(json_object('kind','domain','domain',s.rule_value))
                          ELSE json_array(json_object('kind','account','account_id',s.rule_value)) END,
         '', 1, s.updated_at_ms, 'label', 'tab-' || s.id, 'any'
  FROM split_tab s WHERE s.rule_kind IN ('sender','domain','account');
DROP TABLE fold_base;

-- 3 fold message_category into label_assignment: the category a message was placed in becomes its filing under that built-in label.
INSERT OR IGNORE INTO label_assignment (label_id, message_id, updated_at_ms, decided_by, rule_id, probability, decided_at_ms)
  SELECT mc.tab_id, mc.message_id, mc.updated_at_ms, mc.decided_by, mc.rule_id, mc.probability, mc.decided_at_ms
  FROM message_category mc JOIN label l ON l.id = mc.tab_id AND l.kind = 'builtin';

-- 4 the rules that keep it true
CREATE TRIGGER label_assignment_rule_pair_insert BEFORE INSERT ON label_assignment
WHEN (NEW.decided_by = 'rule') <> (NEW.rule_id IS NOT NULL)
BEGIN SELECT RAISE(ABORT, 'a filing decided by a rule names the rule, and only that one does'); END;
CREATE TRIGGER label_assignment_rule_pair_update BEFORE UPDATE ON label_assignment
WHEN (NEW.decided_by = 'rule') <> (NEW.rule_id IS NOT NULL)
BEGIN SELECT RAISE(ABORT, 'a filing decided by a rule names the rule, and only that one does'); END;

CREATE TRIGGER rule_action_pair_insert BEFORE INSERT ON rule
WHEN (NEW.action_kind = 'label') <> (NEW.action_label_id IS NOT NULL)
BEGIN SELECT RAISE(ABORT, 'a rule that files into a label names the label, and only that one does'); END;
CREATE TRIGGER rule_action_pair_update BEFORE UPDATE ON rule
WHEN (NEW.action_kind = 'label') <> (NEW.action_label_id IS NOT NULL)
BEGIN SELECT RAISE(ABORT, 'a rule that files into a label names the label, and only that one does'); END;
CREATE TRIGGER rule_label_must_exist BEFORE INSERT ON rule
WHEN NEW.action_label_id IS NOT NULL AND NOT EXISTS (SELECT 1 FROM label WHERE id = NEW.action_label_id)
BEGIN SELECT RAISE(ABORT, 'a rule cannot file into a label that does not exist'); END;
CREATE TRIGGER rule_goes_with_its_label AFTER DELETE ON label
BEGIN DELETE FROM rule WHERE action_label_id = OLD.id; END;

CREATE TRIGGER label_builtin_no_delete BEFORE DELETE ON label WHEN OLD.kind = 'builtin'
BEGIN SELECT RAISE(ABORT, 'a built-in category cannot be deleted; unpin it to take it off the tabs'); END;
CREATE TRIGGER label_builtin_no_rename BEFORE UPDATE OF name ON label WHEN OLD.kind = 'builtin' AND NEW.name IS NOT OLD.name
BEGIN SELECT RAISE(ABORT, 'a built-in category cannot be renamed'); END;
CREATE TRIGGER label_builtin_stays_top BEFORE UPDATE OF parent_id ON label WHEN OLD.kind = 'builtin' AND NEW.parent_id IS NOT NULL
BEGIN SELECT RAISE(ABORT, 'a built-in category stays at the top level'); END;
CREATE TRIGGER label_kind_is_fixed BEFORE UPDATE OF kind ON label WHEN NEW.kind <> OLD.kind
BEGIN SELECT RAISE(ABORT, 'a label keeps its kind: built-in categories and owner labels do not change into each other'); END;
CREATE TRIGGER label_scope_exists BEFORE INSERT ON label
WHEN NEW.scope_account_id IS NOT NULL AND NOT EXISTS (SELECT 1 FROM account WHERE id = NEW.scope_account_id)
BEGIN SELECT RAISE(ABORT, 'a label cannot belong to a mailbox that does not exist'); END;
CREATE TRIGGER label_scope_fits_parent_insert BEFORE INSERT ON label
WHEN NEW.parent_id IS NOT NULL AND (SELECT p.scope_account_id FROM label p WHERE p.id = NEW.parent_id) IS NOT NULL
 AND NEW.scope_account_id IS NOT (SELECT p.scope_account_id FROM label p WHERE p.id = NEW.parent_id)
BEGIN SELECT RAISE(ABORT, 'the parent label belongs to one mailbox only, so this label has to belong to the same mailbox'); END;
CREATE TRIGGER label_scope_fits_parent_update BEFORE UPDATE OF parent_id, scope_account_id ON label
WHEN NEW.parent_id IS NOT NULL AND (SELECT p.scope_account_id FROM label p WHERE p.id = NEW.parent_id) IS NOT NULL
 AND NEW.scope_account_id IS NOT (SELECT p.scope_account_id FROM label p WHERE p.id = NEW.parent_id)
BEGIN SELECT RAISE(ABORT, 'the parent label belongs to one mailbox only, so this label has to belong to the same mailbox'); END;
CREATE TRIGGER label_goes_with_its_mailbox AFTER DELETE ON account
BEGIN DELETE FROM label WHERE scope_account_id = OLD.id; END;

CREATE INDEX label_by_parent_order ON label (parent_id, sort_order);
CREATE INDEX label_pinned ON label (pinned) WHERE pinned = 1;

-- 5 new tables
CREATE TABLE block (
  kind TEXT NOT NULL CHECK (kind IN ('sender','domain')),
  value TEXT NOT NULL CHECK (value = lower(value) AND length(value) > 0 AND ((kind = 'sender' AND instr(value, '@') > 1) OR (kind = 'domain' AND instr(value, '@') = 0))),
  since_ms INTEGER NOT NULL,
  PRIMARY KEY (kind, value)
);
CREATE INDEX block_by_since ON block (since_ms);

CREATE TABLE suggestion (
  id TEXT PRIMARY KEY NOT NULL,
  kind TEXT NOT NULL CHECK (kind IN ('block_sender','always_junk_domain','unsubscribe','mute_thread','always_label')),
  subject TEXT NOT NULL CHECK (length(trim(subject)) > 0),
  label_id TEXT REFERENCES label(id) ON DELETE CASCADE,
  offered_at_ms INTEGER NOT NULL,
  accepted INTEGER NOT NULL DEFAULT 0 CHECK (accepted IN (0,1)),
  CHECK ((kind = 'always_label') = (label_id IS NOT NULL))
);
CREATE INDEX suggestion_by_offered ON suggestion (offered_at_ms);
