//! T-03 and T-04 (engine half) of docs/lanes/realcore-firstrun.testplan.md. A real core after a first sync (two fake IMAP accounts, three messages in the first)
//! answers every row of `docs/STATE-PATHS.json` with the JSON kind the file names, or the row says `empty_by_design` and why; the pane reads of the
//! `WhooshCore` trait (labels, ai_providers, agent_configs, speech_engine, decision_model, accounts, sync_states) answer their kinds; a row marked `gap` (a path
//! the fixture serves and the real core does not) is printed and must stay unanswered; and each command in `async_commands` (SaveProviderKey) answers at once and
//! lands on the state paths the file names, on a fake provider on 127.0.0.1 only.
//!
//! Mutants (single edits, each run and seen red: `_captures/replies-settings/mutants-run-r5.log`, runner `run-mutants.py`): P1 the `reply/sources` arm of
//! replies.rs removed (the plan's mutant); P2 `reply/notes` answers a number; P3 and P4 ai_settings.rs `ai_provider/<id>/state` and `/refusal` arms stop answering
//! (T-04); P5 the `inbox/tabs` arm stops answering; P6 `label/<id>/chains/<placement>` stops answering; P7 `RustCore::agent_configs` answers an empty list.
mod first_run_support;

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::time::{Duration, Instant};

use contracts::*;
use first_run_support::*;
use serde_json::Value;

fn file() -> Value {
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../docs/STATE-PATHS.json");
    serde_json::from_str(&std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{} cannot be read: {e}", p.display()))).expect("docs/STATE-PATHS.json is JSON")
}

fn kind_of(v: &Value) -> &'static str {
    match v { Value::Null => "null", Value::Bool(_) => "bool", Value::Number(_) => "number", Value::String(_) => "string", Value::Array(_) => "array", Value::Object(_) => "object" }
}

/// What a `<placeholder>` stands for when the row does not list its values: taken from the real core.
fn resolve(rig: &Rig, name: &str) -> Option<Vec<String>> {
    let query = InboxQuery { tab_id: "all".into(), account_id: None, label_id: None, mailbox: None, account_ids: vec![], filters: vec![], smart_folder_id: None };
    let messages = || rig.core.inbox(query.clone()).into_iter().map(|r| r.message_id).collect::<Vec<_>>();
    match name {
        "label" => Some(rig.core.labels().into_iter().map(|l| l.id).collect()),
        "provider" => Some(rig.core.ai_providers().into_iter().map(|p| p.id).collect()),
        "agent" => Some(rig.core.agent_configs().into_iter().map(|a| a.id).collect()),
        "message" => Some(messages()),
        "thread" => Some(messages().iter().filter_map(|m| rig.core.message(m.clone())).map(|m| m.thread_id).collect()),
        "account" => Some(rig.accounts.clone()),
        _ => None,
    }
}

/// The concrete paths of one row: every placeholder `<name>` replaced by each of its values. A placeholder with no value is an Err the caller names in words.
fn expand(rig: &Rig, row: &Value) -> Result<Vec<String>, String> {
    let mut paths = vec![row["path"].as_str().ok_or("a row has no path")?.to_string()];
    while let Some(i) = paths.iter().position(|p| p.contains('<')) {
        let p = paths.remove(i);
        let (a, rest) = p.split_once('<').unwrap();
        let (name, tail) = rest.split_once('>').ok_or_else(|| format!("{}: a placeholder is not closed", row["path"]))?;
        let values: Vec<String> = match row["instances"][name].as_array() {
            Some(v) => v.iter().map(|x| x.as_str().unwrap().to_string()).collect(),
            None => resolve(rig, name).ok_or_else(|| format!("NOVALUES {}: the placeholder <{name}> is not one the test can fill (label, provider, agent, message, thread, account): list its values in `instances`", row["path"]))?,
        };
        if values.is_empty() { return Err(format!("NOVALUES {}: <{name}> has no value on this core: the row cannot be tested", row["path"])); }
        for v in values { paths.insert(i, format!("{a}{v}{tail}")); }
    }
    Ok(paths)
}

#[test]
fn every_state_path_the_shell_reads_answers_its_json_kind_on_a_new_installation() {
    let rig = Rig::first_sync(3);
    let doc = file();
    let rows = doc["paths"].as_array().expect("paths");
    assert!(rows.len() >= 30, "the file holds {} rows", rows.len());
    let (mut bad, mut gaps, mut tested, mut rig_only, mut untyped) = (vec![], vec![], 0usize, vec![], vec![]);
    let declared_gaps = rows.iter().filter(|r| r["gap"].is_string()).count();
    let mut gap_paths_seen = 0usize;
    for row in rows {
        if let Some(why) = row["rig_only"].as_str() {      // served by a rig core only (e.g. rig/refused-outbound): this core is not a rig core, so it is listed, not asked
            rig_only.push(format!("RIG-ONLY {}: {why}", row["path"]));
            continue;
        }
        // A path the shell reads and nobody typed is a FINDING, named here, never a panic: `checks/parity-paths.py --write` adds such a row with `"kind": null`
        // the moment a new read appears, and this test says which one and what to do (docs/STATE-PATHS.json `about`).
        let Some(kind_text) = row["kind"].as_str() else { untyped.push(format!("UNTYPED {}: add its kind (a JSON kind, `absent`, or `gap` / `empty_by_design` with the reason)", row["path"].as_str().unwrap_or("?"))); continue; };
        let kinds: Vec<&str> = kind_text.split('|').collect();
        let concrete = match expand(&rig, row) { Ok(c) => c, Err(e) => { bad.push(e); continue; } };
        for path in concrete {
            tested += 1;
            let answer = rig.state(&path);
            if let Some(why) = row["gap"].as_str() {
                gap_paths_seen += 1;
                match answer { None => gaps.push(format!("GAP      {path}: {why}")), Some(a) => bad.push(format!("SERVED   {path} is a `gap` row but the real core now answers {a:?}: move the row out of the gaps")) }
                continue;
            }
            if kinds == ["absent"] {
                if let Some(a) = answer { bad.push(format!("SERVED   {path} must never be answered, the core answers {a:?}")); }
                continue;
            }
            if kinds == ["text"] {   // plain words, not JSON (decision_preview/words: the shell reads the sentence as it is, "" when the preview needs none): any answer passes, silence does not
                if answer.is_none() && !row["empty_by_design"].is_string() { bad.push(format!("EMPTY    {path}: the core answers nothing; the file says text and gives no `empty_by_design`")); }
                continue;
            }
            match answer {
                None if row["empty_by_design"].is_string() => {}
                None => bad.push(format!("EMPTY    {path}: the core answers nothing; the file says {} and gives no `empty_by_design`", row["kind"])),
                Some(text) => match serde_json::from_str::<Value>(&text) {
                    Err(e) => bad.push(format!("NOT JSON {path}: {text:?}: {e}")),
                    Ok(v) if !kinds.contains(&kind_of(&v)) => bad.push(format!("KIND     {path}: the file says {}, the core answers a {}: {text}", row["kind"], kind_of(&v))),
                    Ok(_) => {}
                },
            }
        }
    }
    println!("{} rows, {tested} concrete paths, {} gaps, {} rig-only, {} untyped:\n{}\n{}", rows.len(), gaps.len(), rig_only.len(), untyped.len(), gaps.join("\n"), rig_only.join("\n"));
    bad.extend(untyped.iter().cloned());
    assert!(bad.is_empty(), "docs/STATE-PATHS.json and the real core disagree on {} path(s) ({} of them untyped):\n{}", bad.len(), untyped.len(), bad.join("\n"));
    // What the old `assert!(!gaps.is_empty())` meant to protect, said so it can pass when nothing is left to fix: the file was read and every row that is not
    // rig-only was asked at least once (an empty or skipped loop proves nothing), and every row the file declares a `gap` was actually put to the core
    // (a gap row the loop skipped would let a served path hide). With zero gaps declared, zero gap rows must have been asked.
    assert!(tested >= rows.len() - rig_only.len() - untyped.len() && tested > 0, "{} rows (of which {} rig-only, {} untyped) but only {tested} concrete paths were asked: the loop did not ask every row", rows.len(), rig_only.len(), untyped.len());
    assert!(gap_paths_seen >= declared_gaps, "the file declares {declared_gaps} gap row(s) but only {gap_paths_seen} gap path(s) were put to the core");
}

#[test]
fn the_pane_reads_of_the_trait_answer_their_kinds() {
    let rig = Rig::first_sync(3);
    let r = reads(&rig);
    let state_synced = rig.core.sync_states();
    let mut bad = vec![];
    for row in file()["reads"].as_array().unwrap() {
        let call = row["call"].as_str().unwrap();
        let value = match call {
            "sync_states" => Value::Array(state_synced.iter().map(|s| serde_json::json!({"account": s.account_id, "history_fetched": s.history_fetched})).collect()),
            other => r[other].clone(),
        };
        if value.is_null() && row["empty_by_design"].is_string() { continue; }
        let Some(want) = row["kind"].as_str() else { bad.push(format!("UNTYPED {call}: add its kind")); continue; };
        if kind_of(&value) != want { bad.push(format!("KIND     {call}: the file says {}, the core answers a {}", row["kind"], kind_of(&value))); continue; }
        if let (Some(min), Some(a)) = (row["min"].as_u64(), value.as_array()) {
            if (a.len() as u64) < min { bad.push(format!("SHORT    {call}: {} entries, the file says at least {min}", a.len())); }
        }
    }
    assert!(bad.is_empty(), "the trait reads differ from docs/STATE-PATHS.json:\n{}", bad.join("\n"));
}

// ---------------------------------------------------------------- T-04: a command that answers in two steps

fn header<'a>(headers: &'a [(String, String)], name: &str) -> Option<&'a str> { headers.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str()) }

/// A provider on 127.0.0.1 that answers the Anthropic model list: `slow-good-key` after 400 ms, any key starting `good` at once, every other key refused in the
/// provider's words (401 "invalid x-api-key"). It is the only host this test talks to.
fn fake_provider() -> u16 {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = l.local_addr().unwrap().port();
    std::thread::spawn(move || {
        for s in l.incoming().flatten() {
            std::thread::spawn(move || answer(s));
        }
    });
    port
}

fn answer(mut s: TcpStream) {
    let mut r = BufReader::new(s.try_clone().unwrap());
    let mut first = String::new();
    if r.read_line(&mut first).is_err() { return; }
    let mut headers = Vec::new();
    loop {
        let mut line = String::new();
        if r.read_line(&mut line).is_err() || line.trim().is_empty() { break; }
        if let Some((k, v)) = line.split_once(':') { headers.push((k.trim().to_string(), v.trim().to_string())); }
    }
    let len: usize = header(&headers, "content-length").and_then(|v| v.parse().ok()).unwrap_or(0);
    let mut body = vec![0u8; len];
    let _ = r.read_exact(&mut body);
    let key = header(&headers, "x-api-key").map(String::from).or_else(|| header(&headers, "authorization").and_then(|v| v.strip_prefix("Bearer ")).map(String::from)).unwrap_or_default();
    let path = first.split_whitespace().nth(1).unwrap_or("").to_string();
    if key == "slow-good-key" { std::thread::sleep(Duration::from_millis(400)); }
    let (status, text) = if key.contains("good-key") && path == "/models" {
        (200, r#"{"data":[{"id":"model-a"},{"id":"model-b"}],"has_more":false}"#.to_string())
    } else {
        (401, r#"{"type":"error","error":{"type":"authentication_error","message":"invalid x-api-key"}}"#.to_string())
    };
    let _ = write!(s, "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{text}", text.len());
}

fn wait_state(rig: &Rig, path: &str, not: &str) -> String {
    let until = Instant::now() + Duration::from_secs(10);
    loop {
        let s = rig.state(path).unwrap_or_default();
        if s != not || Instant::now() > until { return s; }
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn save_provider_key_answers_at_once_and_lands_on_the_state_paths_the_file_names() {
    let rig = Rig::first_sync(0);
    let port = fake_provider();
    let store_path = rig.core.store().path().to_path_buf();
    engine::ai_settings::set_provider_address(&store_path, "anthropic", Some(format!("http://127.0.0.1:{port}")));
    let doc = file();
    let cmds = doc["async_commands"].as_array().unwrap();
    assert!(cmds.iter().any(|c| c["command"] == "SaveProviderKey"), "the file names no SaveProviderKey");
    for c in cmds {
        assert_eq!(c["command"], "SaveProviderKey", "docs/STATE-PATHS.json names the command {}: this test has no runner for it", c["command"]);
        let paths: Vec<String> = c["state_paths"].as_array().unwrap().iter().map(|p| p.as_str().unwrap().replace("<provider>", "anthropic")).collect();
        let state = &paths[0];
        // 1. a good key: the command answers, the state reads "checking", then "connected", and the model list lands
        let t0 = Instant::now();
        rig.core.run(Command::SaveProviderKey { provider_id: "anthropic".into(), key: SecretText { value: "slow-good-key".into() } }).expect("a key is accepted for checking");
        assert!(t0.elapsed() < Duration::from_millis(250), "the command waited for the provider: {:?}", t0.elapsed());
        assert_eq!(rig.state(state).as_deref(), Some("\"checking\""), "{state} reads checking at once");
        assert_eq!(wait_state(&rig, state, "\"checking\""), "\"connected\"", "{state} lands on connected");
        let models: Value = serde_json::from_str(&rig.state(&paths[2]).unwrap_or_else(|| panic!("{} answers nothing after the check", paths[2]))).unwrap();
        assert_eq!(models, serde_json::json!(["model-a", "model-b"]), "{} lists the provider's models", paths[2]);
        // 2. a refused key: the state reads refused and the provider's words are on the refusal path
        rig.core.run(Command::SaveProviderKey { provider_id: "anthropic".into(), key: SecretText { value: "bad-key".into() } }).expect("a key is accepted for checking");
        assert_eq!(wait_state(&rig, state, "\"checking\""), "\"refused\"", "{state} lands on refused");
        let words = rig.state(&paths[1]).unwrap_or_else(|| panic!("{} answers nothing after a refusal", paths[1]));
        assert!(words.contains("invalid x-api-key"), "{} carries the provider's own words: {words}", paths[1]);
    }
}
