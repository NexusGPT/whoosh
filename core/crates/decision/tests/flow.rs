//! Delta 17, the label flow (docs/PRD.md "A94 pick"; docs/lanes/contracts.md "Evaluation order the decision lane applies"; lead
//! rulings 2026-11-09: entry first-match by LABEL order, chains inside one label by rule position). One test per rule of the order:
//! (1) entry rules of every enabled label, first match by label order, the model never sees the message; (2) the AI over the
//! enabled labels, reading their ticked inputs; (3) the chosen label's exit filter, chains as alternatives, the fallback for the
//! rejected (Primary, Junk, a label, one hop), an empty filter keeps everything, an off card keeps the pick; (4) `flow` names the card
//! and the chain; a flipped fact re-asks entry and exit at once, never the model; the counts and "why here" read the filings.
//!
//! Mutants (single edits; each turns at least one test here red; unobserved, this round is uncompiled):
//! - M1 labels.rs `labels_from_records` sort: drop `ia.cmp(ib)` (position first) -> entry_first_match_wins_by_label_order
//! - M2 flow.rs `after_ai`: `if !label.exit_enabled` -> `if label.exit_enabled` -> exit_kept_by_a_chain..., exit_card_off_keeps_the_pick
//! - M3 flow.rs `after_ai` Rejected: `place` -> `here` (ignore the fallback) -> a_rejected_message_goes_to_primary..., ..._junk, ..._label
//! - M4 flow.rs `offered_paths`: drop `&& l.ai_enabled` -> the_ai_offers_only_labels_whose_ai_card_is_on
//! - M5 decide.rs `state_for`: `if reads.body` -> `if true` -> the_ai_reads_only_the_ticked_inputs
//! - M6 labels.rs `label_target`: `ChainPlacement::Entry` arm -> `_` (exit chains become entry rules) -> exit_kept_by_a_chain...
//! - M7 flip.rs step 2: `after_ai(node, f)` replaced by `return FlipAction::Keep` -> a_flipped_fact_moves_the_message_at_once
//!
//! A140 (owner 2026-11-11, the three-step pipeline; tests/pipeline.rs holds its own cases): an AI-on label's entry chains no longer
//! file; they pick what the AI is asked about. The tests below that are about the AI and the exit filter therefore run on labels whose
//! step 1 is open (entry card off: every mail passes), which is what A94's "no entry rule matched" was; the entry-rule tests run on
//! AI-off labels. Each test changed for A140 says so in a one-line comment.
mod support;
use contracts::{count_label_flow, ChainPlacement, DecidedBy, DecisionPort, DecisionProviderKind, DecisionQuestion, DecisionResult, DecisionState, CoreError, FlowFiling, MatchMode, MessageFact, Rule, RuleAction, RuleCondition, WhyCard};
use decision::flow::decide_pipeline;
use decision::{CurrentFiling, DecideMessage, DecisionSettings, FlipAction, FlowPlace, LabelNode, Outcome, OutcomeList, RefileOnFact, RerunLabel, Source, Value};
use smart_inbox::MessageFacts;
use std::sync::atomic::AtomicI64;
use std::sync::Mutex;
use support::*;

const TRUST: f64 = 0.7;
const REPLY: &str = "l-reply";
const CLIENTS: &str = "l-clients";
const NEWS: &str = "l-news";

fn fact(f: MessageFact, wanted: bool) -> RuleCondition {
    RuleCondition::Fact { fact: f, wanted }
}

fn chain(id: &str, position: u32, label: &str, placement: ChainPlacement, chain_id: u32, conditions: Vec<RuleCondition>, mode: MatchMode) -> Rule {
    Rule { id: id.into(), position, conditions, tab_id: String::new(), enabled: true, action: RuleAction::Label { label_id: label.into() }, match_mode: mode, placement, chain_id }
}

/// Needs reply: entry e2 (subject "contract", position 5); exit chain 0 unanswered yes, chain 1 starred yes; fallback Primary.
/// Clients: entry e1 (from "boss", position 1); exit chain 0 VIP sender yes; fallback Junk.
/// Newsletters: entry e3 (starred yes AND subject "digest"); exit filter on with NO chain; fallback Clients.
fn rules() -> Vec<Rule> {
    use ChainPlacement::{Entry, Exit};
    vec![
        chain("e1", 1, CLIENTS, Entry, 0, vec![RuleCondition::From { words: "boss".into() }], MatchMode::Any),
        chain("x1", 2, REPLY, Exit, 0, vec![fact(MessageFact::Unanswered, true)], MatchMode::All),
        chain("e3", 3, NEWS, Entry, 0, vec![fact(MessageFact::Starred, true), RuleCondition::SubjectContains { words: "digest".into() }], MatchMode::All),
        chain("x2", 4, REPLY, Exit, 1, vec![fact(MessageFact::Starred, true)], MatchMode::All),
        chain("e2", 5, REPLY, Entry, 0, vec![RuleCondition::SubjectContains { words: "contract".into() }], MatchMode::Any),
        chain("x3", 6, CLIENTS, Exit, 0, vec![fact(MessageFact::VipSender, true)], MatchMode::All),
    ]
}

/// The three labels in sidebar order, described, with their cards as above.
fn labels_with(edit: impl Fn(&mut Vec<LabelNode>)) -> Vec<LabelNode> {
    let mut reply = LabelNode::new(REPLY, "Needs reply", None);
    reply.description = "emails waiting for my answer".into();
    reply.exit_enabled = true;
    reply.exit_fallback = Some("primary".into());
    let mut clients = LabelNode::new(CLIENTS, "Clients", None);
    clients.description = "mail about a client deal".into();
    clients.exit_enabled = true;
    clients.exit_fallback = Some("junk".into());
    let mut news = LabelNode::new(NEWS, "Newsletters", None);
    news.description = "digests and newsletters".into();
    news.exit_enabled = true;
    news.exit_fallback = Some(CLIENTS.into());
    let mut v = vec![reply, clients, news];
    edit(&mut v);
    LabelNode::with_chains(&v, &rules())
}
/// A140: the same labels with step 1 open (entry card off) on each, so every mail reaches the AI.
fn open_with(edit: impl Fn(&mut Vec<LabelNode>)) -> Vec<LabelNode> {
    labels_with(|v| {
        for l in v.iter_mut() {
            l.entry_enabled = false;
        }
        edit(v)
    })
}
/// A140: step 1 open on every label (the A94 tests' "no entry rule matched" case).
fn labels() -> Vec<LabelNode> {
    open_with(|_| {})
}
/// A140: the step-1 tests' labels: Needs reply and Clients AI-off with their exit card off (step 3 would apply), Newsletters as drawn.
fn ai_off_with(edit: impl Fn(&mut Vec<LabelNode>)) -> Vec<LabelNode> {
    labels_with(|v| {
        for l in v.iter_mut().take(2) {
            l.ai_enabled = false;
            l.exit_enabled = false;
        }
        edit(v)
    })
}

fn settings(labels: &[LabelNode]) -> DecisionSettings {
    let mut s = DecisionSettings::shipped(DecisionProviderKind::Local, &[Outcome::Labels]);
    s.page_mut(Outcome::Labels).unwrap().list = OutcomeList::labels_from_records(labels, &rules(), Some(TRUST));
    s
}

/// The model: "?" in the subject is Needs reply, "deal" is Clients, "digest" is Newsletters (0.9 each).
fn model() -> std::sync::Arc<FakeLocal> {
    FakeLocal::new(|_id, _q, state| {
        let subject = subject_in(state);
        if subject.contains('?') {
            choice("Needs reply", 0.9)
        } else if subject.contains("deal") {
            choice("Clients", 0.9)
        } else if subject.contains("digest") {
            choice("Newsletters", 0.9)
        } else {
            choice("Clients", 0.1)
        }
    })
}

/// A message with the grid's facts: answered by default (unanswered false), not starred, not from a VIP.
fn m(id: &str, from: &str, subject: &str) -> MessageFacts {
    msg(id, from, subject, "the body")
}

fn verdict_with(labels: &[LabelNode], engine: &dyn DecisionPort, f: &MessageFacts) -> decision::Verdict {
    let s = settings(labels);
    // A140: the Labels page runs as the three-step pipeline every caller uses
    decide_pipeline(&DecideMessage { settings: &s, engine: Some(engine), tabs: &[], labels }, f).get(Outcome::Labels).cloned().expect("the Labels page is on")
}
fn verdict(engine: &dyn DecisionPort, f: &MessageFacts) -> decision::Verdict {
    verdict_with(&labels(), engine, f)
}

// ---------------------------------------------------------------- (1) entry rules

#[test]
fn entry_first_match_wins_by_label_order_and_the_model_never_sees_the_message() {
    let engine = Recording::new(local(model()));
    // both entry rules match; Clients' rule has the lower position, Needs reply is higher in the sidebar: the label order wins
    let f = m("m1", "boss@client-example.com", "contract renewal?");
    // A140: only AI-off labels file by step 1 (and step 3 would apply), so both are AI-off with the exit card off here
    let v = verdict_with(&ai_off_with(|_| {}), &engine, &f);
    assert_eq!(v.value, Value::Label(Some(REPLY.into())));
    assert_eq!((v.decided_by.entry_id.as_str(), &v.decided_by.source), ("rule:e2", &Source::Rule));
    let fl = v.flow.expect("an entry filing names its card");
    assert_eq!((fl.card, fl.chain_id, fl.ai_pick, fl.place), (WhyCard::Entry, Some(0), None, FlowPlace::Label(REPLY.into())));
    assert!(engine.ids().is_empty(), "a sure entry match stops the walk: nothing is asked");
    // and the entry filing is never put through the label's own exit filter: m1 is answered (unanswered no), it stays
}

#[test]
fn an_entry_card_switched_off_files_nothing_and_the_next_label_catches() {
    // A140: AI-off labels (only they file by step 1); an AI-off label with its entry card off files nothing
    let labels = ai_off_with(|v| v[0].entry_enabled = false);
    let engine = Recording::new(local(model()));
    let v = verdict_with(&labels, &engine, &m("m2", "boss@client-example.com", "contract renewal?"));
    assert_eq!((v.value, v.decided_by.entry_id.as_str()), (Value::Label(Some(CLIENTS.into())), "rule:e1"));
}

// ---------------------------------------------------------------- (2) the AI

#[test]
fn the_ai_offers_only_labels_whose_ai_card_is_on() {
    // A140: step 1 open on the AI-on labels, so the AI is asked about this message at all
    let labels = open_with(|v| v[2].ai_enabled = false);
    let engine = Recording::new(local(model()));
    let v = verdict_with(&labels, &engine, &m("m3", "list@news-example.com", "weekly digest"));
    assert_eq!(v.value, Value::Label(None), "the model's Newsletters is not an offered label: nothing is filed");
    let calls = engine.calls.lock().unwrap();
    // fix-aisort round 3: the Labels question in either form (one choice, or one yes/no per label: LABEL_AI_QUESTION)
    let call = calls.iter().find(|c| decision::test_answers::asks_labels(c)).expect("asked");
    assert_eq!(decision::test_answers::offered_labels(call), vec!["Clients".to_string(), "Needs reply".to_string()]);
    let prompt = decision::test_answers::label_meanings(call).join("\n");
    assert!(!prompt.contains("Newsletters"), "an AI card switched off adds no meaning line: {prompt}");
}

/// Records the state of every engine call.
struct States<P: DecisionPort> {
    inner: P,
    seen: Mutex<Vec<DecisionState>>,
}
impl<P: DecisionPort> DecisionPort for States<P> {
    fn provider(&self) -> DecisionProviderKind {
        self.inner.provider()
    }
    fn decide(&self, state: DecisionState, questions: Vec<DecisionQuestion>) -> Result<DecisionResult, CoreError> {
        self.seen.lock().unwrap().push(state.clone());
        self.inner.decide(state, questions)
    }
}

#[test]
fn the_ai_reads_only_the_ticked_inputs() {
    let mut f = m("m4", "pm@partner-example.com", "is the deal signed?");
    f.has_attachment = true;
    // shipped inputs (subject and body): the body goes, the attachments do not
    let engine = States { inner: local(model()), seen: Mutex::new(vec![]) };
    verdict(&engine, &f);
    let seen = engine.seen.lock().unwrap().clone();
    assert_eq!(seen.len(), 1);
    assert_eq!((seen[0].text.as_str(), seen[0].fields.get("subject").map(String::as_str), seen[0].fields.contains_key("attachments")), ("the body", Some("is the deal signed?"), false));
    // every label ticks subject and attachments, not the body
    // A140: step 1 open (`open_with`), so the message reaches the AI
    let labels = open_with(|v| for l in v.iter_mut() { l.ai_reads = contracts::AiReads { subject: true, body: false, rest_of_thread: false, attachments: true } });
    let engine = States { inner: local(model()), seen: Mutex::new(vec![]) };
    verdict_with(&labels, &engine, &f);
    let seen = engine.seen.lock().unwrap().clone();
    assert_eq!(seen.len(), 1, "the Labels page alone: one call, with the labels' own inputs");
    assert_eq!(seen[0].text, "", "the body is not ticked: it does not leave");
    assert_eq!(seen[0].fields.get("attachments").map(String::as_str), Some("yes"));
    assert_eq!(seen[0].fields.get("subject").map(String::as_str), Some("is the deal signed?"));
}

// ---------------------------------------------------------------- (3) the exit filter

#[test]
fn exit_kept_by_a_chain_names_the_exit_card_and_that_chain_and_chains_are_alternatives() {
    let engine = local(model());
    let mut waiting = m("m5", "pm@partner-example.com", "can you check?");
    waiting.unanswered = true;
    let fl = verdict(&engine, &waiting).flow.expect("a flow");
    assert_eq!((fl.card, fl.chain_id, fl.place), (WhyCard::Exit, Some(0), FlowPlace::Label(REPLY.into())));
    let mut starred = m("m6", "pm@partner-example.com", "can you check?");
    starred.starred = true;
    let v = verdict(&engine, &starred);
    assert!(matches!(v.decided_by.source, Source::Model { .. }), "the AI chose; the exit card kept it");
    let fl = v.flow.expect("a flow");
    assert_eq!((fl.card, fl.chain_id, fl.ai_pick.as_deref()), (WhyCard::Exit, Some(1), Some(REPLY)), "the second chain is an alternative");
}

#[test]
fn a_rejected_message_goes_to_primary_and_leaves_a_rejection_row() {
    let engine = local(model());
    let v = verdict(&engine, &m("m7", "pm@partner-example.com", "can you check?"));
    assert_eq!(v.value, Value::Label(None), "Primary carries no label");
    assert_eq!(v.decided_by.to_contract(), DecidedBy::Laya, "the AI decided; the exit card moved it");
    let fl = v.flow.expect("a flow");
    assert_eq!((fl.card, fl.chain_id, fl.ai_pick.as_deref(), &fl.place), (WhyCard::Fallback, None, Some(REPLY), &FlowPlace::Primary));
    let r = fl.rejection("m7", 42).expect("a rejection row");
    assert_eq!((r.ai_label_id.as_str(), r.sent_to.as_str(), r.decided_at_ms), (REPLY, "primary", 42));
}

#[test]
fn a_rejected_message_goes_to_junk_when_that_is_the_fallback() {
    let fl = verdict(&local(model()), &m("m8", "x@unknown-example.com", "a deal for you")).flow.expect("a flow");
    assert_eq!((fl.card, &fl.place, fl.rejection("m8", 1).map(|r| r.sent_to)), (WhyCard::Fallback, &FlowPlace::Junk, Some("junk".to_string())));
    let mut vip = m("m9", "ceo@client-example.com", "a deal for you");
    vip.from_vip = true;
    let fl = verdict(&local(model()), &vip).flow.expect("a flow");
    assert_eq!((fl.card, fl.chain_id, fl.place), (WhyCard::Exit, Some(0), FlowPlace::Label(CLIENTS.into())));
}

#[test]
fn a_rejected_message_goes_to_its_fallback_label_one_hop_only() {
    // Clients sends its rejects to Needs reply, whose own exit filter would reject this message too: it is not applied
    // A140: step 1 open (`open_with`), so the message reaches the AI
    let labels = open_with(|v| v[1].exit_fallback = Some(REPLY.into()));
    let v = verdict_with(&labels, &local(model()), &m("m10", "x@unknown-example.com", "a deal for you"));
    assert_eq!(v.value, Value::Label(Some(REPLY.into())));
    let fl = v.flow.expect("a flow");
    assert_eq!((fl.card, fl.ai_pick.as_deref(), fl.place), (WhyCard::Fallback, Some(CLIENTS), FlowPlace::Label(REPLY.into())));
}

#[test]
fn an_exit_filter_with_no_chain_keeps_everything() {
    let fl = verdict(&local(model()), &m("m11", "list@news-example.com", "weekly digest")).flow.expect("a flow");
    assert_eq!((fl.card, fl.chain_id, fl.place), (WhyCard::Exit, None, FlowPlace::Label(NEWS.into())));
}

#[test]
fn an_exit_card_switched_off_keeps_the_pick() {
    // A140: step 1 open (`open_with`), so the message reaches the AI
    let labels = open_with(|v| v[0].exit_enabled = false);
    let v = verdict_with(&labels, &local(model()), &m("m12", "pm@partner-example.com", "can you check?"));
    assert_eq!(v.value, Value::Label(Some(REPLY.into())));
    let fl = v.flow.expect("a flow");
    assert_eq!((fl.card, fl.chain_id, fl.ai_pick.as_deref()), (WhyCard::Ai, None, Some(REPLY)));
}

// ---------------------------------------------------------------- (4) the flip, "why here", the counts, the re-run

fn refile(f: &MessageFacts, now: CurrentFiling) -> FlipAction {
    refile_with(&labels(), f, now)
}
fn refile_with(l: &[LabelNode], f: &MessageFacts, now: CurrentFiling) -> FlipAction {
    let s = settings(l);
    RefileOnFact { settings: &s, tabs: &[], labels: l }.execute(f, &now)
}

#[test]
fn a_flipped_fact_moves_the_message_at_once_and_back_and_never_asks_the_model() {
    // in Needs reply by exit chain 0 (unanswered); the person replies: unanswered flips to no
    let replied = m("m13", "pm@partner-example.com", "can you check?");
    let now = CurrentFiling { place: FlowPlace::Label(REPLY.into()), card: WhyCard::Exit, chain_id: Some(0), ai_pick: Some(REPLY.into()), decided_by: DecidedBy::Laya };
    match refile(&replied, now) {
        FlipAction::Move { to, decided_by } => {
            assert_eq!((to.card, to.place, to.ai_pick.as_deref()), (WhyCard::Fallback, FlowPlace::Primary, Some(REPLY)));
            assert_eq!(decided_by, DecidedBy::Laya, "the AI's pick stands; only its place moved");
        }
        other => panic!("expected a move to Primary, got {other:?}"),
    }
    // a new message arrives in the thread: unanswered again, it goes back
    let mut waiting = replied.clone();
    waiting.unanswered = true;
    let now = CurrentFiling { place: FlowPlace::Primary, card: WhyCard::Fallback, chain_id: None, ai_pick: Some(REPLY.into()), decided_by: DecidedBy::Laya };
    match refile(&waiting, now) {
        FlipAction::Move { to, .. } => assert_eq!((to.card, to.chain_id, to.place), (WhyCard::Exit, Some(0), FlowPlace::Label(REPLY.into()))),
        other => panic!("expected a move back, got {other:?}"),
    }
    // nothing changed: it stays
    let now = CurrentFiling { place: FlowPlace::Label(REPLY.into()), card: WhyCard::Exit, chain_id: Some(0), ai_pick: Some(REPLY.into()), decided_by: DecidedBy::Laya };
    assert_eq!(refile(&waiting, now), FlipAction::Keep);
}

#[test]
fn a_flipped_fact_re_asks_the_entry_cards_but_never_moves_the_owners_filing() {
    let mut starred = m("m14", "list@news-example.com", "weekly digest");
    starred.starred = true;
    let by_default = CurrentFiling { place: FlowPlace::Primary, card: WhyCard::Hand, chain_id: None, ai_pick: None, decided_by: DecidedBy::Default };
    // A140: only an AI-off label's step 1 files on a flip, so Newsletters is AI-off here (its exit filter has no chain: it keeps)
    let news_off = labels_with(|v| v[2].ai_enabled = false);
    match refile_with(&news_off, &starred, by_default) {
        FlipAction::Move { to, decided_by } => {
            assert_eq!((to.card, to.chain_id, to.place), (WhyCard::Entry, Some(0), FlowPlace::Label(NEWS.into())));
            assert_eq!(decided_by, DecidedBy::Rule { rule_id: "rule:e3".into() });
        }
        other => panic!("expected the entry card to catch it, got {other:?}"),
    }
    let by_owner = CurrentFiling { place: FlowPlace::Label(CLIENTS.into()), card: WhyCard::Hand, chain_id: None, ai_pick: None, decided_by: DecidedBy::Owner };
    assert_eq!(refile_with(&news_off, &starred, by_owner), FlipAction::Keep, "the owner's own filing never moves");
}

#[test]
fn why_here_names_the_card_the_chain_and_the_conditions_that_hold() {
    let l = labels();
    let mut starred = m("m15", "pm@partner-example.com", "can you check?");
    starred.starred = true;
    let v = verdict(&local(model()), &starred);
    let w = v.flow.as_ref().unwrap().why_here(&l, &starred, v.decided_by.to_contract());
    assert_eq!((w.card, w.chain_id, w.matched, w.decided_by), (WhyCard::Exit, Some(1), vec![fact(MessageFact::Starred, true)], DecidedBy::Laya));
}

#[test]
fn the_counts_read_the_filings_of_the_flow() {
    let engine = local(model());
    let entry = m("c1", "boss@client-example.com", "contract renewal?");
    let mut kept_a = m("c2", "pm@partner-example.com", "can you check?");
    kept_a.unanswered = true;
    let mut kept_b = m("c3", "pm@partner-example.com", "free on Monday?");
    kept_b.starred = true;
    let rejected = m("c4", "pm@partner-example.com", "lunch?");
    let mut noise = m("c5", "ceo@client-example.com", "a deal for you");
    noise.from_vip = true;
    let filings: Vec<FlowFiling> = [entry, kept_a, kept_b, rejected, noise]
        .iter()
        .filter_map(|f| verdict(&engine, f).flow.map(|fl| fl.filing(&f.message_id)))
        .collect();
    let c = count_label_flow(REPLY.into(), filings.clone());
    // A140: c1 is no longer filed by an entry rule of an AI-on label; the AI rules on it ("?": Needs reply) and step 3 sends it on
    assert_eq!((c.caught_by_entry, c.sorted_by_ai, c.kept_by_exit, c.sent_to_fallback), (0, 4, 2, 2), "sorted by the AI = kept + sent on");
    let c = count_label_flow(CLIENTS.into(), filings);
    assert_eq!((c.caught_by_entry, c.sorted_by_ai, c.kept_by_exit, c.sent_to_fallback), (0, 1, 1, 0), "the other label's mail never leaks in");
}

#[test]
fn a_rerun_row_carries_its_card_its_chain_and_its_rejection() {
    let l = labels();
    let s = settings(&l);
    let engine = local(model());
    let clock = StepClock { at: AtomicI64::new(0), step: 0 };
    let mut kept = m("r1", "pm@partner-example.com", "can you check?");
    kept.unanswered = true;
    let mut sent_on = m("r2", "pm@partner-example.com", "can you check?");
    sent_on.label_ids = vec![REPLY.into()];
    let out = RerunLabel { settings: &s, engine: Some(&engine), tabs: &[], labels: &l, label_id: REPLY.into(), clock: &clock, budget_ms: None }.execute(vec![kept, sent_on]);
    let row = |id: &str| out.rows.iter().find(|r| r.message_id == id).expect(id);
    let w = row("r1").file_write().expect("filed");
    assert_eq!((w.card, w.chain_id, w.by), (WhyCard::Exit, Some(0), DecidedBy::Laya));
    assert!(row("r1").rejection(5).is_none());
    assert_eq!(row("r2").action, decision::RerunAction::Unfile, "filed today, rejected by the exit filter now: it leaves the label");
    assert_eq!(row("r2").file_write(), None, "Primary carries no label to write");
    assert_eq!(row("r2").rejection(5).map(|r| (r.ai_label_id, r.sent_to)), Some((REPLY.to_string(), "primary".to_string())));
}
