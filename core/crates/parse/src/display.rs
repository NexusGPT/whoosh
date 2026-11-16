//! body-r4 (B): the display document. Built AFTER segmentation from the same DOM (style rules already inlined by `css::inline_styles`):
//! our own pass drops what must never be kept (elements, attributes, CSS properties, unsafe links, remote images), then ammonia's
//! allow-list has the last word. Also: plain text made HTML, the designed/written decision (owner pick B3) and the written document.
use std::collections::HashSet;
use std::sync::OnceLock;

use ammonia::{Builder, UrlRelative};
use regex::Regex;

use crate::css::sanitize_style;
use crate::dom::{escape_attr, escape_text, escaped, Dom, Id, Kind, ROOT};
use crate::sanitise::BLOCKED_IMAGE_SRC;

/// `PARSE_DISPLAY_DEPTH_MAX`: below this many levels of nesting a subtree is written as its escaped text (hostile nesting).
pub const DISPLAY_DEPTH_MAX: usize = 128;
/// `PARSE_PLAIN_QUOTE_DEPTH_MAX`: ">" levels made nested quotes in a plain mail; deeper marks stay literal text.
pub const PLAIN_QUOTE_DEPTH_MAX: usize = 100;
/// Layout thresholds (owner pick B3), ledger rows `PARSE_LAYOUT_*`.
pub const LAYOUT_WRAPPER_MIN_WIDTH_PX: u32 = 300;
pub const LAYOUT_WRAPPER_MIN_WIDTH_PERCENT: u32 = 50;
pub const LAYOUT_FIXED_WIDTH_MIN_PX: u32 = 480;
pub const LAYOUT_FIXED_WIDTH_MAX_PX: u32 = 800;
pub const LAYOUT_CELL_BACKGROUNDS_MIN: usize = 2;
pub const LAYOUT_PICTURES_MIN: u32 = 3;
pub const LAYOUT_TEXT_CHARS_PER_PICTURE_MAX: usize = 300;
pub const LAYOUT_STYLE_RULES_MIN: usize = 12;
/// A picture declared this small or smaller (px, either side) is an icon or a logo, never a hero: it does not make a mail picture-heavy.
pub const LAYOUT_ICON_MAX_PX: u32 = 160;
pub const LAYOUT_TABLE_TEXT_SHARE_MIN: f32 = 0.5;

/// Elements never kept, with everything inside them.
const DROP: &[&str] = &[
    "script", "style", "link", "meta", "base", "title", "frame", "iframe", "frameset", "template", "noscript", "object", "embed", "canvas", "audio",
    "video", "applet", "input", "select", "textarea", "head", "svg", "math", "param", "source", "track",
];
/// Elements never kept whose text is the sender's: the element goes, its children stay.
const UNWRAP: &[&str] = &["form", "dialog", "button", "html", "label", "fieldset", "legend"];
const VOID: &[&str] = &["area", "br", "col", "hr", "img", "wbr"];

/// What the cleaner saw.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CleanStats {
    /// Every remote image replaced (the two counts below together).
    pub remote_images_blocked: u32,
    /// The ones that are pictures (`is_tracker` false) and the ones that say they are invisible (trackers).
    pub pictures_blocked: u32,
    pub trackers_blocked: u32,
    /// `cid:` images that point at a part of this message: (content id, IMAP part number).
    pub cid_images: Vec<(String, String)>,
}

fn safe_href(v: &str) -> bool {
    let v = v.trim().to_ascii_lowercase();
    ["http:", "https:", "mailto:", "tel:"].iter().any(|p| v.starts_with(p))
}

fn event_or_blocked_attr(k: &str) -> bool {
    // the source's own `data-whoosh-src` is never kept: only ours, written below from the picture's `src`
    k.starts_with("on") || matches!(k, "action" | "formaction" | "popover" | "popovertarget" | "background" | "srcset" | "xmlns" | "srcdoc" | "data-whoosh-src")
}

/// The attribute that keeps a blocked picture's address for the view's "Load once" (a `data-` attribute: no web view fetches it).
pub const KEPT_SRC_ATTR: &str = "data-whoosh-src";
/// `PARSE_PICTURE_MIN_PX`: a remote image is a tracker only when it SAYS it is invisible: a stated width or height under this many pixels,
/// or hidden by its style. Everything else, an image with no stated size included, is a picture (ruling of the lead, 2026-11-11: most
/// mail leaves the size to the style sheet or to the picture itself).
pub const PICTURE_MIN_PX: u32 = 3;

/// An address the view may load on the owner's action: http or https only, no space or control character.
pub fn remote_address(v: &str) -> Option<&str> {
    let t = v.trim();
    let low = t.to_ascii_lowercase();
    ((low.starts_with("http://") || low.starts_with("https://")) && !t.chars().any(|c| c.is_whitespace() || c.is_control())).then_some(t)
}

/// Tracker or picture, from the image element's own attributes (width, height, style): see `PICTURE_MIN_PX`.
pub fn is_tracker(attrs: &[(String, String)]) -> bool {
    let get = |n: &str| attrs.iter().find(|(k, _)| k == n).map(|(_, v)| v.as_str());
    let mut dims: Vec<Option<u32>> = Vec::new(); // None: a percentage or another unit (large enough)
    let mut read = |v: &str| {
        let v = v.trim().to_ascii_lowercase();
        let num: String = v.chars().take_while(|c| c.is_ascii_digit()).collect();
        if let Ok(n) = num.parse::<u32>() { dims.push(if v.ends_with('%') { None } else { Some(n) }); }
    };
    for n in ["width", "height"] { if let Some(v) = get(n) { read(v); } }
    let decls = crate::css::parse_declarations(get("style").unwrap_or(""));
    for d in &decls {
        let v = d.value.to_ascii_lowercase().replace(' ', "");
        if (d.prop == "display" && v == "none") || (d.prop == "visibility" && v == "hidden") || (d.prop == "opacity" && v.trim_start_matches('0').trim_start_matches('.').chars().all(|c| c == '0')) { return true; }
        if d.prop == "width" || d.prop == "height" { read(&d.value); }
    }
    dims.iter().any(|d| d.is_some_and(|n| n < PICTURE_MIN_PX))
}

pub struct Cleaner<'a> {
    pub dom: &'a Dom,
    /// (content id without brackets, IMAP part) of the message's parts.
    pub parts: &'a [(String, String)],
}

impl Cleaner<'_> {
    /// Our pass over the node `id` (and its subtree), keeping only the node positions in `keep` (a segment's share), written as HTML.
    pub fn write(&self, id: Id, keep: Option<(usize, usize)>, ord: Option<&crate::segment::Order>, stats: &mut CleanStats) -> String {
        let mut out = String::new();
        self.walk(id, 0, keep, ord, stats, &mut out);
        out
    }

    fn walk(&self, id: Id, depth: usize, keep: Option<(usize, usize)>, ord: Option<&crate::segment::Order>, stats: &mut CleanStats, out: &mut String) {
        let dom = self.dom;
        let inside = |id: Id| match (keep, ord) { (Some((a, b)), Some(o)) => o.pos[id] >= a && o.pos[id] <= b, _ => true };
        let touches = |id: Id| match (keep, ord) { (Some((a, b)), Some(o)) => o.pos[id] <= b && o.end[id] >= a, _ => true };
        match &dom.nodes[id].kind {
            Kind::Document => for c in &dom.nodes[id].children { self.walk(*c, depth, keep, ord, stats, out); },
            Kind::Text(t) => if inside(id) { escape_text(t, out) },
            Kind::Comment | Kind::Other => {}
            Kind::Element { name, html, attrs } => {
                if !*html || DROP.contains(&name.as_str()) { return; }
                let is_void = VOID.contains(&name.as_str());
                if (is_void && !inside(id)) || (!is_void && !touches(id)) { return; }
                if depth >= DISPLAY_DEPTH_MAX {
                    let t: String = dom.descendants(id).into_iter().filter(|n| inside(*n)).filter_map(|n| match &dom.nodes[n].kind { Kind::Text(t) => Some(t.as_str()), _ => None }).collect();
                    escape_text(&t, out);
                    return;
                }
                if UNWRAP.contains(&name.as_str()) {
                    for c in &dom.nodes[id].children { self.walk(*c, depth + 1, keep, ord, stats, out); }
                    return;
                }
                let body = name == "body";
                let tag = if body { "div" } else { name.as_str() };
                out.push('<');
                out.push_str(tag);
                if body { out.push_str(" class=\"whoosh-body\""); }
                for (k, v) in attrs {
                    if event_or_blocked_attr(k) || (body && k == "class") { continue; }
                    let value = match k.as_str() {
                        "style" => { let s = sanitize_style(v, body); if s.is_empty() { continue; } s }
                        "href" => { if !safe_href(v) { continue; } v.clone() }
                        "src" if name == "img" => self.image_src(v, attrs, stats),
                        "src" => continue,
                        "width" | "height" if body => continue,
                        _ => v.clone(),
                    };
                    out.push(' '); out.push_str(k); out.push_str("=\""); escape_attr(&value, out); out.push('"');
                }
                if name == "img" {
                    let src = attrs.iter().find(|(k, _)| k == "src").map(|(_, v)| v.as_str()).unwrap_or("");
                    if let Some(a) = remote_address(src).filter(|_| !is_tracker(attrs)) {
                        out.push(' '); out.push_str(KEPT_SRC_ATTR); out.push_str("=\""); escape_attr(a, out); out.push('"');
                    }
                }
                out.push('>');
                if is_void { return; }
                for c in &dom.nodes[id].children { self.walk(*c, depth + 1, keep, ord, stats, out); }
                out.push_str("</"); out.push_str(tag); out.push('>');
            }
        }
    }

    fn image_src(&self, v: &str, attrs: &[(String, String)], stats: &mut CleanStats) -> String {
        let t = v.trim();
        let low = t.to_ascii_lowercase();
        if let Some(cid) = low.strip_prefix("cid:") {
            let want = cid.trim_matches(|c| c == '<' || c == '>');
            if let Some((c, part)) = self.parts.iter().find(|(c, _)| c.trim_matches(|ch| ch == '<' || ch == '>').eq_ignore_ascii_case(want)) {
                if !stats.cid_images.iter().any(|(x, _)| x == c) { stats.cid_images.push((c.clone(), part.clone())); }
                return format!("cid:{}", c.trim_matches(|ch| ch == '<' || ch == '>'));
            }
            return BLOCKED_IMAGE_SRC.to_string();
        }
        if low.starts_with("http://") || low.starts_with("https://") || low.starts_with("//") {
            stats.remote_images_blocked += 1;
            if is_tracker(attrs) { stats.trackers_blocked += 1 } else { stats.pictures_blocked += 1 }
        }
        BLOCKED_IMAGE_SRC.to_string()
    }
}

/// ammonia's allow-list, the last word on every document this module returns.
pub fn allow_list(html: &str) -> String {
    let mut b = Builder::default();
    b.add_tags(["font", "center"]);
    b.add_generic_attributes(["style", "bgcolor", "width", "height", "align", "valign", "border", "cellpadding", "cellspacing", "dir", "lang", "color", "face", "size", "colspan", "rowspan"]);
    b.add_allowed_classes("div", ["whoosh-body"]);
    b.add_tag_attributes("img", [KEPT_SRC_ATTR]);
    b.url_schemes(HashSet::from(["http", "https", "mailto", "tel", "cid"]));
    b.url_relative(UrlRelative::Deny);
    b.attribute_filter(|element, attribute, value| match attribute {
        "style" => { let s = sanitize_style(value, false); if s.is_empty() { None } else { Some(s.into()) } }
        "href" => if safe_href(value) { Some(value.into()) } else { None },
        KEPT_SRC_ATTR => if element == "img" && remote_address(value).is_some() { Some(value.into()) } else { None },
        "src" if element == "img" => if value.trim().to_ascii_lowercase().starts_with("cid:") { Some(value.into()) } else { Some(BLOCKED_IMAGE_SRC.into()) },
        _ => Some(value.into()),
    });
    b.clean(html).to_string()
}

/// A whole cleaned document (or one segment's share of it).
pub fn clean_dom(dom: &Dom, parts: &[(String, String)], keep: Option<(usize, usize)>, ord: Option<&crate::segment::Order>) -> (String, CleanStats) {
    let mut stats = CleanStats::default();
    let ours = Cleaner { dom, parts }.write(ROOT, keep, ord, &mut stats);
    (allow_list(&ours), stats)
}

/// Our own pass alone, before the allow-list (for the tests that prove each layer on its own).
#[doc(hidden)]
pub fn first_pass(html: &str, parts: &[(String, String)]) -> String {
    let dom = Dom::parse(html);
    let mut stats = CleanStats::default();
    Cleaner { dom: &dom, parts }.write(ROOT, None, None, &mut stats)
}

// ---------------------------------------------------------------- plain text made HTML

fn link_re() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r#"(?i)\b(?:https?://[^\s<>"]+|www\.[^\s<>"]+|[a-z0-9._%+-]+@[a-z0-9.-]+\.[a-z]{2,}|(?:[a-z0-9-]+\.)+(?:com|org|net|edu|gov|io|co|fr|de|be|nl|uk|es|it|pt|pl|ee|fi|se|ru|ch|at|eu|info|biz|app|dev)(?:/[^\s<>"]*)?)"#).expect("static"))
}

/// One line's text, escaped, with web addresses, mail addresses and bare domains as links.
pub fn linkify(line: &str, out: &mut String) {
    let mut last = 0;
    for m in link_re().find_iter(line) {
        let mut s = m.as_str();
        while s.ends_with(['.', ',', ';', ':', '!', '?', ')', '\'']) { s = &s[..s.len() - 1]; }
        if s.is_empty() { continue; }
        let (a, b) = (m.start(), m.start() + s.len());
        escape_text(&line[last..a], out);
        let low = s.to_ascii_lowercase();
        let href = if low.starts_with("http://") || low.starts_with("https://") { s.to_string() } else if s.contains('@') && !s.contains('/') { format!("mailto:{s}") } else { format!("https://{s}") };
        out.push_str("<a href=\""); escape_attr(&href, out); out.push_str("\">"); escape_text(s, out); out.push_str("</a>");
        last = b;
    }
    escape_text(&line[last..], out);
}

/// Plain text to HTML: a blank line ends a paragraph, single line breaks are kept, links made, ">" lines nested quotes (deeper than
/// PLAIN_QUOTE_DEPTH_MAX stays literal), a line of two or more "-" or "_" alone is a rule, everything escaped. Expects canonical text
/// (format=flowed already unfolded).
pub fn plain_to_html(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + text.len() / 4);
    let mut depth = 0usize;
    let mut open_p = false;
    let mut first_in_p = true;
    for line in text.split('\n') {
        let (d, at) = crate::canon::quote_depth(line);
        let (d, at) = if d > PLAIN_QUOTE_DEPTH_MAX {
            // keep PLAIN_QUOTE_DEPTH_MAX levels; the marks past them stay as text
            let mut seen = 0; let mut cut = 0;
            for (i, ch) in line.char_indices() { if ch == '>' { seen += 1; if seen == PLAIN_QUOTE_DEPTH_MAX { cut = i + 1; break; } } }
            (PLAIN_QUOTE_DEPTH_MAX, cut)
        } else { (d, at) };
        let body = &line[at..];
        if d != depth {
            if open_p { out.push_str("</p>"); open_p = false; }
            while depth < d { out.push_str("<blockquote>"); depth += 1; }
            while depth > d { out.push_str("</blockquote>"); depth -= 1; }
        }
        let t = body.trim_end();
        if t.trim().is_empty() {
            if open_p { out.push_str("</p>"); open_p = false; }
            continue;
        }
        if t.trim().len() >= 2 && (t.trim().chars().all(|c| c == '-') || t.trim().chars().all(|c| c == '_')) {
            if open_p { out.push_str("</p>"); open_p = false; }
            out.push_str("<hr>");
            continue;
        }
        if !open_p { out.push_str("<p>"); open_p = true; first_in_p = true; }
        if !first_in_p { out.push_str("<br>"); }
        linkify(body, &mut out);
        first_in_p = false;
    }
    if open_p { out.push_str("</p>"); }
    while depth > 0 { out.push_str("</blockquote>"); depth -= 1; }
    out
}

/// The failure document: the text, escaped, line breaks kept, nothing else.
pub fn escaped_plain(text: &str) -> String { format!("<div style=\"white-space:pre-wrap\">{}</div>", escaped(text)) }

// ---------------------------------------------------------------- designed or written (owner pick B3)

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layout { Designed, Written }
impl Layout { pub fn as_str(self) -> &'static str { match self { Layout::Designed => "designed", Layout::Written => "written" } } }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutReason { BodyBackground, WrapperBackground, NestedTables, FixedWidthTable, CellBackgrounds, PictureHeavy, StyleRules, MailingFooter, TextInLayoutTables, NoDesignSignal, PageHidesText }
impl LayoutReason {
    pub fn as_str(self) -> &'static str {
        match self {
            LayoutReason::BodyBackground => "body_background", LayoutReason::WrapperBackground => "wrapper_background", LayoutReason::NestedTables => "nested_tables",
            LayoutReason::FixedWidthTable => "fixed_width_table", LayoutReason::CellBackgrounds => "cell_backgrounds", LayoutReason::PictureHeavy => "picture_heavy",
            LayoutReason::StyleRules => "style_rules", LayoutReason::MailingFooter => "mailing_footer", LayoutReason::TextInLayoutTables => "text_in_layout_tables",
            LayoutReason::NoDesignSignal => "no_design_signal", LayoutReason::PageHidesText => "page_hides_text",
        }
    }
}

/// `PARSE_PAGE_HIDES_TEXT_MIN_WORDS`: a text part with fewer words says too little to judge a page by.
pub const PAGE_HIDES_TEXT_MIN_WORDS: usize = 12;
/// `PARSE_PAGE_HIDES_TEXT_SHARE`: a page that shows under one in this many of the text part's words hides what the mail says.
pub const PAGE_HIDES_TEXT_SHARE: usize = 4;

/// A CSS length that is nothing: "0", "0px", "0.0pt".
fn is_zero_length(v: &str) -> bool {
    let n: String = v.trim().chars().take_while(|c| c.is_ascii_digit() || *c == '.').collect();
    !n.is_empty() && n.parse::<f32>().is_ok_and(|x| x == 0.0)
}

/// The first number of a declared size in pixels ("600", "600px"); a share ("100%"), `auto` or anything else is not a size.
fn declared_px(v: &str) -> Option<f32> {
    let t = v.trim().to_ascii_lowercase();
    let n: String = t.chars().take_while(|c| c.is_ascii_digit() || *c == '.').collect();
    let unit = t[n.len()..].trim();
    if n.is_empty() || !(unit.is_empty() || unit == "px") { return None; }
    n.parse().ok()
}

/// What a display document shows: the words a reader can see, all of its words, its shown pictures that are larger than an icon, and
/// the words (each with a letter or a digit) that nothing hides but a size of zero.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Shown { pub words: usize, pub all_words: usize, pub pictures: usize, pub zero_words: usize }

/// The words of a display document a reader can see, and all of its words (see `shown`).
pub fn shown_words(html: &str) -> (usize, usize) { let s = shown(html); (s.words, s.all_words) }

/// What the page shows. A word is not seen when its element or one above it is set to `display:none`, `visibility:hidden` or `opacity:0`
/// (or carries `hidden`), or is drawn at `font-size:0` with no size set again between it and the word. A shown picture counts unless it
/// declares both sides at LAYOUT_ICON_MAX_PX or less (an icon, a logo, a tracker): a picture with no declared size may be a banner.
/// Read from the inline styles alone (the style rules are inlined before the page is written). Iterative: no recursion.
pub fn shown(html: &str) -> Shown {
    let dom = Dom::parse(html);
    let (mut seen, mut all, mut pictures, mut zeroed) = (0usize, 0usize, 0usize, 0usize);
    let mut stack: Vec<(Id, bool, bool)> = vec![(ROOT, false, false)];
    while let Some((id, zero, hidden)) = stack.pop() {
        let (mut zero, mut hidden) = (zero, hidden);
        match &dom.nodes[id].kind {
            Kind::Text(t) => {
                let n = t.split_whitespace().count();
                all += n;
                if !zero && !hidden { seen += n; }
                if zero && !hidden { zeroed += t.split_whitespace().filter(|w| w.chars().any(char::is_alphanumeric)).count(); }
                continue;
            }
            Kind::Comment | Kind::Other => continue,
            Kind::Document => {}
            Kind::Element { name, html, attrs } => {
                if !*html || DROP.contains(&name.as_str()) { continue; }
                if attrs.iter().any(|(k, _)| k == "hidden") { hidden = true; }
                let attr = |n: &str| attrs.iter().find(|(k, _)| k == n).map(|(_, v)| v.as_str());
                let (mut w, mut h) = (attr("width").and_then(declared_px), attr("height").and_then(declared_px));
                if let Some(style) = attr("style") {
                    for decl in crate::css::parse_declarations(style) {
                        let v = decl.value.trim().to_ascii_lowercase();
                        match decl.prop.as_str() {
                            "font-size" => zero = is_zero_length(&v),
                            "display" => if v == "none" { hidden = true },
                            "visibility" => if v == "hidden" || v == "collapse" { hidden = true },
                            "opacity" => if v.parse::<f32>().is_ok_and(|o| o == 0.0) { hidden = true },
                            "width" => if let Some(px) = declared_px(&v) { w = Some(px) },
                            "height" => if let Some(px) = declared_px(&v) { h = Some(px) },
                            _ => {}
                        }
                    }
                }
                if name == "img" && !hidden {
                    let icon = matches!((w, h), (Some(a), Some(b)) if a.max(b) <= LAYOUT_ICON_MAX_PX as f32);
                    if !icon { pictures += 1; }
                }
            }
        }
        for c in dom.nodes[id].children.iter().rev() { stack.push((*c, zero, hidden)); }
    }
    Shown { words: seen, all_words: all, pictures, zero_words: zeroed }
}

/// True when the page hides what the mail says. It shows no picture larger than an icon (a mail built of pictures says it with them and
/// keeps its look), and either the text part holds PAGE_HIDES_TEXT_MIN_WORDS words or more and the page shows under one in
/// PAGE_HIDES_TEXT_SHARE of that count, or the page holds PAGE_HIDES_TEXT_MIN_WORDS words or more at a size of zero and they outnumber
/// the words it shows (owner's picture 216, 14 Nov 2026: the lines a sender's system adds under its page are shown, and are no reason to
/// leave the message itself unseen). A page with none of the hiding declarations is not parsed: it hides nothing.
pub fn page_hides_text(display_html: &str, text: &str) -> bool {
    let low = display_html.to_ascii_lowercase();
    if !["font-size:0", "font-size: 0", "display:none", "display: none", "visibility:hidden", "visibility: hidden", "opacity:0", "opacity: 0", " hidden"].iter().any(|m| low.contains(m)) { return false; }
    let s = shown(display_html);
    if s.pictures > 0 { return false; }
    let words = text.split_whitespace().count();
    (words >= PAGE_HIDES_TEXT_MIN_WORDS && s.words * PAGE_HIDES_TEXT_SHARE < words) || (s.zero_words >= PAGE_HIDES_TEXT_MIN_WORDS && s.zero_words > s.words)
}

/// The measured features the decision reads (printed by the corpus test).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct LayoutFeatures {
    pub body_background: bool,
    pub wrapper_background: bool,
    pub table_depth: usize,
    pub fixed_width_tables: usize,
    pub cell_backgrounds: usize,
    pub remote_pictures: u32,
    pub text_chars: usize,
    pub layout_table_text_share: f32,
    pub style_rules: usize,
    pub mailing_footer: bool,
}

fn colour_is_plain(v: &str) -> bool {
    let v = v.trim().trim_end_matches("!important").trim().to_ascii_lowercase().replace(' ', "");
    v.is_empty() || matches!(v.as_str(), "white" | "#fff" | "#ffffff" | "transparent" | "none" | "inherit" | "initial" | "unset" | "rgb(255,255,255)" | "rgba(0,0,0,0)" | "window")
}

fn background_of(dom: &Dom, id: Id) -> bool {
    if dom.attr(id, "bgcolor").is_some_and(|v| !colour_is_plain(v)) { return true; }
    let style = dom.attr(id, "style").unwrap_or("");
    crate::css::parse_declarations(style).iter().any(|d| (d.prop == "background" || d.prop == "background-color" || d.prop == "background-image") && !colour_is_plain(&d.value))
}

/// (px, percent) of an element's width, from its attribute or its style.
fn width_of(dom: &Dom, id: Id) -> (Option<u32>, Option<u32>) {
    let mut vals: Vec<String> = dom.attr(id, "width").map(|w| vec![w.to_string()]).unwrap_or_default();
    for d in crate::css::parse_declarations(dom.attr(id, "style").unwrap_or("")) { if d.prop == "width" || d.prop == "max-width" { vals.push(d.value); } }
    let (mut px, mut pc) = (None, None);
    for v in vals {
        let v = v.trim().to_ascii_lowercase();
        let num: String = v.chars().take_while(|c| c.is_ascii_digit()).collect();
        let Ok(n) = num.parse::<u32>() else { continue };
        if v.ends_with('%') { pc = Some(n) } else { px = Some(n) }
    }
    (px, pc)
}

/// Measures the cleaned document. `style_rules` is counted on the source (the cleaned document holds no style element).
pub fn layout_features(clean: &str, remote_pictures: u32, style_rules: usize) -> LayoutFeatures {
    let dom = Dom::parse(clean);
    let mut f = LayoutFeatures { remote_pictures, style_rules, ..Default::default() };
    let mut text_total = 0usize;
    let mut text_tables = 0usize;
    // pictures in reading order with the text that came before each: a signature block (small pictures after the last words) is not a design signal
    let mut pictures: Vec<(usize, bool)> = Vec::new();
    let all = dom.descendants(ROOT);
    // table depth per node, iteratively (parents come before children in `all`)
    let mut tdepth = vec![0usize; dom.nodes.len()];
    let mut layout_table = vec![false; dom.nodes.len()];
    for id in &all {
        let id = *id;
        let parent = dom.nodes[id].parent;
        let (pd, pl) = parent.map_or((0, false), |p| (tdepth[p], layout_table[p]));
        tdepth[id] = pd;
        layout_table[id] = pl;
        if let Some(tag) = dom.tag(id) {
            let (px, pc) = width_of(&dom, id);
            let bg = background_of(&dom, id);
            if tag == "img" {
                let h = dom.attr(id, "height").and_then(|v| v.trim().trim_end_matches("px").parse::<u32>().ok());
                let small = px.is_some_and(|w| w <= LAYOUT_ICON_MAX_PX) || h.is_some_and(|v| v <= LAYOUT_ICON_MAX_PX);
                pictures.push((text_total, small));
            }
            if dom.attr(id, "class") == Some("whoosh-body") && bg { f.body_background = true; }
            if bg && matches!(tag, "table" | "div" | "td" | "center" | "tbody" | "tr") && (px.is_some_and(|w| w >= LAYOUT_WRAPPER_MIN_WIDTH_PX) || pc.is_some_and(|w| w >= LAYOUT_WRAPPER_MIN_WIDTH_PERCENT)) { f.wrapper_background = true; }
            if tag == "table" {
                tdepth[id] = pd + 1;
                f.table_depth = f.table_depth.max(tdepth[id]);
                let fixed = px.is_some_and(|w| (LAYOUT_FIXED_WIDTH_MIN_PX..=LAYOUT_FIXED_WIDTH_MAX_PX).contains(&w));
                if fixed { f.fixed_width_tables += 1; }
                if fixed || pd >= 1 || pc == Some(100) { layout_table[id] = true; }
            }
            if matches!(tag, "td" | "th") && bg { f.cell_backgrounds += 1; }
        }
        if let Kind::Text(t) = &dom.nodes[id].kind {
            let n = t.trim().chars().count();
            text_total += n;
            if layout_table[id] { text_tables += n; }
        }
    }
    f.text_chars = text_total;
    // Only pictures that can carry a design count: not an icon or logo (declared small), and not one after the last words (the signature). The caller's count is the upper bound.
    let counted = pictures.iter().filter(|(before, small)| !*small && *before < text_total).count() as u32;
    f.remote_pictures = remote_pictures.min(counted);
    f.layout_table_text_share = if text_total == 0 { 0.0 } else { text_tables as f32 / text_total as f32 };
    let text = dom.plain_text(ROOT);
    f.mailing_footer = crate::segment::patterns().unsubscribe.iter().any(|r| r.is_match(&text));
    f
}

/// Designed when any strong signal is there (when the signals disagree, designed: drawing a designed mail as written breaks it), or two
/// medium ones; else written (the default, and the safe side for a thread).
pub fn decide_layout(f: &LayoutFeatures) -> (Layout, Vec<LayoutReason>) {
    let mut strong = Vec::new();
    if f.body_background { strong.push(LayoutReason::BodyBackground); }
    if f.wrapper_background { strong.push(LayoutReason::WrapperBackground); }
    if f.table_depth >= 2 { strong.push(LayoutReason::NestedTables); }
    if f.fixed_width_tables > 0 { strong.push(LayoutReason::FixedWidthTable); }
    if f.cell_backgrounds >= LAYOUT_CELL_BACKGROUNDS_MIN { strong.push(LayoutReason::CellBackgrounds); }
    if f.remote_pictures >= LAYOUT_PICTURES_MIN && f.text_chars / (f.remote_pictures as usize).max(1) < LAYOUT_TEXT_CHARS_PER_PICTURE_MAX { strong.push(LayoutReason::PictureHeavy); }
    let mut medium = Vec::new();
    if f.style_rules >= LAYOUT_STYLE_RULES_MIN { medium.push(LayoutReason::StyleRules); }
    if f.mailing_footer { medium.push(LayoutReason::MailingFooter); }
    if f.layout_table_text_share >= LAYOUT_TABLE_TEXT_SHARE_MIN { medium.push(LayoutReason::TextInLayoutTables); }
    if !strong.is_empty() || medium.len() >= 2 {
        strong.extend(medium);
        (Layout::Designed, strong)
    } else {
        (Layout::Written, vec![LayoutReason::NoDesignSignal])
    }
}

/// The written document: the cleaned document without the sender's font family, font size, text colour and backgrounds (the view draws it
/// in the app's type); bold, italic, links, lists, quotes, data tables and inline pictures stay.
pub fn written_document(clean: &str) -> String {
    let mut dom = Dom::parse(clean);
    let ids = dom.descendants(ROOT);
    for id in ids {
        if !dom.is_element(id) { continue; }
        for a in ["bgcolor", "color", "face", "size", "background"] { dom.remove_attr(id, a); }
        if let Some(style) = dom.attr(id, "style").map(str::to_string) {
            let kept: String = crate::css::parse_declarations(&style).into_iter()
                .filter(|d| !matches!(d.prop.as_str(), "font-family" | "font" | "font-size" | "color" | "background" | "background-color" | "background-image" | "mso-fareast-font-family" | "mso-bidi-font-family" | "mso-ascii-font-family" | "mso-hansi-font-family"))
                .map(|d| format!("{}:{};", d.prop, d.value)).collect();
            if kept.is_empty() { dom.remove_attr(id, "style"); } else { dom.set_attr(id, "style", kept); }
        }
    }
    let body = dom.find(ROOT, "body").unwrap_or(ROOT);
    allow_list(&dom.serialize_children(body, DISPLAY_DEPTH_MAX))
}

/// The plain document: the written document without the sender's boxes (widths, paddings, margins, borders, alignment, line heights).
/// What a page that hides its words is drawn as (owner's picture 216, 14 Nov 2026: kept, the boxes left blank bands above and under the
/// words and a narrow centred column beside lines at the left edge). Its tables are layout: each becomes plain blocks, one under the
/// other (the cells of a row that holds two or more stay side by side). Words, links, lists and pictures stay; what the sender set not to
/// show stays unseen (`display:none`, a height of zero). A picture keeps a size said in pixels, and the block of one that is alone in it
/// keeps a space under it.
pub fn plain_document(clean: &str) -> String {
    let mut dom = Dom::parse(&written_document(clean));
    let ids = dom.descendants(ROOT);
    let only_child = |dom: &Dom, parent: Id, child: Id| dom.nodes[parent].children.iter().all(|c| *c == child || match &dom.nodes[*c].kind { Kind::Text(t) => t.trim().is_empty(), Kind::Comment => true, _ => false });
    // the cells of a row that holds two or more stay side by side (a line of small pictures stays a line)
    let mut beside: HashSet<Id> = HashSet::new();
    // the block of a picture that is alone in it (a mark above the words) keeps a space under it: on the block, so the space is there
    // whether the picture is loaded or stands as its placeholder (read on main168: the mark touched the first line)
    let mut alone: HashSet<Id> = HashSet::new();
    for id in &ids {
        match dom.tag(*id) {
            Some("tr") => {
                let cells: Vec<Id> = dom.nodes[*id].children.iter().copied().filter(|c| matches!(dom.tag(*c), Some("td" | "th"))).collect();
                if cells.len() >= 2 { beside.extend(cells); }
            }
            Some("img") => {
                let mut node = *id;
                if let Some(p) = dom.nodes[node].parent { if dom.tag(p) == Some("a") && only_child(&dom, p, node) { node = p; } }
                if let Some(block) = dom.nodes[node].parent.filter(|p| *p != ROOT && dom.is_element(*p) && only_child(&dom, *p, node)) { alone.insert(block); }
            }
            _ => {}
        }
    }
    for id in ids {
        if !dom.is_element(id) { continue; }
        let img = dom.tag(id) == Some("img");
        if !img { for a in ["width", "height", "align", "valign", "cellpadding", "cellspacing", "border"] { dom.remove_attr(id, a); } }
        // the sender's layout tables become plain blocks: the words start where the lines under them start (read on main166 and main167:
        // each cell of four tables, one in the other, took the app's own cell padding and set the words 16 points right of the lines
        // under them; a spacing attribute does not win over that style)
        if let Kind::Element { name, html: true, .. } = &mut dom.nodes[id].kind {
            if matches!(name.as_str(), "table" | "thead" | "tbody" | "tfoot" | "tr" | "td" | "th") { *name = "div".to_string(); }
        }
        let style = dom.attr(id, "style").map(str::to_string).unwrap_or_default();
        let mut kept: String = crate::css::parse_declarations(&style).into_iter().filter(|d| {
            let (p, v) = (d.prop.as_str(), d.value.trim().to_ascii_lowercase());
            match p {
                "width" | "height" if img => declared_px(&v).is_some(),
                "height" | "max-height" => is_zero_length(&v),
                "display" => v == "none",
                "width" | "max-width" | "min-width" | "min-height" | "text-align" | "vertical-align" | "float" | "line-height" | "direction" | "border-collapse" | "border-spacing" | "table-layout" => false,
                _ => !(p.starts_with("padding") || p.starts_with("margin") || p.starts_with("border") || p.starts_with("outline") || p.starts_with("mso-")),
            }
        }).map(|d| format!("{}:{};", d.prop, d.value)).collect();
        let unseen = kept.contains("display:none");
        if beside.contains(&id) && !unseen { kept.push_str("display:inline-block;vertical-align:top;margin-right:12px;"); }
        if alone.contains(&id) && !unseen { kept.push_str("margin-bottom:12px;"); }
        if kept.is_empty() { dom.remove_attr(id, "style"); } else { dom.set_attr(id, "style", kept); }
    }
    let body = dom.find(ROOT, "body").unwrap_or(ROOT);
    allow_list(&dom.serialize_children(body, DISPLAY_DEPTH_MAX))
}

/// Rules in a source's style elements (a medium designed signal).
pub fn count_style_rules(dom: &Dom) -> usize {
    dom.descendants(ROOT).into_iter().filter(|n| dom.tag(*n) == Some("style")).map(|s| dom.plain_text(s).matches('}').count()).sum()
}
