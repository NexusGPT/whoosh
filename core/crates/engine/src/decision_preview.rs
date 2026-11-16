//! `Command::PreviewDecisionList` on the real core (gap-preview-core): "Preview on my last 20 messages" of Settings > Decisions, run on the list the PAGE holds.
//!
//! - the list used is the list given: every entry the page holds, ticked or not (an unticked one decides nothing, and the entry numbers still count it), with its
//!   question as edited. The lists saved in the store are never read for the page being previewed;
//! - the mail is the `DECISION_PREVIEW_N` newest messages of the inbox (the ledger's number, `decision::tuning::decision_preview_n`), read through a read-only
//!   connection: nothing is written to the store (a test compares the database file before and after);
//! - the engine is the one the arrival path uses (`Filing::port`: the owner's saved decision model, Local on this Mac or Jev with its key AND the stored
//!   consent, which `JevDecision` re-checks before any request; no model chosen = rules only, nothing leaves the Mac);
//! - a model can answer slowly, so the decision crate's bound (`DECISION_PREVIEW_MS`, read from the ledger) stops the run and names what it did not reach. While
//!   the ledger holds no measured number the preview WITH a model is refused in words (`PreviewDecisions::from_ledger`); the rules-only preview has no model call to
//!   bound and runs;
//! - the rows wait in memory (never in the store), in the `PreviewBook` the engine's core object holds (gap-decisions-real: it was a process-wide map keyed by the
//!   store path), for `WhooshCore::decision_preview()`; the sentence for an empty or cut-short preview is the state `decision_preview/words`;
//! - `Command::ApplyDecisionPreview` (`apply`, gap-decisions-real) writes the placements of the LAST preview, for the page it ran on, through the writes the arrival path
//!   uses (`smart_inbox::load::write_placements` for a tab, `label_file_flow` for a label, the owner's own filing never overwritten). It refuses a preview that is stale
//!   (the newest mail, or where one of those messages sits, changed since the run), writes nothing then, and a second Apply writes nothing new. The arrival path's
//!   writes carry no undo entry (flow_write.rs, place_new.rs), so Apply carries none either. State `decision_preview/applied` is "true" after an Apply, "false"
//!   before; `decision_preview/placed` is how many placements that Apply wrote.
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use contracts::{labels_without_views, Clock, CoreError, DecidedBy, DecisionPort, DecisionProviderKind, DecisionVerdict, PreviewEntry, PreviewEntryKind, RuleCondition, WhyCard};
use rusqlite::{params, Connection, OptionalExtension};
use smart_inbox::list::DecisionPage;
use smart_inbox::load::write_placements;
use smart_inbox::Placement;
use decision::entry::{label_paths, Entry, EntryKind, LabelNode, OutcomeList, Value, When};
use decision::{DecideMessage, DecisionSettings, FlowPlace, Outcome, PreviewDecisions, PreviewInput, Source, TabOption};
use smart_inbox::load::{facts_for, load_context, open_reader};
use smart_inbox::{Cond, TabRef, TabSet};
use store::Store;

/// The page's separator between the names of a label path (AILabelTree.separator).
const PAGE_SEP: &str = " › ";
/// The words of a row no entry answered.
const NONE_WORDS: &str = "—";
pub const EMPTY_WORDS: &str = "There is no mail in your inbox to preview yet.";

/// What Apply writes for one previewed message.
#[derive(Debug, Clone)]
enum Target {
    Tab(String),
    Label(String),
}

#[derive(Debug, Clone)]
struct Plan {
    message_id: String,
    target: Target,
    by: DecidedBy,
    entry_id: String,
    /// The model entry that decided (a model filing only), as `label_assignment.decided_entry_id` keeps it.
    model_entry: Option<String>,
    position: usize,
    probability: Option<f64>,
    card: WhyCard,
    chain_id: Option<u32>,
}

/// Where each of the previewed messages sits: (message id, tab id, the labels it is filed under, sorted). Compared at Apply: a different one means the preview is stale.
type Snapshot = Vec<(String, String, Vec<String>)>;

/// One run of the preview: the rows the page draws, the sentence, and (private) what Apply would write and the state of the mail it was computed on.
#[derive(Debug, Clone, Default)]
pub struct Last {
    pub rows: Vec<DecisionVerdict>,
    pub words: String,
    plans: Vec<Plan>,
    page: Option<Outcome>,
    snapshot: Snapshot,
    /// Some(n) once Apply ran on this preview: n placements were written.
    placed: Option<usize>,
}

/// A refused preview: the error the command answers, and (for the refusal the page shows in words) the run to keep.
#[derive(Debug)]
pub struct Refused {
    pub error: CoreError,
    pub keep: Option<Last>,
}

impl From<CoreError> for Refused {
    fn from(error: CoreError) -> Self { Refused { error, keep: None } }
}

/// The last preview of this core (a field of `RustCore`, not a process-wide map: two cores never see each other's rows).
#[derive(Default)]
pub struct PreviewBook(Mutex<Option<Last>>);

impl PreviewBook {
    fn put(&self, last: Last) { *self.0.lock().unwrap_or_else(|e| e.into_inner()) = Some(last); }
    fn read<R>(&self, f: impl FnOnce(Option<&Last>) -> R) -> R { f(self.0.lock().unwrap_or_else(|e| e.into_inner()).as_ref()) }
}

/// `WhooshCore::decision_preview`: the rows of the last run (none before the first).
pub(crate) fn rows(book: &PreviewBook) -> Vec<DecisionVerdict> {
    book.read(|l| l.map(|l| l.rows.clone()).unwrap_or_default())
}

/// States `decision_preview/words` (the sentence of the last run, "" when it needs none), `decision_preview/applied` ("true" after an Apply of the last preview) and
/// `decision_preview/placed` (what that Apply wrote); None for another path.
pub(crate) fn state(book: &PreviewBook, path: &str) -> Option<String> {
    match path {
        "decision_preview/words" => Some(book.read(|l| l.map(|l| l.words.clone()).unwrap_or_default())),
        "decision_preview/applied" => Some(book.read(|l| l.is_some_and(|l| l.placed.is_some())).to_string()),
        "decision_preview/placed" => Some(book.read(|l| l.and_then(|l| l.placed).unwrap_or(0)).to_string()),
        _ => None,
    }
}

fn unexpected(what: &str, e: impl std::fmt::Display) -> CoreError {
    CoreError::Unexpected { reason_words: format!("{what}: {e}") }
}

/// The bound the arm passes: the ledger's number, or its refusal in the crate's own words while it holds none.
pub fn ledger_budget() -> Result<u64, CoreError> {
    decision::tuning::decision_preview_ms().ok_or_else(|| CoreError::Invalid {
        field: "DECISION_PREVIEW_MS".into(),
        reason_words: "docs/CONSTANTS.md holds no measured number for the preview bound yet".into(),
    })
}

/// The command: previews `entries` as the list of `page_id`, keeps the rows in `book`, answers with nothing to undo.
pub fn run(book: &PreviewBook, store: &Store, port: Option<Arc<dyn DecisionPort>>, clock: &dyn Clock, page_id: &str, entries: &[PreviewEntry]) -> Result<(), CoreError> {
    match preview(store, port, clock, page_id, entries, ledger_budget()) {
        Ok(last) => {
            book.put(last);
            Ok(())
        }
        Err(Refused { error, keep }) => {
            if let Some(last) = keep {
                book.put(last);
            }
            Err(error)
        }
    }
}

/// `run` with the bound given (a test passes its own number; the arm passes the ledger's). Without a model the bound is not asked for.
/// It returns the run (nothing is kept here: `run` puts it in the book).
pub fn preview(store: &Store, port: Option<Arc<dyn DecisionPort>>, clock: &dyn Clock, page_id: &str, entries: &[PreviewEntry], budget: Result<u64, CoreError>) -> Result<Last, Refused> {
    let outcome = Outcome::from_key(page_id).ok_or_else(|| CoreError::Invalid { field: "page_id".into(), reason_words: format!("there is no decision page “{page_id}”") })?;
    let budget_ms = match (&port, budget) {
        (None, b) => b.unwrap_or(u64::MAX / 4),
        (Some(_), Ok(ms)) => ms,
        (Some(_), Err(e)) => {
            return Err(Refused { keep: Some(Last { words: reason(&e), ..Last::default() }), error: e });
        }
    };

    // the store, read only
    let labels = labels_without_views(store.labels_full().map_err(|e| unexpected("the labels could not be read", e))?);
    let rules = store.label_rules_all().map_err(|e| unexpected("the label rules could not be read", e))?;
    let nodes = LabelNode::with_chains(&labels.iter().map(LabelNode::from_label).collect::<Vec<_>>(), &rules);
    let conn = open_reader(store.path()).map_err(|e| unexpected("the mail database could not be opened for reading", e))?;
    let ctx = load_context(&conn).map_err(|e| unexpected("the owner's addresses and VIPs could not be read", e))?;
    let ids = recent_ids(&conn).map_err(|e| unexpected("the recent mail could not be read", e))?;
    if ids.is_empty() {
        return Ok(Last { words: EMPTY_WORDS.into(), page: Some(outcome), ..Last::default() });
    }
    let facts = facts_for(&conn, &ctx, &ids).map_err(|e| unexpected("the recent mail could not be read", e))?;
    let today = |id: &str| -> Option<String> {
        let tab: Option<String> = conn.query_row("SELECT tab_id FROM message_category WHERE message_id = ?1", [id], |r| r.get(0)).ok();
        Some(tab.unwrap_or_else(|| ctx.tabs.primary()))
    };
    let inputs: Vec<PreviewInput> = facts.into_iter().map(|f| PreviewInput { current_tab: today(&f.message_id), facts: f }).collect();
    let snapshot = snapshot_of(&conn, &ctx.tabs.primary(), &ids).map_err(|e| unexpected("the recent mail could not be read", e))?;
    let by_id: HashMap<String, (String, String)> = inputs.iter().map(|i| (i.facts.message_id.clone(), (sender_words(&i.facts), i.facts.subject.clone()))).collect();

    // the list as the page holds it; the pages it reads join with their shipped lists
    let kind = port.as_ref().map(|p| p.provider()).unwrap_or(DecisionProviderKind::Rules);
    let (list, positions) = list_of(outcome, entries, &nodes, &ctx.tabs);
    let mut on = vec![outcome];
    if outcome == Outcome::NotifyMe {
        on.extend([Outcome::Vips, Outcome::NeedsMyReply]);
    }
    let mut settings = DecisionSettings::shipped(kind, &on);
    if let Some(page) = settings.page_mut(outcome) {
        page.list = list;
    }
    let tabs = TabOption::from_tab_set(&ctx.tabs);
    let p = PreviewDecisions { decide: DecideMessage { settings: &settings, engine: port.as_deref(), tabs: &tabs, labels: &nodes }, clock, tabs: Some(&ctx.tabs), budget_ms }.execute(inputs);

    let paths: HashMap<String, String> = label_paths(&nodes).into_iter().map(|(k, v)| (v, k.replace(" > ", PAGE_SEP))).collect();
    let mut plans: Vec<Plan> = Vec::new();
    let out: Vec<DecisionVerdict> = p
        .rows
        .iter()
        .map(|row| {
            let (sender, subject) = by_id.get(&row.message_id).cloned().unwrap_or_default();
            let v = row.verdicts.iter().find(|v| v.outcome == outcome);
            match v {
                Some(v) => {
                    let n = positions.get(v.decided_by.position - 1).copied().unwrap_or(v.decided_by.position as u32);
                    let outcome_words = words_of(outcome, v, &paths, &ctx.tabs);
                    let by = match &v.decided_by.source {
                        Source::Rule if outcome == Outcome::Labels => format!("entry {n} · rule · {outcome_words}"),
                        Source::Rule => format!("entry {n} · rule"),
                        Source::Model { answer, probability, .. } => format!("entry {n} · the model said {answer} ({probability:.2})"),
                        Source::Default => format!("entry {n} · otherwise"),
                    };
                    let differs = outcome == Outcome::WhichTab && row.differs_from_today == Some(true);
                    if let Some(plan) = plan_of(outcome, row, v, differs, &snapshot, &ctx.tabs) {
                        plans.push(plan);
                    }
                    DecisionVerdict { message_id: row.message_id.clone(), entry: Some(n), outcome: outcome_words, differs, decided_by: by, sender, subject }
                }
                None => DecisionVerdict { message_id: row.message_id.clone(), entry: None, outcome: NONE_WORDS.into(), differs: false, decided_by: "no entry answered".into(), sender, subject },
            }
        })
        .collect();

    let mut words = String::new();
    if !p.not_reached.is_empty() {
        words = format!("The preview stopped at its time limit: {} of {} messages were not reached.", p.not_reached.len(), p.not_reached.len() + out.len());
    } else if let Some(e) = &p.engine_error {
        words = format!("The model could not answer, so the rules alone decided: {}", reason(e));
    }
    Ok(Last { rows: out, words, plans, page: Some(outcome), snapshot, placed: None })
}

/// The newest `DECISION_PREVIEW_N` inbox messages, newest first (the sample a preview runs on, and the list Apply checks is still the same).
fn recent_ids(conn: &Connection) -> rusqlite::Result<Vec<String>> {
    let n = decision::tuning::decision_preview_n();
    let mut st = conn.prepare("SELECT id FROM message WHERE folder = 'INBOX' AND archived = 0 ORDER BY received_at_ms DESC, id LIMIT ?1")?;
    let rows = st.query_map([n as i64], |r| r.get::<_, String>(0))?;
    rows.collect()
}

/// Where each message sits now: its tab (Primary when no row says otherwise) and the labels it is filed under.
fn snapshot_of(conn: &Connection, primary: &str, ids: &[String]) -> rusqlite::Result<Snapshot> {
    let mut tab = conn.prepare_cached("SELECT tab_id FROM message_category WHERE message_id = ?1")?;
    let mut labels = conn.prepare_cached("SELECT label_id FROM label_assignment WHERE message_id = ?1 ORDER BY label_id")?;
    let mut out = Vec::with_capacity(ids.len());
    for id in ids {
        let t: Option<String> = tab.query_row([id], |r| r.get(0)).optional()?;
        let ls: Vec<String> = labels.query_map([id], |r| r.get::<_, String>(0))?.collect::<Result<_, _>>()?;
        out.push((id.clone(), t.unwrap_or_else(|| primary.to_string()), ls));
    }
    Ok(out)
}

/// What Apply would write for one previewed message (None: nothing to write for it). Only the pages that keep a placement in the store write: "Which tab" files the
/// message in the tab, "Labels" files it under the label; the other pages answer a reading, not a stored placement. A proposed new label is never written.
fn plan_of(outcome: Outcome, row: &decision::PreviewRow, v: &decision::Verdict, differs: bool, snapshot: &Snapshot, tabs: &TabSet) -> Option<Plan> {
    let by = v.decided_by.to_contract();
    let target = match (outcome, &v.value) {
        (Outcome::WhichTab, Value::Tab(t)) if differs => Target::Tab(tabs.resolve(t)),
        (Outcome::Labels, Value::Label(Some(id))) if v.proposal.is_none() => {
            let held = snapshot.iter().find(|(m, _, _)| *m == row.message_id).map(|(_, _, ls)| ls.contains(id)).unwrap_or(false);
            if held {
                return None;
            }
            Target::Label(id.clone())
        }
        _ => return None,
    };
    let (card, chain_id) = match &v.flow {
        Some(fl) if matches!(fl.place, FlowPlace::Label(_)) => (fl.card, fl.chain_id),
        _ => (
            match v.decided_by.source {
                Source::Rule => WhyCard::Entry,
                Source::Model { .. } => WhyCard::Ai,
                Source::Default => WhyCard::Fallback,
            },
            None,
        ),
    };
    Some(Plan { message_id: row.message_id.clone(), target, by, entry_id: v.decided_by.entry_id.clone(), model_entry: v.decided_by.decided_entry_id(), position: v.decided_by.position, probability: v.decided_by.probability(), card, chain_id })
}

const NO_PREVIEW: &str = "Run the decision model on recent mail first.";
const STALE: &str = "The recent mail changed since this preview ran, so nothing was applied. Run the preview again.";

/// `Command::ApplyDecisionPreview`: writes the placements of the last preview (for the page it ran on) and answers how many it wrote. Refused in words, writing nothing,
/// when there is no preview or the preview is stale. A second Apply of the same preview writes nothing new.
pub fn apply(book: &PreviewBook, store: &Store, now_ms: i64) -> Result<usize, CoreError> {
    let mut guard = book.0.lock().unwrap_or_else(|e| e.into_inner());
    let last = match guard.as_mut() {
        Some(l) if !l.rows.is_empty() && l.page.is_some() => l,
        _ => return Err(CoreError::Invalid { field: "preview".into(), reason_words: NO_PREVIEW.into() }),
    };
    let conn = open_reader(store.path()).map_err(|e| unexpected("the mail database could not be opened for reading", e))?;
    let primary = load_context(&conn).map_err(|e| unexpected("the owner's addresses and VIPs could not be read", e))?.tabs.primary();
    let ids = recent_ids(&conn).map_err(|e| unexpected("the recent mail could not be read", e))?;
    let now = snapshot_of(&conn, &primary, &ids).map_err(|e| unexpected("the recent mail could not be read", e))?;
    if now != last.snapshot {
        return Err(CoreError::Invalid { field: "preview".into(), reason_words: STALE.into() });
    }
    if let Some(n) = last.placed {
        return Ok(n);
    }
    let plans = last.plans.clone();
    let message_ids: Vec<String> = plans.iter().map(|p| p.message_id.clone()).collect();
    store
        .write(move |ctx| {
            ctx.will_change(&message_ids)?;
            let mut tabs = Vec::new();
            for p in &plans {
                match &p.target {
                    Target::Tab(tab_id) => tabs.push(Placement { message_id: p.message_id.clone(), tab_id: tab_id.clone(), decided_by: p.by.clone(), probability: p.probability, page: DecisionPage::WhichTab, entry_position: p.position, entry_id: p.entry_id.clone() }),
                    Target::Label(label_id) => {
                        // the owner's own filing is never overwritten (flow_write.rs `owner_filed`)
                        let by: Option<String> = ctx
                            .tx
                            .prepare_cached("SELECT decided_by FROM label_assignment WHERE label_id = ?1 AND message_id = ?2")?
                            .query_row(params![label_id, p.message_id], |r| r.get(0))
                            .optional()?;
                        if by.as_deref() != Some("owner") {
                            ctx.label_file_flow(label_id, &p.message_id, &p.by, p.model_entry.as_deref(), p.probability, p.card, p.chain_id, now_ms)?;
                        }
                    }
                }
            }
            if !tabs.is_empty() {
                write_placements(ctx, &tabs, now_ms)?;
            }
            Ok(())
        })
        .map_err(|e| unexpected("the placements could not be written", e))?;
    // what the write left: the count is the placements that now stand as previewed (a message the owner filed by hand stays as the owner left it)
    let after = snapshot_of(&conn, &primary, &ids).map_err(|e| unexpected("the recent mail could not be read", e))?;
    let placed = last
        .plans
        .iter()
        .filter(|p| {
            after.iter().find(|(m, _, _)| *m == p.message_id).is_some_and(|(_, tab, ls)| match &p.target {
                Target::Tab(t) => tab == t,
                Target::Label(l) => ls.contains(l),
            })
        })
        .count();
    last.snapshot = after;
    last.placed = Some(placed);
    Ok(placed)
}

fn reason(e: &CoreError) -> String {
    match e {
        CoreError::Action { failure } => failure.reason_words.clone(),
        CoreError::Invalid { reason_words, .. } | CoreError::Unexpected { reason_words } => reason_words.clone(),
        other => other.to_string(),
    }
}

fn sender_words(f: &smart_inbox::MessageFacts) -> String {
    if f.from_name.trim().is_empty() { f.from_address.clone() } else { f.from_name.clone() }
}

/// The page's entries as the decision crate's list (only the ticked ones), and the 1-based number each one has on the page.
fn list_of(outcome: Outcome, entries: &[PreviewEntry], nodes: &[LabelNode], tabs: &TabSet) -> (OutcomeList, Vec<u32>) {
    let key = outcome.key();
    let paths: HashMap<String, String> = label_paths(nodes).into_iter().flat_map(|(k, v)| [(k.replace(" > ", PAGE_SEP), v.clone()), (k, v)]).collect();
    let mut list = Vec::new();
    let mut positions = Vec::new();
    for (i, e) in entries.iter().enumerate() {
        if !e.enabled {
            continue;
        }
        let n = i as u32 + 1;
        let then = || value_of(outcome, &e.outcome, &paths, tabs);
        let kind = match e.kind {
            PreviewEntryKind::Rule => EntryKind::Rule { when: vec![when_of(&e.match_key)], then: then() },
            PreviewEntryKind::Model => EntryKind::Model { question: e.question.clone(), trust_from: e.trust.unwrap_or(100) as f64 / 100.0 },
            PreviewEntryKind::Otherwise => EntryKind::Default { then: then() },
        };
        let id = match e.kind {
            PreviewEntryKind::Rule => format!("{key}/entry-{n}"),
            PreviewEntryKind::Model if outcome == Outcome::Labels => decision::LABELS_MODEL_ID.to_string(),
            PreviewEntryKind::Model => format!("{key}/model"),
            PreviewEntryKind::Otherwise => format!("{key}/default"),
        };
        list.push(Entry { id, kind });
        positions.push(n);
    }
    (OutcomeList { outcome, entries: list }, positions)
}

/// A rule's key as the page names it. A key the core has no condition for matches nothing.
fn when_of(key: &str) -> When {
    let words = |w: &str| w.trim().to_string();
    match key {
        "vip" => When::Fact(Cond::FromVip),
        "list" => When::Fact(Cond::ListHeader),
        "me" => When::Fact(Cond::FromOwner),
        "reply" => When::Outcome(Outcome::NeedsMyReply, true),
        "code" => When::Any(["code", "verification", "sign in"].iter().map(|w| When::Fact(Cond::User(RuleCondition::SubjectContains { words: w.to_string() }))).collect()),
        k if k.starts_with("from:") => When::Fact(Cond::User(RuleCondition::From { words: words(&k[5..]) })),
        k if k.starts_with("subject:") => When::Fact(Cond::User(RuleCondition::SubjectContains { words: words(&k[8..]) })),
        _ => When::Fact(Cond::FromDomainIn(vec![])),
    }
}

/// The page's word for an answer, as the crate's value.
fn value_of(outcome: Outcome, word: &str, paths: &HashMap<String, String>, tabs: &TabSet) -> Value {
    match outcome {
        Outcome::WhichTab => Value::Tab(tabs.tab_for_choice(word).unwrap_or_else(|| TabRef::Builtin(word.trim().to_lowercase()))),
        Outcome::NotifyMe => Value::Yes(word.trim().eq_ignore_ascii_case("notify")),
        Outcome::Labels => Value::Label(paths.get(word.trim()).cloned()),
        Outcome::Vips | Outcome::NeedsMyReply | Outcome::SignInCodes => Value::Yes(word.trim().eq_ignore_ascii_case("yes")),
    }
}

/// A verdict's value in the page's words (what the row's chip shows).
fn words_of(outcome: Outcome, v: &decision::Verdict, paths: &HashMap<String, String>, tabs: &TabSet) -> String {
    if let Some(p) = &v.proposal {
        return p.path_words.replace(" > ", PAGE_SEP);
    }
    match &v.value {
        Value::Tab(t) => {
            let id = tabs.resolve(t);
            tabs.tabs.iter().find(|x| x.id == id).map(|x| x.name.clone()).unwrap_or(id)
        }
        Value::Yes(b) => match (outcome, *b) {
            (Outcome::NotifyMe, true) => "notify",
            (Outcome::NotifyMe, false) => "silent",
            (_, true) => "yes",
            (_, false) => "no",
        }
        .into(),
        Value::Label(Some(id)) => paths.get(id).cloned().unwrap_or_else(|| id.clone()),
        Value::Label(None) => "none".into(),
    }
}
