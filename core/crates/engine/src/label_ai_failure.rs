//! Lane fix-aisort (owner 2026-11-12 15:02: "AI filter seeing 0 messgaes"): what the label AI could NOT judge, said once and asked again.
//!
//! - A judgement that failed (the port refused or erred) or that cannot be read (no answer, another form, no probability from 0 to 1:
//!   `decision::decide::unreadable`) is never kept as the AI's "no" (`file_arrivals::keep_the_nos` keeps only `AiStep::Asked`): the mail
//!   stays waiting (`label/<id>/flow/counts` `waiting_for_ai`).
//! - Each run of the label flow that hit such a failure writes ONE core.log line per label it touched, with the label id, the count waiting
//!   and the model's own words (never mail text, cut at `FAILURE_WORDS_MAX` characters); the same line is not written again until a
//!   judgement of that label succeeds or the words change.
//! - The label's `label/<id>/flow/ai_refusal` says it with one catalogue sentence (`AI_FAILED_WORDS`, code `AI_FAILED_CODE`), read by
//!   `decision_model::LabelModel::state` when nothing else refuses: the inspector shows it instead of a silent "0 accepted".
//! - A run where the model answered writes one core.log line with what it judged: accepted, under the trust level (with the highest
//!   probability read) and not judged, so a trust level that no answer reaches shows on the owner's Mac.
//! - The waiting mails are asked again in the background (`retry_ids`, on the filing worker: at the end of each sync run and once at start).
//! - Once per data folder, the "no" rows Laya wrote before this fix are cleared (`clear_old_nos_once`): nothing in a row tells a real "no"
//!   from an unreadable one. The log line gives the count; those mails are then waiting and judged again.
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use store::{Store, INBOX_FOLDER};

use crate::file_arrivals::{AiStep, Filed};

/// The code of `label/<id>/flow/ai_refusal` when the model could not judge the mail of the label (beside `ai_settings::AI_NO_MODEL_CODE`).
pub(crate) const AI_FAILED_CODE: &str = "ai_failed";
/// The sentence the inspector shows for it: a catalogue key (Sources/Resources/Localizable.xcstrings, proposed in docs/lanes/fix-aisort.md).
pub(crate) const AI_FAILED_WORDS: &str = "The AI could not judge the mail of this label. It tries again in the background; core.log says why.";
/// The most characters of the model's own failure words a log line keeps (a number of this lane, proposed as LABEL_AI_FAILURE_WORDS_MAX).
pub(crate) const FAILURE_WORDS_MAX: usize = 200;
/// The most waiting mails one background retry asks about (a number of this lane, proposed as LABEL_AI_RETRY_MAX): a retry is a background
/// job on the filing worker and the model takes its time per mail; the next sync run's retry takes the next ones.
pub(crate) const AI_RETRY_MAX: usize = 50;
/// The file in the data folder that records the one-time clean-up of the "no" rows written before this fix.
pub(crate) const CLEARED_MARKER: &str = "ai-label-no-cleared";

/// Per store (its path: tests open many), per label: the model's words of the last failed judgement, None after a success.
static FAILED: Mutex<Option<HashMap<PathBuf, HashMap<String, String>>>> = Mutex::new(None);

fn with_failed<T>(f: impl FnOnce(&mut HashMap<PathBuf, HashMap<String, String>>) -> T) -> T {
    let mut g = FAILED.lock().unwrap_or_else(|e| e.into_inner());
    f(g.get_or_insert_with(HashMap::new))
}

/// The model's words of the last failed judgement of `label_id` on this store, None when its last judgement succeeded or none failed.
pub(crate) fn failure_of(store_path: &Path, label_id: &str) -> Option<String> {
    with_failed(|m| m.get(store_path).and_then(|l| l.get(label_id).cloned()))
}

/// The words as the book keeps them (one line, at most `FAILURE_WORDS_MAX` characters).
pub(crate) fn cut_words(words: &str) -> String {
    cut(words)
}

fn cut(words: &str) -> String {
    let one_line = words.replace(['\n', '\r'], " ");
    if one_line.chars().count() <= FAILURE_WORDS_MAX { one_line } else { format!("{}...", one_line.chars().take(FAILURE_WORDS_MAX).collect::<String>()) }
}

/// After one run of the label flow (`file_arrivals::run_with_book`): records and logs what could not be judged, clears what was, and logs
/// what the model answered. `candidates` = per message, the AI-on labels whose step 1 it passed; `below` = per message, the probabilities
/// read under the trust level; `trust` = the trust level in force.
pub(crate) fn note_run(store: &Store, out: &[Filed], candidates: &HashMap<String, Vec<String>>, below: &HashMap<String, Vec<f64>>, accepted: usize, trust: Option<f64>) {
    let log = crate::log::path_beside(store.path());
    let mut failed: HashMap<String, String> = HashMap::new();
    let mut judged: HashSet<String> = HashSet::new();
    let mut aside: SetAsideCounts = Default::default();
    let (mut asked, mut not_judged) = (0usize, 0usize);
    for f in out {
        let labels = candidates.get(&f.message_id).cloned().unwrap_or_default();
        match &f.ai {
            AiStep::Refused(words) => {
                not_judged += 1;
                for l in labels { failed.entry(l).or_insert_with(|| cut(words)); }
            }
            // no model to ask: the mail waits too, and the log says it once (the card already says `NO_MODEL_WORDS`)
            AiStep::NoProvider => {
                not_judged += 1;
                for l in labels { failed.entry(l).or_insert_with(|| cut(crate::decision_model::NO_MODEL_WORDS)); }
            }
            AiStep::Asked => {
                asked += 1;
                judged.extend(labels);
            }
            // round 11: a mail the judge refused for itself is judged as far as the label goes: no failure for the card
            AiStep::SetAside(reason) => {
                for l in &labels { *aside.entry(l.clone()).or_default().entry(reason.clone()).or_default() += 1; }
                judged.extend(labels);
            }
            _ => {}
        }
    }
    log_set_aside(store, &aside);
    if failed.is_empty() && judged.is_empty() {
        return;
    }
    let waiting: HashMap<String, usize> = if failed.is_empty() { HashMap::new() } else { crate::label_flow::waiting_for_ai(store).into_iter().map(|(l, ids)| (l, ids.len())).collect() };
    let mut lines = Vec::new();
    with_failed(|m| {
        let book = m.entry(store.path().to_path_buf()).or_default();
        for l in &judged {
            if !failed.contains_key(l) { book.remove(l); }
        }
        let mut ids: Vec<&String> = failed.keys().collect();
        ids.sort();
        for l in ids {
            let words = &failed[l];
            if book.get(l) != Some(words) {
                lines.push(format!("label AI {l}: {} mails wait for the AI, which could not judge them: {words}", waiting.get(l).copied().unwrap_or(0)));
            }
            book.insert(l.clone(), words.clone());
        }
    });
    for line in lines {
        crate::log::log_line(&log, &line);
    }
    if asked > 0 {
        let probs: Vec<f64> = below.values().flatten().copied().collect();
        let under = below.values().filter(|v| !v.is_empty()).count();
        let highest = probs.iter().copied().fold(None, |m: Option<f64>, p| Some(m.map_or(p, |m| m.max(p))));
        let trust = trust.map_or("none".to_string(), |t| format!("{t:.2}"));
        let highest = highest.map_or("none".to_string(), |h| format!("{h:.2}"));
        crate::log::log_line(&log, &format!("label AI: judged {asked} mails: {accepted} accepted, {under} under the trust level {trust} (highest probability {highest}), {not_judged} not judged"));
    }
}

/// Per label, per reason id: how many mails the judge refused for themselves in one run (round 11).
pub(crate) type SetAsideCounts = std::collections::BTreeMap<String, std::collections::BTreeMap<String, usize>>;

/// Round 11: ONE core.log line per run for the mails set aside, the count per label and the judge's reason ids (never mail text); nothing
/// when none was set aside.
pub(crate) fn log_set_aside(store: &Store, aside: &SetAsideCounts) {
    if aside.is_empty() {
        return;
    }
    let parts: Vec<String> = aside
        .iter()
        .map(|(l, rs)| format!("{l} {} ({})", rs.values().sum::<usize>(), rs.iter().map(|(r, k)| format!("{r} {k}")).collect::<Vec<_>>().join(", ")))
        .collect();
    crate::log::log_line(&crate::log::path_beside(store.path()), &format!("label AI: mails set aside that the judge would not read, kept so they are not asked again: {}", parts.join("; ")));
}

/// After one step of a label's Re-run (`flow_ctx::rerun_label`): the model's words when a judgement failed, and whether any row was judged.
pub(crate) fn note_rerun(store: &Store, label_id: &str, failed: Option<String>, judged: bool) {
    let line = with_failed(|m| {
        let book = m.entry(store.path().to_path_buf()).or_default();
        match failed.map(|w| cut(&w)) {
            Some(words) => {
                let new = book.get(label_id) != Some(&words);
                book.insert(label_id.to_string(), words.clone());
                new.then_some(words)
            }
            None => {
                if judged { book.remove(label_id); }
                None
            }
        }
    });
    if let Some(words) = line {
        let waiting = crate::label_flow::waiting_for_ai(store).into_iter().find(|(l, _)| l == label_id).map_or(0, |(_, ids)| ids.len());
        crate::log::log_line(&crate::log::path_beside(store.path()), &format!("label AI {label_id}: {waiting} mails wait for the AI, which could not judge them: {words}"));
    }
}

/// Lane fix-aisort round 2: the code of `label/<id>/flow/ai_refusal` when the AI judged the label's mail and was sure of none.
pub(crate) const AI_UNDER_TRUST_CODE: &str = "ai_under_trust";
/// Its sentence, a catalogue key with three numbers (judged, the best in percent, the trust level in percent) the shell fills
/// (`Words.text(key, judged, highest, trust)`; the finding is in docs/lanes/fix-aisort.md, round 2).
pub(crate) const AI_UNDER_TRUST_WORDS: &str = "The AI judged %d mails and was sure of none: its best was %d %%, and it files from %d %%.";
/// The bins of the spread line, upper bounds (the last bin is "at or above the trust level"). A fixed reading grid of this lane, not a
/// product number: it only says where the answers fall.
const SPREAD_EDGES: [f64; 5] = [0.2, 0.4, 0.5, 0.6, 0.8];

/// Per store, per label: (mails judged since its last acceptance, the highest probability among them), while no answer reached the trust level.
static UNDER: Mutex<Option<HashMap<PathBuf, HashMap<String, (usize, f64)>>>> = Mutex::new(None);

/// `(judged, highest)` of a label whose AI judged mail since its last acceptance and was sure of none; None once it accepted one.
pub(crate) fn under_trust_of(store_path: &Path, label_id: &str) -> Option<(usize, f64)> {
    UNDER.lock().unwrap_or_else(|e| e.into_inner()).get_or_insert_with(HashMap::new).get(store_path).and_then(|m| m.get(label_id).copied())
}

/// The spread line's bins for `ps` against `trust`: counts under 0.2, 0.2 to 0.4, 0.4 to 0.5, 0.5 to 0.6, 0.6 to the trust level, at or above it.
pub(crate) fn spread(ps: &[f64], trust: f64) -> [usize; 6] {
    let mut bins = [0usize; 6];
    for &p in ps {
        let k = if p >= trust { 5 } else { SPREAD_EDGES.iter().position(|e| p < *e).unwrap_or(4).min(4) };
        bins[k] += 1;
    }
    bins
}

/// After one run: per label, ONE core.log line with the spread of the probabilities its judged mails got (counts only, no mail text, no
/// ids), and the under-trust book the card reads (`label/<id>/flow/ai_refusal` code `ai_under_trust`).
pub(crate) fn note_spread(store: &Store, scores: &[(String, f64)], accepted_by_label: &HashMap<String, usize>, trust: Option<f64>) {
    if scores.is_empty() {
        return;
    }
    let t = trust.unwrap_or(1.0);
    let mut by_label: HashMap<&str, Vec<f64>> = HashMap::new();
    for (l, p) in scores {
        by_label.entry(l.as_str()).or_default().push(*p);
    }
    let mut labels: Vec<&&str> = by_label.keys().collect();
    labels.sort();
    let log = crate::log::path_beside(store.path());
    for l in labels {
        let ps = &by_label[*l];
        let b = spread(ps, t);
        crate::log::log_line(&log, &format!(
            "label AI: {l}: {} judged; probability under 0.2: {}, 0.2 to 0.4: {}, 0.4 to 0.5: {}, 0.5 to 0.6: {}, 0.6 to {t:.2}: {}, {t:.2} and over: {}",
            ps.len(), b[0], b[1], b[2], b[3], b[4], b[5]
        ));
    }
    // round 8: Apple's model answers 1 or 0 (a pick or not): no "sure of none" to report, and the book of an earlier judge goes
    if judge_of(store.path()).as_deref() == Some("apple") {
        UNDER.lock().unwrap_or_else(|e| e.into_inner()).get_or_insert_with(HashMap::new).remove(store.path());
        return;
    }
    let mut g = UNDER.lock().unwrap_or_else(|e| e.into_inner());
    let book = g.get_or_insert_with(HashMap::new).entry(store.path().to_path_buf()).or_default();
    for (l, ps) in &by_label {
        if accepted_by_label.get(*l).copied().unwrap_or(0) > 0 {
            book.remove(*l);
            continue;
        }
        let highest = ps.iter().copied().filter(|p| p.is_finite()).fold(0.0_f64, f64::max);
        if highest >= t {
            // an answer reached the trust level (another label was surer): the AI is not "sure of none" here
            book.remove(*l);
            continue;
        }
        let e = book.entry(l.to_string()).or_insert((0, 0.0));
        e.0 += ps.len();
        e.1 = e.1.max(highest);
    }
}

/// Round 8: the file beside the database naming the judge the kept "no" answers were given by (`laya` or `apple`).
pub(crate) const JUDGE_MARKER: &str = "ai-label-judge";

/// Per store: the judge of the last Labels answer (round 8). Apple's model answers 1 or 0, so it has no "unsure" to report as under the trust level.
static JUDGE: Mutex<Option<HashMap<PathBuf, String>>> = Mutex::new(None);

fn judge_of(store_path: &Path) -> Option<String> {
    JUDGE.lock().unwrap_or_else(|e| e.into_inner()).get_or_insert_with(HashMap::new).get(store_path).cloned()
}

/// Who a model's "no" is kept under (round 8): the port's provider as the filings name it (`flow_ctx::ai_decider`), except that the on-device
/// port's Labels answers come from Apple's model when the last one did (`note_judge`): then `DecidedBy::Apple` (migration 0031).
/// Lane decided-model: or `DecidedBy::Model` when the last answer came from a catalogue model other than Laya (the marker holds its id).
pub(crate) fn model_decider(store: &Store, port: Option<&std::sync::Arc<dyn contracts::DecisionPort>>) -> contracts::DecidedBy {
    let by = crate::flow_ctx::ai_decider(port);
    if by == contracts::DecidedBy::Laya && judge_of(store.path()).as_deref() == Some("apple") { contracts::DecidedBy::Apple } else { named_model(store, by) }
}

/// Lane decided-model (SPEC A133): a filing the on-device port made (`DecidedBy::Laya`, as the verdict names every local answer) is the
/// model's whose id the judge marker holds, when that is neither "laya" nor "apple" (`note_judge`, read from the answer's checkpoint
/// "model:<id>", `decision_model::MODEL_CHECKPOINT_PREFIX`). Any other decider is returned as it is.
pub(crate) fn named_model(store: &Store, by: contracts::DecidedBy) -> contracts::DecidedBy {
    if by != contracts::DecidedBy::Laya {
        return by;
    }
    match judge_of(store.path()) {
        Some(id) if !["laya", "apple", NO_JUDGE_YET, ""].contains(&id.as_str()) => contracts::DecidedBy::Model { model: id },
        _ => by,
    }
}

/// Lane decided-model: true when the judge marker holds a catalogue model other than Laya and Apple.
pub(crate) fn judged_by_model(store: &Store) -> bool {
    matches!(named_model(store, contracts::DecidedBy::Laya), contracts::DecidedBy::Model { .. })
}

/// Round 8 (lead's ruling): the kept "no" answers belong to the judge that gave them. When a Labels answer comes from another judge than the
/// marker names, every model's "no" goes (`Store::ai_no_clear_on_question` with the judge's name), the store's waiting mails are asked again
/// (`CLEARED`), the under-trust book empties, and one log line says it. No marker yet: Laya's answers are the ones kept (the marker is written,
/// nothing goes); Apple's first answer clears them. Called by `decision_model::LayaPort` after each answer to a per-label Labels question.
pub(crate) fn note_judge(store: &Store, judge: &str) {
    if judge_of(store.path()).as_deref() == Some(judge) {
        return;
    }
    JUDGE.lock().unwrap_or_else(|e| e.into_inner()).get_or_insert_with(HashMap::new).insert(store.path().to_path_buf(), judge.to_string());
    let marker = store.path().with_file_name(JUDGE_MARKER);
    let log = crate::log::path_beside(store.path());
    // no mark at all (a store this start did not mark, see `clear_without_judge_mark`): Laya's answers are the ones kept
    if !marker.exists() && judge == "laya" {
        if let Err(e) = std::fs::write(&marker, "laya\n") { crate::log::log_line(&log, &format!("label AI: the judge's mark could not be written: {e}")); }
        return;
    }
    match store.ai_no_clear_on_question(judge, &marker) {
        Ok(None) => {}
        Ok(Some((before, n))) => {
            UNDER.lock().unwrap_or_else(|e| e.into_inner()).get_or_insert_with(HashMap::new).remove(store.path());
            // round 9: the start's mark `none` (nothing judged since its clean-up) to the first judge, with nothing kept since: no news
            if n > 0 || before.as_deref() != Some(NO_JUDGE_YET) {
                CLEARED.lock().unwrap_or_else(|e| e.into_inner()).get_or_insert_with(HashSet::new).insert(store.path().to_path_buf());
                crate::log::log_line(&log, &format!(
                    "label AI: the judge changed from {} to {judge}: {n} kept \"no\" answers of the model were cleared; their mails are judged again in the background",
                    before.as_deref().unwrap_or("laya (no mark)")
                ));
            }
        }
        Err(e) => crate::log::log_line(&log, &format!("label AI: the kept \"no\" answers of the other judge were not cleared, tried again at the next answer: {e}")),
    }
}

/// The judge mark written at start by `clear_without_judge_mark`: no judge has answered since the kept "no" answers were cleared.
pub(crate) const NO_JUDGE_YET: &str = "none";

/// Round 9 (owner's Mac, main102: no `ai-label-judge` beside the database, every mail already holding Laya's kept "no", so no mail waited, no
/// question was asked, and Apple's model never gave the first answer that clears them): at START, when the judge mark is missing, every
/// model's kept "no" goes once, the store's waiting mails are asked again (`CLEARED`), one log line says it, and the mark is written as
/// `none` (so the next start does not clear again). The first answer then writes the judge's name (`note_judge`), and a later change of judge
/// still clears there. Runs after the question marker (`clear_on_question_change`), whose own clean-up and line come first.
pub(crate) fn clear_without_judge_mark(store: &Store) -> Option<usize> {
    let marker = store.path().with_file_name(JUDGE_MARKER);
    if marker.exists() {
        return None;
    }
    let log = crate::log::path_beside(store.path());
    match store.ai_no_clear_models() {
        Ok(n) => {
            UNDER.lock().unwrap_or_else(|e| e.into_inner()).get_or_insert_with(HashMap::new).remove(store.path());
            if n > 0 {
                CLEARED.lock().unwrap_or_else(|e| e.into_inner()).get_or_insert_with(HashSet::new).insert(store.path().to_path_buf());
                crate::log::log_line(&log, &format!("label AI: no judge is marked yet: {n} kept \"no\" answers of the model were cleared; their mails are judged again in the background"));
            }
            if let Err(e) = std::fs::write(&marker, format!("{NO_JUDGE_YET}\n")) {
                crate::log::log_line(&log, &format!("label AI: the judge's mark could not be written, the clean-up runs again at the next start: {e}"));
            }
            Some(n)
        }
        Err(e) => {
            crate::log::log_line(&log, &format!("label AI: the kept \"no\" answers were not cleared, tried again at the next start: {e}"));
            None
        }
    }
}

/// Round 11: per store, the labels a Save or a card change made pick mails the AI has not judged: their waiting mails are asked at once, batch
/// after batch (`filing_worker` `retry`), not at the next sync's end.
static PENDING: Mutex<Option<HashMap<PathBuf, HashSet<String>>>> = Mutex::new(None);

/// Round 11: the label's step 1 or AI step changed (a Save of the inspector, a card, its AI inputs): its waiting mails are to be judged now.
pub(crate) fn judge_label_now(store_path: &Path, label_id: &str) {
    PENDING.lock().unwrap_or_else(|e| e.into_inner()).get_or_insert_with(HashMap::new).entry(store_path.to_path_buf()).or_default().insert(label_id.to_string());
}

/// Whether this store's kept "no" were cleared at this start and its waiting mails are still to be asked (round 9: `set_laya_port` asks them).
pub(crate) fn cleared_pending(store_path: &Path) -> bool {
    CLEARED.lock().unwrap_or_else(|e| e.into_inner()).get_or_insert_with(HashSet::new).contains(store_path)
}

/// Stores whose old "no" rows were cleared at this start (`clear_old_nos_once`): every waiting mail of theirs is asked again once.
static CLEARED: Mutex<Option<HashSet<PathBuf>>> = Mutex::new(None);

/// Round 16: the on-device model can answer again (its weights arrived: an answer came after its failure, or the decision model was saved
/// again): every waiting mail of this store is asked again once, whatever label it waits for. A successful answer clears the failure mark
/// of its label (`failure_of`), so without this the mails that waited for that label would not be asked until the next sync's end.
pub(crate) fn ask_waiting_again(store_path: &Path) {
    CLEARED.lock().unwrap_or_else(|e| e.into_inner()).get_or_insert_with(HashSet::new).insert(store_path.to_path_buf());
}

/// The INBOX mails to ask again: waiting for the AI of a label whose last judgement failed (or of any AI-on label on a store whose old "no"
/// rows were cleared at this start), that no label holds and no rejection row names (so a new judgement can only file them, never move a
/// filing), the first `AI_RETRY_MAX` by id. A label that never failed is not asked again here: its mail was judged, or is asked on arrival.
pub(crate) fn retry_ids(store: &Store) -> Vec<String> {
    let cleared = CLEARED.lock().unwrap_or_else(|e| e.into_inner()).get_or_insert_with(HashSet::new).contains(store.path());
    let judge_now: HashSet<String> = PENDING.lock().unwrap_or_else(|e| e.into_inner()).get_or_insert_with(HashMap::new).get(store.path()).cloned().unwrap_or_default();
    let mut all: Vec<String> = crate::label_flow::waiting_for_ai(store)
        .into_iter()
        .filter(|(l, _)| cleared || judge_now.contains(l) || failure_of(store.path(), l).is_some())
        .flat_map(|(_, ids)| ids)
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    if all.is_empty() {
        CLEARED.lock().unwrap_or_else(|e| e.into_inner()).get_or_insert_with(HashSet::new).remove(store.path());
        PENDING.lock().unwrap_or_else(|e| e.into_inner()).get_or_insert_with(HashMap::new).remove(store.path());
        return vec![];
    }
    all.sort();
    let Ok(reader) = smart_inbox::load::open_reader(store.path()) else { return vec![] };
    let held = |id: &String| -> bool {
        reader
            .query_row("SELECT (SELECT COUNT(*) FROM label_assignment WHERE message_id = ?1) + (SELECT COUNT(*) FROM flow_rejection WHERE message_id = ?1)", rusqlite::params![id], |r| r.get::<_, i64>(0))
            .map_or(true, |n| n > 0)
    };
    all.into_iter()
        .filter(|id| store.message(id).ok().flatten().is_some_and(|m| m.folder == INBOX_FOLDER && !m.is_outgoing))
        .filter(|id| !held(id))
        .take(AI_RETRY_MAX)
        .collect()
}

/// The file beside the database that holds the label AI's question as `<form>|<trust>` (round 4).
pub(crate) const QUESTION_MARKER: &str = "ai-label-question";

/// The label AI's question as one line: its form (`per-label` or `choice`, `decision::tuning::labels_per_label`) and its trust level.
pub(crate) fn question_now() -> String {
    let form = if decision::tuning::labels_per_label() { "per-label" } else { "choice" };
    let trust = decision::tuning::model_trust_label().map_or("none".to_string(), |t| format!("{t}"));
    format!("{form}|{trust}")
}

/// Round 4 (lead's ruling, 12 November): at each start, when the question (`question_now`) differs from the one the kept "no" answers were
/// given under, they go: every model's "no" row, one log line with the count and both questions, the store's waiting mails asked again
/// (`CLEARED`), and the under-trust book of every label emptied (the card's old best share goes). Answers the rows removed, None when the
/// question is the same (or the clean-up failed: the log says; it is tried again at the next start).
pub(crate) fn clear_on_question_change(store: &Store) -> Option<usize> {
    let log = crate::log::path_beside(store.path());
    let now = question_now();
    match store.ai_no_clear_on_question(&now, &store.path().with_file_name(QUESTION_MARKER)) {
        Ok(None) => None,
        Ok(Some((before, n))) => {
            UNDER.lock().unwrap_or_else(|e| e.into_inner()).get_or_insert_with(HashMap::new).remove(store.path());
            if n > 0 || before.is_some() {
                CLEARED.lock().unwrap_or_else(|e| e.into_inner()).get_or_insert_with(HashSet::new).insert(store.path().to_path_buf());
                crate::log::log_line(&log, &format!(
                    "label AI: the question changed from {} to {now}: {n} kept \"no\" answers of the model were cleared; their mails are judged again in the background",
                    before.as_deref().unwrap_or("(none kept)")
                ));
            }
            Some(n)
        }
        Err(e) => {
            crate::log::log_line(&log, &format!("label AI: the kept \"no\" answers of an older question were not cleared, tried again at the next start: {e}"));
            None
        }
    }
}

/// Once per data folder: the "no" rows Laya wrote before this fix go (see the header), with one core.log line giving the count. Answers the
/// count (None: done before, or it failed: the log says; it is tried again at the next start).
pub(crate) fn clear_old_nos_once(store: &Store) -> Option<usize> {
    let log = crate::log::path_beside(store.path());
    let marker = store.path().with_file_name(CLEARED_MARKER);
    match store.ai_no_clear_once("laya", &marker) {
        Ok(Some(0)) => Some(0),
        Ok(Some(n)) => {
            CLEARED.lock().unwrap_or_else(|e| e.into_inner()).get_or_insert_with(HashSet::new).insert(store.path().to_path_buf());
            crate::log::log_line(&log, &format!("label AI: {n} kept \"no\" answers of Laya written before an unreadable answer stopped counting as a \"no\" were cleared once; their mails are judged again in the background"));
            Some(n)
        }
        Ok(None) => None,
        Err(e) => {
            crate::log::log_line(&log, &format!("label AI: the one-time clean-up of Laya's kept \"no\" answers was not done, tried again at the next start: {e}"));
            None
        }
    }
}
