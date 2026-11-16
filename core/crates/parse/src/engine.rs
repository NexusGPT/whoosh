//! body-r4: the mail content engine. For one message: (A) its content as ordered, classified segments with `fold_from`, and (B) a display
//! document safe to show, `layout: designed | written`, and for a written mail a second document in the app's type. Pure library: no
//! network, no store. Never an error to the caller: any failure (a panic, the time limit) degrades that one message to its text, fully
//! visible, escaped, with a warning. Meant for store time, in the background (`prepare_in_background`), never when a mail opens.
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use mail_parser::{MessageParser, PartKind};

use crate::canon::{canonical, decode_text, unflow, CharsetDowngrade};
use crate::css::{inline_styles, Inlined, Skipped};
use crate::display::{self, Layout, LayoutFeatures, LayoutReason};
use crate::dom::Dom;
use crate::mime::ParseError;
use crate::segment::{self, band, classify, fold_from, merge, Block, Context, Earlier, Label, Reason, Segment, SegmentKind, Visibility};

/// `PARSE_PREPARE_TIME_LIMIT_MS`: the time one message may take in `prepare_in_background` before it degrades to escaped text.
pub const PREPARE_TIME_LIMIT_MS: u64 = 2_000;
/// `PARSE_PREPARE_RETRY_TIME_LIMIT_MS`: the time the ONE further run of a message gets after its first run reached `PREPARE_TIME_LIMIT_MS`.
/// The first limit is wall time: a loaded Mac, or an app held in the background, spends it without the engine having worked.
pub const PREPARE_RETRY_TIME_LIMIT_MS: u64 = 20_000;
/// `PARSE_HTML_NESTING_MAX`: an HTML body whose tags nest deeper than this (estimated by a linear scan before any parser runs: the HTML
/// tree builder's cost grows with the square of the depth) is shown as escaped text with a warning.
pub const HTML_NESTING_MAX: usize = 2_000;
/// `PARSE_BODY_BYTES_MAX`: a rendering (HTML or plain) larger than this is shown as escaped text with a warning, so the engine's memory
/// is bounded by its input (the DOM, the blocks and the segments each grow linearly with it).
pub const BODY_BYTES_MAX: usize = 8 * 1024 * 1024;
/// The worker's stack (html5ever and ammonia recurse a little on what the depth cap leaves).
const WORKER_STACK_BYTES: usize = 64 * 1024 * 1024;

/// Every warning the engine can give (a closed list).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Warning {
    CharsetUnknownReadTolerantly,
    CharsetUtf8ReadAsWindows1252,
    InliningSkipped(Skipped),
    NothingVisible,
    TimeLimit,
    Failure,
    NotAMessage,
    TooDeep,
    TooLarge,
}

impl Warning {
    pub fn as_str(self) -> &'static str {
        match self {
            Warning::CharsetUnknownReadTolerantly => "unknown charset, read as UTF-8 with replacements",
            Warning::CharsetUtf8ReadAsWindows1252 => "declared UTF-8 but over 2 % replacement characters: read as windows-1252",
            Warning::InliningSkipped(s) => s.as_str(),
            Warning::NothingVisible => "nothing would have been visible: the whole text is shown as one unknown segment",
            Warning::TimeLimit => "time limit reached: shown as escaped text",
            Warning::Failure => "the engine failed on this message: shown as escaped text",
            Warning::NotAMessage => "these bytes are not an email message",
            Warning::TooDeep => "HTML nested deeper than the cap: shown as escaped text",
            Warning::TooLarge => "body larger than the cap: shown as escaped text",
        }
    }
}

/// One message's renderings, as the engine reads them.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Parts {
    pub subject: String,
    /// The sender's display name (a signature signal).
    pub sender_name: Option<String>,
    /// The plain rendering, decoded (format=flowed already unfolded).
    pub text: Option<String>,
    /// The HTML rendering, decoded, untouched.
    pub html: Option<String>,
    /// (content id, IMAP part number) of the message's parts, for `cid:` images.
    pub parts: Vec<(String, String)>,
    pub warnings: Vec<Warning>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Prepared {
    /// The text every segment range points into.
    pub canonical: String,
    pub segments: Vec<Segment>,
    /// F2: the first segment of the fold ("three dots"), or None.
    pub fold_from: Option<usize>,
    /// (B) the whole message, cleaned: safe to show.
    pub display_html: String,
    /// B3: for a written mail, the display document without the sender's fonts, sizes, colours and backgrounds; None for a designed one.
    pub written_html: Option<String>,
    pub layout: Layout,
    pub layout_reasons: Vec<LayoutReason>,
    pub layout_features: LayoutFeatures,
    /// The style inliner's result: how many rules, or which cap stopped it (then nothing was inlined).
    pub inlining: Inlined,
    pub remote_images_blocked: u32,
    /// The two kinds of `remote_images_blocked`: pictures (address kept in `data-whoosh-src` when http or https) and trackers (an image
    /// that says it is invisible: `display::is_tracker`).
    pub pictures_blocked: u32,
    pub pixel_trackers: u32,
    /// `cid:` images resolved to a part of this message: (content id, IMAP part).
    pub cid_images: Vec<(String, String)>,
    pub warnings: Vec<Warning>,
    /// The renderings as they came in, untouched (the original is retained).
    pub original_text: Option<String>,
    pub original_html: Option<String>,
    /// True when a failure degraded this message to escaped text.
    pub degraded: bool,
}

/// How the engine runs: a deadline checked between stages; `fault` makes it fail on purpose (the failure path's own test).
#[derive(Debug, Clone, Default)]
pub struct Options {
    pub deadline: Option<Instant>,
    #[doc(hidden)]
    pub fault: bool,
}

#[derive(Debug)]
enum Stop { TimeLimit, TooDeep, TooLarge }

/// The deepest tag nesting of an HTML text, by a linear scan of its tags (void and self-closed tags do not nest).
pub fn nesting_estimate(html: &str) -> usize {
    const VOID: &[&str] = &["area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "source", "track", "wbr", "param", "o:p"];
    let b = html.as_bytes();
    let (mut depth, mut max, mut i) = (0usize, 0usize, 0usize);
    while i < b.len() {
        if b[i] != b'<' { i += 1; continue; }
        let close = b.get(i + 1) == Some(&b'/');
        let s = if close { i + 2 } else { i + 1 };
        let mut e = s;
        while e < b.len() && (b[e].is_ascii_alphanumeric() || b[e] == b':' || b[e] == b'-') { e += 1; }
        if e == s { i += 1; continue; }
        let name = html[s..e].to_ascii_lowercase();
        let end = html[e..].find('>').map_or(b.len(), |x| e + x);
        let self_closed = end > 0 && b.get(end.saturating_sub(1)) == Some(&b'/');
        if close { depth = depth.saturating_sub(1); } else if !VOID.contains(&name.as_str()) && !self_closed { depth += 1; max = max.max(depth); }
        i = end;
    }
    max
}

fn check(o: &Options) -> Result<(), Stop> { if o.deadline.is_some_and(|d| Instant::now() >= d) { Err(Stop::TimeLimit) } else { Ok(()) } }

/// The renderings of one raw message, by the charset chain (declared, tolerant, windows-1252).
pub fn parts_of(raw: &[u8]) -> Result<Parts, ParseError> {
    let msg = MessageParser::new().parse(raw).ok_or(ParseError::NotAMessage)?;
    let mut out = Parts { subject: msg.subject().unwrap_or("").to_string(), ..Default::default() };
    out.sender_name = msg.from().and_then(|l| l.mailboxes().next().and_then(|m| m.name().map(str::to_string)));
    let mut read = |p: &mail_parser::MessagePart<'_>| -> String {
        let ct = p.content_type();
        let (s, w) = decode_text(&p.decoded(), ct.as_ref().and_then(|c| c.attribute("charset")));
        for x in w {
            out.warnings.push(match x { CharsetDowngrade::UnknownCharsetReadTolerantly => Warning::CharsetUnknownReadTolerantly, CharsetDowngrade::Utf8ReadAsWindows1252 => Warning::CharsetUtf8ReadAsWindows1252 });
        }
        let flowed = ct.as_ref().and_then(|c| c.attribute("format")).is_some_and(|f| f.eq_ignore_ascii_case("flowed"));
        let delsp = ct.as_ref().and_then(|c| c.attribute("delsp")).is_some_and(|f| f.eq_ignore_ascii_case("yes"));
        if flowed && matches!(p.kind(), PartKind::Text) { unflow(&s, delsp) } else { s }
    };
    let text = msg.text_body().find(|p| matches!(p.kind(), PartKind::Text)).map(|p| read(&p));
    let html = msg.html_body().find(|p| matches!(p.kind(), PartKind::Html)).map(|p| read(&p));
    // owner's picture 216 (14 Nov 2026): the cut tags of the page ended, a text part of page code read as its words
    let (text, html) = crate::pagecode::right_parts(text, html);
    out.text = text;
    out.html = html;
    out.parts = crate::mime::attachments_of(raw).unwrap_or_default().into_iter().filter_map(|a| a.content_id.map(|c| (c, a.part))).collect();
    Ok(out)
}

/// The engine on a raw message (in the caller's thread; see `prepare_in_background`).
pub fn prepare_message(raw: &[u8], earlier: &[&str], opts: &Options) -> Prepared {
    match catch_unwind(AssertUnwindSafe(|| parts_of(raw))) {
        Ok(Ok(parts)) => prepare(&parts, earlier, opts),
        Ok(Err(_)) => degraded(&String::from_utf8_lossy(raw), None, None, vec![Warning::NotAMessage]),
        Err(_) => degraded(&String::from_utf8_lossy(raw), None, None, vec![Warning::Failure]),
    }
}

/// The engine on one message's renderings. `earlier`: the plain text of the thread's earlier messages, nearest ancestor first.
pub fn prepare(parts: &Parts, earlier: &[&str], opts: &Options) -> Prepared {
    match catch_unwind(AssertUnwindSafe(|| run(parts, earlier, opts))) {
        Ok(Ok(p)) => p,
        Ok(Err(Stop::TimeLimit)) => degraded(&fallback_text(parts), parts.text.clone(), parts.html.clone(), with(&parts.warnings, Warning::TimeLimit)),
        Ok(Err(Stop::TooLarge)) => degraded(&fallback_text(parts), parts.text.clone(), parts.html.clone(), with(&parts.warnings, Warning::TooLarge)),
        Ok(Err(Stop::TooDeep)) => degraded(&fallback_text(parts), parts.text.clone(), parts.html.clone(), with(&parts.warnings, Warning::TooDeep)),
        Err(_) => degraded(&fallback_text(parts), parts.text.clone(), parts.html.clone(), with(&parts.warnings, Warning::Failure)),
    }
}

/// The engine on a worker thread, under a time limit: what store time calls. Past the limit the message is escaped text with a warning
/// (the worker stops at its next deadline check). The caller's thread only waits, then escapes the text the worker sent first (or, when
/// the worker has not got that far, the raw body after its header).
pub fn prepare_in_background(raw: Vec<u8>, earlier: Vec<String>, limit: Duration) -> Prepared {
    prepare_in_background_stalled(raw, earlier, limit, Duration::ZERO)
}

/// `prepare_in_background` with the worker held `stall` before it starts (the test of the caller's own time limit; zero in use).
#[doc(hidden)]
pub fn prepare_in_background_stalled(raw: Vec<u8>, earlier: Vec<String>, limit: Duration, stall: Duration) -> Prepared {
    enum Msg { Text(String), Done(Box<Prepared>) }
    let raw = std::sync::Arc::new(raw);
    let (tx, rx) = mpsc::channel::<Msg>();
    let deadline = Instant::now() + limit;
    let r2 = raw.clone();
    let spawned = std::thread::Builder::new().name("parse-prepare".into()).stack_size(WORKER_STACK_BYTES).spawn(move || {
        if !stall.is_zero() { std::thread::sleep(stall); }
        let e: Vec<&str> = earlier.iter().map(String::as_str).collect();
        let opts = Options { deadline: Some(deadline), fault: false };
        match catch_unwind(AssertUnwindSafe(|| parts_of(&r2))) {
            Ok(Ok(parts)) => {
                let _ = tx.send(Msg::Text(fallback_text(&parts)));
                let _ = tx.send(Msg::Done(Box::new(prepare(&parts, &e, &opts))));
            }
            _ => { let _ = tx.send(Msg::Done(Box::new(prepare_message(&r2, &e, &opts)))); }
        }
    });
    if spawned.is_err() { return degraded(&raw_body_text(&raw), None, None, vec![Warning::Failure]); }
    let end = deadline + Duration::from_millis(50);
    let mut text: Option<String> = None;
    loop {
        match rx.recv_timeout(end.saturating_duration_since(Instant::now())) {
            Ok(Msg::Text(t)) => text = Some(t),
            Ok(Msg::Done(p)) => return *p,
            Err(_) => return degraded(&text.unwrap_or_else(|| raw_body_text(&raw)), None, None, vec![Warning::TimeLimit]),
        }
    }
}

fn with(ws: &[Warning], w: Warning) -> Vec<Warning> { let mut v = ws.to_vec(); v.push(w); v }

fn strip_tags(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut inside = false;
    for c in html.chars() { match c { '<' => inside = true, '>' if inside => { inside = false; if !out.ends_with(char::is_whitespace) { out.push(' ') } } c if !inside => out.push(c), _ => {} } }
    crate::entities::decode_entities(&out)
}

fn fallback_text(parts: &Parts) -> String {
    match (&parts.text, &parts.html) { (Some(t), _) => t.clone(), (None, Some(h)) => strip_tags(h), _ => String::new() }
}

/// The raw body after the header block, as text, tags stripped when it holds HTML elements (the time limit's fallback when the worker has not
/// even decoded the message: no parser runs on the caller's thread).
fn raw_body_text(raw: &[u8]) -> String {
    let at = raw.windows(4).position(|w| w == b"\r\n\r\n").map(|p| p + 4).or_else(|| raw.windows(2).position(|w| w == b"\n\n").map(|p| p + 2)).unwrap_or(0);
    let body = String::from_utf8_lossy(&raw[at..]);
    // tags are stripped only from what looks like HTML: a plain "<friend>" is the sender's text
    let low = body.to_ascii_lowercase();
    let html = ["<html", "<body", "<div", "<p>", "<p ", "<br", "<table", "<span"].iter().any(|t| low.contains(t));
    if html { strip_tags(&body) } else { body.into_owned() }
}

/// Fully visible, escaped, one UNKNOWN segment, the warning said.
fn degraded(text: &str, original_text: Option<String>, original_html: Option<String>, warnings: Vec<Warning>) -> Prepared {
    let canon = canonical(text);
    let html = display::escaped_plain(&canon);
    let segments = if canon.is_empty() { Vec::new() } else {
        vec![Segment { kind: SegmentKind::Unknown, text: canon.clone(), html: html.clone(), safe_html: html.clone(), confidence: 0.0, visibility: Visibility::Show, reasons: vec![Reason::Failure], range: 0..canon.len() }]
    };
    Prepared {
        canonical: canon, segments, fold_from: None, display_html: html.clone(), written_html: Some(html), layout: Layout::Written,
        layout_reasons: vec![LayoutReason::NoDesignSignal], layout_features: LayoutFeatures::default(), inlining: Inlined::default(),
        remote_images_blocked: 0, pictures_blocked: 0, pixel_trackers: 0, cid_images: Vec::new(), warnings, original_text, original_html, degraded: true,
    }
}

fn norm(s: &str) -> String { s.to_lowercase().split(|c: char| !c.is_alphanumeric()).filter(|t| !t.is_empty()).collect::<Vec<_>>().join(" ") }

/// Stage 1's cross-check: where the plain part shows as the sender's own text what the HTML puts away, confidence only goes DOWN.
fn cross_check(labels: &mut [Label], blocks: &[Block], text: &str, cx: &Context<'_>) {
    let canon = canonical(text);
    let pb = segment::plain_blocks(&canon);
    let pl = classify(&pb, cx);
    let mut own = std::collections::HashSet::new();
    let mut away = std::collections::HashSet::new();
    for (b, l) in pb.iter().zip(&pl) {
        let k = norm(&b.content);
        if k.split(' ').count() < 3 { continue; }
        if l.kind.authored() { own.insert(k); } else { away.insert(k); }
    }
    for (b, l) in blocks.iter().zip(labels.iter_mut()) {
        if l.kind.authored() { continue; }
        let k = norm(&b.content);
        if own.contains(&k) && !away.contains(&k) && !l.reasons.contains(&Reason::PartsDisagree) {
            l.confidence *= segment::PARTS_DISAGREE_FACTOR;
            l.reasons.push(Reason::PartsDisagree);
        }
    }
}

fn run(parts: &Parts, earlier: &[&str], opts: &Options) -> Result<Prepared, Stop> {
    if opts.fault { panic!("the failure path's test asked for a failure"); }
    check(opts)?;
    let earlier: Vec<Earlier> = earlier.iter().map(|t| Earlier::new(t)).collect();
    let cx = Context { subject: &parts.subject, sender: parts.sender_name.as_deref(), earlier: &earlier };
    let mut warnings = parts.warnings.clone();
    let mut p = Prepared {
        canonical: String::new(), segments: Vec::new(), fold_from: None, display_html: String::new(), written_html: None, layout: Layout::Written,
        layout_reasons: vec![LayoutReason::NoDesignSignal], layout_features: LayoutFeatures::default(), inlining: Inlined::default(),
        remote_images_blocked: 0, pictures_blocked: 0, pixel_trackers: 0, cid_images: Vec::new(), warnings: Vec::new(), original_text: parts.text.clone(), original_html: parts.html.clone(), degraded: false,
    };
    let mut segments: Vec<Segment> = Vec::new();
    if let Some(html) = parts.html.as_deref().filter(|h| !h.trim().is_empty()) {
        if html.len() > BODY_BYTES_MAX { return Err(Stop::TooLarge); }
        if nesting_estimate(html) > HTML_NESTING_MAX { return Err(Stop::TooDeep); }
        let mut dom = Dom::parse(html);
        check(opts)?;
        let style_rules = display::count_style_rules(&dom);
        p.inlining = inline_styles(&mut dom, opts.deadline);
        match p.inlining.skipped { Some(Skipped::Timeout) => return Err(Stop::TimeLimit), Some(s) => warnings.push(Warning::InliningSkipped(s)), None => {} }
        check(opts)?;
        let ord = segment::order(&dom);
        let roles = if dom.nodes.len() <= crate::css::ELEMENTS_MAX * 2 { segment::roles(&dom) } else { vec![None; dom.nodes.len()] };
        let (canon, blocks) = segment::html_blocks(&dom, &roles, &ord);
        check(opts)?;
        let mut labels = classify(&blocks, &cx);
        if let Some(t) = &parts.text { cross_check(&mut labels, &blocks, t, &cx); }
        check(opts)?;
        let merged = merge(&canon, &blocks, &labels);
        for (i, (range, l, br)) in merged.iter().enumerate() {
            let a = if i == 0 { 0 } else { blocks[br.start].first_pos };
            let b = merged.get(i + 1).map_or(usize::MAX, |(_, _, nb)| blocks[nb.start].first_pos.saturating_sub(1));
            let mut stats = display::CleanStats::default();
            let ours = display::Cleaner { dom: &dom, parts: &parts.parts }.write(crate::dom::ROOT, Some((a, b)), Some(&ord), &mut stats);
            let safe = display::allow_list(&ours);
            segments.push(Segment { kind: l.kind, text: canon[range.clone()].to_string(), html: ours, safe_html: safe, confidence: l.confidence, visibility: Visibility::Show, reasons: l.reasons.clone(), range: range.clone() });
            check(opts)?;
        }
        let (clean, stats) = display::clean_dom(&dom, &parts.parts, None, None);
        p.remote_images_blocked = stats.remote_images_blocked;
        p.pictures_blocked = stats.pictures_blocked;
        p.pixel_trackers = stats.trackers_blocked;
        p.cid_images = stats.cid_images;
        p.layout_features = display::layout_features(&clean, stats.remote_images_blocked, style_rules);
        let (layout, reasons) = display::decide_layout(&p.layout_features);
        p.layout = layout;
        p.layout_reasons = reasons;
        if layout == Layout::Written { p.written_html = Some(display::written_document(&clean)); }
        p.display_html = clean;
        p.canonical = canon;
    } else {
        if parts.text.as_deref().is_some_and(|t| t.len() > BODY_BYTES_MAX) { return Err(Stop::TooLarge); }
        let canon = canonical(parts.text.as_deref().unwrap_or(""));
        let blocks = segment::plain_blocks(&canon);
        let labels = classify(&blocks, &cx);
        for (range, l, _) in merge(&canon, &blocks, &labels) {
            let t = &canon[range.clone()];
            let html = display::plain_to_html(t);
            let safe = display::allow_list(&html);
            segments.push(Segment { kind: l.kind, text: t.to_string(), html, safe_html: safe, confidence: l.confidence, visibility: Visibility::Show, reasons: l.reasons.clone(), range });
        }
        p.display_html = display::allow_list(&display::plain_to_html(&canon));
        p.written_html = Some(p.display_html.clone());
        p.canonical = canon;
    }
    check(opts)?;
    for s in &mut segments { s.visibility = band(s.kind, s.confidence, &s.reasons); }
    // never empty: a body with text keeps at least one visible segment
    if !p.canonical.trim().is_empty() && !segments.iter().any(|s| s.visibility == Visibility::Show) {
        warnings.push(Warning::NothingVisible);
        let html = if p.display_html.is_empty() { display::escaped_plain(&p.canonical) } else { p.display_html.clone() };
        segments = vec![Segment { kind: SegmentKind::Unknown, text: p.canonical.clone(), html: html.clone(), safe_html: html, confidence: 0.0, visibility: Visibility::Show, reasons: vec![Reason::NothingVisible], range: 0..p.canonical.len() }];
    }
    p.fold_from = fold_from(&segments);
    p.segments = segments;
    p.warnings = warnings;
    Ok(p)
}
