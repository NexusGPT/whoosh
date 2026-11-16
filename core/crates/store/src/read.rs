//! The read queries. They take a plain connection, so the same code serves the reader connection
//! and the writer's own connection (the writer reads after its commit to build an event).
//!
//! Invariant the sync lanes must keep, not enforced here: the inbox folder is stored as `INBOX`
//! in message.folder, for every provider. Every inbox query filters on that exact text so it can use
//! the index message_by_folder (account_id, folder, received_at_ms DESC).
use crate::error::StoreError;
use contracts::{inbox_view_id, DecidedBy, InboxQuery, InboxRow, InboxView, MailThread};
use rusqlite::{params, Connection, OptionalExtension};

/// Rows in the first page of a tab. Also the page the inbox stream carries.
/// Not yet in docs/CONSTANTS.md: see docs/lanes/core-store.md "## Contract deltas".
pub const INBOX_PAGE_ROWS: usize = 50;

pub const INBOX_FOLDER: &str = "INBOX";

/// The category uncategorised mail sits in: always the Primary view's id (contracts delta 16). Primary is a view, not a
/// split tab, so hiding or reordering a split_tab row never moves the default. The connection stays in the signature so
/// every caller reads the default the same way it always did.
pub fn primary_tab(_conn: &Connection) -> Result<String, StoreError> {
    Ok(inbox_view_id(InboxView::Primary))
}

/// The tab test of every inbox read (contracts delta 16): the All view (`?2 = 'all'`) holds every row VISIBLE keeps, any
/// category; any other tab id holds the rows filed in it, unfiled rows reading as `?1` (the primary category).
fn tab_matches() -> String {
    format!("(?2 = '{}' OR COALESCE(mc.tab_id, ?1) = ?2)", inbox_view_id(InboxView::All))
}

/// real-labels: the tab id of an owner label (a label with no split tab of that id, not a view) reads as that label's filter on the All
/// view: the tab lists exactly the inbox mail filed under the label (`LABELLED`), with the SQL of every other tab, so the walk and its
/// query plan stay the ones read_budget.rs measures. A query that also names another label keeps that label; the rows filed under both are
/// kept by the caller (`inbox`). None = an ordinary tab or view.
pub fn label_tab_query(conn: &Connection, q: &InboxQuery) -> Result<Option<InboxQuery>, StoreError> {
    let Some(label) = label_tab(conn, &q.tab_id)? else { return Ok(None) };
    if q.label_id.as_ref().is_some_and(|other| *other != label) {
        return Ok(Some(InboxQuery { tab_id: inbox_view_id(InboxView::All), ..q.clone() }));
    }
    Ok(Some(InboxQuery { tab_id: inbox_view_id(InboxView::All), label_id: Some(label), ..q.clone() }))
}

/// real-labels: Some(label id) when `tab_id` names an owner label that is not a split tab and not a view.
pub fn label_tab(conn: &Connection, tab_id: &str) -> Result<Option<String>, StoreError> {
    if contracts::inbox_view_from_id(tab_id.to_string()).is_some() {
        return Ok(None);
    }
    Ok(conn
        .prepare_cached("SELECT l.id FROM label l WHERE l.id = ?1 AND l.kind <> 'builtin' AND NOT EXISTS (SELECT 1 FROM split_tab s WHERE s.id = ?1)")?
        .query_row(params![tab_id], |r| r.get::<_, String>(0))
        .optional()?)
}

pub fn enabled_tabs(conn: &Connection) -> Result<Vec<String>, StoreError> {
    let mut st = conn.prepare_cached("SELECT id FROM split_tab WHERE enabled = 1 ORDER BY sort_order, id")?;
    let rows = st.query_map([], |r| r.get::<_, String>(0))?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

/// The tab a message sits in right now, or None when the message does not exist.
pub fn tab_of(conn: &Connection, primary: &str, message_id: &str) -> Result<Option<String>, StoreError> {
    Ok(conn
        .prepare_cached(
            "SELECT COALESCE(mc.tab_id, ?1) FROM message m LEFT JOIN message_category mc ON mc.message_id = m.id WHERE m.id = ?2",
        )?
        .query_row(params![primary, message_id], |r| r.get(0))
        .optional()?)
}

/// A muted message (delta 18) is in no inbox list and no count: the Muted view holds it.
pub(crate) const VISIBLE: &str = "m.folder = 'INBOX' AND m.archived = 0 AND m.muted = 0 AND NOT EXISTS (SELECT 1 FROM snooze s WHERE s.message_id = m.id)";
pub(crate) const LABELLED: &str = "(?4 IS NULL OR EXISTS (SELECT 1 FROM label_assignment la WHERE la.message_id = m.id AND la.label_id = ?4))";

/// One list row per CONVERSATION (lead, 13 Nov, owner picture 205: one conversation of three inbox mails was listed twice, once per mail,
/// each row with the badge 3). The two list queries below stay the ordered index walks they were (store/tests/read_budget.rs measures
/// them on one million mails); they also return each mail's thread, and `inbox` keeps the FIRST mail of each thread it meets: among the
/// pinned rows the newest pin, in the walk the newest mail. A thread that has a pinned row is not listed again below it. A first version
/// asked SQLite per candidate row whether another mail of its thread stood for it; on a tab that holds 2 % of the mail that probe ran
/// for every mail of the account before the tab test and the read took seconds.
const WALK_OVERFETCH: usize = 1;   // the first page is the old page; only a page that lost rows to the dedupe is asked again, longer
/// The longest page the walk asks of one account: past it the conversations found so far are the answer (a ceiling against a thread of
/// tens of thousands of mails, never reached by a real mailbox's first page).
const WALK_ASK_MAX: usize = 1 << 16;

/// One tab's list: pinned rows first (newest pin first), then the rest newest first. At most `limit` rows.
pub fn inbox(conn: &Connection, q: &InboxQuery, limit: usize) -> Result<Vec<InboxRow>, StoreError> {
    if q.smart_folder_id.is_some() {
        // contracts delta 14: a smart folder is a saved SEARCH; the search crate runs it (search::smart_folder_rows) and
        // hands the ids to `rows_for`. Answering it here as a tab would list the wrong rows without saying so.
        return Err(StoreError::Invalid("a smart folder's rows come from its saved search (search::smart_folder_rows), not from a tab".into()));
    }
    if let Some(as_all) = label_tab_query(conn, q)? {
        let rows = inbox(conn, &as_all, limit)?;
        let tab_label = label_tab(conn, &q.tab_id)?;
        if as_all.label_id == tab_label {
            return Ok(rows);
        }
        let mut carries = conn.prepare_cached("SELECT 1 FROM label_assignment WHERE label_id = ?1 AND message_id = ?2")?;
        let mut kept = Vec::with_capacity(rows.len());
        for r in rows {
            if carries.exists(params![tab_label, r.message_id])? {
                kept.push(r);
            }
        }
        return Ok(kept);
    }
    let primary = primary_tab(conn)?;
    let accounts: Vec<String> = match &q.account_id {
        Some(a) => vec![a.clone()],
        None => {
            let mut st = conn.prepare_cached("SELECT id FROM account ORDER BY sort_order, id")?;
            let rows = st.query_map([], |r| r.get::<_, String>(0))?;
            rows.collect::<Result<Vec<_>, _>>()?
        }
    };
    let label = q.label_id.clone();

    // Pinned rows: few, so one query across the accounts is enough.
    let pinned_sql = inbox_pinned_sql();
    // one row per conversation: of a thread's pinned mails the first in this order (the newest pin) is its row
    let mut pinned_threads: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut ids: Vec<String> = Vec::new();
    {
        let mut st = conn.prepare_cached(&pinned_sql)?;
        let rows = st.query_map(params![primary, q.tab_id, q.account_id, label, limit as i64], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
        for row in rows {
            let (id, thread) = row?;
            if pinned_threads.insert(thread) {
                ids.push(id);
            }
        }
    }

    // The rest: one ordered index walk per account, each stopping at `limit`, then a merge of at most accounts x limit keys.
    let rest_sql = inbox_walk_sql();
    let room = limit.saturating_sub(ids.len());
    if room > 0 {
        let mut keys: Vec<(i64, String)> = Vec::new();
        let mut st = conn.prepare_cached(&rest_sql)?;
        for account in &accounts {
            // The walk is newest first, so the first mail met of a thread is its newest: later ones are the same conversation again and are
            // dropped, as is every mail of a thread that already has a pinned row. A page that held fewer than `room` conversations while the
            // account has more mail is asked again, four times as long, until it holds them or the account's mail ends.
            let mut ask = room.saturating_mul(WALK_OVERFETCH).max(1);
            loop {
                let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
                let mut own: Vec<(i64, String)> = Vec::new();
                let mut fetched = 0usize;
                let rows = st.query_map(params![primary, q.tab_id, account, label, ask as i64], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?)))?;
                for row in rows {
                    let (at, id, thread) = row?;
                    fetched += 1;
                    if own.len() < room && !pinned_threads.contains(&thread) && seen.insert(thread) {
                        own.push((at, id));
                    }
                }
                if own.len() >= room || fetched < ask || ask >= WALK_ASK_MAX {
                    keys.extend(own);
                    break;
                }
                ask = ask.saturating_mul(4);
            }
        }
        keys.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
        ids.extend(keys.into_iter().take(room).map(|(_, id)| id));
    }
    project(conn, &ids)
}

/// The SQL of the pinned-rows query. It drives from `pin` (CROSS JOIN fixes the order): left to itself the
/// planner walks all of `message`, which took 0.76 s on 1M messages.
pub fn inbox_pinned_sql() -> String {
    format!(
        "SELECT m.id, m.thread_id FROM pin p CROSS JOIN message m ON m.id = p.message_id LEFT JOIN message_category mc ON mc.message_id = m.id \
         WHERE {VISIBLE} AND {tab} AND (?3 IS NULL OR m.account_id = ?3) AND {LABELLED} \
         ORDER BY p.pinned_at_ms DESC, m.id LIMIT ?5",
        tab = tab_matches()
    )
}

/// The SQL of the per-account walk, for the query-plan test.
pub fn inbox_walk_sql() -> String {
    format!(
        "SELECT m.received_at_ms, m.id, m.thread_id FROM message m LEFT JOIN message_category mc ON mc.message_id = m.id \
         WHERE m.account_id = ?3 AND {VISIBLE} AND {tab} AND {LABELLED} \
         AND NOT EXISTS (SELECT 1 FROM pin p WHERE p.message_id = m.id) \
         ORDER BY m.received_at_ms DESC LIMIT ?5",
        tab = tab_matches()
    )
}

/// The SQL of a view's or a tab's size: every VISIBLE row of the tab, every account, pinned rows included.
pub fn tab_count_sql() -> String {
    format!(
        "SELECT count(*) FROM message m LEFT JOIN message_category mc ON mc.message_id = m.id WHERE {VISIBLE} AND {tab}",
        tab = tab_matches()
    )
}

/// How many messages a tab holds, the same rows its list would page through ("primary", "all" or a tab id).
pub fn tab_count(conn: &Connection, tab_id: &str) -> Result<u64, StoreError> {
    let primary = primary_tab(conn)?;
    let n: i64 = conn.prepare_cached(&tab_count_sql())?.query_row(params![primary, tab_id], |r| r.get(0))?;
    Ok(n.max(0) as u64)
}

impl crate::Store {
    /// The size of a tab or a view for the state path `view/<id>/count` (contracts delta 16).
    pub fn tab_count(&self, tab_id: &str) -> Result<u64, StoreError> {
        self.read(|c| tab_count(c, tab_id))
    }

    /// The mails a list row stands for (lead, 13 Nov: a list row is a conversation): each given id, then the other mails of its thread that
    /// the inbox lists show (INBOX, not archived, not muted, not snoozed), newest first. An id the store does not hold is kept as given, so
    /// the command that follows refuses it in its own words. No id appears twice.
    pub fn conversation_messages(&self, ids: &[String]) -> Result<Vec<String>, StoreError> {
        self.read(|c| {
            let mut st = c.prepare_cached(&format!(
                "SELECT m.id FROM message o JOIN message m ON m.thread_id = o.thread_id AND m.account_id = o.account_id \
                 WHERE o.id = ?1 AND m.id <> o.id AND {VISIBLE} ORDER BY m.received_at_ms DESC, m.id"
            ))?;
            let mut out: Vec<String> = Vec::with_capacity(ids.len());
            let mut seen = std::collections::HashSet::new();
            for id in ids {
                if seen.insert(id.clone()) { out.push(id.clone()); }
                let rows = st.query_map(params![id], |r| r.get::<_, String>(0))?;
                for r in rows {
                    let r = r?;
                    if seen.insert(r.clone()) { out.push(r); }
                }
            }
            Ok(out)
        })
    }
}

/// The category's decided_by column (and rule_id for a rule) as the contract's enum; None when the row has no category.
fn decided_by_from(word: Option<String>, rule_id: Option<String>) -> Option<DecidedBy> {
    match word?.as_str() {
        "rule" => rule_id.map(|rule_id| DecidedBy::Rule { rule_id }),
        "laya" => Some(DecidedBy::Laya),
        "jev" => Some(DecidedBy::Jev),
        "owner" => Some(DecidedBy::Owner),
        _ => Some(DecidedBy::Default),
    }
}

/// The columns of one list row over `message m`, in the order `row_from` reads them. Every list read (`project` and
/// `rows_for_messages`) selects these, so a field means the same thing in every list.
const ROW_COLUMNS: &str = "m.id, m.thread_id, m.account_id, m.sender_name, m.subject, m.preview, m.received_at_ms, m.read, \
                EXISTS (SELECT 1 FROM pin p WHERE p.message_id = m.id), \
                (SELECT count(*) FROM attachment a WHERE a.message_id = m.id AND NOT (a.inline OR coalesce(json_extract(CASE WHEN json_valid(a.remote_id) THEN a.remote_id END, '$.inline'), 0))), \
                (SELECT count(*) FROM message t WHERE t.thread_id = m.thread_id), m.from_address, \
                (SELECT c.decided_by FROM message_category c WHERE c.message_id = m.id), \
                (SELECT c.rule_id FROM message_category c WHERE c.message_id = m.id)";

/// A sender's name without the quotes some senders wrap it in (`'Ayvens'`, `"Bob"`): the name as a person reads it. Empty stays empty.
pub fn clean_sender_name(name: &str) -> String {
    let n = name.trim();
    for (a, b) in [('\'', '\''), ('"', '"'), ('\u{2018}', '\u{2019}'), ('\u{201C}', '\u{201D}')] {
        if n.chars().count() >= 2 && n.starts_with(a) && n.ends_with(b) { return n[a.len_utf8()..n.len() - b.len_utf8()].trim().to_string(); }
    }
    n.to_string()
}

/// The sender as a row shows it: the cleaned name, and the address when the mail carries no name (owner's pictures 213 and 214, 13 Nov 2026: a row with
/// an empty sender line; 16 of his 288 mails carry no name, each with an address).
pub fn shown_sender(name: &str, address: &str) -> String {
    let n = clean_sender_name(name);
    if n.is_empty() { address.trim().to_string() } else { n }
}

/// One list row from a result row of `ROW_COLUMNS`; its label names are filled by the caller.
fn row_from(r: &rusqlite::Row<'_>) -> rusqlite::Result<InboxRow> {
    Ok(InboxRow {
        message_id: r.get(0)?,
        thread_id: r.get(1)?,
        account_id: r.get(2)?,
        sender_name: shown_sender(&r.get::<_, String>(3)?, &r.get::<_, String>(11)?),
        subject: r.get(4)?,
        preview: r.get(5)?,
        received_at_ms: r.get(6)?,
        unread: r.get::<_, i64>(7)? == 0,
        pinned: r.get::<_, i64>(8)? != 0,
        label_names: Vec::new(),
        attachment_count: r.get::<_, i64>(9)?.max(0) as u32,
        thread_count: r.get::<_, i64>(10)?.max(0) as u32,
        sender_address: r.get(11)?,
        decided_by: decided_by_from(r.get::<_, Option<String>>(12)?, r.get::<_, Option<String>>(13)?),
        reminder_fired: false,
    })
}

/// Turn ids into list rows, in the order given. An id that no longer exists is skipped.
pub fn project(conn: &Connection, ids: &[String]) -> Result<Vec<InboxRow>, StoreError> {
    let mut row_st = conn.prepare_cached(&format!("SELECT {ROW_COLUMNS} FROM message m WHERE m.id = ?1"))?;
    let mut label_st = conn.prepare_cached(
        "SELECT l.name FROM label_assignment la JOIN label l ON l.id = la.label_id WHERE la.message_id = ?1 ORDER BY l.name",
    )?;
    let mut out = Vec::with_capacity(ids.len());
    for id in ids {
        let base = row_st.query_row(params![id], row_from).optional()?;
        if let Some(mut row) = base {
            let names = label_st.query_map(params![id], |r| r.get::<_, String>(0))?;
            row.label_names = names.collect::<Result<Vec<_>, _>>()?;
            out.push(row);
        }
    }
    Ok(out)
}

/// Ids per statement in `rows_for_messages`: under SQLite's bound-variable limit on every build (999 before SQLite 3.32,
/// 32766 since), and small enough that the statement cache keeps at most two texts (a full chunk and the last one).
pub const ROWS_FOR_MESSAGES_CHUNK: usize = contracts::constants::ROWS_FOR_MESSAGES_CHUNK as usize;

/// The list row of each message id, whatever folder holds it (SPEC A141: the search screen draws every hit). One row per id
/// that names a stored message, in the order given; an unknown id is skipped; a repeated id gives one row, at its first place.
/// No tab, folder, archive, snooze, account or page filter. Two statements per ROWS_FOR_MESSAGES_CHUNK ids (the rows, then
/// their label names), where `project` runs two per id.
pub fn rows_for_messages(conn: &Connection, ids: &[String]) -> Result<Vec<InboxRow>, StoreError> {
    let mut seen = std::collections::HashSet::with_capacity(ids.len());
    let wanted: Vec<&String> = ids.iter().filter(|id| seen.insert(id.as_str())).collect();
    let mut found: std::collections::HashMap<String, InboxRow> = std::collections::HashMap::with_capacity(wanted.len());
    for chunk in wanted.chunks(ROWS_FOR_MESSAGES_CHUNK) {
        let marks = vec!["?"; chunk.len()].join(",");
        let mut row_st = conn.prepare_cached(&format!("SELECT {ROW_COLUMNS} FROM message m WHERE m.id IN ({marks})"))?;
        let rows = row_st.query_map(rusqlite::params_from_iter(chunk.iter()), row_from)?;
        for row in rows {
            let row = row?;
            found.insert(row.message_id.clone(), row);
        }
        // the label names of the chunk, each message's sorted by name as `project` sorts them
        let mut label_st = conn.prepare_cached(&format!(
            "SELECT la.message_id, l.name FROM label_assignment la JOIN label l ON l.id = la.label_id \
             WHERE la.message_id IN ({marks}) ORDER BY la.message_id, l.name"
        ))?;
        let names = label_st.query_map(rusqlite::params_from_iter(chunk.iter()), |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
        for pair in names {
            let (id, name) = pair?;
            if let Some(row) = found.get_mut(&id) {
                row.label_names.push(name);
            }
        }
    }
    Ok(wanted.into_iter().filter_map(|id| found.remove(id.as_str())).collect())
}

/// A snoozed message and when it comes back.
#[derive(Debug, Clone, PartialEq)]
pub struct SnoozedRow {
    pub row: InboxRow,
    pub return_at_ms: i64,
}

/// The Snoozed mailbox: soonest return first.
pub fn snoozed(conn: &Connection) -> Result<Vec<SnoozedRow>, StoreError> {
    let mut st = conn.prepare_cached("SELECT message_id, return_at_ms FROM snooze ORDER BY return_at_ms, message_id")?;
    let pairs: Vec<(String, i64)> = st
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<Result<Vec<_>, _>>()?;
    let mut out = Vec::with_capacity(pairs.len());
    for (id, return_at_ms) in pairs {
        if let Some(row) = project(conn, std::slice::from_ref(&id))?.into_iter().next() {
            out.push(SnoozedRow { row, return_at_ms });
        }
    }
    Ok(out)
}

/// The zero state reads this: the next time any snoozed message returns.
pub fn next_snooze_return_ms(conn: &Connection) -> Result<Option<i64>, StoreError> {
    Ok(conn.query_row("SELECT min(return_at_ms) FROM snooze", [], |r| r.get(0))?)
}

/// The Pinned mailbox: newest pin first.
pub fn pinned(conn: &Connection) -> Result<Vec<InboxRow>, StoreError> {
    let mut st = conn.prepare_cached("SELECT message_id FROM pin ORDER BY pinned_at_ms DESC, message_id")?;
    let ids: Vec<String> = st.query_map([], |r| r.get(0))?.collect::<Result<Vec<_>, _>>()?;
    project(conn, &ids)
}

/// One thread: its messages oldest first, its participants in order of first appearance.
pub fn thread(conn: &Connection, thread_id: &str) -> Result<Option<MailThread>, StoreError> {
    let head: Option<(String, String)> = conn
        .query_row("SELECT account_id, subject FROM thread WHERE id = ?1", params![thread_id], |r| Ok((r.get(0)?, r.get(1)?)))
        .optional()?;
    let Some((account_id, subject)) = head else { return Ok(None) };
    let mut st = conn.prepare_cached("SELECT id, from_address, sender_name FROM message WHERE thread_id = ?1 ORDER BY received_at_ms, id")?;
    let rows: Vec<(String, String, String)> = st
        .query_map(params![thread_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
        .collect::<Result<Vec<_>, _>>()?;
    // "Name <address>", the contract's form (the reader draws the part before " <"): the name the mail itself carries, the address alone
    // when it carries none. Lead, 13 Nov (owner A162, real-engine walk and the owner's picture 205): the reader's header line read a raw
    // address beside a list row that read the person's name.
    let mut seen: Vec<&str> = Vec::new();
    let mut participants: Vec<String> = Vec::new();
    for (_, from, name) in &rows {
        if !seen.contains(&from.as_str()) {
            seen.push(from.as_str());
            let name = clean_sender_name(name);
            participants.push(if name.is_empty() || name.eq_ignore_ascii_case(from) { from.clone() } else { format!("{name} <{from}>") });
        }
    }
    Ok(Some(MailThread { id: thread_id.to_string(), account_id, subject, participants, message_ids: rows.into_iter().map(|(id, _, _)| id).collect() }))
}

/// The SQL of the unread-ids read: the same view the inbox shows (same tab, account and label filters), unread rows only,
/// ids only, no ordering and no row projection. One index walk per account (message_by_folder).
pub fn unread_ids_sql() -> String {
    format!(
        "SELECT m.id FROM message m LEFT JOIN message_category mc ON mc.message_id = m.id \
         WHERE m.account_id = ?3 AND m.read = 0 AND {VISIBLE} AND {tab} AND {LABELLED}",
        tab = tab_matches()
    )
}

/// Every unread message id of one inbox view (a tab, optionally one account, optionally one label). Pinned rows count:
/// a pinned message is part of the view. mark_all_read asks this instead of walking the whole view as rows.
pub fn unread_ids(conn: &Connection, q: &InboxQuery) -> Result<Vec<String>, StoreError> {
    // real-labels: a label tab's unread are the label's unread on the All view (`label_tab_query`)
    if let Some(as_all) = label_tab_query(conn, q)? {
        if as_all.label_id == label_tab(conn, &q.tab_id)? {
            return unread_ids(conn, &as_all);
        }
    }
    let primary = primary_tab(conn)?;
    let accounts: Vec<String> = match &q.account_id {
        Some(a) => vec![a.clone()],
        None => {
            let mut st = conn.prepare_cached("SELECT id FROM account ORDER BY sort_order, id")?;
            let rows = st.query_map([], |r| r.get::<_, String>(0))?;
            rows.collect::<Result<Vec<_>, _>>()?
        }
    };
    let mut st = conn.prepare_cached(&unread_ids_sql())?;
    let mut out = Vec::new();
    for account in &accounts {
        let rows = st.query_map(params![primary, q.tab_id, account, q.label_id], |r| r.get::<_, String>(0))?;
        for row in rows {
            out.push(row?);
        }
    }
    Ok(out)
}

/// The descendant query (migrations lane, docs/lanes/migrations.md "## Round 2"): the messages that carry the label or any
/// label below it. `path >= P || ' > ' AND path < P || ' ?'` is the prefix match on "P > " written as a range ('?' sorts one
/// byte after '>'): it uses label_by_path, reads '%' and '_' in names as text (LIKE would not), and does not match
/// "Clients 2" or "Clients%" under "Clients".
pub fn messages_under_label_sql() -> &'static str {
    "SELECT m.id FROM message m WHERE m.id IN ( \
        SELECT la.message_id FROM label_assignment la WHERE la.label_id IN ( \
            SELECT ?1 UNION ALL \
            SELECT d.id FROM label t JOIN label d ON d.path >= t.path || ' > ' AND d.path < t.path || ' ?' WHERE t.id = ?1)) \
     ORDER BY m.received_at_ms DESC, m.id LIMIT ?2"
}

/// The newest `limit` messages under a label, sub-labels included, in any folder (the caller filters if it needs the inbox only).
pub fn messages_under_label(conn: &Connection, label_id: &str, limit: usize) -> Result<Vec<InboxRow>, StoreError> {
    let mut st = conn.prepare_cached(messages_under_label_sql())?;
    let ids: Vec<String> = st.query_map(params![label_id, limit as i64], |r| r.get(0))?.collect::<Result<Vec<_>, _>>()?;
    project(conn, &ids)
}

/// gap-remind: a message of a thread whose reminder fired, as the return to the inbox needs it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReminderReturnCandidate {
    pub message_id: String,
    pub account_id: String,
    pub folder: String,
    pub archived: bool,
}

impl crate::Store {
    /// The reminder's projection onto the inbox rows: the thread ids whose reply reminder is FIRED at `now_ms` by the contract's rule (its
    /// time came and no reply arrived by then; the same set the Reminders view lists, so the two never disagree). Read only; one query
    /// over the `reminder` table, whatever the page size. A thread with a waiting or an answered reminder is not in it.
    pub fn reminder_fired_threads(&self, now_ms: i64) -> Result<std::collections::HashSet<String>, StoreError> {
        self.read(move |c| {
            let sent = crate::reminders::fired_message_ids(c, now_ms, "", 10_000)?;
            let mut st = c.prepare_cached("SELECT thread_id FROM message WHERE id = ?1")?;
            let mut out = std::collections::HashSet::with_capacity(sent.len());
            for id in &sent {
                if let Some(t) = st.query_row(params![id], |r| r.get::<_, String>(0)).optional()? {
                    out.insert(t);
                }
            }
            Ok(out)
        })
    }

    /// The messages the owner RECEIVED in the threads of these sent messages, with where each sits now: what the return to the inbox moves.
    pub fn reminder_return_candidates(&self, sent_ids: &[String]) -> Result<Vec<ReminderReturnCandidate>, StoreError> {
        let sent = sent_ids.to_vec();
        self.read(move |c| {
            let mut st = c.prepare_cached(
                "SELECT m.id, m.account_id, m.folder, m.archived FROM message m \
                 WHERE m.thread_id = (SELECT thread_id FROM message WHERE id = ?1) \
                   AND lower(m.from_address) NOT IN (SELECT lower(address) FROM account) ORDER BY m.received_at_ms, m.id",
            )?;
            let mut out = Vec::new();
            for id in &sent {
                let rows = st.query_map(params![id], |r| {
                    Ok(ReminderReturnCandidate { message_id: r.get(0)?, account_id: r.get(1)?, folder: r.get(2)?, archived: r.get::<_, i64>(3)? != 0 })
                })?;
                for row in rows {
                    out.push(row?);
                }
            }
            Ok(out)
        })
    }
}
