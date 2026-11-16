//! Delta 23 probes: received attachments (transfer state, the gallery, the download rule) and excerpts that are not mail (source chunks).
//! MUTANTS (_captures/replies-contract/mutants.py X01..X13; each must turn the test named red):
//!   X01 a file with a local path reads Downloading when a fetch is in flight (the path no longer wins)   -> a_file_is_here_coming_failed_or_not_on_this_mac_yet
//!   X02 the last failure is never reported                                                                  -> a_file_is_here_coming_failed_or_not_on_this_mac_yet
//!   X03 the gallery keeps the inline images                                                                 -> the_gallery_leaves_the_inline_images_out_and_keeps_the_order
//!   X04 a second download of a file already coming starts a second fetch                                    -> a_download_is_answered_at_once_and_is_idempotent
//!   X05 a file exactly at the keep cap is refused (boundary)                                                -> a_download_is_answered_at_once_and_is_idempotent
//!   X06 a file already on this Mac is fetched again                                                         -> a_download_is_answered_at_once_and_is_idempotent
//!   X07 a source chunk's vector key is its bare id (it meets a mail chunk's key)                            -> the_two_chunk_kinds_never_share_a_vector_key_or_an_excerpt_id
//!   X08 the first key of the source space reads as a mail chunk                                             -> the_two_chunk_kinds_never_share_a_vector_key_or_an_excerpt_id
//!   X09 a source chunk's excerpt id is plain decimal (it reads as a mail chunk)                             -> the_two_chunk_kinds_never_share_a_vector_key_or_an_excerpt_id
//!   X10 the keys of a MAIL folder read as a source chunk origin                                             -> a_source_chunk_names_its_source_and_its_item_and_round_trips
//!   X11 `check_source_chunk` lets blank text through                                                        -> a_source_chunk_names_its_source_and_its_item_and_round_trips
//!   X12 the JSON spells the origin kind `files`                                                             -> the_state_json_names_its_keys
//!   X13 the transfer state `not_on_this_mac` is spelt with dashes                                           -> the_state_json_names_its_keys
use contracts::*;

fn code_of<T: std::fmt::Debug>(r: Result<T, CoreError>) -> String { match r { Err(CoreError::Invalid { field, .. }) => field, o => panic!("expected a refusal, got {o:?}") } }
fn file(id: &str, size: u64, local: bool, inline: bool) -> AttachmentFile {
    AttachmentFile { id: id.into(), message_id: "m1".into(), file_name: format!("{id}.pdf"), mime_type: "application/pdf".into(), size_bytes: size, local_path: local.then(|| "/x".to_string()), inline, content_id: inline.then(|| "logo@x".to_string()) }
}

#[test]
fn a_file_is_here_coming_failed_or_not_on_this_mac_yet() {
    let t = |f: AttachmentFile, flight, fail: Option<&str>| attachment_transfer_of(f, flight, fail.map(String::from));
    assert_eq!(t(file("a", 1, false, false), false, None), AttachmentTransfer::NotOnThisMac, "no path: not on this Mac yet, read at once");
    assert_eq!(t(file("a", 1, false, false), true, None), AttachmentTransfer::Downloading);
    assert_eq!(t(file("a", 1, true, false), true, Some("timeout")), AttachmentTransfer::OnThisMac, "the path wins: the file is here whatever happened before");
    assert_eq!(t(file("a", 1, false, false), false, Some("the server said no")), AttachmentTransfer::Failed { reason_words: "the server said no".into() });
    assert_eq!(t(file("a", 1, false, false), true, Some("old failure")), AttachmentTransfer::Downloading, "a new fetch hides the last failure");
    assert_eq!(t(file("a", 1, false, false), false, Some("  ")), AttachmentTransfer::NotOnThisMac, "blank words are no failure");
}

#[test]
fn the_gallery_leaves_the_inline_images_out_and_keeps_the_order() {
    let g = gallery_attachments(vec![file("a", 1, true, false), file("logo", 1, false, true), file("b", 1, false, false)]);
    assert_eq!(g.iter().map(|f| f.id.as_str()).collect::<Vec<_>>(), ["a", "b"]);
    assert!(gallery_attachments(vec![file("logo", 1, false, true)]).is_empty());
}

#[test]
fn a_download_is_answered_at_once_and_is_idempotent() {
    let plan = |f: Option<AttachmentFile>, flight| plan_download_attachment("a".into(), f, flight);
    assert_eq!(plan(Some(file("a", 10, false, false)), false).unwrap(), DownloadPlan::Start);
    assert_eq!(plan(Some(file("a", 10, true, false)), false).unwrap(), DownloadPlan::AlreadyOnThisMac, "already here: not fetched again");
    assert_eq!(plan(Some(file("a", 10, false, false)), true).unwrap(), DownloadPlan::AlreadyDownloading, "already coming: one fetch");
    assert!(matches!(plan(None, false), Err(CoreError::NotFound { .. })), "an unknown file");
    assert_eq!(plan(Some(file("a", ATTACHMENT_KEEP_MAX_BYTES, false, false)), false).unwrap(), DownloadPlan::Start, "exactly the cap is kept");
    assert_eq!(code_of(plan(Some(file("a", ATTACHMENT_KEEP_MAX_BYTES + 1, false, false)), false)), ATTACHMENT_DOWNLOAD_REFUSAL_CODE);
    assert_eq!(ATTACHMENT_KEEP_MAX_BYTES, 25 * 1024 * 1024, "proposal for docs/CONSTANTS.md: a change is a ledger decision");
}

fn sc(id: i64, origin: SourceChunkOrigin) -> SourceChunk { SourceChunk { id, origin, chunk_no: 0, date_ms: Some(5), title: "Proposition Q4.pdf".into(), text: "Trois ateliers.".into(), embedded_at_ms: None } }
fn fileo() -> SourceChunkOrigin { SourceChunkOrigin::File { folder_path: "~/Documents/Propositions".into(), item_path: "2026/Proposition Q4.pdf".into() } }
fn evo() -> SourceChunkOrigin { SourceChunkOrigin::Event { calendar_id: "cal-work".into(), event_id: "ev-1".into() } }

#[test]
fn a_source_chunk_names_its_source_and_its_item_and_round_trips() {
    for o in [fileo(), evo()] {
        let (s, i) = (source_chunk_source_key(o.clone()), source_chunk_item_key(o.clone()));
        assert_eq!(source_chunk_origin_from_keys(s, i), Some(o));
    }
    assert_eq!(source_chunk_source_key(fileo()), reply_source_key(ReplySourceRef::LocalFolder { path: "~/Documents/Propositions".into() }), "the source key is the reply-source key: a removed source takes its chunks");
    assert_eq!(source_chunk_origin_from_keys(reply_source_key(ReplySourceRef::MailFolder { account_id: "a".into(), folder: "INBOX".into() }), "x".into()), None, "mail has its own chunk table");
    assert_eq!(source_chunk_origin_from_keys(source_chunk_source_key(fileo()), " ".into()), None);
    check_source_chunk(sc(1, fileo())).unwrap();
    assert_eq!(code_of(check_source_chunk(sc(0, fileo()))), SOURCE_CHUNK_REFUSAL_CODE);
    assert_eq!(code_of(check_source_chunk(SourceChunk { text: " ".into(), ..sc(1, fileo()) })), SOURCE_CHUNK_REFUSAL_CODE);
    assert_eq!(code_of(check_source_chunk(sc(1, SourceChunkOrigin::Event { calendar_id: "".into(), event_id: "e".into() }))), SOURCE_CHUNK_REFUSAL_CODE);
}

#[test]
fn the_two_chunk_kinds_never_share_a_vector_key_or_an_excerpt_id() {
    assert_eq!(chunk_vector_key(42), 42, "a mail chunk's key is unchanged: delta 21's rows and vectors stay valid");
    assert_eq!(source_chunk_vector_key(1), SOURCE_CHUNK_KEY_BASE + 1);
    assert!(!vector_key_is_source_chunk(chunk_vector_key(1_000_000_000)) && vector_key_is_source_chunk(source_chunk_vector_key(1)));
    assert!(vector_key_is_source_chunk(SOURCE_CHUNK_KEY_BASE), "the first key of the source space");
    assert_eq!(source_chunk_id_from_vector_key(source_chunk_vector_key(77)), Some(77));
    assert_eq!(source_chunk_id_from_vector_key(chunk_vector_key(77)), None, "a mail chunk's key names no source chunk");
    assert_eq!(source_chunk_excerpt_id(7), "s7");
    assert_eq!(source_chunk_id_from_excerpt_id("s7".into()), Some(7));
    assert_eq!(source_chunk_id_from_excerpt_id("7".into()), None, "a plain decimal is a mail chunk");
    assert_eq!(chunk_id_from_excerpt_id("s7".into()), None, "and a source chunk id is no mail chunk id");
    assert_eq!(source_chunk_id_from_excerpt_id("s0".into()), None);
}

#[test]
fn the_state_json_names_its_keys() {
    let j: serde_json::Value = serde_json::from_str(&source_chunk_json(sc(3, fileo()))).unwrap();
    let mut k: Vec<&String> = j.as_object().unwrap().keys().collect(); k.sort();
    assert_eq!(k, ["chunk_no", "date_ms", "embedded_at_ms", "id", "origin", "text", "title"]);
    assert_eq!(j["origin"], serde_json::json!({"kind": "file", "folder_path": "~/Documents/Propositions", "item_path": "2026/Proposition Q4.pdf"}));
    let e: serde_json::Value = serde_json::from_str(&source_chunk_json(SourceChunk { date_ms: None, ..sc(4, evo()) })).unwrap();
    assert_eq!(e["origin"], serde_json::json!({"kind": "event", "calendar_id": "cal-work", "event_id": "ev-1"}));
    assert!(e["date_ms"].is_null() && e["embedded_at_ms"].is_null());
    assert_eq!(attachment_transfer_json(AttachmentTransfer::NotOnThisMac), "{\"state\":\"not_on_this_mac\"}");
    assert_eq!(attachment_transfer_json(AttachmentTransfer::Downloading), "{\"state\":\"downloading\"}");
    assert_eq!(attachment_transfer_json(AttachmentTransfer::OnThisMac), "{\"state\":\"on_this_mac\"}");
    let f: serde_json::Value = serde_json::from_str(&attachment_transfer_json(AttachmentTransfer::Failed { reason_words: "no \"route\"".into() })).unwrap();
    assert_eq!((f["state"].clone(), f["reason_words"].clone()), (serde_json::json!("failed"), serde_json::json!("no \"route\"")));
    let cmd = Command::DownloadAttachment { attachment_id: "m1:att:001".into() };
    assert_eq!(cmd.clone(), cmd);
    let ev = CoreEvent::AttachmentTransferChanged { attachment_id: "m1:att:001".into(), message_id: "m1".into(), transfer: AttachmentTransfer::OnThisMac };
    assert_eq!(ev.clone(), ev);
}
