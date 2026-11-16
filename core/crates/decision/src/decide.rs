//! Use case: decide one message on every page the owner switched on. One engine call per message carries every model
//! question a page may reach (a call costs its slowest question, so they are batched: retrieval-dossier-jev.md Need 4).
//! A switched-off page is never walked and its question is never sent.
//!
//! Delta 17: the Labels page is a label flow (`crate::flow`): its rule entries are the entry cards, its model entry offers only the
//! labels whose AI card is on and reads the inputs they tick, and the label the model picks passes its exit card before the verdict
//! is returned (`Verdict.flow` names the card and the chain). When those inputs differ from what the other pages read (subject and
//! body), the Labels question goes in a second engine call with its own state; with the shipped inputs it rides the one call.
use crate::entry::{all_hold, EntryKind, LabelNode, OutcomeList, TabOption, Value, LABEL_PATH_SEP};
use crate::flow::{of_verdict, offered_paths, offered_reads, offered_rest_of_thread};
use crate::labels::meaning_prompt;
use crate::outcome::Outcome;
use crate::settings::DecisionSettings;
use crate::thread_context::{encode, THREAD_FIELD};
use crate::tuning::decision_body_chars;
use crate::verdict::{DecidedByEntry, LabelProposal, Source, Verdict};
use contracts::{default_ai_reads, AiReads, CoreError, DecisionAnswer, DecisionPort, DecisionProviderKind, DecisionQuestion, DecisionState};
use smart_inbox::MessageFacts;
use std::collections::HashMap;

pub struct DecideMessage<'a> {
    pub settings: &'a DecisionSettings,
    /// The engine behind every model entry; None or `engine = Rules` = Engine Off.
    pub engine: Option<&'a dyn DecisionPort>,
    /// The tabs the "Which tab" model may answer.
    pub tabs: &'a [TabOption],
    /// The owner's label tree (the "Labels" model answers a path in it, or proposes a new child). Per message, only the
    /// labels visible to its account are offered and read (a shared label always; an account's label on that account).
    pub labels: &'a [LabelNode],
}

#[derive(Debug, Clone, PartialEq)]
pub struct Decisions {
    /// One verdict per switched-on page, in `Outcome::ALL` order.
    pub verdicts: Vec<Verdict>,
    /// The model questions sent (0 when no model entry could be reached or the engine is off).
    pub questions_sent: Vec<DecisionQuestion>,
    /// The engine's refusal or failure in its own words; every model entry then fell through. Lane fix-aisort: also set when the engine
    /// answered the Labels question in a way that cannot be read (`unreadable`): no answer for it, an answer of another kind, or a
    /// probability that is not a number from 0 to 1. Such an answer is no judgement, so no caller may keep it as the model's "no".
    pub engine_error: Option<CoreError>,
    /// Lane fix-aisort: the model entries whose answer was read and fell under their trust level, with the probability read (the log
    /// says them, so a trust level that no answer reaches shows in core.log; never mail text).
    pub below_trust: Vec<(String, f64)>,
    /// Lane fix-aisort round 2: per offered label (its id), the probability the Labels answer gave it: its share of the one choice, or with
    /// `tuning::labels_per_label` the yes-probability of its own question. Counts for core.log (`label_ai_failure::note_run`); never mail text.
    pub label_scores: Vec<(String, f64)>,
}

impl Decisions {
    pub fn get(&self, o: Outcome) -> Option<&Verdict> {
        self.verdicts.iter().find(|v| v.outcome == o)
    }
}

/// What reaches the engine: from, to, cc, subject, list header yes or no, the first DECISION_BODY_CHARS of the body
/// (docs/DECISION-MODEL.md appendix, "What leaves the device for Jev"). Never attachments, never the thread.
pub fn state_of(f: &MessageFacts) -> DecisionState {
    state_for(f, default_ai_reads())
}

/// The state with the inputs a label flow ticks (delta 17, « L'IA lit »): the subject only when ticked, the body only when ticked,
/// and when the attachments are ticked the one attachment fact the synced columns hold ("attachments: yes/no"; the facts carry no
/// file names or text yet, a gap named in docs/lanes/decision-r2.md). The sender, recipients and list header always go.
pub fn state_for(f: &MessageFacts, reads: AiReads) -> DecisionState {
    let mut fields = HashMap::new();
    fields.insert("from".to_string(), f.from_address.clone());
    fields.insert("to".to_string(), f.to.join(", "));
    fields.insert("cc".to_string(), f.cc.join(", "));
    if reads.subject {
        fields.insert("subject".to_string(), f.subject.clone());
    }
    fields.insert("list_header".to_string(), if f.has_list_header { "yes" } else { "no" }.to_string());
    if reads.attachments {
        fields.insert("attachments".to_string(), if f.has_attachment { "yes" } else { "no" }.to_string());
    }
    let text = if reads.body { f.body.chars().take(decision_body_chars()).collect() } else { String::new() };
    DecisionState { text, fields }
}

/// `state_for` plus the rest of the thread (lane labels-r5): when the label flow ticks it and the facts carry earlier messages
/// (`smart_inbox::thread::attach_thread`), they ride in the state field `thread`, oldest first and NOT yet cut: each engine cuts them to
/// its own model's budget (`crate::thread_context::fit`), which is the only place that knows it. Off = the state `state_for` gives.
pub fn state_with_thread(f: &MessageFacts, reads: AiReads, rest_of_thread: bool) -> DecisionState {
    let mut state = state_for(f, reads);
    if rest_of_thread && !f.thread_before.is_empty() {
        state.fields.insert(THREAD_FIELD.to_string(), encode(&f.thread_before));
    }
    state
}

fn question_for(list: &OutcomeList, entry_id: &str, question: &str, tabs: &[TabOption], labels: &[LabelNode]) -> DecisionQuestion {
    let id = entry_id.to_string();
    let prompt = question.to_string();
    match list.outcome {
        Outcome::WhichTab => DecisionQuestion::Choice { id, prompt, options: tabs.iter().map(|t| t.name.clone()).collect() },
        Outcome::Labels => {
            // delta 17: only the labels whose AI card is on are offered (a parent's name still builds its child's path)
            let mut options: Vec<String> = offered_paths(labels).into_keys().collect();
            options.sort();
            // the labels' meanings ride in the instructions: the contract's choice carries option names only
            DecisionQuestion::Choice { id, prompt: meaning_prompt(&prompt, labels), options }
        }
        _ => DecisionQuestion::Noul { id, prompt },
    }
}

/// Lane fix-aisort round 11 (owner 2026-11-12 23:44, "Why here doesn't"): the code a judge puts on a failure that belongs to ONE mail (its text
/// refused for content, its language, its size, or an answer with no label of the list): asking again gives the same answer, so the mail is SET
/// ASIDE once instead of waiting for ever. Carried on `ActionFailed.code` (the contract's own field), the reason in `reason_words` as an id.
pub const MAIL_REFUSED_CODE: &str = "apple_mail_refused";

/// The reason id of a failure that belongs to one mail (`MAIL_REFUSED_CODE`); None for a failure of the model itself (down, a crash, a time-out).
pub fn mail_refusal(e: &CoreError) -> Option<String> {
    match e {
        CoreError::Action { failure } if failure.code.as_deref() == Some(MAIL_REFUSED_CODE) => Some(failure.reason_words.clone()),
        _ => None,
    }
}

/// Lane fix-aisort round 2: the per-label question (`tuning::labels_per_label`). Its id is the model entry's, `#`, the label id.
pub const LABEL_QUESTION_SEP: char = '#';

pub fn label_question_id(entry_id: &str, label_id: &str) -> String {
    format!("{entry_id}{LABEL_QUESTION_SEP}{label_id}")
}

/// The model entry a question id belongs to (a per-label question's id carries the label after `#`).
pub fn entry_of_question(question_id: &str) -> &str {
    question_id.split_once(LABEL_QUESTION_SEP).map_or(question_id, |(e, _)| e)
}

/// The statement a per-label yes/no question asks about (a noul's default wording is "yes, the statement holds"): the label's path and
/// its description, in the owner's words.
pub fn label_statement(path: &str, description: &str) -> String {
    format!("This email belongs under the label \"{path}\": {}", description.trim())
}

/// The label path and description a per-label statement names (`label_statement` read back); None for any other text.
pub fn statement_label(statement: &str) -> Option<(&str, &str)> {
    let rest = statement.strip_prefix("This email belongs under the label \"")?;
    let (path, description) = rest.split_once("\": ")?;
    Some((path, description))
}

/// The probability that THIS label applies, from its own yes/no answer: the provider's number on a noul is the yes-probability (Laya:
/// LayaDecisionPort.swift `.noul(id, yes: yes >= 0.5, probability: yes)`). Any other form answers None (unreadable).
pub fn label_applies(a: &DecisionAnswer) -> Option<f64> {
    match a {
        DecisionAnswer::Noul { probability, .. } => Some(*probability),
        _ => None,
    }
}

/// The per-label questions of the Labels model entry, one per offered label (path order).
fn per_label_questions(entry_id: &str, labels: &[LabelNode]) -> Vec<DecisionQuestion> {
    let mut paths: Vec<(String, String)> = offered_paths(labels).into_iter().collect();
    paths.sort();
    paths
        .into_iter()
        .filter_map(|(path, id)| {
            let node = labels.iter().find(|l| l.id == id)?;
            Some(DecisionQuestion::Noul { id: label_question_id(entry_id, &id), prompt: label_statement(&path, &node.description) })
        })
        .collect()
}

/// Reads the per-label answers of the entry: Ok((label id, path, yes-probability)) for each question that went in a returned call, in path
/// order; Err(why) when one of them cannot be read (`unreadable`). Ok(empty) when none was sent.
pub fn read_per_label(entry_id: &str, paths: &HashMap<String, String>, returned: &[DecisionQuestion], answers: &HashMap<String, DecisionAnswer>) -> Result<Vec<(String, String, f64)>, String> {
    let mut sorted: Vec<(&String, &String)> = paths.iter().collect();
    sorted.sort();
    let mut out = Vec::new();
    for (path, id) in sorted {
        let qid = label_question_id(entry_id, id);
        let Some(sent) = returned.iter().find(|q| crate::adapters::wire::question_id(q) == qid) else { continue };
        let a = answers.get(&qid);
        let p = a.and_then(label_applies);
        if let Some(why) = unreadable(Some(sent), a, p) {
            return Err(why);
        }
        if let Some(p) = p {
            out.push((id.clone(), path.clone(), p));
        }
    }
    Ok(out)
}

struct Read {
    value: Value,
    words: String,
    probability: f64,
    proposal: Option<LabelProposal>,
}

fn read_answer(outcome: Outcome, a: &DecisionAnswer, tabs: &[TabOption], paths: &HashMap<String, String>) -> Option<Read> {
    match a {
        DecisionAnswer::Noul { yes, probability, .. } => {
            // The provider's number is the yes-probability; the confidence in the answer given is its own side.
            let p = if *yes { *probability } else { 1.0 - probability };
            Some(Read { value: Value::Yes(*yes), words: if *yes { "yes" } else { "no" }.into(), probability: p, proposal: None })
        }
        DecisionAnswer::Choice { choice, probabilities, .. } => {
            let p = probabilities.get(choice).copied().unwrap_or(0.0);
            match outcome {
                Outcome::WhichTab => {
                    let t = tabs.iter().find(|t| t.name.eq_ignore_ascii_case(choice.trim()))?;
                    Some(Read { value: Value::Tab(t.tab.clone()), words: t.name.clone(), probability: p, proposal: None })
                }
                Outcome::Labels => {
                    let said = choice.trim();
                    if let Some(id) = paths.get(said) {
                        return Some(Read { value: Value::Label(Some(id.clone())), words: said.into(), probability: p, proposal: None });
                    }
                    // A new child under an existing parent: proposed, never written.
                    let (parent, name) = said.rsplit_once(LABEL_PATH_SEP.trim()).map(|(a, b)| (a.trim(), b.trim()))?;
                    let parent_id = paths.get(parent)?;
                    if name.is_empty() {
                        return None;
                    }
                    let proposal = LabelProposal { parent_id: parent_id.clone(), name: name.into(), path_words: said.into(), probability: p };
                    Some(Read { value: Value::Label(None), words: format!("a new label {said}"), probability: p, proposal: Some(proposal) })
                }
                _ => None,
            }
        }
        DecisionAnswer::Score { .. } => None,
    }
}

/// Lane fix-aisort: why the Labels model entry's answer cannot be read, None when it can. `sent` = its question went in a call that
/// returned (a refused call is the caller's `engine_error` already). A choice outside the offered labels is readable: it is the model
/// saying that none fits (`read_answer` gives None and the list falls through, a "no").
pub fn unreadable(sent: Option<&DecisionQuestion>, answer: Option<&DecisionAnswer>, read_probability: Option<f64>) -> Option<String> {
    let q = sent?;
    let Some(a) = answer else { return Some("the model gave no answer to the question".into()) };
    let same_kind = matches!((q, a), (DecisionQuestion::Choice { .. }, DecisionAnswer::Choice { .. }) | (DecisionQuestion::Score { .. }, DecisionAnswer::Score { .. }) | (DecisionQuestion::Noul { .. }, DecisionAnswer::Noul { .. }));
    if !same_kind {
        return Some("the model answered in another form than the question asked".into());
    }
    match read_probability {
        Some(p) if !(p.is_finite() && (0.0..=1.0).contains(&p)) => Some("the model's answer carries no probability from 0 to 1".into()),
        _ => None,
    }
}

impl DecideMessage<'_> {
    fn engine_on(&self) -> Option<&dyn DecisionPort> {
        if self.settings.engine == DecisionProviderKind::Rules {
            return None;
        }
        self.engine
    }

    /// Decides one message on every switched-on page. A140: the Labels page runs as the three-step pipeline
    /// (`crate::flow::decide_pipeline`), so no caller can skip step 1's filter on the AI; the plain walk is `walk`.
    pub fn execute(&self, f: &MessageFacts) -> Decisions {
        crate::flow::decide_pipeline(self, f)
    }

    /// The plain walk of the pages as handed over (the grammar of docs/DECISION-MODEL.md), with no A140 view of the Labels page. Only
    /// `flow::decide_pipeline` calls it, with the per-message view it built; everyone else calls `execute`.
    pub(crate) fn walk(&self, f: &MessageFacts) -> Decisions {
        let pages: Vec<&OutcomeList> = self.settings.pages.iter().filter(|p| p.enabled).map(|p| &p.list).collect();
        let visible: Vec<LabelNode> = self.labels.iter().filter(|l| l.visible_to(&f.account_id)).cloned().collect();

        // 1. Which model entries can a walk reach? Stop at a rule that surely matches or at the default.
        let none = HashMap::new();
        let mut questions = Vec::new();
        let per_label = crate::tuning::labels_per_label();
        if self.engine_on().is_some() {
            for list in &pages {
                for e in &list.entries {
                    match &e.kind {
                        EntryKind::Rule { when, .. } => {
                            if all_hold(when, f, &none).0 {
                                break;
                            }
                        }
                        EntryKind::Model { .. } if list.outcome == Outcome::Labels && per_label => questions.extend(per_label_questions(&e.id, &visible)),
                        EntryKind::Model { question, .. } => questions.push(question_for(list, &e.id, question, self.tabs, &visible)),
                        EntryKind::Default { .. } => break,
                    }
                }
            }
        }

        // 2. One engine call for all of them; the Labels question rides it unless its labels tick other inputs (delta 17), then it
        //    goes alone with its own state.
        let labels_reads = offered_reads(&visible);
        let labels_thread = offered_rest_of_thread(&visible) && !f.thread_before.is_empty();
        let (mut shared, mut own) = (Vec::new(), Vec::new());
        for q in &questions {
            let is_labels = pages.iter().any(|l| l.outcome == Outcome::Labels && l.entries.iter().any(|e| e.id == entry_of_question(crate::adapters::wire::question_id(q))));
            if is_labels && (labels_reads != default_ai_reads() || labels_thread) { own.push(q.clone()) } else { shared.push(q.clone()) }
        }
        let mut answers: HashMap<String, DecisionAnswer> = HashMap::new();
        let mut provider = self.settings.engine;
        let mut engine_error = None;
        // lane fix-aisort: the questions whose call returned (answered or not): a missing answer to one of them is unreadable
        let mut returned: Vec<DecisionQuestion> = Vec::new();
        if let Some(port) = self.engine_on() {
            for (state, batch) in [(state_of(f), shared), (state_with_thread(f, labels_reads, labels_thread), own)] {
                if batch.is_empty() {
                    continue;
                }
                let sent = batch.clone();
                match port.decide(state, batch) {
                    Ok(r) => {
                        returned.extend(sent);
                        provider = r.answered_by;
                        for a in r.answers {
                            let id = match &a {
                                DecisionAnswer::Choice { id, .. } | DecisionAnswer::Score { id, .. } | DecisionAnswer::Noul { id, .. } => id.clone(),
                            };
                            answers.insert(id, a);
                        }
                    }
                    Err(e) => {
                        if engine_error.is_none() {
                            engine_error = Some(e);
                        }
                    }
                }
            }
        }

        // 3. Walk each page, first match wins; earlier pages' answers feed later conditions.
        let paths = offered_paths(&visible);
        let mut answered: HashMap<Outcome, Value> = HashMap::new();
        let mut verdicts = Vec::new();
        let mut below_trust = Vec::new();
        let mut label_scores = Vec::new();
        for list in pages {
            let mut found: Option<(Value, usize, Source, Option<LabelProposal>)> = None;
            for (i, e) in list.entries.iter().enumerate() {
                match &e.kind {
                    EntryKind::Rule { when, then } => {
                        if all_hold(when, f, &answered).0 {
                            found = Some((then.clone(), i, Source::Rule, None));
                        }
                    }
                    // lane fix-aisort round 2: one yes/no per offered label; the best yes-probability at or above the trust level files
                    EntryKind::Model { trust_from, question } if list.outcome == Outcome::Labels && per_label => match read_per_label(&e.id, &paths, &returned, &answers) {
                        Err(why) => {
                            if engine_error.is_none() {
                                engine_error = Some(CoreError::Unexpected { reason_words: format!("the model's answer could not be read: {why}") });
                            }
                        }
                        Ok(scores) => {
                            let asked = !scores.is_empty();
                            label_scores.extend(scores.iter().map(|(id, _, p)| (id.clone(), *p)));
                            let best = scores.into_iter().fold(None, |b: Option<(String, String, f64)>, s| if b.as_ref().map_or(true, |b| s.2 > b.2) { Some(s) } else { b });
                            if let Some((id, path, p)) = best {
                                if p >= *trust_from {
                                    found = Some((Value::Label(Some(id)), i, Source::Model { provider, answer: path, probability: p }, None));
                                } else {
                                    below_trust.push((e.id.clone(), p));
                                }
                            }
                            // round 4 (lead's ruling): no offered label reached the trust level and the engine answers a choice in free text
                            // (`DecisionSettings::choice_is_open`: Jev): the one choice is asked too, and ONLY a proposed new child is read from it
                            // (at the trust level, as in choice mode); its share never files an existing label. Laya is asked nothing more.
                            if found.is_none() && asked && self.settings.choice_is_open {
                                if let Some(port) = self.engine_on() {
                                    let q = question_for(list, &e.id, question, self.tabs, &visible);
                                    if let Ok(r) = port.decide(state_with_thread(f, labels_reads, labels_thread), vec![q]) {
                                        let read = r.answers.iter().find(|a| matches!(a, DecisionAnswer::Choice { id, .. } if *id == e.id)).and_then(|a| read_answer(list.outcome, a, self.tabs, &paths));
                                        if let Some(r) = read.filter(|r| r.proposal.is_some() && r.probability >= *trust_from) {
                                            found = Some((r.value, i, Source::Model { provider, answer: r.words, probability: r.probability }, r.proposal));
                                        }
                                    }
                                }
                            }
                        }
                    },
                    EntryKind::Model { trust_from, .. } => {
                        let read = answers.get(&e.id).and_then(|a| read_answer(list.outcome, a, self.tabs, &paths));
                        // lane fix-aisort round 2: each offered label's share of the one choice, for the spread in core.log
                        if let (Outcome::Labels, Some(DecisionAnswer::Choice { probabilities, .. })) = (list.outcome, answers.get(&e.id)) {
                            let mut shares: Vec<(String, f64)> = paths.iter().filter_map(|(path, id)| probabilities.get(path).map(|p| (id.clone(), *p))).collect();
                            shares.sort_by(|a, b| a.0.cmp(&b.0));
                            label_scores.extend(shares);
                        }
                        // lane fix-aisort: an answer to the Labels question that cannot be read is no judgement: it is the engine's failure
                        if list.outcome == Outcome::Labels {
                            let sent = returned.iter().find(|q| crate::adapters::wire::question_id(q) == e.id);
                            if let Some(why) = unreadable(sent, answers.get(&e.id), read.as_ref().map(|r| r.probability)) {
                                if engine_error.is_none() {
                                    engine_error = Some(CoreError::Unexpected { reason_words: format!("the model's answer could not be read: {why}") });
                                }
                                continue;
                            }
                        }
                        if let Some(r) = read {
                            if r.probability >= *trust_from {
                                let src = Source::Model { provider, answer: r.words, probability: r.probability };
                                found = Some((r.value, i, src, r.proposal));
                            } else {
                                below_trust.push((e.id.clone(), r.probability));
                            }
                        }
                    }
                    EntryKind::Default { then } => found = Some((then.clone(), i, Source::Default, None)),
                }
                if let Some((value, i, source, proposal)) = found.take() {
                    let entry_id = list.entries[i].id.clone();
                    let mut v = Verdict {
                        message_id: f.message_id.clone(),
                        outcome: list.outcome,
                        value,
                        decided_by: DecidedByEntry { page: list.outcome, entry_id, position: i + 1, source },
                        proposal,
                        flow: None,
                    };
                    // delta 17: the Labels verdict names its card and chain; the model's pick passes the exit card, which may send it on
                    if list.outcome == Outcome::Labels {
                        v.flow = of_verdict(&v, &visible, f);
                        if let Some(fl) = &v.flow {
                            v.value = fl.place.value();
                        }
                    }
                    answered.insert(list.outcome, v.value.clone());
                    verdicts.push(v);
                    break;
                }
            }
        }
        Decisions { verdicts, questions_sent: questions, engine_error, below_trust, label_scores }
    }
}
