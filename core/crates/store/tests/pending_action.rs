//! engine-act: the store's `pending_action` calls (migration 0011). A row is written in the caller's transaction, read when due,
//! deleted when the server took it, kept with the server's words and a later time when it refused. Seen red on main (no table,
//! no calls) and on the mutants named in docs/lanes/engine-act.md.
use contracts::{AccountConfig, AuthMethod, ConnectionSecurity, DetectionSource, MailProvider};
use store::{NewPending, Store};

mod common;
use common::TempDb;

fn config() -> AccountConfig {
    AccountConfig { imap_host: "127.0.0.1".into(), imap_port: 1, imap_security: ConnectionSecurity::Ssl, smtp_host: "127.0.0.1".into(), smtp_port: 1,
        smtp_security: ConnectionSecurity::Starttls, username: "a@example.test".into(), smtp_username: "a@example.test".into(), auth_method: AuthMethod::Password, source: DetectionSource::Manual }
}
fn row(account: &str, uid: i64, kind: &str) -> NewPending {
    NewPending { account_id: account.into(), message_id: Some(format!("m{uid}")), folder: "INBOX".into(), uid: Some(uid), kind: kind.into(), payload_json: r#"{"read":true}"#.into() }
}

#[test]
fn a_row_is_written_in_the_callers_transaction_read_when_due_retried_and_taken() {
    let db = TempDb::new("pending-a");
    let store = Store::open(&db.0).unwrap();
    let a = store.add_account("a@example.test", "A", MailProvider::Imap, "s\u{1f}a", false, config(), 1).unwrap();
    // a transaction that fails after the add leaves no row: the row is the caller's transaction's
    let (a2, a3) = (a.clone(), a.clone());
    let failed = store.write(move |ctx| { ctx.pending_action_add(&row(&a2, 1, "set_read"), 100)?; Err::<(), _>(store::StoreError::Invalid("the local write failed".into())) });
    assert!(failed.is_err());
    assert!(store.pending_actions().unwrap().is_empty(), "rolled back with the local write");
    let id = store.write(move |ctx| ctx.pending_action_add(&row(&a3, 2, "set_read"), 100)).unwrap();
    assert_eq!(store.pending_actions_due(99).unwrap().len(), 0, "not due before its time");
    let due = store.pending_actions_due(100).unwrap();
    assert_eq!(due.len(), 1);
    assert_eq!((due[0].id, due[0].uid, due[0].tries, due[0].last_error.clone(), due[0].next_at_ms), (id, Some(2), 0, None, 100));
    assert_eq!(store.pending_action_retry(vec![id], "NO try later".into(), 2_100).unwrap(), 1);
    let after = store.pending_actions().unwrap();
    assert_eq!((after[0].tries, after[0].last_error.as_deref(), after[0].next_at_ms), (1, Some("NO try later"), 2_100), "the refusal is kept, with when to try again");
    assert_eq!(store.pending_action_next_due().unwrap(), Some(2_100));
    assert!(store.pending_actions_due(2_099).unwrap().is_empty());
    assert_eq!(store.pending_action_done(vec![id]).unwrap(), 1);
    assert!(store.pending_actions().unwrap().is_empty(), "taken = gone");
    assert_eq!(store.pending_action_next_due().unwrap(), None);
}

#[test]
fn the_table_refuses_an_unknown_kind_and_goes_with_its_account() {
    let db = TempDb::new("pending-b");
    let store = Store::open(&db.0).unwrap();
    let a = store.add_account("a@example.test", "A", MailProvider::Imap, "s\u{1f}a", false, config(), 1).unwrap();
    let a2 = a.clone();
    assert!(store.write(move |ctx| ctx.pending_action_add(&row(&a2, 1, "snooze"), 1)).is_err(), "a snooze is local: no such server kind");
    let a3 = a.clone();
    assert!(store.write(move |ctx| ctx.pending_action_add(&NewPending { payload_json: "not json".into(), ..row(&a3, 1, "set_read") }, 1)).is_err(), "the payload is JSON");
    let a4 = a.clone();
    store.write(move |ctx| ctx.pending_action_add(&row(&a4, 1, "set_read"), 1)).unwrap();
    let a5 = a.clone();
    store.write(move |ctx| { ctx.tx.execute("DELETE FROM account WHERE id = ?1", [&a5])?; Ok(()) }).unwrap();
    assert!(store.pending_actions().unwrap().is_empty(), "a removed account's waiting changes go with it");
}
