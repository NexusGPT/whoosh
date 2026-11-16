//! par-actions: send again, mark all read, empty Trash and Spam, mute, star, trash, spam, List-Unsubscribe one-click.
mod common;
use actions::{EmptyTrashAndSpam, MarkAllRead, Mute, SendAgain, Spam, Star, Trash, Undo, Unsubscribe, WriteBack};
use common::*;
use contracts::CoreError;

fn with_extras() -> Vec<store::NewMessage> {
    let mut v = four();
    // A sent message of the owner, with two recipients and one attachment.
    let mut sent = msg("sent1", "ts1", "acc-a", OWN_A, NOW - 500_000);
    sent.to = vec!["x@ext.test".into(), "y@ext.test".into()];
    sent.folder = "Sent".into();
    sent.read = true;
    v.push(new_msg(sent));
    // A mailing-list message.
    let mut list = new_msg(msg("list1", "tl1", "acc-b", "news@list.test", NOW - 400_000));
    list.has_list_header = true;
    v.push(list);
    // A two-message thread for mute.
    v.push(new_msg(msg("th1", "tthread", "acc-a", "carol@ext.test", NOW - 300_000)));
    v.push(new_msg(msg("th2", "tthread", "acc-a", "carol@ext.test", NOW - 290_000)));
    v
}

#[test]
fn send_again_makes_a_draft_with_the_same_recipients_subject_and_text() {
    let r = rig("send-again", with_extras());
    r.store.write(|ctx| { ctx.tx.execute("INSERT INTO attachment (id, message_id, file_name, mime_type, size_bytes) VALUES ('att1', 'sent1', 'a.pdf', 'application/pdf', 10)", [])?; Ok(()) }).unwrap();
    let out = SendAgain::new(r.env.clone()).execute("sent1".into()).unwrap();
    let id = out.created_id.expect("a new draft id");
    assert_eq!(scalar(&r, "SELECT count(*) FROM draft"), 1);
    let q = format!("SELECT subject || '|' || to_json || '|' || attachment_ids_json || '|' || from_account_id || '|' || body FROM draft WHERE id = '{id}'");
    let got = r.store.write(move |ctx| Ok(ctx.tx.query_row(&q, [], |x| x.get::<_, String>(0))?)).unwrap();
    assert_eq!(got, "Subject sent1|[\"x@ext.test\",\"y@ext.test\"]|[\"att1\"]|acc-a|the body");
    assert!(r.server.asked().is_empty(), "nothing leaves until the owner sends the draft");
}

#[test]
fn send_again_refuses_a_message_the_owner_did_not_send() {
    let r = rig("send-again-foreign", with_extras());
    let e = SendAgain::new(r.env.clone()).execute("m1".into()).unwrap_err();
    assert!(matches!(e, CoreError::Invalid { .. }));
    assert_eq!(scalar(&r, "SELECT count(*) FROM draft"), 0);
}

#[test]
fn mark_all_read_marks_every_unread_row_of_the_view() {
    let r = rig("mark-all", with_extras());
    let before = scalar(&r, "SELECT count(*) FROM message WHERE read = 0 AND folder = 'INBOX'");
    assert!(before >= 4);
    MarkAllRead::new(r.env.clone()).execute(primary()).unwrap();
    assert_eq!(scalar(&r, "SELECT count(*) FROM message WHERE read = 0 AND folder = 'INBOX'"), 0);
    assert!(r.store.counts().unwrap().tabs.iter().all(|t| t.unread == 0));
    assert_counts_match(&r);
    let asked = r.server.asked().len();
    MarkAllRead::new(r.env.clone()).execute(primary()).unwrap();
    assert_eq!(r.server.asked().len(), asked, "a second call has nothing to mark and asks no server");
}

#[test]
fn trash_and_spam_move_the_message_out_of_the_inbox_and_a_refusal_returns_it() {
    let r = rig("trash-spam", with_extras());
    Trash::new(r.env.clone()).execute(vec!["m1".into()]).unwrap();
    Spam::new(r.env.clone()).execute(vec!["m2".into()]).unwrap();
    assert_eq!(text(&r, "SELECT folder FROM message WHERE id = 'm1'"), "Trash");
    assert_eq!(text(&r, "SELECT folder FROM message WHERE id = 'm2'"), "Spam");
    assert!(!inbox_ids(&r).contains(&"m1".to_string()));
    assert!(matches!(&r.server.asked()[0], WriteBack::MoveToFolder { folder, messages, .. } if folder == "Trash" && messages[0].folder == "INBOX"));
    r.server.refuse("acc-a");
    let e = Trash::new(r.env.clone()).execute(vec!["m3".into()]).unwrap_err();
    assert_action_failed(e, "Alpha");
    assert_eq!(text(&r, "SELECT folder FROM message WHERE id = 'm3'"), "INBOX");
    assert_counts_match(&r);
}

#[test]
fn star_adds_the_system_label_and_unstar_removes_it_without_an_undo_entry() {
    let r = rig("star", with_extras());
    let out = Star::new(r.env.clone()).execute(vec!["m1".into()], true).unwrap();
    assert!(out.undo_entry_id.is_none());
    assert_eq!(scalar(&r, "SELECT count(*) FROM label_assignment JOIN label ON label.id = label_id WHERE label.name = 'Starred'"), 1);
    Star::new(r.env.clone()).execute(vec!["m1".into()], false).unwrap();
    assert_eq!(scalar(&r, "SELECT count(*) FROM label_assignment"), 0);
}

#[test]
fn mute_labels_and_archives_the_whole_thread_and_unmute_lifts_only_the_label() {
    let r = rig("mute", with_extras());
    Mute::new(r.env.clone()).execute(vec!["th1".into()], true).unwrap();
    assert_eq!(scalar(&r, "SELECT count(*) FROM message WHERE thread_id = 'tthread' AND archived = 1"), 2, "both messages of the thread");
    assert_eq!(scalar(&r, "SELECT count(*) FROM label_assignment JOIN label ON label.id = label_id WHERE label.name = 'Muted'"), 2);
    Mute::new(r.env.clone()).execute(vec!["th1".into()], false).unwrap();
    assert_eq!(scalar(&r, "SELECT count(*) FROM label_assignment"), 0);
    assert_eq!(scalar(&r, "SELECT count(*) FROM message WHERE thread_id = 'tthread' AND archived = 1"), 2, "unmute does not bring the thread back by itself");
    assert_counts_match(&r);
}

#[test]
fn a_refused_mute_leaves_the_thread_in_the_inbox_with_no_label() {
    let r = rig("mute-refused", with_extras());
    r.server.refuse("acc-a");
    let e = Mute::new(r.env.clone()).execute(vec!["th2".into()], true).unwrap_err();
    assert_action_failed(e, "Alpha");
    assert_eq!(scalar(&r, "SELECT count(*) FROM message WHERE thread_id = 'tthread' AND archived = 1"), 0);
    assert_eq!(scalar(&r, "SELECT count(*) FROM label_assignment"), 0);
    assert_counts_match(&r);
}

#[test]
fn empty_trash_and_spam_deletes_after_the_server_agrees_and_only_for_that_account() {
    let r = rig("empty", with_extras());
    Trash::new(r.env.clone()).execute(vec!["m1".into(), "m2".into()]).unwrap();
    Spam::new(r.env.clone()).execute(vec!["m3".into()]).unwrap();
    r.server.refuse("acc-b");
    let e = EmptyTrashAndSpam::new(r.env.clone()).execute(None).unwrap_err();
    assert_action_failed(e, "Beta");
    // acc-a answered first and agreed: its trash and spam are gone; acc-b's refused: its message stays.
    assert_eq!(scalar(&r, "SELECT count(*) FROM message WHERE id IN ('m1','m3')"), 0);
    assert_eq!(scalar(&r, "SELECT count(*) FROM message WHERE id = 'm2'"), 1, "the refused account keeps its message");
    r.server.accept_all();
    let n = EmptyTrashAndSpam::new(r.env.clone()).execute(None).unwrap();
    assert_eq!(n, 1);
    assert_eq!(scalar(&r, "SELECT count(*) FROM message WHERE folder IN ('Trash','Spam')"), 0);
    assert_eq!(scalar(&r, "SELECT count(*) FROM thread WHERE id = 't2'"), 0, "a thread left empty goes");
    assert_counts_match(&r);
}

#[test]
fn unsubscribe_asks_the_server_first_and_changes_nothing_on_a_refusal() {
    let r = rig("unsub", with_extras());
    r.server.refuse("acc-b");
    let e = Unsubscribe::new(r.env.clone()).execute("list1".into()).unwrap_err();
    assert_action_failed(e, "Beta");
    assert_eq!(scalar(&r, "SELECT archived FROM message WHERE id = 'list1'"), 0);
    assert_eq!(scalar(&r, "SELECT count(*) FROM label_assignment"), 0);
    r.server.accept_all();
    Unsubscribe::new(r.env.clone()).execute("list1".into()).unwrap();
    assert!(matches!(r.server.asked().last().unwrap(), WriteBack::Unsubscribe { account_id, .. } if account_id == "acc-b"));
    assert_eq!(scalar(&r, "SELECT archived FROM message WHERE id = 'list1'"), 1);
    assert_eq!(scalar(&r, "SELECT count(*) FROM label_assignment JOIN label ON label.id = label_id WHERE label.name = 'Unsubscribed'"), 1);
}

#[test]
fn unsubscribe_on_a_message_without_a_list_header_is_refused_in_words() {
    let r = rig("unsub-none", with_extras());
    let e = Unsubscribe::new(r.env.clone()).execute("m1".into()).unwrap_err();
    assert!(matches!(e, CoreError::Invalid { .. }));
    assert!(r.server.asked().is_empty());
}

#[test]
fn undo_with_nothing_pushed_by_these_actions_says_nothing_to_undo() {
    let r = rig("par-undo", with_extras());
    Star::new(r.env.clone()).execute(vec!["m1".into()], true).unwrap();
    MarkAllRead::new(r.env.clone()).execute(primary()).unwrap();
    let e = Undo::new(r.env.clone()).execute().unwrap_err();
    assert_eq!(e.to_string(), "undo: Nothing to undo");
}
