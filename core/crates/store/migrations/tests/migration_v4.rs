//! Migration 0004 (delta 9): labels are the tabs, label rules, suggestions and blocks, the attachment vector key.
//! One test per table or column group, each with the case that breaks it, plus the fold of an existing database and the reversal.
use rusqlite::{params, Connection};
use store_migrations::{migrate, revert_to, schema_version, MIGRATIONS};

const CONSTANTS: &str = include_str!("../../../../../docs/CONSTANTS.md");

fn fresh() -> Connection {
    let mut c = Connection::open_in_memory().unwrap();
    c.pragma_update(None, "foreign_keys", true).unwrap();
    migrate(&mut c).unwrap();
    c
}

/// A database exactly as 0003 left it, so the fold has something to fold.
fn at_v3() -> Connection {
    let c = Connection::open_in_memory().unwrap();
    c.pragma_update(None, "foreign_keys", true).unwrap();
    for (_, sql) in &MIGRATIONS[..3] { c.execute_batch(sql).unwrap(); }
    c.pragma_update(None, "user_version", 3).unwrap();
    c
}

fn account(c: &Connection, id: &str) {
    c.execute("INSERT INTO account (id, address, provider, display_name, colour, sort_order, credential_kind, keychain_ref, updated_at_ms) VALUES (?1, ?2, 'google', 'Work', '#336699', 0, 'oauth', ?3, 1)",
        params![id, format!("{id}@example.test"), format!("whoosh\u{1f}{id}")]).unwrap();
}
fn message(c: &Connection, id: &str, acc: &str) {
    c.execute("INSERT INTO thread (id, account_id, subject) VALUES (?1, ?2, 's')", params![format!("t-{id}"), acc]).unwrap();
    c.execute("INSERT INTO message (id, thread_id, account_id, sender_name, from_address, subject, received_at_ms, folder) VALUES (?1, ?2, ?3, 'A', 'a@example.test', 's', 1, 'INBOX')", params![id, format!("t-{id}"), acc]).unwrap();
}
fn refused(r: rusqlite::Result<usize>) -> String { match r { Ok(_) => panic!("accepted but must be refused"), Err(e) => e.to_string() } }
fn one<T: rusqlite::types::FromSql>(c: &Connection, sql: &str) -> T { c.query_row(sql, [], |r| r.get(0)).unwrap() }
fn owner_label(c: &Connection, id: &str, name: &str, parent: Option<&str>, scope: Option<&str>) -> rusqlite::Result<usize> {
    c.execute("INSERT INTO label (id, name, parent_id, updated_at_ms, scope_account_id) VALUES (?1, ?2, ?3, 1, ?4)", params![id, name, parent, scope])
}

#[test]
fn the_built_in_categories_come_from_the_ledger_minus_the_mailbox_and_all_but_social_are_pinned() {
    let c = fresh();
    assert_eq!(schema_version(&c).unwrap(), store_migrations::latest_version());
    let ledger: Vec<String> = CONSTANTS.lines().find(|l| l.trim_start().starts_with("| SPLIT_TABS_DEFAULT |")).unwrap().split('|').nth(2).unwrap()
        .split(',').map(|n| n.trim().to_string()).filter(|n| n != "VIPs").collect();
    let rows: Vec<(String, String, i64, i64)> = c.prepare("SELECT id, name, pinned, sort_order FROM label WHERE kind = 'builtin' ORDER BY sort_order").unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))).unwrap().map(Result::unwrap).collect();
    assert_eq!(rows.iter().map(|r| r.1.clone()).collect::<Vec<_>>(), ledger, "the migration's literals equal the ledger row");
    assert_eq!(rows.iter().map(|r| r.0.as_str()).collect::<Vec<_>>(), ["primary", "needs-reply", "updates", "promotions", "social"]);
    assert_eq!(rows.iter().map(|r| r.2).collect::<Vec<_>>(), [1, 1, 1, 1, 0]);
    assert_eq!(one::<String>(&c, "SELECT path FROM label WHERE id = 'needs-reply'"), "Needs Reply", "the 0002 triggers derived the path");
}

#[test]
fn a_built_in_cannot_be_deleted_renamed_moved_or_turned_into_an_owner_label() {
    let c = fresh();
    owner_label(&c, "w", "Work", None, None).unwrap();
    assert!(refused(c.execute("DELETE FROM label WHERE id = 'primary'", [])).contains("cannot be deleted"));
    assert!(refused(c.execute("UPDATE label SET name = 'Main' WHERE id = 'primary'", [])).contains("cannot be renamed"));
    assert!(refused(c.execute("UPDATE label SET parent_id = 'w' WHERE id = 'primary'", [])).contains("top level"));
    assert!(refused(c.execute("UPDATE label SET kind = 'owner' WHERE id = 'primary'", [])).contains("keeps its kind"));
    c.execute("UPDATE label SET pinned = 0, colour = '#112233', description = 'mail that needs me first' WHERE id = 'primary'", []).unwrap();
    assert_eq!(one::<i64>(&c, "SELECT count(*) FROM label WHERE kind = 'builtin'"), 5, "unpinning, colour and the meaning stay editable");
}

#[test]
fn a_label_belongs_to_all_mailboxes_or_one_and_a_child_keeps_its_parents_mailbox() {
    let c = fresh();
    account(&c, "a1"); account(&c, "a2");
    owner_label(&c, "shared", "Shared", None, None).unwrap();
    owner_label(&c, "only1", "Only one", None, Some("a1")).unwrap();
    assert!(refused(owner_label(&c, "bad", "Bad", None, Some("ghost"))).contains("does not exist"));
    assert!(refused(owner_label(&c, "kid2", "Kid", Some("only1"), Some("a2"))).contains("same mailbox"));
    assert!(refused(owner_label(&c, "kid0", "Kid", Some("only1"), None)).contains("same mailbox"));
    owner_label(&c, "kid1", "Kid", Some("only1"), Some("a1")).unwrap();
    owner_label(&c, "under_shared", "Under shared", Some("shared"), Some("a2")).unwrap();
    assert!(refused(c.execute("UPDATE label SET scope_account_id = 'a2' WHERE id = 'kid1'", [])).contains("same mailbox"));
    c.execute("DELETE FROM account WHERE id = 'a1'", []).unwrap();
    assert_eq!(one::<i64>(&c, "SELECT count(*) FROM label WHERE id IN ('only1','kid1')"), 0, "a mailbox's own labels go with it");
    assert_eq!(one::<i64>(&c, "SELECT count(*) FROM label WHERE id IN ('shared','under_shared')"), 2);
}

#[test]
fn a_filing_decided_by_a_rule_names_the_rule_and_the_old_default_is_the_owner() {
    let c = fresh();
    account(&c, "a1"); message(&c, "m1", "a1");
    owner_label(&c, "w", "Work", None, None).unwrap();
    c.execute("INSERT INTO label_assignment (label_id, message_id, updated_at_ms) VALUES ('w', 'm1', 1)", []).unwrap();
    assert_eq!(one::<String>(&c, "SELECT decided_by FROM label_assignment"), "owner", "a writer from before 0004 files as the owner");
    assert!(refused(c.execute("INSERT INTO label_assignment (label_id, message_id, updated_at_ms, decided_by) VALUES ('primary', 'm1', 1, 'rule')", [])).contains("names the rule"));
    assert!(refused(c.execute("INSERT INTO label_assignment (label_id, message_id, updated_at_ms, decided_by, rule_id) VALUES ('primary', 'm1', 1, 'laya', 'r1')", [])).contains("names the rule"));
    c.execute("INSERT INTO label_assignment (label_id, message_id, updated_at_ms, decided_by, rule_id, probability) VALUES ('primary', 'm1', 1, 'rule', 'r1', NULL)", []).unwrap();
    c.execute("INSERT INTO label_assignment (label_id, message_id, updated_at_ms, decided_by, probability) VALUES ('updates', 'm1', 1, 'jev', 0.8)", []).unwrap();
    assert!(refused(c.execute("UPDATE label_assignment SET probability = 1.5 WHERE label_id = 'updates'", [])).to_lowercase().contains("check"));
}

#[test]
fn a_rule_can_file_into_a_label_and_goes_with_it() {
    let c = fresh();
    owner_label(&c, "w", "Work", None, None).unwrap();
    let ins = |id: &str, action: &str, label: Option<&str>| c.execute(
        "INSERT INTO rule (id, position, conditions_json, tab_id, enabled, updated_at_ms, action_kind, action_label_id, match_mode) VALUES (?1, 0, '[]', '', 1, 1, ?2, ?3, 'all')", params![id, action, label]);
    assert!(refused(ins("r1", "label", None)).contains("names the label"));
    assert!(refused(ins("r2", "mute", Some("w"))).contains("names the label"));
    assert!(refused(ins("r3", "label", Some("ghost"))).contains("does not exist"));
    ins("r4", "label", Some("w")).unwrap();
    c.execute("INSERT INTO rule (id, position, conditions_json, tab_id, updated_at_ms) VALUES ('old', 1, '[]', 'updates', 1)", []).unwrap();
    assert_eq!(one::<String>(&c, "SELECT action_kind || '/' || match_mode FROM rule WHERE id = 'old'"), "tab/any", "a v1 rule keeps its meaning");
    assert!(c.execute("INSERT INTO rule (id, position, conditions_json, tab_id, updated_at_ms, match_mode) VALUES ('x', 2, '[]', '', 1, 'some')", []).is_err());
    c.execute("DELETE FROM label WHERE id = 'w'", []).unwrap();
    assert_eq!(one::<i64>(&c, "SELECT count(*) FROM rule WHERE id = 'r4'"), 0, "deleting a label deletes the rules that file into it");
}

#[test]
fn a_block_is_a_lowercase_sender_or_domain_once() {
    let c = fresh();
    let ins = |k: &str, v: &str| c.execute("INSERT INTO block (kind, value, since_ms) VALUES (?1, ?2, 5)", params![k, v]);
    ins("sender", "spam@x.com").unwrap();
    ins("domain", "bad.example").unwrap();
    assert!(refused(ins("sender", "spam@x.com")).to_lowercase().contains("unique"));
    assert!(ins("sender", "Spam@x.com").is_err(), "lowercase only");
    assert!(ins("sender", "bad.example").is_err() && ins("domain", "a@bad.example").is_err() && ins("sender", "@x.com").is_err(), "a sender has an @ after a name, a domain has none");
    assert!(ins("host", "x.com").is_err());
}

#[test]
fn a_suggestion_names_its_label_only_for_always_label_and_goes_with_the_label() {
    let c = fresh();
    owner_label(&c, "w", "Work", None, None).unwrap();
    let ins = |id: &str, kind: &str, subj: &str, label: Option<&str>| c.execute("INSERT INTO suggestion (id, kind, subject, label_id, offered_at_ms) VALUES (?1, ?2, ?3, ?4, 9)", params![id, kind, subj, label]);
    ins("s1", "block_sender", "a@x.com", None).unwrap();
    ins("s2", "always_label", "a@x.com", Some("w")).unwrap();
    assert!(ins("s3", "always_label", "a@x.com", None).is_err() && ins("s4", "mute_thread", "t1", Some("w")).is_err());
    assert!(ins("s5", "block_sender", "  ", None).is_err() && ins("s6", "shout", "x", None).is_err());
    assert_eq!(one::<i64>(&c, "SELECT accepted FROM suggestion WHERE id = 's1'"), 0);
    c.execute("DELETE FROM label WHERE id = 'w'", []).unwrap();
    assert_eq!(one::<i64>(&c, "SELECT count(*) FROM suggestion WHERE id = 's2'"), 0);
}

#[test]
fn an_attachments_vector_key_is_free_or_unique() {
    let c = fresh();
    account(&c, "a1"); message(&c, "m1", "a1");
    for id in ["att1", "att2"] {
        c.execute("INSERT INTO attachment (id, message_id, file_name, mime_type, size_bytes) VALUES (?1, 'm1', 'f.jpg', 'image/jpeg', 1)", [id]).unwrap();
        c.execute("INSERT INTO attachment_content (attachment_id, updated_at_ms) VALUES (?1, 1)", [id]).unwrap();
    }
    assert_eq!(one::<Option<String>>(&c, "SELECT vector_key FROM attachment_content WHERE attachment_id = 'att1'"), None);
    c.execute("UPDATE attachment_content SET embedded = 1, vector_key = 'm1/att1' WHERE attachment_id = 'att1'", []).unwrap();
    assert!(refused(c.execute("UPDATE attachment_content SET vector_key = 'm1/att1' WHERE attachment_id = 'att2'", [])).to_lowercase().contains("unique"));
    c.execute("UPDATE attachment_content SET vector_key = NULL WHERE attachment_id = 'att2'", []).unwrap();
}

#[test]
fn an_existing_database_folds_its_tabs_and_categories_into_labels() {
    let mut c = at_v3();
    account(&c, "a1"); message(&c, "m1", "a1"); message(&c, "m2", "a1");
    // split_tab as seeded (six defaults, Social off), reordered, plus a sender tab, a domain tab, a label tab and a clashing name
    let tab = |id: &str, name: &str, ord: i64, kind: &str, val: &str, en: i64| c.execute("INSERT INTO split_tab (id, name, sort_order, rule_kind, rule_value, enabled, updated_at_ms) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 7)", params![id, name, ord, kind, val, en]).unwrap();
    tab("updates", "Updates", 0, "builtin", "Updates", 1);
    tab("primary", "Primary", 1, "builtin", "Primary", 1);
    tab("needs-reply", "Needs Reply", 2, "builtin", "Needs Reply", 0);
    tab("promotions", "Promotions", 3, "builtin", "Promotions", 1);
    tab("social", "Social", 4, "builtin", "Social", 0);
    tab("vips", "VIPs", 5, "builtin", "VIPs", 1);
    tab("linea", "Linea > Supply", 6, "sender", "orders@linea.example", 1);
    tab("news", "News", 7, "domain", "news.example", 0);
    tab("acc", "Account tab", 8, "account", "a1", 1);
    c.execute("INSERT INTO label (id, name, updated_at_ms) VALUES ('mine', 'Promotions', 1)", []).unwrap();
    c.execute("INSERT INTO label (id, name, updated_at_ms) VALUES ('pick', 'Picked', 1)", []).unwrap();
    tab("pickedtab", "Picked", 9, "label", "pick", 1);
    c.execute("INSERT INTO message_category (message_id, tab_id, decided_by, rule_id, probability, decided_at_ms, updated_at_ms) VALUES ('m1','updates','rule','r9',NULL,3,3)", []).unwrap();
    c.execute("INSERT INTO message_category (message_id, tab_id, decided_by, rule_id, probability, decided_at_ms, updated_at_ms) VALUES ('m2','vips','laya',NULL,0.9,3,3)", []).unwrap();
    migrate(&mut c).unwrap();
    let pins: Vec<(String, i64)> = c.prepare("SELECT id, pinned FROM label WHERE kind = 'builtin' ORDER BY sort_order").unwrap().query_map([], |r| Ok((r.get(0)?, r.get(1)?))).unwrap().map(Result::unwrap).collect();
    assert_eq!(pins, [("updates".into(), 1), ("primary".into(), 1), ("needs-reply".into(), 0), ("promotions".into(), 1), ("social".into(), 0)], "the owner's tab order and on/off survive");
    assert_eq!(one::<i64>(&c, "SELECT pinned FROM label WHERE id = 'pick'"), 1, "a tab on a label pins that label");
    assert_eq!(one::<String>(&c, "SELECT name FROM label WHERE id = 'mine'"), "Promotions (mine)", "an owner label with a built-in's name is renamed, not lost");
    assert_eq!(one::<String>(&c, "SELECT name || '/' || pinned FROM label WHERE id = 'tab-linea'"), "Linea - Supply/1", "a name with > is made safe");
    assert_eq!(one::<String>(&c, "SELECT conditions_json || action_kind || action_label_id FROM rule WHERE id = 'rule-tab-linea'"), r#"[{"kind":"from","words":"orders@linea.example"}]labeltab-linea"#);
    assert_eq!(one::<String>(&c, "SELECT conditions_json FROM rule WHERE id = 'rule-tab-news'"), r#"[{"kind":"domain","domain":"news.example"}]"#);
    assert_eq!(one::<i64>(&c, "SELECT pinned FROM label WHERE id = 'tab-news'"), 0, "a hidden tab becomes an unpinned label");
    let f: (String, Option<String>) = c.query_row("SELECT decided_by, rule_id FROM label_assignment WHERE message_id = 'm1' AND label_id = 'updates'", [], |r| Ok((r.get(0)?, r.get(1)?))).unwrap();
    assert_eq!(f, ("rule".to_string(), Some("r9".to_string())), "message_category folded into label_assignment with decided_by");
    assert_eq!(one::<i64>(&c, "SELECT count(*) FROM label_assignment WHERE message_id = 'm2'"), 0, "VIPs is a mailbox now: its category has no label to fold into");
    assert_eq!(one::<i64>(&c, "SELECT count(*) FROM split_tab"), 10, "the old table is left as history");
    assert_eq!(one::<i64>(&c, "SELECT count(*) FROM message_category"), 2);
}

#[test]
fn the_reversal_restores_the_0003_schema_and_keeps_the_owners_labels() {
    let v3 = at_v3();
    let objects = |c: &Connection| -> Vec<(String, String)> {
        c.prepare("SELECT type, name FROM sqlite_master WHERE name NOT LIKE 'sqlite_%' ORDER BY type, name").unwrap().query_map([], |r| Ok((r.get(0)?, r.get(1)?))).unwrap().map(Result::unwrap).collect()
    };
    let mut c = fresh();
    account(&c, "a1");
    owner_label(&c, "w", "Work", None, None).unwrap();
    c.execute("INSERT INTO block (kind, value, since_ms) VALUES ('domain','bad.example',1)", []).unwrap();
    let applied = revert_to(&mut c, 3).unwrap();
    assert_eq!((applied.from_version, applied.to_version), (store_migrations::latest_version(), 3)); // from the latest (0005 since contracts delta 10)
    assert_eq!(objects(&c), objects(&v3), "after the reversal the schema objects are exactly 0003's");
    let cols = |c: &Connection, t: &str| -> Vec<String> { c.prepare(&format!("PRAGMA table_info({t})")).unwrap().query_map([], |r| r.get::<_, String>(1)).unwrap().map(Result::unwrap).collect() };
    for t in ["label", "label_assignment", "rule", "attachment_content"] { assert_eq!(cols(&c, t), cols(&v3, t), "{t} columns"); }
    assert_eq!(one::<String>(&c, "SELECT name FROM label"), "Work", "the owner's label stays; the built-ins are gone");
    migrate(&mut c).unwrap();
    assert_eq!(one::<i64>(&c, "SELECT count(*) FROM label WHERE kind = 'builtin'"), 5, "re-applying brings the built-ins back");
    assert!(!revert_to(&mut c, store_migrations::latest_version()).unwrap().changed_anything());
}
