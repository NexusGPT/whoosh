//! What the filter reads of one message, and what it knows of the person. Plain owned data: the engine fills it from
//! the parsed message (decoded names, text, HTML) and from two headers the parse crate does not expose yet
//! (Reply-To, Authentication-Results); see docs/lanes/core-junk.md "Wiring".
use crate::domain::{domain_of, registrable_domain};

/// One message as the filter reads it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Message {
    /// The display name of From, already decoded (RFC 2047) by the parser.
    pub from_name: Option<String>,
    /// The address of From, lower-case.
    pub from_email: String,
    /// The addresses of Reply-To, lower-case; empty when the header is absent.
    pub reply_to: Vec<String>,
    pub subject: String,
    /// The plain-text body (the parser makes it from the HTML when there is no text part).
    pub text: String,
    /// The HTML body, when there is one; only the links are read from it.
    pub html: Option<String>,
    /// The values of the Authentication-Results headers, topmost header first (unfolded).
    pub authentication_results: Vec<String>,
}

impl Message {
    /// Builds a message from a raw header block (as IMAP returns BODY[HEADER]) plus the decoded bodies. Names are taken
    /// as written (no RFC 2047 decoding); the engine uses the parser's decoded names and fills the struct itself.
    pub fn from_header_block(headers: &str, text: &str, html: Option<&str>) -> Message {
        let fields = unfold_headers(headers);
        let mut m = Message { text: text.to_string(), html: html.map(str::to_string), ..Message::default() };
        for (name, value) in &fields {
            match name.as_str() {
                "from" if m.from_email.is_empty() => {
                    let (n, e) = parse_address(value);
                    m.from_name = n;
                    m.from_email = e;
                }
                "subject" if m.subject.is_empty() => m.subject = value.clone(),
                "reply-to" => {
                    for part in value.split(',') {
                        let (_, e) = parse_address(part);
                        if !e.is_empty() {
                            m.reply_to.push(e);
                        }
                    }
                }
                "authentication-results" => m.authentication_results.push(value.clone()),
                _ => {}
            }
        }
        m
    }
}

/// Header lines unfolded (a line starting with a space or tab continues the one before) as (lower-case name, value).
/// Stops at the first blank line.
pub fn unfold_headers(block: &str) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    for raw in block.split('\n') {
        let line = raw.trim_end_matches('\r');
        if line.is_empty() {
            break;
        }
        if line.starts_with(' ') || line.starts_with('\t') {
            if let Some(last) = out.last_mut() {
                last.1.push(' ');
                last.1.push_str(line.trim());
            }
            continue;
        }
        if let Some((name, value)) = line.split_once(':') {
            out.push((name.trim().to_ascii_lowercase(), value.trim().to_string()));
        }
    }
    out
}

/// The first address of an address field as (display name, lower-case address):
/// `"Ann Lee" <ann@x.example>`, `Ann Lee <ann@x.example>`, `<ann@x.example>` and `ann@x.example` all read.
pub fn parse_address(field: &str) -> (Option<String>, String) {
    let f = field.trim();
    if let Some(lt) = f.find('<') {
        if let Some(rel) = f[lt..].find('>') {
            let gt = lt + rel;
            let name = f[..lt].trim().trim_matches('"').trim().to_string();
            let email = f[lt + 1..gt].trim().to_ascii_lowercase();
            return (if name.is_empty() { None } else { Some(name) }, email);
        }
    }
    (None, f.to_ascii_lowercase())
}

/// A person the owner knows (the engine builds these from the contact table).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Contact {
    pub name: String,
    pub emails: Vec<String>,
}

/// Where a hard rule came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HardSource {
    /// "Block sender" / "Block domain".
    Block,
    /// An "Always junk" rule.
    AlwaysJunk,
}

/// What a hard rule matches.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Matcher {
    /// One address, any case.
    Sender(String),
    /// A domain and every subdomain of it.
    Domain(String),
}

/// A block or an "always junk" rule: the person decided, so the score is certain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HardRule {
    pub source: HardSource,
    pub matcher: Matcher,
}

impl HardRule {
    pub fn block_sender(address: &str) -> HardRule {
        HardRule { source: HardSource::Block, matcher: Matcher::Sender(address.trim().to_ascii_lowercase()) }
    }
    pub fn block_domain(domain: &str) -> HardRule {
        HardRule { source: HardSource::Block, matcher: Matcher::Domain(clean_domain(domain)) }
    }
    pub fn always_junk_sender(address: &str) -> HardRule {
        HardRule { source: HardSource::AlwaysJunk, matcher: Matcher::Sender(address.trim().to_ascii_lowercase()) }
    }
    pub fn always_junk_domain(domain: &str) -> HardRule {
        HardRule { source: HardSource::AlwaysJunk, matcher: Matcher::Domain(clean_domain(domain)) }
    }

    /// True when the sender address falls under this rule.
    pub fn matches(&self, from_email: &str) -> bool {
        match &self.matcher {
            Matcher::Sender(a) => from_email.trim().eq_ignore_ascii_case(a),
            Matcher::Domain(d) => match domain_of(from_email) {
                Some(fd) => fd == *d || fd.ends_with(&format!(".{d}")),
                None => false,
            },
        }
    }
}

fn clean_domain(d: &str) -> String {
    d.trim().trim_start_matches('@').trim_end_matches('.').to_ascii_lowercase()
}

/// What the filter knows of the person: who they know, and what they blocked.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Context {
    pub contacts: Vec<Contact>,
    pub hard_rules: Vec<HardRule>,
}

/// True when two addresses share a registrable domain.
pub(crate) fn same_site(a: &str, b: &str) -> bool {
    match (domain_of(a), domain_of(b)) {
        (Some(x), Some(y)) => registrable_domain(&x) == registrable_domain(&y),
        _ => false,
    }
}
