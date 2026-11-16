//! A list row is a CONVERSATION (lead, 13 Nov; owner picture 205: one conversation of three inbox mails was listed twice, once per
//! mail). `Store::inbox` lists a thread once, at its newest mail the list shows, or once among the pinned rows; and
//! `Store::conversation_messages` names the mails such a row stands for.
//! Each test names, in its comment, the one-line broken copy of core/crates/store/src/read.rs that turns it red.
//! Failure messages name message ids only, never a subject or any other mail text.
mod common;
use common::*;
use contracts::{InboxQuery, InboxRow};
use store::Store;

fn ids_of(rows: &[InboxRow]) -> Vec<String> { rows.iter().map(|r| r.message_id.clone()).collect() }
fn owned(ids: &[&str]) -> Vec<String> { ids.iter().map(|s| s.to_string()).collect() }
fn tab(id: &str) -> InboxQuery {
    InboxQuery { tab_id: id.into(), account_id: None, label_id: None, mailbox: None, account_ids: vec![], filters: vec![], smart_folder_id: None }
}
/// One inbox mail of acc-a in thread `thread`, filed in `tab`.
fn put(s: &Store, id: &str, thread: &str, at: i64, tab: &str) {
    s.insert_messages(vec![new_msg(msg(id, thread, "acc-a", "someone@ext.test", at), Some(tab))]).unwrap();
}
/// A thread of three inbox mails (t1: a 100, b 200, c 300) between two threads of one mail (x 150, y 250).
fn three_and_two(db: &TempDb) -> Store {
    let s = small_store(db, 0);
    put(&s, "a", "t1", 100, "primary");
    put(&s, "x", "tx", 150, "primary");
    put(&s, "b", "t1", 200, "primary");
    put(&s, "y", "ty", 250, "primary");
    put(&s, "c", "t1", 300, "primary");
    s
}

/// The thread of three mails is ONE row, at its newest mail, with the badge 3; the other threads keep their place.
/// Broken copy (red): the walk's loop in `inbox` without `&& seen.insert(thread)` (one row per mail: c, y, b, x, a).
#[test]
fn a_conversation_of_three_mails_is_one_row_at_its_newest_mail() {
    let db = TempDb::new("conv-one-row");
    let s = three_and_two(&db);
    let rows = s.inbox(&tab("primary")).unwrap();
    assert_eq!(ids_of(&rows), owned(&["c", "y", "x"]), "one row per conversation, newest first");
    assert_eq!(rows[0].thread_count, 3, "the row still counts the conversation's mails");
    assert_eq!(ids_of(&s.inbox(&tab("all")).unwrap()), owned(&["c", "y", "x"]), "the All view lists conversations too");
}

/// When the newest mail leaves the list (archived), the conversation's row is its next mail: nothing is hidden for good.
/// Broken copy (red): the dedupe keyed on the thread BEFORE the list's own filter (a `seen` filled from every mail of the thread): the archived newest mail hides b and a.
#[test]
fn the_row_falls_back_to_the_next_mail_the_list_shows() {
    let db = TempDb::new("conv-fallback");
    let s = three_and_two(&db);
    s.set_archived(owned(&["c"]), true).unwrap();
    assert_eq!(ids_of(&s.inbox(&tab("primary")).unwrap()), owned(&["y", "b", "x"]));
}

/// A tab lists a conversation by the newest of ITS OWN mails: a newer mail filed in another tab does not hide it here.
/// Broken copy (red): one `seen` set shared by every tab's read (a thread met in Updates is dropped in Primary).
#[test]
fn a_newer_mail_in_another_tab_does_not_hide_the_conversation_here() {
    let db = TempDb::new("conv-tabs");
    let s = small_store(&db, 0);
    put(&s, "a", "t1", 100, "primary");
    put(&s, "b", "t1", 200, "updates");
    assert_eq!(ids_of(&s.inbox(&tab("primary")).unwrap()), owned(&["a"]), "Primary shows the thread by its own mail");
    assert_eq!(ids_of(&s.inbox(&tab("updates")).unwrap()), owned(&["b"]));
    assert_eq!(ids_of(&s.inbox(&tab("all")).unwrap()), owned(&["b"]), "All lists it once, at its newest mail");
}

/// A pinned mail stands for its conversation among the pinned rows, and no second row of that conversation follows below.
/// Broken copy (red): the walk's loop without `!pinned_threads.contains(&thread)` (c is listed again below the pinned a).
#[test]
fn a_pinned_mail_is_the_conversations_only_row() {
    let db = TempDb::new("conv-pinned");
    let s = three_and_two(&db);
    s.set_pinned("a".into(), true, 1_000).unwrap();
    let rows = s.inbox(&tab("primary")).unwrap();
    assert_eq!(ids_of(&rows), owned(&["a", "y", "x"]), "the pinned mail first, then the other conversations");
    assert!(rows[0].pinned);
    s.set_pinned("b".into(), true, 2_000).unwrap();
    assert_eq!(ids_of(&s.inbox(&tab("primary")).unwrap()), owned(&["b", "y", "x"]), "two pinned mails of one conversation: the newest pin is its row");
}

/// The mails a row stands for: the given mail first, then its conversation's other listed mails, newest first; never a mail the lists
/// do not show (archived), never one twice, and an unknown id is kept as given.
/// Broken copy (red): `conversation_messages` without `AND {VISIBLE}` (the archived mail is taken along).
#[test]
fn a_row_stands_for_every_listed_mail_of_its_conversation() {
    let db = TempDb::new("conv-messages");
    let s = three_and_two(&db);
    assert_eq!(s.conversation_messages(&owned(&["c"])).unwrap(), owned(&["c", "b", "a"]));
    assert_eq!(s.conversation_messages(&owned(&["x"])).unwrap(), owned(&["x"]), "a conversation of one mail");
    assert_eq!(s.conversation_messages(&owned(&["c", "b", "y"])).unwrap(), owned(&["c", "b", "a", "y"]), "no mail twice");
    s.set_archived(owned(&["a"]), true).unwrap();
    assert_eq!(s.conversation_messages(&owned(&["c"])).unwrap(), owned(&["c", "b"]), "an archived mail is not taken along");
    assert_eq!(s.conversation_messages(&owned(&["no-such-id"])).unwrap(), owned(&["no-such-id"]));
}

/// The conversation's people are served as "Name <address>", once each, in order of first appearance; the address alone when the mail
/// carries no name. Broken copy (red): `thread` pushing `from.clone()` for every mail (the reader's header reads raw addresses).
#[test]
fn a_conversations_people_carry_their_names() {
    let db = TempDb::new("conv-people");
    let s = small_store(&db, 0);
    let mut a = msg("a", "t1", "acc-a", "nadia@ext.test", 100);
    a.sender_name = "Nadia Benali".into();
    let mut b = msg("b", "t1", "acc-a", "nadia@ext.test", 200);
    b.sender_name = "Nadia Benali".into();
    let c = msg("c", "t1", "acc-a", "bare@ext.test", 300); // `msg` gives the address as the name: no name of its own
    s.insert_messages(vec![new_msg(a, Some("primary")), new_msg(b, Some("primary")), new_msg(c, Some("primary"))]).unwrap();
    let t = s.thread("t1").unwrap().expect("the thread is stored");
    assert_eq!(t.participants, ["Nadia Benali <nadia@ext.test>", "bare@ext.test"]);
}


/// Owner's pictures 213 and 214 (13 Nov 2026): a list row with an empty sender line, and a name drawn with its quotes. A mail with no sender name
/// shows its address in its row; a name wrapped in quotes shows without them, in the row, in the conversation's people and in the mail itself.
/// Broken copies (red): `row_from` serving the stored name as it is; `clean_sender_name` keeping the quotes.
#[test]
fn a_row_shows_the_address_for_a_missing_name_and_a_quoted_name_without_its_quotes() {
    let db = TempDb::new("sender-shown");
    let s = small_store(&db, 0);
    let mut a = msg("a", "t1", "acc-a", "noname@ext.test", 100);
    a.sender_name = String::new();
    let mut b = msg("b", "t2", "acc-a", "fleet@ext.test", 200);
    b.sender_name = "'Ayvens'".into();
    let mut c = msg("c", "t3", "acc-a", "bob@ext.test", 300);
    c.sender_name = "\"Bob Stone\"".into();
    let mut d = msg("d", "t4", "acc-a", "ann@ext.test", 400);
    d.sender_name = "Ann O'Neil".into();
    s.insert_messages(vec![new_msg(a, Some("primary")), new_msg(b, Some("primary")), new_msg(c, Some("primary")), new_msg(d, Some("primary"))]).unwrap();
    let rows = s.inbox(&tab("primary")).unwrap();
    let shown: Vec<(String, String)> = rows.iter().map(|r| (r.message_id.clone(), r.sender_name.clone())).collect();
    assert_eq!(shown, [("d".to_string(), "Ann O'Neil".to_string()), ("c".into(), "Bob Stone".into()), ("b".into(), "Ayvens".into()), ("a".into(), "noname@ext.test".into())]);
    assert_eq!(s.thread("t2").unwrap().expect("stored").participants, ["Ayvens <fleet@ext.test>"]);
    assert_eq!(s.thread("t1").unwrap().expect("stored").participants, ["noname@ext.test"]);
    assert_eq!(s.message("b").unwrap().expect("stored").sender_name, "Ayvens");
    assert_eq!(s.message("a").unwrap().expect("stored").sender_name, "", "the mail itself keeps no name: the reader shows the address once");
}
