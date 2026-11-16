//! The label AI's model on the real core (lane labels-r5, round 5; docs/lanes/ai-settings-core.md "The label AI").
//!
//! Before this file nothing in production gave the label flow a `DecisionPort`: `SaveDecisionModel` answered not-built-yet,
//! `decision_model()` answered None, and only tests and the rig's `check_flow` called `set_decision_port`, so on the owner's Mac the AI
//! card never ran. Now:
//! - `SaveDecisionModel` is served on the `decision_model` row (`save`, `read`): the owner's choice (Settings, Decisions) is kept.
//! - the model the AI card uses is the SAVED DECISION MODEL, resolved AT THE MOMENT IT JUDGES (`LabelModel::port`, called by `Filing::port`): a
//!   key saved later works at the next message with no restart, a key removed refuses again at the next one. Jev and Laya are NOT rows of the
//!   providers table (`ai-providers/src/table.rs` holds openai, anthropic, gemini, openrouter, ollama, lmstudio, chatgpt): Jev has its own adapter in
//!   the decision crate (`JevDecision`, endpoint and wire written down in docs and read from the org's client), Laya the typed `DecisionPort` the shell hands over (LayaOnDevice.swift).
//! - what is missing, so the AI card cannot judge yet and says so (never a crash, never a silent "sorted by nothing"):
//!   Jev: the real core refuses `SaveProviderKey` for "jev" (no way to store its key), and there is no persisted consent (`decision::HasConsent` has
//!   an in-memory implementation only; no command or record carries the owner's agreement), so `JevDecision` refuses "you have not agreed yet to send
//!   mail to Jev; nothing was sent" and no request is made. Laya: the shell hands the core its on-device `DecisionPort` at launch (`open_rust_core_with_decision`, `RustCore::set_laya_port`).
//! - the reason is readable on `label/<id>/flow/ai_refusal`: `{"code":"ai_no_model","words":"..."}`, or null when the AI card is off or a model can judge;
//!   lane fix-aisort: `{"code":"ai_failed","words":"The AI could not judge the mail of this label. ..."}` when a model is set and its last judgement of
//!   the label failed or could not be read (`label_ai_failure.rs`).
use std::sync::{Arc, Condvar, Mutex};

use ai_providers::transport::{Request, Response, Transport};
use contracts::{decision_consent_key, CommandOutcome, CoreError, DecisionModel, DecisionPort, DecisionProviderKind, DecisionQuestion, DecisionResult, DecisionState};
use decision::adapters::jev::JevDecision;
use decision::thread_context::{chars_for_tokens, decode, fit, render, THREAD_FIELD};
use decision::HasConsent;
use secrets::ReadSecret;
use store::Store;

use crate::rust_core::Keyring;

/// The words when no decision model is chosen (or the chosen one cannot run here).
pub(crate) const NO_MODEL_WORDS: &str = "No model is set for sorting mail: choose one in Settings, Decisions.";
pub(crate) const NO_KEY_WORDS: &str = "Jev has no key: add it in Settings, Decisions.";
pub(crate) const NO_CONSENT_WORDS: &str = "Jev may not see your mail until you agree to it once: agree in Settings, Decisions.";

fn json_str(v: &str) -> String {
    let mut o = String::from("\"");
    for c in v.chars() {
        match c { '"' => o.push_str("\\\""), '\\' => o.push_str("\\\\"), c if (c as u32) < 0x20 => o.push_str(&format!("\\u{:04x}", c as u32)), c => o.push(c) }
    }
    o.push('"');
    o
}

fn unexpected(why: String) -> CoreError {
    CoreError::Unexpected { reason_words: why }
}

/// `Command::SaveDecisionModel`: keeps the row. Jev needs its key saved first (its Keychain reference is on the record).
pub(crate) fn save(store: &Store, model: DecisionModel, now_ms: i64) -> Result<CommandOutcome, CoreError> {
    contracts::check_decision_model_consent(model.provider, consent_given(store, model.provider))?;
    if model.provider == DecisionProviderKind::Jev && model.keychain.is_none() {
        return Err(CoreError::Invalid { field: "keychain".into(), reason_words: "Save the Jev key first: Jev needs it before it can decide anything".into() });
    }
    store.decision_model_put(model, now_ms).map_err(|e| unexpected(e.to_string()))?;
    Ok(CommandOutcome { undo_entry_id: None, created_id: None })
}

/// Whether the owner's stored consent covers this provider (false for one that needs none: it is never asked).
pub(crate) fn consent_given(store: &Store, provider: DecisionProviderKind) -> bool {
    decision_consent_key(provider).is_some_and(|k| store.decision_consent_given(&k).unwrap_or(false))
}

/// `Command::GiveDecisionConsent`: the owner's yes or no for a hosted provider, kept in `decision_consent`; refused for one that stays on this Mac.
pub(crate) fn give_consent(store: &Store, provider: DecisionProviderKind, given: bool, now_ms: i64) -> Result<CommandOutcome, CoreError> {
    contracts::check_give_decision_consent(provider)?;
    let key = decision_consent_key(provider).ok_or_else(|| unexpected("no consent key for the provider".into()))?;
    store.decision_consent_set(&key, given, now_ms).map_err(|e| unexpected(e.to_string()))?;
    Ok(CommandOutcome { undo_entry_id: None, created_id: None })
}

/// State `decision_consent/<jev|local|rules>`: `{"needed":bool,"given":bool}`.
pub(crate) fn consent_state(store: &Store, path: &str) -> Option<String> {
    let parts: Vec<&str> = path.split('/').collect();
    let ["decision_consent", who] = parts.as_slice() else { return None };
    let provider = match *who { "jev" => DecisionProviderKind::Jev, "local" => DecisionProviderKind::Local, "rules" => DecisionProviderKind::Rules, _ => return None };
    Some(contracts::decision_consent_json(provider, consent_given(store, provider)))
}

/// The owner's stored consent, read at each call: a withdrawal stops the very next judgement, and none is ever assumed.
struct StoredConsent(Arc<Store>);

impl HasConsent for StoredConsent {
    fn execute(&self, provider: DecisionProviderKind) -> bool {
        consent_given(&self.0, provider)
    }
}

/// `WhooshCore::decision_model`: the owner's choice, None until one was saved.
pub(crate) fn read(store: &Store) -> Option<DecisionModel> {
    match store.decision_model_get() {
        Ok(m) => m,
        Err(e) => {
            eprintln!("decision model: the saved choice could not be read: {e}");
            None
        }
    }
}

/// Jev's connection: the app's one provider transport (`ProviderHttps`, ai_settings.rs): HTTPS to `decision::adapters::jev::JEV_URL` (api.typesafe.ai,
/// POST /v1/systemone, `Authorization: Bearer <the owner's Jev key>`), bounded by AI_HTTP_TIMEOUT_MS; a hidden test copy refuses it (only this Mac is reached).
fn default_jev_transport() -> Arc<dyn Transport> {
    Arc::new(crate::ai_settings::ProviderHttps::new())
}

/// TEST SEAM: the REAL transport (`ProviderHttps`) with Jev's address swapped for a fake provider on this Mac (`http://127.0.0.1:<port><path>`), so a test
/// carries a real request through the real path without any network.
struct JevLoopback { port: u16, inner: crate::ai_settings::ProviderHttps }

impl Transport for JevLoopback {
    fn send(&self, request: &Request) -> Result<Response, String> {
        crate::ai_settings::note_off_device_request();
        let rest = request.url.split_once("://").map(|(_, r)| r).unwrap_or(&request.url);
        let path = rest.find('/').map(|k| &rest[k..]).unwrap_or("/");
        self.inner.send(&Request { url: format!("http://127.0.0.1:{}{path}", self.port), ..request.clone() })
    }
}

/// The words of a failure as the owner reads them.
/// Round 16: the shell's codes for a Laya refusal that waits for the model itself: the weights are not on this Mac
/// (`LayaOnDevice.notReadyCode`), or they did not load (`LayaOnDevice.loadFailedCode`). Sources/Features/OnDeviceAI/Contract/LayaOnDevice.swift.
pub(crate) const LAYA_NOT_READY_CODE: &str = "laya_not_ready";
pub(crate) const LAYA_LOAD_FAILED_CODE: &str = "laya_load_failed";

/// The last failure of the on-device model: its words (the card's refusal) and whether it only waits for the model to arrive.
#[derive(Clone, Debug)]
struct LayaFailure {
    words: String,
    until_model_arrives: bool,
}

/// Round 16: a refusal that only waits for the model (not on this Mac yet, or not loaded): asking again once it is there answers.
fn waits_for_the_model(e: &CoreError) -> bool {
    matches!(e, CoreError::Action { failure } if matches!(failure.code.as_deref(), Some(LAYA_NOT_READY_CODE) | Some(LAYA_LOAD_FAILED_CODE)))
}

fn failure_words(e: &CoreError) -> String {
    match e {
        CoreError::Action { failure } => failure.reason_words.clone(),
        CoreError::Invalid { reason_words, .. } | CoreError::Unexpected { reason_words } => reason_words.clone(),
        other => other.to_string(),
    }
}

/// One on-device model call at a time for the background askers (lane fix-junkgate): the label flow's worker (`Filing`'s jobs) and the junk
/// filter's late opinion (`junk_filter::late_opinions`) take a turn here around the model call itself and nothing else, so a suspect arrival
/// the label flow also asks about never has two calls in flight (one call's RAM and CPU, the owner's "beware of ram"). A reader-facing act
/// (`Filing::port`: the decision preview, `file_stored_messages`) takes no turn and never waits on it.
pub(crate) struct ModelGate {
    busy: Mutex<bool>,
    freed: Condvar,
}

/// A turn at the on-device model; the next asker goes when it is dropped.
pub(crate) struct Turn(Arc<ModelGate>);

impl ModelGate {
    pub(crate) fn new() -> Arc<ModelGate> {
        Arc::new(ModelGate { busy: Mutex::new(false), freed: Condvar::new() })
    }

    /// Waits until no other background call is in flight, then takes the turn.
    pub(crate) fn turn(self: &Arc<ModelGate>) -> Turn {
        let mut busy = self.busy.lock().unwrap_or_else(|e| e.into_inner());
        while *busy {
            busy = self.freed.wait(busy).unwrap_or_else(|e| e.into_inner());
        }
        *busy = true;
        Turn(self.clone())
    }

    /// The turn when it is free now, else None (never waits).
    pub(crate) fn try_turn(self: &Arc<ModelGate>) -> Option<Turn> {
        let mut busy = self.busy.lock().unwrap_or_else(|e| e.into_inner());
        if *busy { return None; }
        *busy = true;
        Some(Turn(self.clone()))
    }
}

impl Drop for Turn {
    fn drop(&mut self) {
        *self.0.busy.lock().unwrap_or_else(|e| e.into_inner()) = false;
        self.0.freed.notify_one();
    }
}

/// The on-device decision model (Laya, run by the shell) as the label flow's port. The shell hands the core a `DecisionPort` that answers
/// typed questions with the checkpoint's own tokenizer, window and calibration (`Sources/Features/OnDeviceAI/Contract/`), so no number about the
/// model lives here. This wrapper does two things the shell cannot: it puts the rest of the thread INTO the state text, after the body and newest
/// message first, so the shell's token cut (which keeps the start of the state) drops the OLDEST messages first and never the body; and it remembers
/// the last failure (weights not on this Mac, the runtime missing...) so `label/<id>/flow/ai_refusal` can say why.
struct LayaPort {
    inner: Arc<dyn DecisionPort>,
    failure: Arc<Mutex<Option<LayaFailure>>>,
    /// Round 16: set when an answer came after a remembered failure (the weights arrived, the runtime loaded): the filing worker then asks the
    /// mails that waited meanwhile, in the background (`Filing::start`, `LabelModel::take_recovered`).
    recovered: Arc<std::sync::atomic::AtomicBool>,
    /// Some for the label flow's worker: the call takes its turn at the shared gate (`ModelGate`); None for a reader-facing act, and for the
    /// junk filter, which takes its turn itself (`LocalOpinion`).
    gate: Option<Arc<ModelGate>>,
    /// Lane fix-aisort round 8: the store (the judge marker, `label_ai_failure::note_judge`) and the reason the shell's Labels judge fell back
    /// to Laya (`fallback:<reason>` on the result's checkpoint), read by `LabelModel::state`.
    store: Arc<Store>,
    apple_note: Arc<Mutex<Option<String>>>,
}

/// Lane fix-aisort round 8: the checkpoint of a Labels result Apple's on-device model answered (Swift `LabelJudgeRouter.appleCheckpoint`).
pub(crate) const APPLE_CHECKPOINT: &str = "apple-on-device";
/// The prefix of the checkpoint of a Labels result Laya answered because Apple's model was not available (Swift `LabelJudgeRouter.fallbackPrefix`).
pub(crate) const FALLBACK_PREFIX: &str = "fallback:";
/// Lane decided-model (SPEC A133): the prefix of the checkpoint of an answer by a catalogue model other than Laya, "model:<catalogue id>"
/// (Swift `LabelReaderPort.modelCheckpointPrefix`). The id is the judge (`label_ai_failure::note_judge`), and the mail's decider is
/// `DecidedBy::Model { model: id }`.
pub(crate) const MODEL_CHECKPOINT_PREFIX: &str = "model:";
/// The code and sentence of `label/<id>/flow/ai_refusal` when the Labels judge fell back to Laya because Apple Intelligence is turned off.
pub(crate) const AI_NEEDS_APPLE_CODE: &str = "ai_needs_apple";
pub(crate) const AI_NEEDS_APPLE_WORDS: &str = "Sorting mail into labels needs Apple Intelligence: turn it on in System Settings, then Apple Intelligence and Siri.";

/// The state as Laya reads it: the thread moved from its field into the text, cut to the room the on-device window leaves after the body (a pessimistic
/// character estimate, `ON_DEVICE_CONTEXT_TOKENS` over `ON_DEVICE_CHARS_PER_TOKEN_FLOOR`; the shell then cuts to the checkpoint's own length).
pub(crate) fn state_for_laya(mut state: DecisionState) -> DecisionState {
    let Some(field) = state.fields.remove(THREAD_FIELD) else { return state };
    let used = state.text.chars().count() + state.fields.values().map(|v| v.chars().count()).sum::<usize>();
    let room = chars_for_tokens(decision::tuning::on_device_context_tokens()).saturating_sub(used);
    let thread = render(&fit(&decode(&field), room));
    if !thread.is_empty() {
        state.text.push_str("\n\n");
        state.text.push_str(&thread);
    }
    state
}

impl DecisionPort for LayaPort {
    fn provider(&self) -> DecisionProviderKind {
        DecisionProviderKind::Local
    }

    fn decide(&self, state: DecisionState, questions: Vec<DecisionQuestion>) -> Result<DecisionResult, CoreError> {
        let state = state_for_laya(state);
        // round 8: a per-label Labels question in this call (the judge that answered it is read from the result)
        let asks_labels = questions.iter().any(|q| matches!(q, DecisionQuestion::Noul { id, .. } if id.starts_with(&format!("{}{}", decision::LABELS_MODEL_ID, decision::decide::LABEL_QUESTION_SEP))));
        let turn = self.gate.as_ref().map(|g| g.turn());
        let out = self.inner.decide(state, questions);
        drop(turn);
        // round 12 (main110's --slice-aisort: `ai_refusal {'code': 'ai_no_model', 'words': 'unsafe'}`): a refusal that belongs to ONE mail
        // (`decision::decide::mail_refusal`, the mail is set aside) is not the model's failure: it never becomes the label's refusal, and its
        // reason id is never shown as words. Only a failure of the model itself is remembered; any answer clears it.
        match &out {
            Err(e) if decision::decide::mail_refusal(e).is_some() => {}
            _ => {
                let mut kept = self.failure.lock().unwrap_or_else(|e| e.into_inner());
                if out.is_ok() && kept.is_some() {
                    self.recovered.store(true, std::sync::atomic::Ordering::SeqCst);
                }
                *kept = out.as_ref().err().map(|e| LayaFailure { words: failure_words(e), until_model_arrives: waits_for_the_model(e) });
            }
        }
        if let Ok(r) = out.as_ref() {
            let checkpoint = r.checkpoint.as_deref().unwrap_or("");
            // lane decided-model: an answer by a catalogue model other than Laya names it, whatever question it answered
            if let Some(id) = checkpoint.strip_prefix(MODEL_CHECKPOINT_PREFIX) {
                if asks_labels {
                    *self.apple_note.lock().unwrap_or_else(|e| e.into_inner()) = None;
                }
                if !id.is_empty() {
                    crate::label_ai_failure::note_judge(&self.store, id);
                }
            } else if asks_labels {
                let judge = if checkpoint == APPLE_CHECKPOINT { "apple" } else { "laya" };
                *self.apple_note.lock().unwrap_or_else(|e| e.into_inner()) = checkpoint.strip_prefix(FALLBACK_PREFIX).map(str::to_string);
                crate::label_ai_failure::note_judge(&self.store, judge);
            } else if checkpoint != APPLE_CHECKPOINT && crate::label_ai_failure::judged_by_model(&self.store) {
                // lead's ruling: the marker goes back to Laya at Laya's next answer, whatever it answered (an Apple mark stays: Apple's
                // model answers the Labels question alone, and Laya answers the others beside it)
                crate::label_ai_failure::note_judge(&self.store, "laya");
            }
        }
        out
    }
}

/// Where the label AI gets its model, asked afresh at every judgement.
pub(crate) struct LabelModel {
    store: Arc<Store>,
    keychain: Arc<dyn Keyring>,
    /// The on-device decision model the shell handed over; None until it does (or when the shell takes it back).
    laya: Mutex<Option<Arc<dyn DecisionPort>>>,
    /// Why the last on-device judgement failed, in the shell's words; None after a success.
    laya_failure: Arc<Mutex<Option<LayaFailure>>>,
    /// Round 16: an answer came after a remembered Laya failure (`LayaPort::decide`); the filing worker takes it once and asks the waiting mails.
    recovered: Arc<std::sync::atomic::AtomicBool>,
    /// What a Jev request goes out over: `default_jev_transport` (the real HTTPS), or a test's own.
    jev_transport: Mutex<Arc<dyn Transport>>,
    /// One on-device call at a time for the label flow's worker and the junk filter (`ModelGate`).
    gate: Arc<ModelGate>,
    /// Round 8: why the shell's Labels judge last fell back to Laya (an `AppleReason` raw value); None when Apple's model judged.
    apple_note: Arc<Mutex<Option<String>>>,
}

impl LabelModel {
    pub(crate) fn new(store: Arc<Store>, keychain: Arc<dyn Keyring>) -> LabelModel {
        LabelModel {
            store, keychain, laya: Mutex::new(None), laya_failure: Arc::new(Mutex::new(None)), recovered: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            jev_transport: Mutex::new(default_jev_transport()), gate: ModelGate::new(), apple_note: Arc::new(Mutex::new(None)),
        }
    }

    /// Round 16: the remembered Laya failure goes (the owner saved the decision model again, as the shell does when Laya's weights arrive):
    /// the next judgement asks the model afresh.
    pub(crate) fn clear_laya_failure(&self) {
        *self.laya_failure.lock().unwrap_or_else(|e| e.into_inner()) = None;
    }

    /// Round 16: true once after an answer followed a remembered failure (the model arrived): the waiting mails are asked now.
    pub(crate) fn take_recovered(&self) -> bool {
        self.recovered.swap(false, std::sync::atomic::Ordering::SeqCst)
    }

    /// Round 16 (main114, the owner's "download the model, press Re-run, and the AI still judges nothing"): whether the AI may be asked now.
    /// The refusal's words still say why on the card, but a remembered Laya failure that only waits for the model (its weights were not on this
    /// Mac, or they did not load) never stops the model from being asked again: once the weights are there the next question answers, and
    /// that answer clears it. Any other refusal (no model chosen, rules only, Jev without its key or consent, a Laya failure of another kind)
    /// blocks as before.
    pub(crate) fn blocks_asking(&self) -> bool {
        let Some(refusal) = self.refusal() else { return false };
        let local = self.chosen().is_some_and(|m| m.provider == DecisionProviderKind::Local) && self.laya.lock().unwrap_or_else(|e| e.into_inner()).is_some();
        let waits = self.laya_failure.lock().unwrap_or_else(|e| e.into_inner()).as_ref().is_some_and(|f| f.until_model_arrives && f.words == refusal.1);
        !(local && waits)
    }

    /// The shell hands the core its on-device decision model (Laya); None takes it back.
    pub(crate) fn set_laya(&self, port: Option<Arc<dyn DecisionPort>>) {
        *self.laya_failure.lock().unwrap_or_else(|e| e.into_inner()) = None;
        *self.laya.lock().unwrap_or_else(|e| e.into_inner()) = port;
    }

    /// Points Jev's real transport at a fake provider on this Mac (tests only).
    pub(crate) fn set_jev_loopback(&self, port: u16) {
        *self.jev_transport.lock().unwrap_or_else(|e| e.into_inner()) = Arc::new(JevLoopback { port, inner: crate::ai_settings::ProviderHttps::new() });
    }

    /// The transport a Jev request goes out over (tests; production keeps the default).
    pub(crate) fn set_jev_transport(&self, t: Arc<dyn Transport>) {
        *self.jev_transport.lock().unwrap_or_else(|e| e.into_inner()) = t;
    }

    /// The typed Laya `DecisionPort` the shell handed over (`open_rust_core_with_decision`, LayaOnDevice.swift): the only on-device hand-over.
    /// `gated`: the call takes its turn at the shared gate (the label flow's worker); false for a reader-facing act and for the junk filter.
    fn laya_port(&self, gated: bool) -> Option<Arc<dyn DecisionPort>> {
        let inner = self.laya.lock().unwrap_or_else(|e| e.into_inner()).clone()?;
        Some(Arc::new(LayaPort {
            inner, failure: self.laya_failure.clone(), recovered: self.recovered.clone(), gate: gated.then(|| self.gate.clone()), store: self.store.clone(),
            apple_note: self.apple_note.clone(),
        }))
    }

    /// The decision model in force: the owner's saved choice, else Local (the shipped engine, docs/DECISION-MODEL.md: Laya on this Mac, nothing leaves)
    /// when the shell handed one over. The pane's choice reaches the row through `SaveDecisionModel`.
    fn chosen(&self) -> Option<DecisionModel> {
        read(&self.store).or_else(|| {
            self.laya.lock().unwrap_or_else(|e| e.into_inner()).as_ref()?;
            Some(DecisionModel { provider: DecisionProviderKind::Local, model: Some("laya".into()), keychain: None, decisions_enabled: vec![], question_words: Default::default(), last_run_ms: None })
        })
    }

    /// The Jev key is in the secret store under the row's reference.
    fn jev_key_present(&self, model: &DecisionModel) -> bool {
        let secrets: Arc<dyn ReadSecret> = self.keychain.clone();
        model.keychain.as_ref().is_some_and(|r| secrets.execute(r).is_ok())
    }

    /// The port for this judgement, or None when the decision model in force cannot judge (none chosen, rules only, Laya not handed over, Jev without its key).
    /// A reader-facing act's port: the on-device call takes no turn at the gate, so it never waits behind a background call.
    pub(crate) fn port(&self) -> Option<Arc<dyn DecisionPort>> {
        self.port_for(false)
    }

    /// The label flow worker's port: the same choice, the on-device call taking its turn at the gate shared with the junk filter.
    pub(crate) fn queued_port(&self) -> Option<Arc<dyn DecisionPort>> {
        self.port_for(true)
    }

    /// The junk filter's on-device model and the gate it takes its turns at: the shell's Laya whenever the model in force is Laya OR a hosted
    /// one (Jev): the label flow's choice of a hosted model never takes the junk filter's on-device opinion away, and the junk question never
    /// goes to the hosted one. None when Laya was not handed over or the owner chose rules only.
    pub(crate) fn on_device_for_junk(&self) -> Option<(Arc<dyn DecisionPort>, Arc<ModelGate>)> {
        let model = self.chosen()?;
        match model.provider {
            DecisionProviderKind::Rules => None,
            _ => Some((self.laya_port(false)?, self.gate.clone())),
        }
    }

    /// The on-device model and its gate, whatever the saved choice (the app set a hosted port for the label flow: the junk filter keeps Laya).
    pub(crate) fn on_device(&self) -> Option<(Arc<dyn DecisionPort>, Arc<ModelGate>)> {
        Some((self.laya_port(false)?, self.gate.clone()))
    }

    fn port_for(&self, gated: bool) -> Option<Arc<dyn DecisionPort>> {
        let model = self.chosen()?;
        match model.provider {
            DecisionProviderKind::Jev if self.jev_key_present(&model) => {
                let secrets: Arc<dyn ReadSecret> = self.keychain.clone();
                Some(Arc::new(JevDecision { transport: self.jev_transport.lock().unwrap_or_else(|e| e.into_inner()).clone(), secrets, key: model.keychain?, consent: Arc::new(StoredConsent(self.store.clone())) }))
            }
            DecisionProviderKind::Local => self.laya_port(gated),
            _ => None,
        }
    }

    /// `(code, words)` of why the AI card cannot judge now; None when it can. Laya with its port is told only after a judgement failed (its words:
    /// the weights are not on this Mac...). A Jev port exists with its key but has no consent yet, so it is told too.
    pub(crate) fn refusal(&self) -> Option<(String, String)> {
        let code = crate::ai_settings::AI_NO_MODEL_CODE.to_string();
        let words = match self.chosen() {
            Some(m) if m.provider == DecisionProviderKind::Local && self.laya.lock().unwrap_or_else(|e| e.into_inner()).is_some() => {
                return self.laya_failure.lock().unwrap_or_else(|e| e.into_inner()).as_ref().map(|f| (code, f.words.clone()));
            }
            // Jev judges when its key is stored AND the owner's consent is on record: the refusal names the one that is missing, and says nothing otherwise
            Some(m) if m.provider == DecisionProviderKind::Jev => {
                if !self.jev_key_present(&m) { NO_KEY_WORDS } else if !consent_given(&self.store, DecisionProviderKind::Jev) { NO_CONSENT_WORDS } else { return None; }
            }
            _ => NO_MODEL_WORDS,
        };
        Some((code, words.into()))
    }

    /// State `label/<id>/flow/ai_refusal`: the refusal of a label whose AI card is on, else `null`; None for another path or an unknown label.
    pub(crate) fn state(&self, path: &str) -> Option<String> {
        let parts: Vec<&str> = path.split('/').collect();
        let ["label", id, "flow", "ai_refusal"] = parts.as_slice() else { return None };
        let label = self.store.labels_full().ok()?.into_iter().find(|l| l.id == *id)?;
        // fix-aisort: the last judgement of this label failed or could not be read, by a model that RAN (the words are not "no model"): one
        // catalogue sentence (the model's own words are in core.log), never a silent "0 accepted". A refusal of the saved choice that names
        // its own cause (Laya's words, Jev's key or consent) is said first; the general "no model" gives way to it (the app's or the check's
        // port judged, not the saved choice).
        let under = crate::label_ai_failure::under_trust_of(self.store.path(), id);
        let ran_and_failed = label.ai_enabled
            && crate::label_ai_failure::failure_of(self.store.path(), id).is_some_and(|w| w != crate::label_ai_failure::cut_words(NO_MODEL_WORDS));
        Some(match self.refusal().filter(|_| label.ai_enabled) {
            Some((code, words)) if !((ran_and_failed || under.is_some()) && words == NO_MODEL_WORDS) => format!("{{\"code\":{},\"words\":{}}}", json_str(&code), json_str(&words)),
            _ if ran_and_failed => {
                format!("{{\"code\":{},\"words\":{}}}", json_str(crate::label_ai_failure::AI_FAILED_CODE), json_str(crate::label_ai_failure::AI_FAILED_WORDS))
            }
            // fix-aisort round 2: the AI judged this label's mail and no answer reached the trust level: the counts, so the card says why "0 accepted"
            // round 8: the Labels judge fell back to Laya because Apple Intelligence is turned off: the one thing the owner can switch on
            _ if label.ai_enabled && self.apple_note.lock().unwrap_or_else(|e| e.into_inner()).as_deref() == Some("not_enabled") => {
                format!("{{\"code\":{},\"words\":{}}}", json_str(AI_NEEDS_APPLE_CODE), json_str(AI_NEEDS_APPLE_WORDS))
            }
            _ if label.ai_enabled && under.is_some() => {
                let (judged, highest) = under.unwrap_or((0, 0.0));
                let trust = decision::tuning::model_trust_label().unwrap_or(1.0);
                let pct = |v: f64| (v * 100.0).round().clamp(0.0, 100.0) as u32;
                format!(
                    "{{\"code\":{},\"highest_percent\":{},\"judged\":{judged},\"trust_percent\":{},\"words\":{}}}",
                    json_str(crate::label_ai_failure::AI_UNDER_TRUST_CODE), pct(highest), pct(trust), json_str(crate::label_ai_failure::AI_UNDER_TRUST_WORDS)
                )
            }
            Some((code, words)) => format!("{{\"code\":{},\"words\":{}}}", json_str(&code), json_str(&words)),
            _ => "null".into(),
        })
    }
}
