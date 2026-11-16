//! fix-teachcore, part B (docs/lanes/hid-ai.md F5; docs/DECISION-MODEL.md v2 §3 "Teaching lives on each decision page"): what each decision page shows under
//! its list ("7 corrections since Monday", "Make a rule from these", "Refit the model"), served by the real engine from what the store already records.
//!
//! WHERE A CORRECTION IS RECORDED TODAY: the tabs and the Needs Reply flag are built-in labels (migration 0004: `primary`, `needs-reply`, `updates`,
//! `promotions`, `social`), and the owner's hand on one (Label, MoveToLabel: actions `label_assign`) is a `label_assignment` row with `decided_by = 'owner'`
//! and its time (`updated_at_ms`; `decided_at_ms` when the label flow wrote it). Z takes the row away, so an undone correction is not counted. A rule, Laya,
//! Jev or the default write their own `decided_by`. So:
//! - `decision_teaching/<page>/corrections` (number): the owner's rows on the page's labels written in the last TEACHING_WINDOW_MS ("since a day");
//! - `decision_teaching/<page>/shared` (string, "" when none): what every one of those corrections shares, in this order: the sender address, its domain,
//!   the mailing list (`list:<the list's first List-Unsubscribe address>`: the store keeps no List-Id; two mails of one list carry the same header, the
//!   identity realcore.rs already uses for "the same list");
//! - `decision_teaching/examples` (number): every owner row on the labels a model answers for (Which tab, Needs my reply), any age: what Laya may be refitted on;
//! - `decision_teaching/refits` (number): the refits asked of this core (accepted by the gates below) since it opened.
//! The pages with no record of a correction (Sign-in codes: "confirm a code" writes nothing; VIPs, Notify me, Labels) answer 0 and "".
//!
//! `Command::RefitDecisionModel`: refused in words for Jev (no refit: "Make a rule from these" is its fix), for rules only, and under DECISION_REFIT_EXAMPLES
//! examples. Past those gates the ask is counted, and the answer says in words that this Mac cannot refit yet: the design's refit is Laya's `fit` and
//! `save_calibration` (DECISION-MODEL.md section 3), and no routine that fits a calibration exists in core/crates/decision or the on-device model
//! (Sources/Features/OnDeviceAI: the checkpoint's own temperatures only). Nothing here claims a refit happened.
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use contracts::{Command, CommandOutcome, CoreError, DecisionProviderKind};
use rusqlite::{params_from_iter, Connection};
use store::Store;

use crate::rust_core::now_ms;
use crate::sign_in::codes;

/// PROPOSED for docs/CONSTANTS.md (row DECISION_REFIT_EXAMPLES, value 35, source DECISION-MODEL.md v2 §3 "Laya: after 35 examples"): read from the ledger
/// through `decision::tuning::ledger_number` once the row exists; this number stands in for it until then.
pub const DECISION_REFIT_EXAMPLES: u64 = 35;

/// "since a day" (docs/lanes/hid-ai.md F5): the window of the corrections a page counts. PROPOSED for docs/CONSTANTS.md as DECISION_TEACHING_WINDOW_MS.
pub const TEACHING_WINDOW_MS: i64 = 24 * 60 * 60 * 1000;

/// The examples a refit needs: the ledger's row when it holds one, else DECISION_REFIT_EXAMPLES.
pub fn refit_examples() -> u64 {
    decision::tuning::ledger_number("DECISION_REFIT_EXAMPLES").map(|v| v as u64).unwrap_or(DECISION_REFIT_EXAMPLES)
}

pub const JEV_NO_REFIT_WORDS: &str = "Jev is not refitted: make a rule from these corrections instead.";
pub const RULES_NO_REFIT_WORDS: &str = "The model is off, so there is nothing to refit: choose Local in Settings, Decisions.";
/// The answer past the gates: the ask is counted, no refit is run (see the file's header).
pub const REFIT_NOT_ON_THIS_MAC_WORDS: &str = "Whoosh cannot refit the model on this Mac yet: your corrections are kept for when it can.";

/// The labels a page's corrections are made on (the built-in labels of migration 0004); None for a page that is not one of the six.
fn page_labels(page: &str) -> Option<&'static [&'static str]> {
    match page {
        "which-tab" => Some(&["primary", "updates", "promotions", "social"]),
        "needs-reply" => Some(&["needs-reply"]),
        "sign-in-codes" | "vips" | "notify-me" | "labels" => Some(&[]),
        _ => None,
    }
}

/// The labels a model answers for (the pages whose list ships "ask the model" and whose corrections are recorded): Laya's examples.
const EXAMPLE_LABELS: [&str; 5] = ["primary", "updates", "promotions", "social", "needs-reply"];

fn placeholders(n: usize) -> String { vec!["?"; n].join(",") }

/// The owner's corrections on `labels` since `since_ms`: (message id, sender address, the list's first unsubscribe address).
fn corrections(c: &Connection, labels: &[&str], since_ms: i64) -> rusqlite::Result<Vec<(String, String, Option<String>)>> {
    if labels.is_empty() { return Ok(vec![]); }
    let sql = format!(
        "SELECT m.id, m.from_address, (SELECT j.value FROM json_each(m.list_unsubscribe_json) j ORDER BY j.key LIMIT 1) \
         FROM label_assignment a JOIN message m ON m.id = a.message_id \
         WHERE a.label_id IN ({}) AND a.decided_by = 'owner' AND COALESCE(a.decided_at_ms, a.updated_at_ms) >= ? \
         ORDER BY m.id",
        placeholders(labels.len())
    );
    let mut st = c.prepare_cached(&sql)?;
    let args: Vec<rusqlite::types::Value> = labels.iter().map(|l| rusqlite::types::Value::Text(l.to_string())).chain(std::iter::once(rusqlite::types::Value::Integer(since_ms))).collect();
    let rows = st.query_map(params_from_iter(args), |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, Option<String>>(2)?)))?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

/// Every owner row on the labels a model answers for, any age.
fn examples(c: &Connection) -> rusqlite::Result<u64> {
    let sql = format!("SELECT count(*) FROM label_assignment WHERE label_id IN ({}) AND decided_by = 'owner'", placeholders(EXAMPLE_LABELS.len()));
    c.query_row(&sql, params_from_iter(EXAMPLE_LABELS.iter()), |r| r.get::<_, i64>(0)).map(|n| n.max(0) as u64)
}

fn domain_of(address: &str) -> Option<String> {
    address.rsplit_once('@').map(|(_, d)| d.trim().to_lowercase()).filter(|d| !d.is_empty())
}

/// What every correction shares: the sender address, else its domain, else the list; "" when they share none (or there are none).
pub fn shared(rows: &[(String, String, Option<String>)]) -> String {
    let Some(first) = rows.first() else { return String::new() };
    let address = first.1.trim().to_lowercase();
    if !address.is_empty() && rows.iter().all(|r| r.1.trim().to_lowercase() == address) { return address; }
    if let Some(d) = domain_of(&first.1) {
        if rows.iter().all(|r| domain_of(&r.1).as_deref() == Some(d.as_str())) { return d; }
    }
    if let Some(list) = first.2.as_deref().filter(|l| !l.is_empty()) {
        if rows.iter().all(|r| r.2.as_deref() == Some(list)) { return format!("list:{list}"); }
    }
    String::new()
}

fn json_string(s: &str) -> String {
    let mut o = String::from("\"");
    for c in s.chars() {
        match c { '"' => o.push_str("\\\""), '\\' => o.push_str("\\\\"), c if (c as u32) < 0x20 => o.push_str(&format!("\\u{:04x}", c as u32)), c => o.push(c) }
    }
    o.push('"');
    o
}

/// The refits asked, per store file, since this process opened it (the count the checks read; no refit is run: see the header).
fn refits() -> &'static Mutex<HashMap<PathBuf, u64>> {
    static R: OnceLock<Mutex<HashMap<PathBuf, u64>>> = OnceLock::new();
    R.get_or_init(Default::default)
}

fn refits_of(store: &Store) -> u64 { refits().lock().unwrap_or_else(|e| e.into_inner()).get(store.path()).copied().unwrap_or(0) }

fn unexpected(what: &str, e: impl std::fmt::Display) -> CoreError { CoreError::Unexpected { reason_words: format!("{what}: {e}") } }

/// A read-only connection of its own (the decision preview's way: `smart_inbox::load::open_reader`), so a state read never waits on the writer.
fn reader(store: &Store) -> Option<Connection> {
    match smart_inbox::load::open_reader(store.path()) {
        Ok(c) => Some(c),
        Err(e) => { eprintln!("decision teaching: the mail database could not be opened for reading: {e}"); None }
    }
}

/// The state paths of the teaching block (one dispatch line in `RustCore::state`), each ONE JSON value; None for another path or an unknown page.
pub(crate) fn state(store: &Store, path: &str) -> Option<String> {
    let p: Vec<&str> = path.split('/').collect();
    match p.as_slice() {
        ["decision_teaching", "examples"] => {
            let c = reader(store)?;
            examples(&c).map_err(|e| eprintln!("decision teaching: the examples could not be counted: {e}")).ok().map(|n| n.to_string())
        }
        ["decision_teaching", "refits"] => Some(refits_of(store).to_string()),
        ["decision_teaching", page, leaf @ ("corrections" | "shared")] => {
            let labels = page_labels(page)?;
            let c = reader(store)?;
            let rows = corrections(&c, labels, now_ms() - TEACHING_WINDOW_MS).map_err(|e| eprintln!("decision teaching: the corrections could not be read: {e}")).ok()?;
            Some(if *leaf == "corrections" { rows.len().to_string() } else { json_string(&shared(&rows)) })
        }
        _ => None,
    }
}

/// The command this file serves (one dispatch line in `RustCore::run`).
pub(crate) fn is_command(c: &Command) -> bool { matches!(c, Command::RefitDecisionModel) }

/// `Command::RefitDecisionModel`: the gates in order (the model in force, then the examples), then the counted ask and its answer in words.
pub(crate) fn run(store: &Store, command: Command) -> Result<CommandOutcome, CoreError> {
    if !is_command(&command) { return Err(CoreError::Invalid { field: "command".into(), reason_words: format!("{command:?} is not a teaching command") }); }
    // the saved choice; none saved = the shipped engine, Local (decision_model.rs `LabelModel::chosen`)
    match crate::decision_model::read(store).map(|m| m.provider) {
        Some(DecisionProviderKind::Jev) => return Err(CoreError::Invalid { field: "provider".into(), reason_words: JEV_NO_REFIT_WORDS.into() }),
        Some(DecisionProviderKind::Rules) => return Err(CoreError::Invalid { field: "provider".into(), reason_words: RULES_NO_REFIT_WORDS.into() }),
        // Apple's on-device model is never a saved choice (the pane offers Local, Jev and Rules); it has nothing to refit either
        Some(DecisionProviderKind::Apple) => return Err(CoreError::Invalid { field: "provider".into(), reason_words: JEV_NO_REFIT_WORDS.replace("Jev", "Apple Intelligence") }),
        Some(DecisionProviderKind::Local) | None => {}
    }
    let c = smart_inbox::load::open_reader(store.path()).map_err(|e| unexpected("the mail database could not be opened for reading", e))?;
    let have = examples(&c).map_err(|e| unexpected("the examples could not be counted", e))?;
    let need = refit_examples();
    if have < need {
        return Err(CoreError::Invalid { field: "examples".into(), reason_words: format!("A refit needs {need} corrections; you have {have} so far.") });
    }
    *refits().lock().unwrap_or_else(|e| e.into_inner()).entry(store.path().to_path_buf()).or_insert(0) += 1;
    crate::log::log_line(&crate::log::path_beside(store.path()), &format!("decision refit: asked on {have} examples; not run: no calibration fit exists on this Mac"));
    Err(codes::refusal(REFIT_NOT_ON_THIS_MAC_WORDS, codes::NOT_BUILT_YET, ""))
}
