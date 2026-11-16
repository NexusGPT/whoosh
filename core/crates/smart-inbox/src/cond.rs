//! Conditions that read only the facts. `NeedsReply` needs the other list and is answered by the placer.
use crate::facts::MessageFacts;
use crate::list::Cond;
use contracts::{MessageFact, RuleCondition, SplitTabRule};

/// `needle` found in `hay` as whole words: a letter or digit may not touch the match where the needle itself starts or ends
/// with one ("sale" does not match "wholesale" or "Salesforce"; "% off" matches "40% off").
pub fn contains_words(hay: &str, needle: &str) -> bool {
    let (h, n) = (hay.to_ascii_lowercase(), needle.to_ascii_lowercase());
    if n.trim().is_empty() {
        return false;
    }
    let alnum = |c: char| c.is_alphanumeric();
    let mut from = 0;
    while let Some(i) = h[from..].find(&n) {
        let start = from + i;
        let end = start + n.len();
        let left_ok = !n.starts_with(alnum) || !h[..start].chars().next_back().is_some_and(alnum);
        let right_ok = !n.ends_with(alnum) || !h[end..].chars().next().is_some_and(alnum);
        if left_ok && right_ok {
            return true;
        }
        from = start + n.chars().next().map_or(1, |c| c.len_utf8());
        if from >= h.len() {
            break;
        }
    }
    false
}

pub fn contains_ci(hay: &str, needle: &str) -> bool {
    !needle.trim().is_empty() && hay.to_ascii_lowercase().contains(&needle.trim().to_ascii_lowercase())
}

/// `domain` is the sender's domain or a subdomain of the wanted one.
pub fn domain_matches(sender_domain: &str, wanted: &str) -> bool {
    let (s, w) = (sender_domain.trim().to_ascii_lowercase(), wanted.trim().trim_start_matches('@').to_ascii_lowercase());
    !w.is_empty() && (s == w || s.ends_with(&format!(".{w}")))
}

pub fn user_condition(c: &RuleCondition, f: &MessageFacts) -> bool {
    match c {
        RuleCondition::From { words } => contains_ci(&f.from_address, words),
        RuleCondition::To { words } => f.to.iter().any(|a| contains_ci(a, words)),
        RuleCondition::Cc { words } => f.cc.iter().any(|a| contains_ci(a, words)),
        RuleCondition::Domain { domain } => domain_matches(&f.domain(), domain),
        RuleCondition::SubjectContains { words } => contains_ci(&f.subject, words),
        RuleCondition::BodyContains { words } => contains_ci(&f.body, words),
        RuleCondition::ListHeader { words } => f.has_list_header && (words.trim().is_empty() || f.list_id.as_deref().is_some_and(|l| contains_ci(l, words))),
        RuleCondition::Account { account_id } => &f.account_id == account_id,
        RuleCondition::HasAttachment => f.has_attachment,
        // delta 17: one of the eight yes/no facts with the value wanted
        RuleCondition::Fact { fact, wanted } => fact_holds(*fact, f) == *wanted,
        // round 18: the negated twin holds exactly when its positive twin does not
        RuleCondition::Not { field, value } => !user_condition(&RuleCondition::positive_twin(*field, value), f),
    }
}

/// Whether one of the eight grid facts is true of the message, read from this crate's facts (the same reading as the contract's
/// `fact_holds` over its own facts: the mailing list is the list-header bit, the VIP sender is the VIP list).
pub fn fact_holds(fact: MessageFact, f: &MessageFacts) -> bool {
    match fact {
        MessageFact::Unread => f.unread,
        MessageFact::Unanswered => f.unanswered,
        MessageFact::HasAttachment => f.has_attachment,
        MessageFact::Starred => f.starred,
        MessageFact::VipSender => f.from_vip,
        MessageFact::CcMe => f.cc_me,
        MessageFact::MailingList => f.has_list_header,
        MessageFact::RepliedByMe => f.replied_by_me,
    }
}

/// A tab the owner added collects mail by sender, domain, account or label. Built-in and decision-category tabs are
/// not conditions: the first is a position in the shipped list, the second is the model's answer.
pub fn tab_rule(r: &SplitTabRule, f: &MessageFacts) -> bool {
    match r {
        SplitTabRule::Sender { address } => f.from_address.eq_ignore_ascii_case(address.trim()),
        SplitTabRule::Domain { domain } => domain_matches(&f.domain(), domain),
        SplitTabRule::Account { account_id } => &f.account_id == account_id,
        // A label rule names a node; it matches a message carrying that label or any label below it.
        SplitTabRule::Label { label_id } => f.label_chains.iter().any(|chain| chain.iter().any(|n| n == label_id)),
        SplitTabRule::DecisionCategory { .. } | SplitTabRule::Builtin { .. } => false,
    }
}

impl Cond {
    /// For a label rule: the exact node the message carries that made it match (the rule's own node, or a descendant of it).
    pub fn label_node(&self, f: &MessageFacts) -> Option<String> {
        let Cond::TabRule(SplitTabRule::Label { label_id }) = self else { return None };
        f.label_chains.iter().find(|chain| chain.iter().any(|n| n == label_id)).and_then(|chain| chain.last().cloned())
    }

    /// None = this condition needs more than the facts (the Needs my reply answer).
    pub fn pure(&self, f: &MessageFacts) -> Option<bool> {
        Some(match self {
            Cond::User(c) => user_condition(c, f),
            Cond::TabRule(r) => tab_rule(r, f),
            Cond::FromVip => f.from_vip,
            Cond::FromOwner => f.from_owner,
            Cond::ListHeader => f.has_list_header,
            Cond::Gmail(c) => f.gmail_category() == Some(*c),
            Cond::FromDomainIn(ds) => {
                let d = f.domain();
                ds.iter().any(|w| domain_matches(&d, w))
            }
            Cond::SubjectOrBodyHasAny(ws) => {
                let text = format!("{} {}", f.subject, f.body);
                ws.iter().any(|w| contains_words(&text, w))
            }
            Cond::OwnerWroteToSender => f.history.owner_wrote_to,
            Cond::NeedsReply => return None,
        })
    }
}
