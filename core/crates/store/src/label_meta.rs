//! Delta 9's label reads and writes over migration 0004: the full label record (description, pin, scope, kind, order), the
//! sheet's create and update, drag-to-reorder, pin, the label's fixed rule, how a message was filed (`decided_by`), blocks and
//! suggestions. The checks are the contract's pure functions (`validate_label_spec`, `reorder_label`, `check_label_*`,
//! `suggestion_effect`); this file only stores what they allow. Additive: no older function changes behaviour.
use crate::error::StoreError;
use crate::labels::new_id;
use crate::writer::WriteCtx;
use crate::Store;
use contracts::{
    message_fact_from_key, message_fact_key, AiReads, Block, BlockTarget, ChainPlacement, CoreError, DecidedBy, FlowCard, FlowFiling, FlowRejection, LabelChain,
    LabelKind, LabelScope, LabelSpec, MailLabel, MatchMode, MessageFacts, Rule, RuleAction, RuleCondition, RuleSet, Suggestion, SuggestionEffect, SuggestionKind,
    WhyCard, WhyHere,
};
use rusqlite::{params, Connection, OptionalExtension};

fn refusal(e: CoreError) -> StoreError { StoreError::Invalid(match e { CoreError::Invalid { reason_words, .. } => reason_words, other => other.to_string() }) }

fn json_str(s: &str) -> String {
    let mut o = String::from("\"");
    for c in s.chars() { match c { '"' => o.push_str("\\\""), '\\' => o.push_str("\\\\"), c if (c as u32) < 0x20 => o.push_str(&format!("\\u{:04x}", c as u32)), c => o.push(c) } }
    o.push('"');
    o
}

/// The rule conditions as stored in `rule.conditions_json`: an array of `{"kind": ..., <field>: ...}`. The migration's fold writes the same shape.
pub fn encode_conditions(cs: &[RuleCondition]) -> String {
    let one = |c: &RuleCondition| match c {
        RuleCondition::From { words } => format!("{{\"kind\":\"from\",\"words\":{}}}", json_str(words)),
        RuleCondition::To { words } => format!("{{\"kind\":\"to\",\"words\":{}}}", json_str(words)),
        RuleCondition::Cc { words } => format!("{{\"kind\":\"cc\",\"words\":{}}}", json_str(words)),
        RuleCondition::Domain { domain } => format!("{{\"kind\":\"domain\",\"domain\":{}}}", json_str(domain)),
        RuleCondition::SubjectContains { words } => format!("{{\"kind\":\"subject\",\"words\":{}}}", json_str(words)),
        RuleCondition::BodyContains { words } => format!("{{\"kind\":\"body\",\"words\":{}}}", json_str(words)),
        RuleCondition::ListHeader { words } => format!("{{\"kind\":\"list_header\",\"words\":{}}}", json_str(words)),
        RuleCondition::Account { account_id } => format!("{{\"kind\":\"account\",\"account_id\":{}}}", json_str(account_id)),
        RuleCondition::HasAttachment => "{\"kind\":\"has_attachment\"}".to_string(),
        // round 18: the negated twin of a text or identity condition: its field's key and its value
        RuleCondition::Not { field, value } => format!("{{\"kind\":\"not\",\"field\":{},\"value\":{}}}", json_str(field.key()), json_str(value)),
        // delta 17: one of the eight facts with the value wanted, as a JSON boolean
        RuleCondition::Fact { fact, wanted } => format!("{{\"kind\":\"fact\",\"fact\":{},\"wanted\":{}}}", json_str(&message_fact_key(*fact)), wanted),
    };
    format!("[{}]", cs.iter().map(one).collect::<Vec<_>>().join(","))
}

/// Reads `rule.conditions_json` back through SQLite's own JSON functions (no parser of ours); an unknown kind is refused in words.
pub fn decode_conditions(c: &Connection, json: &str) -> Result<Vec<RuleCondition>, StoreError> {
    // json_extract hands a JSON true/false back as the integer 1/0; a fact's key comes back as text
    let mut s = c.prepare("SELECT json_extract(value, '$.kind'), json_extract(value, '$.words'), json_extract(value, '$.domain'), json_extract(value, '$.account_id'), json_extract(value, '$.fact'), json_extract(value, '$.wanted'), json_extract(value, '$.field'), json_extract(value, '$.value') FROM json_each(?1) ORDER BY key")?;
    type Row = (String, Option<String>, Option<String>, Option<String>, Option<String>, Option<i64>, Option<String>, Option<String>);
    let rows = s.query_map(params![json], |r| Ok((r.get::<_, String>(0)?, r.get::<_, Option<String>>(1)?, r.get::<_, Option<String>>(2)?, r.get::<_, Option<String>>(3)?, r.get::<_, Option<String>>(4)?, r.get::<_, Option<i64>>(5)?, r.get::<_, Option<String>>(6)?, r.get::<_, Option<String>>(7)?)))?.collect::<Result<Vec<Row>, _>>()?;
    rows.into_iter().map(|(k, w, d, a, fact, wanted, nfield, nvalue)| {
        let words = || w.clone().ok_or_else(|| StoreError::Invalid(format!("a {k} condition in the store has no words")));
        Ok(match k.as_str() {
            "from" => RuleCondition::From { words: words()? }, "to" => RuleCondition::To { words: words()? }, "cc" => RuleCondition::Cc { words: words()? },
            "subject" => RuleCondition::SubjectContains { words: words()? }, "body" => RuleCondition::BodyContains { words: words()? },
            "list_header" => RuleCondition::ListHeader { words: words()? },
            "domain" => RuleCondition::Domain { domain: d.ok_or_else(|| StoreError::Invalid("a domain condition in the store has no domain".into()))? },
            "account" => RuleCondition::Account { account_id: a.ok_or_else(|| StoreError::Invalid("an account condition in the store has no account".into()))? },
            "has_attachment" => RuleCondition::HasAttachment,
            "not" => {
                let key = nfield.ok_or_else(|| StoreError::Invalid("a negated condition in the store does not say which field".into()))?;
                let field = contracts::RuleField::from_key(&key).ok_or_else(|| StoreError::Invalid(format!("unknown field {key:?} in a stored negated condition")))?;
                RuleCondition::Not { field, value: nvalue.ok_or_else(|| StoreError::Invalid("a negated condition in the store has no value".into()))? }
            }
            "fact" => {
                let key = fact.ok_or_else(|| StoreError::Invalid("a fact condition in the store does not say which fact".into()))?;
                let fact = message_fact_from_key(key.clone()).ok_or_else(|| StoreError::Invalid(format!("unknown fact {key:?} in a stored rule")))?;
                let wanted = match wanted {
                    Some(1) => true,
                    Some(0) => false,
                    _ => return Err(StoreError::Invalid(format!("the fact condition {key:?} in the store does not say yes or no"))),
                };
                RuleCondition::Fact { fact, wanted }
            }
            other => return Err(StoreError::Invalid(format!("unknown rule condition {other:?} in the store"))),
        })
    }).collect()
}

const LABEL_SELECT: &str = "SELECT id, name, parent_id, colour, path, depth, description, pinned, scope_account_id, kind, sort_order, \
    entry_enabled, ai_enabled, exit_enabled, ai_reads_subject, ai_reads_body, ai_reads_attachments, exit_fallback, ai_reads_thread FROM label";

fn label_from(r: &rusqlite::Row<'_>) -> rusqlite::Result<MailLabel> {
    let scope: Option<String> = r.get(8)?;
    let kind: String = r.get(9)?;
    Ok(MailLabel {
        id: r.get(0)?, name: r.get(1)?, parent_id: r.get(2)?, colour: r.get(3)?,
        account_ids: scope.iter().cloned().collect(), path: r.get(4)?, depth: r.get::<_, i64>(5)? as u8, unread: 0,
        description: r.get(6)?, pinned: r.get::<_, i64>(7)? == 1,
        scope: match scope { Some(a) => LabelScope::Account { account_id: a }, None => LabelScope::All },
        kind: if kind == "builtin" { LabelKind::BuiltIn } else { LabelKind::Owner }, order: r.get::<_, i64>(10)? as u32,
        // delta 17 (migration 0009): the three switches, what the AI reads, where the exit filter sends the others
        entry_enabled: r.get::<_, i64>(11)? == 1, ai_enabled: r.get::<_, i64>(12)? == 1, exit_enabled: r.get::<_, i64>(13)? == 1,
        ai_reads: AiReads { subject: r.get::<_, i64>(14)? == 1, body: r.get::<_, i64>(15)? == 1, rest_of_thread: r.get::<_, i64>(18)? == 1, attachments: r.get::<_, i64>(16)? == 1 },
        exit_fallback: r.get(17)?,
    })
}

/// The `rule.placement` word of a chain's card.
pub(crate) fn placement_word(p: ChainPlacement) -> &'static str { match p { ChainPlacement::Entry => "entry", ChainPlacement::Exit => "exit" } }
fn placement_from(w: &str) -> ChainPlacement { if w == "exit" { ChainPlacement::Exit } else { ChainPlacement::Entry } }

/// The `label_assignment.flow_card` word of the card that placed a message; None = by hand or the default.
fn card_word(c: WhyCard) -> Option<&'static str> {
    match c { WhyCard::Entry => Some("entry"), WhyCard::Ai => Some("ai"), WhyCard::Exit => Some("exit"), WhyCard::Fallback => Some("fallback"), WhyCard::Hand => None }
}
fn card_from(w: Option<&str>) -> WhyCard {
    match w { Some("entry") => WhyCard::Entry, Some("ai") => WhyCard::Ai, Some("exit") => WhyCard::Exit, Some("fallback") => WhyCard::Fallback, _ => WhyCard::Hand }
}

/// Lane fix-aisort round 11: the `decided_model` of a kept row that sets a mail aside (the judge refused the mail itself: its content, language
/// or size, or no label of the list). Stored under the on-device decider 'laya', so every clean-up by decider takes it too.
pub const SET_ASIDE_MODEL: &str = "apple-refused";

/// Lane fix-aisort round 17: the `decided_model` of a row that is NOT an answer: the AI could not judge this mail for this label (busy, not
/// ready, a failure that can pass, no model yet), so the mail waits for it. On a label with no step 1 (Needs Reply as seeded) nothing else
/// says the mail waits (`waiting_for_ai` counted only what step 1 picked); this row does, and the background retry asks it again. It is no
/// "no": every reader of the kept answers leaves it out (`ai_no_labels`, `ai_no_messages`), every clean-up of the kept answers keeps it, and
/// the mail's judgement (an answer, a set-aside) replaces or removes it.
pub const WAITING_MODEL: &str = "ai-waiting";

/// The decider from the stored columns. Migration 0031: `decided_by 'laya'` with `decided_model 'apple'` is Apple's on-device model.
/// Lane decided-model (SPEC A133): `decided_by 'laya'` with any other `decided_model` is that catalogue model, except the two words that
/// are no model (`SET_ASIDE_MODEL`, `WAITING_MODEL`).
pub fn decided_by_from(word: &str, rule: Option<String>, model: Option<&str>) -> DecidedBy {
    match (word, model) {
        ("laya", Some("apple")) => DecidedBy::Apple,
        ("laya", Some(id)) if id != SET_ASIDE_MODEL && id != WAITING_MODEL && !id.is_empty() => DecidedBy::Model { model: id.to_string() },
        ("rule", _) => DecidedBy::Rule { rule_id: rule.unwrap_or_default() },
        ("laya", _) => DecidedBy::Laya,
        ("jev", _) => DecidedBy::Jev,
        ("owner", _) => DecidedBy::Owner,
        _ => DecidedBy::Default,
    }
}

/// The stored columns of a decider: `decided_by` (the CHECK's words) and `decided_model` (migration 0031: 'apple' for Apple's on-device
/// model, which is stored as the on-device decider 'laya'; lane decided-model: the catalogue id of any other on-device model, stored the
/// same way; NULL otherwise).
pub fn decided_by_columns(by: &DecidedBy) -> (&'static str, Option<String>, Option<String>) {
    match by {
        DecidedBy::Rule { rule_id } => ("rule", Some(rule_id.clone()), None),
        DecidedBy::Laya => ("laya", None, None),
        DecidedBy::Apple => ("laya", None, Some("apple".to_string())),
        DecidedBy::Model { model } => ("laya", None, Some(model.clone())),
        DecidedBy::Jev => ("jev", None, None),
        DecidedBy::Owner => ("owner", None, None),
        DecidedBy::Default => ("default", None, None),
    }
}

/// The id of a chain's rule record. Entry chain 0 is the sheet's rule set (`LabelSpec.rules`), stored as delta 9 stored it
/// (`rule-<label>`); every other chain is `rule-<label>-<entry|exit>-<n>`. Unique because (label, placement, chain) is.
fn chain_rule_id(label_id: &str, p: ChainPlacement, chain_id: u32) -> String {
    if p == ChainPlacement::Entry && chain_id == 0 { rule_id_for(label_id) } else { format!("rule-{label_id}-{}-{chain_id}", placement_word(p)) }
}

fn not_found(kind: &str, id: impl Into<String>) -> CoreError { CoreError::NotFound { kind: kind.into(), id: id.into() } }

/// Every label rule (both cards, every chain), in position order: what the decision lane builds a label flow from.
fn label_rules_all(c: &Connection) -> Result<Vec<Rule>, StoreError> {
    let mut s = c.prepare(
        "SELECT id, position, conditions_json, enabled, action_label_id, match_mode, placement, chain_id FROM rule \
         WHERE action_kind = 'label' AND action_label_id IS NOT NULL ORDER BY position, id",
    )?;
    type Row = (String, i64, String, i64, String, String, String, i64);
    let rows = s.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?, r.get(7)?)))?.collect::<Result<Vec<Row>, _>>()?;
    rows.into_iter()
        .map(|(id, position, json, enabled, label_id, mode, placement, chain)| {
            Ok(Rule {
                id, position: position.max(0) as u32, conditions: decode_conditions(c, &json)?, tab_id: String::new(), enabled: enabled == 1,
                action: RuleAction::Label { label_id }, match_mode: if mode == "all" { MatchMode::All } else { MatchMode::Any },
                placement: placement_from(&placement), chain_id: chain.max(0) as u32,
            })
        })
        .collect()
}

/// The chains of one (label, placement), by number.
fn read_chains(c: &Connection, label_id: &str, p: ChainPlacement) -> Result<Vec<LabelChain>, StoreError> {
    let mut s = c.prepare("SELECT chain_id, conditions_json, match_mode FROM rule WHERE action_label_id = ?1 AND placement = ?2 ORDER BY chain_id")?;
    let rows = s.query_map(params![label_id, placement_word(p)], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?)))?.collect::<Result<Vec<_>, _>>()?;
    rows.into_iter()
        .map(|(n, json, mode)| Ok(LabelChain { chain_id: n.max(0) as u32, rules: RuleSet { conditions: decode_conditions(c, &json)?, match_mode: if mode == "all" { MatchMode::All } else { MatchMode::Any } } }))
        .collect()
}

/// What the foot of a label's inspector counts, one row per (message, place): every filing with the card that placed it (the AI's pick
/// is the label itself for the AI and the exit card, the rejection's label for a fallback filing), then every rejection that left the
/// message under no label (Primary or Junk). `count_label_flow` turns these into the four figures; nothing is stored twice.
fn flow_filings(c: &Connection) -> Result<Vec<FlowFiling>, StoreError> {
    let mut s = c.prepare(
        "SELECT la.message_id, la.label_id, la.flow_card, fr.ai_label_id FROM label_assignment la \
         LEFT JOIN flow_rejection fr ON fr.message_id = la.message_id AND fr.sent_to = la.label_id",
    )?;
    let rows = s.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, Option<String>>(2)?, r.get::<_, Option<String>>(3)?)))?.collect::<Result<Vec<_>, _>>()?;
    let mut out: Vec<FlowFiling> = rows
        .into_iter()
        .map(|(message_id, label_id, card, rejected_from)| {
            let card = card_from(card.as_deref());
            let ai_pick = match card { WhyCard::Ai | WhyCard::Exit => Some(label_id.clone()), WhyCard::Fallback => rejected_from, _ => None };
            FlowFiling { message_id, ended_in: Some(label_id), card, ai_pick }
        })
        .collect();
    let mut s = c.prepare("SELECT message_id, ai_label_id FROM flow_rejection WHERE sent_to IN ('primary', 'junk')")?;
    for row in s.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))? {
        let (message_id, ai) = row?;
        out.push(FlowFiling { message_id, ended_in: None, card: WhyCard::Fallback, ai_pick: Some(ai) });
    }
    Ok(out)
}

impl Store {
    /// What decides a stored mail's kind for the notch (`contracts::notch_kind_of`): its sign-in code, the VIP fact of `rule_facts_of` (the read the label
    /// rules use: an address, the sender's domain or a parent of it), its category tab and its list header. None for an id the store does not hold.
    pub fn notch_facts(&self, message_id: &str) -> Result<Option<contracts::NotchFacts>, StoreError> {
        self.read(|c| {
            let Some(f) = rule_facts_of(c, message_id)? else { return Ok(None) };
            let (code, tab): (Option<String>, Option<String>) = c.query_row(
                "SELECT m.login_code, (SELECT mc.tab_id FROM message_category mc WHERE mc.message_id = m.id) FROM message m WHERE m.id = ?1",
                params![message_id], |r| Ok((r.get(0)?, r.get(1)?)))?;
            Ok(Some(contracts::NotchFacts {
                has_sign_in_code: code.map(|c| !c.trim().is_empty()).unwrap_or(false), vip_sender: f.vip_sender, tab_id: tab, has_list_header: !f.list_header.is_empty(),
            }))
        })
    }
}

fn read_rejection(c: &Connection, message_id: &str) -> Result<Option<FlowRejection>, StoreError> {
    Ok(c.query_row("SELECT message_id, ai_label_id, sent_to, chain_id, decided_at_ms FROM flow_rejection WHERE message_id = ?1", params![message_id], |r| {
        Ok(FlowRejection { message_id: r.get(0)?, ai_label_id: r.get(1)?, sent_to: r.get(2)?, chain_id: r.get::<_, Option<i64>>(3)?.map(|n| n.max(0) as u32), decided_at_ms: r.get(4)? })
    }).optional()?)
}

/// The facts a label rule reads of one stored message, from columns already synced (never a second fetch), for the "why here" read.
/// The four delta 17 facts that are not columns read the same SQL as the smart inbox's `facts_for` (`Store::LATER_IN_THREAD_SQL`,
/// `Store::OWNER_WROTE_EARLIER_SQL`); the VIP fact reads the vip table (an address, or the sender's domain or a parent of it).
fn rule_facts_of(c: &Connection, message_id: &str) -> Result<Option<MessageFacts>, StoreError> {
    let row = c.query_row(
        "SELECT m.id, m.account_id, m.sender_name, m.from_address, m.to_json, m.cc_json, m.subject, COALESCE(m.body_text, m.preview), m.has_list_header, \
                m.read, m.received_at_ms, m.starred, m.thread_id, EXISTS (SELECT 1 FROM attachment a WHERE a.message_id = m.id AND NOT (a.inline OR coalesce(json_extract(CASE WHEN json_valid(a.remote_id) THEN a.remote_id END, '$.inline'), 0))) \
         FROM message m WHERE m.id = ?1",
        params![message_id],
        |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?, r.get::<_, String>(3)?, r.get::<_, String>(4)?, r.get::<_, String>(5)?,
                r.get::<_, String>(6)?, r.get::<_, Option<String>>(7)?, r.get::<_, i64>(8)?, r.get::<_, i64>(9)?, r.get::<_, i64>(10)?, r.get::<_, i64>(11)?,
                r.get::<_, String>(12)?, r.get::<_, i64>(13)?)),
    ).optional()?;
    let Some((id, account_id, from_name, from_address, to_json, cc_json, subject, body, list, read, at, starred, thread, attach)) = row else { return Ok(None) };
    let list_of = |json: &str| -> Result<Vec<String>, StoreError> {
        let mut s = c.prepare_cached("SELECT value FROM json_each(?1)")?;
        let v = s.query_map(params![json], |r| r.get::<_, String>(0))?.collect::<Result<Vec<_>, _>>()?;
        Ok(v)
    };
    let (to, cc) = (list_of(&to_json)?, list_of(&cc_json)?);
    let own: Vec<String> = { let mut s = c.prepare_cached("SELECT lower(address) FROM account")?; let v = s.query_map([], |r| r.get::<_, String>(0))?.collect::<Result<Vec<_>, _>>()?; v };
    let mine = |a: &String| own.contains(&a.trim().to_lowercase());
    let from_owner = own.contains(&from_address.trim().to_lowercase());
    let later: bool = c.query_row(Store::LATER_IN_THREAD_SQL, params![thread, at, id], |r| r.get(0))?;
    let replied: bool = c.query_row(Store::OWNER_WROTE_EARLIER_SQL, params![thread, at, id], |r| r.get(0))?;
    let vips: Vec<(String, String)> = { let mut s = c.prepare_cached("SELECT kind, lower(value) FROM vip")?; let v = s.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<Result<Vec<_>, _>>()?; v };
    let sender = from_address.trim().to_lowercase();
    let domain = sender.rsplit_once('@').map(|(_, d)| d.to_string()).unwrap_or_default();
    let vip_sender = vips.iter().any(|(k, v)| {
        let v = v.trim().trim_start_matches('@');
        if k == "domain" { !v.is_empty() && (domain == v || domain.ends_with(&format!(".{v}"))) } else { v == sender }
    });
    Ok(Some(MessageFacts {
        message_id: id, account_id, from_name, from_address, subject, body: body.unwrap_or_default(), has_attachment: attach == 1,
        // the store keeps the one list-header bit (not the List-Id words): a marker word stands for it, as the decision lane's rule_facts does
        list_header: if list == 1 { "list".into() } else { String::new() },
        unread: read == 0, received_at_ms: at,
        unanswered: !from_owner && !later, starred: starred == 1, vip_sender,
        cc_me: cc.iter().any(&mine) && !to.iter().any(&mine), replied_by_me: replied,
        to, cc,
    }))
}

impl Store {
    /// "A later message in this thread" over (?1 thread id, ?2 received_at_ms, ?3 message id): the waiting definition of counts.rs
    /// ("the owner has not answered it" = inbound and nothing newer in the thread) asks it with NOT. One text for both readers of the
    /// delta 17 facts (this store's "why here" facts and the smart inbox's facts_for), so "unanswered" cannot read two ways.
    pub const LATER_IN_THREAD_SQL: &'static str = "SELECT EXISTS (SELECT 1 FROM message n WHERE n.thread_id = ?1 \
        AND (n.received_at_ms > ?2 OR (n.received_at_ms = ?2 AND n.id > ?3)))";
    /// "Already answered by me" (owner ruling 2026-11-10, replacing "a later message of mine"): I wrote EARLIER in this thread, that is at
    /// least one stored message from one of the owner's addresses (any folder, Sent included) dated before this message (?2 received_at_ms,
    /// ties by id ?3). A thread where I replied and a new unread message arrived is yes; a thread I never wrote in is no.
    pub const OWNER_WROTE_EARLIER_SQL: &'static str = "SELECT EXISTS (SELECT 1 FROM message n WHERE n.thread_id = ?1 \
        AND (n.received_at_ms < ?2 OR (n.received_at_ms = ?2 AND n.id < ?3)) \
        AND lower(n.from_address) IN (SELECT lower(address) FROM account))";
}

fn all(c: &Connection) -> Result<Vec<MailLabel>, StoreError> {
    let mut s = c.prepare(&format!("{LABEL_SELECT} ORDER BY sort_order, id"))?;
    let rows = s.query_map([], label_from)?.collect::<Result<Vec<_>, _>>()?;
    Ok(contracts::ordered_tree(rows))
}

/// real-labels: the owner labels that are tabs now (pinned), in tree order; the counts list them after the split tabs (counts.rs `build`).
pub(crate) fn pinned_owner_label_ids(c: &Connection) -> Result<Vec<String>, StoreError> {
    Ok(all(c)?.into_iter().filter(|l| l.pinned && l.kind == LabelKind::Owner).map(|l| l.id).collect())
}

fn rule_id_for(label_id: &str) -> String { format!("rule-{label_id}") }

fn write_rules(c: &Connection, label_id: &str, rules: &RuleSet, now_ms: i64) -> Result<(), StoreError> {
    let id = rule_id_for(label_id);
    if rules.conditions.is_empty() {
        c.execute("DELETE FROM rule WHERE id = ?1", params![id])?;
        return Ok(());
    }
    let mode = if rules.match_mode == MatchMode::All { "all" } else { "any" };
    // delta 17: the sheet's rule set is the label's first ENTRY chain. A new row takes chain 0, or the next free number when migration
    // 0009 numbered another rule of this label 0 (the unique index on (label, placement, chain) would refuse a second 0).
    c.execute(
        "INSERT INTO rule (id, position, conditions_json, tab_id, enabled, updated_at_ms, action_kind, action_label_id, match_mode, placement, chain_id) \
         VALUES (?1, (SELECT coalesce(max(position) + 1, 0) FROM rule), ?2, '', 1, ?3, 'label', ?4, ?5, 'entry', \
                 (SELECT CASE WHEN EXISTS (SELECT 1 FROM rule WHERE action_label_id = ?4 AND placement = 'entry' AND chain_id = 0) \
                              THEN (SELECT max(chain_id) + 1 FROM rule WHERE action_label_id = ?4 AND placement = 'entry') ELSE 0 END)) \
         ON CONFLICT(id) DO UPDATE SET conditions_json = excluded.conditions_json, match_mode = excluded.match_mode, enabled = 1, updated_at_ms = excluded.updated_at_ms",
        params![id, encode_conditions(&rules.conditions), now_ms, label_id, mode],
    )?;
    Ok(())
}

fn read_rules(c: &Connection, label_id: &str) -> Result<RuleSet, StoreError> {
    let row: Option<(String, String)> = c.query_row("SELECT conditions_json, match_mode FROM rule WHERE id = ?1 AND action_label_id = ?2", params![rule_id_for(label_id), label_id], |r| Ok((r.get(0)?, r.get(1)?))).optional()?;
    Ok(match row {
        Some((json, mode)) => RuleSet { conditions: decode_conditions(c, &json)?, match_mode: if mode == "all" { MatchMode::All } else { MatchMode::Any } },
        None => RuleSet { conditions: vec![], match_mode: MatchMode::Any },
    })
}

fn kind_word(k: SuggestionKind) -> &'static str {
    match k { SuggestionKind::BlockSender => "block_sender", SuggestionKind::AlwaysJunkDomain => "always_junk_domain", SuggestionKind::Unsubscribe => "unsubscribe", SuggestionKind::MuteThread => "mute_thread", SuggestionKind::AlwaysLabel => "always_label" }
}
fn kind_from(w: &str) -> Result<SuggestionKind, StoreError> {
    Ok(match w { "block_sender" => SuggestionKind::BlockSender, "always_junk_domain" => SuggestionKind::AlwaysJunkDomain, "unsubscribe" => SuggestionKind::Unsubscribe, "mute_thread" => SuggestionKind::MuteThread, "always_label" => SuggestionKind::AlwaysLabel,
        other => return Err(StoreError::Invalid(format!("unknown suggestion kind {other:?} in the store"))) })
}

impl WriteCtx<'_> {
    fn get_full(&self, id: &str) -> Result<Option<MailLabel>, StoreError> {
        Ok(self.tx.query_row(&format!("{LABEL_SELECT} WHERE id = ?1"), params![id], label_from).optional()?)
    }
    fn must_get(&self, id: &str) -> Result<MailLabel, StoreError> { self.get_full(id)?.ok_or_else(|| StoreError::Invalid(format!("there is no label {id}"))) }
    fn set_fields(&mut self, id: &str, spec: &LabelSpec, now_ms: i64) -> Result<(), StoreError> {
        // A140 part 3: a changed description is a changed question: the AI's kept "no" for this label goes (`ai_label_no`, migration 0022)
        if self.get_full(id)?.is_some_and(|l| l.description != spec.description) {
            self.ai_no_clear_label(id)?;
        }
        let scope = match &spec.scope { LabelScope::All => None, LabelScope::Account { account_id } => Some(account_id.clone()) };
        self.tx.execute("UPDATE label SET colour = ?2, description = ?3, scope_account_id = ?4, pinned = ?5, updated_at_ms = ?6 WHERE id = ?1",
            params![id, spec.colour, spec.description, scope, spec.pinned, now_ms]).map_err(|e| StoreError::Invalid(e.to_string()))?;
        write_rules(self.tx, id, &spec.rules, now_ms)
    }

    /// Creates a label from the sheet: checked by the contract first, then written with its rule. Returns the new label.
    pub fn label_create_full(&mut self, spec: LabelSpec, now_ms: i64) -> Result<MailLabel, StoreError> {
        let parent = match &spec.parent_id { Some(p) => Some(self.must_get(p)?), None => None };
        contracts::validate_label_spec(spec.clone(), parent).map_err(refusal)?;
        let id = new_id("label");
        self.tx.execute(
            "INSERT INTO label (id, name, parent_id, account_ids_json, updated_at_ms, sort_order, scope_account_id) VALUES (?1, ?2, ?3, '[]', ?4, (SELECT coalesce(max(sort_order) + 1, 0) FROM label WHERE parent_id IS ?3), ?5)",
            params![id, spec.name.trim(), spec.parent_id, now_ms, match &spec.scope { LabelScope::All => None, LabelScope::Account { account_id } => Some(account_id.clone()) }],
        ).map_err(|e| StoreError::Invalid(e.to_string()))?;
        self.set_fields(&id, &spec, now_ms)?;
        self.must_get(&id)
    }

    /// Replaces every editable field with the sheet's. A rename or a new parent on a built-in category is refused in words; a changed
    /// parent is a move to the end of the new parent's children.
    pub fn label_update_full(&mut self, id: &str, spec: LabelSpec, now_ms: i64) -> Result<MailLabel, StoreError> {
        let cur = self.must_get(id)?;
        let parent = match &spec.parent_id { Some(p) => Some(self.must_get(p)?), None => None };
        contracts::validate_label_spec(spec.clone(), parent).map_err(refusal)?;
        if spec.name.trim() != cur.name { contracts::check_label_rename(cur.clone()).map_err(refusal)?; self.tx.execute("UPDATE label SET name = ?2 WHERE id = ?1", params![id, spec.name.trim()]).map_err(|e| StoreError::Invalid(e.to_string()))?; }
        if spec.parent_id != cur.parent_id {
            let end = u32::MAX;
            let moved = contracts::reorder_label(all(self.tx)?, id.to_string(), spec.parent_id.clone(), end).map_err(refusal)?;
            self.write_order(&moved)?;
        }
        self.set_fields(id, &spec, now_ms)?;
        self.must_get(id)
    }

    fn write_order(&mut self, labels: &[MailLabel]) -> Result<(), StoreError> {
        for l in labels { self.tx.execute("UPDATE label SET sort_order = ?2, parent_id = ?3 WHERE id = ?1 AND (sort_order <> ?2 OR parent_id IS NOT ?3)", params![l.id, l.order, l.parent_id]).map_err(|e| StoreError::Invalid(e.to_string()))?; }
        Ok(())
    }

    /// Drag in the sidebar. The contract's `reorder_label` decides; the new order and parent are written; paths follow by trigger.
    pub fn label_reorder(&mut self, id: &str, new_parent: Option<String>, new_index: u32) -> Result<Vec<MailLabel>, StoreError> {
        let moved = contracts::reorder_label(all(self.tx)?, id.to_string(), new_parent, new_index).map_err(refusal)?;
        // parents first so the depth and loop triggers see the final tree one step at a time
        self.write_order(&moved)?;
        all(self.tx)
    }

    /// Pin or unpin. The last pinned label cannot be unpinned (the list always has a first tab), refused in words.
    pub fn label_pin(&mut self, id: &str, pinned: bool, now_ms: i64) -> Result<(), StoreError> {
        let cur = self.must_get(id)?;
        if !pinned && cur.pinned && self.tx.query_row("SELECT count(*) FROM label WHERE pinned = 1", [], |r| r.get::<_, i64>(0))? <= 1 {
            return Err(StoreError::Invalid("at least one label has to stay pinned: the list always has a first tab".into()));
        }
        self.tx.execute("UPDATE label SET pinned = ?2, updated_at_ms = ?3 WHERE id = ?1", params![id, pinned, now_ms])?;
        self.label_tab_follow(id)
    }

    /// real-labels: a built-in label's tab is its split tab (the categories' counts, counts.rs), so its pin is that tab's on/off, as the
    /// test engine keeps them together (Sources/CoreBridge/FixtureLabels.swift `syncTabsFromLabels`). An owner label has no split tab:
    /// nothing changes (its tab is its pin, counts.rs `build`). A tab already in step is not written.
    pub fn label_tab_follow(&mut self, id: &str) -> Result<(), StoreError> {
        self.tx.execute(
            "UPDATE split_tab SET enabled = (SELECT l.pinned FROM label l WHERE l.id = ?1) \
             WHERE id = ?1 AND EXISTS (SELECT 1 FROM label l WHERE l.id = ?1 AND l.kind = 'builtin') \
               AND enabled IS NOT (SELECT l.pinned FROM label l WHERE l.id = ?1)",
            params![id],
        )?;
        Ok(())
    }

    /// Deletes an owner's label with the labels under it; a built-in is refused in words. Messages stay; their filing under it goes.
    pub fn label_delete_tree(&mut self, id: &str) -> Result<usize, StoreError> {
        let cur = self.must_get(id)?;
        contracts::check_label_delete(cur).map_err(refusal)?;
        let ids: Vec<String> = {
            let mut s = self.tx.prepare("SELECT id FROM label WHERE id = ?1 OR path >= (SELECT path FROM label WHERE id = ?1) || ' > ' AND path < (SELECT path FROM label WHERE id = ?1) || ' ?' ORDER BY depth DESC")?;
            let v = s.query_map(params![id], |r| r.get(0))?.collect::<Result<Vec<String>, _>>()?;
            v
        };
        for d in &ids { self.tx.execute("DELETE FROM label WHERE id = ?1", params![d])?; }
        Ok(ids.len())
    }

    /// Round 13 (owner, main106: the Needs Reply tab read "All done" while 25 mails were filed under it): a built-in label is also a TAB, and a tab lists
    /// the mail whose `message_category` row names it (`read::tab_matches`; `label_tab` leaves the built-ins to that walk). The flow files by `label_assignment`
    /// alone, so a mail the AI put under Needs Reply was never on its tab and the strip counted none. Filing under a built-in label now also puts the mail in
    /// that tab (one row per message, the newest decision replaces the classifier's), and taking it off the label takes it off the tab (`builtin_tab_drop`).
    pub(crate) fn builtin_tab_put(&mut self, label_id: &str, message_id: &str, word: &str, rule: Option<&str>, probability: Option<f64>, now_ms: i64) -> Result<(), StoreError> {
        if !self.tx.prepare_cached("SELECT 1 FROM label WHERE id = ?1 AND kind = 'builtin'")?.exists(params![label_id])? {
            return Ok(());
        }
        self.tx.execute(
            "INSERT INTO message_category (message_id, tab_id, decided_by, rule_id, probability, decided_at_ms, updated_at_ms) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6) \
             ON CONFLICT(message_id) DO UPDATE SET tab_id = excluded.tab_id, decided_by = excluded.decided_by, rule_id = excluded.rule_id, probability = excluded.probability, \
             decided_at_ms = excluded.decided_at_ms, updated_at_ms = excluded.updated_at_ms",
            params![message_id, label_id, word, rule, probability, now_ms],
        ).map_err(|e| StoreError::Invalid(e.to_string()))?;
        Ok(())
    }

    /// The message leaves a built-in label: it leaves that label's tab too (back to the Primary walk). A message whose tab is another one stays where it is.
    /// The mail an earlier build filed under a built-in label (a `label_assignment` row) that has no tab row yet: it gets the label's tab, with the filing's own
    /// decider. A mail the classifier already put in a tab keeps it. Declared first, so the counts move by the difference. Returns how many were put.
    pub(crate) fn backfill_builtin_tabs(&mut self) -> Result<usize, StoreError> {
        const FROM: &str = "FROM label_assignment la JOIN label l ON l.id = la.label_id AND l.kind = 'builtin' LEFT JOIN message_category mc ON mc.message_id = la.message_id WHERE mc.message_id IS NULL";
        let ids: Vec<String> = {
            let mut st = self.tx.prepare(&format!("SELECT DISTINCT la.message_id {FROM}"))?;
            let rows = st.query_map([], |r| r.get::<_, String>(0))?;
            rows.collect::<Result<Vec<_>, _>>()?
        };
        if ids.is_empty() {
            return Ok(0);
        }
        self.will_change(&ids)?;
        self.tx.execute(
            &format!("INSERT OR IGNORE INTO message_category (message_id, tab_id, decided_by, rule_id, probability, decided_at_ms, updated_at_ms) \
                      SELECT la.message_id, la.label_id, la.decided_by, la.rule_id, la.probability, COALESCE(la.decided_at_ms, la.updated_at_ms), la.updated_at_ms {FROM}"),
            [],
        ).map_err(|e| StoreError::Invalid(e.to_string()))?;
        Ok(ids.len())
    }

    pub(crate) fn builtin_tab_drop(&mut self, label_id: &str, message_id: &str) -> Result<(), StoreError> {
        self.tx.execute(
            "DELETE FROM message_category WHERE message_id = ?1 AND tab_id = ?2 AND EXISTS (SELECT 1 FROM label WHERE id = ?2 AND kind = 'builtin')",
            params![message_id, label_id],
        ).map_err(|e| StoreError::Invalid(e.to_string()))?;
        Ok(())
    }

    /// Files a message under a label with who decided it (the owner, a rule, the models). A second filing replaces the first's decision.
    pub fn label_file(&mut self, label_id: &str, message_id: &str, by: &DecidedBy, probability: Option<f64>, now_ms: i64) -> Result<(), StoreError> {
        self.label_file_entry(label_id, message_id, by, None, probability, now_ms)
    }

    /// `label_file` with the decision model's page entry that decided (delta 12, `decided_entry_id`). The entry is kept
    /// only when the model decided (Laya, Jev); for the owner, a rule or the default it is None whatever is passed.
    pub fn label_file_entry(&mut self, label_id: &str, message_id: &str, by: &DecidedBy, entry: Option<&str>, probability: Option<f64>, now_ms: i64) -> Result<(), StoreError> {
        // contracts delta 11: a label the owner removed from this thread is never filed back by a rule or the model
        if !contracts::may_file_label(by.clone(), self.label_removed_by_user(label_id, message_id)?) {
            return Ok(());
        }
        if matches!(by, DecidedBy::Owner) {
            self.label_removal_lift(label_id, message_id)?;
        }
        let (word, rule, model) = decided_by_columns(by);
        let entry = if matches!(by, DecidedBy::Laya | DecidedBy::Jev | DecidedBy::Apple | DecidedBy::Model { .. }) { entry.map(str::to_string) } else { None };
        self.tx.execute(
            "INSERT INTO label_assignment (label_id, message_id, updated_at_ms, decided_by, rule_id, probability, decided_at_ms, decided_entry_id, decided_model) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?3, ?7, ?8) \
             ON CONFLICT(label_id, message_id) DO UPDATE SET decided_by = excluded.decided_by, decided_model = excluded.decided_model, rule_id = excluded.rule_id, probability = excluded.probability, decided_at_ms = excluded.decided_at_ms, updated_at_ms = excluded.updated_at_ms, decided_entry_id = excluded.decided_entry_id",
            params![label_id, message_id, now_ms, word, rule, probability, entry, model],
        ).map_err(|e| StoreError::Invalid(e.to_string()))?;
        self.builtin_tab_put(label_id, message_id, word, rule.as_deref(), probability, now_ms)?;
        Ok(())
    }

    /// Blocks a sender or a domain; a second block of the same target changes nothing (the first time stays).
    pub fn block_add(&mut self, target: &BlockTarget, now_ms: i64) -> Result<bool, StoreError> {
        let (kind, value) = match target { BlockTarget::Sender { address } => ("sender", address.trim().to_lowercase()), BlockTarget::Domain { domain } => ("domain", domain.trim().trim_start_matches('@').to_lowercase()) };
        Ok(self.tx.execute("INSERT OR IGNORE INTO block (kind, value, since_ms) VALUES (?1, ?2, ?3)", params![kind, value, now_ms]).map_err(|e| StoreError::Invalid(e.to_string()))? == 1)
    }

    pub fn suggestion_put(&mut self, s: &Suggestion) -> Result<(), StoreError> {
        self.tx.execute("INSERT INTO suggestion (id, kind, subject, label_id, offered_at_ms, accepted) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![s.id, kind_word(s.kind), s.subject, s.label_id, s.offered_at_ms, s.accepted]).map_err(|e| StoreError::Invalid(e.to_string()))?;
        Ok(())
    }

    /// Takes a suggestion up: the contract says what to write (`suggestion_effect`), the block is written here, and the suggestion is marked
    /// accepted. The effect is returned so the caller does the rest (a mute, an unsubscribe, a rule). A second accept is refused in words.
    pub fn suggestion_accept(&mut self, id: &str, now_ms: i64) -> Result<SuggestionEffect, StoreError> {
        let s = read_suggestion(self.tx, id)?.ok_or_else(|| StoreError::Invalid(format!("there is no suggestion {id}")))?;
        let effect = contracts::suggestion_effect(s).map_err(refusal)?;
        if let SuggestionEffect::Block { target } = &effect { self.block_add(target, now_ms)?; }
        if let SuggestionEffect::LabelRule { address, label_id } = &effect {
            let mut rules = read_rules(self.tx, label_id)?;
            if rules.match_mode == MatchMode::All && !rules.conditions.is_empty() {
                return Err(StoreError::Invalid("this label's rules need all of their conditions; add the sender in the label's sheet".into()));
            }
            rules.match_mode = MatchMode::Any;
            rules.conditions.push(RuleCondition::From { words: address.clone() });
            write_rules(self.tx, label_id, &rules, now_ms)?;
        }
        self.tx.execute("UPDATE suggestion SET accepted = 1 WHERE id = ?1", params![id])?;
        Ok(effect)
    }

    /// fix-suggest: a dismissal keeps the row as a tombstone (its id becomes `dismissed:<rowid>:<id>`), so the engine never offers the same
    /// kind and subject again (`suggestion_offer`); every read leaves tombstones out, so the dismissed id answers nothing, as before.
    pub fn suggestion_dismiss(&mut self, id: &str) -> Result<bool, StoreError> {
        Ok(self.tx.execute("UPDATE suggestion SET id = 'dismissed:' || rowid || ':' || id WHERE id = ?1 AND accepted = 0 AND id NOT LIKE 'dismissed:%'", params![id])? == 1)
    }

    /// fix-suggest: the engine's offer after an act (engine/src/suggest_offers.rs). One offer per kind and subject, ever: nothing is offered
    /// when a row of that kind and subject exists, on offer, accepted or dismissed. Returns the new suggestion's id, or None.
    pub fn suggestion_offer(&mut self, kind: SuggestionKind, subject: &str, label_id: Option<&str>, now_ms: i64) -> Result<Option<String>, StoreError> {
        let subject = subject.trim();
        if subject.is_empty() { return Ok(None); }
        let seen: bool = self.tx.query_row("SELECT EXISTS (SELECT 1 FROM suggestion WHERE kind = ?1 AND lower(subject) = lower(?2))", params![kind_word(kind), subject], |r| r.get(0))?;
        if seen { return Ok(None); }
        let s = Suggestion { id: new_id("suggestion"), kind, subject: subject.to_string(), label_id: label_id.map(str::to_string), offered_at_ms: now_ms, accepted: false };
        self.suggestion_put(&s)?;
        Ok(Some(s.id))
    }
}

fn read_suggestion(c: &Connection, id: &str) -> Result<Option<Suggestion>, StoreError> {
    let row = c.query_row("SELECT id, kind, subject, label_id, offered_at_ms, accepted FROM suggestion WHERE id = ?1 AND id NOT LIKE 'dismissed:%'", params![id], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?, r.get::<_, Option<String>>(3)?, r.get::<_, i64>(4)?, r.get::<_, bool>(5)?))).optional()?;
    row.map(|(id, k, subject, label_id, offered_at_ms, accepted)| Ok(Suggestion { id, kind: kind_from(&k)?, subject, label_id, offered_at_ms, accepted })).transpose()
}

impl Store {
    /// Every label in tree order, with its description, pin, scope, kind and order. The built-in categories come first.
    pub fn labels_full(&self) -> Result<Vec<MailLabel>, StoreError> { self.read(all) }
    /// The pinned labels in tree order: the tab strip.
    pub fn pinned_labels(&self) -> Result<Vec<MailLabel>, StoreError> { self.read(|c| Ok(contracts::pinned_in_tree_order(all(c)?))) }
    /// The label's fixed rule set (empty when it has none).
    pub fn label_rules(&self, label_id: &str) -> Result<RuleSet, StoreError> { self.read(|c| read_rules(c, label_id)) }
    /// How a message was filed under a label: who decided it, the rule if a rule did.
    pub fn label_filing(&self, label_id: &str, message_id: &str) -> Result<Option<(DecidedBy, Option<f64>)>, StoreError> {
        self.read(|c| {
            let row: Option<(String, Option<String>, Option<f64>, Option<String>)> = c.query_row("SELECT decided_by, rule_id, probability, decided_model FROM label_assignment WHERE label_id = ?1 AND message_id = ?2", params![label_id, message_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))).optional()?;
            Ok(row.map(|(w, rule, p, model)| (decided_by_from(&w, rule, model.as_deref()), p)))
        })
    }
    /// How many messages are filed under a label now, whoever filed them: the "Re-run on N filed messages" offer (`WhooshCore::label_rerun_count`).
    pub fn label_filed_count(&self, label_id: &str) -> Result<u32, StoreError> {
        self.read(|c| Ok(c.query_row("SELECT count(*) FROM label_assignment WHERE label_id = ?1", params![label_id], |r| r.get::<_, i64>(0))?.max(0) as u32))
    }
    /// The blocked senders and domains, newest first.
    pub fn blocks(&self) -> Result<Vec<Block>, StoreError> {
        self.read(|c| {
            let mut s = c.prepare("SELECT kind, value, since_ms FROM block ORDER BY since_ms DESC, kind, value")?;
            let rows = s.query_map([], |r| { let (k, v): (String, String) = (r.get(0)?, r.get(1)?); Ok(Block { target: if k == "sender" { BlockTarget::Sender { address: v } } else { BlockTarget::Domain { domain: v } }, since_ms: r.get(2)? }) })?;
            Ok(rows.collect::<Result<Vec<_>, _>>()?)
        })
    }
    /// The suggestions on offer (not accepted), newest first.
    pub fn suggestions(&self) -> Result<Vec<Suggestion>, StoreError> {
        self.read(|c| {
            let mut s = c.prepare("SELECT id FROM suggestion WHERE accepted = 0 AND id NOT LIKE 'dismissed:%' ORDER BY offered_at_ms DESC, id")?;
            let ids = s.query_map([], |r| r.get::<_, String>(0))?.collect::<Result<Vec<_>, _>>()?;
            ids.iter().filter_map(|i| read_suggestion(c, i).transpose()).collect()
        })
    }
    pub fn label_create_full(&self, spec: LabelSpec, now_ms: i64) -> Result<MailLabel, StoreError> { self.write(move |ctx| ctx.label_create_full(spec, now_ms)) }
    pub fn label_update_full(&self, id: String, spec: LabelSpec, now_ms: i64) -> Result<MailLabel, StoreError> { self.write(move |ctx| ctx.label_update_full(&id, spec, now_ms)) }
    pub fn label_reorder(&self, id: String, new_parent: Option<String>, new_index: u32) -> Result<Vec<MailLabel>, StoreError> { self.write(move |ctx| ctx.label_reorder(&id, new_parent, new_index)) }
    pub fn label_pin(&self, id: String, pinned: bool, now_ms: i64) -> Result<(), StoreError> { self.write(move |ctx| ctx.label_pin(&id, pinned, now_ms)) }
    pub fn label_delete_tree(&self, id: String) -> Result<usize, StoreError> { self.write(move |ctx| ctx.label_delete_tree(&id)) }
    pub fn label_file(&self, label_id: String, message_id: String, by: DecidedBy, probability: Option<f64>, now_ms: i64) -> Result<(), StoreError> { self.write(move |ctx| ctx.label_file(&label_id, &message_id, &by, probability, now_ms)) }
    pub fn label_file_entry(&self, label_id: String, message_id: String, by: DecidedBy, entry: Option<String>, probability: Option<f64>, now_ms: i64) -> Result<(), StoreError> { self.write(move |ctx| ctx.label_file_entry(&label_id, &message_id, &by, entry.as_deref(), probability, now_ms)) }
    pub fn block_add(&self, target: BlockTarget, now_ms: i64) -> Result<bool, StoreError> { self.write(move |ctx| ctx.block_add(&target, now_ms)) }
    pub fn suggestion_put(&self, s: Suggestion) -> Result<(), StoreError> { self.write(move |ctx| ctx.suggestion_put(&s)) }
    pub fn suggestion_accept(&self, id: String, now_ms: i64) -> Result<SuggestionEffect, StoreError> { self.write(move |ctx| ctx.suggestion_accept(&id, now_ms)) }
    pub fn suggestion_dismiss(&self, id: String) -> Result<bool, StoreError> { self.write(move |ctx| ctx.suggestion_dismiss(&id)) }
}

// ---------------------------------------------------------------- delta 17: a label is a flow of three cards
// The checks are the contract's (`check_set_card`, `check_set_ai_reads`, `check_set_exit_fallback`, `validate_chain`, `next_chain_id`);
// a refusal keeps the contract's code (`CoreError::Invalid { field }`), so the outer Result is the store's trouble and the inner one
// the owner's refusal. A refusal writes nothing.
type Refusable<T> = Result<Result<T, CoreError>, StoreError>;

impl WriteCtx<'_> {
    /// Switches one card of a label. Switching off keeps everything the card holds.
    pub fn label_set_card(&mut self, label_id: &str, card: FlowCard, enabled: bool, now_ms: i64) -> Refusable<()> {
        let Some(label) = self.get_full(label_id)? else { return Ok(Err(not_found("label", label_id))) };
        if let Err(e) = contracts::check_set_card(label, card, enabled) { return Ok(Err(e)); }
        let column = match card { FlowCard::Entry => "entry_enabled", FlowCard::Ai => "ai_enabled", FlowCard::Exit => "exit_enabled" };
        self.tx.execute(&format!("UPDATE label SET {column} = ?2, updated_at_ms = ?3 WHERE id = ?1"), params![label_id, enabled, now_ms])?;
        Ok(Ok(()))
    }

    /// What the AI reads of a message for this label (at least one input).
    pub fn label_set_ai_reads(&mut self, label_id: &str, reads: AiReads, now_ms: i64) -> Refusable<()> {
        if self.get_full(label_id)?.is_none() { return Ok(Err(not_found("label", label_id))); }
        if let Err(e) = contracts::check_set_ai_reads(reads) { return Ok(Err(e)); }
        // A140 part 3: changed "AI reads" ticks change what the AI judged: its kept "no" for this label goes
        if self.get_full(label_id)?.is_some_and(|l| l.ai_reads != reads) {
            self.ai_no_clear_label(label_id)?;
        }
        self.tx.execute(
            "UPDATE label SET ai_reads_subject = ?2, ai_reads_body = ?3, ai_reads_attachments = ?4, ai_reads_thread = ?6, updated_at_ms = ?5 WHERE id = ?1",
            params![label_id, reads.subject, reads.body, reads.attachments, now_ms, reads.rest_of_thread],
        )?;
        Ok(Ok(()))
    }

    /// Where the exit filter sends the others: a label id, "primary" or "junk"; None clears it (refused while the filter is on).
    pub fn label_set_exit_fallback(&mut self, label_id: &str, fallback: Option<String>, now_ms: i64) -> Refusable<()> {
        let Some(label) = self.get_full(label_id)? else { return Ok(Err(not_found("label", label_id))) };
        if let Err(e) = contracts::check_set_exit_fallback(label, fallback.clone(), all(self.tx)?) { return Ok(Err(e)); }
        let stored = fallback.map(|f| f.trim().to_string()).filter(|f| !f.is_empty());
        self.tx.execute("UPDATE label SET exit_fallback = ?2, updated_at_ms = ?3 WHERE id = ?1", params![label_id, stored, now_ms])?;
        Ok(Ok(()))
    }

    /// Adds a chain to one card of a label; returns its number (one past the highest in use, never a freed one).
    pub fn label_chain_add(&mut self, label_id: &str, placement: ChainPlacement, chain: RuleSet, now_ms: i64) -> Refusable<u32> {
        if self.get_full(label_id)?.is_none() { return Ok(Err(not_found("label", label_id))); }
        if let Err(e) = contracts::validate_chain(chain.clone()) { return Ok(Err(e)); }
        let n = contracts::next_chain_id(read_chains(self.tx, label_id, placement)?.into_iter().map(|c| c.chain_id).collect());
        let mode = if chain.match_mode == MatchMode::All { "all" } else { "any" };
        self.tx.execute(
            "INSERT INTO rule (id, position, conditions_json, tab_id, enabled, updated_at_ms, action_kind, action_label_id, match_mode, placement, chain_id) \
             VALUES (?1, (SELECT coalesce(max(position) + 1, 0) FROM rule), ?2, '', 1, ?3, 'label', ?4, ?5, ?6, ?7)",
            params![chain_rule_id(label_id, placement, n), encode_conditions(&chain.conditions), now_ms, label_id, mode, placement_word(placement), n],
        )?;
        Ok(Ok(n))
    }

    /// Replaces one chain whole (the grid, the rows and the mode are one value). An unknown chain is NotFound.
    pub fn label_chain_update(&mut self, label_id: &str, placement: ChainPlacement, chain_id: u32, chain: RuleSet, now_ms: i64) -> Refusable<()> {
        if let Err(e) = contracts::validate_chain(chain.clone()) { return Ok(Err(e)); }
        let mode = if chain.match_mode == MatchMode::All { "all" } else { "any" };
        let n = self.tx.execute(
            "UPDATE rule SET conditions_json = ?4, match_mode = ?5, updated_at_ms = ?6 WHERE action_label_id = ?1 AND placement = ?2 AND chain_id = ?3",
            params![label_id, placement_word(placement), chain_id, encode_conditions(&chain.conditions), mode, now_ms],
        )?;
        Ok(if n == 0 { Err(not_found("chain", format!("{label_id}/{}/{chain_id}", placement_word(placement)))) } else { Ok(()) })
    }

    /// Removes one chain. Its number is not handed out again while a higher one lives (`next_chain_id`).
    pub fn label_chain_remove(&mut self, label_id: &str, placement: ChainPlacement, chain_id: u32) -> Refusable<()> {
        let n = self.tx.execute("DELETE FROM rule WHERE action_label_id = ?1 AND placement = ?2 AND chain_id = ?3", params![label_id, placement_word(placement), chain_id])?;
        Ok(if n == 0 { Err(not_found("chain", format!("{label_id}/{}/{chain_id}", placement_word(placement)))) } else { Ok(()) })
    }

    /// `label_file_entry` plus the card that placed the message and its chain (`label_assignment.flow_card / flow_chain_id`), the two
    /// facts the counts and "why here" read. A label the owner removed from the thread is still never filed back.
    #[allow(clippy::too_many_arguments)]
    pub fn label_file_flow(&mut self, label_id: &str, message_id: &str, by: &DecidedBy, entry: Option<&str>, probability: Option<f64>, card: WhyCard, chain_id: Option<u32>, now_ms: i64) -> Result<(), StoreError> {
        self.label_file_entry(label_id, message_id, by, entry, probability, now_ms)?;
        let chain = match card { WhyCard::Entry | WhyCard::Exit => chain_id, _ => None };
        self.tx.execute("UPDATE label_assignment SET flow_card = ?3, flow_chain_id = ?4 WHERE label_id = ?1 AND message_id = ?2", params![label_id, message_id, card_word(card), chain])?;
        Ok(())
    }

    /// The exit filter sent the AI's pick elsewhere: one row per message, the newest decision replaces the older.
    pub fn flow_rejection_put(&mut self, r: &FlowRejection) -> Result<(), StoreError> {
        self.tx.execute(
            "INSERT OR REPLACE INTO flow_rejection (message_id, ai_label_id, sent_to, chain_id, decided_at_ms) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![r.message_id, r.ai_label_id, r.sent_to, r.chain_id, r.decided_at_ms],
        )?;
        Ok(())
    }

    /// The AI's pick passed (or the message was re-sorted elsewhere): its rejection row goes.
    pub fn flow_rejection_clear(&mut self, message_id: &str) -> Result<bool, StoreError> {
        Ok(self.tx.execute("DELETE FROM flow_rejection WHERE message_id = ?1", params![message_id])? == 1)
    }

    /// A140 part 3: the label AI ruled NO on `message_id` for `label_id` (step 1 had picked it). One row per pair, the newest replaces the
    /// older; `by` is the model as the filing rows name it (Laya or Jev); any other decider is refused in words (only a model says no).
    pub fn ai_no_put(&mut self, message_id: &str, label_id: &str, by: &DecidedBy, now_ms: i64) -> Result<(), StoreError> {
        self.ai_no_put_as(message_id, label_id, by, None, now_ms)
    }

    /// `ai_no_put` with the model's name set by the caller (round 11: `SET_ASIDE_MODEL` for a mail the judge refused for itself).
    pub fn ai_no_put_as(&mut self, message_id: &str, label_id: &str, by: &DecidedBy, model_override: Option<&str>, now_ms: i64) -> Result<(), StoreError> {
        // fix-aisort round 8, migration 0031: Apple's model is stored as the on-device decider 'laya' with decided_model 'apple'
        let (word, model) = match by {
            DecidedBy::Laya => ("laya", None),
            DecidedBy::Apple => ("laya", Some("apple")),
            // lane decided-model: a catalogue model other than Laya, by its id, stored as the on-device decider like Apple's
            DecidedBy::Model { model } => ("laya", Some(model.as_str())),
            DecidedBy::Jev => ("jev", None),
            _ => return Err(StoreError::Invalid("only the label AI (Laya, Jev or Apple Intelligence) can rule a mail out of a label".into())),
        };
        let model = model_override.or(model);
        // fix-aisort round 7: a mail (or a label) removed between the judgement and this write (the server said "no longer exist") is
        // skipped: its pair writes nothing instead of failing the foreign key and with it the whole batch's write
        self.tx.execute(
            "INSERT OR REPLACE INTO ai_label_no (message_id, label_id, decided_by, decided_at_ms, decided_model) \
             SELECT ?1, ?2, ?3, ?4, ?5 WHERE EXISTS (SELECT 1 FROM message WHERE id = ?1) AND EXISTS (SELECT 1 FROM label WHERE id = ?2)",
            params![message_id, label_id, word, now_ms, model],
        )?;
        Ok(())
    }

    /// A140 part 3: every kept "no" of a label goes (its Re-run, a changed description or AI-reads ticks). Returns how many went.
    pub fn ai_no_clear_label(&mut self, label_id: &str) -> Result<usize, StoreError> {
        Ok(self.tx.execute("DELETE FROM ai_label_no WHERE label_id = ?1 AND decided_model IS NOT ?2", params![label_id, WAITING_MODEL])?)
    }
}

impl Store {
    /// A140 part 3: keeps the label AI's "no" for each (message, label) pair, decided by `by`, in one transaction.
    pub fn ai_no_put(&self, pairs: Vec<(String, String)>, by: DecidedBy, now_ms: i64) -> Result<(), StoreError> {
        if pairs.is_empty() {
            return Ok(());
        }
        self.write(move |ctx| {
            for (message_id, label_id) in &pairs {
                ctx.ai_no_put(message_id, label_id, &by, now_ms)?;
            }
            Ok(())
        })
    }

    /// Lane fix-aisort round 11: sets aside each (message, label) pair the judge refused for the mail itself: a kept row like a "no"
    /// (`decided_model` `SET_ASIDE_MODEL`), so the mail is not waiting and not asked again until a marker or the label's Re-run clears it.
    pub fn ai_set_aside_put(&self, pairs: Vec<(String, String)>, by: DecidedBy, now_ms: i64) -> Result<(), StoreError> {
        if pairs.is_empty() {
            return Ok(());
        }
        self.write(move |ctx| {
            for (message_id, label_id) in &pairs {
                ctx.ai_no_put_as(message_id, label_id, &by, Some(SET_ASIDE_MODEL), now_ms)?;
            }
            Ok(())
        })
    }

    /// Lane fix-aisort round 17: each (message, label) pair the AI could not judge waits for it (`WAITING_MODEL`). A pair that already holds
    /// a kept answer (a "no", a set-aside) keeps it; a mail or label removed meanwhile writes nothing.
    pub fn ai_waiting_put(&self, pairs: Vec<(String, String)>, now_ms: i64) -> Result<(), StoreError> {
        if pairs.is_empty() {
            return Ok(());
        }
        self.write(move |ctx| {
            for (message_id, label_id) in &pairs {
                ctx.tx.execute(
                    "INSERT OR IGNORE INTO ai_label_no (message_id, label_id, decided_by, decided_at_ms, decided_model) \
                     SELECT ?1, ?2, 'laya', ?3, ?4 WHERE EXISTS (SELECT 1 FROM message WHERE id = ?1) AND EXISTS (SELECT 1 FROM label WHERE id = ?2)",
                    params![message_id, label_id, now_ms, WAITING_MODEL],
                )?;
            }
            Ok(())
        })
    }

    /// Lane fix-aisort round 17: the AI judged these pairs: their waiting rows go (a kept answer written for the pair is left as it is).
    pub fn ai_waiting_clear(&self, pairs: Vec<(String, String)>) -> Result<(), StoreError> {
        if pairs.is_empty() {
            return Ok(());
        }
        self.write(move |ctx| {
            for (message_id, label_id) in &pairs {
                ctx.tx.execute("DELETE FROM ai_label_no WHERE message_id = ?1 AND label_id = ?2 AND decided_model = ?3", params![message_id, label_id, WAITING_MODEL])?;
            }
            Ok(())
        })
    }

    /// Lane fix-aisort round 17: the mails that wait for the label's AI because it could not judge them (sorted).
    pub fn ai_waiting_messages(&self, label_id: &str) -> Result<Vec<String>, StoreError> {
        let label_id = label_id.to_string();
        self.read(move |c| {
            let mut st = c.prepare_cached("SELECT message_id FROM ai_label_no WHERE label_id = ?1 AND decided_model = ?2 ORDER BY message_id")?;
            let rows = st.query_map(params![label_id, WAITING_MODEL], |r| r.get::<_, String>(0))?;
            let out = rows.collect::<Result<Vec<_>, _>>()?;
            Ok(out)
        })
    }

    /// Lane fix-aisort round 11: how many mails of the label are set aside (state `label/<id>/flow/ai_set_aside`).
    pub fn ai_set_aside_count(&self, label_id: &str) -> Result<usize, StoreError> {
        let label_id = label_id.to_string();
        self.read(move |c| Ok(c.query_row("SELECT count(*) FROM ai_label_no WHERE label_id = ?1 AND decided_model = ?2", params![label_id, SET_ASIDE_MODEL], |r| r.get::<_, i64>(0))? as usize))
    }

    /// A140 part 3: forgets every kept "no" of a label (the owner pressed Re-run: judge again).
    pub fn ai_no_clear_label(&self, label_id: &str) -> Result<usize, StoreError> {
        let label_id = label_id.to_string();
        self.write(move |ctx| ctx.ai_no_clear_label(&label_id))
    }

    /// Lane fix-aisort: forgets, ONCE per data folder, every kept "no" that `decided_by` wrote before the fix that stopped an unreadable answer
    /// from being kept as a "no" (nothing in a row tells a real "no" from an unreadable one, so all of that decider's go). `marker` is the file
    /// that records it is done; Ok(None) when it was done before, Ok(Some(n)) with the rows removed. The marker is written after the delete,
    /// so a failure is tried again at the next start.
    pub fn ai_no_clear_once(&self, decided_by: &str, marker: &std::path::Path) -> Result<Option<usize>, StoreError> {
        if marker.exists() {
            return Ok(None);
        }
        let by = decided_by.to_string();
        let n = self.write(move |ctx| Ok(ctx.tx.execute("DELETE FROM ai_label_no WHERE decided_by = ?1 AND decided_model IS NOT ?2", params![by, WAITING_MODEL])?))?;
        std::fs::write(marker, format!("{n}\n")).map_err(|e| StoreError::Invalid(format!("the mark of the one-time clean-up could not be written: {e}")))?;
        Ok(Some(n))
    }

    /// Lane fix-aisort round 4: a changed QUESTION clears the kept "no", as a changed description does (this file, the `ai_no_clear_label` call of the description update). The
    /// question is its form and its trust level, written by the caller as one line (`<form>|<trust>`) and kept in `marker` beside the database.
    /// When the marker is missing or holds another line, every `ai_label_no` row a model decided (laya, jev: the only deciders the table allows)
    /// goes, then the marker is written (so a failure is tried again at the next start). Ok(None) when the question is the same; Ok(Some((the
    /// line before, if any, rows removed))) otherwise. Filings (`label_assignment`), the owner's included, are never touched.
    pub fn ai_no_clear_on_question(&self, question: &str, marker: &std::path::Path) -> Result<Option<(Option<String>, usize)>, StoreError> {
        let before = std::fs::read_to_string(marker).ok().map(|s| s.trim().to_string());
        if before.as_deref() == Some(question) {
            return Ok(None);
        }
        let n = self.write(move |ctx| Ok(ctx.tx.execute("DELETE FROM ai_label_no WHERE decided_by IN ('laya', 'jev') AND decided_model IS NOT ?1", params![WAITING_MODEL])?))?;
        std::fs::write(marker, format!("{question}\n")).map_err(|e| StoreError::Invalid(format!("the mark of the label AI's question could not be written: {e}")))?;
        Ok(Some((before, n)))
    }

    /// Lane fix-aisort round 9: every model's kept "no" goes (`decided_by` 'laya' or 'jev'; Apple's are stored as 'laya', migration 0031).
    /// Returns how many went.
    pub fn ai_no_clear_models(&self) -> Result<usize, StoreError> {
        self.write(|ctx| Ok(ctx.tx.execute("DELETE FROM ai_label_no WHERE decided_by IN ('laya', 'jev') AND decided_model IS NOT ?1", params![WAITING_MODEL])?))
    }

    /// A140 part 3: the labels the AI ruled NO on for this message (sorted).
    pub fn ai_no_labels(&self, message_id: &str) -> Result<Vec<String>, StoreError> {
        let message_id = message_id.to_string();
        self.read(move |c| {
            let mut st = c.prepare_cached("SELECT label_id FROM ai_label_no WHERE message_id = ?1 AND decided_model IS NOT ?2 ORDER BY label_id")?;
            let rows = st.query_map(params![message_id, WAITING_MODEL], |r| r.get::<_, String>(0))?;
            let out = rows.collect::<Result<Vec<_>, _>>()?;
            Ok(out)
        })
    }

    /// A140 part 3: the messages the AI ruled NO on for this label (the counts' `waiting_for_ai` leaves them out).
    pub fn ai_no_messages(&self, label_id: &str) -> Result<Vec<String>, StoreError> {
        let label_id = label_id.to_string();
        self.read(move |c| {
            let mut st = c.prepare_cached("SELECT message_id FROM ai_label_no WHERE label_id = ?1 AND decided_model IS NOT ?2 ORDER BY message_id")?;
            let rows = st.query_map(params![label_id, WAITING_MODEL], |r| r.get::<_, String>(0))?;
            let out = rows.collect::<Result<Vec<_>, _>>()?;
            Ok(out)
        })
    }
}

fn flatten<T>(r: Refusable<T>) -> Result<T, CoreError> { r.map_err(CoreError::from)? }

impl Store {
    pub fn label_set_card(&self, label_id: String, card: FlowCard, enabled: bool, now_ms: i64) -> Result<(), CoreError> { flatten(self.write(move |ctx| ctx.label_set_card(&label_id, card, enabled, now_ms))) }
    pub fn label_set_ai_reads(&self, label_id: String, reads: AiReads, now_ms: i64) -> Result<(), CoreError> { flatten(self.write(move |ctx| ctx.label_set_ai_reads(&label_id, reads, now_ms))) }
    pub fn label_set_exit_fallback(&self, label_id: String, fallback: Option<String>, now_ms: i64) -> Result<(), CoreError> { flatten(self.write(move |ctx| ctx.label_set_exit_fallback(&label_id, fallback, now_ms))) }
    pub fn label_chain_add(&self, label_id: String, placement: ChainPlacement, chain: RuleSet, now_ms: i64) -> Result<u32, CoreError> { flatten(self.write(move |ctx| ctx.label_chain_add(&label_id, placement, chain, now_ms))) }
    pub fn label_chain_update(&self, label_id: String, placement: ChainPlacement, chain_id: u32, chain: RuleSet, now_ms: i64) -> Result<(), CoreError> { flatten(self.write(move |ctx| ctx.label_chain_update(&label_id, placement, chain_id, chain, now_ms))) }
    pub fn label_chain_remove(&self, label_id: String, placement: ChainPlacement, chain_id: u32) -> Result<(), CoreError> { flatten(self.write(move |ctx| ctx.label_chain_remove(&label_id, placement, chain_id))) }

    /// The chains of one card of a label, by number.
    pub fn label_chains(&self, label_id: &str, placement: ChainPlacement) -> Result<Vec<LabelChain>, StoreError> { self.read(|c| read_chains(c, label_id, placement)) }
    /// Every label rule, both cards: the decision lane's input for a label flow (with `labels_full`, which is in sidebar order).
    pub fn label_rules_all(&self) -> Result<Vec<Rule>, StoreError> { self.read(label_rules_all) }
    /// The filings the four figures of every label's foot are counted from (`contracts::count_label_flow`).
    pub fn label_flow_filings(&self) -> Result<Vec<FlowFiling>, StoreError> { self.read(flow_filings) }
    /// The message's rejection row, if the exit filter sent the AI's pick elsewhere.
    pub fn flow_rejection(&self, message_id: &str) -> Result<Option<FlowRejection>, StoreError> { self.read(|c| read_rejection(c, message_id)) }
    /// The facts a label rule reads of one stored message (columns already synced).
    pub fn rule_facts(&self, message_id: &str) -> Result<Option<MessageFacts>, StoreError> { self.read(|c| rule_facts_of(c, message_id)) }

    /// "Why here?" for one message in one place: `place` is a label id, "primary" or "junk". In a label: the card and chain of its
    /// filing and the conditions of that chain that hold now. In Primary or Junk: the exit filter that sent it there (card Fallback),
    /// or None when no rejection names that place. The rejection row keeps no provider, so a Primary/Junk "why" says `Default` for
    /// who decided (named as a gap in docs/lanes/decision-r2.md).
    pub fn label_why(&self, message_id: &str, place: &str) -> Result<Option<WhyHere>, StoreError> {
        self.read(|c| {
            let Some(facts) = rule_facts_of(c, message_id)? else { return Ok(None) };
            if place == "primary" || place == "junk" {
                let Some(r) = read_rejection(c, message_id)?.filter(|r| r.sent_to == place) else { return Ok(None) };
                return Ok(Some(WhyHere { card: WhyCard::Fallback, chain_id: r.chain_id, matched: vec![], decided_by: DecidedBy::Default }));
            }
            let row: Option<(String, Option<String>, Option<String>, Option<i64>, Option<String>)> = c.query_row(
                "SELECT decided_by, rule_id, flow_card, flow_chain_id, decided_model FROM label_assignment WHERE label_id = ?1 AND message_id = ?2",
                params![place, message_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
            ).optional()?;
            let Some((word, rule, card, chain, model)) = row else { return Ok(None) };
            let card = card_from(card.as_deref());
            let placement = match card { WhyCard::Entry => Some(ChainPlacement::Entry), WhyCard::Exit => Some(ChainPlacement::Exit), _ => None };
            let chain = match (placement, chain) {
                (Some(p), Some(n)) => read_chains(c, place, p)?.into_iter().find(|ch| i64::from(ch.chain_id) == n),
                _ => None,
            };
            Ok(Some(contracts::why_here(card, chain, facts, decided_by_from(&word, rule, model.as_deref()))))
        })
    }
}

// ---------------------------------------------------------------- the decision model row (labels-r5, round 5)
// `decision_model` (migration 0001, one row, id 1) was never written: `SaveDecisionModel` was not served and `decision_model()` answered None.

impl Store {
    /// Keeps the decision model the owner chose: its provider, the model id, the Keychain reference of the Jev key (the key itself is
    /// never here), the decisions it takes and its question words. Replaces the earlier row.
    pub fn decision_model_put(&self, m: contracts::DecisionModel, now_ms: i64) -> Result<(), StoreError> {
        // Apple's on-device model is never a saved choice (the pane offers Local, Jev and Rules; the table's CHECK holds those three): it is
        // reached through Local, so a stray Apple is kept as "local"
        let provider = match m.provider {
            contracts::DecisionProviderKind::Local | contracts::DecisionProviderKind::Apple => "local",
            contracts::DecisionProviderKind::Jev => "jev",
            contracts::DecisionProviderKind::Rules => "rules",
        };
        let key = m.keychain.as_ref().map(|r| format!("{}\u{1f}{}", r.service, r.account));
        let enabled = crate::ops::json_strings(&m.decisions_enabled);
        let mut words: Vec<(&String, &String)> = m.question_words.iter().collect();
        words.sort();
        let words = format!("{{{}}}", words.iter().map(|(k, v)| format!("{}:{}", crate::ops::json_string(k), crate::ops::json_string(v))).collect::<Vec<_>>().join(","));
        self.write(move |ctx| {
            ctx.tx.execute(
                "INSERT INTO decision_model (id, provider, model, jev_api_key_ref, decisions_enabled_json, question_words_json, last_run_ms, updated_at_ms) \
                 VALUES (1, ?1, ?2, ?3, ?4, ?5, ?6, ?7) \
                 ON CONFLICT(id) DO UPDATE SET provider = excluded.provider, model = excluded.model, jev_api_key_ref = excluded.jev_api_key_ref, \
                    decisions_enabled_json = excluded.decisions_enabled_json, question_words_json = excluded.question_words_json, \
                    last_run_ms = excluded.last_run_ms, updated_at_ms = excluded.updated_at_ms",
                params![provider, m.model, key, enabled, words, m.last_run_ms, now_ms],
            )?;
            Ok(())
        })
    }

    /// The decision model the owner chose; None until `SaveDecisionModel` has run.
    pub fn decision_model_get(&self) -> Result<Option<contracts::DecisionModel>, StoreError> {
        self.read(|c| {
            let row: Option<(String, Option<String>, Option<String>, Option<i64>)> = c
                .query_row("SELECT provider, model, jev_api_key_ref, last_run_ms FROM decision_model WHERE id = 1", [], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
                .optional()?;
            let Some((provider, model, key, last_run_ms)) = row else { return Ok(None) };
            let mut s = c.prepare("SELECT value FROM decision_model, json_each(decision_model.decisions_enabled_json) WHERE decision_model.id = 1")?;
            let decisions_enabled = s.query_map([], |r| r.get::<_, String>(0))?.collect::<Result<Vec<_>, _>>()?;
            let mut s = c.prepare("SELECT key, value FROM decision_model, json_each(decision_model.question_words_json) WHERE decision_model.id = 1")?;
            let question_words = s.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?.collect::<Result<std::collections::HashMap<_, _>, _>>()?;
            Ok(Some(contracts::DecisionModel {
                provider: match provider.as_str() { "jev" => contracts::DecisionProviderKind::Jev, "rules" => contracts::DecisionProviderKind::Rules, _ => contracts::DecisionProviderKind::Local },
                model,
                keychain: key.and_then(|k| k.split_once('\u{1f}').map(|(s, a)| contracts::KeychainRef { service: s.into(), account: a.into() })),
                decisions_enabled, question_words, last_run_ms,
            }))
        })
    }
}

// ---------------------------------------------------------------- the built-in labels' content (labels-r5, round 6)
// Needs Reply, Updates, Promotions and Social were created by migration 0004 as EMPTY shells (no description, no rule): on a new installation
// nothing sorted mail ("need reply is seems like empty and not configured", the owner on main65). `seed_builtin_labels` fills each once.

/// What one built-in label starts with: the AI card's description, an entry chain that files WITHOUT any AI, an exit chain with its fallback.
#[derive(Debug, Clone, PartialEq)]
pub struct BuiltinSeed {
    pub id: &'static str,
    pub description: String,
    pub entry: Option<RuleSet>,
    pub exit: Option<RuleSet>,
}

const UPDATES_DESCRIPTION: &str = "Automatic notices about my accounts and orders: receipts, shipping and delivery news, confirmations, alerts, statements.";
const PROMOTIONS_DESCRIPTION: &str = "Marketing and offers: newsletters, sales, discounts and promotional campaigns.";
const SOCIAL_DESCRIPTION: &str = "Messages from social networks and communities: invitations, mentions, new followers and activity of my contacts.";

fn fact_chain(fact: contracts::MessageFact) -> RuleSet {
    RuleSet { conditions: vec![RuleCondition::Fact { fact, wanted: true }], match_mode: MatchMode::All }
}

/// The content the built-in labels start with, in English (the core holds no catalogue: the descriptions are the owner's words from the day they are
/// seeded, and the model reads them as written). Needs Reply is the approved example (docs/design/boards/labels-flow-r1, the fixture core): the AI
/// reads « Emails that wait for a reply from me... », the exit filter keeps what is still unanswered (Sans réponse: yes) and the others go to Primary.
/// Promotions files by a deterministic rule, a mailing list (List-Id or List-Unsubscribe), so it fills with no AI; Updates and Social are the AI's.
pub fn builtin_seeds() -> Vec<BuiltinSeed> {
    vec![
        BuiltinSeed { id: "needs-reply", description: contracts::default_reply_ai_description(), entry: None, exit: Some(fact_chain(contracts::MessageFact::Unanswered)) },
        BuiltinSeed { id: "updates", description: UPDATES_DESCRIPTION.into(), entry: None, exit: None },
        BuiltinSeed { id: "promotions", description: PROMOTIONS_DESCRIPTION.into(), entry: Some(fact_chain(contracts::MessageFact::MailingList)), exit: None },
        BuiltinSeed { id: "social", description: SOCIAL_DESCRIPTION.into(), entry: None, exit: None },
    ]
}

impl WriteCtx<'_> {
    /// Fills one built-in label when it is still an empty shell: a built-in kind, no description, no chain on either card, the exit filter off with no
    /// fallback. Anything the owner wrote (a description, a rule, a card setting) makes it untouched here.
    fn seed_builtin(&mut self, seed: &BuiltinSeed, now_ms: i64) -> Result<bool, StoreError> {
        let Some(label) = self.get_full(seed.id)? else { return Ok(false) };
        let shell = label.kind == LabelKind::BuiltIn
            && label.description.trim().is_empty()
            && !label.exit_enabled
            && label.exit_fallback.is_none()
            && read_chains(self.tx, seed.id, ChainPlacement::Entry)?.is_empty()
            && read_chains(self.tx, seed.id, ChainPlacement::Exit)?.is_empty();
        if !shell {
            return Ok(false);
        }
        self.tx.execute("UPDATE label SET description = ?2, updated_at_ms = ?3 WHERE id = ?1", params![seed.id, seed.description, now_ms])?;
        let inner = |r: Result<u32, CoreError>| r.map(|_| ()).map_err(|e| StoreError::Invalid(e.to_string()));
        if let Some(chain) = &seed.entry {
            inner(self.label_chain_add(seed.id, ChainPlacement::Entry, chain.clone(), now_ms)?)?;
            // A140 (lead's default, 2026-11-11): a built-in label that ships with an entry chain ships with its AI card off, so its step 1
            // alone files (no model needed, as before A140); its description stays. Only this seed changes: no stored label is migrated.
            self.tx.execute("UPDATE label SET ai_enabled = 0, updated_at_ms = ?2 WHERE id = ?1", params![seed.id, now_ms])?;
        }
        if let Some(chain) = &seed.exit {
            inner(self.label_chain_add(seed.id, ChainPlacement::Exit, chain.clone(), now_ms)?)?;
            self.tx.execute("UPDATE label SET exit_enabled = 1, exit_fallback = 'primary', updated_at_ms = ?2 WHERE id = ?1", params![seed.id, now_ms])?;
        }
        Ok(true)
    }
}

impl Store {
    /// Seeds the built-in labels that are still empty shells (see `builtin_seeds`); returns the ids it filled. Idempotent: a label that has a
    /// description, a chain or an exit filter is never touched, so what the owner wrote always wins. If the owner empties a built-in label
    /// completely (no description, no rule), the next launch fills it again.
    pub fn seed_builtin_labels(&self, now_ms: i64) -> Result<Vec<String>, StoreError> {
        self.write(move |ctx| {
            let mut filled = Vec::new();
            for seed in builtin_seeds() {
                if ctx.seed_builtin(&seed, now_ms)? {
                    filled.push(seed.id.to_string());
                }
            }
            ctx.backfill_builtin_tabs()?;   // round 13: the filings an earlier build made under a built-in label reach its tab
            Ok(filled)
        })
    }
}

// ---------------------------------------------------------------- the stored consent for a hosted decision model (labels-r5, round 7; migration 0017)

impl Store {
    /// Keeps the owner's yes (`given`) or no to sending the label AI's mail text to the hosted decision provider stored as `key` ("jev"). No row = not
    /// given; withdrawing deletes the row, so the next call is refused.
    pub fn decision_consent_set(&self, key: &str, given: bool, now_ms: i64) -> Result<(), StoreError> {
        let key = key.to_string();
        self.write(move |ctx| {
            if given {
                ctx.tx.execute("INSERT INTO decision_consent (provider, given_at_ms) VALUES (?1, ?2) ON CONFLICT(provider) DO UPDATE SET given_at_ms = excluded.given_at_ms", params![key, now_ms])?;
            } else {
                ctx.tx.execute("DELETE FROM decision_consent WHERE provider = ?1", params![key])?;
            }
            Ok(())
        })
    }

    /// Whether the owner's consent is stored for this provider key.
    pub fn decision_consent_given(&self, key: &str) -> Result<bool, StoreError> {
        let key = key.to_string();
        self.read(move |c| Ok(c.query_row("SELECT EXISTS (SELECT 1 FROM decision_consent WHERE provider = ?1)", params![key], |r| r.get::<_, i64>(0))? == 1))
    }
}
