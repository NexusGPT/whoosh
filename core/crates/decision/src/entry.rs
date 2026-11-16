//! The list grammar of a decision page. A RULE entry is conditions → the page's outcome; a MODEL entry is "ask the model:
//! <question in words> · trust it from <level>" (below the level the list continues); a DEFAULT closes the list.
//! The "Which tab" and "Needs my reply" shipped lists are the smart-inbox lane's (`smart_inbox::DecisionList`), converted
//! here, so each shipped list lives once.
use crate::outcome::Outcome;
use crate::tuning::MODEL_TRUST_SIGN_IN_CODE;
use contracts::{default_ai_reads, AiReads, ChainPlacement, LabelChain, LabelScope, MailLabel, Rule, RuleAction, RuleCondition, RuleSet};
use smart_inbox::{Cond, DecisionList, EntryKind as SiKind, MessageFacts, TabRef, Then};
use std::collections::HashMap;

/// What a page settles for one message.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Tab(TabRef),
    Yes(bool),
    /// The label added on arrival; None = no label.
    Label(Option<String>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum When {
    /// A condition on the message itself (the smart-inbox grammar: the owner's rule conditions, VIP list, list header…).
    Fact(Cond),
    /// Another page's answer: "if Needs my reply is yes", "if VIP".
    Outcome(Outcome, bool),
    /// Any of these (one rule, several words: subject contains "code", "verification" or "sign in").
    Any(Vec<When>),
    /// The fixed rules of a label (contract delta 9), judged by the contract's own `rule_set_matches`: the function the
    /// add-label sheet's live count and the store call, so a rule is written once and the count and the filing agree.
    LabelRules(RuleSet),
}

#[derive(Debug, Clone, PartialEq)]
pub enum EntryKind {
    Rule { when: Vec<When>, then: Value },
    Model { question: String, trust_from: f64 },
    Default { then: Value },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    /// Names the entry in `decided_by` ("which-tab/model", "rule:<id>").
    pub id: String,
    pub kind: EntryKind,
}

#[derive(Debug, Clone, PartialEq)]
pub struct OutcomeList {
    pub outcome: Outcome,
    pub entries: Vec<Entry>,
}

/// A tab the "Which tab" model may answer, named by its key or id and by the name the model reads.
#[derive(Debug, Clone, PartialEq)]
pub struct TabOption {
    pub tab: TabRef,
    pub name: String,
}

impl TabOption {
    /// The tabs the model may answer, as the smart inbox lists them (its names and their references).
    pub fn from_tab_set(tabs: &smart_inbox::TabSet) -> Vec<TabOption> {
        tabs.model_options().into_iter().filter_map(|name| tabs.tab_for_choice(&name).map(|tab| TabOption { tab, name })).collect()
    }
}

/// One node of the owner's label tree (the store's label table: id, name, parent_id), with what delta 9 added: the
/// meaning the model reads and the one account the label belongs to.
#[derive(Debug, Clone, PartialEq)]
pub struct LabelNode {
    pub id: String,
    pub name: String,
    pub parent_id: Option<String>,
    /// What the label means, in the owner's words (`MailLabel.description`); "" = none, so the model is never asked about it.
    pub description: String,
    /// The one account the label belongs to (`LabelScope::Account`); None = every mailbox.
    pub account_id: Option<String>,
    /// Delta 17, the label's three cards (`MailLabel.entry_enabled / ai_enabled / exit_enabled`): an off entry card files nothing by
    /// its rules, an off AI card is never offered to the model, an off exit card keeps every AI pick.
    pub entry_enabled: bool,
    pub ai_enabled: bool,
    pub exit_enabled: bool,
    /// What the model reads of a message for this label (`MailLabel.ai_reads`).
    pub ai_reads: AiReads,
    /// Lane labels-r5: the AI also reads the rest of the thread (the earlier messages, cut to the model's size budget, oldest first).
    /// `from_label` reads it from the contract's `AiReads.rest_of_thread` (delta 20/21, the card's « Reste du fil » tick); `with_rest_of_thread`
    /// sets it by hand. Off by default, so every existing label reads what it read before.
    pub rest_of_thread: bool,
    /// Where the exit filter sends what it rejects: a label id, "primary" or "junk" (`MailLabel.exit_fallback`).
    pub exit_fallback: Option<String>,
    /// The label's chains with the rule record each one is, entry card then exit card (`with_chains` fills them from the rules).
    pub chains: Vec<NodeChain>,
}

/// One chain of a label with the rule record that holds it: the rule id names it in `decided_by`, the chain number in "why here".
#[derive(Debug, Clone, PartialEq)]
pub struct NodeChain {
    pub rule_id: String,
    pub placement: ChainPlacement,
    pub chain: LabelChain,
}

impl LabelNode {
    /// A node with no meaning, shared by every mailbox.
    pub fn new(id: &str, name: &str, parent_id: Option<&str>) -> LabelNode {
        LabelNode {
            id: id.into(), name: name.into(), parent_id: parent_id.map(str::to_string), description: String::new(), account_id: None,
            entry_enabled: true, ai_enabled: true, exit_enabled: false, ai_reads: default_ai_reads(), rest_of_thread: false, exit_fallback: None, chains: vec![],
        }
    }

    /// The same node with the rest-of-the-thread input ticked or not (see the field).
    pub fn with_rest_of_thread(mut self, on: bool) -> LabelNode {
        self.rest_of_thread = on;
        self
    }

    /// From the label record as the store and the fixture hand it over.
    pub fn from_label(l: &MailLabel) -> LabelNode {
        let account_id = match &l.scope {
            LabelScope::All => None,
            LabelScope::Account { account_id } => Some(account_id.clone()),
        };
        LabelNode {
            id: l.id.clone(), name: l.name.clone(), parent_id: l.parent_id.clone(), description: l.description.clone(), account_id,
            entry_enabled: l.entry_enabled, ai_enabled: l.ai_enabled, exit_enabled: l.exit_enabled, ai_reads: l.ai_reads, rest_of_thread: l.ai_reads.rest_of_thread,
            exit_fallback: l.exit_fallback.clone(), chains: vec![],
        }
    }

    /// The nodes with their chains from the rule records: every switched-on label rule of the node's label, entry and exit, by chain
    /// number. Pass the labels in sidebar order (`labels()` is): the entry card's first-match-wins reads that order.
    pub fn with_chains(labels: &[LabelNode], rules: &[Rule]) -> Vec<LabelNode> {
        labels.iter().map(|l| {
            let mut n = l.clone();
            let mut chains: Vec<NodeChain> = rules.iter().filter(|r| r.enabled && matches!(&r.action, RuleAction::Label { label_id } if label_id == &l.id))
                .map(|r| NodeChain { rule_id: r.id.clone(), placement: r.placement, chain: LabelChain { chain_id: r.chain_id, rules: RuleSet { conditions: r.conditions.clone(), match_mode: r.match_mode } } })
                .collect();
            chains.sort_by_key(|c| (c.placement == ChainPlacement::Exit, c.chain.chain_id));
            n.chains = chains;
            n
        }).collect()
    }

    /// The chains of one card, by number.
    pub fn chains_of(&self, placement: ChainPlacement) -> Vec<LabelChain> {
        self.chains.iter().filter(|c| c.placement == placement).map(|c| c.chain.clone()).collect()
    }

    /// Whether a message of `account_id` can carry this label (a shared label: always; an account's label: that account only).
    pub fn visible_to(&self, account_id: &str) -> bool {
        match &self.account_id {
            None => true,
            Some(a) => a == account_id,
        }
    }
}

/// The separator of a label path as the model reads and answers it: "client > Provanta > Orbit".
pub const LABEL_PATH_SEP: &str = " > ";

/// path words -> label id, for every node of the tree (a cycle stops the walk).
pub fn label_paths(labels: &[LabelNode]) -> HashMap<String, String> {
    let by_id: HashMap<&str, &LabelNode> = labels.iter().map(|l| (l.id.as_str(), l)).collect();
    let mut out = HashMap::new();
    for l in labels {
        let mut names = vec![l.name.clone()];
        let mut seen = vec![l.id.as_str()];
        let mut at = l.parent_id.as_deref();
        while let Some(p) = at.and_then(|id| by_id.get(id)) {
            if seen.contains(&p.id.as_str()) {
                break;
            }
            seen.push(p.id.as_str());
            names.push(p.name.clone());
            at = p.parent_id.as_deref();
        }
        names.reverse();
        out.insert(names.join(LABEL_PATH_SEP), l.id.clone());
    }
    out
}

impl When {
    /// None = needs a page that has not answered (or is switched off) yet.
    pub fn eval(&self, f: &MessageFacts, answered: &HashMap<Outcome, Value>) -> Option<bool> {
        match self {
            When::Fact(c) => c.pure(f),
            When::Outcome(o, want) => match answered.get(o) {
                Some(Value::Yes(b)) => Some(b == want),
                Some(_) => Some(false),
                None => None,
            },
            When::Any(ws) => {
                let mut unknown = false;
                for w in ws {
                    match w.eval(f, answered) {
                        Some(true) => return Some(true),
                        None => unknown = true,
                        Some(false) => {}
                    }
                }
                if unknown { None } else { Some(false) }
            }
            When::LabelRules(rules) => Some(contracts::rule_set_matches(rules.clone(), crate::labels::rule_facts(f))),
        }
    }
}

/// All conditions hold. A condition on a page that never answered reads as not matching (a switched-off page answers
/// nothing, so a rule that needs it never fires); `pending` tells the caller it was unknown.
pub fn all_hold(when: &[When], f: &MessageFacts, answered: &HashMap<Outcome, Value>) -> (bool, bool) {
    let mut pending = false;
    for w in when {
        match w.eval(f, answered) {
            Some(true) => {}
            Some(false) => return (false, false),
            None => pending = true,
        }
    }
    (!pending, pending)
}

fn from_then(t: &Then) -> Value {
    match t {
        Then::Tab(r) => Value::Tab(r.clone()),
        Then::Answer(b) => Value::Yes(*b),
    }
}

fn from_cond(c: &Cond) -> When {
    match c {
        Cond::NeedsReply => When::Outcome(Outcome::NeedsMyReply, true),
        other => When::Fact(other.clone()),
    }
}

fn rule(id: &str, when: Vec<When>, then: Value) -> Entry {
    Entry { id: id.to_string(), kind: EntryKind::Rule { when, then } }
}
fn default(id: &str, then: Value) -> Entry {
    Entry { id: id.to_string(), kind: EntryKind::Default { then } }
}

pub const SIGN_IN_CODE_QUESTION: &str = "Does this email carry a one-time code, a login link or a verification number?";
/// "if subject contains "code", "verification", "sign in"" (docs/DECISION-MODEL.md section 1): words, not numbers.
const SIGN_IN_WORDS: &[&str] = &["code", "verification", "sign in"];

impl OutcomeList {
    /// A smart-inbox list (shipped, or with the owner's tab entries in front) as a page of this crate.
    pub fn from_smart_inbox(outcome: Outcome, list: &DecisionList) -> OutcomeList {
        let entries = list
            .entries
            .iter()
            .map(|e| Entry {
                id: e.id.clone(),
                kind: match &e.kind {
                    SiKind::Rule { when, then } => EntryKind::Rule { when: when.iter().map(from_cond).collect(), then: from_then(then) },
                    SiKind::Model { question, trust_from } => EntryKind::Model { question: question.clone(), trust_from: *trust_from },
                    SiKind::Default { then } => EntryKind::Default { then: from_then(then) },
                },
            })
            .collect();
        OutcomeList { outcome, entries }
    }

    /// The shipped list of each page (docs/DECISION-MODEL.md section 1).
    pub fn shipped(outcome: Outcome) -> OutcomeList {
        let k = outcome.key();
        let entries = match outcome {
            Outcome::WhichTab => return OutcomeList::from_smart_inbox(outcome, &DecisionList::shipped_which_tab()),
            Outcome::NeedsMyReply => return OutcomeList::from_smart_inbox(outcome, &DecisionList::shipped_needs_reply()),
            // 1 · if subject contains "code", "verification", "sign in" → yes · 2 · ask the model (trust from 90 %) · 3 · otherwise no
            Outcome::SignInCodes => vec![
                rule(
                    &format!("{k}/subject-words"),
                    vec![When::Any(SIGN_IN_WORDS.iter().map(|w| When::Fact(Cond::User(RuleCondition::SubjectContains { words: w.to_string() }))).collect())],
                    Value::Yes(true),
                ),
                Entry { id: format!("{k}/model"), kind: EntryKind::Model { question: SIGN_IN_CODE_QUESTION.into(), trust_from: MODEL_TRUST_SIGN_IN_CODE } },
                default(&format!("{k}/default"), Value::Yes(false)),
            ],
            // 1 · your VIP list (addresses and domains) · 2 · otherwise no (no model entry shipped)
            Outcome::Vips => vec![rule(&format!("{k}/vip-list"), vec![When::Fact(Cond::FromVip)], Value::Yes(true)), default(&format!("{k}/default"), Value::Yes(false))],
            // 1 · if VIP → notify · 2 · if Needs my reply is yes → notify · 3 · if has a list header → silent · 4 · otherwise silent
            Outcome::NotifyMe => vec![
                rule(&format!("{k}/vip"), vec![When::Outcome(Outcome::Vips, true)], Value::Yes(true)),
                rule(&format!("{k}/needs-reply"), vec![When::Outcome(Outcome::NeedsMyReply, true)], Value::Yes(true)),
                rule(&format!("{k}/list-header"), vec![When::Fact(Cond::ListHeader)], Value::Yes(false)),
                default(&format!("{k}/default"), Value::Yes(false)),
            ],
            // your rules only (no model entry shipped)
            Outcome::Labels => vec![default(&format!("{k}/default"), Value::Label(None))],
        };
        OutcomeList { outcome, entries }
    }

    pub fn model_entries(&self) -> impl Iterator<Item = (usize, &Entry)> {
        self.entries.iter().enumerate().filter(|(_, e)| matches!(e.kind, EntryKind::Model { .. }))
    }
}
