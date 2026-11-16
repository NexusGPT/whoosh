//! ai-settings-core: the AI settings of Settings, AI and agents in the real core, and the one answer every other engine part asks
//! for: "the model to use" for a job (`model_for`).
//!
//! - SaveProviderKey answers at once (the owner's "The objective is <50ms for any operation"). The pasted key is checked by ONE model
//!   listing on its own thread (`ai_providers::check::check_key`); state `ai_provider/<id>/state` reads "checking" meanwhile, then
//!   "connected" or "refused" with the provider's own words at `ai_provider/<id>/refusal`. An accepted key goes to the Keychain through
//!   the core's secrets port, THEN its provider row (models, the Keychain reference) to the store; a refused key stores nothing.
//! - The key never reaches the database, a state value or core.log: the row holds `service` U+001F `account`, the log holds the
//!   provider's words (ai-providers scrubs a key the provider echoes back).
//! - SaveAgentConfig and ChooseSpeechEngine as the fixture core answers them (Sources/CoreBridge/FixtureCore.swift), on the store.
//! - The built-in agents (Ask my inbox, the Writer) are the fixture's; they are served until the owner saves one, which then wins.
//!
//! - TestProviderKey (fix-teachcore, docs/lanes/hid-ai.md F6; docs/DECISION-MODEL.md:63 "a Test button that sends one fixed sample"): Jev's kept key is read
//!   from the Keychain and ONE fixed sample (`jev_sample`, never the owner's mail) goes through the decision crate's Jev client (`JevDecision`) on its own
//!   thread; the command answers at once, `ai_provider/jev/state` reads "checking", then "connected", or "refused" with Jev's own words. Saving the key sends nothing.
//!
//! The in-flight checks live beside the core, one book per store file (the core's struct is not this lane's to grow).
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use ai_providers::check::{check_key, provider_by_slug};
use ai_providers::ports::{CompleteText, TextRequest};
use ai_providers::providers::{build, Provider};
use ai_providers::table::{info, ProviderId, ProviderInfo, BUILT_IN_AGENT_MODEL, BUILT_IN_AGENT_PROVIDER, SPEECH};
use ai_providers::transport::{Body, Method, Request, Response, Transport};
use contracts::{AgentConfig, AgentPermission, AiProvider, Command, CommandOutcome, CoreError, KeychainRef, SpeechEngine, SpeechEngineKind};
use contracts::{DecisionPort, DecisionProviderKind, DecisionQuestion, DecisionState};
use decision::adapters::jev::JevDecision;
// The provider state words are the contract's (contracts/src/ai_provider_state.rs, delta 24b).
use contracts::{ai_provider_state_key, provider_state_after_check, provider_state_after_save, provider_state_is_usable, provider_state_json, AiProviderState};
use secrets::{DeleteSecret, ReadSecret, Secret, SecretKind, StoreSecret};
use store::ai_settings::{permission_key, SpeechEngineRow};
use store::Store;

use crate::consts::AI_HTTP_TIMEOUT_MS;
use crate::rust_core::{now_ms, Keyring};
use crate::sign_in::authority::{allowed, host_of};
use crate::sign_in::codes;

/// reply-engine (granted edit): one request's bound PER JOB, read from `job_timeout_ms` only. PROPOSED for docs/CONSTANTS.md (named here once until the
/// ledger holds the rows; the key check keeps AI_HTTP_TIMEOUT_MS): a whole reply written by a cloud model, and the voice analysis reading up to a
/// hundred sent messages, take longer than the labelling flow's short answer.
pub use contracts::constants::{AI_DRAFT_TIMEOUT_MS, AI_VOICE_TIMEOUT_MS};   // generated from docs/CONSTANTS.md

/// The bound of one request of `job` (the one place the per-job timeouts are read).
pub fn job_timeout_ms(job: AiJob) -> u64 {
    match job { AiJob::Draft => AI_DRAFT_TIMEOUT_MS, AiJob::VoiceAnalysis => AI_VOICE_TIMEOUT_MS, AiJob::Labels => AI_HTTP_TIMEOUT_MS }
}

/// The refusal code of "no model is set for this job". PROPOSED for the contract (docs/lanes/ai-settings-core.md "Contract proposal").
pub const AI_NO_MODEL_CODE: &str = "ai_no_model";

/// The provider row shown for the model on this Mac (the fixture core's `local`): always connected, never asked for a key.
pub const ON_DEVICE_ID: &str = "local";
const ON_DEVICE_NAME: &str = "On this Mac";
const ON_DEVICE_MODEL: &str = "on-device";

/// The built-in agents' ids (the fixture core's, and the agents crate's `WRITER_ID`).
pub const ASK_AGENT_ID: &str = "inbox";
pub const WRITER_AGENT_ID: &str = "writer";

// The provider state words of `ai_provider/<id>/state` are the contract's (`contracts::AiProviderState`, `ai_provider_state_key`):
// "not connected", "checking", "connected", "refused", and "not checked" for Jev's kept key (no Jev key check is written down).

/// The decision model Jev (crate decision `adapters::jev`): its key is kept like a provider's, under this id, with no model list.
pub const JEV_ID: &str = "jev";
const JEV_NAME: &str = "Jev";

// ---------------------------------------------------------------- the book of one store

#[derive(Default)]
struct KeyCheck {
    /// The newest SaveProviderKey (or remove) of this provider; an older check's answer is dropped.
    generation: u64,
    running: bool,
    /// The provider's words for the last refused key, until the next save or remove.
    refusal: Option<String>,
    /// fix-teachcore: the last check of the key now kept ended accepted (Jev's Test answered); a save or remove clears it.
    accepted: bool,
}

#[derive(Default)]
struct Book {
    checks: Mutex<HashMap<String, KeyCheck>>,
    /// One applier at a time: "is this check still the newest" and its two writes happen under it.
    applying: Mutex<()>,
    speech_state: Mutex<Option<String>>,
    /// Where a provider is asked instead of the table's address (a test's local fake on 127.0.0.1). Production sets none.
    addresses: Mutex<HashMap<String, String>>,
    transport: OnceLock<Arc<dyn Transport>>,
    /// reply-engine: the transports of the jobs, one per timeout.
    timed: Mutex<HashMap<u64, Arc<dyn Transport>>>,
    /// Tests only (`set_scripted_transport`): every model a job uses talks to this instead of HTTPS.
    scripted: Mutex<Option<Arc<dyn Transport>>>,
    /// rig-core R2: the hidden test copy's loopback port for the key providers' hosts, shared by every HTTPS transport of this book
    /// (`route_key_providers_to_loopback`); None on the owner's copy.
    loopback: Arc<Mutex<Option<u16>>>,
    /// rig-core, mutant `choices-in-memory` only: the agents SaveAgentConfig kept here instead of the store.
    agents_in_memory: Mutex<Vec<AgentConfig>>,
}

fn books() -> &'static Mutex<HashMap<PathBuf, Arc<Book>>> {
    static BOOKS: OnceLock<Mutex<HashMap<PathBuf, Arc<Book>>>> = OnceLock::new();
    BOOKS.get_or_init(Default::default)
}

fn book(store_path: &Path) -> Arc<Book> {
    books().lock().unwrap_or_else(|e| e.into_inner()).entry(store_path.to_path_buf()).or_default().clone()
}

impl Book {
    fn https(&self, timeout_ms: u64) -> Arc<ProviderHttps> { Arc::new(ProviderHttps::with(timeout_ms, self.loopback.clone())) }
    /// The key check's transport: the plain one (AI_HTTP_TIMEOUT_MS) with this book's loopback.
    fn transport(&self) -> Arc<dyn Transport> { self.transport.get_or_init(|| Arc::new(ProviderHttps { loopback: self.loopback.clone(), ..ProviderHttps::new() })).clone() }
    /// The transport of a job's model: the test's script when one is set, else HTTPS bounded by `timeout_ms`.
    fn job_transport(&self, timeout_ms: u64) -> Arc<dyn Transport> {
        if let Some(t) = self.scripted.lock().unwrap_or_else(|e| e.into_inner()).clone() { return t; }
        self.timed.lock().unwrap_or_else(|e| e.into_inner()).entry(timeout_ms).or_insert_with(|| self.https(timeout_ms)).clone()
    }
    fn address(&self, slug: &str) -> Option<String> { self.addresses.lock().unwrap_or_else(|e| e.into_inner()).get(slug).cloned() }
    fn checks(&self) -> std::sync::MutexGuard<'_, HashMap<String, KeyCheck>> { self.checks.lock().unwrap_or_else(|e| e.into_inner()) }

    /// A new save (or a remove) of `slug`: every check before it is now stale.
    fn begin(&self, slug: &str, running: bool) -> u64 {
        let mut g = self.checks();
        let c = g.entry(slug.to_string()).or_default();
        c.generation += 1;
        c.running = running;
        c.refusal = None;
        c.accepted = false;
        c.generation
    }

    fn is_newest(&self, slug: &str, generation: u64) -> bool { self.checks().get(slug).is_some_and(|c| c.generation == generation) }

    fn finish(&self, slug: &str, generation: u64, refusal: Option<String>) {
        let mut g = self.checks();
        if let Some(c) = g.get_mut(slug).filter(|c| c.generation == generation) {
            c.running = false;
            c.accepted = refusal.is_none();
            c.refusal = refusal;
        }
    }
}

/// TESTS ONLY (reply-engine): the models of every job of the core at `store_path` talk to `transport` (a script answering in the provider's JSON)
/// instead of HTTPS; None restores HTTPS. The key check is not affected.
#[doc(hidden)]
pub fn set_scripted_transport(store_path: &Path, transport: Option<Arc<dyn Transport>>) {
    *book(store_path).scripted.lock().unwrap_or_else(|e| e.into_inner()) = transport;
}

/// Tests and the hidden check only: ask `slug` at `base_url` (a local fake) instead of the providers table's address. None restores it.
pub fn set_provider_address(store_path: &Path, slug: &str, base_url: Option<String>) {
    let b = book(store_path);
    let mut g = b.addresses.lock().unwrap_or_else(|e| e.into_inner());
    match base_url { Some(u) => { g.insert(slug.to_string(), u); } None => { g.remove(slug); } }
}

/// rig-core R2, the hidden test copy only (`WHOOSH_CORE_CHECK_PROVIDERS=<port>`, read by rig.rs under `open_production`'s `rig`): every
/// request to a key provider's host (the providers table's OpenAI, Anthropic, Gemini, OpenRouter) or to Jev's host (fix-jevrig) goes in clear HTTP to 127.0.0.1:<port>,
/// path kept. The product's own path then runs end to end against checks/realcore_aifake.py: the key check, the model list, `model_for`.
pub fn route_key_providers_to_loopback(store_path: &Path, port: u16) {
    *book(store_path).loopback.lock().unwrap_or_else(|e| e.into_inner()) = Some(port);
}

/// The loopback address of `url` when its host is a key provider's or Jev's (`http://127.0.0.1:<port>/<path>`, path kept: Jev's is
/// `/v1/systemone`, fix-jevrig); None for any other host. Reached only through a book whose loopback port a rig launch set.
pub fn loopback_url(url: &str, port: u16) -> Option<String> {
    let host = host_of(url).to_ascii_lowercase();
    let jev = host_of(decision::adapters::jev::JEV_URL).eq_ignore_ascii_case(&host);
    (jev || key_providers().any(|i| host_of(i.base_url).eq_ignore_ascii_case(&host))).then(|| {
        let rest = url.split_once("://").map(|(_, r)| r).unwrap_or(url);
        let path = rest.find('/').map(|k| &rest[k..]).unwrap_or("/");
        format!("http://127.0.0.1:{port}{path}")
    })
}

// ---------------------------------------------------------------- the providers this core can take a key for

/// A provider of the table whose key the owner pastes (not a sign-in, not a local server).
fn key_provider(slug: &str) -> Option<&'static ProviderInfo> {
    provider_by_slug(slug).map(info).filter(|i| i.needs_key && !i.sign_in)
}

fn key_providers() -> impl Iterator<Item = &'static ProviderInfo> {
    ai_providers::table::PROVIDERS.iter().filter(|i| i.needs_key && !i.sign_in)
}

fn parse_ref(stored: &str) -> Option<KeychainRef> {
    stored.split_once('\u{1f}').map(|(s, a)| KeychainRef { service: s.to_string(), account: a.to_string() })
}

fn log(store: &Store, line: &str) { crate::log::log_line(&crate::log::path_beside(store.path()), line); }

/// The words of a failure as the owner reads them (a provider's refusal is its own words).
fn words_of(e: &CoreError) -> String {
    match e {
        CoreError::Action { failure } => failure.reason_words.clone(),
        CoreError::Invalid { reason_words, .. } | CoreError::Unexpected { reason_words } => reason_words.clone(),
        other => other.to_string(),
    }
}

fn outcome(created_id: Option<String>) -> CommandOutcome { CommandOutcome { undo_entry_id: None, created_id } }

// ---------------------------------------------------------------- commands

/// The commands this module serves (one dispatch line in `RustCore::run`).
pub fn is_command(c: &Command) -> bool {
    // real-settings: RemoveProviderKey and GiveAgentConsent (the contract's, delta 7 and 22) are served here, where the provider rows and their keys are kept
    matches!(c, Command::SaveProviderKey { .. } | Command::SaveAgentConfig { .. } | Command::ChooseSpeechEngine { .. } | Command::RemoveProviderKey { .. } | Command::GiveAgentConsent { .. }
        | Command::TestProviderKey { .. })   // fix-teachcore: Jev's Test button (F6)
}

pub fn run(store: &Arc<Store>, keychain: &Arc<dyn Keyring>, command: Command) -> Result<CommandOutcome, CoreError> {
    match command {
        Command::SaveProviderKey { provider_id, key } => save_provider_key(store, keychain, &provider_id, key.value),
        Command::SaveAgentConfig { config } => save_agent(store, config),
        Command::ChooseSpeechEngine { kind } => choose_speech(store, kind),
        Command::RemoveProviderKey { provider_id } => remove_provider_key(store, keychain, &provider_id),
        Command::GiveAgentConsent { provider_id } => give_consent(store, &provider_id),
        Command::TestProviderKey { provider_id } => test_provider_key(store, keychain, &provider_id),
        other => Err(CoreError::Invalid { field: "command".into(), reason_words: format!("{other:?} is not an AI settings command") }),
    }
}

/// Starts the key check and answers at once. The answer lands on `ai_provider/<id>/state`.
pub fn save_provider_key(store: &Arc<Store>, keychain: &Arc<dyn Keyring>, slug: &str, key: String) -> Result<CommandOutcome, CoreError> {
    if slug == JEV_ID { return save_jev_key(store, keychain, key); }
    let Some(provider) = key_provider(slug) else { return Err(CoreError::NotFound { kind: "provider".into(), id: slug.to_string() }) };
    let key = key.trim().to_string();
    if key.is_empty() { return Err(CoreError::Invalid { field: "key".into(), reason_words: "Paste the key first.".into() }); }
    let b = book(store.path());
    let generation = b.begin(slug, true);
    let (store2, keys, b2, slug2) = (store.clone(), keychain.clone(), b.clone(), slug.to_string());
    let started = std::thread::Builder::new().name("whoosh-ai-key-check".into()).spawn(move || {
        let answer = check_key(provider.id, b2.transport(), &key, b2.address(&slug2));
        apply(&store2, &keys, &b2, provider, generation, answer.map(|models| (models, Secret::new(key.into_bytes()))));
    });
    if let Err(e) = started {
        b.finish(slug, generation, Some(format!("The app could not start the key check: {e}")));
    }
    Ok(outcome(None))
}

/// Jev's key: trimmed, refused in words when empty, kept in the keyring, then its `ai_provider` row (connected, no model list). No request is
/// made: no Jev key check is written down in this repo. Answers at once; `created_id` is the stored reference, `service` U+001F `account`
/// (the shape the store keeps), for the Decisions pane's `SaveDecisionModel { keychain }`.
fn save_jev_key(store: &Arc<Store>, keychain: &Arc<dyn Keyring>, key: String) -> Result<CommandOutcome, CoreError> {
    let key = key.trim().to_string();
    if key.is_empty() { return Err(CoreError::Invalid { field: "key".into(), reason_words: "Paste the Jev key first.".into() }); }
    let b = book(store.path());
    let _one = b.applying.lock().unwrap_or_else(|e| e.into_inner());
    b.begin(JEV_ID, false);
    let r = StoreSecret::execute(&**keychain, SecretKind::ProviderCredential, JEV_ID, &Secret::new(key.into_bytes()))?;
    let stored = format!("{}\u{1f}{}", r.service, r.account);
    if let Err(e) = store.ai_provider_connect(JEV_ID.to_string(), JEV_NAME.to_string(), vec![], stored.clone(), now_ms()) {
        if let Err(d) = DeleteSecret::execute(&**keychain, &r) { log(store, &format!("ai key {JEV_ID}: the Keychain item could not be removed after a failed save: {}", words_of(&d))); }
        return Err(e.into());
    }
    log(store, &format!("ai key {JEV_ID}: kept, {}", ai_provider_state_key(provider_state_after_save(false))));
    Ok(outcome(Some(stored)))
}

// ---------------------------------------------------------------- Jev's Test button (fix-teachcore, F6)

/// The words when the Test is asked with no Jev key kept (the test engine's, FixtureDecide.swift `testJevKey`).
pub const JEV_NO_KEY_WORDS: &str = "Paste the Jev key first.";

/// The ONE fixed sample the Test sends (DECISION-MODEL.md:63): none of the owner's mail, the fields the design lists (from, to, cc, subject, the list header,
/// the body text) as the Jev client writes them (`decision::adapters::wire::state_text_with_thread`).
pub fn jev_sample() -> DecisionState {
    let fields = [("from", "sample@example.com"), ("to", "you@example.com"), ("cc", ""), ("subject", "Whoosh key test"), ("list_header", "no")];
    DecisionState {
        text: "This is the fixed sample Whoosh sends to test a Jev key. It is not one of your messages.".into(),
        fields: fields.into_iter().map(|(k, v)| (k.to_string(), v.to_string())).collect(),
    }
}

/// The sample's one question: the shipped "Which tab?" (DECISION-MODEL.md section 3's table: its instructions and its four tabs).
pub fn jev_sample_question() -> DecisionQuestion {
    DecisionQuestion::Choice {
        id: "which_tab".into(),
        prompt: "Which inbox tab should this email sit in?".into(),
        options: ["Primary", "Updates", "Promotions", "Social"].into_iter().map(String::from).collect(),
    }
}

/// The Test sends the fixed sample and none of the owner's mail, so the owner's consent to send MAIL to Jev (C-J5.4, `decision_model::consent_given`) is not
/// what it asks; this consent covers the sample alone and is never handed to a judgement of the owner's mail.
struct SampleOnly;

impl decision::HasConsent for SampleOnly {
    fn execute(&self, provider: DecisionProviderKind) -> bool { provider == DecisionProviderKind::Jev }
}

/// A transport whose requests go to `base` (scheme, host and port), path kept: Jev's address set by `set_provider_address(.., "jev", ..)` (a test's twin
/// on 127.0.0.1). The inner transport (this book's `ProviderHttps`) still applies the rig guard and refuses clear HTTP past this Mac.
struct Rebased { base: String, inner: Arc<dyn Transport> }

impl Transport for Rebased {
    fn send(&self, request: &Request) -> Result<Response, String> {
        let rest = request.url.split_once("://").map(|(_, r)| r).unwrap_or(&request.url);
        let path = rest.find('/').map(|k| &rest[k..]).unwrap_or("/");
        self.inner.send(&Request { url: format!("{}{path}", self.base.trim_end_matches('/')), ..request.clone() })
    }
}

/// One fixed sample to Jev with the kept key, through the decision crate's client: Ok when Jev answered it, else Jev's words (the client keeps a key the
/// body echoes out of them: `jev_words`).
fn ask_jev_sample(b: &Book, keychain: Arc<dyn Keyring>, key: KeychainRef) -> Result<(), String> {
    let inner = b.transport();
    let transport: Arc<dyn Transport> = match b.address(JEV_ID) { Some(base) => Arc::new(Rebased { base, inner }), None => inner };
    let secrets: Arc<dyn ReadSecret> = keychain;
    let jev = JevDecision { transport, secrets, key, consent: Arc::new(SampleOnly) };
    jev.decide(jev_sample(), vec![jev_sample_question()]).map(|_| ()).map_err(|e| words_of(&e))
}

/// `Command::TestProviderKey`: answers at once; the sample goes on its own thread and its answer lands on `ai_provider/jev/state` and `/refusal`. A save or a
/// remove while it runs makes its answer stale (dropped, as a key check's). The key itself never reaches a log line or a state value.
pub fn test_provider_key(store: &Arc<Store>, keychain: &Arc<dyn Keyring>, slug: &str) -> Result<CommandOutcome, CoreError> {
    if slug != JEV_ID {
        let Some(provider) = key_provider(slug) else { return Err(CoreError::NotFound { kind: "provider".into(), id: slug.to_string() }) };
        return Err(CoreError::Invalid { field: "provider_id".into(), reason_words: format!("{} is checked when its key is saved: paste the key again to check it.", provider.name) });
    }
    let Some(key) = store.ai_provider_row(JEV_ID)?.and_then(|row| row.keychain_ref).as_deref().and_then(parse_ref) else {
        return Err(CoreError::Invalid { field: "key".into(), reason_words: JEV_NO_KEY_WORDS.into() });
    };
    let b = book(store.path());
    let generation = {
        let _one = b.applying.lock().unwrap_or_else(|e| e.into_inner());
        b.begin(JEV_ID, true)
    };
    log(store, &format!("ai key {JEV_ID}: test sent"));
    let (store2, keys, b2) = (store.clone(), keychain.clone(), b.clone());
    let started = std::thread::Builder::new().name("whoosh-jev-key-test".into()).spawn(move || {
        let answer = ask_jev_sample(&b2, keys, key);
        let _one = b2.applying.lock().unwrap_or_else(|e| e.into_inner());
        if !b2.is_newest(JEV_ID, generation) { return; }
        match &answer {
            Ok(()) => log(&store2, &format!("ai key {JEV_ID}: test answered, connected")),
            Err(words) => log(&store2, &format!("ai key {JEV_ID}: test refused: {words}")),
        }
        b2.finish(JEV_ID, generation, answer.err());
    });
    if let Err(e) = started {
        b.finish(JEV_ID, generation, Some(format!("The app could not start the key test: {e}")));
    }
    Ok(outcome(None))
}

/// The check's answer, if it is still the newest: accepted = the key to the Keychain, then the row; refused = nothing kept.
fn apply(store: &Store, keychain: &Arc<dyn Keyring>, b: &Book, provider: &'static ProviderInfo, generation: u64, answer: Result<(Vec<String>, Secret), CoreError>) {
    let _one = b.applying.lock().unwrap_or_else(|e| e.into_inner());
    if !b.is_newest(provider.slug, generation) { return; }
    let refusal = match answer {
        Err(e) => Some(words_of(&e)),
        Ok((models, secret)) => keep(store, keychain, provider, models, &secret).err(),
    };
    match &refusal {
        Some(words) => log(store, &format!("ai key {}: refused: {words}", provider.slug)),
        None => log(store, &format!("ai key {}: accepted", provider.slug)),
    }
    b.finish(provider.slug, generation, refusal);
}

fn keep(store: &Store, keychain: &Arc<dyn Keyring>, provider: &ProviderInfo, models: Vec<String>, secret: &Secret) -> Result<(), String> {
    let r = StoreSecret::execute(&**keychain, SecretKind::ProviderCredential, provider.slug, secret).map_err(|e| format!("The key could not be kept in the Keychain: {}", words_of(&e)))?;
    let stored = format!("{}\u{1f}{}", r.service, r.account);
    if let Err(e) = store.ai_provider_connect(provider.slug.to_string(), provider.name.to_string(), models, stored, now_ms()) {
        // the row is the record the app reads: without it the Keychain item would be an orphan nobody can remove
        if let Err(d) = DeleteSecret::execute(&**keychain, &r) { log(store, &format!("ai key {}: the Keychain item could not be removed after a failed save: {}", provider.slug, words_of(&d))); }
        return Err(format!("The key could not be saved: {e}"));
    }
    Ok(())
}

/// Removes a provider's key: the Keychain item and the provider row (PROPOSED contract command `RemoveProviderKey`). A check in
/// flight is dropped. Removing a provider that holds no key is not an error.
pub fn remove_provider_key(store: &Arc<Store>, keychain: &Arc<dyn Keyring>, slug: &str) -> Result<CommandOutcome, CoreError> {
    if key_provider(slug).is_none() && slug != JEV_ID { return Err(CoreError::NotFound { kind: "provider".into(), id: slug.to_string() }); }
    let b = book(store.path());
    let _one = b.applying.lock().unwrap_or_else(|e| e.into_inner());
    b.begin(slug, false);
    // real-settings: the Keychain item goes FIRST. When the Keychain refuses, the provider row stays (still "connected", the owner is told in words) instead of
    // leaving a key in the Keychain that no row points to and no screen can remove.
    if let Some(r) = store.ai_provider_row(slug)?.and_then(|row| row.keychain_ref).as_deref().and_then(parse_ref) {
        DeleteSecret::execute(&**keychain, &r)?;
    }
    // the owner's "Allow" survives the key (the test engine's `forgetKeyLocked` keeps it): the row stays, without its key; no consent, no row
    if store.agent_consent_at(slug)?.is_some() { store.ai_provider_key_cleared(slug.to_string())?; } else { store.ai_provider_forget(slug.to_string())?; }
    log(store, &format!("ai key {slug}: removed"));
    Ok(outcome(None))
}

/// The owner's "Allow" on the consent question for `provider_id` (once per provider, C-J5.4), kept in its provider row. Read back at
/// `agent_consent/<id>` and as `AiProvider.mail_consent_at_ms`. A provider that is not one of the pane's is NotFound.
pub fn give_consent(store: &Store, provider_id: &str) -> Result<CommandOutcome, CoreError> {
    let name = if provider_id == JEV_ID { JEV_NAME } else if provider_id == ON_DEVICE_ID { ON_DEVICE_NAME } else if let Some(i) = key_provider(provider_id) { i.name } else {
        return Err(CoreError::NotFound { kind: "provider".into(), id: provider_id.to_string() });
    };
    store.agent_consent_give(provider_id.to_string(), name.to_string(), now_ms())?;
    log(store, &format!("ai consent {provider_id}: given"));
    Ok(outcome(None))
}

fn built_ins() -> Vec<AgentConfig> {
    let (provider, model) = (info(BUILT_IN_AGENT_PROVIDER).slug.to_string(), BUILT_IN_AGENT_MODEL.to_string());
    vec![
        AgentConfig { id: ASK_AGENT_ID.into(), name: "Ask my inbox".into(), purpose: "Answers questions about your mail and proposes labels".into(), provider_id: provider.clone(), model: model.clone(), instructions: String::new(), allowed: vec![AgentPermission::ReadMail, AgentPermission::Search, AgentPermission::Label], built_in: true },
        AgentConfig { id: WRITER_AGENT_ID.into(), name: "Writer".into(), purpose: "Drafts replies in your voice".into(), provider_id: provider, model, instructions: String::new(), allowed: vec![AgentPermission::ReadMail, AgentPermission::Search, AgentPermission::Draft], built_in: true },
    ]
}

fn known_provider(id: &str) -> bool { id == ON_DEVICE_ID || key_provider(id).is_some() }

fn save_agent(store: &Store, config: AgentConfig) -> Result<CommandOutcome, CoreError> {
    let name = config.name.trim().to_string();
    if name.is_empty() { return Err(CoreError::Invalid { field: "name".into(), reason_words: "an agent needs a name".into() }); }
    if !known_provider(&config.provider_id) { return Err(CoreError::Invalid { field: "provider".into(), reason_words: "pick a provider for this agent first".into() }); }
    if crate::rig::on(store.path(), crate::rig::RigMutant::SlowSave) { std::thread::sleep(crate::rig::SLOW_SAVE); }
    let all = agents(store);
    let mut c = AgentConfig { name: name.clone(), ..config };
    if c.id.is_empty() {
        if all.iter().any(|a| a.name.to_lowercase() == name.to_lowercase()) {
            return Err(CoreError::Invalid { field: "name".into(), reason_words: format!("an agent called “{name}” exists already") });
        }
        let mut n = all.len() + 1;
        while all.iter().any(|a| a.id == format!("agent-{n}")) { n += 1; }
        c.id = format!("agent-{n}");
        c.built_in = false;
    } else {
        let Some(old) = all.iter().find(|a| a.id == c.id) else { return Err(CoreError::NotFound { kind: "agent".into(), id: c.id }) };
        c.built_in = old.built_in;
    }
    let id = c.id.clone();
    if crate::rig::on(store.path(), crate::rig::RigMutant::ChoicesInMemory) {
        let b = book(store.path());
        let mut kept = b.agents_in_memory.lock().unwrap_or_else(|e| e.into_inner());
        kept.retain(|a| a.id != c.id);
        kept.push(c);
        return Ok(outcome(Some(id)));
    }
    store.agent_config_put(c, now_ms())?;
    Ok(outcome(Some(id)))
}

fn choose_speech(store: &Store, kind: SpeechEngineKind) -> Result<CommandOutcome, CoreError> {
    let b = book(store.path());
    let set = |word: &str| *b.speech_state.lock().unwrap_or_else(|e| e.into_inner()) = Some(word.to_string());
    let provider = match kind {
        // on this Mac: the app's own on-device recognizer (Sources/Features/AttachmentIndex/OnDeviceSpeech.swift); nothing to download here
        SpeechEngineKind::Local => None,
        SpeechEngineKind::Cloud { provider_id } => {
            let name = key_provider(&provider_id).map(|i| i.name).unwrap_or(provider_id.as_str()).to_string();
            if !SPEECH.iter().any(|s| s.slug == provider_id) {
                set("failed");
                return Err(CoreError::Invalid { field: "speech_engine".into(), reason_words: format!("{name} has no speech engine in this app: choose another.") });
            }
            if !store.ai_provider_row(&provider_id)?.is_some_and(|r| r.connected) {
                set("needs key");
                return Err(CoreError::Invalid { field: "speech_engine".into(), reason_words: format!("Add your {name} key in Settings: AI providers first.") });
            }
            Some(provider_id)
        }
    };
    store.speech_engine_put(SpeechEngineRow { cloud_provider_id: provider, ready: true })?;
    set("ready");
    Ok(outcome(None))
}

// ---------------------------------------------------------------- reads

/// The provider rows the pane shows: each key provider of the table (connected once its key was accepted) and the model on this Mac.
pub fn providers(store: &Store) -> Vec<AiProvider> {
    let rows = store.ai_provider_rows().unwrap_or_default();
    let consents = store.agent_consents().unwrap_or_default();
    let consent = |id: &str| consents.iter().find(|(i, _)| i == id).map(|(_, at)| *at);
    let mut out: Vec<AiProvider> = key_providers().map(|i| match rows.iter().find(|r| r.id == i.slug) {
        Some(r) => AiProvider { id: r.id.clone(), name: i.name.to_string(), models: r.models.clone(), connected: r.connected, keychain: r.keychain_ref.as_deref().and_then(parse_ref), mail_consent_at_ms: consent(&r.id) },
        None => AiProvider { id: i.slug.to_string(), name: i.name.to_string(), models: vec![], connected: false, keychain: None, mail_consent_at_ms: None },
    }).collect();
    out.push(AiProvider { id: ON_DEVICE_ID.into(), name: ON_DEVICE_NAME.into(), models: vec![ON_DEVICE_MODEL.into()], connected: true, keychain: None, mail_consent_at_ms: consent(ON_DEVICE_ID) });
    // Jev's row once its key is kept (the Decisions pane reads its Keychain reference here); Jev lists no models
    // (real-settings: only once its key is kept; a consent given before the key makes a row with no key, which is not a Jev row to show)
    if let Some(r) = rows.iter().find(|r| r.id == JEV_ID && r.keychain_ref.is_some()) {
        out.push(AiProvider { id: JEV_ID.into(), name: JEV_NAME.into(), models: vec![], connected: r.connected, keychain: r.keychain_ref.as_deref().and_then(parse_ref), mail_consent_at_ms: consent(JEV_ID) });
    }
    out
}

/// The built-in agents (each replaced by the owner's saved copy), then the agents the owner made, in the order made.
pub fn agents(store: &Store) -> Vec<AgentConfig> {
    let mut saved = store.agent_config_rows().unwrap_or_default();
    for m in book(store.path()).agents_in_memory.lock().unwrap_or_else(|e| e.into_inner()).iter() {
        match saved.iter_mut().find(|s| s.id == m.id) { Some(s) => *s = m.clone(), None => saved.push(m.clone()) }
    }
    let built = built_ins();
    let mut out: Vec<AgentConfig> = built.iter().map(|b| saved.iter().find(|s| s.id == b.id).cloned().unwrap_or_else(|| b.clone())).collect();
    out.extend(saved.into_iter().filter(|s| !built.iter().any(|b| b.id == s.id)));
    out
}

pub fn speech(store: &Store) -> SpeechEngine {
    match store.speech_engine_row().ok().flatten() {
        Some(r) => SpeechEngine { kind: r.cloud_provider_id.map(|provider_id| SpeechEngineKind::Cloud { provider_id }).unwrap_or(SpeechEngineKind::Local), ready: r.ready },
        None => SpeechEngine { kind: SpeechEngineKind::Local, ready: false },
    }
}

fn provider_state(store: &Store, slug: &str) -> AiProviderState {
    if slug == ON_DEVICE_ID { return AiProviderState::Connected; }
    let kept = store.ai_provider_row(slug).ok().flatten().is_some_and(|r| r.connected);
    let b = book(store.path());
    let (running, refused, accepted) = b.checks().get(slug).map(|c| (c.running, c.refusal.is_some(), c.accepted)).unwrap_or((false, false, false));
    // Jev's kept key reads "not checked" until its Test (fix-teachcore, F6): "checking" while the sample is out, then "connected" or "refused"
    if slug == JEV_ID {
        if !kept { return AiProviderState::NotConnected; }
        if running { return provider_state_after_save(true); }
        if refused { return provider_state_after_check(false); }
        return if accepted { provider_state_after_check(true) } else { provider_state_after_save(false) };
    }
    if running { return provider_state_after_save(true); }
    if refused { return provider_state_after_check(false); }
    if kept { provider_state_after_check(true) } else { AiProviderState::NotConnected }
}

pub(crate) fn json_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn json_strings<'a>(words: impl Iterator<Item = &'a str>) -> String { format!("[{}]", words.map(json_string).collect::<Vec<_>>().join(",")) }

/// The AI settings' state paths (one dispatch line in `RustCore::state`), each ONE JSON value, split on `/` and `.` as the fixture splits:
///   ai_provider/<id>/state       "checking" | "connected" | "refused" | "not connected"
///   ai_provider/<id>/refusal     the provider's words for the last refused key (absent otherwise)
///   ai_provider/<id>/connected   true | false (the fixture's path)
///   ai_provider/<id>/models      [model id], the provider's own list from the accepted key's check
///   ai_provider/<id>/key         never answered: a key is never read back (the fixture's path)
///   provider_credential/<id>/stored   true | false: a Keychain reference is kept for it (the fixture's path)
///   speech_engine/chosen/state   "ready" | "needs key" | "failed" (absent before the first choice; the fixture's path)
///   agent_config/<id or name>/allowed   ["readMail", ...] (the fixture's path and spelling)
///   agent_config/<id or name>/model, /provider   the agent's model id and provider slug (real core only)
pub fn state(store: &Store, path: &str) -> Option<String> {
    let p: Vec<&str> = path.split(['/', '.']).collect();
    match p.as_slice() {
        ["ai_provider", id, "state"] if known_provider(id) || *id == JEV_ID => Some(provider_state_json(provider_state(store, id))),
        ["ai_provider", id, "refusal"] => book(store.path()).checks().get(*id).and_then(|c| c.refusal.as_deref().map(json_string)),
        ["ai_provider", id, "connected"] => providers(store).into_iter().find(|a| a.id == *id).map(|a| a.connected.to_string()),
        ["ai_provider", id, "models"] => providers(store).into_iter().find(|a| a.id == *id).map(|a| json_strings(a.models.iter().map(String::as_str))),
        ["provider_credential", id, "stored"] if key_provider(id).is_some() || *id == JEV_ID => Some(store.ai_provider_row(id).ok().flatten().is_some_and(|r| r.keychain_ref.is_some()).to_string()),
        ["speech_engine", "chosen", "state"] => {
            let word = book(store.path()).speech_state.lock().unwrap_or_else(|e| e.into_inner()).clone();
            word.or_else(|| store.speech_engine_row().ok().flatten().map(|r| if r.ready { "ready".into() } else { "needs key".into() })).map(|w| json_string(&w))
        }
        ["agent_config", who, "allowed"] => agents(store).into_iter().find(|a| a.id == *who || a.name.to_lowercase() == who.to_lowercase())
            .map(|a| json_strings(a.allowed.iter().map(permission_key))),
        // rig-core, T-13: the agent's model and provider as the owner chose them (a real-core path; the fixture serves neither yet)
        ["agent_config", who, "model"] => agents(store).into_iter().find(|a| a.id == *who || a.name.to_lowercase() == who.to_lowercase()).map(|a| json_string(&a.model)),
        ["agent_config", who, "provider"] => agents(store).into_iter().find(|a| a.id == *who || a.name.to_lowercase() == who.to_lowercase()).map(|a| json_string(&a.provider_id)),
        _ => None,
    }
}

// ---------------------------------------------------------------- the model to use

/// What a model is asked to do. Each job reads its model from one agent of Settings, AI and agents.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiJob {
    /// An AI reply draft, a rewrite (the Writer).
    Draft,
    /// The analysis of how the owner writes (the Writer).
    VoiceAnalysis,
    /// Sorting mail into labels: the decision model's (Laya on this Mac or Jev), which this core does not keep yet.
    Labels,
}

/// The model a job uses now: its provider (built through the table, over this core's HTTPS), the model id, the Keychain reference the
/// provider reads the key by AT THE CALL (the key itself is never held here), and the provider-side budgets.
pub struct ModelChoice {
    pub provider: ProviderId,
    pub provider_name: &'static str,
    pub model: String,
    pub key: Option<KeychainRef>,
    /// True for a model on this Mac (no key, nothing leaves it): the caller sizes its prompt to the on-device room.
    pub on_device: bool,
    /// One request's bound (`job_timeout_ms`: the job's own; the key check keeps AI_HTTP_TIMEOUT_MS); the job's own token ceilings stay with the job.
    pub timeout_ms: u64,
    adapter: Box<dyn Provider>,
}

impl ModelChoice {
    /// One completion; the provider reads the key from the Keychain now. A refusal is the provider's own words.
    pub fn complete(&self, system: Option<String>, prompt: String, max_output_tokens: u32) -> Result<String, CoreError> {
        CompleteText::execute(&*self.adapter, self.key.as_ref(), &TextRequest { model: self.model.clone(), system, prompt, max_output_tokens })
    }
    pub fn adapter(&self) -> &dyn Provider { &*self.adapter }
}

fn no_model(words: &str) -> CoreError { codes::refusal(words, AI_NO_MODEL_CODE, "") }

/// THE function other engine parts call: the model to use for `job`, or the refusal in words (code `AI_NO_MODEL_CODE`).
pub fn model_for(store: &Store, keychain: &Arc<dyn Keyring>, job: AiJob) -> Result<ModelChoice, CoreError> {
    let agent_id = match job {
        AiJob::Draft | AiJob::VoiceAnalysis => WRITER_AGENT_ID,
        AiJob::Labels => return Err(no_model(crate::decision_model::NO_MODEL_WORDS)), // labels-r5: sorted by the SAVED DECISION MODEL (Jev or Laya), which is not a ModelChoice: decision_model.rs
    };
    let Some(agent) = agents(store).into_iter().find(|a| a.id == agent_id) else { return Err(no_model("No model is set for replies: choose the Writer's provider and model in Settings, AI and agents.")) };
    let Some(provider) = provider_by_slug(&agent.provider_id).map(info) else {
        return Err(no_model(&format!("The Writer's provider {:?} cannot write replies in this version: choose another in Settings, AI and agents.", agent.provider_id)));
    };
    if agent.model.trim().is_empty() { return Err(no_model(&format!("No model is chosen for {}: choose one in Settings, AI and agents.", provider.name))); }
    let key = if provider.needs_key {
        // a key a job may use: the contract's test on the provider's state ("connected"; "checking" or "refused" after a new paste is not)
        let row = store.ai_provider_row(provider.slug)?.filter(|_| provider_state_is_usable(provider_state(store, provider.slug)));
        match row.and_then(|r| r.keychain_ref).as_deref().and_then(parse_ref) {
            Some(r) => Some(r),
            None => return Err(no_model(&format!("{} is not connected: paste its key in Settings, AI and agents.", provider.name))),
        }
    } else { None };
    let b = book(store.path());
    let secrets: Arc<dyn ReadSecret> = keychain.clone();
    let timeout_ms = job_timeout_ms(job);
    let adapter = build(provider.id, b.job_transport(timeout_ms), secrets, b.address(provider.slug));
    Ok(ModelChoice { provider: provider.id, provider_name: provider.name, model: agent.model, key, on_device: !provider.needs_key, timeout_ms, adapter })
}

// ---------------------------------------------------------------- HTTPS to the provider

/// HTTPS to a provider (clear HTTP to this Mac's own address only: Ollama, LM Studio, a test's fake), over the engine's reqwest and ring TLS.
/// The client is built on the first request, on the check's own thread. `pub(crate)` for the decision model's Jev (decision_model.rs).
///
/// On a rig copy nothing leaves this Mac (crate netguard, closed by `rig::apply`): the URL's host is asked of `netguard::outbound_allowed`
/// before the request (`route`), the client follows NO redirect (a provider's API answers where it is asked; a 3xx is returned to the caller
/// as the provider's answer), and every name the client itself resolves goes through `guarded_lookup` (the guard, then netguard's lookup).
/// One counter, netguard's, serves every refusal (`rig/refused-outbound`).
pub(crate) struct ProviderHttps {
    client: OnceLock<Result<reqwest::blocking::Client, String>>,
    timeout_ms: u64,
    /// rig-core R2: the book's loopback port for the key providers' hosts (`route_key_providers_to_loopback`); None on the owner's copy.
    loopback: Arc<Mutex<Option<u16>>>,
}

/// A name the provider client resolves: refused before any lookup when it is not this Mac on a rig copy, else netguard's lookup (the
/// system resolver, or a test's stub). The port is the connector's to set.
pub(crate) fn guarded_lookup(host: &str) -> Result<Vec<std::net::SocketAddr>, CoreError> {
    netguard::outbound_allowed(host)?;
    netguard::lookup(host, 0).map_err(|e| CoreError::Unexpected { reason_words: format!("{host} could not be looked up: {e}") })
}

/// reqwest's resolver seam over `guarded_lookup`.
struct GuardedResolve;

impl reqwest::dns::Resolve for GuardedResolve {
    fn resolve(&self, name: reqwest::dns::Name) -> reqwest::dns::Resolving {
        let host = name.as_str().to_string();
        Box::pin(async move {
            let addrs = guarded_lookup(&host).map_err(|e| Box::new(std::io::Error::other(e.to_string())) as Box<dyn std::error::Error + Send + Sync>)?;
            Ok(Box::new(addrs.into_iter()) as reqwest::dns::Addrs)
        })
    }
}

/// The provider client: no redirect followed, every lookup guarded, the request's bound.
fn provider_client(timeout_ms: u64) -> Result<reqwest::blocking::Client, String> {
    // ring is the TLS provider (Cargo.toml: rustls-no-provider); an Err means one is installed already, which is fine
    let _ = rustls::crypto::ring::default_provider().install_default();
    reqwest::blocking::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .dns_resolver(Arc::new(GuardedResolve))
        .timeout(Duration::from_millis(timeout_ms))
        .build()
        .map_err(|e| format!("{e:?}"))
}

impl ProviderHttps {
    /// HTTPS bounded by AI_HTTP_TIMEOUT_MS with no loopback routing: the decision model's Jev (`decision::adapters::jev::JEV_URL`).
    pub(crate) fn new() -> ProviderHttps { ProviderHttps::with(AI_HTTP_TIMEOUT_MS, Arc::new(Mutex::new(None))) }
    fn with(timeout_ms: u64, loopback: Arc<Mutex<Option<u16>>>) -> ProviderHttps { ProviderHttps { client: OnceLock::new(), timeout_ms, loopback } }

    /// Where a request goes, or why it is not sent: a key provider's host goes to the rig's loopback when one is set (`route_key_providers_to_loopback`,
    /// rig only); then the rig guard (netguard: this Mac only on a rig copy, counted); then HTTPS to any host, clear HTTP to this Mac's own
    /// address only (Ollama, LM Studio). Plain HTTP to any other host is refused.
    fn route(&self, url: &str) -> Result<String, String> {
        let port = *self.loopback.lock().unwrap_or_else(|e| e.into_inner());
        let url = port.and_then(|p| loopback_url(url, p)).unwrap_or_else(|| url.to_string());
        netguard::outbound_allowed_url(&url).map_err(|e| words_of(&e))?;
        if allowed(&url) { Ok(url) } else { Err(format!("{} is not reached over HTTPS, so it was not asked", host_of(&url))) }
    }
}

/// fix-aicore (hid-ai F2): the requests this client was asked to send to a model provider OFF this Mac since launch (a key check, a question to Jev,
/// a hosted Writer's call), served at `provider_request/count`. Counted on the address the caller asked for, before the rig's loopback routing and
/// before the guard (a rig refusal is an attempt too, and is also counted at `rig/refused-outbound`); a model on this Mac (Ollama, LM Studio, a
/// local address) is not counted. A run count in memory, not history.
static OFF_DEVICE_REQUESTS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// `provider_request/count`: see `OFF_DEVICE_REQUESTS`.
pub fn provider_request_count() -> u64 { OFF_DEVICE_REQUESTS.load(std::sync::atomic::Ordering::SeqCst) }
/// A request that leaves this Mac through a seam which rewrites its address before `ProviderHttps` sees it (the Jev loopback of the tests).
pub(crate) fn note_off_device_request() { OFF_DEVICE_REQUESTS.fetch_add(1, std::sync::atomic::Ordering::SeqCst); }

impl Transport for ProviderHttps {
    fn send(&self, request: &Request) -> Result<Response, String> {
        if !netguard::is_this_mac(&netguard::host_of_url(&request.url)) { OFF_DEVICE_REQUESTS.fetch_add(1, std::sync::atomic::Ordering::SeqCst); }   // fix-aicore F2
        let url = self.route(&request.url)?;
        let routed = (url != request.url).then(|| Request { url, ..request.clone() });
        let request = routed.as_ref().unwrap_or(request);
        let client = self.client.get_or_init(|| provider_client(self.timeout_ms))
            .as_ref().map_err(|e| format!("the app could not start its connection to the provider: {e}"))?;
        let builder = match request.method { Method::Get => client.get(&request.url), Method::Post => client.post(&request.url) };
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

    #[test]
    fn state_strings_are_json() {
        assert_eq!(json_string("a \"b\" \\ c\n"), "\"a \\\"b\\\" \\\\ c\\n\"");
        assert_eq!(json_strings(["x", "y"].into_iter()), "[\"x\",\"y\"]");
    }

    /// A stand-in for the whole network: an HTTP proxy on 127.0.0.1 that every request of the transport under test is sent through. It keeps
    /// the first line of each connection (a CONNECT names the host the request was for) and answers nothing, so nothing leaves this Mac.
    fn proxy_network() -> (u16, Arc<Mutex<Vec<String>>>) {
        use std::io::{BufRead, BufReader};
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = l.local_addr().unwrap().port();
        let seen = Arc::new(Mutex::new(Vec::new()));
        let s2 = seen.clone();
        std::thread::spawn(move || for c in l.incoming().flatten() {
            let mut line = String::new();
            let _ = BufReader::new(c).read_line(&mut line);
            s2.lock().unwrap().push(line.trim().to_string());
        });
        (port, seen)
    }

    /// A Jev judgement with its key, its consent and its model all set, every request through the proxy (the guard runs first).
    fn jev_judgement() -> (Result<contracts::DecisionResult, CoreError>, Vec<String>) {
        use contracts::{DecisionPort, DecisionProviderKind, DecisionQuestion, DecisionState};
        let (port, seen) = proxy_network();
        let _ = rustls::crypto::ring::default_provider().install_default();
        let client = reqwest::blocking::Client::builder().proxy(reqwest::Proxy::all(format!("http://127.0.0.1:{port}")).unwrap())
            .timeout(Duration::from_millis(1500)).build().map_err(|e| format!("{e:?}"));
        let transport = ProviderHttps { client: OnceLock::from(client), ..ProviderHttps::new() };
        let keys = Arc::new(secrets::MemoryKeychain::new(secrets::Namespace::rig()));
        let key = StoreSecret::execute(&*keys, SecretKind::ProviderCredential, JEV_ID, &Secret::new("FIXTURE-tsa-jev-key")).unwrap();
        let consent = Arc::new(decision::MemoryConsent::default());
        consent.give(DecisionProviderKind::Jev);
        let jev = decision::adapters::jev::JevDecision { transport: Arc::new(transport), secrets: keys, key, consent };
        let question = DecisionQuestion::Noul { id: "needs_reply".into(), prompt: "Does it wait for my reply?".into() };
        let out = jev.decide(DecisionState { text: "from: a\nsubject: Lunch?\n\nAre you free on Friday?".into(), fields: HashMap::new() }, vec![question]);
        std::thread::sleep(Duration::from_millis(200));
        let seen = seen.lock().unwrap().clone();
        (out, seen)
    }

    /// The resolver stub (netguard's `set_resolver` seam): it counts every name asked and answers 127.0.0.1, so a guard a mutant removes
    /// still reaches this Mac only.
    struct CountingResolver(Mutex<Vec<String>>);
    impl netguard::Resolve for CountingResolver {
        fn lookup(&self, host: &str, port: u16) -> std::io::Result<Vec<std::net::SocketAddr>> {
            self.0.lock().unwrap().push(host.to_string());
            Ok(vec![std::net::SocketAddr::from(([127, 0, 0, 1], port))])
        }
    }

    /// A provider on this Mac that answers every request with a redirect to `location`.
    fn redirecting_fake(location: &'static str) -> u16 {
        use std::io::{BufRead, BufReader, Write};
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = l.local_addr().unwrap().port();
        std::thread::spawn(move || for mut c in l.incoming().flatten() {
            let mut r = BufReader::new(c.try_clone().unwrap());
            loop { let mut line = String::new(); if r.read_line(&mut line).unwrap_or(0) == 0 || line.trim().is_empty() { break; } }
            let _ = write!(c, "HTTP/1.1 302 Found\r\nLocation: {location}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
        });
        port
    }

    /// ONE test, in order, because closing the process (netguard) cannot be undone:
    /// (a) the owner's copy (open process): Jev's host is reached over HTTPS, plain HTTP past this Mac is refused, a local server is
    ///     reached, a key provider keeps its address with no rig loopback and goes to 127.0.0.1 with one; a Jev judgement reaches the network
    ///     stand-in;
    /// (b) then the process is closed as `rig::apply` closes it, and the resolver replaced by the counting stub: Jev's host and every host
    ///     past this Mac are refused AND counted on netguard's one counter (`rig/refused-outbound`); a Jev judgement with key, consent and
    ///     model set reaches nothing;
    /// (c) a provider on this Mac that redirects to a name past this Mac: the redirect is NOT followed (the 302 is the answer), the name is
    ///     never looked up, and nothing is counted;
    /// (d) a name the client resolves itself is refused before the lookup.
    /// Mutants: N1 (the client follows redirects again), N2 (the resolver seam skips the guard), N3 (`route` skips the guard), J4, J5.
    #[test]
    fn the_provider_transport_reaches_past_this_mac_on_the_owners_copy_only_and_never_follows_a_redirect() {
        assert!(!netguard::is_closed(), "no test of this binary closes the process before this one");
        // the fence from the first line on: any name this test's clients resolve is recorded and answered with 127.0.0.1
        let stub = Arc::new(CountingResolver(Mutex::new(Vec::new())));
        netguard::set_resolver(stub.clone());
        let owners = ProviderHttps::new();
        let jev = decision::adapters::jev::JEV_URL;
        assert_eq!(owners.route(jev).as_deref(), Ok(jev));
        assert!(owners.route(&jev.replacen("https://", "http://", 1)).is_err(), "plain HTTP to Jev's host is refused");
        assert!(owners.route("http://example.org/v1/models").is_err());
        assert_eq!(owners.route("http://127.0.0.1:1234/v1/models").as_deref(), Ok("http://127.0.0.1:1234/v1/models"), "a local server on this Mac");
        assert_eq!(owners.route("https://api.anthropic.com/v1/models").as_deref(), Ok("https://api.anthropic.com/v1/models"), "no loopback outside a rig");
        let routed = ProviderHttps::with(AI_HTTP_TIMEOUT_MS, Arc::new(Mutex::new(Some(4242))));
        assert_eq!(routed.route("https://api.anthropic.com/v1/models").as_deref(), Ok("http://127.0.0.1:4242/v1/models"));
        let (out, seen) = jev_judgement();
        assert!(!format!("{out:?}").contains("is not this Mac"), "the owner's copy is not refused: {out:?}");
        assert!(seen.iter().any(|l| l.starts_with("CONNECT api.typesafe.ai:443")), "the owner's copy's request went out (to the stand-in): {seen:?}");

        // (b) the rig closes the process
        netguard::close_to_this_mac();
        let before = netguard::refused_count();
        let refused = owners.route(jev).unwrap_err();
        assert!(refused.contains("api.typesafe.ai is not this Mac"), "{refused}");
        assert_eq!((netguard::refused_count(), netguard::last_refused().as_deref()), (before + 1, Some("api.typesafe.ai")), "counted on netguard's one counter");
        assert!(owners.route("https://example.org/x").is_err() && owners.route("https://198.51.100.1/x").is_err());
        assert_eq!(owners.route("http://localhost:1234/v1/models").as_deref(), Ok("http://localhost:1234/v1/models"));
        assert_eq!(owners.route("http://[::1]:9/x").as_deref(), Ok("http://[::1]:9/x"));
        assert_eq!(routed.route("https://api.anthropic.com/v1/models").as_deref(), Ok("http://127.0.0.1:4242/v1/models"), "the rig loopback still works");
        let (out, seen) = jev_judgement();
        assert!(format!("{out:?}").contains("api.typesafe.ai is not this Mac"), "{out:?}");
        assert!(seen.is_empty(), "the network saw {seen:?}");

        // (c) a redirect to a name past this Mac
        const OUTSIDE: &str = "api.outside-of-this-mac.example";
        let port = redirecting_fake("https://api.outside-of-this-mac.example/v1/models");
        let counted = netguard::refused_count();
        let answer = ProviderHttps::new().send(&Request { method: Method::Get, url: format!("http://127.0.0.1:{port}/v1/models"), headers: vec![], body: Body::None });
        assert_eq!(answer.as_ref().map(|r| r.status), Ok(302), "the redirect is the answer, never followed: {answer:?}");
        assert!(stub.0.lock().unwrap().iter().all(|h| h != OUTSIDE), "the redirect's name was looked up: {:?}", stub.0.lock().unwrap());
        assert_eq!(netguard::refused_count(), counted, "nothing tried to leave");

        // (d) the client's own lookups are guarded
        let counted = netguard::refused_count();
        assert!(guarded_lookup(OUTSIDE).is_err());
        assert!(stub.0.lock().unwrap().iter().all(|h| h != OUTSIDE), "refused before the lookup");
        assert_eq!(netguard::refused_count(), counted + 1);
        assert!(guarded_lookup("localhost").is_ok() && stub.0.lock().unwrap().iter().any(|h| h == "localhost"), "this Mac's name goes to the lookup");
        let outside: Vec<String> = stub.0.lock().unwrap().iter().filter(|h| !netguard::is_this_mac(h)).cloned().collect();
        assert!(outside.is_empty(), "a name past this Mac was looked up: {outside:?}");
    }

    #[test]
    fn only_key_providers_take_a_key() {
        assert!(key_provider("anthropic").is_some() && key_provider("openai").is_some());
        assert!(key_provider("ollama").is_none() && key_provider("chatgpt").is_none() && key_provider("jev").is_none());
    }
}
