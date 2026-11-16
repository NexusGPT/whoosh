//! The hidden check's labels (rig only, `WHOOSH_CORE_CHECK_FLOW=1`, next to `WHOOSH_CORE_CHECK_SERVERS`): the check cannot make a
//! label through the sheet without driving the labels screen, so the rig copy seeds two with the store's own calls, the ones the
//! labels screen makes (A140: their AI card off, so their entry rule files by itself; a third, "Renewals", keeps its AI card on: its step 1
//! picks a "renewal" subject and the check's AI rules on it). "Contracts" files by an entry rule on the subject; "Answered" files by an entry rule on the fact "already answered by
//! me", which since the owner's ruling of 2026-11-10 means "I wrote earlier in this thread" (the check's Sent message precedes the inbox message
//! that follows it and is stored after it, so the sync's late Sent message flips the inbox one). The label's description keeps its first wording,
//! which the aifake's meanings table and the 3b selftest repeat. Both are pinned, so `core state inbox/tabs` lists them. Nothing else about the core changes,
//! and the owner's copy never reads the variable.
//!
//! realcore-slice3b adds the model: `WHOOSH_CORE_CHECK_AI=<port>` (read in `seed`, so only where `WHOOSH_CORE_CHECK_FLOW=1` is, on the
//! rig copy) arms one AI provider for the label flow, the providers table's LM Studio at `http://127.0.0.1:<port>` with a dummy key kept
//! in a memory keychain, behind the decision crate's own `DecisionPort` (`ProviderPort`). The flow reads it through `flow_ctx::port_of`
//! when the app set none (the owner's copy never arms it, so there the slot stays what the app made it). `WHOOSH_CORE_MUTANT=
//! flow-never-asks-ai` arms nothing: the flow has no provider again. The transport reaches loopback only; nothing leaves this Mac.
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use ai_providers::ports::{CompleteText, TextRequest};
use ai_providers::providers::{build, Provider};
use ai_providers::table::ProviderId;
use ai_providers::transport::{Body, Method, Request, Response, Transport};
use contracts::{
    CoreError, DecisionPort, DecisionProviderKind, DecisionQuestion, DecisionResult, DecisionState, FlowCard, KeychainRef, LabelScope, LabelSpec, MatchMode, MessageFact,
    RuleCondition, RuleSet,
};
use decision::adapters::wire::{read_answers, request_body, state_text};
use secrets::{MemoryKeychain, Namespace, Secret, SecretKind, StoreSecret};
use store::Store;

use crate::consts::AI_HTTP_TIMEOUT_MS;
use crate::rust_core::now_ms;

pub(crate) const CONTRACTS: &str = "Contracts";
pub(crate) const ANSWERED: &str = "Answered";
/// A140 (lane flow3-core): the one seeded label with its AI card ON. Step 1 = subject contains "renewal" (entry chain 0); the AI (the check's
/// fake provider) rules on what it picks from the description; no exit filter. Contracts (AI off) is above it and wins a "contract" subject first.
pub(crate) const RENEWALS: &str = "Renewals";
/// Lane fix-aisort, the hidden slice `--slice-aisort` only (`WHOOSH_CORE_CHECK_AISORT=1` beside `WHOOSH_CORE_CHECK_FLOW=1`): a second AI-on label whose
/// step 1 picks the same "renewal" subjects, so the check's AI has two labels to choose from and answers a mail that shares no word with
/// Renewals' meaning with this one (the fake's first option): Renewals then holds one mail the AI accepted and one it said no to.
pub(crate) const LATER: &str = "Later";
const AISORT_VARIABLE: &str = "WHOOSH_CORE_CHECK_AISORT";

const AI_VARIABLE: &str = "WHOOSH_CORE_CHECK_AI";
const NEVER_ASKS: &str = "flow-never-asks-ai";
/// The only address the check's AI transport reaches, whatever a request names.
const LOOPBACK: &str = "http://127.0.0.1:";
/// The model id the check's requests name and its answers are recorded under (the provider's own words are the fake's).
const CHECK_MODEL: &str = "whoosh-check-model";
/// A stand-in key: it lives in a memory keychain, travels as the bearer, and opens nothing.
const DUMMY_KEY: &str = "check-dummy-key";
const SYSTEM: &str = "Answer the JSON request you are given with one JSON object {\"answers\": {<question id>: {\"choice\": <one option, as written>, \"confidence\": <0 to 1>}}} and nothing else.";
const NAME: &str = "the check's AI provider";

fn spec(name: &str, description: &str, rule: RuleCondition) -> LabelSpec {
    LabelSpec {
        name: name.into(), parent_id: None, colour: None, description: description.into(), scope: LabelScope::All,
        rules: RuleSet { conditions: vec![rule], match_mode: MatchMode::Any }, pinned: true,
    }
}

/// Seeds the two labels once (a second open finds them and adds nothing), and arms the check's AI provider when asked.
pub(crate) fn seed(store: &Store) -> Result<(), String> {
    arm_ai()?;
    let held = store.labels_full().map_err(|e| format!("the check's labels could not be looked up: {e}"))?;
    if held.iter().any(|l| l.name == CONTRACTS) { return Ok(()); }
    let now = now_ms();
    // A140: Contracts and Answered keep "files by an entry rule": their AI card is off, so step 1 alone files (their descriptions stay, the
    // aifake's meanings table repeats them). Renewals is the AI-on case: step 1 picks, the AI rules.
    let rules = [
        (spec(CONTRACTS, "mail about contracts", RuleCondition::SubjectContains { words: "contract".into() }), false),
        (spec(ANSWERED, "mail I have answered", RuleCondition::Fact { fact: MessageFact::RepliedByMe, wanted: true }), false),
        (spec(RENEWALS, "mail about contract renewals to read and countersign", RuleCondition::SubjectContains { words: "renewal".into() }), true),
    ];
    let later = (std::env::var(AISORT_VARIABLE).as_deref() == Ok("1"))
        .then(|| (spec(LATER, "kept for someday", RuleCondition::SubjectContains { words: "renewal".into() }), true));
    for (s, ai_on) in rules.into_iter().chain(later) {
        let name = s.name.clone();
        let made = store.label_create_full(s, now).map_err(|e| format!("the check's label {name} could not be made: {e}"))?;
        if !ai_on {
            store.label_set_card(made.id, FlowCard::Ai, false, now).map_err(|e| format!("the check's label {name} could not switch its AI card off: {e}"))?;
        }
    }
    Ok(())
}

/// The provider the check armed; empty when none was asked for, or under the mutant.
static AI: OnceLock<Option<Arc<dyn DecisionPort>>> = OnceLock::new();

/// What the flow asks when the app set no provider (`flow_ctx::port_of`).
pub(crate) fn ai_port() -> Option<Arc<dyn DecisionPort>> {
    AI.get().and_then(|p| p.clone())
}

/// Reads `WHOOSH_CORE_CHECK_AI=<port>` once and arms the provider. A value that is not a port is refused in words.
fn arm_ai() -> Result<(), String> {
    let Ok(value) = std::env::var(AI_VARIABLE) else { return Ok(()) };
    let port: u16 = value.trim().parse().map_err(|_| format!("{AI_VARIABLE} is {value:?}, which is not a port number"))?;
    let never = std::env::var("WHOOSH_CORE_MUTANT").as_deref() == Ok(NEVER_ASKS);
    let provider: Option<Arc<dyn DecisionPort>> = if never { None } else { Some(Arc::new(ProviderPort::at(port)?)) };
    let _ = AI.set(provider);
    Ok(())
}

/// The providers table's LM Studio as the label flow's decision model: the decision crate's own request (the same text the local model
/// is handed) goes out as one chat message, and the provider's text is read as the decision crate's `answers` object.
pub(crate) struct ProviderPort {
    complete: Box<dyn Provider>,
    key: KeychainRef,
}

impl ProviderPort {
    pub(crate) fn at(port: u16) -> Result<ProviderPort, String> {
        let keys = Arc::new(MemoryKeychain::new(Namespace::rig()));
        let key = StoreSecret::execute(&*keys, SecretKind::ProviderCredential, "check-ai", &Secret::new(DUMMY_KEY)).map_err(|e| format!("the check's AI key could not be kept: {e}"))?;
        let complete = build(ProviderId::LmStudio, Arc::new(Loopback::new()), keys, Some(format!("{LOOPBACK}{port}")));
        Ok(ProviderPort { complete, key })
    }
}

impl DecisionPort for ProviderPort {
    fn provider(&self) -> DecisionProviderKind {
        DecisionProviderKind::Local
    }

    fn decide(&self, state: DecisionState, questions: Vec<DecisionQuestion>) -> Result<DecisionResult, CoreError> {
        let prompt = request_body(&state_text(&state, &state.text), CHECK_MODEL, &questions);
        let request = TextRequest { model: CHECK_MODEL.into(), system: Some(SYSTEM.into()), prompt, max_output_tokens: decision::tuning::on_device_context_tokens() as u32 };
        let text = CompleteText::execute(&*self.complete, Some(&self.key), &request)?;
        let answers = read_answers(NAME, &text, &questions)?;
        Ok(DecisionResult { answers, answered_by: DecisionProviderKind::Local, checkpoint: Some(CHECK_MODEL.into()) })
    }
}

/// The HTTP seam over the engine's reqwest, loopback only. The client is built on the first request, on the worker's own thread.
struct Loopback {
    client: OnceLock<Result<reqwest::blocking::Client, String>>,
}

impl Loopback {
    fn new() -> Loopback {
        Loopback { client: OnceLock::new() }
    }

    /// `http://127.0.0.1:<digits>/...` and nothing else.
    fn allowed(url: &str) -> bool {
        url.strip_prefix(LOOPBACK).and_then(|rest| rest.split_once('/')).is_some_and(|(port, _)| !port.is_empty() && port.chars().all(|c| c.is_ascii_digit()))
    }
}

impl Transport for Loopback {
    fn send(&self, request: &Request) -> Result<Response, String> {
        if !Loopback::allowed(&request.url) {
            return Err(format!("the check's AI transport reaches {LOOPBACK}<port> only, not {}", request.url));
        }
        let client = self
            .client
            .get_or_init(|| {
                // ring is the TLS provider (Cargo.toml: rustls-no-provider); an Err means one is installed already, which is fine
                let _ = rustls::crypto::ring::default_provider().install_default();
                // no redirect is followed: `allowed` judges the first address only, so a loopback server must not send the client elsewhere
                // the nearest ledger bound for one request to a local server (no AI row exists yet; see docs/lanes/realcore-slice3b.md)
                reqwest::blocking::Client::builder().redirect(reqwest::redirect::Policy::none()).timeout(Duration::from_millis(AI_HTTP_TIMEOUT_MS)).build().map_err(|e| format!("{e:?}"))
            })
            .as_ref()
            .map_err(|e| format!("the check's AI connection could not start: {e}"))?;
        let builder = match request.method {
            Method::Get => client.get(&request.url),
            Method::Post => client.post(&request.url),
        };
        let mut builder = request.headers.iter().fold(builder, |b, (k, v)| b.header(k.as_str(), v.as_str()));
        builder = match &request.body {
            Body::None => builder,
            Body::Json(text) => builder.body(text.clone()),
            Body::Bytes { content_type, data } => builder.header("Content-Type", content_type.as_str()).body(data.clone()),
        };
        let response = builder.send().map_err(|e| e.without_url().to_string())?;
        let status = response.status().as_u16();
        let body = response.text().map_err(|e| e.without_url().to_string())?;
        Ok(Response { status, body })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use contracts::{ChainPlacement, RuleCondition};

    /// The seam makes the two labels the hidden check reads, pinned, with their entry rule; opening again adds nothing.
    /// Mutant C1: drop the `held` guard in `seed` -> the second seed doubles the labels.
    #[test]
    fn the_check_labels_are_seeded_once_pinned_with_their_entry_rule() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(&dir.path().join("whoosh.sqlite")).unwrap();
        seed(&store).unwrap();
        seed(&store).unwrap();
        let labels = store.labels_full().unwrap();
        let named = |n: &str| labels.iter().filter(|l| l.name == n).collect::<Vec<_>>();
        assert_eq!((named(CONTRACTS).len(), named(ANSWERED).len()), (1, 1), "a second open adds nothing");
        assert!(named(CONTRACTS)[0].pinned && named(ANSWERED)[0].pinned, "pinned: `inbox/tabs` lists them");
        let chain = store.label_chains(&named(ANSWERED)[0].id, ChainPlacement::Entry).unwrap();
        assert_eq!(chain.len(), 1);
        assert_eq!(chain[0].rules.conditions, vec![RuleCondition::Fact { fact: MessageFact::RepliedByMe, wanted: true }]);
    }

    /// The rig's AI transport never leaves loopback, whatever address a request names.
    /// Mutant C2: `allowed` -> `true` -> a request to another host is sent.
    #[test]
    fn the_check_transport_reaches_loopback_only() {
        assert!(Loopback::allowed("http://127.0.0.1:4567/chat/completions"));
        for url in ["https://api.openai.com/v1/chat/completions", "http://127.0.0.1/chat/completions", "http://127.0.0.1:45x7/models", "http://127.0.0.1:1@evil.example/x", "http://localhost:4567/models"] {
            assert!(!Loopback::allowed(url), "{url}");
            let err = Loopback::new().send(&Request { method: Method::Get, url: url.into(), headers: vec![], body: Body::None }).unwrap_err();
            assert!(err.contains("loopback") || err.contains("127.0.0.1"), "{err}");
        }
    }

    /// A loopback server that answers 302 to another address is not followed: the 302 comes back as the answer and the second address is
    /// never dialled (it is a second loopback listener that counts, so the test itself opens nothing off this Mac).
    /// Broken copy: remove `.redirect(reqwest::redirect::Policy::none())` from the client in `Loopback::send` -> the second listener is
    /// asked once and the answer is not 302.
    #[test]
    fn the_check_transport_does_not_follow_a_redirect_off_the_first_address() {
        use std::io::{Read, Write};
        use std::net::TcpListener;
        use std::sync::atomic::{AtomicUsize, Ordering::SeqCst};
        fn serve(reply: String) -> (u16, Arc<AtomicUsize>) {
            let l = TcpListener::bind("127.0.0.1:0").expect("bind loopback");
            let port = l.local_addr().unwrap().port();
            let hits = Arc::new(AtomicUsize::new(0));
            let h = hits.clone();
            std::thread::spawn(move || {
                for s in l.incoming() {
                    let Ok(mut s) = s else { continue };
                    h.fetch_add(1, SeqCst);
                    let _ = s.set_read_timeout(Some(Duration::from_secs(5)));
                    let mut buf = [0u8; 4096];
                    let _ = s.read(&mut buf);
                    let _ = s.write_all(reply.as_bytes());
                }
            });
            (port, hits)
        }
        let (second_port, second_hits) = serve("HTTP/1.1 200 OK\r\ncontent-length: 2\r\nconnection: close\r\n\r\nok".into());
        let (first_port, first_hits) = serve(format!("HTTP/1.1 302 Found\r\nlocation: http://127.0.0.1:{second_port}/elsewhere\r\ncontent-length: 0\r\nconnection: close\r\n\r\n"));
        let answer = Loopback::new()
            .send(&Request { method: Method::Post, url: format!("{LOOPBACK}{first_port}/chat/completions"), headers: vec![], body: Body::Json("{}".into()) })
            .expect("the 302 comes back as an answer");
        assert_eq!(answer.status, 302, "the redirect is returned, not followed");
        assert_eq!(first_hits.load(SeqCst), 1, "the first address was asked once");
        assert_eq!(second_hits.load(SeqCst), 0, "the address the 302 names was never dialled");
    }
}
