//! The two counts per tab (contracts::counts): unread and waiting. The tally is kept in memory by the
//! writer and moved by the DIFFERENCE a write makes to the messages it declared; `recount` is the
//! from-scratch query the tally must always equal (tests/counts_match.rs holds them equal).
//!
//! unread  = unread messages in the tab: inbox folder, not archived, not snoozed, not muted (delta 18).
//! waiting = the latest message of a thread (any folder) is inbound (not from an own account address),
//!           sits in the inbox folder, and is not archived or snoozed: the owner has not answered it.
use crate::error::StoreError;
use contracts::{inbox_tabs, inbox_view_from_id, inbox_view_id, AiReads, Counts, InboxTabKind, InboxView, LabelKind, LabelScope, MailLabel, TabCounts, MAILBOX_ALL_KEY};
use rusqlite::{params, Connection, OptionalExtension};
use std::collections::HashMap;

/// tab id -> (unread, waiting)
pub type Tally = HashMap<String, (i64, i64)>;

/// The tally's own entry for the VIPs mailbox row (unread only; waiting stays 0). It is no tab: `build` leaves it out of the All view's sum, and no
/// split tab or label can carry this id.
pub const VIPS_KEY: &str = "\u{1}mailbox:vips";

/// The VIPs view's sender rule, word for word as `StoredView::Vips` in realcore.rs `view_rows` writes it (a VIP address, or a VIP domain by suffix).
/// tests/vips_unread.rs holds the figure equal to the unread rows that view lists.
const VIP_SENDER: &str = "(EXISTS (SELECT 1 FROM vip v WHERE v.kind = 'address' AND v.value = lower(m.from_address)) \
    OR EXISTS (SELECT 1 FROM vip v WHERE v.kind = 'domain' AND lower(m.from_address) LIKE '%@' || v.value))";

/// Unread rows of the VIPs view (VISIBLE rows from a VIP sender), from scratch, or those of one message when `message_id` is given.
fn vips_unread(conn: &Connection, message_id: Option<&str>) -> Result<i64, StoreError> {
    let sql = format!("SELECT count(*) FROM message m WHERE m.read = 0 AND {VIP_SENDER} AND {visible} AND (?1 IS NULL OR m.id = ?1)", visible = crate::read::VISIBLE);
    Ok(conn.prepare_cached(&sql)?.query_row(params![message_id], |r| r.get::<_, i64>(0))?)
}

/// What one message adds to the VIPs unread figure right now (0 or 1).
pub fn vips_contribution(conn: &Connection, message_id: &str) -> Result<i64, StoreError> { vips_unread(conn, Some(message_id)) }

const WAITING: &str = "m.from_address NOT IN (SELECT address FROM account) \
    AND NOT EXISTS (SELECT 1 FROM message n WHERE n.thread_id = m.thread_id \
        AND (n.received_at_ms > m.received_at_ms OR (n.received_at_ms = m.received_at_ms AND n.id > m.id)))";

fn grouped_sql(extra: &str) -> String {
    format!(
        "SELECT COALESCE(mc.tab_id, ?1) AS tab, \
                SUM(CASE WHEN m.read = 0 THEN 1 ELSE 0 END), \
                SUM(CASE WHEN {WAITING} THEN 1 ELSE 0 END) \
         FROM message m LEFT JOIN message_category mc ON mc.message_id = m.id \
         WHERE m.archived = 0 AND m.muted = 0 AND m.folder = 'INBOX' AND NOT EXISTS (SELECT 1 FROM snooze s WHERE s.message_id = m.id) {extra} \
         GROUP BY tab"
    )
}

/// The tally from scratch: one pass over the messages.
pub fn recount(conn: &Connection, primary: &str) -> Result<Tally, StoreError> {
    let mut st = conn.prepare_cached(&grouped_sql(""))?;
    let rows = st.query_map(params![primary], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?, r.get::<_, i64>(2)?)))?;
    let mut tally = Tally::new();
    for row in rows {
        let (tab, unread, waiting) = row?;
        tally.insert(tab, (unread, waiting));
    }
    tally.insert(VIPS_KEY.to_string(), (vips_unread(conn, None)?, 0));
    Ok(tally)
}

/// What one message adds to the tally right now (None when it adds nothing or does not exist).
pub fn contribution(conn: &Connection, primary: &str, message_id: &str) -> Result<Option<(String, i64, i64)>, StoreError> {
    Ok(conn
        .prepare_cached(&grouped_sql("AND m.id = ?2"))?
        .query_row(params![primary, message_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .optional()?)
}

pub fn add(into: &mut Tally, tab: &str, unread: i64, waiting: i64, sign: i64) {
    let e = into.entry(tab.to_string()).or_insert((0, 0));
    e.0 += sign * unread;
    e.1 += sign * waiting;
}

/// real-labels: (unread, waiting) of an owner label's tab: the inbox rows filed under the label (the same VISIBLE test as every tab),
/// one walk of the label's assignments (their primary key leads with label_id).
pub fn label_tab_figures(conn: &Connection, label_id: &str) -> Result<(i64, i64), StoreError> {
    let sql = format!(
        "SELECT COALESCE(SUM(CASE WHEN m.read = 0 THEN 1 ELSE 0 END), 0), COALESCE(SUM(CASE WHEN {WAITING} THEN 1 ELSE 0 END), 0) \
         FROM label_assignment la JOIN message m ON m.id = la.message_id WHERE la.label_id = ?1 AND {visible}",
        visible = crate::read::VISIBLE
    );
    Ok(conn.prepare_cached(&sql)?.query_row(params![label_id], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?)))?)
}

/// The labels `contracts::inbox_tabs` needs to say which labels are tabs and in what order (id, parent, place among siblings, pinned, kind, name):
/// ONE query, the same `ORDER BY sort_order, id` as label_meta.rs `all`, so the tree order is the one `inbox/tabs` draws. The other fields are not read.
fn strip_labels(conn: &Connection) -> Result<Vec<MailLabel>, StoreError> {
    let mut st = conn.prepare_cached("SELECT id, name, parent_id, pinned, kind, sort_order FROM label ORDER BY sort_order, id")?;
    let rows = st.query_map([], |r| {
        let kind: String = r.get(4)?;
        Ok(MailLabel {
            id: r.get(0)?, name: r.get(1)?, parent_id: r.get(2)?, colour: None, account_ids: vec![], path: String::new(), depth: 1, unread: 0,
            description: String::new(), pinned: r.get::<_, i64>(3)? == 1, scope: LabelScope::All,
            kind: if kind == "builtin" { LabelKind::BuiltIn } else { LabelKind::Owner }, order: r.get::<_, i64>(5)? as u32,
            entry_enabled: true, ai_enabled: true, exit_enabled: false,
            ai_reads: AiReads { subject: true, body: true, rest_of_thread: false, attachments: false }, exit_fallback: None,
        })
    })?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

/// The label tabs in the order `inbox/tabs` (engine reads.rs) draws them: the pinned labels in tree order (`contracts::inbox_tabs`), then the ones that have a
/// `split_tab` row take the places of those tabs in the order of their rows (a tab without a row, an owner label, keeps its place). `rows` = every
/// `split_tab` id in order. Moving the tabs is the same step reads.rs takes, so a ReorderSplitTabs moves both lists alike.
fn strip_order(labels: Vec<MailLabel>, rows: &[(String, bool)]) -> Vec<(String, LabelKind)> {
    let kinds: HashMap<String, LabelKind> = labels.iter().map(|l| (l.id.clone(), l.kind)).collect();
    let mut tabs: Vec<String> = inbox_tabs(labels).into_iter().filter(|t| t.kind == InboxTabKind::Label).map(|t| t.id).collect();
    let slots: Vec<usize> = (0..tabs.len()).filter(|&i| rows.iter().any(|(id, _)| *id == tabs[i])).collect();
    let mut moved: Vec<String> = rows.iter().filter(|(id, _)| slots.iter().any(|&i| tabs[i] == *id)).map(|(id, _)| id.clone()).collect();
    if moved.len() == slots.len() {
        for i in slots { tabs[i] = moved.remove(0); }
    }
    tabs.into_iter().map(|id| { let kind = kinds.get(&id).copied().unwrap_or(LabelKind::Owner); (id, kind) }).collect()
}

/// The value the counts stream carries (contracts delta 16): the Primary view, the All view, then the tabs in the order of `inbox/tabs` (the pinned
/// labels in tree order, the tabs that have a `split_tab` row in the order of those rows), a hidden tab (`enabled = 0`) out; then any enabled `split_tab`
/// row that is not a label (a sender, domain or account tab), in row order. Primary is the "primary" tally entry; All is the sum of EVERY tally entry
/// (the tally is keyed by category, hidden tabs included, and holds inbox rows only, so Junk and Trash are never in it): All >= Primary.
/// The order costs two queries (the labels, the `split_tab` rows) whatever the number of tabs; each owner label's figures are the one walk of `label_tab_figures`.
pub fn build(conn: &Connection, tally: &Tally) -> Result<Counts, StoreError> {
    let one_tab = |tab_id: String, (u, w): (i64, i64)| TabCounts { tab_id, unread: u.max(0) as u32, waiting: w.max(0) as u32 };
    let primary = inbox_view_id(InboxView::Primary);
    let primary_figures = tally.get(&primary).copied().unwrap_or((0, 0));
    let all_figures = tally.iter().filter(|(k, _)| k.as_str() != VIPS_KEY).fold((0, 0), |acc, (_, (u, w))| (acc.0 + u.max(&0), acc.1 + w.max(&0)));
    let mut tabs = vec![one_tab(primary, primary_figures), one_tab(inbox_view_id(InboxView::All), all_figures)];
    let rows: Vec<(String, bool)> = {
        let mut st = conn.prepare_cached("SELECT id, enabled FROM split_tab ORDER BY sort_order, id")?;
        let found = st.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)? != 0)))?;
        found.collect::<Result<Vec<_>, _>>()?
    };
    // real-labels: a pinned owner label is a tab too (D1 "Labels are the tabs"). Its figures are the inbox mail filed under it (`label_tab_figures`), the rows
    // read.rs `inbox` lists for it; unpinned, it is no tab. A built-in label's tab is its split tab (its figures are the tally's), which its pin switches
    // (label_meta.rs `label_tab_follow`).
    for (id, kind) in strip_order(strip_labels(conn)?, &rows) {
        if inbox_view_from_id(id.clone()).is_some() || tabs.iter().any(|t| t.tab_id == id) { continue; }
        match rows.iter().find(|(row, _)| *row == id) {
            Some((_, false)) => {}                                                            // hidden: out
            Some(_) => { let figures = tally.get(&id).copied().unwrap_or((0, 0)); tabs.push(one_tab(id.clone(), figures)); }
            None if kind == LabelKind::Owner => { let figures = label_tab_figures(conn, &id)?; tabs.push(one_tab(id, figures)); }
            None => {}                                                                        // a built-in label with no split tab is no tab, as before
        }
    }
    for (id, enabled) in &rows {
        if *enabled && inbox_view_from_id(id.clone()).is_none() && !tabs.iter().any(|t| t.tab_id == *id) {
            let figures = tally.get(id).copied().unwrap_or((0, 0));
            tabs.push(one_tab(id.clone(), figures));
        }
    }
    let one = |sql: &str| -> Result<u32, StoreError> { Ok(conn.query_row(sql, [], |r| r.get::<_, i64>(0))?.max(0) as u32) };
    let mut mailboxes = HashMap::new();
    // All inboxes: the unread of the inbox itself, which is the All view's figure (every category summed). The key was in the contract and nothing filled it:
    // the sidebar's All inboxes row drew no count, and the menu bar fell back to a sum over every tab (Primary, All and each pinned label: about twice the mail).
    mailboxes.insert(MAILBOX_ALL_KEY.to_string(), all_figures.0.max(0) as u32);
    // VIPs: the unread rows of the VIPs view, kept in the tally like the figures above (never a scan of the messages here).
    mailboxes.insert("vips".to_string(), tally.get(VIPS_KEY).map(|f| f.0).unwrap_or(0).max(0) as u32);
    mailboxes.insert("snoozed".to_string(), one("SELECT count(*) FROM snooze")?);
    mailboxes.insert("pinned".to_string(), one("SELECT count(*) FROM pin")?);
    mailboxes.insert("drafts".to_string(), one("SELECT count(*) FROM draft")?);
    mailboxes.insert("outbox".to_string(), one("SELECT count(*) FROM outbox_entry WHERE state NOT IN ('sent','cancelled')")?);
    Ok(Counts { tabs, mailboxes })
}
