//! The element tree the two config formats are read from (the Mozilla Autoconfig clientConfig and the
//! Autodiscover POX response), built by roxmltree. Names keep only their local part (namespaces dropped).

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Element {
    pub name: String,
    pub attrs: Vec<(String, String)>,
    pub children: Vec<Node>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Node {
    Element(Element),
    Text(String),
}

impl Element {
    pub fn attr(&self, name: &str) -> Option<&str> {
        self.attrs.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str())
    }
    pub fn elements(&self) -> impl Iterator<Item = &Element> {
        self.children.iter().filter_map(|n| match n { Node::Element(e) => Some(e), Node::Text(_) => None })
    }
    pub fn children_named<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a Element> + 'a {
        self.elements().filter(move |e| e.name.eq_ignore_ascii_case(name))
    }
    pub fn child(&self, name: &str) -> Option<&Element> { self.elements().find(|e| e.name.eq_ignore_ascii_case(name)) }
    /// The element's own text, trimmed.
    pub fn text(&self) -> String {
        let mut s = String::new();
        for n in &self.children {
            if let Node::Text(t) = n { s.push_str(t); }
        }
        s.trim().to_string()
    }
    pub fn child_text(&self, name: &str) -> Option<String> { self.child(name).map(Element::text) }
}

fn convert(n: roxmltree::Node<'_, '_>) -> Element {
    Element {
        name: n.tag_name().name().to_string(),
        attrs: n.attributes().map(|a| (a.name().to_string(), a.value().to_string())).collect(),
        children: n
            .children()
            .filter_map(|c| {
                if c.is_element() { Some(Node::Element(convert(c))) } else if c.is_text() { c.text().map(|t| Node::Text(t.to_string())) } else { None }
            })
            .collect(),
    }
}

/// Parses a document and returns its root element.
pub(crate) fn parse(doc: &str) -> Result<Element, String> {
    let doc = roxmltree::Document::parse(doc.trim_start_matches('\u{feff}')).map_err(|e| e.to_string())?;
    Ok(convert(doc.root_element()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_entities_cdata_comments_and_namespaces() {
        let doc = r#"<?xml version="1.0"?><!-- c --><a:Root xmlns:a="u"><Name kind='x'>Tom &amp; &#x4A;erry</Name><!-- c2 --><Raw><![CDATA[<b>&amp;</b>]]></Raw><Empty/></a:Root>"#;
        let root = parse(doc).expect("parses");
        assert_eq!(root.name, "Root");
        assert_eq!(root.child_text("Name").as_deref(), Some("Tom & Jerry"));
        assert_eq!(root.child("Name").and_then(|e| e.attr("kind")), Some("x"));
        assert_eq!(root.child_text("Raw").as_deref(), Some("<b>&amp;</b>"));
        assert!(root.child("Empty").is_some());
    }

    #[test]
    fn refuses_a_mismatched_or_unclosed_document() {
        assert!(parse("<a><b></a>").is_err());
        assert!(parse("<a>").is_err());
        assert!(parse("<a>&nope;</a>").is_err());
        assert!(parse("<a></a><b/>").is_err());
    }
}
