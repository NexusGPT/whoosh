-- 0009 (delta 17): a label is a flow of three cards (Entry rules, AI sorting, Exit filter). docs/DATA-MAP.md "A label is a flow" and the
-- new record flow-rejection. The brief named this file 0005_*: 0005 to 0008 were taken, so this is the next number, 0009.
-- Additive only; reversal: 0009_label_flow.down.sql (written first). The defaults keep every existing label behaving as before:
-- entry rules on (they were always on), AI sorting on, exit filter off, the AI reads subject and body, no fallback.
--   label             + entry_enabled, ai_enabled, exit_enabled, ai_reads_subject, ai_reads_body, ai_reads_attachments, exit_fallback
--   rule              + placement ('entry' | 'exit'), chain_id (a rule record IS one chain; alternatives inside one (label, placement))
--   label_assignment  + flow_card, flow_chain_id (which card placed the message and by which chain; NULL = by hand or the default)
--   flow_rejection    new table: the AI's pick that the exit filter sent elsewhere (a message sent to Primary or Junk has no assignment)
-- No foreign key is declared on an ALTERed column (SQLite cannot drop such a column on reversal); triggers keep the references true.

ALTER TABLE label ADD COLUMN entry_enabled INTEGER NOT NULL DEFAULT 1 CHECK (entry_enabled IN (0,1));
ALTER TABLE label ADD COLUMN ai_enabled INTEGER NOT NULL DEFAULT 1 CHECK (ai_enabled IN (0,1));
ALTER TABLE label ADD COLUMN exit_enabled INTEGER NOT NULL DEFAULT 0 CHECK (exit_enabled IN (0,1));
ALTER TABLE label ADD COLUMN ai_reads_subject INTEGER NOT NULL DEFAULT 1 CHECK (ai_reads_subject IN (0,1));
ALTER TABLE label ADD COLUMN ai_reads_body INTEGER NOT NULL DEFAULT 1 CHECK (ai_reads_body IN (0,1));
ALTER TABLE label ADD COLUMN ai_reads_attachments INTEGER NOT NULL DEFAULT 0 CHECK (ai_reads_attachments IN (0,1));
ALTER TABLE label ADD COLUMN exit_fallback TEXT;

ALTER TABLE rule ADD COLUMN placement TEXT NOT NULL DEFAULT 'entry' CHECK (placement IN ('entry','exit'));
ALTER TABLE rule ADD COLUMN chain_id INTEGER NOT NULL DEFAULT 0 CHECK (chain_id >= 0);

ALTER TABLE label_assignment ADD COLUMN flow_card TEXT CHECK (flow_card IS NULL OR flow_card IN ('entry','ai','exit','fallback'));
ALTER TABLE label_assignment ADD COLUMN flow_chain_id INTEGER CHECK (flow_chain_id IS NULL OR flow_chain_id >= 0);

-- 1 backfill. Every rule that files into a label is an ENTRY chain; the chains of one label are numbered 0, 1, ... by position (ties by id),
-- which is the order the decision lane already read them in (delta 9: several rules of one label are alternatives).
UPDATE rule SET chain_id = (SELECT count(*) FROM rule o WHERE o.action_label_id = rule.action_label_id AND (o.position < rule.position OR (o.position = rule.position AND o.id < rule.id)))
  WHERE action_label_id IS NOT NULL;
-- a filing decided by a rule was placed by the entry card; by the model, by the AI card (no exit filter existed); by the owner or the default, by hand (NULL)
UPDATE label_assignment SET flow_card = 'entry', flow_chain_id = (SELECT r.chain_id FROM rule r WHERE r.id = label_assignment.rule_id) WHERE decided_by = 'rule';
UPDATE label_assignment SET flow_card = 'ai' WHERE decided_by IN ('jev','laya');

CREATE UNIQUE INDEX rule_one_chain_number ON rule (action_label_id, placement, chain_id) WHERE action_label_id IS NOT NULL;

-- 2 the rules that keep it true (the words are the contract's: label_flow.rs)
CREATE TRIGGER label_exit_needs_fallback_insert BEFORE INSERT ON label
WHEN NEW.exit_enabled = 1 AND (NEW.exit_fallback IS NULL OR length(trim(NEW.exit_fallback)) = 0)
BEGIN SELECT RAISE(ABORT, 'Say where the other messages go before you turn the exit filter on'); END;
CREATE TRIGGER label_exit_needs_fallback_update BEFORE UPDATE OF exit_enabled, exit_fallback ON label
WHEN NEW.exit_enabled = 1 AND (NEW.exit_fallback IS NULL OR length(trim(NEW.exit_fallback)) = 0)
BEGIN SELECT RAISE(ABORT, 'Say where the other messages go before you turn the exit filter on'); END;

CREATE TRIGGER label_fallback_is_a_place_insert BEFORE INSERT ON label
WHEN NEW.exit_fallback IS NOT NULL AND (NEW.exit_fallback = NEW.id OR (NEW.exit_fallback NOT IN ('primary','junk') AND NOT EXISTS (SELECT 1 FROM label WHERE id = NEW.exit_fallback)))
BEGIN SELECT RAISE(ABORT, 'the other messages go to another label, to Primary or to Junk'); END;
CREATE TRIGGER label_fallback_is_a_place_update BEFORE UPDATE OF exit_fallback ON label
WHEN NEW.exit_fallback IS NOT NULL AND (NEW.exit_fallback = NEW.id OR (NEW.exit_fallback NOT IN ('primary','junk') AND NOT EXISTS (SELECT 1 FROM label WHERE id = NEW.exit_fallback)))
BEGIN SELECT RAISE(ABORT, 'the other messages go to another label, to Primary or to Junk'); END;

CREATE TRIGGER label_ai_reads_something_insert BEFORE INSERT ON label
WHEN NEW.ai_reads_subject + NEW.ai_reads_body + NEW.ai_reads_attachments = 0
BEGIN SELECT RAISE(ABORT, 'the AI has to read at least one of: subject, body, attachments'); END;
CREATE TRIGGER label_ai_reads_something_update BEFORE UPDATE OF ai_reads_subject, ai_reads_body, ai_reads_attachments ON label
WHEN NEW.ai_reads_subject + NEW.ai_reads_body + NEW.ai_reads_attachments = 0
BEGIN SELECT RAISE(ABORT, 'the AI has to read at least one of: subject, body, attachments'); END;

-- a label that is somebody's fallback leaves with its filter: the label that sent mail there switches its exit filter off and forgets it
CREATE TRIGGER label_fallback_goes_with_its_label AFTER DELETE ON label
BEGIN UPDATE label SET exit_enabled = 0, exit_fallback = NULL WHERE exit_fallback = OLD.id; END;

-- an exit chain belongs to a label (a tab rule has no exit)
CREATE TRIGGER rule_exit_is_a_label_rule_insert BEFORE INSERT ON rule
WHEN NEW.placement = 'exit' AND NEW.action_kind <> 'label'
BEGIN SELECT RAISE(ABORT, 'an exit chain belongs to a label'); END;
CREATE TRIGGER rule_exit_is_a_label_rule_update BEFORE UPDATE OF placement, action_kind ON rule
WHEN NEW.placement = 'exit' AND NEW.action_kind <> 'label'
BEGIN SELECT RAISE(ABORT, 'an exit chain belongs to a label'); END;

-- 3 the new table
CREATE TABLE flow_rejection (
  message_id TEXT PRIMARY KEY NOT NULL REFERENCES message(id) ON DELETE CASCADE,
  ai_label_id TEXT NOT NULL REFERENCES label(id) ON DELETE CASCADE,
  sent_to TEXT NOT NULL CHECK (length(trim(sent_to)) > 0),
  chain_id INTEGER CHECK (chain_id IS NULL OR chain_id >= 0),
  decided_at_ms INTEGER NOT NULL
);
CREATE INDEX flow_rejection_by_label ON flow_rejection (ai_label_id);
