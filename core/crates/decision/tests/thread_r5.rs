//! Lane labels-r5: the rest of the thread as a fourth input of a label's AI card, cut to each model's size budget, oldest first.
//! (Owner, 2026-11-10: "here in AI filter we should add 'rest of message thread' - beware because laya has a size limit (same for Jev)".)
//!
//! One test per rule: the cut keeps the NEWEST messages and drops the oldest first; it never fails (a budget too small for one
//! message cuts it, a budget too small for a line gives none); Jev's request carries the thread cut to its declared size; the local
//! prompt carries it inside the window and never at the body's cost; off by default; one label ticking it sends it in its own call;
//! a label whose AI card is off sends nothing.
//!
//! Mutants (single edits; each seen red, logs in _captures/labels-r5/mutants.out):
//! - T1 thread_context.rs `fit`: `parts.iter().rev()` -> `parts.iter()` (keeps the OLDEST) -> the_cut_keeps_the_newest_messages_and_drops_the_oldest_first
//! - T2 thread_context.rs `jev_budget_chars` returns `usize::MAX` (no cut) -> jevs_request_carries_the_thread_cut_to_its_declared_size
//! - T3 adapters/local.rs: the guard loop `while estimate_tokens(&prompt) > w && ...` -> `while false && ...` -> the_local_prompt_..._inside_the_window
//! - T4 decide.rs: `labels_reads != default_ai_reads() || labels_thread` -> `labels_reads != default_ai_reads()` -> a_label_ticking_the_thread_sends_it_in_its_own_call
//! - T5 decide.rs `state_with_thread`: drop `rest_of_thread &&` -> the_thread_is_off_by_default
//! - T7 docs/CONSTANTS.md: the row `DECISION_THREAD_CHARS` renamed `DECISION_THREAD_CHARZ` -> the_thread_cap_is_the_ledgers_row
//! - T6 flow.rs `offered_rest_of_thread`: drop `l.ai_enabled &&` -> a_label_whose_ai_card_is_off_sends_no_thread
mod support;
use ai_providers::transport::{Body, Method, Request, Response, Transport};
use contracts::{CoreError, DecisionAnswer, DecisionPort, DecisionProviderKind, DecisionQuestion, DecisionResult, DecisionState};
use decision::adapters::jev::JevDecision;
use decision::adapters::laya_budget::{estimate_tokens, window};
use decision::decide::state_with_thread;
use decision::thread_context::{decode, encode, fit, jev_budget_chars, render, text_of, THREAD_FIELD, THREAD_HEAD};
use decision::tuning::{decision_body_chars, ledger_number};
use decision::{DecideMessage, DecisionSettings, LabelNode, MemoryConsent, Outcome, OutcomeList};
use secrets::{MemoryKeychain, Namespace, Secret, SecretKind, StoreSecret};
use serde_json::{json, Value};
use smart_inbox::facts::ThreadPart;
use smart_inbox::MessageFacts;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use support::*;

const DAY_MS: i64 = 86_400_000;
const T0: i64 = 1_700_000_000_000;

/// `n` earlier messages, oldest first: "m0 xxx..." ... "m<n-1> xxx...", each `chars` long.
fn parts(n: usize, chars: usize) -> Vec<ThreadPart> {
    (0..n)
        .map(|i| {
            let mut text = format!("m{i} ");
            text.push_str(&"x".repeat(chars.saturating_sub(text.len())));
            ThreadPart { from: format!("p{i}@thread-example.com"), at_ms: T0 + i as i64 * DAY_MS, from_owner: i % 3 == 2, text }
        })
        .collect()
}

fn chars(s: &str) -> usize {
    s.chars().count()
}

// ---------------------------------------------------------------- the cut

#[test]
fn the_cut_keeps_the_newest_messages_and_drops_the_oldest_first() {
    let all = parts(6, 400);
    let budget = jev_budget_chars();
    let kept = fit(&all, budget);
    assert!(!kept.is_empty() && kept.len() < all.len(), "some messages fit and some do not: kept {}", kept.len());
    assert_eq!(kept, all[all.len() - kept.len()..], "the kept messages are the newest ones, whole, in order: no gap, no cut text");
    assert!(chars(&render(&kept)) <= budget, "the rendering stays inside the budget");
    let one_more = &all[all.len() - kept.len() - 1..];
    assert!(chars(&render(one_more)) > budget, "the next older message would not have fitted");
    // everything fits when the budget is big enough: nothing is dropped for nothing
    assert_eq!(fit(&all, 1_000_000), all);
}

#[test]
fn the_cut_never_fails() {
    let one = parts(1, 5_000);
    // the newest message alone is over the budget: its start is kept, the rendering still fits
    let kept = fit(&one, 300);
    assert_eq!(kept.len(), 1);
    assert!(kept[0].text.starts_with("m0 ") && chars(&kept[0].text) < 5_000);
    assert!(chars(&render(&kept)) <= 300, "{}", chars(&render(&kept)));
    // a budget that cannot hold the head and one short line gives no thread, not an error
    assert!(fit(&one, 10).is_empty());
    assert!(fit(&one, 0).is_empty());
    assert!(fit(&[], 2_000).is_empty());
    assert_eq!(render(&[]), "");
    // a field that is not the encoding gives no messages
    assert!(decode("not json").is_empty() && decode("[{\"from\":1}]").is_empty());
    let all = parts(4, 120);
    assert_eq!(decode(&encode(&all)), all, "the encoding round-trips");
}

#[test]
fn the_rendering_reads_newest_first_with_a_date_and_who() {
    let text = render(&parts(3, 20));
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines[0], THREAD_HEAD);
    assert!(lines[1].starts_with("[2023-11-16] me: m2 "), "{}", lines[1]);
    assert!(lines[2].starts_with("[2023-11-15] p1@thread-example.com: m1 "), "{}", lines[2]);
    assert!(lines[3].starts_with("[2023-11-14] p0@thread-example.com: m0 "), "{}", lines[3]);
}

// ---------------------------------------------------------------- the engines

struct Seen(Mutex<Vec<Request>>);
impl Transport for Seen {
    fn send(&self, request: &Request) -> Result<Response, String> {
        self.0.lock().unwrap().push(request.clone());
        Ok(Response { status: 200, body: json!({"answers": {"q": {"type": "noul", "noul": 0.5}}}).to_string() })
    }
}

fn question() -> Vec<DecisionQuestion> {
    vec![DecisionQuestion::Noul { id: "q".into(), prompt: "Is this about money?".into() }]
}

fn facts_with_thread(n: usize, chars: usize) -> MessageFacts {
    let mut f = msg("m-now", "ann@clients-example.com", "Re: villa", "the body of the newest message");
    f.thread_before = parts(n, chars);
    f
}

#[test]
fn jevs_request_carries_the_thread_cut_to_its_declared_size() {
    let transport = Arc::new(Seen(Mutex::new(vec![])));
    let keychain = Arc::new(MemoryKeychain::new(Namespace::rig()));
    let key = keychain.execute(SecretKind::ProviderCredential, "jev", &Secret::new("jev-test-key")).unwrap();
    let consent = Arc::new(MemoryConsent::default());
    consent.give(DecisionProviderKind::Jev);
    let engine = JevDecision { transport: transport.clone(), secrets: keychain, key, consent };
    let f = facts_with_thread(12, 600);
    engine.decide(state_with_thread(&f, contracts::default_ai_reads(), true), question()).unwrap();
    let seen = transport.0.lock().unwrap();
    let Body::Json(body) = &seen[0].body else { panic!("json body") };
    let body: Value = serde_json::from_str(body).unwrap();
    let state = body["state"].as_str().unwrap();
    let (head, thread) = state.split_once(&format!("\n\n{THREAD_HEAD}")).expect("the thread follows the body");
    assert!(head.ends_with("the body of the newest message"), "the body is untouched and comes first");
    assert!(chars(&format!("{THREAD_HEAD}{thread}")) <= jev_budget_chars(), "the thread is {} characters, the budget {}", chars(thread), jev_budget_chars());
    assert!(thread.contains("m11 ") && !thread.contains("m0 "), "the newest earlier message is read, the oldest is not");
    assert_eq!(seen[0].method, Method::Post);
    // what the ledger declares for the body bounds the thread too: the thread is not a way round DECISION_BODY_CHARS
    assert!(jev_budget_chars() >= decision_body_chars());
}

fn laya_prompt(f: &MessageFacts) -> (String, Value) {
    let fake = FakeLocal::new(|_, _, _| noul(0.5));
    let engine = local(fake.clone());
    engine.decide(state_with_thread(f, contracts::default_ai_reads(), true), question()).unwrap();
    let (prompt, _) = fake.prompts.lock().unwrap()[0].clone();
    let sent: Value = serde_json::from_str(&prompt).unwrap();
    (prompt, sent)
}

#[test]
fn the_local_prompt_carries_the_thread_inside_the_window_and_never_at_the_bodys_cost() {
    // a thread far bigger than the window leaves the newest messages in the room the body leaves
    let f = facts_with_thread(60, 300);
    let (prompt, sent) = laya_prompt(&f);
    let state = sent["state"].as_str().unwrap();
    assert!(estimate_tokens(&prompt) <= window(TEST_SHAPE), "everything handed over fits the window: {} of {}", estimate_tokens(&prompt), window(TEST_SHAPE));
    assert!(state.contains("the body of the newest message"), "the body is kept whole");
    assert!(state.contains(THREAD_HEAD) && state.contains("m59 "), "the newest earlier message is read");
    assert!(!state.contains("m0 "), "the oldest is not");
    // the body fills the room: the thread gives way, the window still holds and the body is kept
    let mut big = msg("m-big", "ann@clients-example.com", "Re: villa", &"lorem ipsum ".repeat(2_000));
    big.thread_before = parts(5, 300);
    let (prompt, sent) = laya_prompt(&big);
    assert!(estimate_tokens(&prompt) <= window(TEST_SHAPE), "{} of {}", estimate_tokens(&prompt), window(TEST_SHAPE));
    assert!(sent["state"].as_str().unwrap().contains("lorem ipsum"), "the body keeps its place");
}

// ---------------------------------------------------------------- the flow

struct Capture {
    states: Mutex<Vec<(DecisionState, Vec<DecisionQuestion>)>>,
}
impl DecisionPort for Capture {
    fn provider(&self) -> DecisionProviderKind {
        DecisionProviderKind::Local
    }
    fn decide(&self, state: DecisionState, questions: Vec<DecisionQuestion>) -> Result<DecisionResult, CoreError> {
        self.states.lock().unwrap().push((state, questions.clone()));
        let answers = questions
            .iter()
            // fix-aisort round 3: the Labels question answered by its form (one choice, or one yes/no per label)
            .map(|q| decision::test_answers::labels_answer(q, Some("Needs reply"), 0.9).unwrap_or_else(|| match q {
                DecisionQuestion::Choice { id, .. } => DecisionAnswer::Choice { id: id.clone(), choice: "Needs reply".into(), probabilities: HashMap::from([("Needs reply".to_string(), 0.9)]) },
                DecisionQuestion::Score { id, .. } => DecisionAnswer::Score { id: id.clone(), value: 0.0 },
                DecisionQuestion::Noul { id, .. } => DecisionAnswer::Noul { id: id.clone(), yes: false, probability: 0.1 },
            }))
            .collect();
        Ok(DecisionResult { answers, answered_by: DecisionProviderKind::Local, checkpoint: None })
    }
}

/// The Labels page and Needs-my-reply on, over `labels`.
fn run_labels(labels: Vec<LabelNode>, f: &MessageFacts) -> Vec<(DecisionState, Vec<DecisionQuestion>)> {
    let mut settings = DecisionSettings::shipped(DecisionProviderKind::Local, &[Outcome::NeedsMyReply, Outcome::Labels]);
    settings.page_mut(Outcome::Labels).unwrap().list = OutcomeList::labels_from_records(&labels, &[], Some(0.7));
    let engine = Capture { states: Mutex::new(vec![]) };
    DecideMessage { settings: &settings, engine: Some(&engine), tabs: &[], labels: &labels }.execute(f);
    let out = engine.states.lock().unwrap().clone();
    out
}

fn described(id: &str, name: &str) -> LabelNode {
    let mut label = LabelNode::new(id, name, None);
    label.description = "emails waiting for my answer".into();
    label
}

/// One described label; `edit` sets its cards and the thread tick.
fn run(edit: impl Fn(&mut LabelNode), f: &MessageFacts) -> Vec<(DecisionState, Vec<DecisionQuestion>)> {
    let mut label = described("l-reply", "Needs reply");
    edit(&mut label);
    run_labels(vec![label], f)
}

fn labels_call(calls: &[(DecisionState, Vec<DecisionQuestion>)]) -> &(DecisionState, Vec<DecisionQuestion>) {
    calls.iter().find(|(_, qs)| decision::test_answers::asks_labels(qs)).expect("the Labels question was asked")
}

#[test]
fn the_thread_is_off_by_default() {
    let f = facts_with_thread(4, 100);
    // a label that does not tick it: the earlier messages never leave, and the Labels question rides the one shared call
    let calls = run(|_| {}, &f);
    assert!(calls.iter().all(|(s, _)| !s.fields.contains_key(THREAD_FIELD)), "no state carries the thread");
    assert_eq!(calls.len(), 1, "one engine call for every question, as before");
    // the state of a message the way the other pages read it carries no thread either, ticked or not
    assert!(!decision::decide::state_of(&f).fields.contains_key(THREAD_FIELD));
    assert!(!state_with_thread(&f, contracts::default_ai_reads(), false).fields.contains_key(THREAD_FIELD));
}

#[test]
fn a_label_ticking_the_thread_sends_it_in_its_own_call() {
    let f = facts_with_thread(4, 100);
    let calls = run(|l| l.rest_of_thread = true, &f);
    assert_eq!(calls.len(), 2, "the Labels question goes alone with its own state; the other questions keep the shared call");
    let (state, questions) = labels_call(&calls);
    assert_eq!(questions.len(), 1);
    assert_eq!(decode(&state.fields[THREAD_FIELD]), f.thread_before, "the whole thread rides in the state; each engine cuts it to its own budget");
    assert!(calls.iter().filter(|(s, _)| s.fields.contains_key(THREAD_FIELD)).count() == 1, "only the Labels call carries it");
    // nothing to send, nothing separate: a message that opens its thread asks as before
    let alone = msg("m-first", "ann@clients-example.com", "Re: villa", "first message");
    let calls = run(|l| l.rest_of_thread = true, &alone);
    assert_eq!(calls.len(), 1);
    assert!(calls.iter().all(|(s, _)| !s.fields.contains_key(THREAD_FIELD)));
}

#[test]
fn a_label_whose_ai_card_is_off_sends_no_thread() {
    let f = facts_with_thread(4, 100);
    // another label keeps the Labels question alive, so the walk reaches the model; only the AI-off label ticks the thread
    let reading = described("l-reading", "Reading");
    let off = described("l-off", "Off").with_rest_of_thread(true);
    let off = LabelNode { ai_enabled: false, ..off };
    let calls = run_labels(vec![reading, off], &f);
    labels_call(&calls);
    assert!(calls.iter().all(|(s, _)| !s.fields.contains_key(THREAD_FIELD)), "an AI card that is off reads nothing, so it sends no thread");
    assert_eq!(calls.len(), 1, "and the Labels question stays in the shared call");
}

#[test]
fn the_thread_text_the_engines_read_is_the_cut_one() {
    // text_of is the one place a state's thread becomes words: cut to the budget it is given, none when the state carries none
    let f = facts_with_thread(10, 200);
    let state = state_with_thread(&f, contracts::default_ai_reads(), true);
    let text = text_of(&state.fields, 500);
    assert!(chars(&text) <= 500 && text.contains("m9 ") && !text.contains("m0 "));
    assert_eq!(text_of(&state.fields, 0), "");
    assert_eq!(text_of(&HashMap::new(), 500), "");
}

#[test]
fn the_thread_cap_is_the_ledgers_row() {
    let row = ledger_number("DECISION_THREAD_CHARS").expect("docs/CONSTANTS.md holds the row DECISION_THREAD_CHARS");
    assert_eq!(jev_budget_chars(), row as usize, "Jev's cap is the ledger's row, not the body's size by default");
}
