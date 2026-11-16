//! The live count of a rule set over the stored mail (lane labels-r5): what the Labels inspector reads as "14 messages · 3 unread".
//!
//! Before this file the real core answered `rule_set_count` with a constant zero, so a filter such as "unread AND already answered by
//! me" counted nothing on a real account whatever the mail held. The corpus is the INBOX messages that came in (what the label flow
//! files: `engine::file_arrivals`), each read with the same facts the flow judges (`MessageFacts`), and the judge is the contract's
//! own `count_rule_matches`, so the sheet's count and the filing agree on one corpus.
//!
//! One SQL read for the whole corpus, and only the columns the rule set reads (`Needs::of`): the thread facts (a later message, an earlier message
//! of mine) are correlated EXISTS over the index `message_by_thread`, computed only when a rule asks, and the body, the
//! recipients and the subject are read only when a rule reads them. 30,000 messages: `tests/perf_r5.rs` (the numbers are in
//! docs/lanes/labels-r5.md). `tests/answered_r5.rs` holds this loader equal, field by field, to `load::facts_for` (the per-message
//! reader), and the projected count equal to the count over the whole corpus, so the readers cannot drift.
use crate::facts::MessageFacts;
use crate::load::{is_vip, load_context, open_reader, Context};
use contracts::{MessageFact, RuleCondition, RuleSet, RuleSetCount};
use rusqlite::Connection;
use store::{Store, INBOX_FOLDER};

/// "A later message in this thread": the text of `Store::LATER_IN_THREAD_SQL` with the message as a column, not a parameter.
const LATER: &str = "EXISTS (SELECT 1 FROM message n WHERE n.thread_id = m.thread_id \
    AND (n.received_at_ms > m.received_at_ms OR (n.received_at_ms = m.received_at_ms AND n.id > m.id)))";
/// "I wrote earlier in this thread": `Store::OWNER_WROTE_EARLIER_SQL` with the message as a column.
const REPLIED: &str = "EXISTS (SELECT 1 FROM message n WHERE n.thread_id = m.thread_id \
    AND (n.received_at_ms < m.received_at_ms OR (n.received_at_ms = m.received_at_ms AND n.id < m.id)) \
    AND lower(n.from_address) IN (SELECT lower(address) FROM account))";
const ATTACHMENT: &str = "EXISTS (SELECT 1 FROM attachment a WHERE a.message_id = m.id)";

/// What a count reads of each message. A column nobody reads is not selected (a literal stands in), so the cost follows the rule set,
/// not the table: "unread AND answered by me" reads the unread bit and one EXISTS, never a body.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Needs {
    pub ids: bool,
    pub body: bool,
    pub subject: bool,
    pub sender: bool,
    pub recipients: bool,
    pub list: bool,
    pub attachment: bool,
    pub starred: bool,
    pub later: bool,
    pub replied: bool,
    pub account: bool,
    /// Newest first. A count does not sort; a caller that shows rows does.
    pub ordered: bool,
}

impl Needs {
    /// Every field: what `load::facts_for` gives, for the comparison and for a caller that wants whole facts.
    pub fn all() -> Needs {
        Needs { ids: true, body: true, subject: true, sender: true, recipients: true, list: true, attachment: true, starred: true, later: true, replied: true, account: true, ordered: true }
    }

    /// The fields the conditions of this rule set read. The unread bit and the date are always read (the count's unread and newest).
    pub fn of(rules: &RuleSet) -> Needs {
        let mut n = Needs::default();
        for c in &rules.conditions {
            // a negated twin reads what its positive twin reads
            let positive;
            let c = match c { RuleCondition::Not { field, value } => { positive = RuleCondition::positive_twin(*field, value); &positive } other => other };
            match c {
                RuleCondition::From { .. } | RuleCondition::Domain { .. } => n.sender = true,
                RuleCondition::To { .. } | RuleCondition::Cc { .. } => n.recipients = true,
                RuleCondition::SubjectContains { .. } => n.subject = true,
                RuleCondition::BodyContains { .. } => n.body = true,
                RuleCondition::ListHeader { .. } => n.list = true,
                RuleCondition::Account { .. } => n.account = true,
                RuleCondition::HasAttachment => n.attachment = true,
                RuleCondition::Fact { fact, .. } => match fact {
                    MessageFact::Unread => {}
                    MessageFact::Unanswered => n.later = true,
                    MessageFact::HasAttachment => n.attachment = true,
                    MessageFact::Starred => n.starred = true,
                    // the VIP list is read from the sender's address
                    MessageFact::VipSender => n.sender = true,
                    MessageFact::CcMe => n.recipients = true,
                    MessageFact::MailingList => n.list = true,
                    MessageFact::RepliedByMe => n.replied = true,
                },
                RuleCondition::Not { .. } => {}
            }
        }
        n
    }
}

fn strings(json: &str) -> Vec<String> {
    serde_json::from_str::<Vec<String>>(json).unwrap_or_default()
}

/// The facts of every inbound INBOX message, newest first, with the fields `needs` names (the others are left empty or false).
pub fn corpus_facts_for(conn: &Connection, ctx: &Context, needs: Needs) -> rusqlite::Result<Vec<MessageFacts>> {
    let col = |on: bool, expr: &'static str, off: &'static str| if on { expr } else { off };
    let sql = format!(
        "SELECT {id}, {thread}, {account}, {name}, {from}, {to}, {cc}, {subject}, {body}, {list}, m.received_at_ms, {attachment}, m.read, {starred}, {later}, {replied} \
         FROM message m WHERE m.account_id IN (SELECT id FROM account) AND m.folder = ?1 AND lower(m.from_address) NOT IN (SELECT lower(address) FROM account) \
         {order}",
        id = col(needs.ids, "m.id", "''"),
        thread = col(needs.ids, "m.thread_id", "''"),
        account = col(needs.account || needs.ids, "m.account_id", "''"),
        name = col(needs.sender, "m.sender_name", "''"),
        from = col(needs.sender, "m.from_address", "''"),
        to = col(needs.recipients, "m.to_json", "'[]'"),
        cc = col(needs.recipients, "m.cc_json", "'[]'"),
        subject = col(needs.subject, "m.subject", "''"),
        body = col(needs.body, "COALESCE(m.body_text, m.preview)", "''"),
        list = col(needs.list, "m.has_list_header", "0"),
        attachment = col(needs.attachment, ATTACHMENT, "0"),
        starred = col(needs.starred, "m.starred", "0"),
        later = col(needs.later, LATER, "0"),
        replied = col(needs.replied, REPLIED, "0"),
        order = col(needs.ordered, "ORDER BY m.received_at_ms DESC, m.id DESC", ""),
    );
    let mut st = conn.prepare(&sql)?;
    let rows = st.query_map([INBOX_FOLDER], |r| {
        let (to, cc) = if needs.recipients { (strings(&r.get::<_, String>(5)?), strings(&r.get::<_, String>(6)?)) } else { (vec![], vec![]) };
        let mine = |a: &String| ctx.own.contains(&a.trim().to_ascii_lowercase());
        let from_address: String = r.get(4)?;
        let (later, replied): (i64, i64) = (r.get(14)?, r.get(15)?);
        let list = r.get::<_, i64>(9)? != 0;
        Ok(MessageFacts {
            message_id: r.get(0)?,
            thread_id: r.get(1)?,
            account_id: r.get(2)?,
            from_name: r.get::<_, Option<String>>(3)?.unwrap_or_default(),
            from_vip: needs.sender && is_vip(&ctx.vips, &from_address),
            from_owner: false,
            cc_me: cc.iter().any(mine) && !to.iter().any(mine),
            from_address,
            subject: r.get(7)?,
            body: r.get(8)?,
            has_list_header: list,
            has_list_unsubscribe: list,
            received_at_ms: r.get(10)?,
            has_attachment: r.get::<_, i64>(11)? != 0,
            unread: r.get::<_, i64>(12)? == 0,
            starred: r.get::<_, i64>(13)? == 1,
            // inbound and nothing newer in its thread (the waiting definition, `load::complete`)
            unanswered: needs.later && later == 0,
            replied_by_me: replied != 0,
            to,
            cc,
            ..Default::default()
        })
    })?;
    rows.collect()
}

/// `corpus_facts_for` with every field (`with_body` false leaves the body empty).
pub fn corpus_facts(conn: &Connection, ctx: &Context, with_body: bool) -> rusqlite::Result<Vec<MessageFacts>> {
    corpus_facts_for(conn, ctx, Needs { body: with_body, ..Needs::all() })
}

/// The live match count of `rules` over the store's inbound INBOX mail: how many match, how many of those are unread, the newest.
/// A store that cannot be read answers zero, as the stub did, but the cause is written to stderr in the store's own words.
pub fn rule_set_count(store: &Store, rules: RuleSet) -> RuleSetCount {
    let empty = RuleSetCount { count: 0, unread: 0, newest_at_ms: None };
    if rules.conditions.is_empty() {
        return empty;
    }
    let read = || -> rusqlite::Result<Vec<MessageFacts>> {
        let conn = open_reader(store.path())?;
        let ctx = load_context(&conn)?;
        corpus_facts_for(&conn, &ctx, Needs::of(&rules))
    };
    match read() {
        Ok(corpus) => contracts::count_rule_matches(rules, corpus.iter().map(MessageFacts::rule_facts).collect()),
        Err(e) => {
            eprintln!("rule_set_count: the mail database could not be read for the count: {e}");
            empty
        }
    }
}

/// The ids of the inbound INBOX messages `rules` matches, newest first, at most `limit`: what pane 2 of the Labels inspector draws
/// (its rows and, with one call per rule row, the numbers beside the rules). The facts are the core's, so the pane can never judge a
/// fact the core knows (answered, starred, VIP, in copy, list) on facts it does not have. Same corpus and same judge as the count.
pub fn rule_set_matching_ids(store: &Store, rules: RuleSet, limit: usize) -> Vec<String> {
    if rules.conditions.is_empty() || limit == 0 {
        return vec![];
    }
    let read = || -> rusqlite::Result<Vec<MessageFacts>> {
        let conn = open_reader(store.path())?;
        let ctx = load_context(&conn)?;
        corpus_facts_for(&conn, &ctx, Needs { ids: true, ordered: true, ..Needs::of(&rules) })
    };
    match read() {
        Ok(corpus) => corpus.into_iter().filter(|f| contracts::rule_set_matches(rules.clone(), f.rule_facts())).take(limit).map(|f| f.message_id).collect(),
        Err(e) => {
            eprintln!("rule_set_matching_ids: the mail database could not be read: {e}");
            vec![]
        }
    }
}
