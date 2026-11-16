//! Cheap header rules in the spirit of SpamAssassin's: no daemon, no network lookup, a fixed score each. They read
//! only what the message already carries: the verdicts the receiving server wrote into Authentication-Results,
//! the sender's own fields and the links of the HTML. No SpamAssassin code or rule text is used (ideas only).
use crate::domain::{domain_of, host_of_url, registrable_domain, visible_host};
use crate::links::extract_links;
use crate::message::{same_site, Contact, Context, Message};
use crate::tuning::*;

/// One header rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RuleId {
    SpfFail,
    SpfSoftfail,
    DkimFail,
    DmarcFail,
    ReplyToMismatch,
    DisplayNameImpersonation,
    LinkMismatch,
}

impl RuleId {
    /// The fixed score of the rule: the chance, taken alone, that a message which trips it is junk.
    pub fn weight(self) -> f64 {
        match self {
            RuleId::SpfFail => RULE_SPF_FAIL,
            RuleId::SpfSoftfail => RULE_SPF_SOFTFAIL,
            RuleId::DkimFail => RULE_DKIM_FAIL,
            RuleId::DmarcFail => RULE_DMARC_FAIL,
            RuleId::ReplyToMismatch => RULE_REPLY_TO_MISMATCH,
            RuleId::DisplayNameImpersonation => RULE_DISPLAY_NAME_IMPERSONATION,
            RuleId::LinkMismatch => RULE_LINK_MISMATCH,
        }
    }

    /// A short stable name (for logs and the "why is this junk" line).
    pub fn name(self) -> &'static str {
        match self {
            RuleId::SpfFail => "spf-fail",
            RuleId::SpfSoftfail => "spf-softfail",
            RuleId::DkimFail => "dkim-fail",
            RuleId::DmarcFail => "dmarc-fail",
            RuleId::ReplyToMismatch => "reply-to-mismatch",
            RuleId::DisplayNameImpersonation => "display-name-impersonation",
            RuleId::LinkMismatch => "link-mismatch",
        }
    }
}

/// The three verdicts of an Authentication-Results header (RFC 8601), lower-case; None when the header does not say.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AuthVerdicts {
    pub spf: Option<String>,
    pub dkim: Option<String>,
    pub dmarc: Option<String>,
}

/// Reads the verdicts from the TOPMOST Authentication-Results header only: the receiving server adds its header on top,
/// so a lower one was written by someone upstream (possibly the sender) and is not believed.
/// DKIM may sign a message several times: one passing signature is enough.
pub fn auth_verdicts(values: &[String]) -> AuthVerdicts {
    let mut v = AuthVerdicts::default();
    let header = match values.first() {
        Some(h) => h,
        None => return v,
    };
    // the first part up to ";" is the server's own name; the rest are "method=result key=value ..." parts
    for part in header.split(';').skip(1) {
        let part = part.trim();
        let (method, rest) = match part.split_once('=') {
            Some(x) => x,
            None => continue,
        };
        let result = rest.split(|c: char| c.is_whitespace() || c == '(').next().unwrap_or("").trim().to_ascii_lowercase();
        if result.is_empty() {
            continue;
        }
        match method.trim().to_ascii_lowercase().as_str() {
            "spf" if v.spf.is_none() => v.spf = Some(result),
            "dmarc" if v.dmarc.is_none() => v.dmarc = Some(result),
            "dkim" if v.dkim.as_deref() != Some("pass") => v.dkim = Some(result),
            _ => {}
        }
    }
    v
}

/// Every rule the message trips, in a fixed order.
pub fn fired_rules(msg: &Message, ctx: &Context) -> Vec<RuleId> {
    let mut out: Vec<RuleId> = Vec::new();
    let auth = auth_verdicts(&msg.authentication_results);
    match auth.spf.as_deref() {
        Some("fail") => out.push(RuleId::SpfFail),
        Some("softfail") => out.push(RuleId::SpfSoftfail),
        _ => {}
    }
    if auth.dkim.as_deref() == Some("fail") {
        out.push(RuleId::DkimFail);
    }
    if auth.dmarc.as_deref() == Some("fail") {
        out.push(RuleId::DmarcFail);
    }
    if reply_to_mismatch(msg) {
        out.push(RuleId::ReplyToMismatch);
    }
    if display_name_impersonation(msg, &ctx.contacts) {
        out.push(RuleId::DisplayNameImpersonation);
    }
    if link_mismatch(msg) {
        out.push(RuleId::LinkMismatch);
    }
    out
}

/// Reply-To points to another site than From (mailing lists and newsletters trip this too, hence the low score).
pub fn reply_to_mismatch(msg: &Message) -> bool {
    if domain_of(&msg.from_email).is_none() {
        return false;
    }
    msg.reply_to.iter().any(|r| domain_of(r).is_some() && !same_site(r, &msg.from_email))
}

/// The display name claims to be someone else: it carries an address on another site than the real sender,
/// or it is the name of a known contact while the address is neither that contact's nor on a site of theirs.
pub fn display_name_impersonation(msg: &Message, contacts: &[Contact]) -> bool {
    let name = match msg.from_name.as_deref() {
        Some(n) => n,
        None => return false,
    };
    let from_site = domain_of(&msg.from_email).map(|d| registrable_domain(&d));
    for piece in name.split_whitespace() {
        let p = piece.trim_matches(|c: char| !(c.is_ascii_alphanumeric() || c == '@' || c == '.' || c == '-' || c == '_' || c == '+'));
        if !p.contains('@') {
            continue;
        }
        if let Some(named) = domain_of(p) {
            match &from_site {
                Some(site) if *site == registrable_domain(&named) => {}
                _ => return true,
            }
        }
    }
    let wanted = normalise_name(name);
    if wanted.chars().count() < 3 {
        return false;
    }
    for c in contacts {
        if normalise_name(&c.name) != wanted {
            continue;
        }
        let known_address = c.emails.iter().any(|e| e.eq_ignore_ascii_case(&msg.from_email));
        let known_site = c.emails.iter().any(|e| same_site(e, &msg.from_email));
        if !known_address && !known_site {
            return true;
        }
    }
    false
}

/// A link shows one site to the reader and goes to another: the text reads as an address, the href leads elsewhere.
pub fn link_mismatch(msg: &Message) -> bool {
    let html = match msg.html.as_deref() {
        Some(h) => h,
        None => return false,
    };
    for link in extract_links(html, LINKS_SCANNED_MAX) {
        let shown = match visible_host(&link.text) {
            Some(s) => s,
            None => continue,
        };
        let target = match host_of_url(&link.href) {
            Some(t) => t,
            None => continue,
        };
        if registrable_domain(&shown) != registrable_domain(&target) {
            return true;
        }
    }
    false
}

fn normalise_name(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        if c.is_alphanumeric() {
            out.extend(c.to_lowercase());
        } else if !out.is_empty() && !out.ends_with(' ') {
            out.push(' ');
        }
    }
    out.trim_end().to_string()
}
