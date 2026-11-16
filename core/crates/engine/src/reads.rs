//! The mail reads of the real core (slice 2), from what the IMAP sync stored: the list for a tab, a mailbox or an
//! account set with the list filters, one message, its body (fetched from the server once, then kept), and the
//! mailboxes' folders by role. Pure translation over the store; nothing here writes except the kept body.
use contracts::{
    account_in, inbox_tabs, inbox_view_from_id, inbox_view_name, passes_filters, query_accounts, InboxQuery, InboxRow, InboxTab, InboxTabKind,
    BodyLayout, BodySegment, Mailbox, MessageBody, SegmentKind, SegmentVisibility, TabCounts, TextRange,
};
use store::{Store, StoredView, INBOX_PAGE_ROWS};
use sync_imap::{role_for, FolderRole};

use crate::imap_source::Roles;

/// The role a mailbox list shows, or None for the mailboxes that are not a server folder.
pub fn mailbox_role(m: Mailbox) -> Option<FolderRole> {
    match m {
        Mailbox::Sent => Some(FolderRole::Sent),
        Mailbox::Drafts => Some(FolderRole::Drafts),
        Mailbox::Junk => Some(FolderRole::Junk),
        Mailbox::Trash => Some(FolderRole::Trash),
        Mailbox::Archive => Some(FolderRole::Archive),
        _ => None,
    }
}

/// Each account's server folders that hold `role`: the role the server named at its last LIST (SPECIAL-USE), else the
/// usual names (`role_for` with no attributes) for folders stored before this launch's LIST.
pub fn folders_with_role(store: &Store, roles: &Roles, accounts: &[String], role: FolderRole) -> Vec<(String, String)> {
    let named = roles.lock().map(|g| g.clone()).unwrap_or_default();
    let mut out = Vec::new();
    for a in accounts {
        let stored = store.account_folders(a).unwrap_or_default();
        for f in stored {
            let r = named.get(a).and_then(|m| m.get(&f).copied()).unwrap_or_else(|| role_for(&f, &[]));
            if r == role { out.push((a.clone(), f)); }
        }
    }
    out
}

fn newest_first(rows: &mut [InboxRow]) {
    rows.sort_by(|a, b| b.pinned.cmp(&a.pinned).then_with(|| b.received_at_ms.cmp(&a.received_at_ms)).then_with(|| a.message_id.cmp(&b.message_id)));
}

/// The list a query shows. A tab reads the store's tab walk (one account, or each account of the set merged); a mailbox
/// reads its role's folders, its flag column or its own table; the filters keep a row only when it passes them all. A
/// smart folder is a saved search: the search slice answers it, so it is empty here (docs/lanes/contracts.md).
pub fn inbox(store: &Store, roles: &Roles, q: &InboxQuery) -> Vec<InboxRow> { inbox_rows(store, roles, q, INBOX_PAGE_ROWS) }

/// `inbox` with the number of rows said (`WhooshCore::inbox_rows`): the newest `limit` rows of the list, in the list's order.
pub fn inbox_rows(store: &Store, roles: &Roles, q: &InboxQuery, limit: usize) -> Vec<InboxRow> {
    if q.smart_folder_id.is_some() { return vec![]; }
    let every: Vec<String> = store.account_summaries().unwrap_or_default().into_iter().map(|a| a.id).collect();
    let scope: Option<Vec<String>> = query_accounts(q.clone());
    let in_scope: Vec<String> = scope.clone().unwrap_or_else(|| every.clone());
    let mut rows: Vec<InboxRow> = match q.mailbox {
        Some(m) => match m {
            Mailbox::Pinned => store.pinned().unwrap_or_default(),
            Mailbox::Snoozed => store.snoozed().unwrap_or_default().into_iter().map(|s| s.row).collect(),
            Mailbox::Outbox => vec![],
            Mailbox::Starred => store.view_rows(StoredView::Starred, scope.clone(), limit).unwrap_or_default(),
            Mailbox::Muted => store.view_rows(StoredView::Muted, scope.clone(), limit).unwrap_or_default(),
            Mailbox::AllMail => store.view_rows(StoredView::AllMail, scope.clone(), limit).unwrap_or_default(),
            Mailbox::Vips => store.view_rows(StoredView::Vips, scope.clone(), limit).unwrap_or_default(),
            // delta 18: the threads whose reminder fired (the time came and no reply arrived by then)
            Mailbox::Reminders => store.view_rows(StoredView::Reminders { now_ms: crate::rust_core::now_ms() }, scope.clone(), limit).unwrap_or_default(),
            // engine-act-r6: a mailbox of server folders reads the store only (never the network). Archive also lists what the owner
            // archived with E (the row stays INBOX + archived; the server's copy moves, and the sync does not re-read Archive at once);
            // the server copy a core move recorded is the moved message's, never a second row (store `mailbox_rows`)
            other => match mailbox_role(other) {
                Some(role) => {
                    let flagged = (role == FolderRole::Archive).then_some(in_scope.as_slice());
                    store.mailbox_rows(&folders_with_role(store, roles, &in_scope, role), flagged, limit).unwrap_or_default()
                }
                None => vec![],
            },
        },
        None => match &scope {
            None => store.inbox_limit(q, limit).unwrap_or_default(),
            Some(set) => {
                let mut merged = Vec::new();
                for a in set {
                    let one = InboxQuery { account_id: Some(a.clone()), account_ids: vec![], ..q.clone() };
                    merged.extend(store.inbox_limit(&one, limit).unwrap_or_default());
                }
                newest_first(&mut merged);
                merged.truncate(limit);
                merged
            }
        },
    };
    rows.retain(|r| account_in(q, &r.account_id));
    crate::reminders::mark_rows(store, &mut rows, crate::rust_core::now_ms());   // gap-remind: the reminder chip of a fired thread
    if !q.filters.is_empty() {
        let ids: Vec<String> = rows.iter().map(|r| r.message_id.clone()).collect();
        let facts = store.filter_facts(&ids).unwrap_or_default();
        rows.retain(|r| facts.get(&r.message_id).is_some_and(|f| passes_filters(*f, q.filters.clone())));
    }
    rows
}

/// engine-act: the Outbox mailbox, as the fixture core lists it (`entryRows`): one row per open outbox entry of the query's accounts,
/// newest first, keyed by its draft (the composer reopens it), named by its recipients.
pub fn outbox_rows(_store: &Store, entries: &[contracts::OutboxEntry], q: &InboxQuery) -> Vec<InboxRow> {
    let mut list: Vec<&contracts::OutboxEntry> = entries.iter().filter(|e| account_in(q, &e.draft.from_account_id)).collect();
    list.sort_by(|a, b| b.created_at_ms.cmp(&a.created_at_ms));
    list.into_iter()
        .map(|e| {
            let d = &e.draft;
            InboxRow {
                message_id: d.id.clone(), thread_id: d.reply_to_thread_id.clone().unwrap_or_else(|| d.id.clone()), account_id: d.from_account_id.clone(),
                sender_name: if d.to.is_empty() { "No recipient".into() } else { d.to.join(", ") },
                subject: if d.subject.is_empty() { "(no subject)".into() } else { d.subject.clone() }, preview: d.body.chars().take(160).collect(),
                received_at_ms: e.created_at_ms, unread: false, pinned: false, label_names: vec![], attachment_count: 0, thread_count: 1,
                sender_address: String::new(), decided_by: None, reminder_fired: false,
            }
        })
        .collect()
}

/// A body as the reader shows it: the plain text, the sanitised html (None for a plain-text message), where the quote starts.
pub fn body_from(message_id: &str, text: String, safe_html: Option<String>, trackers_blocked: u32) -> MessageBody {
    let quote_offset = safe_html.as_deref().and_then(parse::html_quote_start).map(|o| o as u32);
    let quoted_ranges = parse::text_quote_start(&text).map(|s| vec![TextRange { start: s as u32, end: text.len() as u32 }]).unwrap_or_default();
    MessageBody {
        message_id: message_id.to_string(), safe_html, plain_text: text, trackers_blocked, quote_offset, quoted_ranges,
        segments: Vec::new(), fold_from: None, layout: None, layout_reasons: Vec::new(), display_html: None, written_html: None, body_warnings: Vec::new(),
        pictures_blocked: 0, pixel_trackers: 0,
    }
}

/// body-wire (delta 26): a STORED body as the reader shows it: `body_from` plus the mail content engine's result the body fetch kept
/// (table `message_prepared`, JSON). Reads only: the engine never runs here. A body stored before migration 0019, or whose kept JSON cannot be
/// read, has the delta-26 fields empty (never an error).
pub fn body_from_stored(message_id: &str, text: String, safe_html: Option<String>, trackers_blocked: u32, prepared: Option<&str>) -> MessageBody {
    // owner's picture 216 (14 Nov 2026): a text part of page code, kept before the parser read such a part as its words, is read as its
    // words here: the reader, a reply's quote and a copy never get the code.
    let code = parse::text_is_page_code(&text).then(|| parse::page_code_as_page(&text));
    let text = match &code { Some(page) => parse::page_code_words(page), None => text };
    let mut b = body_from(message_id, text, safe_html, trackers_blocked);
    if let Some(p) = prepared { apply_prepared(&mut b, p); }
    if let Some(page) = &code { page_from_code(&mut b, page); }
    show_hidden_words(&mut b);
    b
}

/// owner's picture 216: the stored page of a mail whose text part is page code may have lost words that code still holds (a tag cut
/// short took the sentence after it as its attributes). A stored page that holds under PARSE_PAGE_HOLDS_TEXT_PERCENT of the words is made
/// again from the text part's code, its cut tags ended. The one place where a read runs the engine: on the few mails of this kind, on a
/// page of the text part's size, under the engine's own time limit (past it the stored page stays).
fn page_from_code(b: &mut MessageBody, page: &str) {
    let want = b.plain_text.split_whitespace().count();
    let have = b.display_html.as_deref().or(b.safe_html.as_deref()).map_or(0, |p| parse::shown(p).all_words);
    if want == 0 || have * 100 >= want * parse::PAGE_HOLDS_TEXT_PERCENT { return; }
    let parts = parse::Parts { text: Some(b.plain_text.clone()), html: Some(page.to_string()), ..Default::default() };
    let opts = parse::Options { deadline: Some(std::time::Instant::now() + std::time::Duration::from_millis(parse::PREPARE_TIME_LIMIT_MS)), ..Default::default() };
    let made = parse::prepare(&parts, &[], &opts);
    if made.degraded { return; }
    b.safe_html = Some(made.display_html.clone());
    b.quote_offset = b.safe_html.as_deref().and_then(parse::html_quote_start).map(|o| o as u32);
    b.trackers_blocked = made.remote_images_blocked;
    apply_prepared(b, &prepared_json(&made));
}

/// owner's picture 215 (14 Nov 2026): a mail drew as a white card with no word. Its page kept its words in a block at font-size 0 (the
/// sender's template lost the cell that sets the size again). Such a page is not served as it is (`parse::page_hides_text`; a page built
/// of pictures keeps its look). The reader gets the page's own words in the app's type, its links and pictures kept and the sender's
/// boxes left out (`parse::plain_document`), whole, nothing folded; when the page does not hold PARSE_PAGE_HOLDS_TEXT_PERCENT of the
/// text part's words, it gets the text part.
fn show_hidden_words(b: &mut MessageBody) {
    let Some(page) = b.display_html.as_deref() else { return };
    if !parse::page_hides_text(page, &b.plain_text) { return; }
    let reason = parse::LayoutReason::PageHidesText.as_str().to_string();
    let plain = parse::plain_document(page);
    let want = b.plain_text.split_whitespace().count();
    let own = parse::shown(&plain).words * 100 >= want * parse::PAGE_HOLDS_TEXT_PERCENT;
    let page = if own { plain } else { parse::plain_to_html(&b.plain_text) };
    b.segments = vec![BodySegment {
        kind: SegmentKind::NewContent, text: b.plain_text.clone(), safe_html: page.clone(), confidence: 1.0, visibility: SegmentVisibility::Show,
        reasons: vec![reason.clone()], start: 0, end: b.plain_text.len().min(u32::MAX as usize) as u32,
    }];
    b.fold_from = None;
    b.layout = Some(BodyLayout::Written);
    b.layout_reasons = vec![reason];
    b.display_html = Some(page.clone());
    b.written_html = Some(page);
    if !own {
        b.pictures_blocked = 0;
        b.pixel_trackers = 0;
        b.trackers_blocked = 0;   // the text part holds no picture: no line says one was not loaded
    }
}

fn kind_of(w: &str) -> Option<SegmentKind> {
    Some(match w {
        "NEW_CONTENT" => SegmentKind::NewContent, "INLINE_QUOTE" => SegmentKind::InlineQuote, "REPLY_HISTORY" => SegmentKind::ReplyHistory,
        "FORWARDED_CONTENT" => SegmentKind::ForwardedContent, "SIGNATURE" => SegmentKind::Signature, "DISCLAIMER" => SegmentKind::Disclaimer,
        "UNKNOWN" => SegmentKind::Unknown, _ => return None,
    })
}

fn visibility_of(w: &str) -> Option<SegmentVisibility> {
    Some(match w { "show" => SegmentVisibility::Show, "collapse" => SegmentVisibility::Collapse, "hide" => SegmentVisibility::Hide, _ => return None })
}

/// body-wire: the engine's result as the store keeps it (table `message_prepared`), one JSON object (the engine crate has no JSON library
/// outside its tests: strings are written by `json_text`, read back by `Json::parse` below). Written at body fetch only.
pub fn prepared_json(p: &parse::Prepared) -> String {
    let list = |v: Vec<&str>| format!("[{}]", v.into_iter().map(json_text).collect::<Vec<_>>().join(","));
    let opt = |v: Option<&str>| v.map(json_text).unwrap_or_else(|| "null".to_string());
    let segments: Vec<String> = p.segments.iter().map(|s| format!(
        r#"{{"kind":{},"text":{},"safe_html":{},"confidence":{},"visibility":{},"reasons":{},"start":{},"end":{}}}"#,
        json_text(s.kind.as_str()), json_text(&s.text), json_text(&s.safe_html), if s.confidence.is_finite() { s.confidence } else { 0.0 },
        json_text(s.visibility.as_str()), list(s.reasons.iter().map(|r| r.as_str()).collect()), s.range.start, s.range.end,
    )).collect();
    format!(
        r#"{{"v":1,"segments":[{}],"fold_from":{},"layout":{},"layout_reasons":{},"display_html":{},"written_html":{},"warnings":{},"pictures_blocked":{},"pixel_trackers":{}}}"#,
        segments.join(","), p.fold_from.map_or("null".to_string(), |n| n.to_string()), json_text(p.layout.as_str()),
        list(p.layout_reasons.iter().map(|r| r.as_str()).collect()), json_text(&p.display_html), opt(p.written_html.as_deref()),
        list(p.warnings.iter().map(|w| w.as_str()).collect()), p.pictures_blocked, p.pixel_trackers,
    )
}

/// The JSON `prepared_json` writes, read back: null, booleans, numbers, strings (every escape), arrays, objects; nesting capped at 16.
#[derive(Debug, Clone, PartialEq)]
enum Json { Null, Bool(bool), Num(f64), Str(String), Arr(Vec<Json>), Obj(Vec<(String, Json)>) }

static JSON_NULL: Json = Json::Null;

impl Json {
    fn parse(s: &str) -> Option<Json> {
        let b = s.as_bytes();
        let mut i = 0;
        let v = Self::value(b, &mut i, 0)?;
        Self::ws(b, &mut i);
        (i == b.len()).then_some(v)
    }
    fn ws(b: &[u8], i: &mut usize) { while *i < b.len() && matches!(b[*i], b' ' | b'\n' | b'\r' | b'\t') { *i += 1; } }
    fn value(b: &[u8], i: &mut usize, depth: usize) -> Option<Json> {
        if depth > 16 { return None; }
        Self::ws(b, i);
        match *b.get(*i)? {
            b'n' => { if b[*i..].starts_with(b"null") { *i += 4; Some(Json::Null) } else { None } }
            b't' => { if b[*i..].starts_with(b"true") { *i += 4; Some(Json::Bool(true)) } else { None } }
            b'f' => { if b[*i..].starts_with(b"false") { *i += 5; Some(Json::Bool(false)) } else { None } }
            b'"' => Self::string(b, i).map(Json::Str),
            b'[' => {
                *i += 1;
                let mut out = Vec::new();
                Self::ws(b, i);
                if b.get(*i) == Some(&b']') { *i += 1; return Some(Json::Arr(out)); }
                loop {
                    out.push(Self::value(b, i, depth + 1)?);
                    Self::ws(b, i);
                    match *b.get(*i)? { b',' => *i += 1, b']' => { *i += 1; return Some(Json::Arr(out)); } _ => return None }
                }
            }
            b'{' => {
                *i += 1;
                let mut out = Vec::new();
                Self::ws(b, i);
                if b.get(*i) == Some(&b'}') { *i += 1; return Some(Json::Obj(out)); }
                loop {
                    Self::ws(b, i);
                    if b.get(*i) != Some(&b'"') { return None; }
                    let k = Self::string(b, i)?;
                    Self::ws(b, i);
                    if b.get(*i) != Some(&b':') { return None; }
                    *i += 1;
                    out.push((k, Self::value(b, i, depth + 1)?));
                    Self::ws(b, i);
                    match *b.get(*i)? { b',' => *i += 1, b'}' => { *i += 1; return Some(Json::Obj(out)); } _ => return None }
                }
            }
            _ => {
                let start = *i;
                while *i < b.len() && matches!(b[*i], b'-' | b'+' | b'.' | b'e' | b'E' | b'0'..=b'9') { *i += 1; }
                std::str::from_utf8(&b[start..*i]).ok()?.parse::<f64>().ok().filter(|n| n.is_finite()).map(Json::Num)
            }
        }
    }
    fn hex4(b: &[u8], at: usize) -> Option<u32> { u32::from_str_radix(std::str::from_utf8(b.get(at..at + 4)?).ok()?, 16).ok() }
    fn string(b: &[u8], i: &mut usize) -> Option<String> {
        *i += 1;
        let mut out: Vec<u8> = Vec::new();
        loop {
            let c = *b.get(*i)?;
            *i += 1;
            match c {
                b'"' => return String::from_utf8(out).ok(),
                b'\\' => {
                    let e = *b.get(*i)?;
                    *i += 1;
                    let ch: char = match e {
                        b'"' => '"', b'\\' => '\\', b'/' => '/', b'b' => '\u{8}', b'f' => '\u{c}', b'n' => '\n', b'r' => '\r', b't' => '\t',
                        b'u' => {
                            let hi = Self::hex4(b, *i)?;
                            *i += 4;
                            if (0xD800..0xDC00).contains(&hi) && b.get(*i) == Some(&b'\\') && b.get(*i + 1) == Some(&b'u') {
                                let lo = Self::hex4(b, *i + 2)?;
                                *i += 6;
                                char::from_u32(0x10000 + ((hi - 0xD800) << 10) + (lo.checked_sub(0xDC00)? & 0x3FF))?
                            } else { char::from_u32(hi)? }
                        }
                        _ => return None,
                    };
                    let mut buf = [0u8; 4];
                    out.extend_from_slice(ch.encode_utf8(&mut buf).as_bytes());
                }
                c => out.push(c),
            }
        }
    }
    fn get(&self, key: &str) -> &Json { match self { Json::Obj(kv) => kv.iter().find(|(k, _)| k == key).map_or(&JSON_NULL, |(_, v)| v), _ => &JSON_NULL } }
    fn str(&self) -> Option<&str> { match self { Json::Str(s) => Some(s), _ => None } }
    fn num(&self) -> Option<f64> { match self { Json::Num(n) => Some(*n), _ => None } }
    fn index(&self) -> Option<u32> { self.num().filter(|n| *n >= 0.0 && n.fract() == 0.0 && *n <= u32::MAX as f64).map(|n| n as u32) }
    fn arr(&self) -> Option<&[Json]> { match self { Json::Arr(a) => Some(a), _ => None } }
}

/// Fills the delta-26 fields from the kept JSON. Anything unreadable leaves them empty (a segment with an unknown kind is skipped whole).
fn apply_prepared(b: &mut MessageBody, json: &str) {
    let Some(v) = Json::parse(json) else { return };
    let words = |x: &Json| -> Vec<String> { x.arr().map(|a| a.iter().filter_map(|w| w.str().map(str::to_string)).collect()).unwrap_or_default() };
    let text = |x: &Json| x.str().map(str::to_string);
    // owner's picture 210 (13 Nov 2026): a run the time limit or a failure stopped kept escaped text, while the stored page of the same
    // mail (the sanitiser's) was whole. With a stored page such a record is not served: the reader draws the page by its own layout rule.
    let stopped = [parse::Warning::TimeLimit.as_str(), parse::Warning::Failure.as_str()];
    if b.safe_html.is_some() && words(v.get("warnings")).iter().any(|w| stopped.contains(&w.as_str())) { return; }
    let layout = match v.get("layout").str() { Some("designed") => Some(BodyLayout::Designed), Some("written") => Some(BodyLayout::Written), _ => None };
    if layout.is_none() { return; }
    b.segments = v.get("segments").arr().map(|a| a.iter().filter_map(|s| Some(BodySegment {
        kind: kind_of(s.get("kind").str()?)?, text: text(s.get("text"))?, safe_html: text(s.get("safe_html"))?,
        confidence: s.get("confidence").num()? as f32, visibility: visibility_of(s.get("visibility").str()?)?, reasons: words(s.get("reasons")),
        start: s.get("start").index()?, end: s.get("end").index()?,
    })).collect()).unwrap_or_default();
    b.fold_from = v.get("fold_from").index().filter(|n| (*n as usize) < b.segments.len());
    // owner's picture 212 (13 Nov 2026): a record kept before the parser's rule folds a forward behind a line of dashes. Such a fold is not
    // served: with nothing above it that says something, the reader shows the whole mail.
    if b.fold_from.is_some_and(|f| !b.segments[..f as usize].iter().any(|s| s.visibility == SegmentVisibility::Show && parse::says_something(&s.text))) { b.fold_from = None; }
    b.layout = layout;
    b.layout_reasons = words(v.get("layout_reasons"));
    b.display_html = text(v.get("display_html"));
    b.written_html = text(v.get("written_html"));
    b.body_warnings = words(v.get("warnings"));
    b.pictures_blocked = v.get("pictures_blocked").index().unwrap_or(0);
    b.pixel_trackers = v.get("pixel_trackers").index().unwrap_or(0);
}

/// Parses a raw message fetched from the server: the plain text, the SANITISED html, the trackers it blocked.
pub fn parse_raw(raw: &[u8]) -> Result<(String, Option<String>, u32), String> {
    parse_raw_with_preview(raw).map(|(text, html, trackers, _)| (text, html, trackers))
}

/// body-r1: `parse_raw` plus the row preview made from the same parse (the parse crate's own: the first PREVIEW_CHARS of the
/// visible text; the text itself when the whole of it is quoted), which the store writes over the preview the sync kept.
pub fn parse_raw_with_preview(raw: &[u8]) -> Result<(String, Option<String>, u32, String), String> {
    let a = parse::analyse(raw).map_err(|e| format!("{e:?}"))?;
    let preview = if a.preview.is_empty() { collapsed_preview(&a.message.text) } else { a.preview };
    Ok((a.message.text, a.safe_html.map(|h| h.html), a.trackers_blocked, preview))
}

pub(crate) fn collapsed_preview(text: &str) -> String {
    parse::preview_or_raw(text)
}

/// The preview of a stored body's text, as a download makes it (`parse_raw_with_preview`): the visible text's, else the whole text's.
pub(crate) fn preview_of_stored(text: &str) -> String {
    let visible = &text[..parse::text_quote_start(text).unwrap_or(text.len())];
    let p = parse::preview_of(visible);
    if p.is_empty() { collapsed_preview(text) } else { p }
}

/// body-r1: what the reader shows while a body downloads: the row's preview, unless it is one a sync before body-r1 stored raw
/// (a boundary line first, or a part header in it), which shows nothing rather than MIME. The download repairs the row.
pub fn shown_preview(preview: &str) -> String {
    let p = preview.trim_start();
    let raw = p.starts_with("--") || ["Content-Type:", "Content-Transfer-Encoding:"].iter().any(|h| p.contains(h));
    if raw { String::new() } else { preview.to_string() }
}

/// A string as a JSON value: quoted, with the characters JSON requires escaped.
pub(crate) fn json_text(v: &str) -> String {
    let mut out = String::with_capacity(v.len() + 2);
    out.push('"');
    for ch in v.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// The state paths the inbox strip reads (contracts delta 16, docs/CONTROL-SOCKET.md), answered from the store the way the
/// fixture core answers them: `inbox/tabs` = the strip left to right as JSON `[{id,kind,name,unread,waiting}]` (keys sorted,
/// as the fixture writes them), `view/<primary|all>/{name,unread,waiting,count,is_label}`. A view's name is the contract's
/// `inbox_view_name`, the key the strings table translates. Every other path has no value here (None).
pub fn view_state(store: &Store, path: &str) -> Option<String> {
    let p: Vec<&str> = path.split('/').collect();
    let figures = |tabs: &[TabCounts], id: &str| tabs.iter().find(|t| t.tab_id == id).map(|t| (t.unread, t.waiting)).unwrap_or((0, 0));
    match p.as_slice() {
        ["inbox", "tabs"] => {
            let counts = store.counts().ok()?;
            let mut tabs = inbox_tabs(store.labels_full().ok()?);
            // the owner's split-tab edits: a renamed tab shows its new name (the `split_tab` row's name once it was updated) and the label tabs
            // take the order of their `split_tab` rows (the views stay first; a tab without a row keeps its place)
            let rows = store.split_tab_rows().ok()?;
            for t in tabs.iter_mut().filter(|t| t.kind == InboxTabKind::Label) {
                if let Some(r) = rows.iter().find(|r| r.tab.id == t.id && r.updated_at_ms > 0) { t.name = r.tab.name.clone(); }
            }
            let slots: Vec<usize> = (0..tabs.len()).filter(|&i| tabs[i].kind == InboxTabKind::Label && rows.iter().any(|r| r.tab.id == tabs[i].id)).collect();
            let mut moved: Vec<InboxTab> = rows.iter().filter_map(|r| slots.iter().find(|&&i| tabs[i].id == r.tab.id).map(|&i| tabs[i].clone())).collect();
            if moved.len() == slots.len() { for i in slots { tabs[i] = moved.remove(0); } }
            let items: Vec<String> = tabs
                .into_iter()
                .map(|t| {
                    let (unread, waiting) = figures(&counts.tabs, &t.id);
                    let kind = match t.kind { InboxTabKind::View => "view", InboxTabKind::Label => "label" };
                    format!(r#"{{"id":{},"kind":"{kind}","name":{},"unread":{unread},"waiting":{waiting}}}"#, json_text(&t.id), json_text(&t.name))
                })
                .collect();
            Some(format!("[{}]", items.join(",")))
        }
        ["view", id, field] => {
            let view = inbox_view_from_id(id.to_string())?;
            match *field {
                "name" => Some(json_text(&inbox_view_name(view))),
                "unread" => Some(figures(&store.counts().ok()?.tabs, id).0.to_string()),
                "waiting" => Some(figures(&store.counts().ok()?.tabs, id).1.to_string()),
                "count" => store.tab_count(id).ok().map(|n| n.to_string()),
                "is_label" => Some("false".to_string()),
                _ => None,
            }
        }
        _ => None,
    }
}
