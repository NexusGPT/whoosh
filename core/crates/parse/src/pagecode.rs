//! Page code where a mail's text belongs, and tags a sender's template cut short (owner's picture 216, 14 Nov 2026: a support desk's
//! notice carried its page code in the text part, and one tag of that code stopped in the middle of a word, its `>` never written. The
//! sentence after it was read as the tag's attributes and drew nowhere; shown as text, the part was a wall of code). Text work only: no
//! parser runs here, and each function reads its input once.
use std::borrow::Cow;

/// `PARSE_TEXT_PAGE_CODE_MIN_TAGS`: a text part that starts with a tag of a page element and holds this many such tags, one of them a
/// closing tag, is page code, not the sender's words.
pub const TEXT_PAGE_CODE_MIN_TAGS: usize = 4;

/// `PARSE_PAGE_HOLDS_TEXT_PERCENT`: a page holds what the mail says when it holds this share of the text part's words.
pub const PAGE_HOLDS_TEXT_PERCENT: usize = 75;

/// The elements a mail's page is written with.
const ELEMENTS: &[&str] = &[
    "a", "b", "blockquote", "body", "br", "center", "div", "em", "font", "h1", "h2", "h3", "h4", "h5", "h6", "head", "hr", "html", "i", "img", "li", "link",
    "meta", "ol", "p", "pre", "span", "strong", "style", "table", "tbody", "td", "th", "thead", "title", "tr", "u", "ul",
];
/// Attributes that stand alone in a tag. In a cut tag, a bare word that is none of them is the sender's text.
const ALONE: &[&str] = &[
    "nowrap", "hidden", "disabled", "checked", "selected", "readonly", "multiple", "noshade", "compact", "ismap", "open", "required", "border", "download",
    "novalidate", "async", "defer", "autofocus", "controls", "loop", "muted", "autoplay", "reversed", "itemscope", "default", "noresize", "nohref", "declare",
];
/// The attribute names a cut can stop inside (`cellpa` of `cellpadding`).
const ATTRIBUTES: &[&str] = &[
    "align", "alt", "bgcolor", "border", "cellpadding", "cellspacing", "class", "color", "colspan", "dir", "height", "href", "id", "lang", "name", "rel", "role",
    "rowspan", "src", "style", "target", "title", "type", "valign", "width",
];
/// Attributes whose value is free text: a tag written inside one is the sender's own.
const FREE_TEXT: &[&str] = &["alt", "title", "content", "value", "placeholder", "srcdoc", "label", "summary"];
/// Words of a style value that are not the sender's text.
const STYLE_WORDS: &[&str] = &[
    "auto", "none", "solid", "normal", "bold", "center", "left", "right", "top", "middle", "bottom", "block", "inline", "inline-block", "collapse", "inherit",
    "hidden", "break-word", "sans-serif", "serif", "!important", "important",
];
/// Elements whose content is not page code: nothing inside is read as a tag.
const RAW: &[&str] = &["script", "style", "textarea", "title", "xmp"];

fn space(c: u8) -> bool { matches!(c, b' ' | b'\t' | b'\r' | b'\n' | 0x0c) }

/// The page element a tag at `lt` names: (closing tag, index after the name). None when `lt` starts no tag of a page element.
fn element_at(s: &str, lt: usize) -> Option<(bool, usize)> {
    let b = s.as_bytes();
    if b.get(lt) != Some(&b'<') { return None; }
    let close = b.get(lt + 1) == Some(&b'/');
    let from = if close { lt + 2 } else { lt + 1 };
    let mut e = from;
    while e < b.len() && b[e].is_ascii_alphanumeric() { e += 1; }
    // the name ends at a space, `>` or `/`, or with the text: `<broken` in the middle of a sentence names nothing
    if e == from || !b.get(e).is_none_or(|c| space(*c) || matches!(c, b'>' | b'/')) { return None; }
    ELEMENTS.contains(&s[from..e].to_ascii_lowercase().as_str()).then_some((close, e))
}

/// True when a text part is page code: it starts with a tag of a page element (a doctype or a comment may come first) and holds
/// TEXT_PAGE_CODE_MIN_TAGS such tags or more, one of them a closing tag. A text that starts with anything else is not read further.
pub fn text_is_page_code(text: &str) -> bool {
    let t = text.trim_start();
    if !t.starts_with('<') { return false; }
    let mut at = 0usize;
    loop {
        let rest = &t[at..];
        let b = rest.as_bytes();
        let step = if b.len() >= 9 && b[..9].eq_ignore_ascii_case(b"<!doctype") { rest.find('>').map_or(rest.len(), |x| x + 1) }
            else if rest.starts_with("<!--") { rest.find("-->").map_or(rest.len(), |x| x + 3) }
            else { break };
        at += step;
        at += t[at..].len() - t[at..].trim_start().len();
    }
    if !matches!(element_at(t, at), Some((false, _))) { return false; }
    let b = t.as_bytes();
    let (mut tags, mut closing, mut i) = (0usize, false, at);
    while i < b.len() {
        if b[i] == b'<' {
            if let Some((close, e)) = element_at(t, i) {
                tags += 1;
                closing |= close;
                if tags >= TEXT_PAGE_CODE_MIN_TAGS && closing { return true; }
                i = e;
                continue;
            }
        }
        i += 1;
    }
    false
}

/// True when a text starts with source, not words: a doctype, a comment or the tag of a page element (a row's preview is too short to
/// hold TEXT_PAGE_CODE_MIN_TAGS tags).
pub fn starts_with_page_tag(text: &str) -> bool {
    let t = text.trim_start();
    let b = t.as_bytes();
    (b.len() >= 9 && b[..9].eq_ignore_ascii_case(b"<!doctype")) || t.starts_with("<!--") || matches!(element_at(t, 0), Some((false, _)))
}

/// The last place in `word` where a small letter meets a capital: where a sentence was written onto the end of cut code (`presentatioIf`).
fn seam(word: &str) -> Option<usize> {
    let mut at = None;
    let mut prev: Option<char> = None;
    for (i, c) in word.char_indices() {
        if prev.is_some_and(|p| p.is_lowercase()) && c.is_uppercase() { at = Some(i); }
        prev = Some(c);
    }
    at
}

fn starts_small(word: &str) -> bool { word.chars().next().is_some_and(|c| c.is_lowercase()) }

/// What a start tag is, read from right after its name.
enum Tag {
    /// A whole tag; the index after its `>`.
    Whole(usize),
    /// A tag its sender never ended: the code stops at `tag_end`, the sender's text starts at `text_from`.
    Cut { tag_end: usize, text_from: usize },
}

/// Reads the attributes of a start tag. A tag is cut when another tag (or the end of the text) comes before its `>`, outside a quoted
/// value, or when a quoted value that is not free text holds a tag of a page element.
fn read_tag(s: &str, from: usize) -> Tag {
    let b = s.as_bytes();
    // each attribute read so far: (start, end, a bare word that is no attribute, the start of an unquoted value)
    let mut read: Vec<(usize, usize, bool, Option<usize>)> = Vec::new();
    let mut i = from;
    let stop = loop {
        while i < b.len() && space(b[i]) { i += 1; }
        if i >= b.len() { break b.len(); }
        match b[i] {
            b'>' => return Tag::Whole(i + 1),
            b'<' => break i,
            b'/' | b'=' => { i += 1; continue; }
            _ => {}
        }
        let start = i;
        while i < b.len() && !space(b[i]) && !matches!(b[i], b'=' | b'>' | b'<') { i += 1; }
        if b.get(i) == Some(&b'>') && b[i - 1] == b'/' && i - 1 > start { i -= 1; }   // `<br/>`: the slash ends the tag
        let name_end = i;
        let name = s[start..name_end].to_ascii_lowercase();
        let mut j = i;
        while j < b.len() && space(b[j]) { j += 1; }
        if b.get(j) != Some(&b'=') {
            let alone = name.bytes().all(|c| c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_' | b':' | b'.'))
                && (ALONE.contains(&name.as_str()) || name.starts_with("data-") || name.starts_with("aria-"));
            read.push((start, name_end, !alone, None));
            continue;
        }
        j += 1;
        while j < b.len() && space(b[j]) { j += 1; }
        match b.get(j).copied() {
            Some(q) if q == b'"' || q == b'\'' => {
                let end = s[j + 1..].find(q as char).map_or(b.len(), |x| j + 1 + x);
                let free = FREE_TEXT.contains(&name.as_str()) || name.starts_with("data-") || name.starts_with("aria-");
                // a tag of a page element inside the value: the value was never ended (the quote that ends it belongs to a later tag)
                let inner = if free { None } else { (j + 1..end).find(|k| b[*k] == b'<' && element_at(s, *k).is_some()) };
                match inner {
                    None if end < b.len() => { i = end + 1; read.push((start, i, false, None)); }
                    None => return Tag::Cut { tag_end: start, text_from: text_in_value(s, j + 1, b.len()) },
                    Some(k) => return Tag::Cut { tag_end: start, text_from: text_in_value(s, j + 1, k) },
                }
            }
            _ => {
                let v = j;
                while j < b.len() && !space(b[j]) && !matches!(b[j], b'>' | b'<') { j += 1; }
                i = j;
                read.push((start, i, false, Some(v)));
            }
        }
    };
    // the first bare word that is no attribute starts the sender's text
    let Some(k) = read.iter().position(|r| r.2) else {
        return Tag::Cut { tag_end: read.last().map_or(from, |r| r.1), text_from: stop };
    };
    let (start, end, _, _) = read[k];
    let word = &s[start..end];
    let next_small = read.get(k + 1).is_some_and(|r| starts_small(&s[r.0..r.1]));
    // the sentence was written onto the end of a cut attribute name: `cellpaIf you're`
    if let Some(p) = seam(word) {
        let left = word[..p].to_ascii_lowercase();
        if next_small && left.len() >= 2 && ATTRIBUTES.iter().any(|a| a.starts_with(&left)) {
            return Tag::Cut { tag_end: read[..k].last().map_or(from, |r| r.1), text_from: start + p };
        }
    }
    // or onto the end of a cut value: `role=presentatioIf you're`
    if starts_small(word) && k > 0 {
        if let (_, vend, _, Some(v)) = read[k - 1] {
            if let Some(p) = seam(&s[v..vend]) { return Tag::Cut { tag_end: v + p, text_from: v + p }; }
        }
    }
    Tag::Cut { tag_end: read[..k].last().map_or(from, |r| r.1), text_from: start }
}

/// Where the sender's text starts in a quoted value that was never ended (`from..to` of `s`): after the pieces of style.
fn text_in_value(s: &str, from: usize, to: usize) -> usize {
    let b = s.as_bytes();
    let mut words: Vec<(usize, usize)> = Vec::new();
    let mut i = from;
    while i < to {
        while i < to && space(b[i]) { i += 1; }
        let start = i;
        while i < to && !space(b[i]) { i += 1; }
        if i > start { words.push((start, i)); }
    }
    let style = |w: &str| {
        w.contains([':', ';', '=', '#', '{', '}', '(', ')', '%']) || w.chars().next().is_some_and(|c| c.is_ascii_digit()) || STYLE_WORDS.contains(&w.to_ascii_lowercase().as_str())
    };
    let Some(k) = words.iter().position(|(a, z)| !style(&s[*a..*z])) else { return to };
    let (start, end) = words[k];
    if starts_small(&s[start..end]) && k > 0 {
        let (a, z) = words[k - 1];
        if let Some(p) = seam(&s[a..z]) { return a + p; }
    }
    start
}

/// The page with every cut tag ended where its code stops, so the sentence written after it is text again. A page with no cut tag is
/// returned as it came (nothing is copied). Comments, the doctype, closing tags and the content of style and script are not read.
pub fn close_cut_tags(html: &str) -> Cow<'_, str> {
    let b = html.as_bytes();
    let mut out: Option<String> = None;
    let (mut copied, mut i) = (0usize, 0usize);
    while i < b.len() {
        if b[i] != b'<' { i += 1; continue; }
        if html[i..].starts_with("<!--") { i = html[i + 4..].find("-->").map_or(b.len(), |x| i + 4 + x + 3); continue; }
        let Some(c) = b.get(i + 1).copied() else { break };
        if matches!(c, b'/' | b'!' | b'?') { i = html[i..].find('>').map_or(b.len(), |x| i + x + 1); continue; }
        if !c.is_ascii_alphabetic() { i += 1; continue; }
        let mut e = i + 1;
        while e < b.len() && (b[e].is_ascii_alphanumeric() || matches!(b[e], b':' | b'-')) { e += 1; }
        let name = html[i + 1..e].to_ascii_lowercase();
        match read_tag(html, e) {
            Tag::Whole(end) => {
                i = end;
                if RAW.contains(&name.as_str()) {
                    let low = html[i..].to_ascii_lowercase();
                    i = low.find(&format!("</{name}")).map_or(b.len(), |x| i + x);
                }
            }
            Tag::Cut { tag_end, text_from } => {
                let o = out.get_or_insert_with(|| String::with_capacity(html.len() + 16));
                o.push_str(html[copied..tag_end].trim_end());   // the tag starts with its name: only the space before a dropped attribute goes
                o.push('>');
                copied = text_from;
                i = text_from.max(i + 1);
            }
        }
    }
    match out {
        None => Cow::Borrowed(html),
        Some(mut o) => { o.push_str(&html[copied..]); Cow::Owned(o) }
    }
}

/// A text part of page code, as the page it is: its cut tags ended, and the plain lines after its last tag (a rule of dashes, a line of
/// the sender's system) written as paragraphs.
pub fn page_code_as_page(text: &str) -> String {
    let fixed = close_cut_tags(text);
    match fixed.rfind('>') {
        Some(gt) if !fixed[gt + 1..].trim().is_empty() => format!("{}{}", &fixed[..=gt], crate::display::plain_to_html(&fixed[gt + 1..].replace('\r', ""))),
        _ => fixed.into_owned(),
    }
}

/// The words of a page (`page_code_as_page`), as the text part should have held them: a line per block, no tag.
pub fn page_code_words(page: &str) -> String {
    crate::sanitise::safe_html_to_text(&crate::sanitise::sanitise_html(page).html)
}

/// The two renderings of a mail put right: the cut tags of the page ended; a text part of page code read as its words, and as the page
/// itself when the mail has no page.
pub fn right_parts(text: Option<String>, html: Option<String>) -> (Option<String>, Option<String>) {
    let html = html.map(|h| match close_cut_tags(&h) { Cow::Owned(o) => o, Cow::Borrowed(_) => h.clone() });
    match text {
        Some(t) if text_is_page_code(&t) => {
            let page = page_code_as_page(&t);
            let words = page_code_words(&page);
            let html = if html.as_deref().is_some_and(|h| !h.trim().is_empty()) { html } else { Some(page) };
            (Some(words), html)
        }
        t => (t, html),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const WHOLE: &str = r#"<div style="margin:0 auto;max-width:400px"><table border=0 cellpadding=0 role=presentation><tr><td nowrap style=font-size:0>Hello <a href="https://example.com/a?b=1&c=2">there</a><br/>Bye</td></tr></table></div>"#;

    #[test]
    fn a_page_with_no_cut_tag_is_returned_as_it_came() {
        assert!(matches!(close_cut_tags(WHOLE), Cow::Borrowed(_)));
        let more = "<!--[if mso]><table><tr><td><![endif]--><p>a < b and c > d, 1 <3</p><style>p > a { color: red }</style><script>if (a<b) { x() }</script><img alt=\"<b>Sale</b>\" src=x.png>";
        assert!(matches!(close_cut_tags(more), Cow::Borrowed(_)));
    }

    #[test]
    fn a_tag_cut_in_an_unquoted_value_gives_the_sentence_back_whole() {
        let cut = "<div style=font-size:0><table border=0 cellpadding=0 cellspacing=0 role=presentatioIf you're a customer, write to us.<br><br>Thanks,<br>The team</div>";
        assert_eq!(close_cut_tags(cut), "<div style=font-size:0><table border=0 cellpadding=0 cellspacing=0 role=presentatio>If you're a customer, write to us.<br><br>Thanks,<br>The team</div>");
    }

    #[test]
    fn a_tag_cut_in_an_attribute_name_drops_the_piece_of_name() {
        let cut = "<table border=0 cellpaIf you're a customer, write to us.<br>Thanks";
        assert_eq!(close_cut_tags(cut), "<table border=0>If you're a customer, write to us.<br>Thanks");
    }

    #[test]
    fn a_tag_cut_right_before_the_next_tag_or_the_end_is_ended_there() {
        assert_eq!(close_cut_tags("<p>One</p><table border=0 role=presentatio<br>Two"), "<p>One</p><table border=0 role=presentatio><br>Two");
        assert_eq!(close_cut_tags("<p>One</p><table<br>Two"), "<p>One</p><table><br>Two");
        assert_eq!(close_cut_tags("<p>One</p><table border=0 role=presentatio"), "<p>One</p><table border=0 role=presentatio>");
    }

    #[test]
    fn a_tag_cut_before_a_sentence_keeps_every_word_of_it() {
        let cut = "<td align=left Hello Maria, your parcel is on its way.<br>See you soon</td>";
        assert_eq!(close_cut_tags(cut), "<td align=left>Hello Maria, your parcel is on its way.<br>See you soon</td>");
        // a stand-alone attribute before the words stays in the tag
        assert_eq!(close_cut_tags("<td nowrap Hello Maria.<br>Bye</td>"), "<td nowrap>Hello Maria.<br>Bye</td>");
    }

    #[test]
    fn a_tag_cut_inside_a_quoted_value_gives_the_sentence_back() {
        let cut = "<div style=\"margin:0 auto;max-wIf you're a customer, write to us.<br><a href=\"https://example.com\">Help</a></div>";
        assert_eq!(close_cut_tags(cut), "<div>If you're a customer, write to us.<br><a href=\"https://example.com\">Help</a></div>");
        let cut = "<div class=\"a\" style=\"margin: 0 auto Hello Maria.<br><b>Bye</b></div>";
        assert_eq!(close_cut_tags(cut), "<div class=\"a\">Hello Maria.<br><b>Bye</b></div>");
    }

    #[test]
    fn page_code_is_told_from_text_that_names_a_tag() {
        assert!(text_is_page_code("<div style=background-color:#fff><div><table border=0><tr><td>Hello</td></tr></table></div></div>\r\n\r\n--------\r\nA line."));
        assert!(text_is_page_code("  <!DOCTYPE html>\n<html><body><p>Hello</p></body></html>"));
        assert!(text_is_page_code("<p data-x=\"1\" style=\"margin: 0;\"><span>Dear customer</span>,<br><br>One line.</p>"));
        for plain in [
            "Hello,\n\nUse <b> for bold and </b> to end it. <div><table><tr><td>",
            "<maria@example.com> wrote:\n> Hello <b>you</b></b></b></b>",
            "<https://example.com/a> is the link. <p><p><p></p>",
            "< 5 people came <br><br><br></p>",
            "<br>",
            "<p>One</p>",
            "",
        ] { assert!(!text_is_page_code(plain), "{plain:?}"); }
        for source in ["<!DOCTYPE html PUBLIC \"-//W3C//DTD", " <div style=background-color:#fff><div", "<p data-x=\"1\">Dear", "<!-- a note --><table"] { assert!(starts_with_page_tag(source), "{source:?}"); }
        for words in ["<maria@example.com> wrote:", "<3 thank you", "<https://example.com/a>", "Hello <b>", "<broken", ""] { assert!(!starts_with_page_tag(words), "{words:?}"); }
    }

    #[test]
    fn page_code_becomes_a_page_and_its_words() {
        let text = "<div style=background-color:#fff><div style=font-size:0><table border=0 role=presentatioIf you're a customer, write to us.<br><br>Visit our <a href=https://example.com/help>Help page</a>.<br><br>Thanks,<br>The team</div></table></div>\r\n\r\n--------------------------------\r\nThis is a service line.\r\n\r\n\r\n[AB12CD-EFGH]";
        assert!(text_is_page_code(text));
        let page = page_code_as_page(text);
        assert!(page.contains("role=presentatio>If you're a customer"), "{page}");
        assert!(page.ends_with("<hr><p>This is a service line.</p><p>[AB12CD-EFGH]</p>"), "{page}");
        let words = page_code_words(&page);
        assert!(!words.contains('<') && !words.contains("role="), "{words}");
        for said in ["If you're a customer, write to us.", "Visit our Help page.", "Thanks,", "The team", "This is a service line.", "[AB12CD-EFGH]"] { assert!(words.contains(said), "{said:?} not in {words:?}"); }
        // the mail has no page: the text part is the page; it has one: that one stays
        let (t, h) = right_parts(Some(text.to_string()), None);
        assert_eq!((t.as_deref(), h.as_deref()), (Some(words.as_str()), Some(page.as_str())));
        let (t, h) = right_parts(Some(text.to_string()), Some("<p>The page</p>".into()));
        assert_eq!((t.as_deref(), h.as_deref()), (Some(words.as_str()), Some("<p>The page</p>")));
        // plain words stay what they are
        let (t, h) = right_parts(Some("Hello <b>".into()), None);
        assert_eq!((t.as_deref(), h), (Some("Hello <b>"), None));
    }
}
