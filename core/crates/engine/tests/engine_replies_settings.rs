//! Delta 20 in the real core (docs/lanes/replies-settings.md): every setting of AI replies is stored and served by `RustCore` over a real migrated store.
//! Each command family is run through `RustCore::run` and read back through its state path `reply/...`; each refusal code of the contract is reached once;
//! the JSON is the contract's encoders' (a fresh core answers exactly what the encoders say of the contract's defaults); the working commands answer
//! through the one port `ReplyWork` (the default refuses with `engine.not_built_yet`; a plugged work receives jobs the contract's rules already checked).
//!
//! Mutants (single edits; each was run and seen red, the verbatim results are in _captures/replies-settings/mutants-run.log; the runner is
//! _captures/replies-settings/run-mutants.py):
//! - E1 replies.rs `run`: SetReplyUseEnabled writes the match-thread switch
//! - E2 replies.rs `run`: SetReplyFilterExitRule writes the entry rule
//! - E3 replies.rs `run`: SetOnDemandOption writes the opposite value
//! - E4 replies.rs `run`: SetRetrievalSetting answers Ok and writes nothing
//! - E5 replies.rs `run`: AddReplyLocalFolder no longer asks the disk
//! - E6 replies.rs `run`: RunVoiceAnalysis no longer checks the sample size
//! - E7 replies.rs `run`: DeleteReplyTone answers Ok and deletes nothing
//! - E8 replies.rs `preview`: the prompt is returned unrendered
//! - E9 replies.rs `answer`: reply/counts opens a zero-length window
//! - E10 replies.rs `work_for`: the installed work is ignored
//! - E11 replies.rs `run`: DraftReplyOnDemand no longer checks the instruction against its switch
//! - E12 rust_core.rs `state`: the reply state line is removed
//! - E13 rust_core.rs `run`: the reply command line is removed
//! - E14 replies.rs `sources_json`: the calendars are left out of the pane
//! - E15 replies.rs `run`: SaveFixedNote answers Ok and stores nothing
//! - E16 to E20 (delta 21): the sources pane serves no calendars; reply/calendars/enabled lists every calendar; reply/progress answers empty; SyncReplyCalendars stores nothing; reply/calendars/events answers empty
#[allow(dead_code, unused_imports)]
#[path = "../../autoconfig/tests/support/mod.rs"]
mod detect_fakes;

#[allow(dead_code, unused_imports)]
#[path = "../../sync-imap/tests/support/mod.rs"]
mod imap_fakes;

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};

use contracts::*;
use detect_fakes::{FakeDns, FakeHttp, FakeProbe};
use imap_fakes::{smtp::FakeSmtp, FakeImap, Mbox};
use engine::replies::{install_reply_work, ReplyJob, ReplyWork};
use engine::{RustCore, TestTransport};
use secrets::{MemoryKeychain, Namespace};
use store::{NewMessage, Store};

const USER: &str = "owner@example.test";

fn detector() -> Box<dyn engine::Detect> { Box::new(autoconfig::Detector::new(FakeHttp::default(), FakeDns::default(), FakeProbe::default())) }
fn kc() -> Arc<MemoryKeychain> { Arc::new(MemoryKeychain::new(Namespace::rig())) }
fn config() -> AccountConfig {
    AccountConfig {
        imap_host: "127.0.0.1".into(), imap_port: 1, imap_security: ConnectionSecurity::Ssl, smtp_host: "127.0.0.1".into(), smtp_port: 1,
        smtp_security: ConnectionSecurity::Starttls, username: USER.into(), smtp_username: USER.into(), auth_method: AuthMethod::Password, source: DetectionSource::Manual,
    }
}
fn now() -> i64 { std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as i64 }
fn mail(account: &str, id: &str, thread: &str, at: i64) -> NewMessage {
    NewMessage {
        message: Message {
            id: id.into(), thread_id: thread.into(), account_id: account.into(), sender_name: format!("Sender {id}"), from_address: format!("{id}@ext.test"), to: vec![USER.into()],
            subject: format!("Subject {id}"), preview: format!("Preview {id}"), received_at_ms: at, read: false, archived: false, folder: "INBOX".into(),
            attachment_names: vec![], login_code: None, trackers_blocked: 0, list_unsubscribe: vec![], list_unsubscribe_one_click: false,
            reply_to_name: None, cc: vec![], bcc: vec![], is_outgoing: false,
        },
        remote_id: Some(id.into()), body_text: Some(format!("body of {id}")), body_html: None, has_list_header: false, size_bytes: Some(100), tab_id: None,
    }
}
fn seeded() -> (RustCore, String, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(&dir.path().join("whoosh.sqlite")).unwrap();
    let account = store.add_account(USER, "Owner", MailProvider::Imap, "svc\u{1f}acct", false, config(), 1).unwrap();
    let core = RustCore::with_parts(store, kc(), detector(), TestTransport::PlainLoopback);
    engine::replies_work::hold_background(core.store().path(), true);
    (core, account, dir)
}

fn st(core: &RustCore, path: &str) -> String { core.state(path.into()).unwrap_or_else(|| panic!("no answer for {path}")) }
fn val(core: &RustCore, path: &str) -> serde_json::Value { serde_json::from_str(&st(core, path)).unwrap_or_else(|e| panic!("{path} is not JSON: {e}")) }
fn run(core: &RustCore, c: Command) -> CommandOutcome { core.run(c).unwrap_or_else(|e| panic!("refused: {e}")) }
fn refused(core: &RustCore, c: Command) -> String {
    match core.run(c).expect_err("a refusal") {
        CoreError::Invalid { field, .. } => field,
        CoreError::NotFound { kind, .. } => format!("not_found:{kind}"),
        CoreError::Action { failure } => failure.code.unwrap_or_default(),
        other => panic!("not a refusal: {other}"),
    }
}
fn sql(core: &RustCore, q: &'static str) { core.store().write(move |c| Ok(c.tx.execute(q, [])?)).unwrap(); }
fn labels(core: &RustCore) { sql(core, "INSERT INTO label (id, name, colour, path, depth, updated_at_ms) VALUES ('l1', 'Clients', '#fff', 'Clients', 1, 0), ('l2', 'Team', '#fff', 'Team', 1, 0)"); }
fn rule(words: &str) -> RuleSet { RuleSet { conditions: vec![RuleCondition::From { words: words.into() }], match_mode: MatchMode::Any } }
fn live_draft(core: &RustCore, id: &'static str, thread: &'static str, message: &'static str, state: &'static str) {
    core.store().write(move |c| Ok(c.tx.execute(
        "INSERT INTO reply_draft (id, thread_id, message_id, text, made_by, excerpt_ids_json, instruction, state, revision, created_at_ms, updated_at_ms) VALUES (?1, ?2, ?3, 'Hello', 'on_demand', '[]', NULL, ?4, 0, 5, 6)",
        [id, thread, message, state])?)).unwrap();
}

// ---------------------------------------------------------------- the defaults are the contract's, spelt by the contract's encoders

#[test]
fn a_fresh_core_answers_what_the_contracts_encoders_say_of_the_contracts_defaults() {
    let (core, _, _d) = seeded();
    assert_eq!(st(&core, "reply/use/auto"), reply_use_json(ReplyUse { kind: ReplyUseKind::Auto, enabled: false, tone_id: "voice".into(), match_thread: true, prompt_id: "replies".into() }));
    assert_eq!(st(&core, "reply/use/on_demand"), reply_use_json(ReplyUse { kind: ReplyUseKind::OnDemand, enabled: true, tone_id: "voice".into(), match_thread: true, prompt_id: "replies".into() }));
    assert_eq!(st(&core, "reply/filter"), reply_filter_json(default_reply_filter()));
    assert_eq!(st(&core, "reply/on_demand"), on_demand_json(default_on_demand_options()));
    assert_eq!(st(&core, "reply/retrieval"), retrieval_json(default_retrieval_settings()));
    assert_eq!(st(&core, "reply/voice"), reply_voice_json(default_reply_voice(), ReplyVoiceAnalysis::Idle, vec![]));
    assert_eq!(st(&core, "reply/counts"), reply_counts_json(ReplyStepCounts { days: reply_counts_days(), received: 0, after_entry: 0, after_ai: 0, after_exit: 0, drafts: 0 }));
    assert_eq!(st(&core, "reply/sources"), reply_sources_json(reply_sources_read(vec![], vec![], vec![], RetrievalScope::ThisAccount)));
    let v1 = ReplyPrompt { prompt_id: "replies".into(), version: 1, text: default_reply_prompt_text(), created_at_ms: 0, current: true };
    assert_eq!(st(&core, "reply/prompt/current"), reply_prompt_json(v1.clone()));
    assert_eq!(st(&core, "reply/prompt/versions"), reply_prompt_versions_json(vec![v1]));
    assert_eq!(st(&core, "reply/prompt/variables"), reply_prompt_variables_json(reply_prompt_variables()));
    assert_eq!(st(&core, "reply/notes"), "[]");
    for absent in ["reply/use/nobody", "reply/nothing", "reply/draft/t1", "reply/try/run-1", "message/m1/muted"] {
        assert_eq!(core.state(absent.into()), None, "{absent} has no answer on a fresh core");
    }
}
// mutants E12 and E13: the reply line of `RustCore::state` / `run` is removed (every test of this file goes red: nothing answers).

// ---------------------------------------------------------------- the settings families, each set then read back through its state path

#[test]
fn the_two_uses_are_switched_and_set_apart_through_their_state_paths() {
    let (core, _, _d) = seeded();
    run(&core, Command::SetReplyUseEnabled { kind: ReplyUseKind::Auto, enabled: true });
    assert_eq!(val(&core, "reply/use/auto")["enabled"], true);
    assert_eq!(val(&core, "reply/use/auto")["match_thread"], true, "the match-thread switch did not move");
    assert_eq!(val(&core, "reply/use/on_demand")["enabled"], true, "the other use did not move");
    run(&core, Command::SetReplyUseMatchThread { kind: ReplyUseKind::OnDemand, on: false });
    assert_eq!(val(&core, "reply/use/on_demand")["match_thread"], false);
    assert_eq!(val(&core, "reply/use/on_demand")["enabled"], true, "the enabled switch did not move");
    let tone = run(&core, Command::SaveReplyTone { tone: ReplyTone { id: String::new(), name: "Formel".into(), text: "Vouvoyer.".into() } }).created_id.expect("the tone's id");
    run(&core, Command::SetReplyUseTone { kind: ReplyUseKind::Auto, tone_id: tone.clone() });
    assert_eq!(val(&core, "reply/use/auto")["tone_id"], tone.as_str());
    run(&core, Command::SetReplyUsePrompt { kind: ReplyUseKind::OnDemand, prompt_id: "replies".into() });
    assert_eq!(refused(&core, Command::SetReplyUseTone { kind: ReplyUseKind::Auto, tone_id: "ghost".into() }), "not_found:tone");
    assert_eq!(refused(&core, Command::SetReplyUsePrompt { kind: ReplyUseKind::Auto, prompt_id: "ghost".into() }), "not_found:prompt");
}
// mutant E1: SetReplyUseEnabled's arm calls reply_set_use_match_thread.

#[test]
fn the_filter_cards_labels_rules_reads_and_question() {
    let (core, _, _d) = seeded();
    labels(&core);
    run(&core, Command::SetReplyFilterScope { scope: ReplyLabelScope::Labels { label_ids: vec!["l1".into(), "l2".into()] } });
    assert_eq!(val(&core, "reply/filter")["scope"], serde_json::json!({"all": false, "label_ids": ["l1", "l2"]}));
    run(&core, Command::SetReplyFilterScope { scope: ReplyLabelScope::All });
    assert_eq!(val(&core, "reply/filter")["scope"], serde_json::json!({"all": true, "label_ids": []}));
    run(&core, Command::SetReplyFilterCard { card: FlowCard::Entry, enabled: false });
    run(&core, Command::SetReplyFilterCard { card: FlowCard::Exit, enabled: false });
    let f = val(&core, "reply/filter");
    assert_eq!((f["entry"]["enabled"].as_bool(), f["ai"]["enabled"].as_bool(), f["exit"]["enabled"].as_bool()), (Some(false), Some(true), Some(false)));
    run(&core, Command::SetReplyFilterEntryRule { rule: rule("boss@example.test") });
    run(&core, Command::SetReplyFilterExitRule { rule: RuleSet { conditions: vec![RuleCondition::HasAttachment], match_mode: MatchMode::All } });
    let f = val(&core, "reply/filter");
    assert_eq!(f["entry"]["rule"]["match"], "any");
    assert_eq!(f["entry"]["rule"]["conditions"], serde_json::json!([{"kind": "from", "words": "boss@example.test"}]));
    assert_eq!(f["exit"]["rule"]["match"], "all");
    assert_eq!(f["exit"]["rule"]["conditions"], serde_json::json!([{"kind": "has_attachment"}]));
    assert_eq!(f["entry"]["rule"]["rows"], serde_json::json!([{"kind": "from", "words": "boss@example.test"}]), "the text rows ride with the rule");
    run(&core, Command::SetReplyFilterAiReads { reads: AiReads { subject: true, body: false, rest_of_thread: false, attachments: true } });
    let ai = val(&core, "reply/filter")["ai"].clone();
    assert_eq!((ai["subject"].as_bool(), ai["body"].as_bool(), ai["rest_of_thread"].as_bool(), ai["attachments"].as_bool()), (Some(true), Some(false), Some(false), Some(true)));
    run(&core, Command::SetReplyFilterAiDescription { description: "  Waits for me  ".into() });
    assert_eq!(val(&core, "reply/filter")["ai"]["description"], "Waits for me");
}
// mutant E2: SetReplyFilterExitRule's arm writes FlowCard::Entry (the entry rule is overwritten by the exit rule).

#[test]
fn the_two_on_demand_switches() {
    let (core, _, _d) = seeded();
    run(&core, Command::SetOnDemandOption { option: OnDemandOption::InstructionBefore, enabled: false });
    assert_eq!(val(&core, "reply/on_demand"), serde_json::json!({"instruction_before": false, "change_after": true}));
    run(&core, Command::SetOnDemandOption { option: OnDemandOption::ChangeAfter, enabled: false });
    run(&core, Command::SetOnDemandOption { option: OnDemandOption::InstructionBefore, enabled: true });
    assert_eq!(val(&core, "reply/on_demand"), serde_json::json!({"instruction_before": true, "change_after": false}));
}
// mutant E3: SetOnDemandOption writes `!enabled`.

#[test]
fn retrieval_settings_change_one_at_a_time_and_the_scope_shows_on_the_sources_pane_too() {
    let (core, _, _d) = seeded();
    run(&core, Command::SetRetrievalSetting { change: RetrievalChange::KeepN { n: 9 } });
    run(&core, Command::SetRetrievalSetting { change: RetrievalChange::ExcerptSize { size: ExcerptSize::Section } });
    run(&core, Command::SetRetrievalSetting { change: RetrievalChange::Clash { rule: ClashRule::KeepOldest } });
    run(&core, Command::SetRetrievalSetting { change: RetrievalChange::AiFilterQuestion { question: None } });
    run(&core, Command::SetRetrievalSetting { change: RetrievalChange::AttachedFields { fields: ExcerptFields { date: false, sender: true, folder: false, subject: true } } });
    run(&core, Command::SetRetrievalSetting { change: RetrievalChange::Scope { scope: RetrievalScope::AllAccounts } });
    let r = val(&core, "reply/retrieval");
    assert_eq!((r["keep_n"].as_u64(), r["excerpt_size"].as_str(), r["clash_rule"].as_str(), r["scope"].as_str()), (Some(9), Some("section"), Some("keep_oldest"), Some("all_accounts")));
    assert!(r["ai_filter_question"].is_null(), "no AI filter is an explicit null");
    assert_eq!(r["attached_fields"], serde_json::json!({"date": false, "sender": true, "folder": false, "subject": true}));
    assert_eq!(r["questions_max"], 3, "the settings not changed kept their value");
    assert_eq!(val(&core, "reply/sources")["scope"], "all_accounts", "one truth: the sources pane mirrors the retrieval scope");
    assert_eq!(refused(&core, Command::SetRetrievalSetting { change: RetrievalChange::KeepN { n: 0 } }), REPLY_BAD_SETTING_CODE);
}
// mutant E4: SetRetrievalSetting answers Ok and writes nothing.

#[test]
fn sources_mail_folders_this_mac_and_calendars_with_the_derived_figures() {
    let (core, account, _d) = seeded();
    let inbox = ReplySourceRef::MailFolder { account_id: account.clone(), folder: "INBOX".into() };
    let sent = ReplySourceRef::MailFolder { account_id: account.clone(), folder: "Sent".into() };
    let cal = ReplySourceRef::Calendar { calendar_id: "cal-1".into() };
    for s in [&inbox, &sent, &cal] { core.store().reply_source_ensure(s.clone(), 1).unwrap(); }
    core.store().reply_source_progress(inbox.clone(), 18_000, ReplySyncProgress::UpToDate, 2).unwrap();
    core.store().reply_source_progress(sent.clone(), 96, ReplySyncProgress::Syncing { percent: 62 }, 2).unwrap();
    assert_eq!(val(&core, "reply/sources")["enabled_total"], 0, "nothing is on yet");
    run(&core, Command::SetReplySourceEnabled { source: inbox.clone(), enabled: true });
    run(&core, Command::SetReplySourceEnabled { source: cal.clone(), enabled: true });
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().to_str().unwrap().to_string();
    let added = run(&core, Command::AddReplyLocalFolder { path: format!("{path}/") }).created_id.expect("the source key");
    assert_eq!(added, reply_source_key(ReplySourceRef::LocalFolder { path: path.clone() }), "the key of the folder, its trailing slash dropped");
    run(&core, Command::AddReplyLocalFolder { path: path.clone() });
    let s = val(&core, "reply/sources");
    assert_eq!(s["enabled_total"], 3, "one mail folder, one folder of this Mac, one calendar");
    assert_eq!(s["local_folders"].as_array().unwrap().len(), 1, "the same folder twice is one source");
    assert_eq!(s["calendars"][0]["calendar_id"], "cal-1");
    assert_eq!(s["calendars"][0]["enabled"], true);
    let card = &s["accounts"][0];
    assert_eq!((card["account_id"].as_str(), card["total_folders"].as_u64(), card["enabled_folders"].as_u64(), card["enabled_items"].as_u64()), (Some(account.as_str()), Some(2), Some(1), Some(18_000)), "the header's figures come from the tiles");
    let tile = card["folders"].as_array().unwrap().iter().find(|t| t["source"]["folder"] == "Sent").unwrap();
    assert_eq!(tile["progress"], serde_json::json!({"state": "not_started"}), "a switched-off folder is not started");
    let on = card["folders"].as_array().unwrap().iter().find(|t| t["source"]["folder"] == "INBOX").unwrap();
    assert_eq!((on["item_count"].as_u64(), &on["progress"]), (Some(18_000), &serde_json::json!({"state": "syncing", "percent": 0})), "the count is the stored one; the progress is derived (nothing stored, first sync not ended: syncing)");
    run(&core, Command::RemoveReplyLocalFolder { path: path.clone() });
    assert_eq!(val(&core, "reply/sources")["local_folders"].as_array().unwrap().len(), 0);
    assert_eq!(refused(&core, Command::RemoveReplyLocalFolder { path }), "not_found:folder");
    assert_eq!(refused(&core, Command::SetReplySourceEnabled { source: ReplySourceRef::Calendar { calendar_id: "ghost".into() }, enabled: true }), "not_found:calendar");
}
// mutant E14: sources_json drops the calendars (the pane's calendars and its enabled_total lose them).

#[test]
fn a_folder_of_this_mac_must_exist_on_this_disk() {
    let (core, _, _d) = seeded();
    assert_eq!(refused(&core, Command::AddReplyLocalFolder { path: "/definitely/not/a/folder-xyz".into() }), REPLY_BAD_FOLDER_CODE);
    assert_eq!(refused(&core, Command::AddReplyLocalFolder { path: "relative/folder".into() }), REPLY_BAD_FOLDER_CODE);
    assert_eq!(val(&core, "reply/sources")["local_folders"].as_array().unwrap().len(), 0, "nothing was added");
}
// mutant E5: AddReplyLocalFolder skips `folder_on_disk` (the first refusal vanishes).

#[test]
fn the_voice_the_examples_and_the_other_tones() {
    let (core, _, _d) = seeded();
    run(&core, Command::SetVoiceText { text: "Short and warm.".into() });
    run(&core, Command::SetVoiceSamePersonExamples { count: 5 });
    let formal = run(&core, Command::SaveReplyTone { tone: ReplyTone { id: String::new(), name: "Formel".into(), text: "Vouvoyer.".into() } }).created_id.unwrap();
    run(&core, Command::SaveReplyTone { tone: ReplyTone { id: String::new(), name: "Bref".into(), text: "Trois lignes.".into() } });
    let v = val(&core, "reply/voice");
    assert_eq!((v["voice"]["tone_text"].as_str(), v["voice"]["same_person_examples"].as_u64()), (Some("Short and warm."), Some(5)));
    assert_eq!(v["tones"].as_array().unwrap().iter().map(|t| t["name"].as_str().unwrap()).collect::<Vec<_>>(), ["Formel", "Bref"]);
    assert_eq!(v["analysis"], serde_json::json!({"state": "idle"}));
    run(&core, Command::SaveReplyTone { tone: ReplyTone { id: formal.clone(), name: "Formel".into(), text: "Toujours vouvoyer.".into() } });
    assert_eq!(val(&core, "reply/voice")["tones"][0]["text"], "Toujours vouvoyer.");
    run(&core, Command::SetReplyUseTone { kind: ReplyUseKind::Auto, tone_id: formal.clone() });
    run(&core, Command::DeleteReplyTone { tone_id: formal.clone() });
    assert_eq!(val(&core, "reply/voice")["tones"].as_array().unwrap().len(), 1);
    assert_eq!(val(&core, "reply/use/auto")["tone_id"], "voice", "a removed tone sends the use back to the voice");
    assert_eq!(refused(&core, Command::DeleteReplyTone { tone_id: formal }), "not_found:tone");
    assert_eq!(refused(&core, Command::SetVoiceSamePersonExamples { count: REPLY_EXAMPLES_CEILING + 1 }), REPLY_BAD_SAMPLE_CODE);
    assert_eq!(refused(&core, Command::SaveReplyTone { tone: ReplyTone { id: String::new(), name: "bref".into(), text: "x".into() } }), REPLY_BAD_TONE_CODE, "a name another tone has");
}
// mutant E7: DeleteReplyTone answers Ok and deletes nothing.

#[test]
fn the_prompt_saves_versions_restores_by_a_new_version_and_previews_what_the_model_receives() {
    let (core, account, _d) = seeded();
    core.store().insert_messages(vec![mail(&account, "m1", "t1", 1_000), mail(&account, "m2", "t2", 9_000)]).unwrap();
    let saved = run(&core, Command::SaveReplyPromptVersion { prompt_id: "replies".into(), text: "Answer {{message}} in {{tone}}.\n// a note the model never reads\nKeep {{notes}}.".into() });
    assert_eq!(saved.created_id.as_deref(), Some("2"), "the version number comes back");
    let cur = val(&core, "reply/prompt/current");
    assert_eq!((cur["version"].as_u64(), cur["current"].as_bool()), (Some(2), Some(true)));
    let versions = val(&core, "reply/prompt/versions");
    assert_eq!(versions.as_array().unwrap().iter().map(|v| v["version"].as_u64().unwrap()).collect::<Vec<_>>(), [2, 1], "newest first");
    assert_eq!(versions[1]["current"], false);
    let restored = run(&core, Command::RestoreReplyPromptVersion { prompt_id: "replies".into(), version: 1 });
    assert_eq!(restored.created_id.as_deref(), Some("3"), "a restore is a new version");
    assert_eq!(val(&core, "reply/prompt/current")["text"], default_reply_prompt_text());
    run(&core, Command::SaveReplyPromptVersion { prompt_id: "replies".into(), text: "Answer {{message}} in {{tone}}.\n// a note the model never reads\nKeep {{notes}}.".into() });
    run(&core, Command::SetVoiceText { text: "plain and brief".into() });
    run(&core, Command::SaveFixedNote { note: FixedNote { id: String::new(), title: "Prices".into(), text: "Never quote.".into() } });
    let preview: String = serde_json::from_str(&st(&core, "reply/prompt/preview")).expect("the preview is one JSON string");
    assert_eq!(preview, "Answer De Sender m2 : Subject m2\nPreview m2 in plain and brief.\nKeep Prices : Never quote..", "the newest message, the tone text, the notes; the note line is gone, no placeholder is left");
    assert!(!preview.contains("{{") && !preview.contains("//"));
    assert_eq!(refused(&core, Command::SaveReplyPromptVersion { prompt_id: "replies".into(), text: "no message placeholder".into() }), REPLY_BAD_PROMPT_CODE);
    assert_eq!(refused(&core, Command::SaveReplyPromptVersion { prompt_id: "ghost".into(), text: "{{message}}".into() }), "not_found:prompt");
    assert_eq!(refused(&core, Command::RestoreReplyPromptVersion { prompt_id: "replies".into(), version: 99 }), "not_found:prompt version");
}
// mutant E8: `preview` returns the stored text unrendered (the placeholders and the note line stay).

#[test]
fn the_fixed_notes() {
    let (core, _, _d) = seeded();
    let a = run(&core, Command::SaveFixedNote { note: FixedNote { id: String::new(), title: "Prices".into(), text: "Never quote.".into() } }).created_id.expect("a note id");
    run(&core, Command::SaveFixedNote { note: FixedNote { id: String::new(), title: "Sign-off".into(), text: "Best, Assem".into() } });
    run(&core, Command::SaveFixedNote { note: FixedNote { id: a.clone(), title: "Prices".into(), text: "Never quote a price.".into() } });
    let n = val(&core, "reply/notes");
    assert_eq!(n.as_array().unwrap().len(), 2);
    assert_eq!((n[0]["id"].as_str(), n[0]["text"].as_str()), (Some(a.as_str()), Some("Never quote a price.")));
    run(&core, Command::DeleteFixedNote { note_id: a.clone() });
    assert_eq!(val(&core, "reply/notes").as_array().unwrap().len(), 1);
    assert_eq!(refused(&core, Command::DeleteFixedNote { note_id: a }), "not_found:note");
    assert_eq!(refused(&core, Command::SaveFixedNote { note: FixedNote { id: String::new(), title: " ".into(), text: "x".into() } }), REPLY_BAD_NOTE_CODE);
}
// mutant E15: SaveFixedNote answers Ok and stores nothing.

#[test]
fn the_counts_are_derived_from_the_passes_of_the_last_seven_days_and_a_message_counts_once() {
    let (core, account, _d) = seeded();
    let t = now();
    let hour = 3_600_000;
    core.store().insert_messages((1..=6).map(|i| mail(&account, &format!("m{i}"), &format!("t{i}"), if i == 6 { t - 8 * 24 * hour } else { t - i * hour })).collect()).unwrap();
    live_draft(&core, "d1", "t1", "m1", "proposed");
    let pass = |id: &str, entry, ai, exit, draft: Option<&str>| {
        let at = core.store().message(id).unwrap().unwrap().received_at_ms;
        core.store().reply_record_pass(ReplyPass { message_id: id.into(), received_at_ms: at, entry, ai, exit, draft_id: draft.map(String::from) }).unwrap();
    };
    use ReplyStepResult::*;
    pass("m1", Passed, Passed, Passed, Some("d1"));
    pass("m2", Passed, Passed, Stopped, None);
    pass("m3", Passed, Stopped, NotReached, None);
    pass("m4", Stopped, NotReached, NotReached, None);
    pass("m5", Skipped, Skipped, Passed, None);
    pass("m6", Passed, Passed, Passed, None);
    let c = val(&core, "reply/counts");
    assert_eq!((c["days"].as_u64(), c["received"].as_u64(), c["after_entry"].as_u64(), c["after_ai"].as_u64(), c["after_exit"].as_u64(), c["drafts"].as_u64()),
        (Some(7), Some(5), Some(4), Some(3), Some(2), Some(1)), "the 8-day-old message is out; each figure is a subset of the one before");
    pass("m4", Passed, Passed, Passed, None);
    let c = val(&core, "reply/counts");
    assert_eq!((c["received"].as_u64(), c["after_entry"].as_u64(), c["after_exit"].as_u64()), (Some(5), Some(5), Some(3)), "m4 was looked at again: its row was replaced, so it still counts once");
}
// mutant E9: reply/counts reads the passes since `now` (a zero-length window: every figure is 0).

// ---------------------------------------------------------------- the refusals: every code of the contract reached once

#[test]
fn every_refusal_code_of_the_contract_is_reached_once() {
    let (core, account, _d) = seeded();
    labels(&core);
    core.store().insert_messages(vec![mail(&account, "m1", "t1", 1_000)]).unwrap();
    live_draft(&core, "closed", "t1", "m1", "discarded");
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut hit = |c: Command, want: &str| { let got = refused(&core, c); assert_eq!(got, want); seen.insert(got); };
    hit(Command::SetReplyFilterScope { scope: ReplyLabelScope::Labels { label_ids: vec![] } }, REPLY_SCOPE_EMPTY_CODE);
    hit(Command::SetReplyFilterEntryRule { rule: rule(" ") }, REPLY_BAD_RULE_CODE);
    hit(Command::SetReplyFilterAiReads { reads: AiReads { subject: false, body: false, rest_of_thread: false, attachments: false } }, REPLY_READS_NOTHING_CODE);
    hit(Command::SetReplyFilterAiDescription { description: "".into() }, REPLY_AI_NEEDS_DESCRIPTION_CODE);
    hit(Command::SaveReplyTone { tone: ReplyTone { id: String::new(), name: "".into(), text: "x".into() } }, REPLY_BAD_TONE_CODE);
    hit(Command::SaveReplyPromptVersion { prompt_id: "replies".into(), text: "".into() }, REPLY_BAD_PROMPT_CODE);
    hit(Command::SaveFixedNote { note: FixedNote { id: String::new(), title: "".into(), text: "x".into() } }, REPLY_BAD_NOTE_CODE);
    hit(Command::SetRetrievalSetting { change: RetrievalChange::QuestionsMax { n: REPLY_QUESTIONS_CEILING + 1 } }, REPLY_BAD_SETTING_CODE);
    hit(Command::AddReplyLocalFolder { path: "../up".into() }, REPLY_BAD_FOLDER_CODE);
    hit(Command::RunVoiceAnalysis { sample_size: 0 }, REPLY_BAD_SAMPLE_CODE);
    hit(Command::AcceptReplyDraft { draft_id: "closed".into() }, REPLY_DRAFT_CLOSED_CODE);
    run(&core, Command::SetOnDemandOption { option: OnDemandOption::InstructionBefore, enabled: false });
    // the two switches are off now: the instruction and the change request are refused before any work runs
    hit(Command::DraftReplyOnDemand { thread_id: "t1".into(), instruction: Some("be brief".into()) }, REPLY_INSTRUCTION_OFF_CODE);
    live_draft(&core, "open", "t1", "m1", "proposed");
    run(&core, Command::SetOnDemandOption { option: OnDemandOption::ChangeAfter, enabled: false });
    hit(Command::RequestReplyChange { draft_id: "open".into(), text: "shorter".into() }, REPLY_CHANGE_OFF_CODE);
    run(&core, Command::SetOnDemandOption { option: OnDemandOption::ChangeAfter, enabled: true });
    hit(Command::RequestReplyChange { draft_id: "open".into(), text: "   ".into() }, REPLY_BAD_TEXT_CODE);
    let all: BTreeSet<String> = [REPLY_SCOPE_EMPTY_CODE, REPLY_BAD_RULE_CODE, REPLY_READS_NOTHING_CODE, REPLY_AI_NEEDS_DESCRIPTION_CODE, REPLY_BAD_TONE_CODE, REPLY_BAD_PROMPT_CODE,
        REPLY_BAD_NOTE_CODE, REPLY_BAD_SETTING_CODE, REPLY_BAD_FOLDER_CODE, REPLY_BAD_SAMPLE_CODE, REPLY_DRAFT_CLOSED_CODE, REPLY_INSTRUCTION_OFF_CODE, REPLY_CHANGE_OFF_CODE, REPLY_BAD_TEXT_CODE]
        .iter().map(|c| c.to_string()).collect();
    assert_eq!(seen, all, "every refusal code of reply_ai.rs was reached through the real core");
}
// mutant E6: RunVoiceAnalysis skips check_voice_sample_size (the bad_voice_sample refusal turns into the not-built-yet one).
// mutant E11: DraftReplyOnDemand skips check_on_demand_instruction (reply_instruction_off is never reached).

// ---------------------------------------------------------------- the one port

/// A work that records the jobs it was handed and answers canned reads.
#[derive(Default)]
struct Recording { jobs: Mutex<Vec<ReplyJob>> }
impl ReplyWork for Recording {
    fn execute(&self, _store: &Store, job: ReplyJob) -> Result<CommandOutcome, CoreError> {
        self.jobs.lock().unwrap().push(job);
        Ok(CommandOutcome { undo_entry_id: None, created_id: Some("run-1".into()) })
    }
    fn voice_analysis(&self) -> ReplyVoiceAnalysis { ReplyVoiceAnalysis::Running { read: 12, of: 100 } }
    fn try_report(&self, run_id: &str) -> Option<ReplyTryReport> {
        (run_id == "run-1").then(|| ReplyTryReport {
            run_id: "run-1".into(), message_id: "m1".into(), kind: ReplyUseKind::Auto,
            retrieval: ReplyRetrievalReport { questions: vec!["q?".into()], found: 3, kept: vec![], dropped_by_ai_filter: 1 },
            entry: ReplyStepReport { result: ReplyStepResult::Passed, matched: vec![], model_note: None },
            ai: ReplyStepReport { result: ReplyStepResult::Passed, matched: vec![], model_note: Some("a question".into()) },
            exit: ReplyStepReport { result: ReplyStepResult::Skipped, matched: vec![], model_note: None }, draft_text: Some("Hello".into()),
        })
    }
}

#[test]
fn the_working_commands_refuse_in_words_while_no_model_is_set() {
    // reply-engine installs the draft engine at core start: with no model set (no Writer in Settings, AI and agents) the four commands that need one
    // refuse with `ai_no_model` (ai_settings::model_for); Accept and Discard need no model (engine_reply_work.rs covers them)
    let (core, account, _d) = seeded();
    core.store().insert_messages(vec![mail(&account, "m1", "t1", 1_000)]).unwrap();
    live_draft(&core, "d-open", "t1", "m1", "proposed");
    for c in [
        Command::TryReply { message_id: "m1".into(), kind: ReplyUseKind::Auto },
        Command::RunVoiceAnalysis { sample_size: 50 },
        Command::DraftReplyOnDemand { thread_id: "t1".into(), instruction: None },
        Command::RequestReplyChange { draft_id: "d-open".into(), text: "shorter".into() },
    ] {
        assert_eq!(refused(&core, c.clone()), contracts::AI_NO_MODEL_CODE, "{c:?}");
    }
    assert_eq!(refused(&core, Command::TryReply { message_id: "ghost".into(), kind: ReplyUseKind::Auto }), "not_found:message");
    assert_eq!(refused(&core, Command::DiscardReplyDraft { draft_id: "ghost".into() }), "not_found:reply draft");
    assert_eq!(val(&core, "reply/draft/t1")["id"], "d-open", "nothing the refused commands tried changed the live draft");
    assert_eq!(val(&core, "reply/draft/t1")["state"], "proposed");
}

#[test]
fn a_plugged_work_receives_jobs_the_contracts_rules_already_checked_and_serves_its_own_reads() {
    let (core, account, _d) = seeded();
    core.store().insert_messages(vec![mail(&account, "m1", "t1", 1_000)]).unwrap();
    live_draft(&core, "d-open", "t1", "m1", "edited");
    let rec = Arc::new(Recording::default());
    install_reply_work(core.store().path(), rec.clone());
    let out = run(&core, Command::TryReply { message_id: "m1".into(), kind: ReplyUseKind::Auto });
    assert_eq!(out.created_id.as_deref(), Some("run-1"));
    run(&core, Command::RunVoiceAnalysis { sample_size: 50 });
    run(&core, Command::DraftReplyOnDemand { thread_id: "t1".into(), instruction: Some("  be brief  ".into()) });
    run(&core, Command::DraftReplyOnDemand { thread_id: "t1".into(), instruction: Some("   ".into()) });
    run(&core, Command::RequestReplyChange { draft_id: "d-open".into(), text: "  shorter  ".into() });
    run(&core, Command::DiscardReplyDraft { draft_id: "d-open".into() });
    let jobs = rec.jobs.lock().unwrap().clone();
    assert_eq!(jobs[0], ReplyJob::TryReply { message_id: "m1".into(), kind: ReplyUseKind::Auto });
    assert_eq!(jobs[1], ReplyJob::RunVoiceAnalysis { sample_size: 50 });
    assert_eq!(jobs[2], ReplyJob::DraftOnDemand { thread_id: "t1".into(), instruction: Some("be brief".into()) }, "the instruction arrives trimmed");
    assert_eq!(jobs[3], ReplyJob::DraftOnDemand { thread_id: "t1".into(), instruction: None }, "a blank instruction is none");
    match &jobs[4] { ReplyJob::RequestChange { draft, text } => assert_eq!((draft.id.as_str(), text.as_str()), ("d-open", "shorter")), other => panic!("{other:?}") }
    assert!(matches!(&jobs[5], ReplyJob::Discard { draft } if draft.id == "d-open"));
    assert_eq!(jobs.len(), 6);
    // an already-taken draft is refused before the work: Accept on an Edited draft never reaches it
    assert_eq!(refused(&core, Command::AcceptReplyDraft { draft_id: "d-open".into() }), REPLY_DRAFT_CLOSED_CODE);
    assert_eq!(rec.jobs.lock().unwrap().len(), 6, "the refused Accept reached no work");
    assert_eq!(val(&core, "reply/voice")["analysis"], serde_json::json!({"state": "running", "read": 12, "of": 100}), "the analysis state is the work's");
    let t = val(&core, "reply/try/run-1");
    assert_eq!((t["run_id"].as_str(), t["draft_text"].as_str(), t["ai"]["model_note"].as_str()), (Some("run-1"), Some("Hello"), Some("a question")));
    assert_eq!(core.state("reply/try/run-2".into()), None, "an unknown run has no report");
}
// mutant E10: `work_for` ignores the installed work (the plugged work is never reached).

// ---------------------------------------------------------------- the real core and the fixture core spell the same JSON

fn keys(v: &serde_json::Value, at: &str, out: &mut BTreeSet<String>) {
    match v {
        serde_json::Value::Object(m) => for (k, x) in m { let p = format!("{at}.{k}"); out.insert(p.clone()); keys(x, &p, out); },
        serde_json::Value::Array(a) => for x in a { keys(x, &format!("{at}[]"), out); },
        _ => {}
    }
}
fn key_set(json: &str) -> BTreeSet<String> { let mut o = BTreeSet::new(); keys(&serde_json::from_str(json).unwrap(), "", &mut o); o }

/// The fixture core (Swift) builds every state value from the contract's encoders; the contract's encoders are what a populated real core must
/// answer too: the same keys at every depth, and the same text.
#[test]
fn a_populated_real_core_spells_the_same_keys_as_the_contracts_encoders_over_the_same_values() {
    let (core, account, _d) = seeded();
    labels(&core);
    core.store().insert_messages(vec![mail(&account, "m1", "t1", now() - 3_600_000)]).unwrap();
    let inbox = ReplySourceRef::MailFolder { account_id: account.clone(), folder: "INBOX".into() };
    let cal = ReplySourceRef::Calendar { calendar_id: "cal-1".into() };
    core.store().reply_source_ensure(inbox.clone(), 1).unwrap();
    core.store().reply_source_ensure(cal.clone(), 1).unwrap();
    core.store().reply_source_progress(inbox.clone(), 12, ReplySyncProgress::UpToDate, 2).unwrap();
    run(&core, Command::SetReplySourceEnabled { source: inbox.clone(), enabled: true });
    run(&core, Command::SetReplySourceEnabled { source: cal.clone(), enabled: true });
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().to_str().unwrap().to_string();
    run(&core, Command::AddReplyLocalFolder { path: path.clone() });
    core.store().reply_source_progress(ReplySourceRef::LocalFolder { path: path.clone() }, 7, ReplySyncProgress::Failed { reason_words: "unreadable".into() }, 3).unwrap();
    run(&core, Command::SetReplyFilterScope { scope: ReplyLabelScope::Labels { label_ids: vec!["l1".into()] } });
    run(&core, Command::SetReplyFilterEntryRule { rule: RuleSet { conditions: vec![RuleCondition::From { words: "boss".into() }, RuleCondition::Fact { fact: MessageFact::Unanswered, wanted: true }], match_mode: MatchMode::All } });
    run(&core, Command::SetReplyFilterExitRule { rule: rule("vip") });
    let tone = ReplyTone { id: String::new(), name: "Formel".into(), text: "Vouvoyer.".into() };
    let tone_id = run(&core, Command::SaveReplyTone { tone: tone.clone() }).created_id.unwrap();
    let note_id = run(&core, Command::SaveFixedNote { note: FixedNote { id: String::new(), title: "Prices".into(), text: "Never quote.".into() } }).created_id.unwrap();
    run(&core, Command::SaveReplyPromptVersion { prompt_id: "replies".into(), text: "Reply to {{message}}".into() });
    live_draft(&core, "d1", "t1", "m1", "proposed");

    let expect: Vec<(&str, String)> = vec![
        ("reply/filter", reply_filter_json(ReplyFilter { scope: ReplyLabelScope::Labels { label_ids: vec!["l1".into()] }, entry_enabled: true,
            entry_rule: RuleSet { conditions: vec![RuleCondition::From { words: "boss".into() }, RuleCondition::Fact { fact: MessageFact::Unanswered, wanted: true }], match_mode: MatchMode::All },
            ai_enabled: true, ai_reads: default_reply_ai_reads(), ai_description: default_reply_ai_description(), exit_enabled: true, exit_rule: rule("vip") })),
        ("reply/sources", {
            let tile = |s: &ReplySourceRef, enabled, n, p| ReplySource { source: s.clone(), enabled, item_count: n, progress: p };
            let inbox_tile = tile(&inbox, true, 12, ReplySyncProgress::Syncing { percent: 0 }); // the stored count; the progress is derived: one stored message, none indexed
            reply_sources_json(reply_sources_read(
                vec![reply_account_sources(account.clone(), vec![inbox_tile])],
                vec![tile(&ReplySourceRef::LocalFolder { path: path.clone() }, true, 7, ReplySyncProgress::Failed { reason_words: "unreadable".into() })],
                vec![ReplyCalendar { calendar_id: "cal-1".into(), name: "cal-1".into(), account_label: String::new(), colour: String::new(), enabled: true }],
                RetrievalScope::ThisAccount))
        }),
        ("reply/voice", reply_voice_json(default_reply_voice(), ReplyVoiceAnalysis::Idle, vec![ReplyTone { id: tone_id, ..tone }])),
        ("reply/notes", fixed_notes_json(vec![FixedNote { id: note_id, title: "Prices".into(), text: "Never quote.".into() }])),
        ("reply/retrieval", retrieval_json(default_retrieval_settings())),
        ("reply/counts", reply_counts_json(ReplyStepCounts { days: 7, received: 0, after_entry: 0, after_ai: 0, after_exit: 0, drafts: 0 })),
    ];
    for (path, want) in &expect {
        let got = st(&core, path);
        assert_eq!(key_set(&got), key_set(want), "{path}: the keys differ from the contract's encoder");
        assert_eq!(&got, want, "{path}: the text differs from the contract's encoder");
    }
    // the live draft: the keys of reply_draft_json, the text of the row
    let d = val(&core, "reply/draft/t1");
    let encoded = reply_draft_json(ReplyDraft { id: "d1".into(), thread_id: "t1".into(), message_id: "m1".into(), text: "Hello".into(), made_by: ReplyMadeBy::OnDemand, excerpt_ids: vec![], instruction: None,
        state: ReplyDraftState::Proposed, revision: 0, created_at_ms: 5, updated_at_ms: 6 });
    assert_eq!(st(&core, "reply/draft/t1"), encoded);
    assert_eq!(d["state"], "proposed");
    assert_eq!(core.state("reply/draft/t-other".into()), None, "a thread with no live draft has no answer");
}

// ---------------------------------------------------------------- delta 21: the shell's calendars and the crawl's progress

fn cal(id: &str, name: &str) -> ReplyCalendarInfo { ReplyCalendarInfo { calendar_id: id.into(), name: name.into(), account_label: "iCloud".into(), colour: "#3366FF".into() } }
fn ev(cal: &str, id: &str, start: i64, end: i64) -> CalendarEvent { CalendarEvent { event_id: id.into(), calendar_id: cal.into(), title: format!("Event {id}"), start_ms: start, end_ms: end, attendees: vec!["a@x.test".into()], location: None } }

#[test]
fn the_shell_pushes_calendars_reads_which_are_enabled_and_the_sources_pane_serves_their_real_names() {
    let (core, _, _d) = seeded();
    assert_eq!(st(&core, "reply/calendars/enabled"), "[]");
    assert_eq!(st(&core, "reply/calendars/events"), "[]");
    run(&core, Command::SyncReplyCalendars { calendars: vec![cal("a", "Work"), cal("b", "Home")], window_start_ms: 0, window_end_ms: 10_000, events: vec![ev("a", "e1", 100, 200)] });
    assert_eq!(st(&core, "reply/calendars/enabled"), "[]", "a new calendar starts off");
    assert_eq!(st(&core, "reply/calendars/events"), "[]", "and its events are not kept");
    run(&core, Command::SetReplySourceEnabled { source: ReplySourceRef::Calendar { calendar_id: "b".into() }, enabled: true });
    assert_eq!(st(&core, "reply/calendars/enabled"), enabled_calendar_ids_json(vec![ReplyCalendar { calendar_id: "b".into(), name: "Home".into(), account_label: "iCloud".into(), colour: "#3366FF".into(), enabled: true }]));
    run(&core, Command::SyncReplyCalendars { calendars: vec![cal("a", "Work"), cal("b", "Home")], window_start_ms: 0, window_end_ms: 10_000, events: vec![ev("a", "e1", 100, 200), ev("b", "e2", 300, 400)] });
    assert_eq!(st(&core, "reply/calendars/events"), calendar_events_json(vec![ev("b", "e2", 300, 400)]), "only the enabled calendar's event is kept");
    let pane = val(&core, "reply/sources");
    assert_eq!((pane["calendars"][0]["name"].as_str(), pane["calendars"][0]["colour"].as_str(), pane["calendars"][0]["account_label"].as_str(), pane["calendars"][1]["enabled"].as_bool()),
        (Some("Work"), Some("#3366FF"), Some("iCloud"), Some(true)), "the pane serves the real name, colour and label, no longer the id");
    assert_eq!(pane["enabled_total"], 1);
    assert_eq!(refused(&core, Command::SyncReplyCalendars { calendars: vec![ReplyCalendarInfo { colour: "red".into(), ..cal("c", "Bad") }], window_start_ms: 0, window_end_ms: 10_000, events: vec![] }), CALENDAR_SYNC_REFUSAL_CODE);
}
// mutant E16: sources_json serves no calendars.
// mutant E17: reply/calendars/enabled lists every calendar as enabled.
// mutant E19: SyncReplyCalendars answers Ok and stores nothing.
// mutant E20: reply/calendars/events answers an empty list.

#[test]
fn reply_progress_serves_the_crawls_figures_per_folder_in_the_contracts_spelling() {
    let (core, account, _d) = seeded();
    assert_eq!(st(&core, "reply/progress"), "[]", "no message, no folder");
    let t = now();
    let mut with_body = mail(&account, "m1", "t1", t - 1_000);
    with_body.body_text = Some("body".into());
    core.store().insert_messages(vec![with_body, mail(&account, "m2", "t2", t - 2_000)]).unwrap();
    // m2 has only its preview: the mail() helper gives it a body text too, so clear it
    sql(&core, "UPDATE message SET body_text = NULL WHERE id = 'm2'");
    sql(&core, "INSERT INTO chunk (message_id, part, chunk_no, received_at_ms, sender, folder, subject, text, embedded_at_ms) VALUES ('m1', 'body', 0, 1, 's', 'INBOX', 'subj', 'text', 5)");
    assert_eq!(st(&core, "reply/progress"), folder_progress_json(vec![FolderProgress { account_id: account.clone(), folder: "INBOX".into(), messages: 2, bodies: 1, indexed: 1 }]));
    let p = val(&core, "reply/progress");
    assert_eq!(p[0]["progress"], serde_json::json!({"state": "syncing", "percent": 50}), "half of the messages are indexed");
}
// mutant E18: reply/progress answers an empty list.

// ---------------------------------------------------------------- round 4: the defaults of a NEW installation, on fake IMAP accounts
// "Auto-draft is off until you turn it on. Sources start with Envoyés and Boîte de réception of each account." (the accepted plan)
//
// Mutants (single edits; each was run and seen red, `_captures/replies-settings/mutants-run-r4.log`):
// - D1 the migration 0013 seed: the auto use starts on                       D2 the seed: the on-demand use starts off
// - D3 replies.rs `register_folders`: the Sent role is not on by default     D4 the same: every folder is on by default
// - D5 imap_source.rs: the registration line is removed                      D6 store replies.rs `reply_register_folders`: an existing row is reset
// - D7 replies.rs `register_folders`: only the first account is registered   D8 store replies.rs: a new calendar starts on (the shell's sync)

const PW: &str = "app-password";

fn mbox(name: &str, special: Option<&'static str>) -> Mbox {
    Mbox { name: name.into(), special, uid_validity: 1000, next_uid: 1, highest_modseq: 1, msgs: vec![], expunged: vec![], version: 0 }
}
/// A fake IMAP server whose folders are INBOX, its Sent folder under the SPECIAL-USE flag (`sent` is not an English name), and two others.
fn server(rt: &tokio::runtime::Runtime, user: &str, sent: &str) -> (FakeImap, FakeSmtp) { server_with(rt, user, sent, true) }
/// `special_use` false = a server that sends no SPECIAL-USE flag at all (the names alone say which folder is Sent).
fn server_with(rt: &tokio::runtime::Runtime, user: &str, sent: &str, special_use: bool) -> (FakeImap, FakeSmtp) {
    let imap = rt.block_on(FakeImap::start_with(special_use));
    imap.with(|s| s.boxes = vec![mbox("INBOX", None), mbox(sent, Some("\\Sent")), mbox("Projects", None), mbox("Receipts", None)]);
    (imap, rt.block_on(FakeSmtp::start(user, PW)))
}
fn add_account(core: &RustCore, user: &str, imap: &FakeImap, smtp: &FakeSmtp) -> String {
    let config = AccountConfig {
        imap_host: "127.0.0.1".into(), imap_port: imap.port, imap_security: ConnectionSecurity::Ssl, smtp_host: "127.0.0.1".into(), smtp_port: smtp.port,
        smtp_security: ConnectionSecurity::Starttls, username: user.into(), smtp_username: user.into(), auth_method: AuthMethod::Password, source: DetectionSource::Manual,
    };
    core.run(Command::AddAccountConfigured { address: user.into(), display_name: Some(user.into()), config, password: SecretText { value: PW.into() } }).expect("added").created_id.unwrap()
}
fn wait_until(what: &str, mut ok: impl FnMut() -> bool) {
    let t0 = std::time::Instant::now();
    while t0.elapsed() < std::time::Duration::from_secs(20) {
        if ok() { return; }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    panic!("never reached within 20 s: {what}");
}
/// The folders of one account card of the Sources pane, folder name to switch.
fn folders_of(core: &RustCore, account: &str) -> BTreeMap<String, bool> {
    let pane = val(core, "reply/sources");
    pane["accounts"].as_array().unwrap().iter().filter(|a| a["account_id"] == account)
        .flat_map(|a| a["folders"].as_array().unwrap().iter().map(|f| (f["source"]["folder"].as_str().unwrap().to_string(), f["enabled"].as_bool().unwrap()))).collect()
}
fn wait_folders(core: &RustCore, account: &str, n: usize) { wait_until("the folders of the account are sources", || folders_of(core, account).len() == n); }
fn defaults(sent: &str) -> BTreeMap<String, bool> {
    [("INBOX", true), (sent, true), ("Projects", false), ("Receipts", false)].iter().map(|(n, on)| (n.to_string(), *on)).collect()
}
fn fresh_core() -> (RustCore, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(&dir.path().join("whoosh.sqlite")).unwrap();
    (RustCore::with_parts(store, kc(), detector(), TestTransport::PlainLoopback), dir)
}

#[test]
fn a_new_installation_starts_with_auto_off_and_the_inbox_and_the_sent_folder_of_each_account_on() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let (imap1, smtp1) = server(&rt, "one@example.test", "Éléments envoyés");
    let (imap2, smtp2) = server(&rt, "two@example.test", "Sent Items");
    let (core, _d) = fresh_core();
    assert_eq!(val(&core, "reply/use/auto")["enabled"], false, "auto-draft is off until the owner turns it on");
    assert_eq!(val(&core, "reply/use/on_demand")["enabled"], true);
    let a1 = add_account(&core, "one@example.test", &imap1, &smtp1);
    let a2 = add_account(&core, "two@example.test", &imap2, &smtp2);
    wait_folders(&core, &a1, 4);
    wait_folders(&core, &a2, 4);
    assert_eq!(folders_of(&core, &a1), defaults("Éléments envoyés"), "Inbox and the SPECIAL-USE Sent folder on; Projects and Receipts off");
    assert_eq!(folders_of(&core, &a2), defaults("Sent Items"));
    let pane = val(&core, "reply/sources");
    assert_eq!(pane["enabled_total"], 4, "exactly the Inbox and the Sent folder of each of the two accounts");
    assert_eq!(pane["local_folders"].as_array().unwrap().len(), 0, "no folder of this Mac is a source until the owner adds one");
    assert_eq!(val(&core, "reply/use/auto")["enabled"], false, "adding accounts did not turn auto-draft on");
    // a calendar the shell lists starts off too
    run(&core, Command::SyncReplyCalendars { calendars: vec![cal("c1", "Work")], window_start_ms: 0, window_end_ms: 10_000, events: vec![] });
    let pane = val(&core, "reply/sources");
    assert_eq!((pane["calendars"][0]["enabled"].as_bool(), pane["enabled_total"].as_u64()), (Some(false), Some(4)));
}
// mutants D1 D2 (the seed), D3 D4 (the roles), D5 (no registration at all), D8 (a calendar starts on).

#[test]
fn an_account_added_later_gets_the_same_two_folders_on_and_nothing_else() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let (imap1, smtp1) = server(&rt, "one@example.test", "Éléments envoyés");
    let (imap2, smtp2) = server(&rt, "two@example.test", "Sent Items");
    let (core, _d) = fresh_core();
    let a1 = add_account(&core, "one@example.test", &imap1, &smtp1);
    wait_folders(&core, &a1, 4);
    let first = folders_of(&core, &a1);
    let a2 = add_account(&core, "two@example.test", &imap2, &smtp2);
    wait_folders(&core, &a2, 4);
    assert_eq!(folders_of(&core, &a2), defaults("Sent Items"));
    assert_eq!(folders_of(&core, &a1), first, "the first account did not change");
    assert_eq!(val(&core, "reply/sources")["enabled_total"], 4);
}
// mutant D7: only the first account's folders are registered.

#[test]
fn a_choice_the_owner_made_is_never_reset_by_a_later_sync_or_a_restart() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let (imap, smtp) = server(&rt, "one@example.test", "Éléments envoyés");
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("whoosh.sqlite");
    let keys = kc();
    let core = RustCore::with_parts(Store::open(&path).unwrap(), keys.clone(), detector(), TestTransport::PlainLoopback);
    let a = add_account(&core, "one@example.test", &imap, &smtp);
    wait_folders(&core, &a, 4);
    let folder = |name: &str| ReplySourceRef::MailFolder { account_id: a.clone(), folder: name.into() };
    run(&core, Command::SetReplySourceEnabled { source: folder("INBOX"), enabled: false });
    run(&core, Command::SetReplySourceEnabled { source: folder("Projects"), enabled: true });
    run(&core, Command::SetReplyUseEnabled { kind: ReplyUseKind::Auto, enabled: true });
    let chosen: BTreeMap<String, bool> = [("INBOX", false), ("Éléments envoyés", true), ("Projects", true), ("Receipts", false)].iter().map(|(n, on)| (n.to_string(), *on)).collect();
    assert_eq!(folders_of(&core, &a), chosen);
    // the server grows a folder; the app restarts over the same store: the sync lists the folders again
    imap.with(|s| s.boxes.push(mbox("Newsletters", None)));
    drop(core);
    let core = RustCore::with_parts(Store::open(&path).unwrap(), keys, detector(), TestTransport::PlainLoopback);
    core.start_stored_accounts();
    wait_folders(&core, &a, 5);
    let mut expected = chosen;
    expected.insert("Newsletters".into(), false);
    assert_eq!(folders_of(&core, &a), expected, "the new folder is off; every choice of the owner is as it was");
    assert_eq!(val(&core, "reply/use/auto")["enabled"], true, "the owner's switch of the auto use is kept");
}
// mutant D6: reply_register_folders resets the switch of a folder it already knows.

#[test]
fn a_folder_removed_on_the_server_is_switched_off_and_stays_in_the_pane() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let (imap, smtp) = server(&rt, "one@example.test", "Éléments envoyés");
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("whoosh.sqlite");
    let keys = kc();
    let core = RustCore::with_parts(Store::open(&path).unwrap(), keys.clone(), detector(), TestTransport::PlainLoopback);
    let a = add_account(&core, "one@example.test", &imap, &smtp);
    wait_folders(&core, &a, 4);
    run(&core, Command::SetReplySourceEnabled { source: ReplySourceRef::MailFolder { account_id: a.clone(), folder: "Projects".into() }, enabled: true });
    assert_eq!(folders_of(&core, &a)["Projects"], true);
    imap.with(|s| s.boxes.retain(|b| b.name != "Projects"));
    drop(core);
    let core = RustCore::with_parts(Store::open(&path).unwrap(), keys, detector(), TestTransport::PlainLoopback);
    core.start_stored_accounts();
    wait_until("the lost folder is switched off", || folders_of(&core, &a).get("Projects") == Some(&false));
    let after = folders_of(&core, &a);
    assert_eq!(after.len(), 4, "the row is kept: the pane still lists the folder, now off (it shows the count it had)");
    assert_eq!((after["INBOX"], after["Éléments envoyés"], after["Receipts"]), (true, true, false), "the other folders are as they were");
}
// mutants D9 (a lost folder keeps its switch), D10 (an empty LIST switches every folder off), D11 (the registration reads the views' reader first), D12 (a failed registration panics).

#[test]
fn on_a_server_without_special_use_the_sent_folder_is_found_by_its_name() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    for (i, sent) in ["Sent", "Sent Items", "Éléments envoyés", "Gesendet"].iter().enumerate() {
        let user = format!("user{i}@example.test");
        let (imap, smtp) = server_with(&rt, &user, sent, false);
        let (core, _d) = fresh_core();
        let a = add_account(&core, &user, &imap, &smtp);
        wait_folders(&core, &a, 4);
        assert_eq!(folders_of(&core, &a), defaults(sent), "{sent}: Inbox and this Sent folder on, Projects and Receipts off");
        assert_eq!(val(&core, "reply/sources")["enabled_total"], 2);
    }
}
// mutant D13: the name fallback of `register_folders` is removed (the localized names are no role, so their Sent folder stays off).

// ---------------------------------------------------------------- round 6: what a folder's tile says of its sync (no state lasts for ever)
// Mutants (`_captures/replies-settings/mutants-run-r6.log`): G1 a switched-off folder reads as syncing; G2 an empty folder whose first sync ended is not up to date; G3 the first-sync
// record is empty; G4 a folder with messages reads up to date at once; G5 the crawl's report uses the contract's function again (the stored word stays "syncing" for an empty folder).

/// A meaning model for the test: every text gets the same unit-ish vector, so the indexer can embed.
struct AnyEmbedder;
impl Embedder for AnyEmbedder {
    fn dimension(&self) -> u32 { search_vector::constants::EMBED_DIM as u32 }
    fn embed(&self, texts: Vec<String>) -> Result<Vec<Vec<f32>>, CoreError> { Ok(texts.iter().map(|_| vec![0.5f32; search_vector::constants::EMBED_DIM]).collect()) }
    fn embed_query(&self, _text: String) -> Result<Vec<f32>, CoreError> { Ok(vec![0.5f32; search_vector::constants::EMBED_DIM]) }
    fn embed_titled(&self, _title: Option<String>, texts: Vec<String>) -> Result<Vec<Vec<f32>>, CoreError> { self.embed(texts) }
    fn embed_media(&self, _kind: MediaKind, _path: String) -> Result<Vec<f32>, CoreError> { Ok(vec![0.5f32; search_vector::constants::EMBED_DIM]) }
}

/// One folder's tile of the Sources pane: (switch, item count, progress state, percent).
fn tile(core: &RustCore, account: &str, folder: &str) -> (bool, u64, String, Option<u64>) {
    let pane = val(core, "reply/sources");
    let t = pane["accounts"].as_array().unwrap().iter().filter(|a| a["account_id"] == account).flat_map(|a| a["folders"].as_array().unwrap().clone())
        .find(|f| f["source"]["folder"] == folder).unwrap_or_else(|| panic!("no tile for {folder}"));
    (t["enabled"].as_bool().unwrap(), t["item_count"].as_u64().unwrap(), t["progress"]["state"].as_str().unwrap().to_string(), t["progress"]["percent"].as_u64())
}
fn wait_tile(core: &RustCore, account: &str, folder: &str, state: &str) { wait_until(&format!("{folder} reads {state}"), || tile(core, account, folder).2 == state); }

#[test]
fn an_empty_folder_whose_first_sync_ended_is_up_to_date_and_stays_so() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let (imap, smtp) = server(&rt, "one@example.test", "Sent Items");
    let (core, _d) = fresh_core();
    let a = add_account(&core, "one@example.test", &imap, &smtp);
    wait_folders(&core, &a, 4);
    wait_tile(&core, &a, "Sent Items", "up_to_date");
    wait_tile(&core, &a, "INBOX", "up_to_date");
    assert_eq!(tile(&core, &a, "Sent Items"), (true, 0, "up_to_date".to_string(), None));
    // an empty folder the owner switches on later is up to date at once: its first sync ended long ago
    assert_eq!(tile(&core, &a, "Projects").2, "not_started");
    run(&core, Command::SetReplySourceEnabled { source: ReplySourceRef::MailFolder { account_id: a.clone(), folder: "Projects".into() }, enabled: true });
    assert_eq!(tile(&core, &a, "Projects").2, "up_to_date");
    // the crawl's own report (the event and the stored word) says the same as the pane
    wait_until("the stored progress of the empty Sent folder is up to date", || {
        core.store().reply_sources().unwrap().iter().any(|s| matches!(&s.source, ReplySourceRef::MailFolder { folder, .. } if folder == "Sent Items") && s.progress == ReplySyncProgress::UpToDate)
    });
    core.background().settle_crawl(std::time::Duration::from_secs(20));
    assert_eq!(tile(&core, &a, "Sent Items").2, "up_to_date", "it does not go back");
}

#[test]
fn a_folder_with_three_messages_reads_syncing_until_they_are_indexed_then_up_to_date() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let (imap, smtp) = server(&rt, "one@example.test", "Sent Items");
    imap.seed("INBOX", 3);
    let (core, _d) = fresh_core();
    let a = add_account(&core, "one@example.test", &imap, &smtp);
    wait_folders(&core, &a, 4);
    wait_until("the three messages are stored", || tile(&core, &a, "INBOX").1 == 3);
    let bg = core.background();
    assert!(bg.settle_crawl(std::time::Duration::from_secs(30)) && bg.settle_index(std::time::Duration::from_secs(30)));
    // stored and chunked, but no meaning model handed over: not up to date, and it says syncing with a real figure
    let (on, count, state, percent) = tile(&core, &a, "INBOX");
    assert_eq!((on, count, state.as_str(), percent), (true, 3, "syncing", Some(0)), "three messages, none indexed yet");
    bg.set_embedder(Some(std::sync::Arc::new(AnyEmbedder)));
    assert!(bg.settle_index(std::time::Duration::from_secs(30)));
    wait_tile(&core, &a, "INBOX", "up_to_date");
    assert_eq!(tile(&core, &a, "INBOX").1, 3);
    assert_eq!(val(&core, "reply/progress")[0]["indexed"], 3);
}

#[test]
fn a_switched_off_folder_reads_not_started_whatever_the_sync_does() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let (imap, smtp) = server(&rt, "one@example.test", "Sent Items");
    imap.seed("Projects", 2);
    let (core, _d) = fresh_core();
    let a = add_account(&core, "one@example.test", &imap, &smtp);
    wait_folders(&core, &a, 4);
    wait_until("Projects' two messages are stored", || tile(&core, &a, "Projects").1 == 2);
    let bg = core.background();
    assert_eq!(tile(&core, &a, "Projects"), (false, 2, "not_started".to_string(), None), "off: its messages are counted, its sync is not started");
    assert!(bg.settle_crawl(std::time::Duration::from_secs(30)));
    bg.set_embedder(Some(std::sync::Arc::new(AnyEmbedder)));
    assert!(bg.settle_index(std::time::Duration::from_secs(30)));
    assert_eq!(tile(&core, &a, "Projects").2, "not_started", "a crawl and an indexer that ran did not start an off folder's tile");
    assert_eq!(tile(&core, &a, "Receipts").2, "not_started");
    // switched on it reads the figure; switched off again it is not started
    let folder = ReplySourceRef::MailFolder { account_id: a.clone(), folder: "Projects".into() };
    run(&core, Command::SetReplySourceEnabled { source: folder.clone(), enabled: true });
    assert_ne!(tile(&core, &a, "Projects").2, "not_started");
    run(&core, Command::SetReplySourceEnabled { source: folder, enabled: false });
    assert_eq!(tile(&core, &a, "Projects").2, "not_started");
}
