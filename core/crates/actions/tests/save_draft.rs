//! Delta 8, group 6: Command::SaveDraft through Actions::run.
mod common;
use common::*;
use contracts::{Command, CoreError};

fn save(r: &Rig, from: Option<&str>, to: &[&str], subject: &str, body: &str, thread: Option<&str>) -> Result<contracts::CommandOutcome, CoreError> {
    actions::Actions::new(r.env.clone()).run(Command::SaveDraft { from_account_id: from.map(String::from), to: to.iter().map(|s| s.to_string()).collect(), cc: vec![], subject: subject.into(), body: body.into(), reply_to_thread_id: thread.map(String::from) })
}

#[test]
fn a_draft_is_saved_with_its_account_and_a_reply_draft_takes_its_threads_account() {
    let r = rig("save-draft", four());
    let out = save(&r, None, &["a@example.test", "b\"q@example.test"], "Hello", "Body", None).unwrap();
    let id = out.created_id.expect("the draft id");
    assert_eq!(text(&r, "SELECT subject FROM draft WHERE id = (SELECT id FROM draft LIMIT 1)"), "Hello");
    assert_eq!(scalar(&r, "SELECT count(*) FROM draft WHERE json_valid(to_json) AND json_array_length(to_json) = 2"), 1, "the recipients are a valid JSON list even with a quote in one");
    assert!(!id.is_empty());
    let thread_account = text(&r, "SELECT account_id FROM thread LIMIT 1");
    let thread = text(&r, "SELECT id FROM thread LIMIT 1");
    save(&r, None, &[], "Re: x", "", Some(&thread)).unwrap();
    assert_eq!(text(&r, "SELECT from_account_id FROM draft WHERE reply_to_thread_id IS NOT NULL"), thread_account);
}

#[test]
fn an_empty_draft_and_an_unknown_account_are_refused_and_save_nothing() {
    let r = rig("save-draft-bad", four());
    assert!(matches!(save(&r, None, &["  "], " ", "", None), Err(CoreError::Invalid { .. })));
    assert!(matches!(save(&r, Some("acc-nope"), &["a@example.test"], "s", "b", None), Err(CoreError::NotFound { .. })));
    assert_eq!(scalar(&r, "SELECT count(*) FROM draft"), 0);
}

#[test]
fn updating_a_draft_keeps_its_id_and_thread_and_pushes_no_undo() {
    let r = rig("update-draft", four());
    let thread = text(&r, "SELECT id FROM thread LIMIT 1");
    let id = save(&r, None, &["a@example.test"], "Re: x", "First", Some(&thread)).unwrap().created_id.unwrap();
    let undo_before = scalar(&r, "SELECT count(*) FROM undo_entry");
    let body_q = "SELECT body FROM draft"; // the one draft (count checked below)
    let thread_q = "SELECT reply_to_thread_id FROM draft";
    let out = actions::Actions::new(r.env.clone()).run(Command::UpdateDraft { draft_id: id.clone(), from_account_id: None, to: vec!["b@example.test".into()], cc: vec![], bcc: vec!["boss@example.test".into()], subject: "Re: x".into(), body: "Second".into() }).unwrap();
    assert_eq!((out.undo_entry_id, out.created_id.as_deref()), (None, Some(id.as_str())));
    assert_eq!(scalar(&r, "SELECT count(*) FROM draft"), 1, "one draft, no second copy");
    assert_eq!(text(&r, "SELECT id FROM draft"), id);
    assert_eq!(text(&r, body_q), "Second");
    assert_eq!(text(&r, thread_q), thread);
    assert_eq!(text(&r, "SELECT bcc_json FROM draft"), r#"["boss@example.test"]"#, "delta 12: the Bcc is kept with the draft");
    assert_eq!(scalar(&r, "SELECT count(*) FROM undo_entry"), undo_before, "no undo entry");
    let bad = |draft: &str, to: &str| actions::Actions::new(r.env.clone()).run(Command::UpdateDraft { draft_id: draft.into(), from_account_id: None, to: vec![to.into()], cc: vec![], bcc: vec![], subject: " ".into(), body: "".into() });
    assert!(matches!(bad("draft-nope", "c@example.test"), Err(CoreError::NotFound { .. })));
    assert!(matches!(bad(&id, " "), Err(CoreError::Invalid { .. })));
    assert_eq!(text(&r, body_q), "Second", "a refused update changes nothing");
}
