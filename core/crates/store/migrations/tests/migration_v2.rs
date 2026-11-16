//! Migration 0002 (nested labels): path and depth derived from name and parent_id, a sixth level refused in
//! words, rename and move rewriting every descendant, the descendant query, the backfill, and the reversal.
//! The depth limit is read from docs/CONSTANTS.md (LABEL_DEPTH_MAX); no limit is typed in this file.
use rusqlite::{named_params, params, Connection};
use std::collections::BTreeSet;
use store_migrations::{migrate, revert_to, schema_hash, schema_version, MIGRATIONS};

fn ledger_depth_max() -> usize {
    let p = format!("{}/../../../../docs/CONSTANTS.md", env!("CARGO_MANIFEST_DIR"));
    let text = std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("cannot read {p}: {e}"));
    let line = text.lines().find(|l| l.starts_with("| LABEL_DEPTH_MAX |")).expect("CONSTANTS.md has no LABEL_DEPTH_MAX row");
    line.split('|').nth(2).unwrap().trim().parse().expect("LABEL_DEPTH_MAX is not a number")
}

fn fresh() -> Connection {
    let mut c = Connection::open_in_memory().unwrap();
    migrate(&mut c).unwrap();
    c
}

/// A database exactly as v1 left it (no path, no depth).
fn at_v1() -> Connection {
    let c = Connection::open_in_memory().unwrap();
    c.pragma_update(None, "foreign_keys", true).unwrap();
    c.execute_batch(MIGRATIONS[0].1).unwrap();
    c.pragma_update(None, "user_version", 1).unwrap();
    c
}

fn add_label(c: &Connection, id: &str, name: &str, parent: Option<&str>) -> rusqlite::Result<usize> {
    c.execute("INSERT INTO label (id, name, parent_id, updated_at_ms) VALUES (?1, ?2, ?3, 1)", params![id, name, parent])
}

fn path_of(c: &Connection, id: &str) -> (String, i64) {
    c.query_row("SELECT path, depth FROM label WHERE id = ?1", [id], |r| Ok((r.get(0)?, r.get(1)?))).unwrap()
}

/// Every label's stored path and depth equal what its names and parents say. Zero wrong rows.
fn assert_paths_true(c: &Connection) {
    let wrong: i64 = c
        .query_row(
            "WITH RECURSIVE t(id, path, depth) AS (
               SELECT id, name, 1 FROM label WHERE parent_id IS NULL
               UNION ALL SELECT c.id, t.path || ' > ' || c.name, t.depth + 1 FROM label c JOIN t ON c.parent_id = t.id)
             SELECT count(*) FROM label l LEFT JOIN t ON t.id = l.id WHERE t.id IS NULL OR l.path IS NOT t.path OR l.depth IS NOT t.depth",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(wrong, 0, "labels whose stored path or depth disagree with their names and parents");
}

fn refusal(r: rusqlite::Result<usize>) -> String {
    match r {
        Ok(_) => panic!("the write was accepted but must be refused"),
        Err(e) => e.to_string(),
    }
}

/// Builds a chain l1 > l2 > ... > l{n} and returns the ids.
fn chain(c: &Connection, n: usize) -> Vec<String> {
    let mut ids: Vec<String> = Vec::new();
    for i in 1..=n {
        let id = format!("l{i}");
        add_label(c, &id, &format!("Level{i}"), ids.last().map(String::as_str)).unwrap();
        ids.push(id);
    }
    ids
}

#[test]
fn the_columns_and_indexes_exist_and_the_schema_is_v2() {
    let c = fresh();
    assert_eq!(schema_version(&c).unwrap(), store_migrations::latest_version());   // v2 and every later migration
    let cols: Vec<String> = c.prepare("PRAGMA table_info(label)").unwrap().query_map([], |r| r.get::<_, String>(1)).unwrap().map(Result::unwrap).collect();
    for want in ["parent_id", "path", "depth"] {
        assert!(cols.contains(&want.to_string()), "label has no {want} column: {cols:?}");
    }
    let idx: BTreeSet<String> = c.prepare("PRAGMA index_list(label)").unwrap().query_map([], |r| r.get::<_, String>(1)).unwrap().map(Result::unwrap).collect();
    assert!(idx.contains("label_by_path") && idx.contains("label_by_parent"), "indexes: {idx:?}");
}

#[test]
fn path_and_depth_are_derived_on_insert() {
    let c = fresh();
    add_label(&c, "c", "Clients", None).unwrap();
    add_label(&c, "p", "Provanta", Some("c")).unwrap();
    add_label(&c, "r", "RFP", Some("p")).unwrap();
    assert_eq!(path_of(&c, "c"), ("Clients".into(), 1));
    assert_eq!(path_of(&c, "p"), ("Clients > Provanta".into(), 2));
    assert_eq!(path_of(&c, "r"), ("Clients > Provanta > RFP".into(), 3));
    // a caller cannot choose the path: it is overwritten by the derivation
    c.execute("INSERT INTO label (id, name, parent_id, path, depth, updated_at_ms) VALUES ('x','X',NULL,'Forged',1,1)", []).unwrap();
    assert_eq!(path_of(&c, "x"), ("X".into(), 1));
    assert_paths_true(&c);
}

#[test]
fn the_limit_levels_are_accepted_and_the_next_one_is_refused_in_words() {
    let max = ledger_depth_max();
    let c = fresh();
    let ids = chain(&c, max);
    assert_eq!(path_of(&c, &ids[max - 1]).1 as usize, max);
    let words = refusal(add_label(&c, "too-deep", "One too many", Some(&ids[max - 1])));
    assert!(words.contains(&format!("at most {max} levels deep")), "refusal must say the limit in words, got: {words}");
    let n: i64 = c.query_row("SELECT count(*) FROM label WHERE id = 'too-deep'", [], |r| r.get(0)).unwrap();
    assert_eq!(n, 0, "the refused label must not exist");
    assert_paths_true(&c);
}

#[test]
fn names_that_would_break_a_path_are_refused_in_words() {
    let c = fresh();
    assert!(refusal(add_label(&c, "a", "Clients > Provanta", None)).contains("cannot contain \">\""));
    assert!(refusal(add_label(&c, "b", "   ", None)).contains("needs a name"));
    assert!(refusal(add_label(&c, "d", " Padded", None)).contains("start or end with a space"));
    add_label(&c, "ok", "Fine", None).unwrap();
    assert!(refusal(c.execute("UPDATE label SET name = 'A > B' WHERE id = 'ok'", [])).contains("cannot contain \">\""));
}

#[test]
fn renaming_a_parent_rewrites_every_descendant_and_touches_no_one_else() {
    let c = fresh();
    add_label(&c, "c", "Clients", None).unwrap();
    add_label(&c, "p", "Provanta", Some("c")).unwrap();
    add_label(&c, "r", "RFP", Some("p")).unwrap();
    add_label(&c, "q", "2026", Some("r")).unwrap();
    add_label(&c, "o", "Oranta", Some("c")).unwrap();
    add_label(&c, "c2", "Clients 2", None).unwrap(); // shares the text prefix "Clients", is not a descendant
    add_label(&c, "pct", "Clients%", None).unwrap(); // a LIKE wildcard in a name
    add_label(&c, "under", "Clients_", None).unwrap();
    c.execute("UPDATE label SET name = 'Accounts' WHERE id = 'c'", []).unwrap();
    assert_eq!(path_of(&c, "c"), ("Accounts".into(), 1));
    assert_eq!(path_of(&c, "p").0, "Accounts > Provanta");
    assert_eq!(path_of(&c, "r").0, "Accounts > Provanta > RFP");
    assert_eq!(path_of(&c, "q").0, "Accounts > Provanta > RFP > 2026");
    assert_eq!(path_of(&c, "o").0, "Accounts > Oranta");
    for (id, want) in [("c2", "Clients 2"), ("pct", "Clients%"), ("under", "Clients_")] {
        assert_eq!(path_of(&c, id).0, want, "{id} is not a descendant and must not change");
    }
    // renaming a middle label rewrites only what is under it
    c.execute("UPDATE label SET name = 'Tender' WHERE id = 'r'", []).unwrap();
    assert_eq!(path_of(&c, "q").0, "Accounts > Provanta > Tender > 2026");
    assert_eq!(path_of(&c, "o").0, "Accounts > Oranta");
    assert_paths_true(&c);
}

#[test]
fn moving_a_label_carries_its_sub_labels_and_refuses_loops_and_overflow() {
    let max = ledger_depth_max();
    let c = fresh();
    add_label(&c, "a", "A", None).unwrap();
    add_label(&c, "b", "B", Some("a")).unwrap();
    add_label(&c, "c", "C", Some("b")).unwrap();
    add_label(&c, "z", "Z", None).unwrap();
    // move B (with C) under Z, then to the root
    c.execute("UPDATE label SET parent_id = 'z' WHERE id = 'b'", []).unwrap();
    assert_eq!(path_of(&c, "c"), ("Z > B > C".into(), 3));
    c.execute("UPDATE label SET parent_id = NULL WHERE id = 'b'", []).unwrap();
    assert_eq!(path_of(&c, "b"), ("B".into(), 1));
    assert_eq!(path_of(&c, "c"), ("B > C".into(), 2));
    // loops
    assert!(refusal(c.execute("UPDATE label SET parent_id = 'c' WHERE id = 'b'", [])).contains("under itself or under one of its own sub-labels"));
    assert!(refusal(c.execute("UPDATE label SET parent_id = 'b' WHERE id = 'b'", [])).contains("under itself"));
    // overflow: a chain of the limit, then a two-level subtree moved under its last label
    let ids = chain(&c, max);
    assert!(refusal(c.execute("UPDATE label SET parent_id = ?1 WHERE id = 'b'", [&ids[max - 1]])).contains(&format!("at most {max} levels deep")));
    // a subtree that still fits moves: B (2 levels) under the label at depth max-2
    c.execute("UPDATE label SET parent_id = ?1 WHERE id = 'b'", [&ids[max - 3]]).unwrap();
    assert_eq!(path_of(&c, "c").1 as usize, max);
    assert_paths_true(&c);
}

#[test]
fn deleting_a_parent_promotes_its_children_with_true_paths() {
    let c = fresh();
    add_label(&c, "a", "A", None).unwrap();
    add_label(&c, "b", "B", Some("a")).unwrap();
    add_label(&c, "c", "C", Some("b")).unwrap();
    c.execute("DELETE FROM label WHERE id = 'a'", []).unwrap();
    assert_eq!(path_of(&c, "b"), ("B".into(), 1));
    assert_eq!(path_of(&c, "c"), ("B > C".into(), 2));
    assert_paths_true(&c);
}

fn add_account(c: &Connection) {
    c.execute(
        "INSERT INTO account (id, address, provider, display_name, colour, sort_order, credential_kind, keychain_ref, updated_at_ms)
         VALUES ('a1','a1@example.test','google','Work','#336699',0,'oauth', 'whoosh' || char(31) || 'a1', 1)",
        [],
    )
    .unwrap();
}

fn add_message_with_label(c: &Connection, id: &str, label: &str) {
    c.execute("INSERT INTO thread (id, account_id, subject) VALUES (?1, 'a1', 's')", [format!("t-{id}")]).unwrap();
    c.execute(
        "INSERT INTO message (id, thread_id, account_id, sender_name, from_address, subject, received_at_ms, folder)
         VALUES (?1, ?2, 'a1', 'A', 'a@example.test', 's', 1, 'INBOX')",
        params![id, format!("t-{id}")],
    )
    .unwrap();
    c.execute("INSERT INTO label_assignment (label_id, message_id, updated_at_ms) VALUES (?1, ?2, 1)", params![label, id]).unwrap();
}

/// FINDING for core-store: the exact SQL. Messages carrying the label or any label under it.
pub const MESSAGES_UNDER_LABEL: &str = "SELECT DISTINCT la.message_id FROM label_assignment la WHERE la.label_id IN (
    SELECT :id UNION ALL
    SELECT d.id FROM label t JOIN label d ON d.path >= t.path || ' > ' AND d.path < t.path || ' ?' WHERE t.id = :id)";

fn messages_under(c: &Connection, label: &str) -> BTreeSet<String> {
    let mut s = c.prepare(MESSAGES_UNDER_LABEL).unwrap();
    let rows = s.query_map(named_params! {":id": label}, |r| r.get::<_, String>(0)).unwrap();
    rows.map(Result::unwrap).collect()
}

#[test]
fn a_label_query_returns_the_messages_under_every_descendant() {
    let c = fresh();
    add_account(&c);
    add_label(&c, "c", "Clients", None).unwrap();
    add_label(&c, "p", "Provanta", Some("c")).unwrap();
    add_label(&c, "r", "RFP", Some("p")).unwrap();
    add_label(&c, "o", "Oranta", Some("c")).unwrap();
    add_label(&c, "c2", "Clients 2", None).unwrap();
    add_label(&c, "pct", "Clients%", None).unwrap();
    add_label(&c, "cx", "Clientsx", None).unwrap();
    for (m, l) in [("m-c", "c"), ("m-p", "p"), ("m-r", "r"), ("m-o", "o"), ("m-c2", "c2"), ("m-pct", "pct"), ("m-cx", "cx")] {
        add_message_with_label(&c, m, l);
    }
    c.execute("INSERT INTO label_assignment (label_id, message_id, updated_at_ms) VALUES ('p', 'm-r', 1)", []).unwrap(); // m-r carries two labels of one branch
    let all: BTreeSet<String> = ["m-c", "m-p", "m-r", "m-o"].iter().map(|s| s.to_string()).collect();
    assert_eq!(messages_under(&c, "c"), all, "the root finds every message in its tree and none from look-alike names");
    assert_eq!(messages_under(&c, "p"), ["m-p", "m-r"].iter().map(|s| s.to_string()).collect());
    assert_eq!(messages_under(&c, "r"), ["m-r"].iter().map(|s| s.to_string()).collect());
    assert_eq!(messages_under(&c, "o"), ["m-o"].iter().map(|s| s.to_string()).collect());
    // after a rename the same query still follows the tree
    c.execute("UPDATE label SET name = 'Accounts' WHERE id = 'c'", []).unwrap();
    assert_eq!(messages_under(&c, "c"), all);
}

#[test]
fn the_descendant_query_reads_the_path_index_and_never_walks_every_message() {
    let c = fresh();
    let plan: Vec<String> = c
        .prepare(&format!("EXPLAIN QUERY PLAN {MESSAGES_UNDER_LABEL}"))
        .unwrap()
        .query_map(named_params! {":id": "x"}, |r| r.get::<_, String>(3))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    let text = plan.join("\n");
    assert!(text.contains("label_by_path"), "the prefix match must use the path index:\n{text}");
    assert!(!text.contains("SCAN d") && !text.contains("SCAN label"), "no full label scan:\n{text}");
    assert!(!text.contains("SCAN message"), "never a walk over all messages:\n{text}");
}

#[test]
fn migrating_a_v1_database_backfills_paths() {
    let c = at_v1();
    for (id, name, parent) in [("c", "Clients", None), ("p", "Provanta", Some("c")), ("r", "RFP", Some("p")), ("t", "Team", None)] {
        add_label(&c, id, name, parent).unwrap();
    }
    let mut c = c;
    let applied = migrate(&mut c).unwrap();
    assert_eq!((applied.from_version, applied.to_version), (1, store_migrations::latest_version()));
    assert_eq!(path_of(&c, "r"), ("Clients > Provanta > RFP".into(), 3));
    assert_eq!(path_of(&c, "t"), ("Team".into(), 1));
    assert_paths_true(&c);
    let remaining: i64 = c.query_row("SELECT count(*) FROM label WHERE path IS NULL OR depth IS NULL", [], |r| r.get(0)).unwrap();
    assert_eq!(remaining, 0, "backfill must leave no label without a path");
}

#[test]
fn a_v1_database_that_cannot_be_backfilled_stays_at_v1_and_says_why() {
    let max = ledger_depth_max();
    // too deep
    let mut c = at_v1();
    let mut parent: Option<String> = None;
    for i in 1..=max + 1 {
        let id = format!("d{i}");
        add_label(&c, &id, &format!("L{i}"), parent.as_deref()).unwrap();
        parent = Some(id);
    }
    // a loop and a name with the separator, each in its own database
    let mut looped = at_v1();
    looped.execute_batch("PRAGMA foreign_keys = OFF; INSERT INTO label (id, name, parent_id, updated_at_ms) VALUES ('x','X','y',1), ('y','Y','x',1);").unwrap();
    let mut gt = at_v1();
    add_label(&gt, "g", "A > B", None).unwrap();
    for (what, db) in [("too deep", &mut c), ("a loop", &mut looped), ("a name with >", &mut gt)] {
        let err = migrate(db).expect_err(what).to_string();
        assert!(err.contains("nested labels cannot be added yet"), "{what}: {err}");
        assert_eq!(schema_version(db).unwrap(), 1, "{what}: the database must stay at v1");
        let cols: Vec<String> = db.prepare("PRAGMA table_info(label)").unwrap().query_map([], |r| r.get::<_, String>(1)).unwrap().map(Result::unwrap).collect();
        assert!(!cols.contains(&"path".to_string()), "{what}: the failed migration must leave no path column");
    }
}

#[test]
fn the_reversal_restores_v1_keeps_every_label_and_reapplies_to_the_same_paths() {
    let mut c = fresh();
    add_account(&c);
    add_label(&c, "c", "Clients", None).unwrap();
    add_label(&c, "p", "Provanta", Some("c")).unwrap();
    add_message_with_label(&c, "m1", "p");
    let rows = |c: &Connection| -> Vec<(String, String, Option<String>)> {
        c.prepare("SELECT id, name, parent_id FROM label ORDER BY id").unwrap().query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).unwrap().map(Result::unwrap).collect()
    };
    // The owner's labels must all survive. The built-in category labels are 0004's own seeded rows, and its
    // reversal drops them by design (0004_labels_tabs_blocks.down.sql, "Data lost on reversal"), so they are
    // left out of the before-set and checked gone instead.
    let builtins: BTreeSet<String> = c.prepare("SELECT id FROM label WHERE kind = 'builtin'").unwrap().query_map([], |r| r.get(0)).unwrap().map(Result::unwrap).collect();
    assert!(!builtins.is_empty(), "the built-in labels are seeded at the latest version");
    let before: Vec<_> = rows(&c).into_iter().filter(|(id, _, _)| !builtins.contains(id)).collect();
    assert_eq!(before.len(), 2, "the owner's two labels are in the before-set");
    let applied = revert_to(&mut c, 1).unwrap();
    assert_eq!((applied.from_version, applied.to_version), (store_migrations::latest_version(), 1));
    assert_eq!(schema_version(&c).unwrap(), 1);
    assert_eq!(rows(&c), before, "no owner label lost, and the built-ins 0004 seeded are gone with it");
    let v1 = at_v1();
    let objects = |c: &Connection| -> Vec<(String, String)> {
        c.prepare("SELECT type, name FROM sqlite_master WHERE name NOT LIKE 'sqlite_%' ORDER BY type, name").unwrap().query_map([], |r| Ok((r.get(0)?, r.get(1)?))).unwrap().map(Result::unwrap).collect()
    };
    assert_eq!(objects(&c), objects(&v1), "after the reversal the schema objects are exactly v1's");
    let cols = |c: &Connection| -> Vec<String> { c.prepare("PRAGMA table_info(label)").unwrap().query_map([], |r| r.get::<_, String>(1)).unwrap().map(Result::unwrap).collect() };
    assert_eq!(cols(&c), cols(&v1));
    let kept: i64 = c.query_row("SELECT count(*) FROM label_assignment", [], |r| r.get(0)).unwrap();
    assert_eq!(kept, 1, "assignments untouched");
    eprintln!("schema hash after reversal equals a fresh v1: {}", schema_hash(&c).unwrap() == schema_hash(&v1).unwrap());
    // re-apply: same paths
    migrate(&mut c).unwrap();
    assert_eq!(path_of(&c, "p"), ("Clients > Provanta".into(), 2));
    assert_paths_true(&c);
    // a second revert to the same level changes nothing
    revert_to(&mut c, 1).unwrap();
    assert!(!revert_to(&mut c, 1).unwrap().changed_anything());
}
