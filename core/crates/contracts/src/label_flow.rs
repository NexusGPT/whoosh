//! Delta 17 (docs/PRD.md "A94 pick" and "A94 pick, detail", owner 2026-11-09 18:45 and 18:58): a label is a FLOW of three cards, each
//! with its own switch: Entry rules (a matching message goes straight in, no AI), AI sorting (from the description, reading the inputs
//! the owner ticks), Exit filter (what the AI sorted here must pass; the others go to a fallback). This extends delta 9's rules and
//! delta 16's view ids; it forks neither. A rule record is ONE chain (conditions + match mode); the chains of one (label, placement)
//! are alternatives. The eight yes/no facts are `RuleCondition::Fact`, three-state in the grid (any / yes / no), where "any" is the
//! absence of a condition. The pure functions here are what the Rust core, the store and the Swift FixtureCore all call.
//!
//! WHAT THIS FILE DOES NOT DO: it does not classify. The evaluation order (entry, then AI, then exit) is the decision lane's to
//! apply; docs/lanes/contracts.md "delta 17" writes it down, marked "needs decision lane".
use crate::{
    inbox_view_from_id, inbox_view_id, mailbox_key, rule_set_matches, CoreError, DecidedBy, InboxView, MailLabel, Mailbox, MatchMode,
    MessageFacts, RuleCondition, RuleSet,
};

/// The `field` of the refusal "turn the exit filter on without saying where the others go" (« Dites où vont les autres »). The
/// sentence is `reason_words`; the French of the catalogue key belongs to the i18n lane, the core speaks the English key.
pub const EXIT_NEEDS_FALLBACK_CODE: &str = "exit_needs_fallback";
/// The `field` of every refusal about a fallback that cannot be one (itself, unknown, the All view).
pub const EXIT_FALLBACK_REFUSAL_CODE: &str = "bad_exit_fallback";
/// The `field` of every refusal about a chain that cannot be saved (empty, a fact set twice, a condition without words).
pub const CHAIN_REFUSAL_CODE: &str = "bad_chain";
/// The `field` of the refusal "the AI reads nothing".
pub const AI_READS_NOTHING_CODE: &str = "ai_reads_nothing";

fn refuse(code: &str, words: impl Into<String>) -> CoreError { CoreError::Invalid { field: code.into(), reason_words: words.into() } }

// ---------------------------------------------------------------- schema

/// Which of a label's two rule cards a chain belongs to. Closed: the AI card has no chains.
#[derive(uniffi::Enum, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ChainPlacement { Entry, Exit }

/// The label's three cards, in reading order: what a switch is set on.
#[derive(uniffi::Enum, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FlowCard { Entry, Ai, Exit }

/// What the AI reads of a message (the card's « L'IA lit »). The one truth of the owner's ticks.
#[derive(uniffi::Record, Debug, Clone, Copy, PartialEq, Eq)]
pub struct AiReads {
    pub subject: bool,
    pub body: bool,
    /// the earlier messages of the thread (delta 20: the label AI and the AI replies share this ONE record). Not counted by `check_set_ai_reads`
    /// (a label AI that reads only the thread is meaningless); `check_reply_ai_reads` counts it.
    #[uniffi(default = false)]
    pub rest_of_thread: bool,
    pub attachments: bool,
}

/// The default: subject and body, not the attachments (what the board draws, and what the model read before delta 17).
#[uniffi::export]
pub fn default_ai_reads() -> AiReads { AiReads { subject: true, body: true, rest_of_thread: false, attachments: false } }

/// The eight facts of the grid, in the order the board draws them (Non lu, Sans réponse, Pièce jointe, Favori, Expéditeur VIP,
/// Je suis en copie, Liste de diffusion, Déjà répondu par moi). Closed on purpose: a ninth fact is a contract change.
#[derive(uniffi::Enum, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MessageFact { Unread, Unanswered, HasAttachment, Starred, VipSender, CcMe, MailingList, RepliedByMe }

/// The three states of one fact in the grid. `Any` ("—") is the ABSENCE of a condition; it is never stored as one.
#[derive(uniffi::Enum, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FactState { Any, Yes, No }

/// One cell of the grid.
#[derive(uniffi::Record, Debug, Clone, Copy, PartialEq, Eq)]
pub struct FactSetting { pub fact: MessageFact, pub state: FactState }

/// One chain of a label: its number and its conditions (the `Rule` record without the label it belongs to).
#[derive(uniffi::Record, Debug, Clone, PartialEq, Eq)]
pub struct LabelChain { pub chain_id: u32, pub rules: RuleSet }

/// What the exit filter says about one message the AI sorted into the label.
#[derive(uniffi::Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExitVerdict {
    /// kept; `chain_id` is the chain that kept it, None when the label has no exit chain (an exit filter with no chain filters nothing)
    Kept { chain_id: Option<u32> },
    /// no chain keeps it: it goes to the fallback
    Rejected,
}

/// Where the messages the exit filter rejects go, read from `MailLabel.exit_fallback`.
#[derive(uniffi::Enum, Debug, Clone, PartialEq, Eq)]
pub enum ExitFallback { Label { label_id: String }, Primary, Junk }

/// Which card placed a message where it is (the "why here" tag): a rule of the entry card; the AI with no exit filter on; the AI
/// and the exit filter kept it; the exit filter sent it to its fallback; the owner by hand (or the default).
#[derive(uniffi::Enum, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WhyCard { Entry, Ai, Exit, Fallback, Hand }

/// The per-message "why here" read: the card, the chain (None for the AI alone, the fallback, or by hand), the conditions of that
/// chain that held for this message (structured, the UI words them: the core keeps no sentence, so no language is mixed), and who
/// decided (the delta 9 `DecidedBy`, unchanged).
#[derive(uniffi::Record, Debug, Clone, PartialEq)]
pub struct WhyHere {
    pub card: WhyCard,
    pub chain_id: Option<u32>,
    pub matched: Vec<RuleCondition>,
    pub decided_by: DecidedBy,
}

/// One message's flow result, as the counts read it. `ended_in` is the label it sits in (None = it went to Primary or Junk);
/// `ai_pick` is the label the AI chose, whatever happened after (None when the AI did not decide it).
#[derive(uniffi::Record, Debug, Clone, PartialEq, Eq)]
pub struct FlowFiling { pub message_id: String, pub ended_in: Option<String>, pub card: WhyCard, pub ai_pick: Option<String> }

/// The four figures of the foot of a label's inspector. Never stored: derived from the filings, so a count cannot be counted twice.
/// `sorted_by_ai` = the AI's picks of this label = those kept without an exit filter + kept by the exit filter + sent to the fallback.
#[derive(uniffi::Record, Debug, Clone, PartialEq, Eq)]
pub struct LabelFlowCounts { pub label_id: String, pub caught_by_entry: u32, pub sorted_by_ai: u32, pub kept_by_exit: u32, pub sent_to_fallback: u32 }

/// DATA-MAP `flow-rejection`: the AI picked `ai_label_id` for the message and the exit filter of that label sent it to `sent_to`
/// (a label id, "primary" or "junk"). One row per message, replaced when the message is re-sorted, removed when the AI's pick passes.
/// It is the only flow fact that cannot be read from `label_assignment` (a message sent to Primary or Junk has no assignment).
#[derive(uniffi::Record, Debug, Clone, PartialEq, Eq)]
pub struct FlowRejection { pub message_id: String, pub ai_label_id: String, pub sent_to: String, pub chain_id: Option<u32>, pub decided_at_ms: i64 }

/// DATA-MAP `ai-label-no` (SPEC A140): the label AI ruled NO on `message_id` for `label_id`, after step 1 of that label had picked it.
/// One row per (message, label), replaced by a newer no; a yes has no row (the filing records it). `decided_by` is the model that
/// answered, as the filing rows name it. A changed description of the label removes its rows: a changed question is asked again.
#[derive(uniffi::Record, Debug, Clone, PartialEq, Eq)]
pub struct AiLabelNo { pub message_id: String, pub label_id: String, pub decided_by: String, pub decided_at_ms: i64 }

// ---------------------------------------------------------------- the facts

/// The facts in the grid's order.
#[uniffi::export]
pub fn message_facts_in_grid_order() -> Vec<MessageFact> {
    vec![MessageFact::Unread, MessageFact::Unanswered, MessageFact::HasAttachment, MessageFact::Starred, MessageFact::VipSender,
         MessageFact::CcMe, MessageFact::MailingList, MessageFact::RepliedByMe]
}

/// The key of a fact in stored JSON (`{"kind":"fact","fact":"<key>","wanted":true}`) and in state paths, once.
#[uniffi::export]
pub fn message_fact_key(fact: MessageFact) -> String {
    match fact {
        MessageFact::Unread => "unread", MessageFact::Unanswered => "unanswered", MessageFact::HasAttachment => "has_attachment",
        MessageFact::Starred => "starred", MessageFact::VipSender => "vip_sender", MessageFact::CcMe => "cc_me",
        MessageFact::MailingList => "mailing_list", MessageFact::RepliedByMe => "replied_by_me",
    }.to_string()
}

/// The fact a key names; None for an unknown key (a reader of stored JSON refuses it in words, never guesses).
#[uniffi::export]
pub fn message_fact_from_key(key: String) -> Option<MessageFact> {
    message_facts_in_grid_order().into_iter().find(|f| message_fact_key(*f) == key)
}

/// The fact in plain English, for refusals ("the strings table owns the translations": these are the keys' English).
fn fact_words(fact: MessageFact) -> &'static str {
    match fact {
        MessageFact::Unread => "unread", MessageFact::Unanswered => "unanswered", MessageFact::HasAttachment => "has an attachment",
        MessageFact::Starred => "starred", MessageFact::VipSender => "from a VIP", MessageFact::CcMe => "I am in copy",
        MessageFact::MailingList => "from a mailing list", MessageFact::RepliedByMe => "already answered by me",
    }
}

/// Whether the fact is true of the message. The mailing-list fact is DERIVED from the list header (no second field to drift).
#[uniffi::export]
pub fn fact_holds(fact: MessageFact, facts: MessageFacts) -> bool {
    match fact {
        MessageFact::Unread => facts.unread,
        MessageFact::Unanswered => facts.unanswered,
        MessageFact::HasAttachment => facts.has_attachment,
        MessageFact::Starred => facts.starred,
        MessageFact::VipSender => facts.vip_sender,
        MessageFact::CcMe => facts.cc_me,
        MessageFact::MailingList => !facts.list_header.trim().is_empty(),
        MessageFact::RepliedByMe => facts.replied_by_me,
    }
}

/// The fact a condition is, with the value it wants; None for the nine text conditions. `HasAttachment` is (HasAttachment, true).
#[uniffi::export]
pub fn condition_fact(condition: RuleCondition) -> Option<FactSettingWanted> {
    match condition {
        RuleCondition::Fact { fact, wanted } => Some(FactSettingWanted { fact, wanted }),
        RuleCondition::HasAttachment => Some(FactSettingWanted { fact: MessageFact::HasAttachment, wanted: true }),
        _ => None,
    }
}

/// A fact and the value wanted (`condition_fact`'s answer).
#[derive(uniffi::Record, Debug, Clone, Copy, PartialEq, Eq)]
pub struct FactSettingWanted { pub fact: MessageFact, pub wanted: bool }

/// The condition a grid cell stands for; None for "any" (no row). Pièce jointe "yes" is the old `HasAttachment` (one spelling).
#[uniffi::export]
pub fn fact_condition(fact: MessageFact, state: FactState) -> Option<RuleCondition> {
    match (fact, state) {
        (_, FactState::Any) => None,
        (MessageFact::HasAttachment, FactState::Yes) => Some(RuleCondition::HasAttachment),
        (f, FactState::Yes) => Some(RuleCondition::Fact { fact: f, wanted: true }),
        (f, FactState::No) => Some(RuleCondition::Fact { fact: f, wanted: false }),
    }
}

/// The eight cells of a chain's grid, in grid order. A fact with no condition reads `Any`. A fact set twice reads the FIRST
/// (a chain with two conditions on one fact is refused by `validate_chain`, so only a hand-edited row can carry it).
#[uniffi::export]
pub fn fact_states(conditions: Vec<RuleCondition>) -> Vec<FactSetting> {
    message_facts_in_grid_order().into_iter().map(|f| {
        let state = conditions.iter().filter_map(|c| condition_fact(c.clone())).find(|w| w.fact == f)
            .map(|w| if w.wanted { FactState::Yes } else { FactState::No }).unwrap_or(FactState::Any);
        FactSetting { fact: f, state }
    }).collect()
}

/// Sets one grid cell: every condition on that fact goes, then one is added unless the state is `Any`. The other conditions keep
/// their place; the new fact condition goes last.
#[uniffi::export]
pub fn set_fact(conditions: Vec<RuleCondition>, fact: MessageFact, state: FactState) -> Vec<RuleCondition> {
    let mut out: Vec<RuleCondition> = conditions.into_iter().filter(|c| condition_fact(c.clone()).map(|w| w.fact) != Some(fact)).collect();
    if let Some(c) = fact_condition(fact, state) { out.push(c); }
    out
}

/// The condition rows under the grid (picker + value + remove): every condition that is not a fact. The facts are the grid's.
#[uniffi::export]
pub fn row_conditions(conditions: Vec<RuleCondition>) -> Vec<RuleCondition> {
    conditions.into_iter().filter(|c| condition_fact(c.clone()).is_none()).collect()
}

// ---------------------------------------------------------------- chains

/// Checks one chain before it is written, in words with `CHAIN_REFUSAL_CODE`: at least one condition, every text condition with its
/// words (the delta 9 rule, `validate_label_spec`'s), no fact set twice (one cell of the grid holds one state).
#[uniffi::export]
pub fn validate_chain(rules: RuleSet) -> Result<(), CoreError> {
    if rules.conditions.is_empty() { return Err(refuse(CHAIN_REFUSAL_CODE, "a rule needs at least one condition")); }
    for c in &rules.conditions {
        if let Some(w) = crate::condition_refusal_words(c.clone()) { return Err(refuse(CHAIN_REFUSAL_CODE, w)); }
    }
    for f in message_facts_in_grid_order() {
        let n = rules.conditions.iter().filter_map(|c| condition_fact(c.clone())).filter(|w| w.fact == f).count();
        if n > 1 { return Err(refuse(CHAIN_REFUSAL_CODE, format!("\"{}\" is set twice in one rule: a fact has one value (yes or no)", fact_words(f)))); }
    }
    Ok(())
}

/// The number the next chain of a (label, placement) takes: one past the highest in use, 0 when there is none. Numbers are never reused
/// while a higher one lives, so a "why here" tag naming chain 2 still names it after chain 1 is removed.
#[uniffi::export]
pub fn next_chain_id(existing: Vec<u32>) -> u32 { existing.into_iter().max().map(|m| m + 1).unwrap_or(0) }

/// The chains in their order (by number). Alternatives: the first that matches decides.
fn ordered(chains: &[LabelChain]) -> Vec<&LabelChain> {
    let mut v: Vec<&LabelChain> = chains.iter().collect();
    v.sort_by_key(|c| c.chain_id);
    v
}

/// The first chain (lowest number) that matches the message; None when none does. Alternatives across chains; inside one chain the
/// conditions follow its `match_mode`; an empty chain matches nothing (the delta 9 rule).
#[uniffi::export]
pub fn first_matching_chain(chains: Vec<LabelChain>, facts: MessageFacts) -> Option<u32> {
    ordered(&chains).into_iter().find(|c| rule_set_matches(c.rules.clone(), facts.clone())).map(|c| c.chain_id)
}

/// The exit filter's verdict on a message the AI sorted into the label: kept by the first chain that matches, rejected when none does.
/// With NO chain it keeps everything (the filter has nothing to filter by; turning it on must not empty the label).
#[uniffi::export]
pub fn exit_verdict(chains: Vec<LabelChain>, facts: MessageFacts) -> ExitVerdict {
    if chains.is_empty() { return ExitVerdict::Kept { chain_id: None }; }
    match first_matching_chain(chains, facts) { Some(id) => ExitVerdict::Kept { chain_id: Some(id) }, None => ExitVerdict::Rejected }
}

/// The conditions of a chain that hold for the message: all of them in "all" mode when the chain matched, the true ones in "any" mode.
/// What the "why here" tag shows after the chain's name. Empty when the chain does not match.
#[uniffi::export]
pub fn matched_conditions(rules: RuleSet, facts: MessageFacts) -> Vec<RuleCondition> {
    if !rule_set_matches(rules.clone(), facts.clone()) { return vec![]; }
    match rules.match_mode {
        MatchMode::All => rules.conditions,
        MatchMode::Any => rules.conditions.into_iter()
            .filter(|c| rule_set_matches(RuleSet { conditions: vec![c.clone()], match_mode: MatchMode::Any }, facts.clone())).collect(),
    }
}

/// Assembles the "why here" read: for an entry or exit-kept message the chain and its held conditions; for the others no chain.
#[uniffi::export]
pub fn why_here(card: WhyCard, chain: Option<LabelChain>, facts: MessageFacts, decided_by: DecidedBy) -> WhyHere {
    let (chain_id, matched) = match (&card, chain) {
        (WhyCard::Entry, Some(c)) | (WhyCard::Exit, Some(c)) => (Some(c.chain_id), matched_conditions(c.rules, facts)),
        _ => (None, vec![]),
    };
    WhyHere { card, chain_id, matched, decided_by }
}

// ---------------------------------------------------------------- the counts

/// The four figures of one label over the filings of the corpus.
#[uniffi::export]
pub fn count_label_flow(label_id: String, filings: Vec<FlowFiling>) -> LabelFlowCounts {
    let here = |f: &&FlowFiling| f.ended_in.as_deref() == Some(label_id.as_str());
    let n = |card: WhyCard| filings.iter().filter(here).filter(|f| f.card == card).count() as u32;
    let sent = filings.iter().filter(|f| f.card == WhyCard::Fallback && f.ai_pick.as_deref() == Some(label_id.as_str())).count() as u32;
    let (entry, ai_only, kept) = (n(WhyCard::Entry), n(WhyCard::Ai), n(WhyCard::Exit));
    LabelFlowCounts { label_id: label_id.clone(), caught_by_entry: entry, sorted_by_ai: ai_only + kept + sent, kept_by_exit: kept, sent_to_fallback: sent }
}

// ---------------------------------------------------------------- switches, reads and fallback: the checks

/// Refuses an AI card that reads nothing, in words with `AI_READS_NOTHING_CODE`. (To stop the AI, switch the card off.)
#[uniffi::export]
pub fn check_set_ai_reads(reads: AiReads) -> Result<(), CoreError> {
    if !(reads.subject || reads.body || reads.attachments) {
        return Err(refuse(AI_READS_NOTHING_CODE, "the AI has to read at least one of: subject, body, attachments. To stop the AI sorting here, switch the card off"));
    }
    Ok(())
}

/// The refusal « Dites où vont les autres »: an exit filter on with no fallback.
fn needs_fallback() -> CoreError { refuse(EXIT_NEEDS_FALLBACK_CODE, "Say where the other messages go before you turn the exit filter on") }

/// Refuses switching a card, in words. Turning the exit filter ON needs the label's fallback (`MailLabel.exit_fallback`). The other
/// switches always pass. The caller has already refused a view as the label (`check_view_command`).
#[uniffi::export]
pub fn check_set_card(label: MailLabel, card: FlowCard, enabled: bool) -> Result<(), CoreError> {
    if card == FlowCard::Exit && enabled && label.exit_fallback.as_deref().map(str::trim).unwrap_or("").is_empty() { return Err(needs_fallback()); }
    Ok(())
}

/// Reads a fallback id: a label id, "primary" or "junk". Refused, in words with `EXIT_FALLBACK_REFUSAL_CODE`: the label itself,
/// the All view (it is every message, not a place), an id that is no label. `labels` is every label.
#[uniffi::export]
pub fn parse_exit_fallback(label_id: String, fallback: String, labels: Vec<MailLabel>) -> Result<ExitFallback, CoreError> {
    let id = fallback.trim();
    if id == inbox_view_id(InboxView::Primary) { return Ok(ExitFallback::Primary); }
    if id == mailbox_key(Mailbox::Junk) { return Ok(ExitFallback::Junk); }
    if inbox_view_from_id(id.to_string()) == Some(InboxView::All) {
        return Err(refuse(EXIT_FALLBACK_REFUSAL_CODE, "All is every message of the inbox, not a place: pick a label, Primary or Junk"));
    }
    if id == label_id { return Err(refuse(EXIT_FALLBACK_REFUSAL_CODE, "the other messages cannot go back into the same label")); }
    if !labels.iter().any(|l| l.id == id) { return Err(refuse(EXIT_FALLBACK_REFUSAL_CODE, "that label does not exist: pick a label, Primary or Junk")); }
    Ok(ExitFallback::Label { label_id: id.to_string() })
}

/// Refuses setting the fallback, in words. Clearing it (None) while the exit filter is on is the same refusal as switching on with
/// none; a value is read by `parse_exit_fallback`.
#[uniffi::export]
pub fn check_set_exit_fallback(label: MailLabel, fallback: Option<String>, labels: Vec<MailLabel>) -> Result<(), CoreError> {
    match fallback.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        None if label.exit_enabled => Err(needs_fallback()),
        None => Ok(()),
        Some(f) => parse_exit_fallback(label.id, f.to_string(), labels).map(|_| ()),
    }
}

/// The one rule a stored label row must hold, for the store and its migration trigger to share: exit on needs a fallback.
#[uniffi::export]
pub fn flow_state_is_legal(exit_enabled: bool, exit_fallback: Option<String>) -> bool {
    !exit_enabled || exit_fallback.as_deref().map(str::trim).is_some_and(|s| !s.is_empty())
}
