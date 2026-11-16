-- Reversal of 0002_nested_labels. Written first. Drops only what 0002 added; parent_id belongs to v1 and stays.
-- Nothing is lost: path and depth are derived from name and parent_id, so a later re-apply rebuilds them.
DROP TRIGGER IF EXISTS label_path_after_update;
DROP TRIGGER IF EXISTS label_guard_before_update;
DROP TRIGGER IF EXISTS label_path_after_insert;
DROP TRIGGER IF EXISTS label_guard_before_insert;
DROP INDEX IF EXISTS label_by_parent;
DROP INDEX IF EXISTS label_by_path;
ALTER TABLE label DROP COLUMN depth;
ALTER TABLE label DROP COLUMN path;
