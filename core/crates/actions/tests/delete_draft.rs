//! Command::DeleteDraft through Actions::run, and Z (lead, 13 Nov: the command had no handler in the real core).
mod common;
use common::*;
use contracts::{Command, CoreError};

fn run(r: &Rig, c: Command) -> Result<contracts::CommandOutcome, CoreError> {
    actions::Actions::new(r.env.clone()).run(c)
}

#[test]
fn a_deleted_draft_leaves_and_z_brings_it_back_whole() {
    let r = rig("delete-draft", four());
    let thread = text(&r, "SELECT id FROM thread LIMIT 1");
    let id = run(&r, Command::SaveDraft { from_account_id: None, to: vec!["a@example.test".into()], cc: vec!["c@example.test".into()], subject: "Re: x".into(), body: "Kept words".into(), reply_to_thread_id: Some(thread.clone()) })
        .unwrap()
        .created_id
        .unwrap();
    run(&r, Command::UpdateDraft { draft_id: id.clone(), from_account_id: None, to: vec!["a@example.test".into()], cc: vec!["c@example.test".into()], bcc: vec!["boss@example.test".into()], subject: "Re: x".into(), body: "Kept words".into() }).unwrap();
    let saved = scalar(&r, "SELECT updated_at_ms FROM draft");
    let out = run(&r, Command::DeleteDraft { draft_id: id.clone() }).unwrap();
    assert!(out.undo_entry_id.is_some(), "one undo entry");
    assert_eq!(scalar(&r, "SELECT count(*) FROM draft"), 0, "the draft left");
    run(&r, Command::Undo).unwrap();
    assert_eq!(text(&r, "SELECT id FROM draft"), id, "the same id");
    assert_eq!(text(&r, "SELECT body FROM draft"), "Kept words");
    assert_eq!(text(&r, "SELECT to_json || cc_json || bcc_json FROM draft"), r#"["a@example.test"]["c@example.test"]["boss@example.test"]"#);
    assert_eq!(text(&r, "SELECT reply_to_thread_id FROM draft"), thread);
    assert_eq!(scalar(&r, "SELECT updated_at_ms FROM draft"), saved, "its saved time");
    assert_eq!(scalar(&r, "SELECT count(*) FROM undo_entry WHERE consumed = 0"), 0, "the entry is used");
}

#[test]
fn an_unknown_draft_is_not_found_and_pushes_no_undo() {
    let r = rig("delete-draft-unknown", four());
    assert!(matches!(run(&r, Command::DeleteDraft { draft_id: "draft-nope".into() }), Err(CoreError::NotFound { .. })));
    assert_eq!(scalar(&r, "SELECT count(*) FROM undo_entry"), 0);
}

#[test]
fn two_deleted_drafts_come_back_newest_first() {
    let r = rig("delete-draft-two", four());
    let mk = |body: &str| run(&r, Command::SaveDraft { from_account_id: None, to: vec!["a@example.test".into()], cc: vec![], subject: "s".into(), body: body.into(), reply_to_thread_id: None }).unwrap().created_id.unwrap();
    let (a, b) = (mk("first"), mk("second"));
    run(&r, Command::DeleteDraft { draft_id: a.clone() }).unwrap();
    run(&r, Command::DeleteDraft { draft_id: b.clone() }).unwrap();
    run(&r, Command::Undo).unwrap();
    assert_eq!(text(&r, "SELECT id FROM draft"), b, "the last deleted comes back first");
    run(&r, Command::Undo).unwrap();
    assert_eq!(scalar(&r, "SELECT count(*) FROM draft"), 2);
}
