-- Reversal of 0004_labels_tabs_blocks. Written first. Drops only what 0004 added.
-- Data lost on reversal: pins, descriptions, scopes and the order of labels; every rule that files into a label; the built-in
-- category labels and the filings under them (message_category still holds each message's category); the owner labels that
-- 0004 made out of tabs ('tab-*'); every block and suggestion; the vector keys of attachments (the vector crate re-keys on its next run).
-- split_tab and message_category were never changed, so the pre-0004 tabs and categories come back as they were last written.
DROP TRIGGER IF EXISTS label_goes_with_its_mailbox;
DROP TRIGGER IF EXISTS label_scope_fits_parent_update;
DROP TRIGGER IF EXISTS label_scope_fits_parent_insert;
DROP TRIGGER IF EXISTS label_scope_exists;
DROP TRIGGER IF EXISTS label_kind_is_fixed;
DROP TRIGGER IF EXISTS label_builtin_stays_top;
DROP TRIGGER IF EXISTS label_builtin_no_rename;
DROP TRIGGER IF EXISTS label_builtin_no_delete;
DROP TRIGGER IF EXISTS rule_goes_with_its_label;
DROP TRIGGER IF EXISTS rule_label_must_exist;
DROP TRIGGER IF EXISTS rule_action_pair_update;
DROP TRIGGER IF EXISTS rule_action_pair_insert;
DROP TRIGGER IF EXISTS label_assignment_rule_pair_update;
DROP TRIGGER IF EXISTS label_assignment_rule_pair_insert;
DROP INDEX IF EXISTS suggestion_by_offered;
DROP TABLE IF EXISTS suggestion;
DROP INDEX IF EXISTS block_by_since;
DROP TABLE IF EXISTS block;
DROP INDEX IF EXISTS label_pinned;
DROP INDEX IF EXISTS label_by_parent_order;
DROP INDEX IF EXISTS attachment_content_by_vector_key;
DELETE FROM rule WHERE action_kind = 'label';
DELETE FROM label_assignment WHERE label_id IN (SELECT id FROM label WHERE kind = 'builtin');
DELETE FROM label WHERE kind = 'builtin' OR id LIKE 'tab-%';
ALTER TABLE attachment_content DROP COLUMN vector_key;
ALTER TABLE rule DROP COLUMN match_mode;
ALTER TABLE rule DROP COLUMN action_label_id;
ALTER TABLE rule DROP COLUMN action_kind;
ALTER TABLE label_assignment DROP COLUMN decided_at_ms;
ALTER TABLE label_assignment DROP COLUMN probability;
ALTER TABLE label_assignment DROP COLUMN rule_id;
ALTER TABLE label_assignment DROP COLUMN decided_by;
ALTER TABLE label DROP COLUMN sort_order;
ALTER TABLE label DROP COLUMN kind;
ALTER TABLE label DROP COLUMN scope_account_id;
ALTER TABLE label DROP COLUMN pinned;
ALTER TABLE label DROP COLUMN description;
