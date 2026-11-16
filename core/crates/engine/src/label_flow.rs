//! Delta 17 in the real core: the six label-flow commands and the `core state` paths the label inspector reads, served from the
//! store (`store/src/label_meta.rs`). The shapes are the fixture core's (Sources/CoreBridge/FixtureFlow.swift, keys sorted) so a
//! screen reads both cores the same way; the conditions are the store's own encoding (`store::encode_conditions`).
//! Paths: label/<id>/flow/<entry_enabled|ai_enabled|exit_enabled|ai_reads|exit_fallback|counts> · label/<id>/chains/<entry|exit>
//! · label/<id>/chains/<entry|exit>/count · message/<id>/why/<label id|primary|junk> · message/<id>/flow_rejection ·
//! message/<id>/filing = what the flow did with a message the sync stored: {ai, card, chain_id, decided_by, place}, where `ai` says
//! what became of the model step ("not_reached": an entry rule settled it, "answered": the model placed it; when the flow left it in Primary:
//! "configured" = a model is chosen and can answer, "refused" = a model is chosen but cannot answer now, its words in `ai_refusal` (Laya
//! handed over without its weights, Jev without its key or consent), "no_provider" = no model is chosen), and `ai_refusal` (words, or null).
//! engine-act-r6 round 7: "configured" used to mean only "a port exists", so a rig with Laya handed over but no weights said "configured".
//! A140 (lane flow3-core): `counts` gains `waiting_for_ai` and the pipeline's meaning (`counts_json`); `why` gains `ai` and `entry` for a
//! mail the AI accepted (`entry_json`), the keys and types the fixture core serves (Sources/CoreBridge/FixtureFlow.swift).
use std::collections::HashSet;

use contracts::*;
use store::{encode_conditions, Store};

use crate::rust_core::now_ms;

const OK: CommandOutcome = CommandOutcome { undo_entry_id: None, created_id: None };

/// Whether the command is one of delta 17's six (what `RustCore::run` hands here).
pub(crate) fn is_flow_command(c: &Command) -> bool {
    matches!(c, Command::SetLabelCard { .. } | Command::SetLabelAiReads { .. } | Command::SetLabelExitFallback { .. }
        | Command::AddLabelChain { .. } | Command::UpdateLabelChain { .. } | Command::RemoveLabelChain { .. })
}

/// Runs one of the six. A view as the label is refused first (`check_view_command`, code `view_not_a_label`), as in the fixture core.
pub(crate) fn run(store: &Store, command: Command) -> Result<CommandOutcome, CoreError> {
    check_view_command(command.clone())?;
    let now = now_ms();
    match command {
        Command::SetLabelCard { label_id, card, enabled } => store.label_set_card(label_id, card, enabled, now).map(|_| OK),
        Command::SetLabelAiReads { label_id, reads } => store.label_set_ai_reads(label_id, reads, now).map(|_| OK),
        Command::SetLabelExitFallback { label_id, fallback } => store.label_set_exit_fallback(label_id, fallback, now).map(|_| OK),
        Command::AddLabelChain { label_id, placement, chain } => {
            store.label_chain_add(label_id, placement, chain, now).map(|n| CommandOutcome { undo_entry_id: None, created_id: Some(n.to_string()) })
        }
        Command::UpdateLabelChain { label_id, placement, chain_id, chain } => store.label_chain_update(label_id, placement, chain_id, chain, now).map(|_| OK),
        Command::RemoveLabelChain { label_id, placement, chain_id } => store.label_chain_remove(label_id, placement, chain_id).map(|_| OK),
        _ => Err(CoreError::Invalid { field: "command".into(), reason_words: "not a label flow command".into() }),
    }
}

fn s(v: &str) -> String {
    let mut o = String::from("\"");
    for c in v.chars() {
        match c { '"' => o.push_str("\\\""), '\\' => o.push_str("\\\\"), c if (c as u32) < 0x20 => o.push_str(&format!("\\u{:04x}", c as u32)), c => o.push(c) }
    }
    o.push('"');
    o
}
fn opt_u32(v: Option<u32>) -> String { v.map(|n| n.to_string()).unwrap_or_else(|| "null".into()) }

fn card_words(c: WhyCard) -> &'static str {
    match c { WhyCard::Entry => "entry", WhyCard::Ai => "ai", WhyCard::Exit => "exit", WhyCard::Fallback => "fallback", WhyCard::Hand => "hand" }
}
fn decided_by_words(d: &DecidedBy) -> String {
    match d { // the stored rule id is the entry id, which already carries "rule:" (`Verdict::to_contract`): one prefix, the rule record's own id
        DecidedBy::Rule { rule_id } => format!("rule:{}", rule_id.strip_prefix("rule:").unwrap_or(rule_id)), DecidedBy::Laya => "laya".into(), DecidedBy::Jev => "jev".into(), DecidedBy::Apple => "apple".into(), DecidedBy::Model { model } => format!("model:{model}"), DecidedBy::Owner => "owner".into(), DecidedBy::Default => "default".into() }
}
fn state_words(s: FactState) -> &'static str { match s { FactState::Any => "any", FactState::Yes => "yes", FactState::No => "no" } }

fn chain_json(c: &LabelChain) -> String {
    let mut facts: Vec<(String, &str)> = fact_states(c.rules.conditions.clone()).into_iter().map(|f| (message_fact_key(f.fact), state_words(f.state))).collect();
    facts.sort();
    let facts = facts.iter().map(|(k, v)| format!("{}:{}", s(k), s(v))).collect::<Vec<_>>().join(",");
    format!(
        "{{\"chain_id\":{},\"conditions\":{},\"facts\":{{{}}},\"match\":{},\"rows\":{}}}",
        c.chain_id, encode_conditions(&c.rules.conditions), facts, s(if c.rules.match_mode == MatchMode::All { "all" } else { "any" }),
        encode_conditions(&row_conditions(c.rules.conditions.clone()))
    )
}

fn placement_of(w: &str) -> Option<ChainPlacement> { match w { "entry" => Some(ChainPlacement::Entry), "exit" => Some(ChainPlacement::Exit), _ => None } }

/// The answer of one delta 17 path; None = not one of them (the caller answers its own None).
/// What the label AI can do now, as the filing record says it (rust_core.rs computes it from the filing worker's port and the model's refusal).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum FilingAi { NoProvider, Configured, Refused(String) }

pub(crate) fn state(store: &Store, path: &str, ai: &dyn Fn() -> FilingAi) -> Option<Option<String>> {
    let p: Vec<&str> = path.split('/').collect();
    let label = |id: &str| store.labels_full().ok().and_then(|ls| ls.into_iter().find(|l| l.id == id));
    Some(match (p.first().copied(), p.len()) {
        (Some("label"), 4) if p[2] == "flow" => label(p[1]).and_then(|l| match p[3] {
            "entry_enabled" => Some(l.entry_enabled.to_string()),
            "ai_enabled" => Some(l.ai_enabled.to_string()),
            "exit_enabled" => Some(l.exit_enabled.to_string()),
            // `rest_of_thread` is served only while it is on, so a new installation keeps the three keys of docs/FIRST-RUN.json (the test engine does the same)
            "ai_reads" => Some(format!("{{\"attachments\":{},\"body\":{},{}\"subject\":{}}}", l.ai_reads.attachments, l.ai_reads.body,
                if l.ai_reads.rest_of_thread { "\"rest_of_thread\":true," } else { "" }, l.ai_reads.subject)),
            "exit_fallback" => l.exit_fallback.as_deref().map(s),
            "counts" => counts_json(store, &l),
            // lane fix-aisort round 11: how many picked mails the judge refused for themselves (set aside, not asked again): the card's count
            "ai_set_aside" => store.ai_set_aside_count(&l.id).ok().map(|n| n.to_string()),
            _ => None,
        }),
        // labels-r5 round 7: the sheet's rule set (entry chain 0, the one `LabelSpec.rules` writes), in the store's JSON shape {"match":..,"conditions":[..]};
        // nothing for a label that has none (the fixture core answers the same)
        (Some("label"), 3) if p[2] == "rules" => {
            let (Some(_), Ok(set)) = (label(p[1]), store.label_rules(p[1])) else { return Some(None) };
            if set.conditions.is_empty() { return Some(None) }
            Some(format!("{{\"conditions\":{},\"match\":{}}}", encode_conditions(&set.conditions), s(if set.match_mode == MatchMode::All { "all" } else { "any" })))
        }
        // labels-r5 round 7: who filed a message under a label: "rule:<rule id>", "laya", "jev", "apple", "model:<catalogue id>" (lane decided-model),
        // "owner" or "default"; nothing when it is not filed there
        (Some("message"), 5) if p[2] == "label" && p[4] == "decided_by" => store.label_filing(p[3], p[1]).ok().flatten().map(|(by, _)| s(&decided_by_words(&by))),
        (Some("label"), 4) if p[2] == "chains" => {
            let (Some(_), Some(pl)) = (label(p[1]), placement_of(p[3])) else { return Some(None) };
            store.label_chains(p[1], pl).ok().map(|cs| format!("[{}]", cs.iter().map(chain_json).collect::<Vec<_>>().join(",")))
        }
        (Some("label"), 5) if p[2] == "chains" && p[4] == "count" => {
            let (Some(_), Some(pl)) = (label(p[1]), placement_of(p[3])) else { return Some(None) };
            store.label_chains(p[1], pl).ok().map(|cs| cs.len().to_string())
        }
        (Some("message"), 4) if p[2] == "why" => store.label_why(p[1], p[3]).ok().flatten().map(|w| {
            // A140: `ai` and `entry` (keys sorted, as the fixture core serves them) for a mail the AI accepted into an AI-on label
            let (ai, entry) = match label(p[3]) {
                Some(l) if matches!(w.card, WhyCard::Ai | WhyCard::Exit) && ai_step_on(&l) => ("\"ai\":\"yes\",".to_string(), entry_json(store, p[1], &l).map(|e| format!("\"entry\":{e},")).unwrap_or_default()),
                _ => (String::new(), String::new()),
            };
            format!("{{{ai}\"card\":{},\"chain_id\":{},\"decided_by\":{},{entry}\"matched\":{}}}", s(card_words(w.card)), opt_u32(w.chain_id), s(&decided_by_words(&w.decided_by)), encode_conditions(&w.matched))
        }),
        (Some("message"), 3) if p[2] == "filing" => filing_json(store, p[1], ai()),
        (Some("message"), 3) if p[2] == "flow_rejection" => store.flow_rejection(p[1]).ok().flatten().map(|r| {
            format!("{{\"ai_label_id\":{},\"chain_id\":{},\"sent_to\":{}}}", s(&r.ai_label_id), opt_u32(r.chain_id), s(&r.sent_to))
        }),
        _ => return None,
    })
}

// ---------------------------------------------------------------- A140: the counts and the "why here" of the three-step pipeline

/// Step 2 is on for this label: its AI card is on and it has a description (`decision::flow::ai_step_on`, the one reading of both cores).
fn ai_step_on(l: &MailLabel) -> bool {
    ::decision::flow::ai_step_on(&::decision::LabelNode::from_label(l))
}

/// A label's step-1 chains as they pick: the entry card on, the chains that hold a condition (a chain with none matches nothing), by number.
fn step_one_chains(store: &Store, l: &MailLabel) -> Vec<LabelChain> {
    if !l.entry_enabled {
        return vec![];
    }
    let mut chains: Vec<LabelChain> = store.label_chains(&l.id, ChainPlacement::Entry).unwrap_or_default().into_iter().filter(|c| !c.rules.conditions.is_empty()).collect();
    chains.sort_by_key(|c| c.chain_id);
    chains
}

/// The inbound INBOX messages step 1 of the label picks (any of its chains): the corpus and the judge of the Re-run preview and the sheet's count.
fn picked_by_entry(store: &Store, l: &MailLabel) -> HashSet<String> {
    step_one_chains(store, l).into_iter().flat_map(|c| smart_inbox::corpus::rule_set_matching_ids(store, c.rules, usize::MAX)).collect()
}

/// The message ids one query names for a label (`?1` = the label id); none when the database cannot be read.
fn ids_where(store: &Store, sql: &str, label_id: &str) -> HashSet<String> {
    let Ok(c) = smart_inbox::load::open_reader(store.path()) else { return HashSet::new() };
    let Ok(mut st) = c.prepare(sql) else { return HashSet::new() };
    let out: HashSet<String> = match st.query_map(rusqlite::params![label_id], |r| r.get::<_, String>(0)) {
        Ok(rows) => rows.flatten().collect(),
        Err(_) => HashSet::new(),
    };
    out
}

/// `label/<id>/flow/counts`, A140's meaning, computed here from the store because the frozen `count_label_flow` counts filings by card:
/// `caught_by_entry` = the mails step 1 picks (for every label: an AI-on label's picks are never filed by the entry card);
/// `sorted_by_ai` = the mails the AI accepted (kept, sent to the fallback, or filed with no exit filter), 0 when the AI is off (an AI-off
/// label's step-3 rejection is a fallback filing that the contract would count there); `kept_by_exit` and `sent_to_fallback` from the
/// filings; `waiting_for_ai` = picked mails of an AI-on label that are not filed under it, have no rejection row naming it and no kept
/// "no" of the AI for it (part 3).
fn counts_json(store: &Store, l: &MailLabel) -> Option<String> {
    let c = count_label_flow(l.id.clone(), store.label_flow_filings().ok()?);
    let ai_on = ai_step_on(l);
    let picked = picked_by_entry(store, l);
    let waiting = if ai_on { waiting_among(store, l, &waiting_candidates(store, l, &picked)).len() } else { 0 };
    Some(format!(
        "{{\"caught_by_entry\":{},\"kept_by_exit\":{},\"sent_to_fallback\":{},\"sorted_by_ai\":{},\"waiting_for_ai\":{}}}",
        picked.len(), c.kept_by_exit, c.sent_to_fallback, if ai_on { c.sorted_by_ai } else { 0 }, waiting
    ))
}

/// The picked mails of an AI-on label the AI has not ruled on: not filed under it, no rejection row naming it, no kept "no" of the AI for
/// it (A140 part 3). Lane fix-aisort: a failed or unreadable judgement keeps no row, so its mail stays here and is asked again.
fn waiting_among(store: &Store, l: &MailLabel, picked: &HashSet<String>) -> HashSet<String> {
    let filed = ids_where(store, "SELECT message_id FROM label_assignment WHERE label_id = ?1", &l.id);
    let ruled = ids_where(store, "SELECT message_id FROM flow_rejection WHERE ai_label_id = ?1", &l.id);
    // A140 part 3: and the mails the AI ruled NO on for this label (`ai_label_no`, migration 0022) are not waiting either
    let refused: HashSet<String> = store.ai_no_messages(&l.id).unwrap_or_default().into_iter().collect();
    picked.iter().filter(|m| !filed.contains(*m) && !ruled.contains(*m) && !refused.contains(*m)).cloned().collect()
}

/// Lane fix-aisort round 17 (the owner's Needs Reply of 12 Nov: "197 waiting for the AI", then nothing moved until a Save): the mails that
/// may wait for the label's AI. With a step 1, what it picks. With none (Needs Reply as seeded), step 1 picks nothing, so the mails the AI
/// could not judge are added: the ones the store marks as waiting (`Store::ai_waiting_put`, written when a judgement failed in a way that
/// can pass); a "no", a set-aside and a filing still take a mail out (`waiting_among`).
fn waiting_candidates(store: &Store, l: &MailLabel, picked: &HashSet<String>) -> HashSet<String> {
    let mut out = picked.clone();
    if step_one_chains(store, l).is_empty() {
        out.extend(store.ai_waiting_messages(&l.id).unwrap_or_default());
    }
    out
}

/// Lane fix-aisort: the mails waiting for the AI of each AI-on label (`waiting_for_ai` of `label/<id>/flow/counts`), by label id.
pub(crate) fn waiting_for_ai(store: &Store) -> Vec<(String, HashSet<String>)> {
    let Ok(labels) = store.labels_full() else { return vec![] };
    labels.iter().filter(|l| ai_step_on(l)).map(|l| (l.id.clone(), waiting_among(store, l, &waiting_candidates(store, l, &picked_by_entry(store, l))))).collect()
}

/// The `entry` of the why read: step 1's first chain (by number) that picks the message, with the conditions of it that hold, read now from
/// the message's facts (no stored column): {"chain_id":N,"matched":[..]}; None when no chain picks it (an open step 1).
fn entry_json(store: &Store, message_id: &str, l: &MailLabel) -> Option<String> {
    let facts = store.rule_facts(message_id).ok().flatten()?;
    let c = step_one_chains(store, l).into_iter().find(|c| rule_set_matches(c.rules.clone(), facts.clone()))?;
    Some(format!("{{\"chain_id\":{},\"matched\":{}}}", c.chain_id, encode_conditions(&matched_conditions(c.rules, facts))))
}

/// The flow's own record of one stored message (see the header): None for a message that is not stored.
fn filing_json(store: &Store, message_id: &str, ai_now: FilingAi) -> Option<String> {
    store.message(message_id).ok().flatten()?;
    let reader = smart_inbox::load::open_reader(store.path()).ok()?;
    let cur = crate::flow_ctx::current_filing(&reader, message_id).ok()?;
    let ai = match cur.card {
        WhyCard::Entry => "not_reached",
        WhyCard::Ai | WhyCard::Exit | WhyCard::Fallback => "answered",
        WhyCard::Hand => match &ai_now { FilingAi::Configured => "configured", FilingAi::Refused(_) => "refused", FilingAi::NoProvider => "no_provider" },
    };
    // the refusal's words ride with "refused" only: the screen's "Why here?" says them
    let refusal = match (&cur.card, &ai_now) { (WhyCard::Hand, FilingAi::Refused(w)) => s(w), _ => "null".to_string() };
    Some(format!(
        "{{\"ai\":{},\"ai_refusal\":{},\"card\":{},\"chain_id\":{},\"decided_by\":{},\"place\":{}}}",
        s(ai), refusal, s(card_words(cur.card)), opt_u32(cur.chain_id), s(&decided_by_words(&cur.decided_by)), s(&cur.place.key())
    ))
}
