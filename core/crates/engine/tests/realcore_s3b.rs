//! realcore-slice3b: the labelling flow files BY MEANING on the real core, against a local fake AI provider.
//!
//! The rig copy of the core (`open_production(dir, rig = true)`) reads `WHOOSH_CORE_CHECK_FLOW=1` (two labels seeded by the store's own
//! calls) and `WHOOSH_CORE_CHECK_AI=<port>` (one provider, the table's LM Studio, whose base URL is that loopback port and whose key is a
//! dummy kept in a memory keychain). The fake below plays that provider: it answers the chat/completions shape with the label whose
//! meaning shares a word with the message, else the first candidate, and records every request it gets. Nothing leaves this Mac.
//!
//! One test, because the variables are the process's: this file is its own binary and holds exactly one core.
//!
//! A140 (lane flow3-core): Contracts and Answered are seeded with their AI card off (their entry rule files by itself); the AI files into the
//! third seeded label, Renewals (AI on, step 1 = subject contains "renewal"), only what its step 1 picked. The by-meaning message now lands
//! there, and Contracts is no longer a candidate of the AI.
//!
//! Mutants (single edits, each seen red; the verbatim results are in _captures/realcore-slice3b/mutants.out):
//! - R1 check_flow.rs `arm_ai`: the provider is never armed (the flow has no AI, as before this slice)
//! - R2 check_flow.rs `ProviderPort::decide`: the message body never reaches the provider (`state_text(&state, "")`)
//! - R3 check_flow.rs `ProviderPort::decide`: the provider's pick is read as the first candidate, whatever it said
#[allow(dead_code, unused_imports)]
#[path = "../../sync-imap/tests/support/mod.rs"]
mod imap_fakes;

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use contracts::*;
use engine::RustCore;
use imap_fakes::{smtp::FakeSmtp, FakeImap};
use serde_json::{json, Value};

const USER: &str = "owner@example.test";
const PW: &str = "app-password";
/// A subject Contracts' rule does not catch (it reads the SUBJECT for "contract") and Renewals' step 1 picks ("renewal"), whose body names what
/// the Renewals label means (A140).
const BY_MEANING_SUBJECT: &str = "Orbit renewal terms";
const BY_MEANING_BODY: &str = "Hi, I went through the new contracts today and they look right. Please countersign.";
const RULE_SUBJECT: &str = "Contract renewal for Orbit";

fn wait_until(what: &str, mut ok: impl FnMut() -> bool) {
    let t0 = Instant::now();
    while t0.elapsed() < Duration::from_secs(30) {
        if ok() { return; }
        std::thread::sleep(Duration::from_millis(50));
    }
    panic!("never reached within 30 s: {what}");
}

fn raw(n: u32, subject: &str, body: &str) -> Vec<u8> {
    format!("From: Sender {n} <sender{n}@example.test>\r\nTo: {USER}\r\nSubject: {subject}\r\nMessage-ID: <s3b-{n}@example.test>\r\nDate: Mon, 09 Nov 2026 10:0{n}:00 +0000\r\n\r\n{body}\r\n").into_bytes()
}

// ---------------------------------------------------------------- the fake provider

/// What the fake saw of one request.
#[derive(Debug, Clone)]
struct Seen { path: String, authorization: Option<String>, body: Value }

fn read_request(stream: &mut TcpStream) -> Option<Seen> {
    let mut reader = BufReader::new(stream.try_clone().ok()?);
    let mut first = String::new();
    reader.read_line(&mut first).ok()?;
    let path = first.split_whitespace().nth(1)?.to_string();
    let (mut length, mut authorization) = (0usize, None);
    loop {
        let mut line = String::new();
        reader.read_line(&mut line).ok()?;
        let line = line.trim_end().to_string();
        if line.is_empty() { break; }
        if let Some((k, v)) = line.split_once(':') {
            match k.trim().to_ascii_lowercase().as_str() {
                "content-length" => length = v.trim().parse().ok()?,
                "authorization" => authorization = Some(v.trim().to_string()),
                _ => {}
            }
        }
    }
    let mut body = vec![0u8; length];
    reader.read_exact(&mut body).ok()?;
    Some(Seen { path, authorization, body: serde_json::from_slice(&body).unwrap_or(Value::Null) })
}

/// The label path whose meaning line shares a word of five letters or more with the message; the first candidate when none does.
fn pick(request: &Value) -> Value {
    let user = request["messages"].as_array().and_then(|m| m.iter().find(|x| x["role"] == "user")).and_then(|m| m["content"].as_str()).unwrap_or("");
    let asked: Value = serde_json::from_str(user).unwrap_or(Value::Null);
    let message = asked["state"].as_str().unwrap_or("").to_lowercase();
    let mut answers = serde_json::Map::new();
    let shares = |meaning: &str| meaning.to_lowercase().split(|c: char| !c.is_alphanumeric()).filter(|w| w.len() >= 5).any(|w| message.contains(w));
    // fix-aisort round 3: one yes/no per offered label (`labels/model#<label>`): the same pick over the labels named, a yes for it, a no for the others
    let per_label: Vec<(String, String, String)> = asked["questions"].as_object().into_iter().flatten()
        .filter_map(|(id, q)| ::decision::decide::statement_label(q["instructions"].as_str().unwrap_or("")).map(|(p, d)| (id.clone(), p.to_string(), d.to_string())))
        .collect();
    let chosen_label = per_label.iter().find(|(_, _, d)| shares(d)).or(per_label.first()).map(|(_, p, _)| p.clone());
    for (id, label, _) in &per_label {
        answers.insert(id.clone(), json!({"noul": if Some(label) == chosen_label.as_ref() { 0.97 } else { ::decision::test_answers::FAKE_NO }}));
    }
    for (id, q) in asked["questions"].as_object().into_iter().flatten().filter(|(id, _)| !per_label.iter().any(|(i, _, _)| i == *id)) {
        let options: Vec<String> = q["criteria"].as_object().map(|c| c.keys().cloned().collect()).unwrap_or_default();
        let instructions = q["instructions"].as_str().unwrap_or("");
        let meaning = |option: &str| instructions.lines().find_map(|l| l.strip_prefix(&format!("{option}: "))).unwrap_or("").to_lowercase();
        let hit = options.iter().find(|o| meaning(o).split(|c: char| !c.is_alphanumeric()).filter(|w| w.len() >= 5).any(|w| message.contains(w)));
        let chosen = hit.or(options.first()).cloned().unwrap_or_default();
        answers.insert(id.clone(), json!({"choice": chosen, "confidence": 0.97}));
    }
    json!({"answers": answers})
}

fn start_fake() -> (u16, Arc<Mutex<Vec<Seen>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind loopback");
    let port = listener.local_addr().unwrap().port();
    let seen: Arc<Mutex<Vec<Seen>>> = Arc::new(Mutex::new(Vec::new()));
    let log = seen.clone();
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let mut stream = stream;
            let Some(request) = read_request(&mut stream) else { continue };
            let answer = json!({"choices": [{"message": {"role": "assistant", "content": pick(&request.body).to_string()}}]}).to_string();
            log.lock().unwrap().push(request);
            let _ = write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{answer}", answer.len());
        }
    });
    (port, seen)
}

// ---------------------------------------------------------------- the case

fn under(core: &RustCore, label_id: &str) -> Vec<InboxRow> {
    core.inbox(InboxQuery { tab_id: "all".into(), account_id: None, label_id: Some(label_id.into()), mailbox: None, account_ids: vec![], filters: vec![], smart_folder_id: None })
}

#[test]
fn the_rig_core_files_a_message_by_its_meaning_through_the_provider_the_check_names() {
    let (ai_port, seen) = start_fake();
    let rt = tokio::runtime::Runtime::new().unwrap();
    let imap = rt.block_on(FakeImap::start());
    imap.seed("INBOX", 2);
    imap.with(|s| {
        let inbox = s.boxes.iter_mut().find(|b| b.name == "INBOX").unwrap();
        inbox.msgs[0].raw = raw(1, RULE_SUBJECT, "Please sign.");
        inbox.msgs[1].raw = raw(2, BY_MEANING_SUBJECT, BY_MEANING_BODY);
    });
    let smtp = rt.block_on(FakeSmtp::start(USER, PW));
    let dir = tempfile::tempdir().unwrap();
    // the rig copy's own switches (this binary holds one core, so the process's variables are its own)
    std::env::set_var("WHOOSH_CORE_PLAIN_LOOPBACK", "1");
    std::env::set_var("WHOOSH_CORE_CHECK_FLOW", "1");
    std::env::set_var("WHOOSH_CORE_CHECK_AI", ai_port.to_string());
    let core = RustCore::open_production(dir.path().to_str().unwrap(), true).expect("the rig core opens");
    let contracts = core.store().labels_full().unwrap().into_iter().find(|l| l.name == "Contracts").expect("the rig seeded Contracts").id;
    let renewals = core.store().labels_full().unwrap().into_iter().find(|l| l.name == "Renewals").expect("the rig seeded Renewals (A140)").id;

    let config = AccountConfig {
        imap_host: "127.0.0.1".into(), imap_port: imap.port, imap_security: ConnectionSecurity::Ssl, smtp_host: "127.0.0.1".into(), smtp_port: smtp.port,
        smtp_security: ConnectionSecurity::Starttls, username: USER.into(), smtp_username: USER.into(), auth_method: AuthMethod::Password, source: DetectionSource::Manual,
    };
    core.run(Command::AddAccountConfigured { address: USER.into(), display_name: Some("Owner".into()), config, password: SecretText { value: PW.into() } }).expect("added");
    // A140: the rule message under Contracts (AI off, its step 1 wins first), the by-meaning one under Renewals (AI on, its step 1 picked it)
    wait_until("one message under each label", || { core.flush_filing(); under(&core, &contracts).len() == 1 && under(&core, &renewals).len() == 1 });

    let id_under = |label: &str, subject: &str| { let rows = under(&core, label); let id = rows.iter().find(|r| r.subject == subject).unwrap_or_else(|| panic!("{subject} is not under {label}: {rows:?}")).message_id.clone(); id };
    let (by_rule, by_meaning) = (id_under(&contracts, RULE_SUBJECT), id_under(&renewals, BY_MEANING_SUBJECT));
    let contracts_by_meaning = renewals.clone(); // A140: the label the AI files into is Renewals

    // the message no rule catches: the model placed it, the record says who and which card
    let filing = core.state(format!("message/{by_meaning}/filing")).expect("the filing record");
    assert!(filing.contains("\"card\":\"ai\"") && filing.contains("\"ai\":\"answered\"") && filing.contains("\"decided_by\":\"laya\"") && filing.contains(&format!("\"place\":\"{contracts_by_meaning}\"")), "{filing}");
    // Why here?: the card, no chain (an AI card has none), who decided; A140: the AI's yes and step 1's chain
    let why = core.state(format!("message/{by_meaning}/why/{contracts_by_meaning}")).expect("the why-here read");
    assert!(why.contains("\"card\":\"ai\"") && why.contains("\"chain_id\":null") && why.contains("\"decided_by\":\"laya\""), "{why}");
    assert!(why.contains("\"ai\":\"yes\"") && why.contains("\"entry\":{\"chain_id\":0,"), "{why}");
    // the message an entry rule caught never reached the provider
    let rule_filing = core.state(format!("message/{by_rule}/filing")).expect("the rule filing record");
    assert!(rule_filing.contains("\"card\":\"entry\"") && rule_filing.contains("\"ai\":\"not_reached\""), "{rule_filing}");

    // the fake saw exactly one request: the label's meaning, the message's subject and body, no attachments, the dummy key
    let seen = seen.lock().unwrap().clone();
    assert_eq!(seen.len(), 1, "one request for the one message no rule caught: {seen:?}");
    let r = &seen[0];
    assert_eq!(r.path, "/chat/completions");
    assert!(r.authorization.as_deref().is_some_and(|a| a.starts_with("Bearer ") && a.len() > "Bearer ".len()), "the dummy key travels as a bearer: {:?}", r.authorization);
    let user = r.body["messages"][1]["content"].as_str().expect("the user message");
    let asked: Value = serde_json::from_str(user).expect("the request the decision crate builds");
    let state = asked["state"].as_str().unwrap();
    assert!(state.contains(&format!("subject: {BY_MEANING_SUBJECT}")) && state.contains(BY_MEANING_BODY), "the subject and the body reach the provider: {state}");
    assert!(!state.contains("attachments"), "the label's AI inputs exclude attachments: {state}");
    // fix-aisort round 3: the Labels question in either wire form: one choice (criteria = the labels, the meanings in the instructions), or one
    // yes/no per label (the statement names the label and its meaning)
    let (mut offered, mut meanings) = (Vec::<String>::new(), String::new());
    for question in asked["questions"].as_object().unwrap().values() {
        let instructions = question["instructions"].as_str().unwrap_or("");
        match ::decision::decide::statement_label(instructions) {
            Some((label, meaning)) => { offered.push(label.to_string()); meanings.push_str(&format!("{label}: {meaning}\n")); }
            None => { offered.extend(question["criteria"].as_object().map(|c| c.keys().cloned().collect::<Vec<_>>()).unwrap_or_default()); meanings.push_str(instructions); }
        }
    }
    // A140: the AI-on label whose step 1 picked the message is the candidate; Contracts (AI off) is not offered
    assert!(meanings.contains("Renewals: mail about contract renewals"), "the label's meaning reaches the provider: {meanings}");
    assert!(offered.iter().any(|l| l == "Renewals"), "the label is a candidate: {offered:?}");
    assert!(!offered.iter().any(|l| l == "Contracts"), "an AI-off label is never offered: {offered:?}");
    assert_eq!(r.body["model"], asked["model"], "one model id on both sides of the request");
}
