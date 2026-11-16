//! Reads from, and writes to, the store's tables. Every function takes a `Connection`, so it runs on the read-only
//! connection (arrival, counts) or inside the store's write transaction (edits, re-sort).
use crate::facts::{HeaderFacts, MessageFacts, SenderHistory};
use crate::place::Placement;
use crate::tabs::TabSet;
use contracts::{DecidedBy, Rule, RuleCondition, SplitTab, SplitTabRule, VipList};
use rusqlite::{params, Connection, OpenFlags, OptionalExtension};
use std::collections::HashSet;
use std::path::Path;
use store::{StoreError, WriteCtx};

/// A read-only connection to the store's file (WAL: it never waits for the writer).
pub fn open_reader(path: &Path) -> Result<Connection, rusqlite::Error> {
    Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX)
}

pub struct Context {
    pub tabs: TabSet,
    pub rules: Vec<Rule>,
    pub vips: VipList,
    /// The owner's own addresses, lower case.
    pub own: HashSet<String>,
}

pub fn tab_rule_from_columns(kind: &str, value: &str) -> SplitTabRule {
    let v = value.to_string();
    match kind {
        "sender" => SplitTabRule::Sender { address: v },
        "domain" => SplitTabRule::Domain { domain: v },
        "account" => SplitTabRule::Account { account_id: v },
        "label" => SplitTabRule::Label { label_id: v },
        "decision_category" => SplitTabRule::DecisionCategory { category: v },
        _ => SplitTabRule::Builtin { name: v },
    }
}

pub fn tab_rule_columns(r: &SplitTabRule) -> (&'static str, String) {
    match r {
        SplitTabRule::Sender { address } => ("sender", address.clone()),
        SplitTabRule::Domain { domain } => ("domain", domain.clone()),
        SplitTabRule::Account { account_id } => ("account", account_id.clone()),
        SplitTabRule::Label { label_id } => ("label", label_id.clone()),
        SplitTabRule::DecisionCategory { category } => ("decision_category", category.clone()),
        SplitTabRule::Builtin { name } => ("builtin", name.clone()),
    }
}

pub fn load_tabs(conn: &Connection) -> rusqlite::Result<TabSet> {
    let mut st = conn.prepare_cached("SELECT id, name, sort_order, rule_kind, rule_value, enabled FROM split_tab")?;
    let rows = st.query_map([], |r| {
        Ok(SplitTab { id: r.get(0)?, name: r.get(1)?, order: r.get::<_, i64>(2)? as u32, rule: tab_rule_from_columns(&r.get::<_, String>(3)?, &r.get::<_, String>(4)?), enabled: r.get::<_, i64>(5)? != 0 })
    })?;
    Ok(TabSet::new(rows.collect::<Result<Vec<_>, _>>()?))
}

/// The shape the `rule` table's conditions_json takes (the rule's writer, Settings › Rules, must write exactly this):
/// `[{"kind":"from","words":".."},{"kind":"domain","domain":".."},{"kind":"to"|"cc"|"subject"|"body"|"list_header","words":".."},
///   {"kind":"account","account_id":".."},{"kind":"has_attachment"},{"kind":"fact","fact":"<message_fact_key>","wanted":true|false}]`
/// (the fact kind is delta 17's; store::encode_conditions writes it). A fact with an unknown key or no yes/no is dropped, as an unknown
/// kind always was: this reader feeds tab placement, which must never stop on one bad row.
pub fn conditions_from_json(json: &str) -> Vec<RuleCondition> {
    let Ok(serde_json::Value::Array(items)) = serde_json::from_str::<serde_json::Value>(json) else { return vec![] };
    let s = |v: &serde_json::Value, k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
    items
        .iter()
        .filter_map(|v| {
            Some(match v.get("kind")?.as_str()? {
                "from" => RuleCondition::From { words: s(v, "words") },
                "to" => RuleCondition::To { words: s(v, "words") },
                "cc" => RuleCondition::Cc { words: s(v, "words") },
                "domain" => RuleCondition::Domain { domain: s(v, "domain") },
                "subject" => RuleCondition::SubjectContains { words: s(v, "words") },
                "body" => RuleCondition::BodyContains { words: s(v, "words") },
                "list_header" => RuleCondition::ListHeader { words: s(v, "words") },
                "account" => RuleCondition::Account { account_id: s(v, "account_id") },
                "has_attachment" => RuleCondition::HasAttachment,
                "not" => RuleCondition::Not { field: contracts::RuleField::from_key(&s(v, "field"))?, value: s(v, "value") },
                "fact" => RuleCondition::Fact { fact: contracts::message_fact_from_key(s(v, "fact"))?, wanted: v.get("wanted")?.as_bool()? },
                _ => return None,
            })
        })
        .collect()
}

pub fn load_rules(conn: &Connection) -> rusqlite::Result<Vec<Rule>> {
    // delta 9: only the rules that file into a tab (action_kind 'tab') are loaded here; rules that file into a label are the decision lane's to read.
    // delta 17: a tab rule has one card and one chain: placement Entry, chain 0 (migration 0009 refuses an exit chain on a tab rule).
    let mut st = conn.prepare_cached("SELECT id, position, conditions_json, tab_id, enabled, match_mode FROM rule WHERE action_kind = 'tab' ORDER BY position, id")?;
    let rows = st.query_map([], |r| {
        Ok(Rule { id: r.get(0)?, position: r.get::<_, i64>(1)? as u32, conditions: conditions_from_json(&r.get::<_, String>(2)?), tab_id: r.get(3)?, enabled: r.get::<_, i64>(4)? != 0, action: contracts::RuleAction::Tab, match_mode: if r.get::<_, String>(5)? == "all" { contracts::MatchMode::All } else { contracts::MatchMode::Any }, placement: contracts::ChainPlacement::Entry, chain_id: 0 })
    })?;
    rows.collect()
}

pub fn load_vips(conn: &Connection) -> rusqlite::Result<VipList> {
    let mut vips = VipList { addresses: vec![], domains: vec![] };
    let mut st = conn.prepare_cached("SELECT kind, value FROM vip")?;
    for row in st.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))? {
        let (kind, value) = row?;
        if kind == "domain" { vips.domains.push(value) } else { vips.addresses.push(value) }
    }
    Ok(vips)
}

pub fn load_context(conn: &Connection) -> rusqlite::Result<Context> {
    let mut st = conn.prepare_cached("SELECT address FROM account")?;
    let own = st.query_map([], |r| r.get::<_, String>(0))?.map(|a| a.map(|s| s.to_ascii_lowercase())).collect::<Result<HashSet<_>, _>>()?;
    Ok(Context { tabs: load_tabs(conn)?, rules: load_rules(conn)?, vips: load_vips(conn)?, own })
}

pub fn is_vip(vips: &VipList, address: &str) -> bool {
    let a = address.trim().to_ascii_lowercase();
    let domain = a.rsplit_once('@').map(|(_, d)| d.to_string()).unwrap_or_default();
    vips.addresses.iter().any(|v| v.trim().eq_ignore_ascii_case(&a)) || vips.domains.iter().any(|d| crate::cond::domain_matches(&domain, d))
}

/// What the mailbox says about `address` as of `before_ms`.
pub fn sender_history(conn: &Connection, address: &str, before_ms: i64) -> rusqlite::Result<SenderHistory> {
    let received: i64 = conn.prepare_cached("SELECT count(*) FROM message WHERE lower(from_address) = lower(?1) AND received_at_ms < ?2")?.query_row(params![address, before_ms], |r| r.get(0))?;
    let wrote: i64 = conn
        .prepare_cached(
            "SELECT EXISTS (SELECT 1 FROM message s, json_each(s.to_json) j \
             WHERE s.from_address IN (SELECT address FROM account) AND lower(j.value) = lower(?1) AND s.received_at_ms < ?2)",
        )?
        .query_row(params![address, before_ms], |r| r.get(0))?;
    Ok(SenderHistory { received_before: received.max(0) as u32, owner_wrote_to: wrote != 0 })
}

fn strings(json: &str) -> Vec<String> {
    serde_json::from_str::<Vec<String>>(json).unwrap_or_default()
}

/// Facts for messages already in the store (a re-sort). The List-Id value and X-GM-LABELS are not stored; the store keeps
/// the one list-header bit and the labels the sync assigned.
pub fn facts_for(conn: &Connection, ctx: &Context, ids: &[String]) -> rusqlite::Result<Vec<MessageFacts>> {
    let mut out = Vec::with_capacity(ids.len());
    let mut msg = conn.prepare_cached(
        "SELECT id, thread_id, account_id, from_address, to_json, cc_json, subject, COALESCE(body_text, preview), has_list_header, received_at_ms, \
                EXISTS (SELECT 1 FROM attachment a WHERE a.message_id = message.id), sender_name, read FROM message WHERE id = ?1",
    )?;
    let parents = label_parents(conn)?;
    let mut labels = conn.prepare_cached("SELECT l.id, l.name FROM label_assignment la JOIN label l ON l.id = la.label_id WHERE la.message_id = ?1")?;
    for id in ids {
        let row = msg.query_row([id], |r| {
            Ok(MessageFacts {
                message_id: r.get(0)?,
                thread_id: r.get(1)?,
                account_id: r.get(2)?,
                from_address: r.get(3)?,
                to: strings(&r.get::<_, String>(4)?),
                cc: strings(&r.get::<_, String>(5)?),
                subject: r.get(6)?,
                body: r.get(7)?,
                has_list_header: r.get::<_, i64>(8)? != 0,
                received_at_ms: r.get(9)?,
                has_attachment: r.get::<_, i64>(10)? != 0,
                from_name: r.get::<_, Option<String>>(11)?.unwrap_or_default(),
                unread: r.get::<_, i64>(12)? == 0,
                ..Default::default()
            })
        });
        let mut f = match row {
            Ok(f) => f,
            Err(rusqlite::Error::QueryReturnedNoRows) => continue,
            Err(e) => return Err(e),
        };
        f.has_list_unsubscribe = f.has_list_header;
        for l in labels.query_map([id], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))? {
            let (lid, name) = l?;
            f.label_chains.push(label_chain(&parents, &lid));
            f.label_ids.push(lid);
            f.label_names.push(name);
        }
        complete(conn, ctx, &mut f)?;
        out.push(f);
    }
    Ok(out)
}

/// label id -> parent label id (a root has none).
pub fn label_parents(conn: &Connection) -> rusqlite::Result<std::collections::HashMap<String, Option<String>>> {
    let mut st = conn.prepare_cached("SELECT id, parent_id FROM label")?;
    let rows = st.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, Option<String>>(1)?)))?;
    rows.collect()
}

/// The ids from the root down to `id` (client, Provanta, Orbit). A cycle in parent_id ends the walk instead of looping.
pub fn label_chain(parents: &std::collections::HashMap<String, Option<String>>, id: &str) -> Vec<String> {
    let mut chain = vec![id.to_string()];
    let mut seen: HashSet<String> = chain.iter().cloned().collect();
    let mut cur = id.to_string();
    while let Some(Some(p)) = parents.get(&cur) {
        if !seen.insert(p.clone()) {
            break;
        }
        chain.push(p.clone());
        cur = p.clone();
    }
    chain.reverse();
    chain
}

/// The facts that come from the mailbox and the settings rather than the message: owner, VIP, sender history, and delta 17's four
/// (starred, unanswered, cc me, replied by me), all from columns already synced. The thread questions are the store's own SQL
/// (`store::Store::LATER_IN_THREAD_SQL`, `OWNER_WROTE_EARLIER_SQL`), so the store's "why here" reads them the same way. At arrival the
/// message is not stored yet: its thread's stored messages answer, and starred reads false until the row exists.
pub fn complete(conn: &Connection, ctx: &Context, f: &mut MessageFacts) -> rusqlite::Result<()> {
    f.from_owner = ctx.own.contains(&f.from_address.to_ascii_lowercase());
    f.from_vip = is_vip(&ctx.vips, &f.from_address);
    f.history = sender_history(conn, &f.from_address, f.received_at_ms)?;
    let mine = |a: &String| ctx.own.contains(&a.trim().to_ascii_lowercase());
    f.cc_me = f.cc.iter().any(mine) && !f.to.iter().any(mine);
    let later: bool = conn.prepare_cached(store::Store::LATER_IN_THREAD_SQL)?.query_row(params![f.thread_id, f.received_at_ms, f.message_id], |r| r.get(0))?;
    f.unanswered = !f.from_owner && !later;
    f.replied_by_me = conn.prepare_cached(store::Store::OWNER_WROTE_EARLIER_SQL)?.query_row(params![f.thread_id, f.received_at_ms, f.message_id], |r| r.get(0))?;
    f.starred = conn.prepare_cached("SELECT starred FROM message WHERE id = ?1")?.query_row([&f.message_id], |r| r.get::<_, i64>(0)).optional()?.is_some_and(|v| v == 1);
    Ok(())
}

pub fn apply_headers(f: &mut MessageFacts, h: &HeaderFacts) {
    f.list_id = h.list_id.clone();
    f.has_list_unsubscribe = h.has_list_unsubscribe;
    f.has_list_header = f.has_list_header || h.has_list_header;
}

fn decided_by_columns(d: &DecidedBy) -> (&'static str, Option<String>) {
    match d {
        DecidedBy::Rule { rule_id } => ("rule", Some(rule_id.clone())),
        DecidedBy::Laya => ("laya", None),
        DecidedBy::Jev => ("jev", None),
        // message_category's CHECK has no 'apple' and no model column (read-only history since 0004); Apple's model judges no tab
        DecidedBy::Apple => ("laya", None),
        // lane decided-model: the same for a catalogue model other than Laya (no column here to name it)
        DecidedBy::Model { .. } => ("laya", None),
        DecidedBy::Owner => ("owner", None),
        DecidedBy::Default => ("default", None),
    }
}

/// Write placements to message_category: one row per message, decided_by on every row. A row that already says the same
/// changes nothing (no write, no event). The ids are declared first so the counts move by their difference.
pub fn write_placements(ctx: &mut WriteCtx<'_>, placements: &[Placement], now_ms: i64) -> Result<usize, StoreError> {
    let ids: Vec<String> = placements.iter().map(|p| p.message_id.clone()).collect();
    ctx.will_change(&ids)?;
    let mut st = ctx.tx.prepare_cached(
        "INSERT INTO message_category (message_id, tab_id, decided_by, rule_id, probability, decided_at_ms, updated_at_ms) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6) \
         ON CONFLICT(message_id) DO UPDATE SET tab_id = excluded.tab_id, decided_by = excluded.decided_by, rule_id = excluded.rule_id, \
            probability = excluded.probability, decided_at_ms = excluded.decided_at_ms, updated_at_ms = excluded.updated_at_ms \
         WHERE (message_category.decided_by <> 'owner' OR excluded.decided_by = 'owner') \
            AND (message_category.tab_id <> excluded.tab_id OR message_category.decided_by <> excluded.decided_by \
                 OR message_category.rule_id IS NOT excluded.rule_id OR message_category.probability IS NOT excluded.probability)",
    )?;
    let mut changed = 0;
    for p in placements {
        let (by, rule_id) = decided_by_columns(&p.decided_by);
        changed += st.execute(params![p.message_id, p.tab_id, by, rule_id, p.probability, now_ms])?;
    }
    Ok(changed)
}
