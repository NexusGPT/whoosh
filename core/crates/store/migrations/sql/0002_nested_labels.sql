-- 0002 nested labels (owner, 2026-11-07: "labelling and sublabelling (e.g., client>Provanta) and
-- sub-sub-labelling (up to like 3-5 levels)"). docs/DATA-MAP.md "## Nested labels".
-- Reversal: 0002_nested_labels.down.sql. v1 already holds label.parent_id (nullable, references label);
-- this migration adds the two derived columns, the rules that keep them true, and the indexes.
--
-- Phases, all inside the runner's one transaction (a failure leaves the database at v1):
--   1 expand   path and depth join as nullable columns (SQLite cannot add NOT NULL without a default, and a
--              default would be a lie for a derived value). ALTER ADD COLUMN is in place: no table rewrite.
--   2 backfill one set-based statement, then an assertion that no label is left without a path.
--   3 enforce  triggers derive path and depth on every insert, rename and move, and refuse a bad write in words.
--
-- The number 5 is LABEL_DEPTH_MAX of docs/CONSTANTS.md. A shipped migration is a snapshot, so the literal
-- stays here and tests/migration_v2.rs goes red if the ledger value ever differs: a change then needs a new migration.
--
-- Prefix match: the labels under P are exactly those with  path >= P || ' > '  AND  path < P || ' ?'
-- ('?' sorts one byte after '>'). That is a range on label_by_path, and unlike LIKE it reads '%' and '_' in names as text.

ALTER TABLE label ADD COLUMN path TEXT;
ALTER TABLE label ADD COLUMN depth INTEGER CHECK (depth BETWEEN 1 AND 5);

WITH RECURSIVE tree(id, path, depth) AS (
  SELECT id, name, 1 FROM label WHERE parent_id IS NULL
  UNION ALL
  SELECT c.id, t.path || ' > ' || c.name, t.depth + 1 FROM label c JOIN tree t ON c.parent_id = t.id WHERE t.depth < 5
)
UPDATE label SET
  path = (SELECT tree.path FROM tree WHERE tree.id = label.id),
  depth = (SELECT tree.depth FROM tree WHERE tree.id = label.id);

CREATE TEMP TABLE migration_check (ok INTEGER);
CREATE TEMP TRIGGER migration_check_words BEFORE INSERT ON migration_check WHEN NEW.ok <> 1
BEGIN
  SELECT RAISE(ABORT, 'nested labels cannot be added yet: some labels have an empty name, a ">" in the name, spaces at the ends of the name, a loop in their parents, or more than 5 levels; fix those labels first');
END;
INSERT INTO migration_check SELECT count(*) = 0 FROM label
  WHERE path IS NULL OR depth IS NULL OR name = '' OR instr(name, '>') > 0 OR name <> trim(name);
DROP TABLE migration_check;

CREATE INDEX label_by_path ON label (path);
CREATE INDEX label_by_parent ON label (parent_id);

CREATE TRIGGER label_guard_before_insert BEFORE INSERT ON label
BEGIN
  SELECT RAISE(ABORT, 'a label needs a name') WHERE trim(NEW.name) = '';
  SELECT RAISE(ABORT, 'a label name cannot contain ">" because it separates the levels of a path') WHERE instr(NEW.name, '>') > 0;
  SELECT RAISE(ABORT, 'a label name cannot start or end with a space') WHERE NEW.name <> trim(NEW.name);
  SELECT RAISE(ABORT, 'a label can be at most 5 levels deep; this one would be level 6')
    WHERE NEW.parent_id IS NOT NULL AND (SELECT p.depth FROM label p WHERE p.id = NEW.parent_id) >= 5;
END;

CREATE TRIGGER label_path_after_insert AFTER INSERT ON label
BEGIN
  UPDATE label SET
    path = CASE WHEN NEW.parent_id IS NULL THEN NEW.name ELSE (SELECT p.path FROM label p WHERE p.id = NEW.parent_id) || ' > ' || NEW.name END,
    depth = CASE WHEN NEW.parent_id IS NULL THEN 1 ELSE (SELECT p.depth FROM label p WHERE p.id = NEW.parent_id) + 1 END
  WHERE id = NEW.id;
END;

CREATE TRIGGER label_guard_before_update BEFORE UPDATE OF name, parent_id ON label
BEGIN
  SELECT RAISE(ABORT, 'a label needs a name') WHERE trim(NEW.name) = '';
  SELECT RAISE(ABORT, 'a label name cannot contain ">" because it separates the levels of a path') WHERE instr(NEW.name, '>') > 0;
  SELECT RAISE(ABORT, 'a label name cannot start or end with a space') WHERE NEW.name <> trim(NEW.name);
  SELECT RAISE(ABORT, 'a label cannot be moved under itself or under one of its own sub-labels')
    WHERE NEW.parent_id IS NOT OLD.parent_id AND NEW.parent_id IS NOT NULL
      AND (NEW.parent_id = OLD.id
           OR (SELECT p.path FROM label p WHERE p.id = NEW.parent_id) >= OLD.path || ' > '
              AND (SELECT p.path FROM label p WHERE p.id = NEW.parent_id) < OLD.path || ' ?');
  SELECT RAISE(ABORT, 'a label can be at most 5 levels deep; moving it with its sub-labels there would go past that')
    WHERE NEW.parent_id IS NOT OLD.parent_id AND NEW.parent_id IS NOT NULL
      AND (SELECT p.depth FROM label p WHERE p.id = NEW.parent_id)
          + (SELECT max(d.depth) FROM label d WHERE d.id = OLD.id OR (d.path >= OLD.path || ' > ' AND d.path < OLD.path || ' ?'))
          - OLD.depth + 1 > 5;
END;

-- Rename or move: the label's own path and every sub-label's path are rewritten in the same statement.
CREATE TRIGGER label_path_after_update AFTER UPDATE OF name, parent_id ON label
WHEN NEW.name IS NOT OLD.name OR NEW.parent_id IS NOT OLD.parent_id
BEGIN
  UPDATE label SET
    path = CASE WHEN NEW.parent_id IS NULL THEN NEW.name ELSE (SELECT p.path FROM label p WHERE p.id = NEW.parent_id) || ' > ' || NEW.name END,
    depth = CASE WHEN NEW.parent_id IS NULL THEN 1 ELSE (SELECT p.depth FROM label p WHERE p.id = NEW.parent_id) + 1 END
  WHERE id = NEW.id;
  UPDATE label SET
    path = (SELECT n.path FROM label n WHERE n.id = NEW.id) || substr(path, length(OLD.path) + 1),
    depth = depth + ((SELECT n.depth FROM label n WHERE n.id = NEW.id) - OLD.depth)
  WHERE path >= OLD.path || ' > ' AND path < OLD.path || ' ?';
END;
