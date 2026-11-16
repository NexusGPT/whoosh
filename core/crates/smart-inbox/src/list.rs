//! The ordered list per decision: entries top to bottom, first match wins. A rule entry and the "ask the model"
//! entry are both just entries; the owner moves the model entry where they want it (`move_model_to`).
//! The shipped lists below are the lists of docs/DECISION-MODEL.md section 1, with the extra rule entries named in
//! docs/lanes/core-smart-inbox.md ("Entries I added to the shipped list").
use crate::facts::GmailCategory;
use crate::tuning::{MODEL_TRUST_NEEDS_REPLY, MODEL_TRUST_TAB};
use contracts::{RuleCondition, SplitTabRule};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecisionPage {
    WhichTab,
    NeedsMyReply,
}

impl DecisionPage {
    pub fn words(&self) -> &'static str {
        match self {
            DecisionPage::WhichTab => "Which tab",
            DecisionPage::NeedsMyReply => "Needs my reply",
        }
    }
}

/// A tab named without its display name: renaming a tab never changes where mail goes.
/// `Builtin` is the key a default tab keeps in its rule (`SplitTabRule::Builtin { name }`), `Id` is a tab id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TabRef {
    Builtin(String),
    Id(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Then {
    Tab(TabRef),
    Answer(bool),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Cond {
    /// One of the owner's rule conditions (contracts::RuleCondition).
    User(RuleCondition),
    /// The rule of a tab the owner added (sender, domain, account or label).
    TabRule(SplitTabRule),
    FromVip,
    FromOwner,
    ListHeader,
    Gmail(GmailCategory),
    FromDomainIn(Vec<String>),
    SubjectOrBodyHasAny(Vec<String>),
    OwnerWroteToSender,
    /// The answer of the "Needs my reply" list (asked at most once per message).
    NeedsReply,
}

#[derive(Debug, Clone, PartialEq)]
pub enum EntryKind {
    Rule { when: Vec<Cond>, then: Then },
    /// "ask the model: <question> · trust it from <level>"; below the level the list continues.
    Model { question: String, trust_from: f64 },
    /// Closes the list.
    Default { then: Then },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    /// Names the entry in `decided_by` (a rule entry's id is the rule_id written to message_category).
    pub id: String,
    pub kind: EntryKind,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DecisionList {
    pub page: DecisionPage,
    pub entries: Vec<Entry>,
}

/// Social networks (data, not a number): mail from these domains sits in Social.
const SOCIAL_DOMAINS: &[&str] = &["linkedin.com", "facebookmail.com", "twitter.com", "x.com", "instagram.com", "pinterest.com", "reddit.com", "redditmail.com", "tiktok.com"];
/// Words that make a list mail a promotion (subject or body).
const PROMO_WORDS: &[&str] = &["% off", "discount", "limited time", "special offer", "coupon", "promo code", "free shipping", "flash sale", "on sale", "deal of the day"];

pub const WHICH_TAB_QUESTION: &str = "Which inbox tab should this email sit in?";
pub const NEEDS_REPLY_QUESTION: &str = "Does this email ask me, personally, to answer or to do something?";

fn rule(id: &str, when: Vec<Cond>, then: Then) -> Entry {
    Entry { id: id.to_string(), kind: EntryKind::Rule { when, then } }
}
fn builtin(name: &str) -> Then {
    Then::Tab(TabRef::Builtin(name.to_string()))
}

impl DecisionList {
    /// 1 VIP → VIPs (Primary once that tab is removed) · needs reply → Needs Reply · Gmail's own category · a sender the owner
    /// has written to → Primary · a social network → Social · a list mail with promo words → Promotions · any list header →
    /// Updates · ask the model (trust from 80 %) · otherwise Primary.
    pub fn shipped_which_tab() -> DecisionList {
        let strs = |a: &[&str]| a.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        DecisionList {
            page: DecisionPage::WhichTab,
            entries: vec![
                rule("which-tab/vip", vec![Cond::FromVip], builtin("VIPs")),
                rule("which-tab/needs-reply", vec![Cond::NeedsReply], builtin("Needs Reply")),
                rule("which-tab/gmail-promotions", vec![Cond::Gmail(GmailCategory::Promotions)], builtin("Promotions")),
                rule("which-tab/gmail-social", vec![Cond::Gmail(GmailCategory::Social)], builtin("Social")),
                rule("which-tab/gmail-updates", vec![Cond::Gmail(GmailCategory::Updates)], builtin("Updates")),
                rule("which-tab/gmail-personal", vec![Cond::Gmail(GmailCategory::Primary)], builtin("Primary")),
                rule("which-tab/known-sender", vec![Cond::OwnerWroteToSender], builtin("Primary")),
                rule("which-tab/social-network", vec![Cond::FromDomainIn(strs(SOCIAL_DOMAINS))], builtin("Social")),
                rule("which-tab/promo-list", vec![Cond::ListHeader, Cond::SubjectOrBodyHasAny(strs(PROMO_WORDS))], builtin("Promotions")),
                rule("which-tab/list-header", vec![Cond::ListHeader], builtin("Updates")),
                Entry { id: "which-tab/model".into(), kind: EntryKind::Model { question: WHICH_TAB_QUESTION.into(), trust_from: MODEL_TRUST_TAB } },
                Entry { id: "which-tab/default".into(), kind: EntryKind::Default { then: builtin("Primary") } },
            ],
        }
    }

    /// 1 from me → no · 2 has a list header → no · 3 ask the model (trust from 75 %) · 4 otherwise no.
    pub fn shipped_needs_reply() -> DecisionList {
        DecisionList {
            page: DecisionPage::NeedsMyReply,
            entries: vec![
                rule("needs-reply/from-me", vec![Cond::FromOwner], Then::Answer(false)),
                rule("needs-reply/list-header", vec![Cond::ListHeader], Then::Answer(false)),
                Entry { id: "needs-reply/model".into(), kind: EntryKind::Model { question: NEEDS_REPLY_QUESTION.into(), trust_from: MODEL_TRUST_NEEDS_REPLY } },
                Entry { id: "needs-reply/default".into(), kind: EntryKind::Default { then: Then::Answer(false) } },
            ],
        }
    }

    pub fn model_position(&self) -> Option<usize> {
        self.entries.iter().position(|e| matches!(e.kind, EntryKind::Model { .. }))
    }

    /// The owner drags the model entry to `index` (0-based). Returns false when the list has no model entry.
    pub fn move_model_to(&mut self, index: usize) -> bool {
        let Some(from) = self.model_position() else { return false };
        let entry = self.entries.remove(from);
        self.entries.insert(index.min(self.entries.len()), entry);
        true
    }

    pub fn insert_first(&mut self, mut first: Vec<Entry>) {
        first.append(&mut self.entries);
        self.entries = first;
    }
}
