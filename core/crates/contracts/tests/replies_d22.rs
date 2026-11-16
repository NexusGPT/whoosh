//! Delta 22 probes: attachments on a draft, the refusal codes and the failure event reply-engine asked for.
//! MUTANTS (_captures/replies-contract/mutants.py V01..V15; each must turn the test named red):
//!   V01 a local file that is gone is accepted                                         -> a_file_is_planned_or_refused_in_words
//!   V02 a file exactly at the per-file cap is refused (boundary)                      -> a_file_is_planned_or_refused_in_words
//!   V03 the total cap ignores the file being added                                    -> a_file_is_planned_or_refused_in_words
//!   V04 the same source twice is added twice                                          -> adding_the_same_source_twice_is_one_file
//!   V05 a stored attachment already on this Mac plans WillDownload (and the reverse)  -> a_stored_attachment_is_ready_or_will_download_by_where_it_is
//!   V06 a relative path is accepted                                                   -> a_file_is_planned_or_refused_in_words
//!   V07 forward lists the attachments in the wrong order                              -> forward_puts_every_attachment_of_the_original_on_the_draft
//!   V08 forward does not flag a file over the cap                                     -> forward_puts_every_attachment_of_the_original_on_the_draft
//!   V09 a file that is gone does not stop a send                                      -> a_send_is_refused_for_the_first_file_that_cannot_go
//!   V10 the total cap does not stop a send                                            -> a_send_is_refused_for_the_first_file_that_cannot_go
//!   V11 a closed draft accepts files                                                  -> a_closed_draft_takes_no_file
//!   V12 the JSON spells `total_bytes` as `total`                                      -> the_state_json_names_its_keys_and_the_state_spelling_round_trips
//!   V13 a file name is the FIRST component of its path                                -> the_name_of_a_path_is_its_last_component
//!   V14 the state `will_download` is spelt `will-download`                            -> the_state_json_names_its_keys_and_the_state_spelling_round_trips
//!   V15 the proposed caps change silently                                             -> the_caps_and_the_codes_are_pinned
use contracts::*;

fn code_of<T: std::fmt::Debug>(r: Result<T, CoreError>) -> String { match r { Err(CoreError::Invalid { field, .. }) => field, o => panic!("expected a refusal, got {o:?}") } }
fn lf(p: &str) -> AttachmentSource { AttachmentSource::LocalFile { path: p.into() } }
fn st(m: &str, a: &str) -> AttachmentSource { AttachmentSource::StoredMessage { message_id: m.into(), attachment_id: a.into() } }
fn have(id: &str, source: AttachmentSource, size: u64, state: OutgoingAttachmentState) -> OutgoingAttachment {
    OutgoingAttachment { id: id.into(), owner_id: "d1".into(), source, file_name: "f.pdf".into(), media_type: "application/pdf".into(), size_bytes: size, state }
}
const MIB: u64 = 1024 * 1024;

#[test]
fn a_file_is_planned_or_refused_in_words() {
    let plan = |ex: Vec<OutgoingAttachment>, p: &str, size: Option<u64>| plan_add_attachment(ex, lf(p), None, size, false);
    assert_eq!(plan(vec![], "~/Documents/Devis.pdf", Some(1200)).unwrap(), AttachmentAddPlan::Add { state: OutgoingAttachmentState::Ready, file_name: "Devis.pdf".into() });
    assert_eq!(code_of(plan(vec![], "~/Documents/Devis.pdf", None)), ATTACHMENT_GONE_CODE, "the core found no file");
    assert_eq!(code_of(plan(vec![], "Documents/Devis.pdf", Some(1))), ATTACHMENT_BAD_SOURCE_CODE, "a relative path");
    assert_eq!(code_of(plan(vec![], "/", Some(1))), ATTACHMENT_BAD_SOURCE_CODE, "no file name");
    plan(vec![], "~/big.bin", Some(ATTACHMENT_FILE_MAX_BYTES)).expect("exactly the cap is allowed");
    assert_eq!(code_of(plan(vec![], "~/big.bin", Some(ATTACHMENT_FILE_MAX_BYTES + 1))), ATTACHMENT_TOO_LARGE_CODE);
    // the total counts what is already there PLUS the file being added
    let held = vec![have("a1", lf("~/a"), 9 * MIB, OutgoingAttachmentState::Ready), have("a2", lf("~/b"), 4 * MIB, OutgoingAttachmentState::Ready)];
    plan(held.clone(), "~/c", Some(MIB)).expect("13 + 1 = 14 MiB is the cap exactly");
    assert_eq!(code_of(plan(held, "~/c", Some(MIB + 1))), ATTACHMENT_TOTAL_TOO_LARGE_CODE);
    assert_eq!(code_of(plan_add_attachment(vec![], st(" ", "x"), None, Some(1), true)), ATTACHMENT_BAD_SOURCE_CODE);
    assert_eq!(code_of(plan_add_attachment(vec![], st("m", ""), None, Some(1), true)), ATTACHMENT_BAD_SOURCE_CODE);
}

#[test]
fn adding_the_same_source_twice_is_one_file() {
    let held = vec![have("a1", lf("~/a.pdf"), 10, OutgoingAttachmentState::Ready), have("a2", st("m1", "att"), 10, OutgoingAttachmentState::WillDownload)];
    assert_eq!(plan_add_attachment(held.clone(), lf("~/a.pdf"), None, Some(10), false).unwrap(), AttachmentAddPlan::AlreadyThere { attachment_id: "a1".into() });
    assert_eq!(plan_add_attachment(held.clone(), st("m1", "att"), None, Some(10), false).unwrap(), AttachmentAddPlan::AlreadyThere { attachment_id: "a2".into() });
    assert!(matches!(plan_add_attachment(held, st("m1", "att-2"), None, Some(10), false).unwrap(), AttachmentAddPlan::Add { .. }), "another attachment of the same message is new");
}

#[test]
fn a_stored_attachment_is_ready_or_will_download_by_where_it_is() {
    let plan = |local| plan_add_attachment(vec![], st("m1", "att"), Some("Plan.png".into()), Some(900), local).unwrap();
    assert_eq!(plan(true), AttachmentAddPlan::Add { state: OutgoingAttachmentState::Ready, file_name: "Plan.png".into() });
    assert_eq!(plan(false), AttachmentAddPlan::Add { state: OutgoingAttachmentState::WillDownload, file_name: "Plan.png".into() }, "the core fetches it when the message is built");
}

fn original(name: &str, id: &str, size: u64, local: bool) -> AttachmentFile {
    AttachmentFile { id: id.into(), message_id: "m9".into(), file_name: name.into(), mime_type: "application/pdf".into(), size_bytes: size, local_path: local.then(|| "/x".to_string()), inline: false, content_id: None }
}

#[test]
fn forward_puts_every_attachment_of_the_original_on_the_draft() {
    let list = forward_attachments("d7".into(), vec![original("a.pdf", "att-a", 100, true), original("b.pdf", "att-b", 200, false), original("huge.zip", "att-c", ATTACHMENT_FILE_MAX_BYTES + 1, false)]);
    assert_eq!(list.iter().map(|a| a.id.as_str()).collect::<Vec<_>>(), ["d7/1", "d7/2", "d7/3"], "ids from 1, in the original's order");
    assert_eq!(list.iter().map(|a| a.file_name.as_str()).collect::<Vec<_>>(), ["a.pdf", "b.pdf", "huge.zip"]);
    assert_eq!(list.iter().map(|a| a.state).collect::<Vec<_>>(), [OutgoingAttachmentState::Ready, OutgoingAttachmentState::WillDownload, OutgoingAttachmentState::TooLarge], "a file over the cap is listed, flagged, never dropped");
    assert_eq!(list[1].source, st("m9", "att-b"));
    assert!(list.iter().all(|a| a.owner_id == "d7"));
    assert!(forward_attachments("d7".into(), vec![]).is_empty(), "a message with no attachment forwards none");
}

#[test]
fn a_send_is_refused_for_the_first_file_that_cannot_go() {
    let ok = have("a1", lf("~/a"), 10, OutgoingAttachmentState::Ready);
    check_draft_attachments_sendable(vec![ok.clone(), have("a2", st("m", "x"), 10, OutgoingAttachmentState::WillDownload)]).unwrap();
    check_draft_attachments_sendable(vec![]).unwrap();
    assert_eq!(code_of(check_draft_attachments_sendable(vec![ok.clone(), have("a2", lf("~/b"), 10, OutgoingAttachmentState::Gone)])), ATTACHMENT_GONE_CODE);
    assert_eq!(code_of(check_draft_attachments_sendable(vec![have("a2", st("m", "x"), 10, OutgoingAttachmentState::TooLarge)])), ATTACHMENT_TOO_LARGE_CODE);
    let two = vec![have("a1", lf("~/a"), 8 * MIB, OutgoingAttachmentState::Ready), have("a2", lf("~/b"), 8 * MIB, OutgoingAttachmentState::Ready)];
    assert_eq!(code_of(check_draft_attachments_sendable(two)), ATTACHMENT_TOTAL_TOO_LARGE_CODE, "each file is fine, together they are over the cap");
}

#[test]
fn a_closed_draft_takes_no_file() {
    check_attachment_draft_open(true).unwrap();
    assert_eq!(code_of(check_attachment_draft_open(false)), ATTACHMENT_DRAFT_CLOSED_CODE);
}

#[test]
fn the_name_of_a_path_is_its_last_component() {
    assert_eq!(attachment_file_name_of_path("~/Documents/Devis.pdf".into()), "Devis.pdf");
    assert_eq!(attachment_file_name_of_path("/a/b/".into()), "b");
    assert_eq!(attachment_file_name_of_path("only".into()), "only");
}

#[test]
fn the_state_json_names_its_keys_and_the_state_spelling_round_trips() {
    let list = vec![have("a1", lf("~/a.pdf"), 1200, OutgoingAttachmentState::Ready), have("a2", st("m1", "att"), 800, OutgoingAttachmentState::WillDownload)];
    let j: serde_json::Value = serde_json::from_str(&outgoing_attachments_json(list)).unwrap();
    let mut k: Vec<&String> = j.as_object().unwrap().keys().collect(); k.sort();
    assert_eq!(k, ["attachments", "total_bytes"]);
    assert_eq!(j["total_bytes"], 2000);
    let mut k: Vec<&String> = j["attachments"][0].as_object().unwrap().keys().collect(); k.sort();
    assert_eq!(k, ["file_name", "id", "media_type", "owner_id", "size_bytes", "source", "state"]);
    assert_eq!(j["attachments"][0]["source"], serde_json::json!({"kind": "local_file", "path": "~/a.pdf"}));
    assert_eq!(j["attachments"][1]["source"], serde_json::json!({"kind": "stored_message", "message_id": "m1", "attachment_id": "att"}));
    assert_eq!(j["attachments"][1]["state"], "will_download");
    for s in [OutgoingAttachmentState::Ready, OutgoingAttachmentState::WillDownload, OutgoingAttachmentState::Gone, OutgoingAttachmentState::TooLarge] {
        assert_eq!(outgoing_attachment_state_from_key(outgoing_attachment_state_key(s)), Some(s));
    }
    assert_eq!(outgoing_attachment_state_key(OutgoingAttachmentState::WillDownload), "will_download");
    assert_eq!(outgoing_attachment_state_from_key("sent".into()), None);
}

#[test]
fn the_caps_and_the_codes_are_pinned() {
    assert_eq!((ATTACHMENT_FILE_MAX_BYTES, ATTACHMENT_TOTAL_MAX_BYTES), (10 * MIB, 14 * MIB), "proposals for docs/CONSTANTS.md: a change is a ledger decision");
    assert_eq!((AI_NO_MODEL_CODE, REPLY_VOICE_RUNNING_CODE), ("ai_no_model", "reply_voice_running"), "the one no-model code (ai-settings-core) and the running code (reply-engine)");
    let cmds = vec![Command::RemoveProviderKey { provider_id: "openai".into() }, Command::AddDraftAttachment { draft_id: "d1".into(), source: lf("~/a") }, Command::RemoveDraftAttachment { draft_id: "d1".into(), attachment_id: "a1".into() }];
    for c in &cmds { assert_eq!(&c.clone(), c); }
    let ev = CoreEvent::ReplyWorkFailed { thread_id: Some("t1".into()), draft_id: None, run_id: None, reason_words: "No model is set".into() };
    assert_eq!(ev.clone(), ev);
}
