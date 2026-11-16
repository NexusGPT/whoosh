//! body-r4: a small arena DOM over html5ever (the HTML living standard's own parser, already a dependency of ammonia), so the segmenter can
//! read quote structure (blockquote ancestry, client container classes), the style inliner can match selectors, and the display builder can
//! drop what must never be kept, all before ammonia's allow-list has the last word. Nodes are indices; nothing here recurses on the tree's
//! depth (a hostile message may nest 100,000 elements): walks are iterative, the serialiser flattens what is deeper than `max_depth`.
use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};

use html5ever::interface::tree_builder::{ElemName, ElementFlags, NodeOrText, QuirksMode, TreeSink};
use html5ever::tendril::{StrTendril, TendrilSink};
use html5ever::{parse_document, Attribute, LocalName, Namespace, QualName};

pub type Id = usize;

#[derive(Debug, Clone)]
pub enum Kind {
    Document,
    Text(String),
    Comment,
    /// `html`: false for an SVG or MathML element (never kept).
    Element { name: String, html: bool, attrs: Vec<(String, String)> },
    Other,
}

#[derive(Debug, Clone)]
pub struct Node { pub kind: Kind, pub parent: Option<Id>, pub children: Vec<Id> }

#[derive(Debug, Default)]
pub struct Dom { pub nodes: Vec<Node> }

pub const ROOT: Id = 0;

struct Name(QualName);
impl std::fmt::Debug for Name { fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { write!(f, "{}", self.0.local) } }
impl ElemName for Name {
    fn ns(&self) -> &Namespace { &self.0.ns }
    fn local_name(&self) -> &LocalName { &self.0.local }
}

#[derive(Default)]
struct Sink { nodes: RefCell<Vec<Node>>, names: RefCell<HashMap<Id, QualName>>, templates: RefCell<HashMap<Id, Id>>, annotation: RefCell<HashSet<Id>> }

impl Sink {
    fn add(&self, kind: Kind) -> Id {
        let mut n = self.nodes.borrow_mut();
        n.push(Node { kind, parent: None, children: Vec::new() });
        n.len() - 1
    }
    fn detach(&self, id: Id) {
        let mut n = self.nodes.borrow_mut();
        if let Some(p) = n[id].parent.take() { n[p].children.retain(|c| *c != id); }
    }
    fn place(&self, parent: Id, at: Option<usize>, child: NodeOrText<Id>) {
        // never make a cycle: a node is not placed under itself or under one of its own descendants
        if let NodeOrText::AppendNode(id) = &child {
            let n = self.nodes.borrow();
            let (mut a, mut steps) = (Some(parent), 0usize);
            while let Some(x) = a { if x == *id || steps > n.len() { return; } a = n[x].parent; steps += 1; }
        }
        let node = match child {
            NodeOrText::AppendText(t) => {
                let mut n = self.nodes.borrow_mut();
                let before = match at { Some(i) => i.checked_sub(1).map(|j| n[parent].children[j]), None => n[parent].children.last().copied() };
                if let Some(b) = before { if let Kind::Text(s) = &mut n[b].kind { s.push_str(&t); return; } }
                drop(n);
                self.add(Kind::Text(t.to_string()))
            }
            NodeOrText::AppendNode(id) => id,
        };
        self.detach(node);
        let mut n = self.nodes.borrow_mut();
        n[node].parent = Some(parent);
        let len = n[parent].children.len();
        match at { Some(i) => n[parent].children.insert(i.min(len), node), None => n[parent].children.push(node) }
    }
}

impl TreeSink for Sink {
    type Handle = Id;
    type Output = Dom;
    type ElemName<'a> = Name;

    fn finish(self) -> Dom { Dom { nodes: self.nodes.into_inner() } }
    fn parse_error(&self, _: Cow<'static, str>) {}
    fn get_document(&self) -> Id { ROOT }
    fn set_quirks_mode(&self, _: QuirksMode) {}
    fn same_node(&self, x: &Id, y: &Id) -> bool { x == y }
    fn elem_name<'a>(&'a self, target: &'a Id) -> Name { Name(self.names.borrow().get(target).cloned().expect("an element")) }
    fn get_template_contents(&self, target: &Id) -> Id { *self.templates.borrow().get(target).expect("a template") }
    fn is_mathml_annotation_xml_integration_point(&self, target: &Id) -> bool { self.annotation.borrow().contains(target) }
    fn create_element(&self, name: QualName, attrs: Vec<Attribute>, flags: ElementFlags) -> Id {
        let html = &*name.ns == "http://www.w3.org/1999/xhtml";
        let id = self.add(Kind::Element {
            name: name.local.to_string().to_ascii_lowercase(), html,
            attrs: attrs.into_iter().map(|a| (a.name.local.to_string().to_ascii_lowercase(), a.value.to_string())).collect(),
        });
        self.names.borrow_mut().insert(id, name);
        if flags.template { let t = self.add(Kind::Document); self.templates.borrow_mut().insert(id, t); }
        if flags.mathml_annotation_xml_integration_point { self.annotation.borrow_mut().insert(id); }
        id
    }
    fn create_comment(&self, _: StrTendril) -> Id { self.add(Kind::Comment) }
    fn create_pi(&self, _: StrTendril, _: StrTendril) -> Id { self.add(Kind::Other) }
    fn append(&self, parent: &Id, child: NodeOrText<Id>) { self.place(*parent, None, child) }
    fn append_before_sibling(&self, sibling: &Id, child: NodeOrText<Id>) {
        let (parent, at) = {
            let n = self.nodes.borrow();
            let Some(p) = n[*sibling].parent else { return };
            (p, n[p].children.iter().position(|c| c == sibling).unwrap_or(0))
        };
        self.place(parent, Some(at), child)
    }
    fn append_based_on_parent_node(&self, element: &Id, prev_element: &Id, child: NodeOrText<Id>) {
        if self.nodes.borrow()[*element].parent.is_some() { self.append_before_sibling(element, child) } else { self.append(prev_element, child) }
    }
    fn append_doctype_to_document(&self, _: StrTendril, _: StrTendril, _: StrTendril) {}
    fn add_attrs_if_missing(&self, target: &Id, attrs: Vec<Attribute>) {
        let mut n = self.nodes.borrow_mut();
        if let Kind::Element { attrs: have, .. } = &mut n[*target].kind {
            for a in attrs {
                let k = a.name.local.to_string().to_ascii_lowercase();
                if !have.iter().any(|(h, _)| *h == k) { have.push((k, a.value.to_string())); }
            }
        }
    }
    fn remove_from_parent(&self, target: &Id) { self.detach(*target) }
    fn reparent_children(&self, node: &Id, new_parent: &Id) {
        let kids = std::mem::take(&mut self.nodes.borrow_mut()[*node].children);
        for k in kids { self.nodes.borrow_mut()[k].parent = None; self.place(*new_parent, None, NodeOrText::AppendNode(k)); }
    }
}

const VOID: &[&str] = &["area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "source", "track", "wbr", "param"];

pub fn escape_text(s: &str, out: &mut String) {
    for c in s.chars() { match c { '&' => out.push_str("&amp;"), '<' => out.push_str("&lt;"), '>' => out.push_str("&gt;"), c => out.push(c) } }
}
pub fn escape_attr(s: &str, out: &mut String) {
    for c in s.chars() { match c { '&' => out.push_str("&amp;"), '"' => out.push_str("&quot;"), '<' => out.push_str("&lt;"), '>' => out.push_str("&gt;"), c => out.push(c) } }
}
pub fn escaped(s: &str) -> String { let mut o = String::with_capacity(s.len() + 16); escape_text(s, &mut o); o }

impl Dom {
    pub fn parse(html: &str) -> Dom {
        // the document node must be node 0 (`ROOT`): without it the first element took id 0 and was appended to itself (a cycle, every walk
        // looped until the process was killed: round-4 restart, the three css tests of the unsaved work)
        let sink = Sink::default();
        sink.add(Kind::Document);
        parse_document(sink, Default::default()).one(StrTendril::from(html))
    }

    pub fn tag(&self, id: Id) -> Option<&str> { match &self.nodes[id].kind { Kind::Element { name, html: true, .. } => Some(name), _ => None } }
    pub fn is_element(&self, id: Id) -> bool { matches!(self.nodes[id].kind, Kind::Element { .. }) }
    pub fn attr(&self, id: Id, name: &str) -> Option<&str> {
        match &self.nodes[id].kind { Kind::Element { attrs, .. } => attrs.iter().find(|(k, _)| k == name).map(|(_, v)| v.as_str()), _ => None }
    }
    pub fn set_attr(&mut self, id: Id, name: &str, value: String) {
        if let Kind::Element { attrs, .. } = &mut self.nodes[id].kind {
            match attrs.iter_mut().find(|(k, _)| k == name) { Some(a) => a.1 = value, None => attrs.push((name.to_string(), value)) }
        }
    }
    pub fn remove_attr(&mut self, id: Id, name: &str) { if let Kind::Element { attrs, .. } = &mut self.nodes[id].kind { attrs.retain(|(k, _)| k != name); } }
    pub fn first_child(&self, id: Id) -> Option<Id> { self.nodes[id].children.first().copied() }

    /// Every node under `id` (itself first) in document order, iteratively.
    pub fn descendants(&self, id: Id) -> Vec<Id> {
        let mut out = Vec::new();
        let mut stack = vec![id];
        // bounded: each node is visited once, so a cycle in the tree (round 4: a missing document node made one, and this walk grew a
        // Vec to 78 GB) ends the walk instead of growing it
        let mut seen = vec![false; self.nodes.len()];
        while let Some(n) = stack.pop() {
            if std::mem::replace(&mut seen[n], true) { continue; }
            out.push(n);
            for c in self.nodes[n].children.iter().rev() { if !seen[*c] { stack.push(*c); } }
        }
        out
    }

    /// The nesting depth of `id` below the document (the document is 0).
    pub fn depth(&self, mut id: Id) -> usize { let mut d = 0; while let Some(p) = self.nodes[id].parent { d += 1; id = p; if d > self.nodes.len() { break; } } d }

    /// The text under `id`, `<br>` as a line break, no block breaks (the caller wants a run of inline content).
    pub fn plain_text(&self, id: Id) -> String {
        let mut s = String::new();
        for n in self.descendants(id) {
            match &self.nodes[n].kind {
                Kind::Text(t) => s.push_str(t),
                Kind::Element { name, html: true, .. } if name == "br" => s.push('\n'),
                _ => {}
            }
        }
        s
    }

    /// `<tag attrs>children</tag>` for one node. Below `max_depth` levels a subtree is written as its escaped text.
    pub fn serialize(&self, id: Id, max_depth: usize) -> String { let mut o = String::new(); self.write(id, 0, max_depth, &mut o); o }

    /// The children of `id`, serialised.
    pub fn serialize_children(&self, id: Id, max_depth: usize) -> String {
        let mut o = String::new();
        for c in &self.nodes[id].children { self.write(*c, 0, max_depth, &mut o); }
        o
    }

    fn write(&self, id: Id, depth: usize, max_depth: usize, out: &mut String) {
        match &self.nodes[id].kind {
            Kind::Text(t) => escape_text(t, out),
            Kind::Element { name, attrs, html, .. } => {
                if !*html { return; }
                if depth >= max_depth { escape_text(&self.plain_text(id), out); return; }
                out.push('<'); out.push_str(name);
                for (k, v) in attrs { out.push(' '); out.push_str(k); out.push_str("=\""); escape_attr(v, out); out.push('"'); }
                out.push('>');
                if VOID.contains(&name.as_str()) { return; }
                for c in &self.nodes[id].children { self.write(*c, depth + 1, max_depth, out); }
                out.push_str("</"); out.push_str(name); out.push('>');
            }
            Kind::Document => for c in &self.nodes[id].children { self.write(*c, depth, max_depth, out); },
            Kind::Comment | Kind::Other => {}
        }
    }

    /// The first element named `tag` under `id`.
    pub fn find(&self, id: Id, tag: &str) -> Option<Id> { self.descendants(id).into_iter().find(|n| self.tag(*n) == Some(tag)) }

    /// Detaches `id` from its parent (its subtree stays in the arena, unreachable).
    pub fn remove(&mut self, id: Id) {
        if let Some(p) = self.nodes[id].parent.take() { self.nodes[p].children.retain(|c| *c != id); }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Round 4 root cause: `Dom::parse` made no document node, the first element took id 0 and was appended to itself, and `descendants`
    /// grew without bound (78 GB). The parse now puts the document first, and every walk visits a node once.
    /// Mutants: `r4-no-document-node` (the `sink.add(Kind::Document)` line removed): red on the first assertion;
    /// `r4-walk-unbounded` (the `seen` check in `descendants` removed): red on the time bound (the walk is run on a thread for 500 ms).
    #[test]
    fn the_input_that_grew_to_78_gb_is_bounded() {
        let d = Dom::parse("<style>p{color:red;margin:0}</style><div><p id=a class=x>t</p></div>");
        assert!(matches!(d.nodes[ROOT].kind, Kind::Document), "node 0 is the document");
        let all = d.descendants(ROOT);
        assert!(all.len() <= d.nodes.len(), "{} of {}", all.len(), d.nodes.len());
        assert!(all.iter().all(|n| d.depth(*n) <= d.nodes.len()));
        // a tree with a cycle (what the fault made) ends the walk
        let cyclic = Dom { nodes: vec![
            Node { kind: Kind::Document, parent: None, children: vec![1] },
            Node { kind: Kind::Element { name: "div".into(), html: true, attrs: vec![] }, parent: Some(1), children: vec![1, 1] },
        ] };
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || { let _ = tx.send((cyclic.descendants(ROOT).len(), cyclic.depth(1))); });
        let got = rx.recv_timeout(std::time::Duration::from_millis(500)).expect("the walk over a cycle ends");
        assert_eq!(got.0, 2);
        assert!(got.1 <= 3);
    }
}
