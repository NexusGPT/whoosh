//! What one run of the label flow reads (delta 17, docs/lanes/engine-decision.md): the labels in sidebar order with their cards and
//! chains, the Labels page the decision crate builds from them, the facts a message is judged on, and where a message sits now.
//! Everything is read once per run, from the store: nothing is cached between runs, so an edit of a label (a card, a chain, a
//! fallback) is the next run's input and no signal has to carry it.
use std::sync::{Arc, Mutex};

use contracts::{labels_without_views, CoreError, DecidedBy, DecisionPort, DecisionProviderKind, WhyCard};
use decision::flow::{decide_pipeline, fallback_place};
use decision::{CurrentFiling, DecideMessage, Decisions, DecisionSettings, FlowPlace, LabelNode, Outcome, OutcomeList, RerunAction, RerunLabel};
use rusqlite::{params, Connection, OptionalExtension};
use smart_inbox::load::{facts_for, load_context, open_reader, Context};
use smart_inbox::MessageFacts;
use store::{Store, INBOX_FOLDER};

/// The engine behind the Labels page's model entry: the decision crate's own port (`contracts::DecisionPort`), which the local
/// model and Jev both implement. Empty = no provider is configured: the entry rules and the exit filters still run.
pub(crate) type PortSlot = Arc<Mutex<Option<Arc<dyn DecisionPort>>>>;

/// What the app set, else (the hidden check's rig copy only, armed by `check_flow::seed`) the check's own provider.
pub(crate) fn port_of(slot: &PortSlot) -> Option<Arc<dyn DecisionPort>> {
    slot.lock().unwrap_or_else(|e| e.into_inner()).clone().or_else(crate::check_flow::ai_port)
}

pub(crate) struct FlowCtx {
    pub nodes: Vec<LabelNode>,
    pub settings: DecisionSettings,
    pub port: Option<Arc<dyn DecisionPort>>,
    /// The Labels page holds a model entry: at least one label with its AI card on carries a description.
    pub asks_model: bool,
    reader: Connection,
    mail: Context,
}

impl FlowCtx {
    pub fn open(store: &Store, port: Option<Arc<dyn DecisionPort>>) -> Result<FlowCtx, String> {
        let words = |what: &str, e: &dyn std::fmt::Display| format!("{what}: {e}");
        let labels = labels_without_views(store.labels_full().map_err(|e| words("the labels could not be read", &e))?);
        let rules = store.label_rules_all().map_err(|e| words("the label rules could not be read", &e))?;
        let nodes = LabelNode::with_chains(&labels.iter().map(LabelNode::from_label).collect::<Vec<_>>(), &rules);
        let kind = port.as_ref().map(|p| p.provider()).unwrap_or(DecisionProviderKind::Rules);
        let mut settings = DecisionSettings::shipped(kind, &[Outcome::Labels]);
        // the trust level is the ledger's; while the row is missing the page is rules only (never a literal here)
        let list = OutcomeList::labels_from_ledger(&nodes, &rules).unwrap_or_else(|_| OutcomeList::labels_from_records(&nodes, &rules, None));
        let asks_model = list.has_model_entry();
        if let Some(page) = settings.page_mut(Outcome::Labels) {
            page.list = list;
        }
        let reader = open_reader(store.path()).map_err(|e| words("the mail database could not be opened for reading", &e))?;
        let mail = load_context(&reader).map_err(|e| words("the owner's addresses and VIPs could not be read", &e))?;
        Ok(FlowCtx { nodes, settings, port, asks_model, reader, mail })
    }

    /// The facts of stored messages (columns already synced: no second fetch). A message that is not stored has none.
    pub fn facts(&self, ids: &[String]) -> Result<Vec<MessageFacts>, String> {
        let facts = facts_for(&self.reader, &self.mail, ids).map_err(|e| format!("the facts of the messages could not be read: {e}"))?;
        self.with_thread(facts)
    }

    /// Lane labels-r5: the rest of the thread, for the labels that tick « Reste du fil » (`AiReads.rest_of_thread`, read by
    /// `LabelNode::from_label`). Loaded only when a label with its AI card on ticks it, and only up to the biggest budget a model can use;
    /// each engine then cuts it to its own budget, oldest messages first.
    fn with_thread(&self, mut facts: Vec<MessageFacts>) -> Result<Vec<MessageFacts>, String> {
        if self.nodes.iter().any(|n| n.ai_enabled && n.rest_of_thread) {
            let up_to = decision::thread_context::load_up_to_chars();
            for f in &mut facts {
                smart_inbox::thread::attach_thread(&self.reader, f, up_to).map_err(|e| format!("the earlier messages of the thread could not be read: {e}"))?;
            }
        }
        Ok(facts)
    }

    /// The labelling order on one message, A140's three-step pipeline (`decision::flow::decide_pipeline`): the AI-off labels whose
    /// step 1 matched win first (then their step 3); else the model (when a provider is set) rules over the AI-on labels whose step 1
    /// the message passed, and is not asked when there is none; then the step 3 of the label it chose.
    pub fn decide(&self, f: &MessageFacts) -> Decisions {
        decide_pipeline(&DecideMessage { settings: &self.settings, engine: self.port.as_deref(), tabs: &[], labels: &self.nodes }, f)
    }

    pub fn current(&self, message_id: &str) -> Result<CurrentFiling, String> {
        current_filing(&self.reader, message_id)
    }
}

/// `DecidedBy` from the two columns the store keeps it in.
pub(crate) fn decided_by_from(word: &str, rule_id: Option<String>, model: Option<&str>) -> DecidedBy {
    store::decided_by_from(word, rule_id, model)
}

fn card_from(word: Option<&str>) -> WhyCard {
    match word {
        Some("entry") => WhyCard::Entry,
        Some("ai") => WhyCard::Ai,
        Some("exit") => WhyCard::Exit,
        Some("fallback") => WhyCard::Fallback,
        _ => WhyCard::Hand,
    }
}

/// Where a message sits and what placed it, as the store holds it: the label it carries by a flow card (`label_assignment.flow_card`),
/// else the place its rejection row names (Primary or Junk, which hold no assignment), else Primary by the owner's hand or the default.
/// The label the AI chose, when the exit filter sent the message on, is the rejection row's.
pub(crate) fn current_filing(c: &Connection, message_id: &str) -> Result<CurrentFiling, String> {
    let at = |e: rusqlite::Error| format!("the filing of {message_id} could not be read: {e}");
    let row: Option<(String, String, Option<String>, Option<String>, Option<i64>, Option<String>)> = c
        .query_row(
            "SELECT label_id, decided_by, rule_id, flow_card, flow_chain_id, decided_model FROM label_assignment WHERE message_id = ?1 AND flow_card IS NOT NULL",
            params![message_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)),
        )
        .optional()
        .map_err(at)?;
    let rejection: Option<(String, String)> = c
        .query_row("SELECT ai_label_id, sent_to FROM flow_rejection WHERE message_id = ?1", params![message_id], |r| Ok((r.get(0)?, r.get(1)?)))
        .optional()
        .map_err(at)?;
    Ok(match row {
        Some((label, by, rule, card, chain, model)) => {
            let card = card_from(card.as_deref());
            let ai_pick = match card {
                WhyCard::Ai | WhyCard::Exit => Some(label.clone()),
                WhyCard::Fallback => rejection.map(|(ai, _)| ai),
                _ => None,
            };
            CurrentFiling { place: FlowPlace::Label(label), card, chain_id: chain.map(|n| n as u32), ai_pick, decided_by: decided_by_from(&by, rule, model.as_deref()) }
        }
        None => match rejection {
            // a rejection with no assignment: the exit filter sent the AI's pick to Primary or Junk (the row keeps no provider)
            Some((ai, sent_to)) => CurrentFiling {
                place: fallback_place(Some(&sent_to)).unwrap_or(FlowPlace::Primary),
                card: WhyCard::Fallback,
                chain_id: None,
                ai_pick: Some(ai),
                decided_by: DecidedBy::Default,
            },
            None => CurrentFiling { place: FlowPlace::Primary, card: WhyCard::Hand, chain_id: None, ai_pick: None, decided_by: DecidedBy::Default },
        },
    })
}

/// Who decides a filing the model made when the store no longer says which model it was (a rejection row keeps no provider): the
/// provider set now. None set = the default, as the store reads it.
pub(crate) fn ai_decider(port: Option<&Arc<dyn DecisionPort>>) -> DecidedBy {
    match port.map(|p| p.provider()) {
        Some(DecisionProviderKind::Local) => DecidedBy::Laya,
        Some(DecisionProviderKind::Jev) => DecidedBy::Jev,
        Some(DecisionProviderKind::Apple) => DecidedBy::Apple,
        _ => DecidedBy::Default,
    }
}

// ---------------------------------------------------------------- Command::RerunLabel (lane labels-r5)

fn unexpected(why: String) -> CoreError {
    CoreError::Unexpected { reason_words: why }
}

/// What the last re-run of each label did, and whether one is running: the book `Filing` keeps and the state paths `label/<id>/rerun/*` read.
#[derive(Debug, Clone, Default)]
pub(crate) struct RerunStatus {
    pub running: bool,
    /// Messages the run looks at, and how many it has judged so far.
    pub total: u32,
    pub done: u32,
    /// At least one message kept its filing because the model part could not run.
    pub pending: bool,
    /// Messages under the label when the run ended; messages the run filed into / took out of it.
    pub filed: u32,
    pub refiled: u32,
    pub unfiled: u32,
    /// Inbound messages the label's meaning has not judged yet because the step ended at `AI_RERUN_MAX` (0 when the run reached the oldest one), and
    /// where the next step starts. A re-run with the same description continues from `next_skip`; a changed description starts again at 0.
    pub beyond: u32,
    pub next_skip: usize,
    pub description_key: u64,
    /// Messages the steps of this description have judged already (a continuation never judges one twice).
    pub judged: std::collections::HashSet<String>,
}
pub(crate) type RerunBook = Arc<Mutex<std::collections::HashMap<String, RerunStatus>>>;

/// The most inbound messages ONE STEP of a re-run judges by the label's meaning (the newest first). A number of this lane, proposed for docs/CONSTANTS.md
/// as LABEL_RERUN_MAX_MESSAGES: a re-run is a background job, and a mailbox of 50,000 messages is not judged by a model in one go. A step that stops here
/// SAYS so (`label/<id>/rerun/beyond`: how many older messages are left) and the next Re-run, with the same description, continues from there: it never
/// stops silently.
pub(crate) const AI_RERUN_MAX: usize = 500;
/// Messages judged and written per step, so the result lands as it goes and a stop loses at most one step.
const RERUN_CHUNK: usize = 10;

/// The messages a re-run of the label could look at, by the way they come in: filed under it today, matched by its entry rules now, and, when its AI card
/// is on and it has a description, the newest inbound INBOX messages of this step (the model judges them by the meaning). A140: when the label's step 1
/// picks by chains (entry card on, at least one entry chain), the model is asked only about what step 1 picks, so `rule` IS the AI's part and the newest
/// inbound messages are not added (`ai` stays empty, nothing is left beyond); an open step 1 (no chain, or the card off) lets every mail pass, as before.
struct Parts {
    filed: Vec<String>,
    rule: Vec<String>,
    ai: Vec<String>,
    /// The label's AI part applies (card on, a description): whether or not a model can answer.
    ai_wanted: bool,
    beyond: u32,
}

fn candidate_parts(store: &Store, ctx: &FlowCtx, label_id: &str, skip: usize) -> Result<Parts, String> {
    let words = |what: &str, e: rusqlite::Error| format!("the {what} could not be listed: {e}");
    let filed: Vec<String> = {
        let mut st = ctx
            .reader
            .prepare("SELECT la.message_id FROM label_assignment la JOIN message m ON m.id = la.message_id WHERE la.label_id = ?1 AND m.folder = ?2 ORDER BY la.message_id")
            .map_err(|e| words("messages filed under the label", e))?;
        let rows = st.query_map(params![label_id, INBOX_FOLDER], |r| r.get::<_, String>(0)).map_err(|e| words("messages filed under the label", e))?;
        rows.collect::<Result<Vec<_>, _>>().map_err(|e| words("messages filed under the label", e))?
    };
    let node = ctx.nodes.iter().find(|n| n.id == label_id);
    let mut rule = Vec::new();
    if let Some(node) = node.filter(|n| n.entry_enabled) {
        for chain in node.chains_of(contracts::ChainPlacement::Entry) {
            rule.extend(smart_inbox::corpus::rule_set_matching_ids(store, chain.rules, usize::MAX));
        }
    }
    let mut parts = Parts { filed, rule, ai: Vec::new(), ai_wanted: node.is_some_and(decision::flow::ai_step_on), beyond: 0 };
    if parts.ai_wanted && !node.is_some_and(step_one_by_chains) {
        const INBOUND: &str = "FROM message m WHERE m.folder = ?1 AND lower(m.from_address) NOT IN (SELECT lower(address) FROM account)";
        let mut st = ctx
            .reader
            .prepare(&format!("SELECT m.id {INBOUND} ORDER BY m.received_at_ms DESC, m.id DESC LIMIT ?2 OFFSET ?3"))
            .map_err(|e| words("newest inbound messages", e))?;
        let rows = st.query_map(params![INBOX_FOLDER, AI_RERUN_MAX as i64, skip as i64], |r| r.get::<_, String>(0)).map_err(|e| words("newest inbound messages", e))?;
        parts.ai = rows.collect::<Result<Vec<_>, _>>().map_err(|e| words("newest inbound messages", e))?;
        let all: i64 = ctx.reader.query_row(&format!("SELECT COUNT(*) {INBOUND}"), params![INBOX_FOLDER], |r| r.get(0)).map_err(|e| words("inbound messages", e))?;
        parts.beyond = (all as usize).saturating_sub(skip + AI_RERUN_MAX) as u32;
    }
    Ok(parts)
}

/// A140: the label's step 1 picks by chains (its entry card is on and holds at least one entry chain); else every mail passes it.
fn step_one_by_chains(n: &LabelNode) -> bool {
    n.entry_enabled && !n.chains_of(contracts::ChainPlacement::Entry).is_empty()
}

/// Lane fix-aisort round 17: the AI-on labels with no step 1 (Needs Reply as seeded): a mail their AI could not judge is marked as waiting.
pub(crate) fn labels_without_step_one(nodes: &[LabelNode]) -> std::collections::HashSet<String> {
    nodes.iter().filter(|n| decision::flow::ai_step_on(n) && !step_one_by_chains(n)).map(|n| n.id.clone()).collect()
}

/// The INBOX messages a re-run of `label_id` looks at: those filed under it today, those its entry rules match now, and, when its AI card is on, it has a
/// description AND a model can answer (`ai_ok`), the newest inbound INBOX messages. With no model that can answer, the AI part is not looked at: a
/// preview of "38 join" for mail nothing can judge would promise what the re-run cannot do.
pub(crate) fn rerun_candidates(store: &Store, ctx: &FlowCtx, label_id: &str, skip: usize, judged: &std::collections::HashSet<String>, ai_ok: bool) -> Result<(Vec<String>, u32), String> {
    let p = candidate_parts(store, ctx, label_id, skip)?;
    let mut ids = p.filed;
    ids.extend(p.rule);
    if ai_ok { ids.extend(p.ai); }
    ids.sort();
    ids.dedup();
    ids.retain(|id| !judged.contains(id));
    Ok((ids, if ai_ok { p.beyond } else { 0 }))
}

/// The one line the preview and the inspector say when the label's AI card is on and no model can answer.
pub(crate) fn ai_cannot_words(refusal: Option<(String, String)>) -> Option<String> {
    refusal.map(|(_, words)| {
        if words == crate::decision_model::NO_MODEL_WORDS { "The AI cannot sort yet: no model on this Mac.".to_string() }
        else if words == crate::decision_model::NO_KEY_WORDS { "The AI cannot sort yet: the Jev key is missing.".to_string() }
        else if words == crate::decision_model::NO_CONSENT_WORDS { "The AI cannot sort yet: you have not allowed Jev to read your mail.".to_string() }
        else { format!("The AI cannot sort yet: {}", words.trim_end_matches('.').to_lowercase().replacen("the ", "the ", 1)) }
    })
}

/// State `label/<id>/rerun/preview`: what a re-run would do, in the foot's own terms (`flow/counts`: by rule, by the AI), from ONE computation of the
/// candidates the run uses. `by_rule` messages an entry rule would add; `by_ai` messages the model would judge (0 when no model can answer, and
/// `ai_words` then says so in one line); `leave` filed messages no entry rule keeps (a label with no meaning); `stay` the filed ones that remain.
/// A140: for a label whose AI step is on, step 1 files nothing by itself: what its chains pick is what the model would judge, so `by_rule` is 0 and
/// `by_ai` counts the step-1 picks not filed yet (or the newest inbound messages when step 1 is open).
pub(crate) fn rerun_preview(store: &Store, book: &RerunBook, label_id: &str, ai_ok: bool, refusal: Option<(String, String)>) -> Option<String> {
    let ctx = FlowCtx::open(store, None).ok()?;
    let (skip, judged) = continuation(book, &ctx, label_id);
    let p = candidate_parts(store, &ctx, label_id, skip).ok()?;
    let filed: std::collections::HashSet<&String> = p.filed.iter().collect();
    let rule: std::collections::HashSet<&String> = p.rule.iter().collect();
    let (by_rule, by_ai) = if p.ai_wanted {
        // A140: the AI's part is the step-1 picks (`rule`) and, with an open step 1, the newest inbound messages (`ai`)
        let ai: std::collections::HashSet<&String> = p.rule.iter().chain(p.ai.iter()).collect();
        (0, if ai_ok { ai.iter().filter(|m| !filed.contains(**m) && !judged.contains(**m)).count() } else { 0 })
    } else {
        (rule.iter().filter(|m| !filed.contains(**m) && !judged.contains(**m)).count(), 0)
    };
    let leave = if rule.is_empty() && !ctx.nodes.iter().find(|n| n.id == label_id).is_some_and(|n| n.entry_enabled && !n.chains_of(contracts::ChainPlacement::Entry).is_empty()) || p.ai_wanted { 0 } else { filed.iter().filter(|m| !rule.contains(**m)).count() };
    let ai_words = if p.ai_wanted { ai_cannot_words(refusal.filter(|_| !ai_ok)) } else { None };
    let words = match &ai_words { Some(w) => format!("\"{}\"", w.replace('\\', "\\\\").replace('"', "\\\"")), None => "null".into() };
    Some(format!("{{\"by_rule\":{by_rule},\"by_ai\":{by_ai},\"leave\":{leave},\"stay\":{},\"beyond\":{},\"ai_words\":{words}}}", filed.len().saturating_sub(leave), if ai_ok { p.beyond } else { 0 }))
}

/// The key of what the label's meaning is judged by: a changed description starts a re-run again at the newest message.
fn description_key(ctx: &FlowCtx, label_id: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    ctx.nodes.iter().find(|n| n.id == label_id).map(|n| n.description.trim().to_string()).unwrap_or_default().hash(&mut h);
    h.finish()
}

/// Where the next step of a re-run of the label starts and what it has judged: after the last step when this description left older messages, else
/// at the newest with nothing judged.
fn continuation(book: &RerunBook, ctx: &FlowCtx, label_id: &str) -> (usize, std::collections::HashSet<String>) {
    let book = book.lock().unwrap_or_else(|e| e.into_inner());
    book.get(label_id).filter(|s| s.beyond > 0 && s.description_key == description_key(ctx, label_id)).map_or((0, Default::default()), |s| (s.next_skip, s.judged.clone()))
}

/// How many messages a re-run of the label would look at now (state `label/<id>/rerun/candidates`: the number the Save offer shows).
pub(crate) fn rerun_candidate_count(store: &Store, book: &RerunBook, label_id: &str, ai_ok: bool) -> Option<(usize, u32)> {
    let ctx = FlowCtx::open(store, None).ok()?;
    let (skip, judged) = continuation(book, &ctx, label_id);
    rerun_candidates(store, &ctx, label_id, skip, &judged, ai_ok).ok().map(|(v, beyond)| (v.len(), beyond))
}

/// Older inbound messages the label's meaning has not judged yet: what the last run of this description left (running or ended), else what a first step
/// would leave. State `label/<id>/rerun/beyond`: the shell says "500 of N sorted, press Re-run to continue" from it.
pub(crate) fn rerun_beyond(store: &Store, book: &RerunBook, label_id: &str, ai_ok: bool) -> Option<u32> {
    let ctx = FlowCtx::open(store, None).ok()?;
    let key = description_key(&ctx, label_id);
    if let Some(s) = book.lock().unwrap_or_else(|e| e.into_inner()).get(label_id).filter(|s| s.total > 0 && s.description_key == key) { return Some(s.beyond); }
    rerun_candidates(store, &ctx, label_id, 0, &Default::default(), ai_ok).ok().map(|(_, beyond)| beyond)
}

/// "Re-run on N messages": the labelling order runs again, alone, on the candidates above, in steps of `RERUN_CHUNK`, each written in one transaction as
/// for an arrival (`flow_write::apply`) and counted in `book` as it lands. A message the flow placed leaves its label when the order now puts it
/// elsewhere; a message the OWNER filed by hand never moves; a message whose meaning the model could not judge (no model, or its refusal) keeps its
/// filing. It runs on the filing worker, never on the caller's thread: a model takes its own time and no view waits for it.
pub(crate) fn rerun_label(store: &Store, port: Option<Arc<dyn DecisionPort>>, ai_ok: bool, roles: &crate::imap_source::Roles, label_id: &str, now_ms: i64, book: &RerunBook) -> Result<(), CoreError> {
    use crate::flow_write::{apply, Filing, Write};
    let set = |f: &dyn Fn(&mut RerunStatus)| f(book.lock().unwrap_or_else(|e| e.into_inner()).entry(label_id.to_string()).or_default());
    let fail = |e: CoreError| { set(&|s| s.running = false); e };
    if !store.labels_full().map_err(|e| fail(unexpected(e.to_string())))?.iter().any(|l| l.id == label_id) {
        return Err(fail(CoreError::NotFound { kind: "label".into(), id: label_id.into() }));
    }
    let ctx = FlowCtx::open(store, port).map_err(|e| fail(unexpected(e)))?;
    let (skip, mut judged) = continuation(book, &ctx, label_id);
    // A140 part 3: Re-run means "judge again": the AI's kept "no" for this label goes (a continuation of the same description keeps
    // what the earlier steps of this run judged)
    if skip == 0 {
        store.ai_no_clear_label(label_id).map_err(|e| fail(unexpected(e.to_string())))?;
    }
    let by_model = crate::label_ai_failure::model_decider(store, ctx.port.as_ref());
    let (ids, beyond) = rerun_candidates(store, &ctx, label_id, skip, &judged, ai_ok).map_err(|e| fail(unexpected(e)))?;
    // the label's meaning has nothing to judge it with: the messages only the model could add are not looked at, and the run says its AI part is pending
    let ai_skipped = !ai_ok && ctx.nodes.iter().any(|n| n.id == label_id && n.ai_enabled && !n.description.trim().is_empty());
    let key = description_key(&ctx, label_id);
    set(&|s| *s = RerunStatus { running: true, total: ids.len() as u32, beyond, next_skip: skip + AI_RERUN_MAX, description_key: key, ..Default::default() });
    judged.extend(ids.iter().cloned());
    set(&|s| s.judged = judged.clone());
    let clock = crate::sync::SystemClock;
    let mut aside_counts = crate::label_ai_failure::SetAsideCounts::new();
    for chunk in ids.chunks(RERUN_CHUNK) {
        let facts = ctx.facts(chunk).map_err(|e| fail(unexpected(e)))?;
        let account_of: std::collections::HashMap<String, String> = facts.iter().map(|f| (f.message_id.clone(), f.account_id.clone())).collect();
        let run = RerunLabel { settings: &ctx.settings, engine: ctx.port.as_deref(), tabs: &[], labels: &ctx.nodes, label_id: label_id.into(), clock: &clock, budget_ms: None };
        let rerun = run.execute(facts.clone());
        // round 11: the mails the judge refused for themselves are set aside (kept, not asked again; their filing stays: their rows are Pending)
        let mut aside = Vec::new();
        for (message_id, reason) in &rerun.set_aside {
            let Some(f) = facts.iter().find(|f| &f.message_id == message_id) else { continue };
            for l in decision::flow::ai_candidates(&ctx.nodes, f) {
                *aside_counts.entry(l.clone()).or_default().entry(reason.clone()).or_default() += 1;
                aside.push((message_id.clone(), l));
            }
        }
        store.ai_set_aside_put(aside, DecidedBy::Apple, now_ms).map_err(|e| fail(unexpected(e.to_string())))?;
        // fix-aisort: a failed or unreadable judgement keeps its mail waiting (`Pending` rows write nothing) and is said once
        crate::label_ai_failure::note_rerun(store, label_id, rerun.engine_error.as_ref().map(|e| e.to_string()), rerun.rows.iter().any(|r| r.action != RerunAction::Pending));
        // round 17: as on arrival, a mail the model could not judge waits for the AI-on labels with no step 1 it was offered to, and a judged
        // one no longer waits; a mail set aside is neither
        let open = labels_without_step_one(&ctx.nodes);
        let aside_ids: std::collections::HashSet<&String> = rerun.set_aside.iter().map(|(m, _)| m).collect();
        let (mut waits, mut judged_now) = (Vec::new(), Vec::new());
        for row in &rerun.rows {
            let Some(f) = facts.iter().find(|f| f.message_id == row.message_id) else { continue };
            let offered = decision::flow::ai_candidates(&ctx.nodes, f);
            if row.action == RerunAction::Pending {
                if !aside_ids.contains(&row.message_id) {
                    waits.extend(offered.into_iter().filter(|l| open.contains(l)).map(|l| (row.message_id.clone(), l)));
                }
            } else {
                judged_now.extend(offered.into_iter().map(|l| (row.message_id.clone(), l)));
            }
        }
        store.ai_waiting_put(waits, now_ms).map_err(|e| fail(unexpected(e.to_string())))?;
        store.ai_waiting_clear(judged_now).map_err(|e| fail(unexpected(e.to_string())))?;
        // A140 part 3: the model's "no" of this run, kept as on arrival: every AI-on label whose step 1 the message passed and that the model
        // did not accept, for a row the model answered (not pending, not settled by an AI-off label's step 1)
        if matches!(by_model, DecidedBy::Laya | DecidedBy::Jev | DecidedBy::Apple | DecidedBy::Model { .. }) {
            let mut nos = Vec::new();
            for row in rerun.rows.iter().filter(|r| r.action != RerunAction::Pending) {
                let Some(v) = row.verdict.as_ref().filter(|v| v.decided_by.source != decision::Source::Rule) else { continue };
                let Some(f) = facts.iter().find(|f| f.message_id == row.message_id) else { continue };
                let pick = v.flow.as_ref().and_then(|fl| fl.ai_pick.clone());
                for l in decision::flow::ai_candidates(&ctx.nodes, f).into_iter().filter(|l| pick.as_deref() != Some(l.as_str())) {
                    nos.push((row.message_id.clone(), l));
                }
            }
            store.ai_no_put(nos, by_model.clone(), now_ms).map_err(|e| fail(unexpected(e.to_string())))?;
        }
        let mut writes = Vec::new();
        let (mut refiled, mut unfiled) = (0u32, 0u32);
        for row in rerun.rows.iter().filter(|r| r.action != RerunAction::Pending) {
            let now = ctx.current(&row.message_id).map_err(|e| fail(unexpected(e)))?;
            let file = row.file_write().map(|w| Filing { label_id: w.label_id, by: w.by, entry: w.entry, probability: w.probability, card: w.card, chain_id: w.chain_id });
            // the label a flow card holds the message under leaves when the order puts it somewhere else. `current` names only filings a flow
            // card made (`flow_card IS NOT NULL`), so the owner's hand filing is never a label held here and is never taken off
            let unassign = match &now.place {
                FlowPlace::Label(held) if file.as_ref().map(|f| &f.label_id) != Some(held) => Some(held.clone()),
                _ => None,
            };
            match row.action { RerunAction::File => refiled += 1, RerunAction::Unfile => unfiled += 1, _ => {} }
            let place = row.verdict.as_ref().and_then(|v| v.flow.as_ref()).map(|fl| fl.place.clone());
            let to_junk = match (&place, account_of.get(&row.message_id)) {
                (Some(FlowPlace::Junk), Some(account)) => crate::flow_write::junk_folder(store, roles, account),
                _ => None,
            };
            writes.push(Write {
                message_id: row.message_id.clone(),
                unassign,
                back_to_inbox: now.place == FlowPlace::Junk && place != Some(FlowPlace::Junk),
                file,
                to_junk,
                rejection: row.rejection(now_ms),
            });
        }
        apply(store, writes, now_ms).map_err(|e| fail(unexpected(e)))?;
        let (n, pending) = (chunk.len() as u32, rerun.model_pending);
        set(&|s| { s.done += n; s.refiled += refiled; s.unfiled += unfiled; s.pending |= pending; });
    }
    crate::label_ai_failure::log_set_aside(store, &aside_counts);
    let filed = store.label_filed_count(label_id).unwrap_or(0);
    set(&|s| { s.running = false; s.filed = filed; s.pending |= ai_skipped; });
    Ok(())
}
