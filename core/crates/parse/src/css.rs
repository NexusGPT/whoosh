//! body-r4: the rules of a message's style blocks, put on the elements they match (`inline_styles`) before the style elements are dropped, which is
//! what keeps a newsletter's look. A small CSS reader of our own (no CSS crate is in the offline registry): rules and declarations, selectors of
//! the kinds mail uses (type, class, id, attribute, the four combinators, `:first-child` `:last-child` `:only-child` `:nth-child` `:first-of-type`
//! `:last-of-type` `:empty` `:root`, `:not(` of one simple selector), the cascade by importance, origin, specificity and order. Everything else is
//! dropped with its rule: `:is(` `:where(` `:has(`, a `:not(` that holds another pseudo, pseudo-elements, states we cannot know (`:hover`), and every
//! at-rule (a media query cannot be judged here). Caps (the brief's): style text, top-level selectors, elements, selectors x elements, nesting depth:
//! past one, nothing is inlined and the result says which (`Inlined::skipped`).
use std::time::Instant;

use crate::dom::{Dom, Id, Kind, ROOT};

pub const STYLE_BYTES_MAX: usize = 256 * 1024;
pub const SELECTORS_MAX: usize = 5_000;
pub const ELEMENTS_MAX: usize = 50_000;
pub const MATCHES_MAX: usize = 5_000_000;
pub const DEPTH_MAX: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Skipped { StyleTooLarge, TooManySelectors, TooManyElements, TooManyMatches, TooDeep, Timeout }

impl Skipped {
    pub fn as_str(self) -> &'static str {
        match self {
            Skipped::StyleTooLarge => "style text over the size cap", Skipped::TooManySelectors => "more selectors than the cap",
            Skipped::TooManyElements => "more elements than the cap", Skipped::TooManyMatches => "selectors x elements over the cap",
            Skipped::TooDeep => "nesting deeper than the cap", Skipped::Timeout => "time limit reached",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Inlined { pub rules: usize, pub skipped: Option<Skipped> }

// ---------------------------------------------------------------- text helpers

/// CSS escapes decoded: `\6f` and `\o` are `o`, a backslash before a newline is nothing.
pub fn unescape(s: &str) -> String {
    if !s.contains('\\') { return s.to_string(); }
    let cs: Vec<char> = s.chars().collect();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < cs.len() {
        if cs[i] != '\\' || i + 1 >= cs.len() { out.push(cs[i]); i += 1; continue; }
        i += 1;
        if cs[i].is_ascii_hexdigit() {
            let mut n = 0u32; let mut k = 0;
            while i < cs.len() && k < 6 && cs[i].is_ascii_hexdigit() { n = n * 16 + cs[i].to_digit(16).unwrap_or(0); i += 1; k += 1; }
            if i < cs.len() && cs[i].is_whitespace() { i += 1; }
            out.push(char::from_u32(n).filter(|c| *c != '\0').unwrap_or('\u{FFFD}'));
        } else if cs[i] == '\n' { i += 1; } else { out.push(cs[i]); i += 1; }
    }
    out
}

fn strip_comments(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(a) = rest.find("/*") {
        out.push_str(&rest[..a]);
        match rest[a + 2..].find("*/") { Some(b) => rest = &rest[a + 2 + b + 2..], None => return out }
    }
    out.push_str(rest);
    out
}

/// Splits at top-level `sep` (not inside quotes, parentheses or brackets).
fn split_top(s: &str, sep: char) -> Vec<String> {
    let (mut out, mut cur) = (Vec::new(), String::new());
    let (mut depth, mut quote) = (0i32, None::<char>);
    let mut esc = false;
    for c in s.chars() {
        if esc { cur.push(c); esc = false; continue; }
        if c == '\\' { cur.push(c); esc = true; continue; }
        if let Some(q) = quote { cur.push(c); if c == q { quote = None; } continue; }
        match c {
            '"' | '\'' => { quote = Some(c); cur.push(c); }
            '(' | '[' => { depth += 1; cur.push(c); }
            ')' | ']' => { depth -= 1; cur.push(c); }
            c if c == sep && depth <= 0 => out.push(std::mem::take(&mut cur)),
            c => cur.push(c),
        }
    }
    out.push(cur);
    out
}

// ---------------------------------------------------------------- declarations

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decl { pub prop: String, pub value: String, pub important: bool }

pub fn parse_declarations(block: &str) -> Vec<Decl> {
    let mut out = Vec::new();
    for item in split_top(&strip_comments(block), ';') {
        let Some((p, v)) = item.split_once(':') else { continue };
        let prop = unescape(p.trim()).to_ascii_lowercase();
        if prop.is_empty() || !prop.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') { continue; }
        let mut value = v.trim().to_string();
        let mut important = false;
        let low = value.to_ascii_lowercase();
        if let Some(at) = low.rfind("!important") { if low[at + 10..].trim().is_empty() { important = true; value = value[..at].trim().to_string(); } }
        if value.is_empty() { continue; }
        out.push(Decl { prop, value, important });
    }
    out
}

/// The properties never kept, by their name without a vendor prefix.
pub fn blocked_property(prop: &str) -> bool {
    let p = ["-webkit-", "-moz-", "-ms-", "-o-"].iter().find_map(|v| prop.strip_prefix(v)).unwrap_or(prop);
    matches!(p, "position" | "float" | "clear" | "clip" | "zoom" | "all" | "paint-order" | "behavior" | "binding")
        || p.starts_with("animation") || p.starts_with("offset") || p.starts_with("scrollbar") || prop == "-moz-binding"
}

/// A value that can fetch something, run something or load another sheet: never kept (a remote image stays replaced; a CSS url() is the same).
pub fn blocked_value(value: &str) -> bool {
    let v = unescape(value).to_ascii_lowercase().replace(char::is_whitespace, "");
    ["url(", "expression(", "javascript:", "vbscript:", "@import", "image-set(", "-moz-binding", "behavior:", "src("].iter().any(|b| v.contains(b))
}

/// A style attribute's declarations with the blocked properties and values removed, `!important` dropped, written `prop:value;`.
/// `body_extra`: also drop width and height and their min- and max- forms (the body element's).
pub fn sanitize_style(style: &str, body: bool) -> String {
    let mut out = String::new();
    for d in parse_declarations(style) {
        if blocked_property(&d.prop) || blocked_value(&d.value) { continue; }
        if body && matches!(d.prop.as_str(), "width" | "height" | "min-width" | "max-width" | "min-height" | "max-height") { continue; }
        out.push_str(&d.prop); out.push(':'); out.push_str(&d.value); out.push(';');
    }
    out
}

// ---------------------------------------------------------------- selectors

#[derive(Debug, Clone, PartialEq)]
enum AttrOp { Exists, Eq, Includes, Dash, Prefix, Suffix, Substr }
#[derive(Debug, Clone, PartialEq)]
struct AttrSel { name: String, op: AttrOp, value: String, ci: bool }
#[derive(Debug, Clone, PartialEq)]
enum Pseudo { FirstChild, LastChild, OnlyChild, Nth(i32, i32), FirstOfType, LastOfType, Empty, Root, Not(Box<Compound>) }
#[derive(Debug, Clone, PartialEq, Default)]
struct Compound { tag: Option<String>, id: Option<String>, classes: Vec<String>, attrs: Vec<AttrSel>, pseudos: Vec<Pseudo> }
#[derive(Debug, Clone, Copy, PartialEq)]
enum Comb { Descendant, Child, Next, Later }
#[derive(Debug, Clone)]
struct Selector { parts: Vec<(Comb, Compound)>, spec: (u32, u32, u32) }

fn ident(cs: &[char], i: &mut usize) -> String {
    let mut s = String::new();
    while *i < cs.len() {
        let c = cs[*i];
        if c == '\\' && *i + 1 < cs.len() { s.push(c); s.push(cs[*i + 1]); *i += 2; continue; }
        if c.is_alphanumeric() || c == '-' || c == '_' || (c as u32) > 0x7F { s.push(c); *i += 1; } else { break; }
    }
    unescape(&s)
}

fn parse_nth(arg: &str) -> Option<(i32, i32)> {
    let a = arg.trim().to_ascii_lowercase().replace(' ', "");
    match a.as_str() { "odd" => return Some((2, 1)), "even" => return Some((2, 0)), _ => {} }
    if let Some(n) = a.find('n') {
        let step = match &a[..n] { "" | "+" => 1, "-" => -1, s => s.parse().ok()? };
        let off = if a[n + 1..].is_empty() { 0 } else { a[n + 1..].parse().ok()? };
        Some((step, off))
    } else { Some((0, a.parse().ok()?)) }
}

fn parse_compound(cs: &[char], i: &mut usize) -> Option<Compound> {
    let mut c = Compound::default();
    let start = *i;
    while *i < cs.len() {
        match cs[*i] {
            '*' => { *i += 1; }
            '.' => { *i += 1; let n = ident(cs, i); if n.is_empty() { return None; } c.classes.push(n); }
            '#' => { *i += 1; let n = ident(cs, i); if n.is_empty() { return None; } c.id = Some(n); }
            '[' => {
                *i += 1;
                let mut body = String::new();
                let mut q = None::<char>;
                while *i < cs.len() { let ch = cs[*i]; if let Some(qq) = q { if ch == qq { q = None; } } else if ch == '"' || ch == '\'' { q = Some(ch); } else if ch == ']' { break; } body.push(ch); *i += 1; }
                if *i >= cs.len() { return None; }
                *i += 1;
                let (name, op, rest) = match body.find(|ch| matches!(ch, '=' | '~' | '|' | '^' | '$' | '*')) {
                    None => (body.trim().to_string(), AttrOp::Exists, String::new()),
                    Some(p) => {
                        let (n, r) = body.split_at(p);
                        let (op, r) = if let Some(r) = r.strip_prefix('=') { (AttrOp::Eq, r) } else {
                            let o = match r.chars().next()? { '~' => AttrOp::Includes, '|' => AttrOp::Dash, '^' => AttrOp::Prefix, '$' => AttrOp::Suffix, '*' => AttrOp::Substr, _ => return None };
                            (o, r[1..].strip_prefix('=')?)
                        };
                        (n.trim().to_string(), op, r.trim().to_string())
                    }
                };
                let mut value = rest.trim().to_string();
                let mut ci = false;
                if let Some(v) = value.strip_suffix(" i").or_else(|| value.strip_suffix(" I")) { ci = true; value = v.trim().to_string(); }
                let value = unescape(value.trim_matches(|ch| ch == '"' || ch == '\''));
                if name.is_empty() { return None; }
                c.attrs.push(AttrSel { name: name.to_ascii_lowercase(), op, value, ci });
            }
            ':' => {
                *i += 1;
                if *i < cs.len() && cs[*i] == ':' { return None; }
                let name = ident(cs, i).to_ascii_lowercase();
                let mut arg = None;
                if *i < cs.len() && cs[*i] == '(' {
                    let mut depth = 0; let mut s = String::new();
                    while *i < cs.len() { let ch = cs[*i]; *i += 1; if ch == '(' { depth += 1; if depth == 1 { continue; } } if ch == ')' { depth -= 1; if depth == 0 { break; } } s.push(ch); }
                    if depth != 0 { return None; }
                    arg = Some(s);
                }
                let p = match (name.as_str(), arg) {
                    ("first-child", None) => Pseudo::FirstChild, ("last-child", None) => Pseudo::LastChild, ("only-child", None) => Pseudo::OnlyChild,
                    ("first-of-type", None) => Pseudo::FirstOfType, ("last-of-type", None) => Pseudo::LastOfType, ("empty", None) => Pseudo::Empty, ("root", None) => Pseudo::Root,
                    ("nth-child", Some(a)) => { let (x, y) = parse_nth(&a)?; Pseudo::Nth(x, y) }
                    ("not", Some(a)) => {
                        // a :not( that holds another pseudo is dropped with its rule
                        if a.contains(':') || a.contains(',') || a.contains(' ') || a.contains('>') || a.contains('+') || a.contains('~') { return None; }
                        let inner: Vec<char> = a.trim().chars().collect();
                        let mut j = 0;
                        let comp = parse_compound(&inner, &mut j)?;
                        if j != inner.len() { return None; }
                        Pseudo::Not(Box::new(comp))
                    }
                    _ => return None,
                };
                c.pseudos.push(p);
            }
            ch if ch.is_alphanumeric() || ch == '-' || ch == '_' || ch == '\\' || (ch as u32) > 0x7F => {
                if *i != start { return None; }
                let n = ident(cs, i);
                if n.is_empty() { return None; }
                c.tag = Some(n.to_ascii_lowercase());
            }
            _ => break,
        }
    }
    if *i == start { None } else { Some(c) }
}

fn parse_selector(text: &str) -> Option<Selector> {
    let cs: Vec<char> = text.trim().chars().collect();
    if cs.is_empty() { return None; }
    let mut parts: Vec<(Comb, Compound)> = Vec::new();
    let mut i = 0;
    let mut comb = Comb::Descendant;
    loop {
        let compound = parse_compound(&cs, &mut i)?;
        parts.push((comb, compound));
        let mut saw_ws = false;
        while i < cs.len() && cs[i].is_whitespace() { saw_ws = true; i += 1; }
        if i >= cs.len() { break; }
        comb = match cs[i] {
            '>' => { i += 1; Comb::Child } '+' => { i += 1; Comb::Next } '~' => { i += 1; Comb::Later }
            _ if saw_ws => Comb::Descendant,
            _ => return None,
        };
        while i < cs.len() && cs[i].is_whitespace() { i += 1; }
        if i >= cs.len() { return None; }
    }
    let mut spec = (0u32, 0u32, 0u32);
    for (_, c) in &parts {
        if c.id.is_some() { spec.0 += 1; }
        spec.1 += (c.classes.len() + c.attrs.len() + c.pseudos.len()) as u32;
        if c.tag.as_deref().is_some_and(|t| t != "*") { spec.2 += 1; }
    }
    Some(Selector { parts, spec })
}

fn elem_siblings(dom: &Dom, id: Id) -> Vec<Id> {
    match dom.nodes[id].parent { Some(p) => dom.nodes[p].children.iter().copied().filter(|c| dom.is_element(*c)).collect(), None => vec![id] }
}

fn compound_matches(dom: &Dom, id: Id, c: &Compound) -> bool {
    let Kind::Element { name, attrs, .. } = &dom.nodes[id].kind else { return false };
    if c.tag.as_deref().is_some_and(|t| t != name) { return false; }
    if let Some(want) = &c.id { if dom.attr(id, "id") != Some(want.as_str()) { return false; } }
    if !c.classes.is_empty() {
        let have = dom.attr(id, "class").unwrap_or("");
        if !c.classes.iter().all(|w| have.split_whitespace().any(|h| h == w)) { return false; }
    }
    for a in &c.attrs {
        let Some((_, v)) = attrs.iter().find(|(k, _)| *k == a.name) else { return false };
        let (v, w) = if a.ci { (v.to_ascii_lowercase(), a.value.to_ascii_lowercase()) } else { (v.clone(), a.value.clone()) };
        let ok = match a.op {
            AttrOp::Exists => true, AttrOp::Eq => v == w, AttrOp::Includes => v.split_whitespace().any(|x| x == w),
            AttrOp::Dash => v == w || v.starts_with(&format!("{w}-")), AttrOp::Prefix => !w.is_empty() && v.starts_with(&w),
            AttrOp::Suffix => !w.is_empty() && v.ends_with(&w), AttrOp::Substr => !w.is_empty() && v.contains(&w),
        };
        if !ok { return false; }
    }
    for p in &c.pseudos {
        let ok = match p {
            Pseudo::FirstChild => elem_siblings(dom, id).first() == Some(&id),
            Pseudo::LastChild => elem_siblings(dom, id).last() == Some(&id),
            Pseudo::OnlyChild => elem_siblings(dom, id).len() == 1,
            Pseudo::Nth(a, b) => {
                let n = elem_siblings(dom, id).iter().position(|x| *x == id).map_or(0, |p| p as i32 + 1);
                if *a == 0 { n == *b } else { let d = n - *b; d % *a == 0 && d / *a >= 0 }
            }
            Pseudo::FirstOfType => elem_siblings(dom, id).into_iter().find(|s| dom.tag(*s) == dom.tag(id)) == Some(id),
            Pseudo::LastOfType => elem_siblings(dom, id).into_iter().rev().find(|s| dom.tag(*s) == dom.tag(id)) == Some(id),
            Pseudo::Empty => dom.nodes[id].children.iter().all(|c| matches!(&dom.nodes[*c].kind, Kind::Comment) || matches!(&dom.nodes[*c].kind, Kind::Text(t) if t.is_empty())),
            Pseudo::Root => dom.tag(id) == Some("html"),
            Pseudo::Not(inner) => !compound_matches(dom, id, inner),
        };
        if !ok { return false; }
    }
    true
}

fn matches_from(dom: &Dom, id: Id, parts: &[(Comb, Compound)], idx: usize) -> bool {
    if !compound_matches(dom, id, &parts[idx].1) { return false; }
    if idx == 0 { return true; }
    match parts[idx].0 {
        Comb::Child => dom.nodes[id].parent.is_some_and(|p| dom.is_element(p) && matches_from(dom, p, parts, idx - 1)),
        Comb::Descendant => { let (mut a, mut steps) = (dom.nodes[id].parent, 0usize); while let Some(p) = a { if dom.is_element(p) && matches_from(dom, p, parts, idx - 1) { return true; } a = dom.nodes[p].parent; steps += 1; if steps > dom.nodes.len() { break; } } false }
        Comb::Next => elem_siblings(dom, id).iter().position(|x| *x == id).and_then(|p| p.checked_sub(1)).is_some_and(|q| matches_from(dom, elem_siblings(dom, id)[q], parts, idx - 1)),
        Comb::Later => { let sib = elem_siblings(dom, id); let p = sib.iter().position(|x| *x == id).unwrap_or(0); sib[..p].iter().any(|s| matches_from(dom, *s, parts, idx - 1)) }
    }
}

/// Does `selector` (CSS text) match the element `id`? None when the selector is one we do not read.
pub fn selector_matches(dom: &Dom, id: Id, selector: &str) -> Option<bool> {
    let s = parse_selector(selector)?;
    Some(matches_from(dom, id, &s.parts, s.parts.len() - 1))
}

/// The same selector parsed once, for a table of selectors matched against many elements.
pub struct Matcher(Selector);
impl Matcher {
    pub fn new(selector: &str) -> Option<Matcher> { parse_selector(selector).map(Matcher) }
    pub fn matches(&self, dom: &Dom, id: Id) -> bool { matches_from(dom, id, &self.0.parts, self.0.parts.len() - 1) }
}

// ---------------------------------------------------------------- stylesheet

struct Rule { sel: Selector, decls: Vec<Decl>, order: usize }

/// The rules of one sheet's text. Returns the rules and how many selectors the sheet names (read or not).
fn parse_sheet(text: &str, order0: usize) -> (Vec<Rule>, usize) {
    let t = strip_comments(text);
    let cs: Vec<char> = t.chars().collect();
    let (mut rules, mut count) = (Vec::new(), 0usize);
    let mut i = 0;
    while i < cs.len() {
        while i < cs.len() && cs[i].is_whitespace() { i += 1; }
        if i >= cs.len() { break; }
        let start = i;
        if cs[i] == '@' {
            // an at-rule: up to its `;` or through its block
            let mut depth = 0; let mut q = None::<char>;
            while i < cs.len() {
                let c = cs[i]; i += 1;
                if let Some(qq) = q { if c == qq { q = None; } continue; }
                match c { '"' | '\'' => q = Some(c), '{' => depth += 1, '}' => { depth -= 1; if depth <= 0 { break; } } ';' if depth == 0 => break, _ => {} }
            }
            continue;
        }
        let mut q = None::<char>;
        while i < cs.len() && !(cs[i] == '{' && q.is_none()) { if let Some(qq) = q { if cs[i] == qq { q = None; } } else if cs[i] == '"' || cs[i] == '\'' { q = Some(cs[i]); } i += 1; }
        let selectors: String = cs[start..i.min(cs.len())].iter().collect();
        if i >= cs.len() { break; }
        i += 1;
        let bstart = i;
        let mut q = None::<char>;
        while i < cs.len() && !(cs[i] == '}' && q.is_none()) { if let Some(qq) = q { if cs[i] == qq { q = None; } } else if cs[i] == '"' || cs[i] == '\'' { q = Some(cs[i]); } i += 1; }
        let block: String = cs[bstart..i.min(cs.len())].iter().collect();
        i += 1;
        let decls = parse_declarations(&block);
        for s in split_top(&selectors, ',') {
            count += 1;
            if let Some(sel) = parse_selector(&s) { rules.push(Rule { sel, decls: decls.clone(), order: order0 + rules.len() }); }
        }
    }
    (rules, count)
}

fn check(deadline: Option<Instant>) -> bool { deadline.is_some_and(|d| Instant::now() >= d) }

/// Puts the rules of the document's `<style>` elements on the elements they match, merged with each element's own `style`. The style elements
/// stay where they are (the cleaner drops them). Past a cap, or the deadline, nothing is changed and `skipped` says which.
pub fn inline_styles(dom: &mut Dom, deadline: Option<Instant>) -> Inlined {
    let all = dom.descendants(ROOT);
    let elements = all.iter().filter(|n| dom.is_element(**n)).count();
    let styles: Vec<Id> = all.iter().copied().filter(|n| dom.tag(*n) == Some("style")).collect();
    if styles.is_empty() { return Inlined::default(); }
    let mut text = String::new();
    for s in &styles {
        let media = dom.attr(*s, "media").unwrap_or("").trim().to_ascii_lowercase();
        if !media.is_empty() && media != "all" && media != "screen" { continue; }
        let t = dom.plain_text(*s);
        if text.len() + t.len() > STYLE_BYTES_MAX { return Inlined { rules: 0, skipped: Some(Skipped::StyleTooLarge) }; }
        text.push_str(&t); text.push('\n');
    }
    if elements > ELEMENTS_MAX { return Inlined { rules: 0, skipped: Some(Skipped::TooManyElements) }; }
    if all.iter().any(|n| dom.depth(*n) > DEPTH_MAX) { return Inlined { rules: 0, skipped: Some(Skipped::TooDeep) }; }
    let (rules, selectors) = parse_sheet(&text, 0);
    if selectors > SELECTORS_MAX { return Inlined { rules: 0, skipped: Some(Skipped::TooManySelectors) }; }
    if selectors.saturating_mul(elements) > MATCHES_MAX { return Inlined { rules: 0, skipped: Some(Skipped::TooManyMatches) }; }
    let mut edits: Vec<(Id, String)> = Vec::new();
    for (n, id) in all.iter().enumerate() {
        if n % 256 == 0 && check(deadline) { return Inlined { rules: 0, skipped: Some(Skipped::Timeout) }; }
        if !dom.is_element(*id) { continue; }
        // (important, origin, specificity, order, declaration): inline beats the sheet within one importance
        let mut found: Vec<(bool, u8, (u32, u32, u32), usize, &Decl)> = Vec::new();
        for r in &rules {
            let last = r.sel.parts.len() - 1;
            if matches_from(dom, *id, &r.sel.parts, last) { for d in &r.decls { found.push((d.important, 0, r.sel.spec, r.order, d)); } }
        }
        let own = dom.attr(*id, "style").map(parse_declarations).unwrap_or_default();
        if found.is_empty() && own.is_empty() { continue; }
        for d in &own { found.push((d.important, 1, (0, 0, 0), 0, d)); }
        found.sort_by(|a, b| (a.0, a.1, a.2, a.3).cmp(&(b.0, b.1, b.2, b.3)));
        let mut merged: Vec<(String, String)> = Vec::new();
        for (_, _, _, _, d) in found {
            match merged.iter_mut().find(|(p, _)| *p == d.prop) { Some(m) => m.1 = d.value.clone(), None => merged.push((d.prop.clone(), d.value.clone())) }
        }
        let style: String = merged.iter().map(|(p, v)| format!("{p}:{v};")).collect();
        edits.push((*id, style));
    }
    let n = edits.len();
    for (id, style) in edits { dom.set_attr(id, "style", style); }
    Inlined { rules: rules.len().max(n.min(1)), skipped: None }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn style_of(dom: &Dom, tag: &str) -> String { let id = dom.find(ROOT, tag).unwrap(); dom.attr(id, "style").unwrap_or("").to_string() }

    #[test]
    fn a_rule_lands_on_what_it_matches_with_specificity_importance_and_the_inline_style_in_order() {
        let mut d = Dom::parse("<style>p{color:red;margin:0} .x{color:blue} #a{color:green} p{color:black!important} div > p.x{padding:1px}</style><div><p id=a class=x style='color:pink;font-size:9px'>t</p></div>");
        let r = inline_styles(&mut d, None);
        assert_eq!(r.skipped, None);
        let s = style_of(&d, "p");
        assert!(s.contains("color:black"), "the important rule wins: {s}");
        assert!(s.contains("margin:0") && s.contains("font-size:9px") && s.contains("padding:1px"), "{s}");
        assert_eq!(s.matches("color:").count(), 1, "one colour: {s}");
    }

    #[test]
    fn selectors_of_the_kinds_mail_uses_match_and_the_others_do_not() {
        let d = Dom::parse("<table class=a><tr><td>1</td><td lang=fr-BE>2</td><td>3</td></tr></table>");
        let td = d.descendants(ROOT).into_iter().filter(|n| d.tag(*n) == Some("td")).collect::<Vec<_>>();
        assert_eq!(selector_matches(&d, td[1], "table.a td:nth-child(2)"), Some(true));
        assert_eq!(selector_matches(&d, td[0], "td:first-child"), Some(true));
        assert_eq!(selector_matches(&d, td[2], "td:last-child"), Some(true));
        assert_eq!(selector_matches(&d, td[1], "td[lang|=fr]"), Some(true));
        assert_eq!(selector_matches(&d, td[2], "td + td + td"), Some(true));
        assert_eq!(selector_matches(&d, td[0], "td:not(.q)"), Some(true));
        assert_eq!(selector_matches(&d, td[0], "td:nth-child(odd)"), Some(true));
        assert_eq!(selector_matches(&d, td[1], "td:nth-child(odd)"), Some(false));
        for dropped in [":is(td, th)", "td:where(.a)", "td:has(b)", "td:not(:first-child)", "td:hover", "td::before", "td:not(a, b)"] {
            assert_eq!(selector_matches(&d, td[0], dropped), None, "{dropped}");
        }
    }

    #[test]
    fn blocked_properties_and_values_are_never_kept_even_when_written_with_escapes() {
        let s = sanitize_style("color:red; p\\osition:fixed; FLOAT:left; clear:both; clip:rect(0,0,0,0); zoom:2; all:unset; animation-name:x; -webkit-animation:y; offset-path:none; scrollbar-width:none; paint-order:fill; background:u\\72l(http://t.example/p.gif); width:5px; behavior:url(x)", false);
        assert_eq!(s, "color:red;width:5px;");
        assert_eq!(sanitize_style("width:600px;height:9px;min-width:1px;max-height:2px;color:red", true), "color:red;");
    }

    #[test]
    fn a_cap_inlines_nothing_and_says_which() {
        let big = format!("<style>{}</style><p>t</p>", "p{color:red} ".repeat(30_000));
        assert_eq!(inline_styles(&mut Dom::parse(&big), None).skipped, Some(Skipped::StyleTooLarge));
        let many = format!("<style>{}</style><p>t</p>", (0..5_001).map(|i| format!(".c{i}{{color:red}}")).collect::<String>());
        assert_eq!(inline_styles(&mut Dom::parse(&many), None).skipped, Some(Skipped::TooManySelectors));
        let mut d = Dom::parse("<style>p{color:red}</style><p>t</p>");
        assert_eq!(inline_styles(&mut d, Some(Instant::now())).skipped, Some(Skipped::Timeout));
        assert_eq!(style_of(&d, "p"), "");
    }
}
