//! Folder moves, account moves, deletes and the unread-ids read, as typed store operations.
mod common;
use common::*;
use contracts::{CoreEvent, InboxQuery, StreamKind};
use std::sync::Arc;

fn q(tab: &str) -> InboxQuery {
    InboxQuery { tab_id: tab.into(), account_id: None, label_id: None, mailbox: None, account_ids: vec![], filters: vec![], smart_folder_id: None }
}

fn ids(rows: &[contracts::InboxRow]) -> Vec<String> {
    rows.iter().map(|r| r.message_id.clone()).collect()
}

fn account_of(store: &store::Store, id: &str) -> (String, Option<String>) {
    let id = id.to_string();
    store.write(move |ctx| Ok(ctx.tx.query_row("SELECT account_id, remote_id FROM message WHERE id = ?1", [&id], |r| Ok((r.get(0)?, r.get(1)?)))?)).unwrap()
}

#[test]
fn a_folder_move_takes_the_message_out_of_the_inbox_and_the_counts_follow() {
    let db = TempDb::new("mv-folder");
    let store = small_store(&db, 12);
    store.counts().unwrap();
    let c = Arc::new(Collector::default());
    store.subscribe(StreamKind::Inbox, c.clone());
    store.subscribe(StreamKind::Counts, c.clone());
    let changed = store.write(|ctx| ctx.set_folder(&["m000".to_string(), "m006".to_string()], "Trash")).unwrap();
    assert_eq!(changed, 2);
    assert_eq!(ids(&store.inbox(&q("primary")).unwrap()), Vec::<String>::new());
    assert_eq!(store.counts().unwrap(), store.recount().unwrap());
    let events = c.take();
    assert_eq!(events.iter().filter(|e| matches!(e, CoreEvent::InboxChanged { .. })).count(), 2, "one event for the one tab, one for the All view (delta 16)");
    assert_eq!(events.iter().filter(|e| matches!(e, CoreEvent::CountsChanged { .. })).count(), 1);
    assert_eq!(store.write(|ctx| ctx.set_folder(&["m000".to_string()], "Trash")).unwrap(), 0, "already there: nothing changes");
    store.write(|ctx| ctx.set_folders(&[("m000".to_string(), "INBOX".to_string()), ("m006".to_string(), "INBOX".to_string())])).unwrap();
    assert_eq!(ids(&store.inbox(&q("primary")).unwrap()), ["m006", "m000"], "each message back to its own folder");
}

#[test]
fn an_account_move_clears_remote_id_moves_whole_threads_only_and_restores_exactly() {
    let db = TempDb::new("mv-account");
    let store = small_store(&db, 4);
    // m000 (acc-a, t000) and m001 (acc-b, t000) share a thread in the fixture; give them a remote id and one single-account thread.
    store
        .write(|ctx| {
            ctx.tx.execute("UPDATE message SET remote_id = 'srv-' || id, account_id = 'acc-a'", [])?;
            Ok(())
        })
        .unwrap();
    // t000 = m000 + m001, t001 = m002 + m003. Move m000 only: its thread stays; then m002 and m003 together: their thread moves.
    let one = store.move_to_account(vec!["m000".into()], "acc-b".into()).unwrap();
    assert_eq!((one.from_account_id.as_str(), one.to_account_id.as_str()), ("acc-a", "acc-b"));
    assert!(one.threads.is_empty(), "m001 stays behind, so the thread stays");
    assert_eq!(account_of(&store, "m000"), ("acc-b".to_string(), None));
    let both = store.move_to_account(vec!["m002".into(), "m003".into(), "m000".into()], "acc-b".into()).unwrap();
    assert_eq!(both.moved_ids(), ["m002", "m003"], "m000 is already there");
    assert_eq!(both.threads, [("t001".to_string(), "acc-a".to_string())]);
    store.write(move |ctx| ctx.restore_account_move(&both, &["m002".to_string(), "m003".to_string()])).unwrap();
    assert_eq!(account_of(&store, "m002"), ("acc-a".to_string(), Some("srv-m002".to_string())));
    let thread_account: String = store.write(|ctx| Ok(ctx.tx.query_row("SELECT account_id FROM thread WHERE id = 't001'", [], |r| r.get(0))?)).unwrap();
    assert_eq!(thread_account, "acc-a");
    assert_eq!(store.counts().unwrap(), store.recount().unwrap());
}

#[test]
fn an_account_move_refuses_in_words_and_changes_nothing() {
    let db = TempDb::new("mv-account-bad");
    let store = small_store(&db, 4);
    let w = |e: store::StoreError| e.to_string();
    assert!(w(store.move_to_account(vec!["m000".into()], "ghost".into()).unwrap_err()).contains("account ghost does not exist"));
    assert!(w(store.move_to_account(vec!["nope".into()], "acc-b".into()).unwrap_err()).contains("message nope does not exist"));
    // m000 is in acc-a, m001 in acc-b: two sources into a third account
    let third = store.write(|ctx| Ok(ctx.tx.execute("INSERT INTO account (id, address, provider, display_name, colour, sort_order, credential_kind, keychain_ref, updated_at_ms) VALUES ('acc-c', 'c@example.test', 'imap', 'C', '#336699', 2, 'manual', 'svc' || char(31) || 'c', 0)", [])?));
    third.unwrap();
    assert!(w(store.move_to_account(vec!["m000".into(), "m001".into()], "acc-c".into()).unwrap_err()).contains("one account at a time"));
    assert_eq!(account_of(&store, "m000").0, "acc-a");
}

#[test]
fn deleting_a_folders_messages_removes_empty_threads_and_keeps_the_counts_equal_to_a_recount() {
    let db = TempDb::new("mv-delete");
    let store = small_store(&db, 12);
    store.counts().unwrap();
    store.write(|ctx| ctx.set_folder(&["m000".to_string(), "m001".to_string(), "m002".to_string()], "Trash")).unwrap();
    store.write(|ctx| ctx.set_folder(&["m004".to_string()], "Spam")).unwrap();
    let accounts = store.write(|ctx| ctx.accounts_with_mail_in(&["Trash", "Spam"], None)).unwrap();
    assert_eq!(accounts, ["acc-a", "acc-b"]);
    assert_eq!(store.write(|ctx| ctx.accounts_with_mail_in(&["Trash", "Spam"], Some("acc-b"))).unwrap(), ["acc-b"]);
    let gone = store.write(|ctx| ctx.delete_folder_messages("acc-a", &["Trash", "Spam"])).unwrap();
    assert_eq!(gone, ["m000", "m002", "m004"]);
    let threads: i64 = store.write(|ctx| Ok(ctx.tx.query_row("SELECT count(*) FROM thread WHERE id = 't000'", [], |r| r.get(0))?)).unwrap();
    assert_eq!(threads, 1, "t000 still holds m001");
    assert_eq!(store.delete_messages(vec!["m001".into(), "m001".into(), "m003".into()]).unwrap(), 2);
    let t000: i64 = store.write(|ctx| Ok(ctx.tx.query_row("SELECT count(*) FROM thread WHERE id IN ('t000')", [], |r| r.get(0))?)).unwrap();
    assert_eq!(t000, 0, "the last message of the thread went, so the thread went");
    assert_eq!(store.counts().unwrap(), store.recount().unwrap());
}

#[test]
fn unread_ids_are_the_unread_rows_of_the_view_and_nothing_else() {
    let db = TempDb::new("mv-unread");
    let store = small_store(&db, 30);
    store.set_pinned("m000".into(), true, 10).unwrap();
    store.set_read(vec!["m006".into()], true).unwrap();
    store.set_archived(vec!["m012".into()], true).unwrap();
    store.snooze(vec!["m018".into()], 9_000_000_000_000, 1).unwrap();
    let sorted = |mut v: Vec<String>| {
        v.sort();
        v
    };
    for query in [q("primary"), q("updates"), InboxQuery { tab_id: "primary".into(), account_id: Some("acc-a".into()), label_id: None, mailbox: None, account_ids: vec![], filters: vec![], smart_folder_id: None }] {
        let expected: Vec<String> = store.inbox_limit(&query, 10_000).unwrap().into_iter().filter(|r| r.unread).map(|r| r.message_id).collect();
        assert_eq!(sorted(store.unread_ids(&query).unwrap()), sorted(expected), "{query:?}");
    }
    assert_eq!(sorted(store.unread_ids(&q("primary")).unwrap()), ["m000", "m024"], "pinned counts; read, archived and snoozed do not");
    store.write(|ctx| ctx.label_create(store::NewLabel { id: Some("l".into()), name: "L".into(), now_ms: 1, ..Default::default() }).map(|_| ())).unwrap();
    store.label_assign("l".into(), vec!["m024".into()], 1).unwrap();
    let labelled = InboxQuery { tab_id: "primary".into(), account_id: None, label_id: Some("l".into()), mailbox: None, account_ids: vec![], filters: vec![], smart_folder_id: None };
    assert_eq!(store.unread_ids(&labelled).unwrap(), ["m024"]);
    store.set_read(vec!["m024".into()], true).unwrap();
    assert!(store.unread_ids(&labelled).unwrap().is_empty());
}

#[test]
fn the_account_display_name_is_read_without_a_write() {
    let db = TempDb::new("mv-name");
    let store = small_store(&db, 0);
    assert_eq!(store.account_display_name("acc-a").unwrap().as_deref(), Some("acc-a"));
    assert_eq!(store.account_display_name("ghost").unwrap(), None);
}

#[test]
fn deleting_the_newest_message_of_a_thread_moves_waiting_to_the_one_before_it() {
    let db = TempDb::new("mv-delete-waiting");
    let store = small_store(&db, 0);
    // One thread of two inbound messages in the inbox: only the newer one is "waiting".
    store.insert_messages(vec![new_msg(msg("old", "t1", "acc-a", "x@ext.test", 1_700_000_000_000), Some("primary")), new_msg(msg("new", "t1", "acc-a", "x@ext.test", 1_700_000_001_000), Some("primary"))]).unwrap();
    let waiting = |s: &store::Store| s.counts().unwrap().tabs.iter().find(|t| t.tab_id == "primary").unwrap().waiting;
    assert_eq!(waiting(&store), 1);
    store.delete_messages(vec!["new".into()]).unwrap();
    assert_eq!(waiting(&store), 1, "the older message is now the thread's newest and is waiting");
    assert_eq!(store.counts().unwrap(), store.recount().unwrap());
    store.delete_messages(vec!["old".into()]).unwrap();
    assert_eq!(waiting(&store), 0);
    assert_eq!(store.counts().unwrap(), store.recount().unwrap());
}
