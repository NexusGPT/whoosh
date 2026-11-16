//! junk-r2: the local spam filter (core/crates/junk) wired into the real core, as docs/lanes/core-junk.md "## Wiring for the engine lane"
//! plans it. Four doors, one file:
//! - `store_arrivals`: the sync's live arrivals on an Inbox-role folder are scored before they are stored. A score at or above the move
//!   cut-off sends the message to the account's Junk-role folder (the local row leaves the inbox in the SAME transaction that stores it,
//!   with a `pending_action` row as the ledger, then the IMAP MOVE on the sync's own adapter); at or above the review cut-off it is kept
//!   and marked `suspect`; below, only the score is kept. History, the first screen and a re-read of old mail are never scored: the old
//!   mail of a new account must never be mass-moved (the history teaches instead, `bootstrap`).
//! - `bootstrap`: once per account, after its history is stored: the server's Junk folder learned as junk, its inbox as not junk.
//! - `learn`: the owner's lesson (MarkSpam = junk, the not-junk action = not junk). `junk_learned` is written FIRST, so a repeated command
//!   never trains twice; a changed mind takes the old lesson out (`relearn`).
//! - `mark_not_spam`: the not-junk action (no contract command yet: docs/lanes/junk-r2.md holds its exact text and the core's hunk).
//!
//! The on-device model (Laya, handed over by the shell through the label flow's `DecisionPort`, `Filing::port`) is a fourth signal for the
//! SUSPECT arrivals only (score from the other signals at or above the review cut-off and below the move cut-off, `Thresholds::in_suspect_band`),
//! never every arrival. It never delays storage: the arrival is scored without it and stored; the one question per suspect message is asked
//! afterwards, on a background task, one at a time (`late_opinions`); a late opinion re-scores that one message (`apply_late_opinion`) and
//! can move it. Absent (no model on this Mac ready, a hosted provider, a failure) changes nothing. Only a provider that answers as `Local`
//! (on this Mac) is ever asked: the mail text never goes to a hosted one for the junk filter.
//!
//! ONE `Filter` per store behind a mutex, its model read from the store once and written back after each batch, never after each message.
//! The two cut-offs come from the ledger through `Thresholds::new`.
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};

use actions::{MessageRef, WriteBack};
use contracts::{BlockTarget, CommandOutcome, CoreError, DecisionAnswer, DecisionPort, DecisionProviderKind, DecisionQuestion, DecisionState};
use junk::{Context, Decision, Filter, HardRule, Label, Model, ModelOpinion, Thresholds, Verdict};
use store::{JunkLesson, JunkState, JunkText, NewMessage, NewPending, Store, StoreError, INBOX_FOLDER};
use sync_imap::{FetchedMessage, FolderRole, ImapAdapter, ImapMailAdapter};

use crate::consts::SYNC_BACKOFF_MAX_MS;
use crate::decision_model::{ModelGate, Turn};
use crate::imap_source::new_message;
use crate::sync::SyncDeps;
use crate::writeback::{pending_rows, role_folder};

/// The local folder word of the Spam mailbox (actions::constants): on the server, the account's Junk-role folder.
const SPAM: &str = actions::constants::SPAM_FOLDER;
/// The hidden check's mutant `junk-never-moves` (rig only: read where the sync speaks clear text on the loopback, which only the rig copy
/// and the in-process tests do): a message the filter would move is kept with its score.
pub const NEVER_MOVES_MUTANT: &str = "junk-never-moves";

// ---------------------------------------------------------------- the one filter

type Shared = Arc<Mutex<Filter>>;

fn registry() -> &'static Mutex<HashMap<PathBuf, Shared>> {
    static R: OnceLock<Mutex<HashMap<PathBuf, Shared>>> = OnceLock::new();
    R.get_or_init(|| Mutex::new(HashMap::new()))
}

/// junk-r3: the person's own cut-offs for this store's filter, validated (`Thresholds::new`: 0 < review <= move <= 1). Held for the session;
/// the store keeps no column for them yet (docs/lanes/body-r1.md "Round after the restart" names the migration and the contract command).
pub fn set_thresholds(store: &Store, move_at: f64, review_at: f64) -> Result<(), junk::ThresholdError> {
    let t = Thresholds::new(move_at, review_at)?;
    lock(&filter_for(store)).thresholds = t;
    Ok(())
}

/// The two cut-offs from the ledger (JUNK_MOVE_THRESHOLD, JUNK_REVIEW_THRESHOLD in docs/CONSTANTS.md, through the junk crate's build).
pub fn thresholds() -> Thresholds {
    Thresholds::new(junk::tuning::JUNK_MOVE_THRESHOLD, junk::tuning::JUNK_REVIEW_THRESHOLD).unwrap_or_else(|e| {
        eprintln!("junk: the ledger's cut-offs are refused ({e}); the filter keeps its defaults");
        Thresholds::default()
    })
}

/// The filter of this store: its model read from the store the first time, then held. A stored table that cannot be read is said so
/// and replaced by an empty one (the filter then has no opinion until it learns again).
pub fn filter_for(store: &Store) -> Shared {
    let key = store.path().to_path_buf();
    let mut g = registry().lock().unwrap_or_else(|e| e.into_inner());
    g.entry(key).or_insert_with(|| {
        let model = match store.junk_model() {
            Ok(Some(stored)) => Model::from_bytes(&stored.bytes).unwrap_or_else(|e| {
                eprintln!("junk: the stored junk table is not readable ({e}); learning starts again");
                Model::new()
            }),
            Ok(None) => Model::new(),
            Err(e) => {
                eprintln!("junk: the stored junk table could not be read ({e}); learning starts again");
                Model::new()
            }
        };
        Arc::new(Mutex::new(Filter::new(model, thresholds())))
    }).clone()
}

fn lock(shared: &Shared) -> MutexGuard<'_, Filter> { shared.lock().unwrap_or_else(|e| e.into_inner()) }

fn save(store: &Store, filter: &Filter) {
    let (junk, good) = filter.model.trained();
    if let Err(e) = store.junk_model_save(filter.model.to_bytes(), i64::from(junk), i64::from(good), crate::rust_core::now_ms()) {
        eprintln!("junk: the learned table was not saved ({e}); the lessons since the last save are kept in memory only");
    }
}

// ---------------------------------------------------------------- what the filter reads

/// A fetched message as the filter reads it: the decoded name, the lower-case address, the preview as the text (the body is fetched
/// later), the HTML the first body bytes hold (junk-r3: the link rule reads it; None when the slice reaches no HTML part), and the two
/// headers sync-imap reads from the header block.
pub fn from_fetched(f: &FetchedMessage) -> junk::Message {
    let m = &f.message;
    junk::Message {
        from_name: Some(m.sender_name.trim().to_string()).filter(|n| !n.is_empty()),
        from_email: m.from_address.trim().to_ascii_lowercase(),
        reply_to: f.reply_to.clone(),
        subject: m.subject.clone(),
        text: m.preview.clone(),
        html: f.html_head.clone(),
        authentication_results: f.authentication_results.clone(),
    }
}

/// A stored message as the filter reads it (the bootstrap and a lesson): the row keeps no Authentication-Results and no Reply-To.
pub fn from_stored(t: &JunkText) -> junk::Message {
    junk::Message {
        from_name: Some(t.sender_name.trim().to_string()).filter(|n| !n.is_empty()),
        from_email: t.from_address.trim().to_ascii_lowercase(),
        reply_to: vec![],
        subject: t.subject.clone(),
        text: t.text.clone(),
        html: t.html.clone(),
        authentication_results: vec![],
    }
}

/// What the filter knows of the owner: the blocked senders and domains (Block sender, and a suggestion's domain block, both land in the
/// store's block list) as certain rules, and the contacts for the display-name rule.
pub fn context(store: &Store) -> Context {
    let hard_rules = match store.blocks() {
        Ok(blocks) => blocks.into_iter().map(|b| match b.target {
            BlockTarget::Sender { address } => HardRule::block_sender(&address),
            BlockTarget::Domain { domain } => HardRule::block_domain(&domain),
        }).collect(),
        Err(e) => { eprintln!("junk: the block list could not be read ({e}); arrivals are scored without it"); vec![] }
    };
    let contacts = match store.write(|ctx| ctx.contacts_all()) {
        Ok(all) => all.into_iter().map(|c| junk::Contact { name: c.name, emails: c.addresses.into_iter().map(|a| a.to_ascii_lowercase()).collect() }).collect(),
        Err(e) => { eprintln!("junk: the contacts could not be read ({e}); arrivals are scored without them"); vec![] }
    };
    Context { contacts, hard_rules }
}

/// An arrival the owner has already acted on elsewhere (read or flagged on another client) is never scored.
pub fn eligible(f: &FetchedMessage) -> bool {
    !f.flags.iter().any(|x| x.eq_ignore_ascii_case("\\Seen") || x.eq_ignore_ascii_case("\\Flagged"))
}

/// What happens to one scored arrival.
#[derive(Debug, Clone, PartialEq)]
pub struct Screened {
    pub message_id: String,
    pub uid: u32,
    /// junk-r3: the sender's address, lower-case (the correspondent guard reads it)
    pub from: String,
    pub verdict: Verdict,
    /// What the store records: None = kept with its score only.
    pub state: Option<JunkState>,
}

/// The decision for each eligible arrival (pure: nothing is written). `has_junk_folder` false = the account has no Junk folder: a message
/// the filter would move is only suspected. `never_moves` = the hidden check's mutant.
pub fn decide(filter: &Filter, ctx: &Context, arrivals: &[&FetchedMessage], has_junk_folder: bool, never_moves: bool) -> Vec<Screened> {
    arrivals.iter().filter(|f| eligible(f)).map(|f| {
        let verdict = filter.assess(&from_fetched(f), ctx);
        let state = match verdict.decision {
            Decision::Move if never_moves => None,
            Decision::Move if has_junk_folder => Some(JunkState::Moved),
            Decision::Move | Decision::Suspect => Some(JunkState::Suspect),
            Decision::Keep => None,
        };
        Screened { message_id: f.message.id.clone(), uid: f.uid, from: f.message.from_address.trim().to_ascii_lowercase(), verdict, state }
    }).collect()
}

/// junk-r3, the correspondent guard: a sender the owner has written to (`Store::written_to`) is never moved by the score alone. What
/// the score would move is only suspected; the owner's block (a hard rule) still moves it. A false positive on real correspondence is
/// the worst outcome of a spam filter: the owner never sees the reply they wait for.
pub fn spare_correspondents(screened: &mut [Screened], written_to: &std::collections::HashSet<String>) {
    for s in screened.iter_mut() {
        if s.state == Some(JunkState::Moved) && s.verdict.hard.is_none() && written_to.contains(&s.from) { s.state = Some(JunkState::Suspect); }
    }
}

fn never_moves(d: &SyncDeps) -> bool {
    d.plain_loopback && std::env::var("WHOOSH_CORE_MUTANT").as_deref() == Ok(NEVER_MOVES_MUTANT)
}

fn is_inbox(d: &SyncDeps, account: &str, folder: &str) -> bool {
    let named = d.roles.lock().ok().and_then(|g| g.get(account).and_then(|m| m.get(folder).copied()));
    match named { Some(role) => role == FolderRole::Inbox, None => folder.eq_ignore_ascii_case(INBOX_FOLDER) }
}

// ---------------------------------------------------------------- the arrivals

/// Stores a batch the sync fetched from `folder`. `live` names the messages that are new mail (the caller knows: a change set's new
/// messages, or on a re-read the ones above the stored position); only those, on an Inbox-role folder, are scored. Everything else is
/// stored exactly as before. A message the filter moves is stored in the Spam folder with its `pending_action` row in the same
/// transaction, then moved on the server through the sync's adapter; a refused move keeps its row for the write-back worker.
pub async fn store_arrivals(d: &SyncDeps, account: &str, adapter: &ImapAdapter, folder: &str, arrivals: Vec<FetchedMessage>, live: impl Fn(&FetchedMessage) -> bool) -> Result<(), String> {
    let candidates: Vec<&FetchedMessage> = if is_inbox(d, account, folder) { arrivals.iter().filter(|f| live(f)).collect() } else { vec![] };
    let junk_folder = role_folder(d, account, FolderRole::Junk);
    let mut screened = if candidates.is_empty() { vec![] } else {
        let ctx = context(&d.store);
        let shared = filter_for(&d.store);
        let filter = lock(&shared);
        decide(&filter, &ctx, &candidates, junk_folder.is_some(), never_moves(d))
    };
    // junk-r3: the senders the owner has written to keep their mail in the inbox, whatever the score (a block still moves)
    let senders: Vec<String> = screened.iter().filter(|s| s.state == Some(JunkState::Moved)).map(|s| s.from.clone()).collect();
    if !senders.is_empty() {
        match d.store.written_to(account, &senders) {
            Ok(written) => spare_correspondents(&mut screened, &written),
            Err(e) => { eprintln!("junk {account}: the owner's correspondents could not be read ({e}); nothing is moved by score this time"); spare_correspondents(&mut screened, &senders.into_iter().collect()); }
        }
    }
    // the suspect-band arrivals the on-device model may be asked about once they are stored (the opinion is NOT asked here: storage never waits for it)
    let late: Vec<(String, junk::Message)> = if screened.is_empty() { vec![] } else {
        let t = lock(&filter_for(&d.store)).thresholds;
        screened.iter().filter(|s| s.verdict.hard.is_none() && t.in_suspect_band(s.verdict.score))
            .filter_map(|s| candidates.iter().find(|f| f.message.id == s.message_id).map(|f| (s.message_id.clone(), from_fetched(f)))).collect()
    };
    let junk_folder_late = junk_folder.clone();
    let threads: HashMap<String, String> = arrivals.iter().map(|f| (f.message.id.clone(), f.message.thread_id.clone())).collect();
    let moving: Vec<&Screened> = screened.iter().filter(|s| s.state == Some(JunkState::Moved)).collect();
    let refs: Vec<MessageRef> = moving.iter().map(|s| MessageRef { message_id: s.message_id.clone(), thread_id: threads.get(&s.message_id).cloned().unwrap_or_default(), remote_id: Some(s.uid.to_string()), folder: folder.to_string() }).collect();
    let rows: Vec<NewPending> = if refs.is_empty() { vec![] } else { pending_rows(&WriteBack::MoveToFolder { account_id: account.to_string(), messages: refs, folder: SPAM.to_string() }) };
    let marks: Vec<(String, f64, Option<JunkState>)> = screened.iter().map(|s| (s.message_id.clone(), s.verdict.score, s.state)).collect();
    let moved_ids: Vec<String> = moving.iter().map(|s| s.message_id.clone()).collect();
    // fix-realcore round 2 (main95 s1-001 `icon@notch`: delivered, the notch listed 0 rows): the live INBOX arrivals the filter keeps in the inbox
    // are announced once stored, `ArrivalNotice` on the Inbox stream (the app's ArrivalListener reads it there: notification and notch). The real
    // core never sent it, so on the real engine no arrival ever reached the notch.
    let announce: Vec<String> = candidates.iter().map(|f| f.message.id.clone()).filter(|id| !moved_ids.contains(id)).collect();
    let uids: Vec<u32> = moving.iter().map(|s| s.uid).collect();
    let moved_ids_by_uid: Vec<(u32, String)> = moving.iter().map(|s| (s.uid, s.message_id.clone())).collect();
    // lane reply-headers (SPEC A144): each arrival's Message-ID and References, written with its row in the same transaction
    let headers: Vec<(String, store::ThreadHeaders)> = arrivals.iter().filter_map(crate::imap_source::arrival_headers).collect();
    // real-replyto (0027): and each arrival's Reply-To, in the same transaction
    let reply_to: Vec<(String, Vec<store::ReplyTo>)> = arrivals.iter().map(crate::imap_source::arrival_reply_to).collect();
    let batch: Vec<NewMessage> = arrivals.into_iter().map(new_message).collect();
    let now = crate::rust_core::now_ms();
    let due = now + SYNC_BACKOFF_MAX_MS as i64;
    let ledger = d.store.write(move |ctx| {
        ctx.insert_batch(&batch)?;
        ctx.set_thread_headers(&headers)?;
        ctx.set_reply_to(&reply_to)?;
        for (id, score, state) in &marks { ctx.junk_mark(id, *score, *state)?; }
        if !moved_ids.is_empty() { ctx.set_folder(&moved_ids, SPAM)?; }
        let mut ids = Vec::new();
        for row in &rows { ids.push(ctx.pending_action_add_due(row, now, due)?); }
        Ok(ids)
    }).map_err(|e| e.to_string())?;
    for message_id in announce {
        crate::sync::emit(&d.sinks, contracts::StreamKind::Inbox, contracts::CoreEvent::ArrivalNotice { message_id });
    }
    for s in &screened {
        eprintln!("junk {account}: {} scored {:.3} ({:?}, rules {:?}){}", s.message_id, s.verdict.score, s.verdict.decision, s.verdict.rules,
            match s.state { Some(JunkState::Moved) => " -> moved to Junk", Some(JunkState::Suspect) => " -> suspected", None => "" });
    }
    if let (Some(dest), false) = (junk_folder, uids.is_empty()) {
        match adapter.move_placed(folder, &uids, &dest).await {
            Ok(placed) => finish_move(&d.store, account, &dest, ledger, &placed, &moved_ids_by_uid),
            Err(e) => eprintln!("junk {account}: the server refused the move to {dest}, the write-back worker tries again: {e}"),
        }
    }
    if !late.is_empty() {
        if let Some(opinion) = on_device_opinion(&d.filing) {
            let adapter = d.adapters.lock().ok().and_then(|g| g.get(account).cloned());
            // fix-junkgate: the first question takes its turn at the on-device model NOW, before the label flow is kicked for the same arrivals
            // (sync.rs kicks it after this returns), so the late opinion is never queued behind the label flow's question about the same mail.
            // Only when no batch runs and no call is in flight: the turn then spans the task's start and the call, never a wait.
            let batch = ONE_AT_A_TIME.try_lock().ok();
            let first = batch.as_ref().and_then(|_| opinion.try_turn());
            tokio::spawn(late_opinions(d.store.clone(), opinion, adapter, account.to_string(), folder.to_string(), junk_folder_late, late, batch, first));
        }
    }
    Ok(())
}

/// What follows a move the server accepted: the ledger row is done, and where the server put each message (COPYUID) is kept, so the way back
/// (MarkNotSpam) names the Junk folder's own UID.
fn finish_move(store: &Store, account: &str, dest: &str, ledger: Vec<i64>, placed: &[sync_imap::MovedUid], by_uid: &[(u32, String)]) {
    if let Err(e) = store.pending_action_done(ledger) { eprintln!("junk {account}: the move to {dest} is done but still listed: {e}"); }
    // engine-act-r5: where the server put each one (COPYUID), so the way back (MarkNotSpam) names the Junk folder's own UID
    let hits: Vec<(String, sync_imap::MovedUid)> = placed.iter().filter_map(|p| by_uid.iter().find(|(u, _)| *u == p.from_uid).map(|(_, id)| (id.clone(), *p))).collect();
    let (acc, to) = (account.to_string(), dest.to_string());
    let kept = store.write(move |ctx| { for (id, p) in &hits { ctx.place_record(id, &acc, &to, p.uid_validity, p.to_uid)?; } Ok(()) });
    if let Err(e) = kept { eprintln!("junk {account}: the Junk place of a moved message was not kept ({e})"); }
}

// ---------------------------------------------------------------- the on-device opinion

/// The question the on-device model answers for one message (yes/no: junk or not).
const OPINION_PROMPT: &str = "Is this message junk mail: spam, a scam, phishing or an unwanted advert?";

/// The model that runs on this Mac: a port the app or a check set when it runs on this Mac, else the shell's Laya (decision_model.rs), also
/// when the label flow's model is a HOSTED one (the app's port, or the owner's saved Jev): the junk filter keeps its on-device opinion and
/// never sends the text of a message anywhere (`LocalOpinion::new` takes a `Local` port only). None when no model on this Mac is ready.
/// Laya's calls take their turn at the gate the label flow's worker shares (`ModelGate`): one on-device call at a time.
pub(crate) fn on_device_opinion(filing: &crate::filing_worker::Filing) -> Option<LocalOpinion> {
    let on_device = match filing.app_port() {
        Some(port) if port.provider() == DecisionProviderKind::Local => return LocalOpinion::new(port),
        Some(_) => filing.label_model().on_device(),
        None => filing.label_model().on_device_for_junk(),
    };
    let (port, gate) = on_device?;
    LocalOpinion::gated(port, gate)
}

/// The on-device model's chance that one message is junk, through the typed `DecisionPort` (the question is one yes/no; the answer's
/// probability is the yes-probability). Any failure, any other provider's answer, any number outside [0, 1]: no opinion.
pub struct LocalOpinion {
    port: Arc<dyn DecisionPort>,
    /// The gate shared with the label flow's worker (Laya): each question takes its turn around the model call only. None for a port the
    /// app or a test set.
    gate: Option<Arc<ModelGate>>,
}

impl LocalOpinion {
    /// None for a port that does not run on this Mac: its provider is not `Local`.
    pub fn new(port: Arc<dyn DecisionPort>) -> Option<LocalOpinion> {
        (port.provider() == DecisionProviderKind::Local).then(|| LocalOpinion { port, gate: None })
    }

    /// The same, each call taking its turn at `gate`.
    pub(crate) fn gated(port: Arc<dyn DecisionPort>, gate: Arc<ModelGate>) -> Option<LocalOpinion> {
        LocalOpinion::new(port).map(|o| LocalOpinion { gate: Some(gate), ..o })
    }

    /// The turn at the gate when it is free now (never waits); None without a gate.
    pub(crate) fn try_turn(&self) -> Option<Turn> {
        self.gate.as_ref()?.try_turn()
    }

    /// One question; `turn` is a turn already taken (`try_turn`), else the call waits for its own at the gate.
    fn ask(&self, msg: &junk::Message, turn: Option<Turn>) -> Option<f64> {
        let who = match &msg.from_name { Some(n) => format!("{n} <{}>", msg.from_email), None => msg.from_email.clone() };
        let state = DecisionState { text: format!("From: {who}\nSubject: {}\n\n{}", msg.subject, msg.text), fields: HashMap::new() };
        let question = DecisionQuestion::Noul { id: "junk".into(), prompt: OPINION_PROMPT.into() };
        let turn = turn.or_else(|| self.gate.as_ref().map(|g| g.turn()));
        let out = self.port.decide(state, vec![question]);
        drop(turn);
        match out {
            Ok(r) if r.answered_by == DecisionProviderKind::Local => r.answers.into_iter().find_map(|a| match a {
                DecisionAnswer::Noul { id, probability, .. } if id == "junk" && probability.is_finite() && (0.0..=1.0).contains(&probability) => Some(probability),
                _ => None,
            }),
            Ok(_) => None,
            Err(e) => { eprintln!("junk: the on-device model gave no opinion ({e:?})"); None }
        }
    }
}

impl ModelOpinion for LocalOpinion {
    fn chance_junk(&self, msg: &junk::Message) -> Option<f64> {
        self.ask(msg, None)
    }
}

/// One batch of late opinions at a time in the whole process (`late_opinions`).
static ONE_AT_A_TIME: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// The questions for the suspect messages just stored, one message at a time and one batch at a time in the whole process (a burst of mail
/// never holds more than one model call, so the RAM and the CPU stay what one call costs), each call off the sync's threads. `batch` is the
/// batch turn `store_arrivals` already took (else it is waited for here); `first` the model turn of the first question.
#[allow(clippy::too_many_arguments)]
async fn late_opinions(store: Arc<Store>, opinion: LocalOpinion, adapter: Option<Arc<ImapAdapter>>, account: String, folder: String, junk_folder: Option<String>, asks: Vec<(String, junk::Message)>, batch: Option<tokio::sync::MutexGuard<'static, ()>>, first: Option<Turn>) {
    let _batch = match batch { Some(held) => held, None => ONE_AT_A_TIME.lock().await };
    let opinion = Arc::new(opinion);
    let mut first = first;
    for (id, msg) in asks {
        let (o, turn) = (opinion.clone(), first.take());
        let chance = tokio::task::spawn_blocking(move || o.ask(&msg, turn)).await.ok().flatten();
        if let Some(p) = chance {
            apply_late_opinion(&store, adapter.as_deref(), &account, &folder, junk_folder.as_deref(), &id, p).await;
        }
    }
}

/// A late opinion for one stored message: only while it is still what the filter left it (suspected, unread, not archived, still in `folder`,
/// no lesson from the owner). The score is folded with the opinion (`Filter::with_late_opinion`; it never goes down); a message that now
/// reaches the move cut-off goes to Junk exactly as a scored arrival does (same ledger row, same server move), unless the owner has written
/// to the sender or the account has no Junk folder: then it stays suspected with its higher score.
pub async fn apply_late_opinion(store: &Store, adapter: Option<&ImapAdapter>, account: &str, folder: &str, junk_folder: Option<&str>, id: &str, chance: f64) {
    let Ok(Some((Some(score), Some(JunkState::Suspect)))) = store.junk_of(id) else { return };
    let Ok(Some(m)) = store.message(id) else { return };
    if m.read || m.archived || m.folder != folder || matches!(store.junk_lesson(id), Ok(Some(_))) { return; }
    let (new, decision) = lock(&filter_for(store)).with_late_opinion(score, Some(chance));
    if new <= score { return; }
    let from = m.from_address.trim().to_ascii_lowercase();
    let correspondent = store.written_to(account, std::slice::from_ref(&from)).map(|w| w.contains(&from)).unwrap_or(true);
    let uid = store.junk_remote_id(id).ok().flatten().and_then(|r| r.parse::<u32>().ok());
    let plan = match (decision, junk_folder, uid, correspondent) { (Decision::Move, Some(dest), Some(uid), false) => Some((dest, uid)), _ => None };
    let Some((dest, uid)) = plan else {
        if let Err(e) = store.write({ let id = id.to_string(); move |ctx| { ctx.junk_mark(&id, new, Some(JunkState::Suspect))?; Ok(()) } }) { eprintln!("junk {account}: the late opinion for {id} was not kept ({e})"); }
        return;
    };
    let rows = pending_rows(&WriteBack::MoveToFolder { account_id: account.to_string(), messages: vec![MessageRef { message_id: id.to_string(), thread_id: m.thread_id.clone(), remote_id: Some(uid.to_string()), folder: folder.to_string() }], folder: SPAM.to_string() });
    let (now, mid) = (crate::rust_core::now_ms(), id.to_string());
    let due = now + SYNC_BACKOFF_MAX_MS as i64;
    let ledger = store.write(move |ctx| {
        ctx.junk_mark(&mid, new, Some(JunkState::Moved))?;
        ctx.set_folder(std::slice::from_ref(&mid), SPAM)?;
        let mut ids = Vec::new();
        for row in &rows { ids.push(ctx.pending_action_add_due(row, now, due)?); }
        Ok(ids)
    });
    let ledger = match ledger { Ok(l) => l, Err(e) => { eprintln!("junk {account}: the late move of {id} was not kept ({e})"); return; } };
    eprintln!("junk {account}: {id} re-scored {new:.3} by the on-device opinion -> moved to Junk");
    if let Some(adapter) = adapter {
        match adapter.move_placed(folder, &[uid], dest).await {
            Ok(placed) => finish_move(store, account, dest, ledger, &placed, &[(uid, id.to_string())]),
            Err(e) => eprintln!("junk {account}: the server refused the move to {dest}, the write-back worker tries again: {e}"),
        }
    }
}

// ---------------------------------------------------------------- first run

/// Once per account, after its history is stored: the Junk folder (the server's, and the local Spam word) learned as junk and the inbox
/// as not junk, newest first, at most BOOTSTRAP_MAX_PER_CLASS of each. Recorded in `junk_bootstrap`; a second call changes nothing.
pub fn bootstrap(d: &SyncDeps, account: &str) -> Option<(usize, usize)> {
    match d.store.junk_bootstrapped(account) {
        Ok(false) => {}
        Ok(true) => return None,
        Err(e) => { eprintln!("junk {account}: the first-run record could not be read ({e}); no training"); return None; }
    }
    let max = junk::tuning::BOOTSTRAP_MAX_PER_CLASS;
    let mut junk_folders: Vec<String> = role_folder(d, account, FolderRole::Junk).into_iter().collect();
    junk_folders.push(SPAM.to_string());
    let read = |folders: &[String]| d.store.junk_corpus(account, folders, max).map(|t| t.iter().map(from_stored).collect::<Vec<_>>());
    let (junk_msgs, good_msgs) = match (read(&junk_folders), read(&[INBOX_FOLDER.to_string()])) {
        (Ok(j), Ok(g)) => (j, g),
        (Err(e), _) | (_, Err(e)) => { eprintln!("junk {account}: the folders to learn from could not be read ({e}); no training"); return None; }
    };
    let shared = filter_for(&d.store);
    let report = {
        let mut f = lock(&shared);
        let r = f.bootstrap(&junk_msgs, &good_msgs);
        save(&d.store, &f);
        r
    };
    if let Err(e) = d.store.junk_bootstrap_record(account, report.junk_learned as i64, report.good_learned as i64, crate::rust_core::now_ms()) {
        eprintln!("junk {account}: the first-run training is not recorded ({e}); it runs again at the next sync");
    }
    eprintln!("junk {account}: first-run training learned {} junk and {} not-junk messages", report.junk_learned, report.good_learned);
    Some((report.junk_learned, report.good_learned))
}

// ---------------------------------------------------------------- the owner's lessons

fn label(l: JunkLesson) -> Label { match l { JunkLesson::Junk => Label::Junk, JunkLesson::NotJunk => Label::NotJunk } }

/// The owner's lesson on these messages: `junk_learned` first (one transaction), then the model, then ONE save. A message already
/// learned the same way is not trained again; one learned the other way is relearned. Answers how many lessons changed the model.
pub fn learn(store: &Store, ids: &[String], lesson: JunkLesson) -> Result<usize, StoreError> {
    let texts = store.junk_texts(ids)?;
    if texts.is_empty() { return Ok(0); }
    let keys: Vec<String> = texts.iter().map(|t| t.message_id.clone()).collect();
    let before: Vec<Option<JunkLesson>> = store.write(move |ctx| keys.iter().map(|id| ctx.junk_lesson_put(id, lesson)).collect())?;
    let shared = filter_for(store);
    let mut f = lock(&shared);
    let mut changed = 0;
    for (t, prior) in texts.iter().zip(before) {
        let msg = from_stored(t);
        match prior {
            Some(p) if p == lesson => continue,
            Some(p) => f.relearn(&msg, label(p), label(lesson)),
            None => f.learn(&msg, label(lesson)),
        }
        changed += 1;
    }
    if changed > 0 { save(store, &f); }
    Ok(changed)
}

/// The not-junk action (contracts has no `MarkNotSpam` yet; docs/lanes/junk-r2.md holds its text and the core's hunk). The lesson is
/// learned, the mark goes, and the message comes back to the inbox: a row the sync stored from the server's Junk folder is moved back on
/// the server too (a `pending_action` row, applied by the write-back worker); a row that only carries the local Spam word keeps the UID
/// it had in the inbox, so the server is not asked. An unknown id is refused before anything is written.
pub fn mark_not_spam(d: &SyncDeps, ids: Vec<String>) -> Result<CommandOutcome, CoreError> {
    let failed = |w: &str, e: StoreError| CoreError::Invalid { field: "message".into(), reason_words: format!("{w}: {e}") };
    let mut refs: Vec<(String, MessageRef)> = Vec::new();
    for id in &ids {
        let m = d.store.message(id).map_err(|e| failed("the message could not be read", e))?.ok_or_else(|| CoreError::NotFound { kind: "message".into(), id: id.clone() })?;
        let remote = d.store.junk_remote_id(id).map_err(|e| failed("the message could not be read", e))?;
        refs.push((m.account_id.clone(), MessageRef { message_id: id.clone(), thread_id: m.thread_id.clone(), remote_id: remote, folder: m.folder.clone() }));
    }
    learn(&d.store, &ids, JunkLesson::NotJunk).map_err(|e| failed("the lesson could not be kept", e))?;
    let mut rows: Vec<NewPending> = Vec::new();
    for (account, r) in &refs {
        if role_folder(d, account, FolderRole::Junk).as_deref() == Some(r.folder.as_str()) {
            rows.extend(pending_rows(&WriteBack::MoveToFolder { account_id: account.clone(), messages: vec![r.clone()], folder: INBOX_FOLDER.to_string() }));
        }
    }
    let now = crate::rust_core::now_ms();
    // UIDs belong to a folder: a message that leaves the Junk folder keeps none of its UIDs, or it collides with the INBOX message that has
    // the same number (`message`'s unique key); the server names the new one (COPYUID, applied by the write-back worker, or the sync)
    let leaving: Vec<String> = refs.iter().filter(|(account, r)| role_folder(d, account, FolderRole::Junk).as_deref() == Some(r.folder.as_str())).map(|(_, r)| r.message_id.clone()).collect();
    d.store.write(move |ctx| {
        ctx.clear_remote_ids(&leaving)?;
        ctx.set_folder(&ids, INBOX_FOLDER)?;
        for id in &ids { ctx.junk_clear_state(id)?; }
        for row in &rows { ctx.pending_action_add(row, now)?; }
        Ok(())
    }).map_err(|e| failed("the messages could not be moved back to the inbox", e))?;
    Ok(CommandOutcome { undo_entry_id: None, created_id: None })
}

// ---------------------------------------------------------------- what the screen can read

/// junk-r3, `message/<id>/junk`: why the message is where it is, as JSON:
/// `{"folder":"Spam","score":0.93|null,"state":"moved"|"suspect"|null,"lesson":"junk"|"not_junk"|null,"blocked":bool,"correspondent":bool}`.
/// `score` null = never scored (old mail, a re-read, read elsewhere); `blocked` = the sender or its domain is on the block list now;
/// `correspondent` = the owner has written to the sender (the guard keeps such mail out of Junk on score alone). The header rules that
/// fired are NOT here: the store keeps no column for them (docs/lanes/body-r1.md names the migration a "why" line needs).
pub fn state(store: &Store, path: &str) -> Option<String> {
    let ["message", id, "junk"] = path.split('/').collect::<Vec<_>>()[..] else { return None };
    let m = store.message(id).ok().flatten()?;
    let (score, st) = store.junk_of(id).ok().flatten().unwrap_or((None, None));
    let lesson = store.junk_lesson(id).ok().flatten();
    let from = m.from_address.trim().to_ascii_lowercase();
    let blocked = context(store).hard_rules.iter().any(|r| r.matches(&from));
    let correspondent = store.written_to(&m.account_id, std::slice::from_ref(&from)).map(|w| w.contains(&from)).unwrap_or(false);
    let word = |w: Option<&str>| w.map(|w| format!("\"{w}\"")).unwrap_or_else(|| "null".into());
    Some(format!(
        "{{\"folder\":{},\"score\":{},\"state\":{},\"lesson\":{},\"blocked\":{blocked},\"correspondent\":{correspondent}}}",
        crate::reads::json_text(&m.folder),
        score.map(|s| format!("{s:.4}")).unwrap_or_else(|| "null".into()),
        word(st.map(|s| match s { JunkState::Suspect => "suspect", JunkState::Moved => "moved" })),
        word(lesson.map(|l| match l { JunkLesson::Junk => "junk", JunkLesson::NotJunk => "not_junk" })),
    ))
}
