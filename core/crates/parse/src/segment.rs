//! body-r4 stages 3 to 6: blocks with features, detectors that return evidence (patterns in `src/patterns/*.txt`), the thread's earlier
//! messages, and the rules that turn block sequences into classified segments. THE RULE: hiding a sentence the sender wrote is at least ten
//! times worse than showing old history, so content is hidden only on a STRUCTURAL anchor (an attribution line, a client's container, a
//! separator or header block); quote marks alone never hide; a thread match without quote markup collapses; what is ambiguous stays visible.
use std::collections::HashSet;
use std::ops::Range;
use std::sync::OnceLock;

use regex::Regex;

use crate::canon::{canonical, quote_depth};
use crate::css::Matcher;
use crate::dom::{Dom, Id, Kind, ROOT};

// ---------------------------------------------------------------- ledger constants (rows for docs/CONSTANTS.md, source: the round-4 brief)

/// `PARSE_HIDE_MIN_CONFIDENCE`: at or above, with a structural anchor, a segment is hidden.
pub const HIDE_MIN_CONFIDENCE: f32 = 0.95;
/// `PARSE_COLLAPSE_MIN_CONFIDENCE`: at or above (and below the hide band, or without an anchor), a segment is collapsed; below, shown.
pub const COLLAPSE_MIN_CONFIDENCE: f32 = 0.75;
/// `PARSE_HEADER_BLOCK_MIN_LINES`: consecutive header lines (with From and a date-like field) that make an embedded header block strong.
pub const HEADER_BLOCK_MIN_LINES: usize = 3;
/// `PARSE_SIGNATURE_MAX_LINES`: how many non-blank lines at the end of the sender's text a signature may span.
pub const SIGNATURE_MAX_LINES: usize = 10;
/// `PARSE_FOOTER_MAX_BLOCKS`: how many non-blank blocks at the end of the sender's text a footer may span.
pub const FOOTER_MAX_BLOCKS: usize = 8;
/// `PARSE_BOTTOM_POST_MIN_LINES`: a single quoted run on top of the answer, with no attribution, is a bottom post from this many lines.
pub const BOTTOM_POST_MIN_LINES: usize = 4;
/// `PARSE_THREAD_MATCH_MIN_CHARS`: a trailing run of blocks matched in an earlier message is history only from this many characters.
pub const THREAD_MATCH_MIN_CHARS: usize = 40;
/// `PARSE_THREAD_MATCH_MIN_WORDS`: a block shorter than this is never matched against the thread ("Thanks" is in every message).
pub const THREAD_MATCH_MIN_WORDS: usize = 3;
/// `PARSE_SHINGLE_WORDS` and `PARSE_SHINGLE_MIN_SHARE`: word shingles, and the share of a block's shingles found in an earlier message.
pub const SHINGLE_WORDS: usize = 3;
pub const SHINGLE_MIN_SHARE: f32 = 0.8;
/// `PARSE_ALIGN_MIN_SHARE`, `PARSE_ALIGN_CANDIDATE_SHARE`, `PARSE_ALIGN_MAX_TOKENS`: the bounded alignment runs on blocks whose shingle share
/// is at least the candidate share, over at most this many tokens; a block whose aligned share reaches the minimum is matched.
pub const ALIGN_MIN_SHARE: f32 = 0.85;
pub const ALIGN_CANDIDATE_SHARE: f32 = 0.4;
pub const ALIGN_MAX_TOKENS: usize = 400;
/// `PARSE_PARTS_DISAGREE_FACTOR`: where the plain part shows as the sender's own text what the HTML puts away, confidence is multiplied by this.
pub const PARTS_DISAGREE_FACTOR: f32 = 0.8;

// ---------------------------------------------------------------- the closed lists

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SegmentKind { NewContent, InlineQuote, ReplyHistory, ForwardedContent, Signature, Disclaimer, Unknown }

impl SegmentKind {
    pub fn as_str(self) -> &'static str {
        match self {
            SegmentKind::NewContent => "NEW_CONTENT", SegmentKind::InlineQuote => "INLINE_QUOTE", SegmentKind::ReplyHistory => "REPLY_HISTORY",
            SegmentKind::ForwardedContent => "FORWARDED_CONTENT", SegmentKind::Signature => "SIGNATURE", SegmentKind::Disclaimer => "DISCLAIMER",
            SegmentKind::Unknown => "UNKNOWN",
        }
    }
    /// The sender's own words or the context they answer: never folded, never put away.
    pub fn authored(self) -> bool { matches!(self, SegmentKind::NewContent | SegmentKind::InlineQuote | SegmentKind::Unknown) }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Visibility { Show, Collapse, Hide }

impl Visibility {
    pub fn as_str(self) -> &'static str { match self { Visibility::Show => "show", Visibility::Collapse => "collapse", Visibility::Hide => "hide" } }
}

/// Why a segment is what it is. A heuristic that cannot name one of these may not change visibility.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Reason {
    AttributionLine, OriginalMessageRule, UnderscoreRule, HeaderBlock, ClientQuote, ClientSeparator, ClientAttribution,
    Blockquote, QuoteMarks, ThreadExact, ThreadShingles, ThreadAligned, BottomPost, AuthoredAfter,
    SignatureClosing, SignatureDelimiter, SignatureContact, SignatureSenderName, SignatureContainer,
    LegalNotice, UnsubscribeFooter, ForwardSubject, ForwardHeader, ForwardWithoutComment, PartsDisagree,
    NoHistorySignal, NothingVisible, Failure,
}

impl Reason {
    pub fn as_str(self) -> &'static str {
        match self {
            Reason::AttributionLine => "attribution_line", Reason::OriginalMessageRule => "original_message_rule", Reason::UnderscoreRule => "underscore_rule",
            Reason::HeaderBlock => "header_block", Reason::ClientQuote => "client_quote", Reason::ClientSeparator => "client_separator",
            Reason::ClientAttribution => "client_attribution", Reason::Blockquote => "blockquote", Reason::QuoteMarks => "quote_marks",
            Reason::ThreadExact => "thread_exact", Reason::ThreadShingles => "thread_shingles", Reason::ThreadAligned => "thread_aligned",
            Reason::BottomPost => "bottom_post", Reason::AuthoredAfter => "authored_after", Reason::SignatureClosing => "signature_closing",
            Reason::SignatureDelimiter => "signature_delimiter", Reason::SignatureContact => "signature_contact",
            Reason::SignatureSenderName => "signature_sender_name", Reason::SignatureContainer => "signature_container",
            Reason::LegalNotice => "legal_notice", Reason::UnsubscribeFooter => "unsubscribe_footer", Reason::ForwardSubject => "forward_subject",
            Reason::ForwardHeader => "forward_header", Reason::ForwardWithoutComment => "forward_without_comment", Reason::PartsDisagree => "parts_disagree",
            Reason::NoHistorySignal => "no_history_signal", Reason::NothingVisible => "nothing_visible", Reason::Failure => "failure",
        }
    }
    /// A STRUCTURAL anchor: the only reasons that may hide.
    pub fn is_anchor(self) -> bool {
        matches!(self, Reason::AttributionLine | Reason::OriginalMessageRule | Reason::UnderscoreRule | Reason::HeaderBlock
            | Reason::ClientQuote | Reason::ClientSeparator | Reason::ClientAttribution)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Segment {
    pub kind: SegmentKind,
    /// Exactly `canonical[range]`.
    pub text: String,
    /// The segment's own markup, UNTRUSTED (the source's elements, or the plain text made HTML). Show `safe_html`.
    pub html: String,
    /// `html` cleaned by the display builder: safe to show.
    pub safe_html: String,
    pub confidence: f32,
    pub visibility: Visibility,
    pub reasons: Vec<Reason>,
    /// Byte range in the canonical text.
    pub range: Range<usize>,
}

// ---------------------------------------------------------------- patterns (data files)

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field { From, Date, To, Cc, Subject }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role { Quote, Separator, Attribution, New, Signature }

pub(crate) struct Patterns {
    attribution: Vec<Regex>,
    original: Vec<Regex>,
    forward: Vec<Regex>,
    underscore: Vec<Regex>,
    headers: Vec<(Field, Regex)>,
    pub(crate) containers: Vec<(String, Role, Matcher)>,
    closing: Vec<Regex>,
    delimiter: Vec<Regex>,
    contact: Vec<Regex>,
    legal: Vec<Regex>,
    pub(crate) unsubscribe: Vec<Regex>,
    forward_subject: Vec<Regex>,
    date: Regex,
    address: Regex,
}

fn rows(data: &'static str) -> impl Iterator<Item = Vec<&'static str>> {
    data.lines().filter(|l| !l.trim().is_empty() && !l.starts_with('#')).map(|l| l.split('\t').collect())
}

fn ci(p: &str) -> Regex { Regex::new(&format!("(?i){p}")).expect("a pattern of src/patterns") }

pub(crate) fn patterns() -> &'static Patterns {
    static P: OnceLock<Patterns> = OnceLock::new();
    P.get_or_init(|| {
        let mut p = Patterns {
            attribution: Vec::new(), original: Vec::new(), forward: Vec::new(), underscore: Vec::new(), headers: Vec::new(), containers: Vec::new(),
            closing: Vec::new(), delimiter: Vec::new(), contact: Vec::new(), legal: Vec::new(), unsubscribe: Vec::new(), forward_subject: Vec::new(),
            date: ci(r"\b\d{1,4}[./-]\d{1,2}[./-]\d{1,4}\b|\b\d{1,2}[:h.]\d{2}\b|\b(19|20)\d{2}\b|\b(jan|feb|mar|apr|may|jun|jul|aug|sep|oct|nov|dec|janv|févr|avr|mai|juin|juil|août|sept|déc|mär|okt|dez|ene|abr|ago|dic|gen|giu|lug|set|ott|fev|out|mrt|mei|sty|lut|kwi|maj|cze|lip|sie|wrz|paź|lis|gru)[a-zé]*\.?\s|\b(monday|tuesday|wednesday|thursday|friday|saturday|sunday|mon|tue|tues|wed|thu|thur|thurs|fri|sat|sun|lundi|mardi|mercredi|jeudi|vendredi|samedi|dimanche|montag|dienstag|mittwoch|donnerstag|freitag|samstag|sonntag|lunes|martes|miércoles|jueves|viernes|sábado|domingo|lunedì|martedì|mercoledì|giovedì|venerdì|sabato|domenica|segunda-feira|terça-feira|quarta-feira|quinta-feira|sexta-feira|maandag|dinsdag|woensdag|donderdag|vrijdag|zaterdag|zondag|poniedziałek|wtorek|środa|czwartek|piątek|sobota|niedziela|esmaspäev|teisipäev|kolmapäev|neljapäev|reede|laupäev|pühapäev|maanantaina?|tiistaina?|keskiviikkona?|torstaina?|perjantaina?|lauantaina?|sunnuntaina?|måndag|tisdag|onsdag|torsdag|fredag|lördag|söndag|понедельник|вторник|среда|четверг|пятница|суббота|воскресенье)\b"),
            address: ci(r"[a-z0-9._%+-]+@[a-z0-9.-]+\.[a-z]{2,}"),
        };
        for r in rows(include_str!("patterns/attribution.txt")) { p.attribution.push(ci(r[1])); }
        for r in rows(include_str!("patterns/separators.txt")) {
            let re = ci(r[1]);
            match r[0] { "original" => p.original.push(re), "forward" => p.forward.push(re), _ => p.underscore.push(re) }
        }
        for r in rows(include_str!("patterns/headers.txt")) {
            let f = match r[0] { "from" => Field::From, "date" => Field::Date, "to" => Field::To, "cc" => Field::Cc, _ => Field::Subject };
            p.headers.push((f, ci(&format!(r"^\*?{}\*?\s*:\s*\S", r[1]))));
        }
        for r in rows(include_str!("patterns/containers.txt")) {
            let role = match r[1] { "quote" => Role::Quote, "separator" => Role::Separator, "attribution" => Role::Attribution, "new" => Role::New, _ => Role::Signature };
            p.containers.push((r[0].to_string(), role, Matcher::new(r[2]).expect("a selector of containers.txt")));
        }
        for r in rows(include_str!("patterns/signature.txt")) {
            let re = ci(r[1]);
            match r[0] { "closing" => p.closing.push(re), "delimiter" => p.delimiter.push(re), _ => p.contact.push(re) }
        }
        for r in rows(include_str!("patterns/footer.txt")) {
            if r[0] == "legal" { p.legal.push(ci(r[1])) } else { p.unsubscribe.push(ci(r[1])) }
        }
        for r in rows(include_str!("patterns/forward.txt")) { p.forward_subject.push(ci(r[0])); }
        p
    })
}

fn any(res: &[Regex], s: &str) -> bool { res.iter().any(|r| r.is_match(s)) }

pub fn is_forward_subject(subject: &str) -> bool { any(&patterns().forward_subject, subject) }

// ---------------------------------------------------------------- blocks (stage 3)

#[derive(Debug, Clone, Default)]
pub struct Block {
    /// In the canonical text, the trailing line feed included.
    pub range: Range<usize>,
    /// The text without quote marks, trimmed.
    pub content: String,
    /// ">" marks (plain) or blockquote ancestors (HTML).
    pub depth: usize,
    pub blockquote: bool,
    pub role: Option<Role>,
    /// Inside or after a client separator element.
    pub after_separator: bool,
    pub tag: String,
    pub class: String,
    pub indent: usize,
    pub border: bool,
    pub has_date: bool,
    pub has_address: bool,
    pub header: Option<Field>,
    pub question: bool,
    pub rule: bool,
    /// HTML: the first node position (document order) of the block.
    pub first_pos: usize,
}

impl Block {
    fn blank(&self) -> bool { self.content.trim().is_empty() }
    fn quoted(&self) -> bool { self.depth > 0 || self.role == Some(Role::Quote) }
}

fn features(b: &mut Block) {
    let p = patterns();
    let c = b.content.trim();
    b.has_date = p.date.is_match(c);
    b.has_address = p.address.is_match(c);
    b.header = p.headers.iter().find(|(_, r)| r.is_match(c)).map(|(f, _)| *f);
    b.question = c.ends_with('?');
    b.rule = c.len() >= 2 && (c.chars().all(|ch| ch == '-') || c.chars().all(|ch| ch == '_') || c.chars().all(|ch| ch == '='));
}

/// Plain text: one block per line.
pub fn plain_blocks(canon: &str) -> Vec<Block> {
    let mut out = Vec::new();
    let mut off = 0;
    for line in canon.split_inclusive('\n') {
        let body = line.strip_suffix('\n').unwrap_or(line);
        let (depth, at) = quote_depth(body);
        let content = body[at..].trim().to_string();
        let indent = body.len() - body.trim_start().len();
        let mut b = Block { range: off..off + line.len(), content, depth, indent, tag: "line".into(), ..Default::default() };
        features(&mut b);
        out.push(b);
        off += line.len();
    }
    out
}

const BLOCK_TAGS: &[&str] = &[
    "address", "article", "aside", "blockquote", "body", "center", "dd", "details", "dialog", "div", "dl", "dt", "fieldset", "figcaption",
    "figure", "footer", "form", "h1", "h2", "h3", "h4", "h5", "h6", "header", "hr", "html", "li", "main", "nav", "ol", "p", "pre", "section",
    "summary", "table", "tbody", "thead", "tfoot", "tr", "td", "th", "ul", "br", "caption",
];
const SKIP_TAGS: &[&str] = &["head", "style", "script", "title", "template", "noscript", "object", "embed", "iframe", "frame", "frameset", "applet", "audio", "video", "canvas", "select", "textarea", "button"];

/// The positions of every node in document order and the end of each subtree (for the per-segment markup).
pub struct Order { pub pos: Vec<usize>, pub end: Vec<usize> }

pub fn order(dom: &Dom) -> Order {
    let all = dom.descendants(ROOT);
    let mut pos = vec![usize::MAX; dom.nodes.len()];
    for (i, id) in all.iter().enumerate() { pos[*id] = i; }
    let mut size = vec![1usize; dom.nodes.len()];
    for id in all.iter().rev() { if let Some(p) = dom.nodes[*id].parent { size[p] += size[*id]; } }
    let end = (0..dom.nodes.len()).map(|id| if pos[id] == usize::MAX { 0 } else { pos[id] + size[id] - 1 }).collect();
    Order { pos, end }
}

/// The role of each element by the client container table (`containers.txt`).
pub fn roles(dom: &Dom) -> Vec<Option<Role>> {
    let mut out = vec![None; dom.nodes.len()];
    let p = patterns();
    for id in dom.descendants(ROOT) {
        if !dom.is_element(id) { continue; }
        if let Some((_, r, _)) = p.containers.iter().find(|(_, _, m)| m.matches(dom, id)) { out[id] = Some(*r); }
    }
    out
}

struct Frame { bq: usize, role: Option<Role>, pre: bool, tag: String, class: String, border: bool }

/// HTML: one block per line of rendered text (block elements, `<br>`, `<hr>`, `<pre>` lines), with the context of its first text.
/// Returns the canonical text the block ranges point into.
pub fn html_blocks(dom: &Dom, roles: &[Option<Role>], ord: &Order) -> (String, Vec<Block>) {
    let mut canon = String::new();
    let mut blocks: Vec<Block> = Vec::new();
    let mut line = String::new();
    let mut cur: Option<Block> = None;
    let mut after_sep = false;
    let mut frames: Vec<Frame> = vec![Frame { bq: 0, role: None, pre: false, tag: String::new(), class: String::new(), border: false }];
    // (node, exiting)
    let mut stack: Vec<(Id, bool)> = vec![(ROOT, false)];

    fn flush(line: &mut String, cur: &mut Option<Block>, canon: &mut String, blocks: &mut Vec<Block>, force: bool) {
        let text = canonical(line.trim());
        line.clear();
        let Some(mut b) = cur.take() else { return };
        if text.is_empty() && !force { return; }
        let start = canon.len();
        canon.push_str(&text);
        canon.push('\n');
        b.range = start..canon.len();
        b.content = text;
        features(&mut b);
        blocks.push(b);
    }

    // bounded: every node is entered once and left once
    let mut budget = 2 * dom.nodes.len() + 2;
    while let Some((id, exiting)) = stack.pop() {
        if budget == 0 { break; }
        budget -= 1;
        match &dom.nodes[id].kind {
            Kind::Element { name, html, .. } => {
                let block = *html && BLOCK_TAGS.contains(&name.as_str());
                if exiting {
                    frames.pop();
                    if block { flush(&mut line, &mut cur, &mut canon, &mut blocks, false); }
                    continue;
                }
                if !*html || SKIP_TAGS.contains(&name.as_str()) { continue; }
                if block { flush(&mut line, &mut cur, &mut canon, &mut blocks, false); }
                let top = frames.last().expect("a frame");
                let role = roles[id].or(top.role);
                if roles[id] == Some(Role::Separator) { after_sep = true; }
                let style = dom.attr(id, "style").unwrap_or("").to_ascii_lowercase();
                let frame = Frame {
                    bq: top.bq + usize::from(name == "blockquote"),
                    role,
                    pre: top.pre || name == "pre",
                    tag: if block { name.clone() } else { top.tag.clone() },
                    class: if block { dom.attr(id, "class").unwrap_or("").to_string() } else { top.class.clone() },
                    border: top.border || style.contains("border-left") || style.contains("border-top"),
                };
                if name == "hr" {
                    cur = Some(Block { depth: frame.bq, blockquote: frame.bq > 0, role, after_separator: after_sep, tag: "hr".into(), first_pos: ord.pos[id], ..Default::default() });
                    line.push_str("---");
                    flush(&mut line, &mut cur, &mut canon, &mut blocks, true);
                    continue;
                }
                if name == "img" && cur.is_none() {
                    // an image starts no block, but its position belongs to the next one (it is kept with the text after it)
                }
                frames.push(frame);
                stack.push((id, true));
                for c in dom.nodes[id].children.iter().rev() { stack.push((*c, false)); }
            }
            Kind::Text(t) => {
                let f = frames.last().expect("a frame");
                let start_block = |f: &Frame| Block {
                    depth: f.bq, blockquote: f.bq > 0, role: f.role, after_separator: after_sep, tag: f.tag.clone(), class: f.class.clone(),
                    border: f.border, first_pos: ord.pos[id], ..Default::default()
                };
                if f.pre {
                    let mut parts = t.split('\n').peekable();
                    while let Some(p) = parts.next() {
                        if cur.is_none() { cur = Some(start_block(f)); }
                        line.push_str(p);
                        if parts.peek().is_some() { flush(&mut line, &mut cur, &mut canon, &mut blocks, true); }
                    }
                } else {
                    let collapsed: String = {
                        let mut s = String::with_capacity(t.len());
                        let mut ws = false;
                        for ch in t.chars() { if ch.is_whitespace() && ch != '\u{a0}' { if !ws { s.push(' '); } ws = true; } else { s.push(ch); ws = false; } }
                        s
                    };
                    if collapsed.trim().is_empty() && cur.is_none() { continue; }
                    if cur.is_none() { cur = Some(start_block(f)); }
                    line.push_str(&collapsed.replace('\u{a0}', " "));
                }
            }
            Kind::Document => { for c in dom.nodes[id].children.iter().rev() { stack.push((*c, false)); } }
            Kind::Comment | Kind::Other => {}
        }
    }
    flush(&mut line, &mut cur, &mut canon, &mut blocks, false);
    (canon, blocks)
}

// ---------------------------------------------------------------- detectors (stage 4): evidence, never a verdict

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Ev { Attribution { lines: usize }, OriginalRule, ForwardRule, Underscore, HeaderBlock { lines: usize }, Closing, Delimiter, Contact, SenderName, Legal, Unsubscribe }

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Evidence { pub block: usize, pub ev: Ev, pub strength: f32 }

fn next_nonblank(blocks: &[Block], from: usize) -> Option<usize> { (from..blocks.len()).find(|i| !blocks[*i].blank()) }

/// Attribution lines: a pattern of `attribution.txt` on one line or up to ATTRIBUTION_WRAP_LINES joined, holding a date or an address.
pub fn detect_attribution(blocks: &[Block]) -> Vec<Evidence> {
    let p = patterns();
    let mut out = Vec::new();
    for i in 0..blocks.len() {
        if blocks[i].blank() || blocks[i].content.len() > 400 { continue; }
        let mut joined = String::new();
        let mut j = i;
        for k in 0..crate::tuning::ATTRIBUTION_WRAP_LINES {
            let Some(b) = blocks.get(i + k) else { break };
            if b.blank() || b.depth != blocks[i].depth { break; }
            // a later line that is an attribution by itself is found at its own index (never glued to the sentence before it)
            if k > 0 && (p.date.is_match(&b.content) || p.address.is_match(&b.content)) && any(&p.attribution, &b.content) { break; }
            if !joined.is_empty() { joined.push(' '); }
            joined.push_str(&b.content);
            j = i + k;
            let dated = p.date.is_match(&joined) || p.address.is_match(&joined) || joined.contains('<');
            if dated && any(&p.attribution, &joined) {
                // strong when the quote it introduces follows
                let quoted_after = next_nonblank(blocks, j + 1).is_some_and(|n| blocks[n].depth > blocks[i].depth || blocks[n].role == Some(Role::Quote));
                out.push(Evidence { block: i, ev: Ev::Attribution { lines: k + 1 }, strength: if quoted_after { 0.97 } else { 0.85 } });
                break;
            }
        }
        let _ = j;
    }
    out
}

/// "-----Original Message-----" and the forward rule, the long underscore rule, and embedded header blocks.
pub fn detect_separators(blocks: &[Block]) -> Vec<Evidence> {
    let p = patterns();
    let mut out = Vec::new();
    let header_follows = |i: usize| (i + 1..blocks.len()).filter(|k| !blocks[*k].blank()).take(3).any(|k| blocks[k].header == Some(Field::From) || blocks[k].header == Some(Field::Date));
    for (i, b) in blocks.iter().enumerate() {
        let c = b.content.trim();
        if c.is_empty() { continue; }
        if any(&p.original, c) { out.push(Evidence { block: i, ev: Ev::OriginalRule, strength: if header_follows(i) { 0.97 } else { 0.9 } }); }
        if any(&p.forward, c) { out.push(Evidence { block: i, ev: Ev::ForwardRule, strength: 0.9 }); }
        if any(&p.underscore, c) && next_nonblank(blocks, i + 1).is_some_and(|n| blocks[n].header == Some(Field::From)) {
            out.push(Evidence { block: i, ev: Ev::Underscore, strength: 0.97 });
        }
    }
    // header blocks: each line weak; consecutive lines with From and a date-like Date field are strong
    let mut i = 0;
    while i < blocks.len() {
        if blocks[i].header.is_none() { i += 1; continue; }
        let mut j = i;
        let (mut from, mut date) = (false, false);
        while j < blocks.len() && blocks[j].header.is_some() && blocks[j].depth == blocks[i].depth {
            from |= blocks[j].header == Some(Field::From);
            date |= blocks[j].header == Some(Field::Date) && blocks[j].has_date;
            j += 1;
        }
        let n = j - i;
        let strong = n >= HEADER_BLOCK_MIN_LINES && from && date;
        out.push(Evidence { block: i, ev: Ev::HeaderBlock { lines: n }, strength: if strong { 0.96 } else { 0.3 } });
        i = j.max(i + 1);
    }
    out
}

/// Signature evidence over a run of blocks: closings, the "-- " delimiter, contact details, the sender's own name.
pub fn detect_signature(blocks: &[Block], sender: Option<&str>) -> Vec<Evidence> {
    let p = patterns();
    let name = sender.map(|s| s.trim().trim_matches('"').to_lowercase()).filter(|s| s.len() >= 3);
    let first = name.as_deref().and_then(|n| n.split_whitespace().next()).map(str::to_string).filter(|s| s.chars().count() >= 3);
    let mut out = Vec::new();
    for (i, b) in blocks.iter().enumerate() {
        let c = b.content.trim();
        if c.is_empty() { continue; }
        if c.chars().count() <= 40 && any(&p.closing, c) { out.push(Evidence { block: i, ev: Ev::Closing, strength: 0.6 }); }
        if any(&p.delimiter, b.content.as_str()) || any(&p.delimiter, c) { out.push(Evidence { block: i, ev: Ev::Delimiter, strength: 0.7 }); }
        if c.chars().count() <= 120 && any(&p.contact, c) { out.push(Evidence { block: i, ev: Ev::Contact, strength: 0.5 }); }
        let lc = c.to_lowercase();
        if c.chars().count() <= 60 && (name.as_deref().is_some_and(|n| lc.contains(n)) || first.as_deref().is_some_and(|f| lc.trim_end_matches(|ch: char| !ch.is_alphanumeric()) == f)) {
            out.push(Evidence { block: i, ev: Ev::SenderName, strength: 0.5 });
        }
    }
    out
}

pub fn detect_footer(blocks: &[Block]) -> Vec<Evidence> {
    let p = patterns();
    let mut out = Vec::new();
    for (i, b) in blocks.iter().enumerate() {
        if b.blank() { continue; }
        if any(&p.legal, &b.content) { out.push(Evidence { block: i, ev: Ev::Legal, strength: 0.9 }); }
        if b.content.chars().count() <= 300 && any(&p.unsubscribe, &b.content) { out.push(Evidence { block: i, ev: Ev::Unsubscribe, strength: 0.85 }); }
    }
    out
}

// ---------------------------------------------------------------- history (stage 5)

fn norm_tokens(s: &str) -> Vec<String> {
    s.to_lowercase().split(|c: char| !c.is_alphanumeric()).filter(|t| !t.is_empty()).map(str::to_string).collect()
}

/// One earlier message of the thread, prepared once.
pub struct Earlier { lines: HashSet<String>, shingles: HashSet<String>, tokens: Vec<String> }

impl Earlier {
    pub fn new(text: &str) -> Earlier {
        let canon = canonical(text);
        let mut lines = HashSet::new();
        for l in canon.lines() {
            let (_, at) = quote_depth(l);
            let t = norm_tokens(&l[at..]).join(" ");
            if !t.is_empty() { lines.insert(t); }
        }
        // the tokens of the text without its quote marks, so a block is found whether or not the earlier message quoted it
        let unquoted: String = canon.lines().map(|l| { let (_, at) = quote_depth(l); &l[at..] }).collect::<Vec<_>>().join("\n");
        let tokens = norm_tokens(&unquoted);
        let shingles = tokens.windows(SHINGLE_WORDS).map(|w| w.join(" ")).collect();
        Earlier { lines, shingles, tokens }
    }
}

fn lcs(a: &[String], b: &[String]) -> usize {
    let mut prev = vec![0usize; b.len() + 1];
    let mut cur = vec![0usize; b.len() + 1];
    for x in a {
        for (j, y) in b.iter().enumerate() { cur[j + 1] = if x == y { prev[j] + 1 } else { cur[j].max(prev[j + 1]) }; }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[b.len()]
}

/// The thread's verdict on one block's text: exact normalised line, then shingles, then a bounded alignment on candidates.
pub fn thread_match(content: &str, earlier: &[Earlier]) -> Option<Reason> {
    let toks = norm_tokens(content);
    if toks.len() < THREAD_MATCH_MIN_WORDS { return None; }
    let key = toks.join(" ");
    if earlier.iter().any(|e| e.lines.contains(&key)) { return Some(Reason::ThreadExact); }
    if toks.len() < SHINGLE_WORDS { return None; }
    let sh: Vec<String> = toks.windows(SHINGLE_WORDS).map(|w| w.join(" ")).collect();
    for e in earlier {
        let hit = sh.iter().filter(|s| e.shingles.contains(*s)).count();
        let share = hit as f32 / sh.len() as f32;
        if share >= SHINGLE_MIN_SHARE { return Some(Reason::ThreadShingles); }
        if share >= ALIGN_CANDIDATE_SHARE && toks.len() <= ALIGN_MAX_TOKENS {
            // align against a window of the earlier message around its first shared shingle
            let first = sh.iter().find(|s| e.shingles.contains(*s)).and_then(|s| {
                let w: Vec<&str> = s.split(' ').collect();
                e.tokens.windows(SHINGLE_WORDS).position(|x| x.iter().zip(&w).all(|(a, b)| a == b))
            }).unwrap_or(0);
            let lo = first.saturating_sub(toks.len());
            let hi = (first + 2 * toks.len()).min(e.tokens.len()).min(lo + 2 * ALIGN_MAX_TOKENS);
            let l = lcs(&toks, &e.tokens[lo..hi]);
            if l as f32 / toks.len() as f32 >= ALIGN_MIN_SHARE { return Some(Reason::ThreadAligned); }
        }
    }
    None
}

// ---------------------------------------------------------------- rules (stage 6)

#[derive(Debug, Clone, PartialEq)]
pub struct Label { pub kind: SegmentKind, pub confidence: f32, pub reasons: Vec<Reason> }

fn label(kind: SegmentKind, confidence: f32, reasons: &[Reason]) -> Label { Label { kind, confidence, reasons: reasons.to_vec() } }

pub struct Context<'a> { pub subject: &'a str, pub sender: Option<&'a str>, pub earlier: &'a [Earlier] }

/// One label per block.
pub fn classify(blocks: &[Block], cx: &Context<'_>) -> Vec<Label> {
    let n = blocks.len();
    let new = label(SegmentKind::NewContent, 0.9, &[Reason::NoHistorySignal]);
    let mut out = vec![new.clone(); n];
    if n == 0 { return out; }
    let attributions = detect_attribution(blocks);
    let separators = detect_separators(blocks);

    // the strong separator: everything from it to the end is history (or a forward)
    let mut sep: Option<(usize, f32, Reason)> = None;
    let mut take = |i: usize, s: f32, r: Reason| { if sep.is_none_or(|(j, _, _)| i < j) { sep = Some((i, s, r)); } };
    if let Some(i) = blocks.iter().position(|b| b.after_separator) { take(i, 0.97, Reason::ClientSeparator); }
    let mut forward_rule = None;
    for e in &separators {
        match e.ev {
            Ev::OriginalRule => take(e.block, e.strength, Reason::OriginalMessageRule),
            Ev::Underscore => take(e.block, e.strength, Reason::UnderscoreRule),
            Ev::HeaderBlock { .. } if e.strength >= HIDE_MIN_CONFIDENCE => take(e.block, e.strength, Reason::HeaderBlock),
            Ev::ForwardRule => { take(e.block, e.strength, Reason::ForwardHeader); forward_rule = forward_rule.or(Some(e.block)); }
            _ => {}
        }
    }
    let end = sep.map_or(n, |(i, _, _)| i);

    // attribution lines of the region before the separator
    let mut attr_lines = vec![None::<f32>; n];
    for e in &attributions {
        if e.block >= end { continue; }
        if let Ev::Attribution { lines } = e.ev { for k in e.block..(e.block + lines).min(end) { attr_lines[k] = Some(e.strength); } }
    }
    for (i, b) in blocks.iter().enumerate().take(end) { if b.role == Some(Role::Attribution) && attr_lines[i].is_none() { attr_lines[i] = Some(0.97); } }
    let is_attr = |i: usize| attr_lines[i].is_some_and(|s| s >= HIDE_MIN_CONFIDENCE);
    let quoted = |i: usize| blocks[i].quoted();
    let authored = |i: usize| !blocks[i].blank() && !quoted(i) && !is_attr(i);

    // quoted runs (with the attribution that introduces them): (start, end exclusive)
    let mut runs: Vec<(usize, usize)> = Vec::new();
    let mut i = 0;
    while i < end {
        if quoted(i) || is_attr(i) {
            let s = i;
            let mut e = i;
            while i < end && (quoted(i) || is_attr(i) || blocks[i].blank()) { if !blocks[i].blank() { e = i + 1; } i += 1; }
            runs.push((s, e));
        } else { i += 1; }
    }

    // the tail of the sender's own text (the last authored stretch before the trailing history): signature and footer
    let last_authored = (0..end).rev().find(|i| authored(*i));
    let first_authored = (0..end).find(|i| authored(*i));
    let mut tail_kind: Vec<Option<Label>> = vec![None; n];
    if let (Some(la), Some(fa)) = (last_authored, first_authored) {
        let tail_start = runs.iter().filter(|(_, e)| *e <= la).map(|(_, e)| *e).max().unwrap_or(0).max(fa);
        let window: Vec<usize> = (tail_start..=la).filter(|k| !blocks[*k].blank()).collect();
        // footer first: its first block among the last FOOTER_MAX_BLOCKS, with the sender's text before it
        let foot = detect_footer(&blocks[..=la]);
        let fwin = &window[window.len().saturating_sub(FOOTER_MAX_BLOCKS)..];
        let footer_start = fwin.iter().copied().find(|k| foot.iter().any(|e| e.block == *k)).filter(|k| (0..*k).any(|x| authored(x)));
        if let Some(f) = footer_start {
            let mut rs = Vec::new();
            if foot.iter().any(|e| e.block >= f && e.ev == Ev::Legal) { rs.push(Reason::LegalNotice); }
            if foot.iter().any(|e| e.block >= f && e.ev == Ev::Unsubscribe) { rs.push(Reason::UnsubscribeFooter); }
            for k in f..=la { tail_kind[k] = Some(label(SegmentKind::Disclaimer, 0.9, &rs)); }
        }
        let sig_end = footer_start.map_or(la, |f| f.saturating_sub(1));
        let sev = detect_signature(&blocks[..=la.min(n - 1)], cx.sender);
        let swin: Vec<usize> = window.iter().copied().filter(|k| *k <= sig_end).collect();
        let swin = &swin[swin.len().saturating_sub(SIGNATURE_MAX_LINES)..];
        let starts = |k: usize| sev.iter().any(|e| e.block == k && matches!(e.ev, Ev::Closing | Ev::Delimiter)) || blocks[k].role == Some(Role::Signature);
        if let Some(s) = swin.iter().copied().find(|k| starts(*k)).filter(|k| (0..*k).any(|x| authored(x))) {
            let mut rs: Vec<Reason> = Vec::new();
            let has = |ev: Ev| sev.iter().any(|e| e.block >= s && e.block <= sig_end && std::mem::discriminant(&e.ev) == std::mem::discriminant(&ev));
            if has(Ev::Closing) { rs.push(Reason::SignatureClosing); }
            if has(Ev::Delimiter) { rs.push(Reason::SignatureDelimiter); }
            if has(Ev::Contact) { rs.push(Reason::SignatureContact); }
            if has(Ev::SenderName) { rs.push(Reason::SignatureSenderName); }
            if (s..=sig_end).any(|k| blocks[k].role == Some(Role::Signature)) { rs.push(Reason::SignatureContainer); }
            // TWO independent signals (closing and delimiter are one kind: how the signature starts)
            let starts_kind = usize::from(rs.contains(&Reason::SignatureClosing) || rs.contains(&Reason::SignatureDelimiter));
            let others = rs.iter().filter(|r| matches!(r, Reason::SignatureContact | Reason::SignatureSenderName | Reason::SignatureContainer)).count();
            if starts_kind + others >= 2 { for k in s..=sig_end { if tail_kind[k].is_none() { tail_kind[k] = Some(label(SegmentKind::Signature, 0.9, &rs)); } } }
        }
    }
    let real_authored = |i: usize| authored(i) && tail_kind[i].is_none();

    // quoted runs: inline quote, bottom post or trailing history
    let total_runs = runs.len();
    for (s, e) in &runs {
        let (s, e) = (*s, *e);
        let after = (e..end).any(real_authored);
        let before = (0..s).any(|x| authored(x));
        let anchored = (s..e).any(|k| is_attr(k));
        let client = (s..e).any(|k| blocks[k].role == Some(Role::Quote));
        let bq = (s..e).any(|k| blocks[k].blockquote);
        let mut rs = Vec::new();
        if anchored { rs.push(if (s..e).any(|k| blocks[k].role == Some(Role::Attribution)) { Reason::ClientAttribution } else { Reason::AttributionLine }); }
        if client { rs.push(Reason::ClientQuote); }
        if bq && !client { rs.push(Reason::Blockquote); }
        if !bq && !client { rs.push(Reason::QuoteMarks); }
        let quoted_lines = (s..e).filter(|k| quoted(*k) && !blocks[*k].blank()).count();
        let l = if after {
            if !before && total_runs == 1 && (anchored || quoted_lines >= BOTTOM_POST_MIN_LINES) {
                rs.push(Reason::BottomPost);
                label(SegmentKind::ReplyHistory, 0.85, &rs)
            } else {
                rs.push(Reason::AuthoredAfter);
                label(SegmentKind::InlineQuote, 0.9, &rs)
            }
        } else if anchored || client {
            label(SegmentKind::ReplyHistory, 0.97, &rs)
        } else if bq {
            label(SegmentKind::ReplyHistory, 0.85, &rs)
        } else {
            label(SegmentKind::ReplyHistory, 0.8, &rs)
        };
        for k in s..e { out[k] = l.clone(); }
    }
    for k in 0..end { if let Some(l) = &tail_kind[k] { out[k] = l.clone(); } }

    // the region after the strong separator
    if let Some((s, conf, reason)) = sep {
        let forward_subject = is_forward_subject(cx.subject);
        let forward = forward_rule.is_some_and(|f| f <= s + 3) || (forward_subject && matches!(reason, Reason::HeaderBlock | Reason::ClientSeparator | Reason::OriginalMessageRule | Reason::UnderscoreRule));
        let l = if forward {
            let mut rs = vec![Reason::ForwardHeader];
            if forward_subject { rs.push(Reason::ForwardSubject); }
            if !(0..s).any(|x| authored(x) && says_something(&blocks[x].content)) { rs.push(Reason::ForwardWithoutComment); }
            label(SegmentKind::ForwardedContent, if forward_subject { 0.9 } else { 0.8 }, &rs)
        } else {
            label(SegmentKind::ReplyHistory, conf, &[reason])
        };
        for k in s..n { out[k] = l.clone(); }
    }

    // the thread: blocks the sender's text holds that an earlier message already held
    if !cx.earlier.is_empty() {
        let matched: Vec<Option<Reason>> = (0..n).map(|k| if blocks[k].blank() { None } else { thread_match(&blocks[k].content, cx.earlier) }).collect();
        // quoted history the thread confirms: a little more sure
        for k in 0..n {
            if let Some(r) = matched[k] {
                if out[k].kind == SegmentKind::ReplyHistory && !out[k].reasons.contains(&r) { out[k].reasons.push(r); out[k].confidence = (out[k].confidence + 0.02).min(0.99); }
            }
        }
        // a trailing run of the sender's "own" text found in an earlier message: pasted history, collapsed (never hidden)
        let news: Vec<usize> = (0..end).filter(|k| out[*k].kind == SegmentKind::NewContent && !blocks[*k].blank()).collect();
        let mut run_start = news.len();
        while run_start > 0 && matched[news[run_start - 1]].is_some() { run_start -= 1; }
        if run_start > 0 && run_start < news.len() {
            let chars: usize = news[run_start..].iter().map(|k| blocks[*k].content.chars().count()).sum();
            if chars >= THREAD_MATCH_MIN_CHARS {
                for &k in &news[run_start..] {
                    let r = matched[k].unwrap_or(Reason::ThreadShingles);
                    out[k] = label(SegmentKind::ReplyHistory, 0.9, &[r]);
                }
            }
        }
    }
    // blank blocks take the label of the block before them (the first ones, of the block after)
    for k in 0..n {
        if blocks[k].blank() {
            if k > 0 { out[k] = out[k - 1].clone(); } else if let Some(j) = next_nonblank(blocks, 0) { out[k] = out[j].clone(); }
        }
    }
    out
}

/// The visibility band. Hiding needs the hide band AND a structural anchor; a forward is never hidden; the sender's text is always shown.
pub fn band(kind: SegmentKind, confidence: f32, reasons: &[Reason]) -> Visibility {
    if kind.authored() || reasons.is_empty() { return Visibility::Show; }
    let anchored = reasons.iter().any(|r| r.is_anchor());
    let v = if confidence >= HIDE_MIN_CONFIDENCE && anchored { Visibility::Hide } else if confidence >= COLLAPSE_MIN_CONFIDENCE { Visibility::Collapse } else { Visibility::Show };
    if kind == SegmentKind::ForwardedContent {
        if reasons.contains(&Reason::ForwardWithoutComment) { return Visibility::Show; }
        return v.min(Visibility::Collapse);
    }
    if matches!(kind, SegmentKind::Signature | SegmentKind::Disclaimer) { return v.min(Visibility::Collapse); }
    v
}

/// Text that says something: at least one character that is not a space, ASCII punctuation, a dash, a rule, a bullet or an invisible mark.
/// A line of dashes above a forward is a separator, not the sender's comment (owner's picture 212, 13 Nov 2026: such a mail showed three
/// dashes and folded away the mail it forwarded). An emoji says something.
pub fn says_something(text: &str) -> bool {
    text.chars().any(|c| !(c.is_whitespace() || c.is_control() || c.is_ascii_punctuation()
        || matches!(c, '\u{00A0}' | '\u{00AD}' | '\u{00B7}' | '\u{200B}'..='\u{200F}' | '\u{2010}'..='\u{2015}' | '\u{2022}' | '\u{2026}' | '\u{2060}' | '\u{2500}'..='\u{257F}' | '\u{FEFF}')))
}

/// `fold_from` (owner pick F2): the first segment from which everything to the end is put-away history, forward, signature or footer, when
/// the sender's own text comes before it; None when nothing would be left above the fold.
pub fn fold_from(segments: &[Segment]) -> Option<usize> {
    let foldable = |s: &Segment| matches!(s.kind, SegmentKind::ReplyHistory | SegmentKind::ForwardedContent | SegmentKind::Signature | SegmentKind::Disclaimer)
        && s.visibility != Visibility::Show;
    let mut f = segments.len();
    while f > 0 && foldable(&segments[f - 1]) { f -= 1; }
    if f == segments.len() { return None; }
    // nothing shown above the fold that says something (only a forward, only history, a line of dashes): fold nothing
    if !segments[..f].iter().any(|s| s.visibility == Visibility::Show && says_something(&s.text)) { return None; }
    Some(f)
}

/// Consecutive blocks with the same label become one segment (ranges contiguous: the segments in order are the canonical text).
pub fn merge(canon: &str, blocks: &[Block], labels: &[Label]) -> Vec<(Range<usize>, Label, Range<usize>)> {
    let mut out: Vec<(Range<usize>, Label, Range<usize>)> = Vec::new();
    for (k, (b, l)) in blocks.iter().zip(labels).enumerate() {
        match out.last_mut() {
            Some((r, last, bs)) if last == l => { r.end = b.range.end; bs.end = k + 1; }
            _ => out.push((b.range.clone(), l.clone(), k..k + 1)),
        }
    }
    if let Some(first) = out.first_mut() { first.0.start = 0; }
    if let Some(last) = out.last_mut() { last.0.end = canon.len(); }
    out
}
