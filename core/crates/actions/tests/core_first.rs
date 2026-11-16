//! fn-actions: each use case commits in the store first (under COMMIT_MS), writes back through the adapter port,
//! rolls back with ActionFailed on a refusal, and pushes an undo entry.
mod common;
use actions::{Archive, Label, MarkRead, MoveToAccount, Pin, Snooze, Undo, WriteBack};
use common::*;
use contracts::{CoreError, SnoozeWhen};

fn under_commit(r: &Rig) {
    let took = r.env.last_commit();
    assert!(took.as_millis() as u64 <= constant("COMMIT_MS"), "the store commit took {took:?}, COMMIT_MS is {}", constant("COMMIT_MS"));
}

#[test]
fn archive_commits_first_writes_back_and_pushes_one_undo_entry() {
    let r = rig("archive", four());
    let out = Archive::new(r.env.clone()).execute(vec!["m4".into()]).unwrap();
    under_commit(&r);
    assert!(out.undo_entry_id.is_some());
    assert_eq!(inbox_ids(&r), ["m3", "m2", "m1"], "the archived row is gone from the inbox");
    assert!(matches!(&r.server.asked()[0], WriteBack::SetArchived { account_id, archived: true, .. } if account_id == "acc-b"));
    assert_eq!(scalar(&r, "SELECT count(*) FROM undo_entry WHERE consumed = 0"), 1);
    assert_counts_match(&r);
}

#[test]
fn archive_refused_puts_the_row_back_and_names_the_account() {
    let r = rig("archive-refused", four());
    r.server.refuse("acc-b");
    let before = inbox_ids(&r);
    let e = Archive::new(r.env.clone()).execute(vec!["m4".into()]).unwrap_err();
    assert_action_failed(e, "Beta");
    assert_eq!(inbox_ids(&r), before, "the refused row returns to its place");
    assert_eq!(scalar(&r, "SELECT count(*) FROM undo_entry"), 0, "a refused action leaves no undo entry");
    assert_counts_match(&r);
}

#[test]
fn archive_of_two_accounts_keeps_the_accepted_one_and_returns_the_refused_one() {
    let r = rig("archive-partial", four());
    r.server.refuse("acc-b");
    let e = Archive::new(r.env.clone()).execute(vec!["m1".into(), "m2".into()]).unwrap_err();
    assert_action_failed(e, "Beta");
    assert_eq!(inbox_ids(&r), ["m4", "m3", "m2"], "m1 (Alpha) stayed archived, m2 (Beta) returned");
    assert_eq!(text(&r, "SELECT action_json FROM undo_entry WHERE consumed = 0"), "{\"kind\":\"archive\",\"ids\":[\"m1\"]}", "the undo entry shrinks to what stayed");
    assert_counts_match(&r);
}

#[test]
fn undo_after_archive_restores_folder_and_position() {
    let r = rig("undo-archive", four());
    let before = inbox_ids(&r);
    Archive::new(r.env.clone()).execute(vec!["m3".into()]).unwrap();
    assert_eq!(inbox_ids(&r), ["m4", "m2", "m1"]);
    Undo::new(r.env.clone()).execute().unwrap();
    assert_eq!(inbox_ids(&r), before, "back at the same index");
    assert_eq!(text(&r, "SELECT folder FROM message WHERE id = 'm3'"), "INBOX");
    assert!(matches!(r.server.asked().last().unwrap(), WriteBack::SetArchived { archived: false, .. }), "undo also writes back to the server");
    assert_counts_match(&r);
}

#[test]
fn snooze_hides_the_row_until_its_return_time_and_undo_lifts_it() {
    let r = rig("snooze", four());
    // NOW is Tuesday 2023-11-14 11:30 in the clock's zone (UTC+1): tomorrow 08:00 local = 07:00 UTC on the 15th.
    let out = Snooze::new(r.env.clone()).execute(vec!["m4".into()], SnoozeWhen::TomorrowMorning).unwrap();
    under_commit(&r);
    assert!(out.undo_entry_id.is_some());
    assert_eq!(inbox_ids(&r), ["m3", "m2", "m1"]);
    let at = scalar(&r, "SELECT return_at_ms FROM snooze WHERE message_id = 'm4'");
    assert_eq!(at, 1_700_031_600_000, "08:00 local the next day");
    assert!(matches!(&r.server.asked()[0], WriteBack::Snooze { return_at_ms: Some(t), .. } if *t == at));
    Undo::new(r.env.clone()).execute().unwrap();
    assert_eq!(inbox_ids(&r), ["m4", "m3", "m2", "m1"]);
    assert_eq!(scalar(&r, "SELECT count(*) FROM snooze"), 0);
    assert_counts_match(&r);
}

#[test]
fn snooze_refused_returns_the_row_and_leaves_no_snooze() {
    let r = rig("snooze-refused", four());
    r.server.refuse("acc-a");
    let e = Snooze::new(r.env.clone()).execute(vec!["m3".into()], SnoozeWhen::NextWeek).unwrap_err();
    assert_action_failed(e, "Alpha");
    assert_eq!(inbox_ids(&r), ["m4", "m3", "m2", "m1"]);
    assert_eq!(scalar(&r, "SELECT count(*) FROM snooze"), 0);
    assert_counts_match(&r);
}

#[test]
fn snooze_at_a_time_in_the_past_is_refused_in_words() {
    let r = rig("snooze-past", four());
    let e = Snooze::new(r.env.clone()).execute(vec!["m3".into()], SnoozeWhen::At { at_ms: NOW - 5 }).unwrap_err();
    assert!(matches!(e, CoreError::Invalid { .. }));
    assert_eq!(scalar(&r, "SELECT count(*) FROM snooze"), 0);
}

#[test]
fn pin_puts_the_row_first_and_undo_restores_the_prior_state() {
    let r = rig("pin", four());
    Pin::new(r.env.clone()).execute("m1".into(), true).unwrap();
    under_commit(&r);
    assert_eq!(inbox_ids(&r), ["m1", "m4", "m3", "m2"], "pinned first");
    Undo::new(r.env.clone()).execute().unwrap();
    assert_eq!(inbox_ids(&r), ["m4", "m3", "m2", "m1"]);
    assert_eq!(scalar(&r, "SELECT count(*) FROM pin"), 0);
}

#[test]
fn unpin_then_undo_pins_again() {
    let r = rig("unpin", four());
    Pin::new(r.env.clone()).execute("m2".into(), true).unwrap();
    Pin::new(r.env.clone()).execute("m2".into(), false).unwrap();
    assert_eq!(scalar(&r, "SELECT count(*) FROM pin"), 0);
    Undo::new(r.env.clone()).execute().unwrap();
    assert_eq!(scalar(&r, "SELECT count(*) FROM pin"), 1, "the undo of an unpin pins it again");
}

#[test]
fn pin_refused_leaves_the_row_unpinned() {
    let r = rig("pin-refused", four());
    r.server.refuse("acc-a");
    let e = Pin::new(r.env.clone()).execute("m1".into(), true).unwrap_err();
    assert_action_failed(e, "Alpha");
    assert_eq!(scalar(&r, "SELECT count(*) FROM pin"), 0);
    assert_eq!(scalar(&r, "SELECT count(*) FROM undo_entry"), 0);
}

#[test]
fn label_creates_a_new_name_when_asked_and_reuses_it_after() {
    let r = rig("label", four());
    let first = Label::new(r.env.clone()).execute(vec!["m1".into()], "Receipts".into(), true).unwrap();
    under_commit(&r);
    assert!(first.created_id.is_some(), "a new label reports its id");
    assert!(first.undo_entry_id.is_some());
    let second = Label::new(r.env.clone()).execute(vec!["m2".into()], "receipts".into(), true).unwrap();
    assert!(second.created_id.is_none(), "an existing name (any case) is reused, not created again");
    assert_eq!(scalar(&r, "SELECT count(*) FROM label WHERE kind = 'owner'"), 1, "the five built-ins (delta 9) are not the owner's");
    assert_eq!(scalar(&r, "SELECT count(*) FROM label_assignment"), 2);
}

#[test]
fn label_without_create_on_a_missing_name_is_not_found() {
    let r = rig("label-missing", four());
    let e = Label::new(r.env.clone()).execute(vec!["m1".into()], "Nope".into(), false).unwrap_err();
    assert!(matches!(e, CoreError::NotFound { .. }));
    assert_eq!(scalar(&r, "SELECT count(*) FROM label WHERE kind = 'owner'"), 0);
}

#[test]
fn label_refused_removes_the_assignment_and_the_label_it_created() {
    let r = rig("label-refused", four());
    r.server.refuse("acc-a");
    let e = Label::new(r.env.clone()).execute(vec!["m1".into()], "Receipts".into(), true).unwrap_err();
    assert_action_failed(e, "Alpha");
    assert_eq!(scalar(&r, "SELECT count(*) FROM label_assignment"), 0);
    assert_eq!(scalar(&r, "SELECT count(*) FROM label WHERE kind = 'owner'"), 0, "a label made for this action goes with it");
}

#[test]
fn undo_of_a_label_removes_the_assignment() {
    let r = rig("label-undo", four());
    Label::new(r.env.clone()).execute(vec!["m1".into(), "m2".into()], "Receipts".into(), true).unwrap();
    Undo::new(r.env.clone()).execute().unwrap();
    assert_eq!(scalar(&r, "SELECT count(*) FROM label_assignment"), 0);
    assert!(matches!(r.server.asked().last().unwrap(), WriteBack::Label { applied: false, .. }));
}

#[test]
fn move_to_account_moves_rows_and_undo_moves_them_back() {
    let r = rig("move", four());
    let out = MoveToAccount::new(r.env.clone()).execute(vec!["m1".into()], "acc-b".into()).unwrap();
    under_commit(&r);
    assert!(out.undo_entry_id.is_some());
    assert_eq!(text(&r, "SELECT account_id FROM message WHERE id = 'm1'"), "acc-b");
    assert_eq!(text(&r, "SELECT account_id FROM thread WHERE id = 't1'"), "acc-b", "a thread whose messages all moved moves too");
    Undo::new(r.env.clone()).execute().unwrap();
    assert_eq!(text(&r, "SELECT account_id FROM message WHERE id = 'm1'"), "acc-a");
    assert_eq!(scalar(&r, "SELECT remote_id IS NULL FROM message WHERE id = 'm1'"), 1, "the server issues a new id for the copy it made; sync fills it in");
    assert!(matches!(r.server.asked().last().unwrap(), WriteBack::Move { from_account_id, to_account_id, .. } if from_account_id == "acc-b" && to_account_id == "acc-a"), "undo also writes back to the server");
    assert_counts_match(&r);
}

#[test]
fn move_to_account_leaves_the_source_untouched_when_the_target_refuses() {
    let r = rig("move-refused", four());
    r.server.refuse("acc-b");
    let e = MoveToAccount::new(r.env.clone()).execute(vec!["m1".into(), "m3".into()], "acc-b".into()).unwrap_err();
    assert_action_failed(e, "Beta");
    for id in ["m1", "m3"] {
        let sql: &'static str = if id == "m1" { "SELECT account_id || '/' || remote_id FROM message WHERE id = 'm1'" } else { "SELECT account_id || '/' || remote_id FROM message WHERE id = 'm3'" };
        assert_eq!(text(&r, sql), format!("acc-a/r-{id}"));
    }
    assert_eq!(text(&r, "SELECT account_id FROM thread WHERE id = 't1'"), "acc-a");
    assert_eq!(scalar(&r, "SELECT count(*) FROM undo_entry"), 0);
    assert_eq!(inbox_ids(&r), ["m4", "m3", "m2", "m1"]);
    assert_counts_match(&r);
}

#[test]
fn move_to_an_unknown_account_is_not_found() {
    let r = rig("move-unknown", four());
    let e = MoveToAccount::new(r.env.clone()).execute(vec!["m1".into()], "acc-zzz".into()).unwrap_err();
    assert!(matches!(e, CoreError::NotFound { .. }));
}

#[test]
fn mark_read_writes_back_and_no_undo_entry() {
    let r = rig("mark-read", four());
    let out = MarkRead::new(r.env.clone()).execute(vec!["m1".into(), "m2".into()], true).unwrap();
    under_commit(&r);
    assert!(out.undo_entry_id.is_none(), "marking read writes no undo entry");
    assert_eq!(scalar(&r, "SELECT count(*) FROM message WHERE read = 1"), 2);
    assert_eq!(r.server.asked().len(), 2, "one write-back per account");
    assert_counts_match(&r);
}

#[test]
fn mark_read_that_changes_nothing_asks_no_server() {
    let r = rig("mark-read-noop", four());
    MarkRead::new(r.env.clone()).execute(vec!["m1".into()], false).unwrap();
    assert!(r.server.asked().is_empty());
}

#[test]
fn mark_read_refused_marks_them_unread_again() {
    let r = rig("mark-read-refused", four());
    r.server.refuse("acc-b");
    let e = MarkRead::new(r.env.clone()).execute(vec!["m2".into()], true).unwrap_err();
    assert_action_failed(e, "Beta");
    assert_eq!(scalar(&r, "SELECT count(*) FROM message WHERE read = 1"), 0);
    assert_counts_match(&r);
}

#[test]
fn undo_is_last_in_first_out() {
    let r = rig("lifo", four());
    Archive::new(r.env.clone()).execute(vec!["m4".into()]).unwrap();
    Pin::new(r.env.clone()).execute("m1".into(), true).unwrap();
    Snooze::new(r.env.clone()).execute(vec!["m2".into()], SnoozeWhen::NextWeek).unwrap();
    let undo = Undo::new(r.env.clone());
    undo.execute().unwrap();
    assert_eq!(scalar(&r, "SELECT count(*) FROM snooze"), 0, "the snooze (last in) goes first");
    assert_eq!(scalar(&r, "SELECT count(*) FROM pin"), 1);
    undo.execute().unwrap();
    assert_eq!(scalar(&r, "SELECT count(*) FROM pin"), 0, "then the pin");
    assert_eq!(scalar(&r, "SELECT archived FROM message WHERE id = 'm4'"), 1);
    undo.execute().unwrap();
    assert_eq!(scalar(&r, "SELECT archived FROM message WHERE id = 'm4'"), 0, "then the archive");
    let e = undo.execute().unwrap_err();
    assert_eq!(e.to_string(), "undo: Nothing to undo");
    assert_counts_match(&r);
}

#[test]
fn undo_depth_is_the_ledger_value() {
    let r = rig("depth", four());
    let depth = constant("UNDO_DEPTH") as usize;
    for i in 0..depth + 3 {
        Pin::new(r.env.clone()).execute("m1".into(), i % 2 == 0).unwrap();
    }
    assert_eq!(scalar(&r, "SELECT count(*) FROM undo_entry WHERE consumed = 0") as usize, depth, "older entries fall off");
}

#[test]
fn undo_refused_by_the_server_keeps_the_entry_for_another_try() {
    let r = rig("undo-refused", four());
    Archive::new(r.env.clone()).execute(vec!["m4".into()]).unwrap();
    r.server.refuse("acc-b");
    let e = Undo::new(r.env.clone()).execute().unwrap_err();
    assert_action_failed(e, "Beta");
    assert_eq!(scalar(&r, "SELECT archived FROM message WHERE id = 'm4'"), 1, "still archived");
    assert_eq!(scalar(&r, "SELECT count(*) FROM undo_entry WHERE consumed = 0"), 1, "the entry is not consumed");
    r.server.accept_all();
    Undo::new(r.env.clone()).execute().unwrap();
    assert_eq!(scalar(&r, "SELECT archived FROM message WHERE id = 'm4'"), 0);
}

#[test]
fn an_unknown_message_is_not_found_and_changes_nothing() {
    let r = rig("unknown", four());
    let e = Archive::new(r.env.clone()).execute(vec!["m1".into(), "nope".into()]).unwrap_err();
    assert!(matches!(e, CoreError::NotFound { .. }));
    assert_eq!(scalar(&r, "SELECT count(*) FROM message WHERE archived = 1"), 0, "m1 was not archived either: one action, all or nothing");
}
