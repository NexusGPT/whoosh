-- Reversal of 0009_label_flow. Written first. Drops only what 0009 added.
-- Data lost on reversal: the three switches (a label whose entry card was OFF fires its rules again, as before delta 17), the AI inputs,
-- every fallback, every exit chain (they are deleted: before delta 17 a rule only ever filed INTO a label), the chain numbers (the
-- remaining rules stay as alternatives, as delta 9 read them), which card placed each filing, and every flow_rejection (the messages
-- the exit filter had sent elsewhere keep the place they were sent to).
DROP INDEX IF EXISTS flow_rejection_by_label;
DROP TABLE IF EXISTS flow_rejection;
DROP TRIGGER IF EXISTS rule_exit_is_a_label_rule_update;
DROP TRIGGER IF EXISTS rule_exit_is_a_label_rule_insert;
DROP TRIGGER IF EXISTS label_fallback_goes_with_its_label;
DROP TRIGGER IF EXISTS label_ai_reads_something_update;
DROP TRIGGER IF EXISTS label_ai_reads_something_insert;
DROP TRIGGER IF EXISTS label_fallback_is_a_place_update;
DROP TRIGGER IF EXISTS label_fallback_is_a_place_insert;
DROP TRIGGER IF EXISTS label_exit_needs_fallback_update;
DROP TRIGGER IF EXISTS label_exit_needs_fallback_insert;
DROP INDEX IF EXISTS rule_one_chain_number;
DELETE FROM rule WHERE placement = 'exit';
ALTER TABLE label_assignment DROP COLUMN flow_chain_id;
ALTER TABLE label_assignment DROP COLUMN flow_card;
ALTER TABLE rule DROP COLUMN chain_id;
ALTER TABLE rule DROP COLUMN placement;
ALTER TABLE label DROP COLUMN exit_fallback;
ALTER TABLE label DROP COLUMN ai_reads_attachments;
ALTER TABLE label DROP COLUMN ai_reads_body;
ALTER TABLE label DROP COLUMN ai_reads_subject;
ALTER TABLE label DROP COLUMN exit_enabled;
ALTER TABLE label DROP COLUMN ai_enabled;
ALTER TABLE label DROP COLUMN entry_enabled;
