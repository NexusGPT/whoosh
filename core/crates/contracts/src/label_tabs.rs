//! Delta 9: labels are the tabs (docs/PRD.md "Labels: direction picked, and a label's scope"), rules that target labels,
//! suggestions and blocks, the mailbox selector, the media port's kind. Schema first: every shape here is the single source;
//! the pure functions at the bottom are what the Rust core, the store and the Swift FixtureCore all call, so a rule is
//! written once (a probe in tests/delta9.rs fails when one of them is broken).
use crate::{constants::LABEL_DEPTH_MAX, CoreError, MailLabel, RuleCondition};

const CONSTANTS: &str = include_str!("../../../../docs/CONSTANTS.md");

// ---------------------------------------------------------------- the label record

/// Where a label is visible: every mailbox (account), or one account only (owner 2026-11-08 09:45: "labels can be common
/// to all mailboxes or specific to ones"). The one truth of the owner's choice; `MailLabel.account_ids` is the derived read.
#[derive(uniffi::Enum, Debug, Clone, PartialEq, Eq)]
pub enum LabelScope { All, Account { account_id: String } }

/// A built-in category (Needs Reply, Updates, Promotions, Social; Primary is a view since delta 16) or a label the owner made.
#[derive(uniffi::Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum LabelKind { BuiltIn, Owner }

/// How a rule set's conditions combine (owner 2026-11-08: "match any | all per label").
#[derive(uniffi::Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchMode { Any, All }

/// The fixed rules of one label: the same conditions the split tabs and the rule record use. An EMPTY set matches nothing,
/// in both modes (a label with no fixed rule is filed by its meaning or by hand, never "everything").
#[derive(uniffi::Record, Debug, Clone, PartialEq, Eq)]
pub struct RuleSet {
    pub conditions: Vec<RuleCondition>,
    pub match_mode: MatchMode,
}

/// What a rule does. `Tab` keeps the v1 meaning (the rule's `tab_id`); `Label` names any node of the label tree.
#[derive(uniffi::Enum, Debug, Clone, PartialEq, Eq)]
pub enum RuleAction { Tab, NeedsReply, Vip, Mute, Label { label_id: String } }

/// Every editable field of a label, as the add-label sheet shows them. Used whole by `CreateLabelFull` and `UpdateLabel`:
/// an update REPLACES all of them, so the sheet's "no colour", "no description" and "no rules" are explicit values
/// (`None`, `""`, an empty rule set), never "leave unchanged". The three-nulls trap is closed by construction.
#[derive(uniffi::Record, Debug, Clone, PartialEq, Eq)]
pub struct LabelSpec {
    pub name: String,
    /// the label one level up; None = top level
    pub parent_id: Option<String>,
    /// "#RRGGBB"; None = no colour
    pub colour: Option<String>,
    /// what the label means, in the owner's words; the decision model reads it
    pub description: String,
    pub scope: LabelScope,
    pub rules: RuleSet,
    pub pinned: bool,
}

/// A live match count over the local corpus (the sheet's "14 messages · 3 unread · newest ...").
#[derive(uniffi::Record, Debug, Clone, PartialEq, Eq)]
pub struct RuleSetCount {
    pub count: u32,
    pub unread: u32,
    pub newest_at_ms: Option<i64>,
}

/// What a rule can read of one message.
#[derive(uniffi::Record, Debug, Clone, PartialEq, Eq)]
pub struct MessageFacts {
    pub message_id: String,
    pub account_id: String,
    pub from_name: String,
    pub from_address: String,
    pub to: Vec<String>,
    pub cc: Vec<String>,
    pub subject: String,
    pub body: String,
    pub has_attachment: bool,
    /// the List-Id / List-Unsubscribe header words; "" = none
    pub list_header: String,
    pub unread: bool,
    pub received_at_ms: i64,
    /// delta 17, the facts of the grid beyond unread / attachment / list. Required in Rust (a caller that forgets one is a compile error,
    /// not a silent "no"); UniFFI defaults keep Swift callers written before compiling. The mailing-list fact is `list_header` non-empty.
    /// an inbound message the owner has not answered (the waiting definition)
    #[uniffi(default = false)]
    pub unanswered: bool,
    #[uniffi(default = false)]
    pub starred: bool,
    /// the sender is on the VIP list
    #[uniffi(default = false)]
    pub vip_sender: bool,
    /// the owner is in Cc, not To
    #[uniffi(default = false)]
    pub cc_me: bool,
    /// the owner replied to this message
    #[uniffi(default = false)]
    pub replied_by_me: bool,
}

/// What a re-run does: matching messages not yet filed join, filed messages that no longer match leave, the rest stay.
#[derive(uniffi::Record, Debug, Clone, PartialEq, Eq)]
pub struct RerunPlan {
    pub file: Vec<String>,
    pub unfile: Vec<String>,
    pub keep: Vec<String>,
    /// the label has a description, so the meaning part of the re-run belongs to the decision model: it waits (marked pending)
    pub model_pending: bool,
}

// ---------------------------------------------------------------- suggestions and blocks

#[derive(uniffi::Enum, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SuggestionKind { BlockSender, AlwaysJunkDomain, Unsubscribe, MuteThread, AlwaysLabel }

/// One follow-up the core offers after an action (STATE, device-local, short-lived). `subject` is the sender address, the
/// domain, the list, or the thread id; `label_id` is set for `AlwaysLabel` only. `accepted` is false while offered.
#[derive(uniffi::Record, Debug, Clone, PartialEq, Eq)]
pub struct Suggestion {
    pub id: String,
    pub kind: SuggestionKind,
    pub subject: String,
    pub label_id: Option<String>,
    pub offered_at_ms: i64,
    pub accepted: bool,
}

#[derive(uniffi::Enum, Debug, Clone, PartialEq, Eq)]
pub enum BlockTarget { Sender { address: String }, Domain { domain: String } }

/// A blocked sender or domain (STATE; mirrored to the server's filters where the account allows, else local).
#[derive(uniffi::Record, Debug, Clone, PartialEq, Eq)]
pub struct Block { pub target: BlockTarget, pub since_ms: i64 }

/// What accepting a suggestion writes. The core, the store and the FixtureCore all read this one answer.
#[derive(uniffi::Enum, Debug, Clone, PartialEq, Eq)]
pub enum SuggestionEffect {
    Block { target: BlockTarget },
    Unsubscribe { subject: String },
    MuteThread { thread_id: String },
    /// a fixed rule: from `address`, file under `label_id` (a `Rule` with action Label)
    LabelRule { address: String, label_id: String },
}

// ---------------------------------------------------------------- the mailbox selector

/// The key of the all-inboxes count in `Counts.mailboxes`.
pub const MAILBOX_ALL_KEY: &str = "all inboxes";

/// Which mailbox a list shows besides the inbox (owner 2026-11-08: "there is no other tabs except the sent/spam/etc."). The
/// inbox itself, with its tab strip of pinned labels, is `InboxQuery.mailbox == None`: UniFFI cannot default an enum field, and
/// a variant for "the inbox" beside None would give two spellings of one thing.
#[derive(uniffi::Enum, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Mailbox {
    Outbox, Vips, Pinned, Snoozed, Drafts, Sent, Junk, Trash, Archive,
    /// the built-in views (delta 10, G-then-S / G-then-M / G-then-A): starred messages; muted threads; every message not deleted or junk
    Starred, Muted, AllMail,
    /// delta 18: threads whose "remind me if no reply" fired (`reply_reminder_in_view`); a fired reminder is a row here until cleared
    Reminders,
}

/// The key of a mailbox in `Counts.mailboxes` and in state paths, once.
#[uniffi::export]
pub fn mailbox_key(mailbox: Mailbox) -> String {
    match mailbox {
        Mailbox::Outbox => "outbox", Mailbox::Vips => "vips", Mailbox::Pinned => "pinned",
        Mailbox::Snoozed => "snoozed", Mailbox::Drafts => "drafts", Mailbox::Sent => "sent", Mailbox::Junk => "junk",
        Mailbox::Trash => "trash", Mailbox::Archive => "archive",
        Mailbox::Starred => "starred", Mailbox::Muted => "muted", Mailbox::AllMail => "all mail",
        Mailbox::Reminders => "reminders",
    }.to_string()
}

// ---------------------------------------------------------------- the media port

/// What `Embedder::embed_media` is given: an image, an audio file, or one frame of a video (a still the app extracted).
#[derive(uniffi::Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaKind { Image, Audio, VideoFrame }

// ---------------------------------------------------------------- pure logic

/// The tab named in the ledger that is a mailbox (VIPs), not a category label.
const MAILBOX_NOT_A_LABEL: &str = "VIPs";
/// The category the ledger lists but the owner did not pin by default ("Primary to Promotions pinned").
const UNPINNED_BY_DEFAULT: &str = "Social";

/// The names in the ledger row that are VIEWS since delta 16 (docs/PRD.md "A91 shape"): Primary is a view of the inbox, never a
/// label. The ledger row still lists it (the core seeds the decision model's default category from it); the label list does not.
const VIEW_NOT_A_LABEL_NAMES: [&str; 1] = ["Primary"];

/// The built-in category names, in order, from docs/CONSTANTS.md SPLIT_TABS_DEFAULT minus the mailbox and minus the views.
pub fn builtin_category_names() -> Vec<String> {
    CONSTANTS
        .lines()
        .find(|l| l.trim_start().starts_with("| SPLIT_TABS_DEFAULT |"))
        .and_then(|l| l.split('|').nth(2))
        .map(|cell| cell.split(',').map(|n| n.trim().to_string()).filter(|n| !n.is_empty() && n != MAILBOX_NOT_A_LABEL && !VIEW_NOT_A_LABEL_NAMES.contains(&n.as_str())).collect())
        .unwrap_or_default()
}

/// "Needs Reply" -> "needs-reply": the id of a built-in label equals the id of the tab it replaces, so every tab id in use stays valid.
#[uniffi::export]
pub fn builtin_label_id(name: String) -> String { name.trim().to_lowercase().replace(' ', "-") }

/// The built-in categories as labels: kind BuiltIn, top level, shared by every mailbox, pinned except Social. Primary is not
/// among them (delta 16): it is `InboxView::Primary`, derived, never a row.
#[uniffi::export]
pub fn builtin_labels() -> Vec<MailLabel> {
    builtin_category_names().into_iter().enumerate().map(|(i, name)| MailLabel {
        id: builtin_label_id(name.clone()), path: name.clone(), depth: 1, parent_id: None, colour: None, account_ids: vec![], unread: 0,
        description: String::new(), pinned: name != UNPINNED_BY_DEFAULT, scope: LabelScope::All, kind: LabelKind::BuiltIn, order: i as u32,
        entry_enabled: true, ai_enabled: true, exit_enabled: false, ai_reads: crate::default_ai_reads(), exit_fallback: None,
        name,
    }).collect()
}

fn invalid(field: &str, words: impl Into<String>) -> CoreError { CoreError::Invalid { field: field.into(), reason_words: words.into() } }

/// Rename is refused on a built-in category, in words.
#[uniffi::export]
pub fn check_label_rename(label: MailLabel) -> Result<(), CoreError> {
    if label.kind == LabelKind::BuiltIn && label.parent_id.is_none() { crate::check_not_a_view(label.id.clone(), crate::ViewRefusedAction::Rename)?; }
    if label.kind == LabelKind::BuiltIn { return Err(invalid("label", format!("{} is a built-in category: it cannot be renamed", label.name))); }
    Ok(())
}

/// Delete is refused on a built-in category, in words.
#[uniffi::export]
pub fn check_label_delete(label: MailLabel) -> Result<(), CoreError> {
    if label.kind == LabelKind::BuiltIn && label.parent_id.is_none() { crate::check_not_a_view(label.id.clone(), crate::ViewRefusedAction::Delete)?; }
    if label.kind == LabelKind::BuiltIn { return Err(invalid("label", format!("{} is a built-in category: it cannot be deleted. Unpin it to take it off the tabs", label.name))); }
    Ok(())
}

fn colour_ok(c: &str) -> bool { c.len() == 7 && c.starts_with('#') && c[1..].chars().all(|x| x.is_ascii_hexdigit()) }

/// A label's scope must fit its parent's: under an account-specific label only that account's labels can sit.
fn scope_fits(scope: &LabelScope, parent: &Option<MailLabel>) -> Result<(), CoreError> {
    if let Some(LabelScope::Account { account_id: pa }) = parent.as_ref().map(|p| &p.scope) {
        match scope {
            LabelScope::Account { account_id } if account_id == pa => {}
            _ => return Err(invalid("scope", format!("the parent label belongs to one mailbox only, so this label has to belong to the same mailbox"))),
        }
    }
    Ok(())
}

/// Checks the owner's sheet before anything is written: a name, a colour, rules with words, the depth, the scope under its parent.
/// `parent` is the label `spec.parent_id` names (None for the top level). Every refusal is in words.
#[uniffi::export]
pub fn validate_label_spec(spec: LabelSpec, parent: Option<MailLabel>) -> Result<(), CoreError> {
    let n = spec.name.trim();
    if n.is_empty() { return Err(invalid("name", "a label needs a name")); }
    crate::check_not_a_view_name(n.to_string(), spec.parent_id.clone())?;
    if n.contains('>') { return Err(invalid("name", "a label name cannot contain \">\" because it separates the levels of a path")); }
    if let Some(c) = &spec.colour { if !colour_ok(c) { return Err(invalid("colour", format!("\"{c}\" is not a colour: use #RRGGBB"))); } }
    if let Some(p) = &parent {
        if p.depth as u64 + 1 > LABEL_DEPTH_MAX {
            return Err(invalid("parent", format!("a label can be at most {LABEL_DEPTH_MAX} levels deep; \"{} > {n}\" has {}", p.path, p.depth + 1)));
        }
    }
    scope_fits(&spec.scope, &parent)?;
    for c in &spec.rules.conditions {
        if let Some(w) = condition_refusal_words(c.clone()) { return Err(invalid("rules", w)); }
    }
    Ok(())
}

/// A text condition with no words (or a domain that is an address): refused before it is written. A fact is never blank (delta 17).
pub fn condition_is_blank(c: &RuleCondition) -> bool {
    match c {
        RuleCondition::From { words } | RuleCondition::To { words } | RuleCondition::Cc { words }
        | RuleCondition::SubjectContains { words } | RuleCondition::BodyContains { words } => words.trim().is_empty(),
        RuleCondition::Domain { domain } => domain.trim().is_empty() || domain.contains('@'),
        RuleCondition::Account { account_id } => account_id.trim().is_empty(),
        RuleCondition::ListHeader { words } => words.trim().is_empty(),
        RuleCondition::HasAttachment | RuleCondition::Fact { .. } => false,
        // a negated twin is blank exactly when its positive twin is (a blank "is not" would match every message)
        RuleCondition::Not { field, value } => condition_is_blank(&RuleCondition::positive_twin(*field, value)),
    }
}

fn tree_children<'a>(labels: &'a [MailLabel], parent: Option<&str>, known: &std::collections::HashSet<&str>) -> Vec<&'a MailLabel> {
    let mut kids: Vec<(usize, &MailLabel)> = labels.iter().enumerate().filter(|(_, l)| match (&l.parent_id, parent) {
        (None, None) => true,
        (Some(p), Some(q)) => p == q,
        // a label whose parent is not in the list is shown at the top level, never dropped
        (Some(p), None) => !known.contains(p.as_str()),
        _ => false,
    }).collect();
    kids.sort_by(|a, b| a.1.order.cmp(&b.1.order).then(a.0.cmp(&b.0)));
    kids.into_iter().map(|(_, l)| l).collect()
}

/// The tree order the sidebar draws: a parent before its children, siblings by `order` (ties keep the list's own order).
/// The pinned tabs follow this order.
#[uniffi::export]
pub fn ordered_tree(labels: Vec<MailLabel>) -> Vec<MailLabel> {
    let known: std::collections::HashSet<&str> = labels.iter().map(|l| l.id.as_str()).collect();
    let mut out = Vec::with_capacity(labels.len());
    fn walk(labels: &[MailLabel], parent: Option<&str>, known: &std::collections::HashSet<&str>, out: &mut Vec<MailLabel>) {
        for l in tree_children(labels, parent, known) {
            out.push(l.clone());
            walk(labels, Some(&l.id), known, out);
        }
    }
    walk(&labels, None, &known, &mut out);
    out
}

/// The pinned labels in tree order: the tab strip, left to right.
#[uniffi::export]
pub fn pinned_in_tree_order(labels: Vec<MailLabel>) -> Vec<MailLabel> {
    ordered_tree(labels).into_iter().filter(|l| l.pinned).collect()
}

fn names_path(labels: &[MailLabel], id: &str) -> Vec<String> {
    let mut names = vec![];
    let mut cur = labels.iter().find(|l| l.id == id);
    while let Some(l) = cur { names.push(l.name.clone()); cur = l.parent_id.as_deref().and_then(|p| labels.iter().find(|x| x.id == p)); }
    names.reverse();
    names
}

/// Moves a label (with its sub-labels) to `new_parent` at position `new_index` among that parent's children and returns the
/// whole list in tree order with `order`, `path` and `depth` rewritten. Refused in words: a drop on itself or into its own
/// subtree, a subtree that would pass LABEL_DEPTH_MAX, a name already used by a sibling, a built-in leaving the top level,
/// an account-specific parent over a label of another scope. An index past the end means last.
#[uniffi::export]
pub fn reorder_label(labels: Vec<MailLabel>, label_id: String, new_parent: Option<String>, new_index: u32) -> Result<Vec<MailLabel>, CoreError> {
    crate::check_not_a_view(label_id.clone(), crate::ViewRefusedAction::Reorder)?;
    if let Some(p) = &new_parent { crate::check_not_a_view(p.clone(), crate::ViewRefusedAction::AddUnder)?; }
    let me = labels.iter().find(|l| l.id == label_id).cloned().ok_or(CoreError::NotFound { kind: "label".into(), id: label_id.clone() })?;
    let parent = match &new_parent {
        Some(pid) => Some(labels.iter().find(|l| &l.id == pid).cloned().ok_or(CoreError::NotFound { kind: "label".into(), id: pid.clone() })?),
        None => None,
    };
    if me.kind == LabelKind::BuiltIn && new_parent != me.parent_id {
        return Err(invalid("parent", format!("{} is a built-in category: it stays at the top level", me.name)));
    }
    // the subtree of `me`
    let mut subtree: Vec<String> = vec![me.id.clone()];
    let mut i = 0;
    while i < subtree.len() {
        let cur = subtree[i].clone();
        for l in labels.iter().filter(|l| l.parent_id.as_deref() == Some(cur.as_str())) { subtree.push(l.id.clone()); }
        i += 1;
    }
    if let Some(p) = &parent {
        if subtree.contains(&p.id) { return Err(invalid("parent", "a label cannot go inside itself or one of its own sub-labels")); }
    }
    let new_depth = parent.as_ref().map(|p| p.depth as u64).unwrap_or(0) + 1;
    let deepest = subtree.iter().filter_map(|id| labels.iter().find(|l| &l.id == id)).map(|l| l.depth as u64).max().unwrap_or(me.depth as u64);
    if deepest - me.depth as u64 + new_depth > LABEL_DEPTH_MAX {
        return Err(invalid("parent", format!("a label can be at most {LABEL_DEPTH_MAX} levels deep; moving \"{}\" with its sub-labels there would go past that", me.name)));
    }
    if labels.iter().any(|l| l.id != me.id && l.parent_id == new_parent && l.name.eq_ignore_ascii_case(&me.name)) {
        return Err(invalid("name", format!("there is already a label named \"{}\" there", me.name)));
    }
    scope_fits(&me.scope, &parent)?;
    // new sibling order
    let known: std::collections::HashSet<&str> = labels.iter().map(|l| l.id.as_str()).collect();
    let mut sibs: Vec<String> = tree_children(&labels, new_parent.as_deref(), &known).into_iter().filter(|l| l.id != me.id).map(|l| l.id.clone()).collect();
    let at = (new_index as usize).min(sibs.len());
    sibs.insert(at, me.id.clone());
    let old_sibs: Vec<String> = tree_children(&labels, me.parent_id.as_deref(), &known).into_iter().filter(|l| l.id != me.id).map(|l| l.id.clone()).collect();
    let mut out = labels.clone();
    for (k, id) in sibs.iter().enumerate() { if let Some(l) = out.iter_mut().find(|l| &l.id == id) { l.order = k as u32; } }
    if me.parent_id != new_parent { for (k, id) in old_sibs.iter().enumerate() { if let Some(l) = out.iter_mut().find(|l| &l.id == id) { l.order = k as u32; } } }
    if let Some(l) = out.iter_mut().find(|l| l.id == me.id) { l.parent_id = new_parent.clone(); }
    // paths and depths of the subtree
    let snapshot = out.clone();
    for id in &subtree {
        let names = names_path(&snapshot, id);
        if let Some(l) = out.iter_mut().find(|l| &l.id == id) { l.path = names.join(crate::LABEL_PATH_SEPARATOR); l.depth = names.len() as u8; }
    }
    Ok(ordered_tree(out))
}

/// A text condition with a blank or whitespace-only value matches NOTHING (delta 25: `""` is contained in every string, so a stored blank rule joined every message).
/// Saving such a rule is refused in words (`RULE_NO_VALUE_WORDS`); rules already stored with a blank value stay as they are and match nothing (no migration).
fn has(hay: &str, needle: &str) -> bool { let n = needle.trim(); !n.is_empty() && hay.to_lowercase().contains(&n.to_lowercase()) }

/// The words of the refusal when a rule is saved with no value (a code per caller: `rules` for a label sheet, `bad_chain` for a flow chain, `bad_reply_rule` for a reply filter).
pub const RULE_NO_VALUE_WORDS: &str = "This rule has no value.";
/// The words for a domain written as an address.
pub const RULE_DOMAIN_WORDS: &str = "a domain looks like example.com";

/// Why a condition cannot be saved, in words; None when it can. A blank text value, a blank domain or account: "This rule has no value."; a domain with an @: how a domain looks.
#[uniffi::export]
pub fn condition_refusal_words(c: RuleCondition) -> Option<String> {
    if let RuleCondition::Domain { domain } = &c { if !domain.trim().is_empty() && domain.contains('@') { return Some(RULE_DOMAIN_WORDS.into()); } }
    if condition_is_blank(&c) { Some(RULE_NO_VALUE_WORDS.into()) } else { None }
}

fn condition_matches(c: &RuleCondition, f: &MessageFacts) -> bool {
    match c {
        RuleCondition::From { words } => has(&f.from_address, words) || has(&f.from_name, words),
        RuleCondition::To { words } => f.to.iter().any(|t| has(t, words)),
        RuleCondition::Cc { words } => f.cc.iter().any(|t| has(t, words)),
        RuleCondition::Domain { domain } => {
            let d = domain.trim().trim_start_matches('@').to_lowercase();
            let host = f.from_address.rsplit('@').next().unwrap_or("").to_lowercase();
            !d.is_empty() && (host == d || host.ends_with(&format!(".{d}")))
        }
        RuleCondition::SubjectContains { words } => has(&f.subject, words),
        RuleCondition::BodyContains { words } => has(&f.body, words),
        // delta 25: a blank list-header value matches nothing too ("from a mailing list" at all is the fact `MessageFact::MailingList`)
        RuleCondition::ListHeader { words } => !f.list_header.is_empty() && has(&f.list_header, words),
        RuleCondition::Account { account_id } => !account_id.trim().is_empty() && &f.account_id == account_id,
        RuleCondition::HasAttachment => f.has_attachment,
        // delta 17: one of the eight facts, with the value wanted (`fact_holds` is the one reading of each)
        RuleCondition::Fact { fact, wanted } => crate::fact_holds(*fact, f.clone()) == *wanted,
        // round 18: the negated twin holds exactly when its positive twin does not
        RuleCondition::Not { field, value } => !condition_matches(&RuleCondition::positive_twin(*field, value), f),
    }
}

/// Whether one message matches a rule set. An empty set matches nothing, whatever the mode.
#[uniffi::export]
pub fn rule_set_matches(rules: RuleSet, facts: MessageFacts) -> bool {
    if rules.conditions.is_empty() { return false; }
    match rules.match_mode {
        MatchMode::Any => rules.conditions.iter().any(|c| condition_matches(c, &facts)),
        MatchMode::All => rules.conditions.iter().all(|c| condition_matches(c, &facts)),
    }
}

/// The live count of a rule set over a corpus: how many match, how many of those are unread, the newest.
#[uniffi::export]
pub fn count_rule_matches(rules: RuleSet, corpus: Vec<MessageFacts>) -> RuleSetCount {
    let hits: Vec<&MessageFacts> = corpus.iter().filter(|f| rule_set_matches(rules.clone(), (*f).clone())).collect();
    RuleSetCount { count: hits.len() as u32, unread: hits.iter().filter(|f| f.unread).count() as u32, newest_at_ms: hits.iter().map(|f| f.received_at_ms).max() }
}

/// The re-run of one label. `filed` = the ids of the messages now filed under it. With no fixed rules the rules part cannot
/// judge: every filed message stays. `has_description` marks the meaning part pending (it belongs to the decision model),
/// and then a filed message no rule matches STAYS (delta 12): the label's meaning may still hold it, so it leaves only when
/// the decision model has spoken (the decision crate's re-run marks it Pending, never Unfile, while the model is pending).
#[uniffi::export]
pub fn plan_rerun(rules: RuleSet, has_description: bool, corpus: Vec<MessageFacts>, filed: Vec<String>) -> RerunPlan {
    let ruled = !rules.conditions.is_empty();
    let mut plan = RerunPlan { file: vec![], unfile: vec![], keep: vec![], model_pending: has_description };
    for f in &corpus {
        let is_filed = filed.contains(&f.message_id);
        let hit = ruled && rule_set_matches(rules.clone(), f.clone());
        match (is_filed, hit, ruled) {
            (true, true, _) | (true, false, false) => plan.keep.push(f.message_id.clone()),
            (true, false, true) if has_description => plan.keep.push(f.message_id.clone()),
            (true, false, true) => plan.unfile.push(f.message_id.clone()),
            (false, true, _) => plan.file.push(f.message_id.clone()),
            (false, false, _) => {}
        }
    }
    plan
}

/// The line the undo bar's place shows for a suggestion.
#[uniffi::export]
pub fn suggestion_words(kind: SuggestionKind, subject: String) -> String {
    match kind {
        SuggestionKind::BlockSender => format!("Block {subject}"),
        SuggestionKind::AlwaysJunkDomain => format!("Always junk mail from {subject}"),
        SuggestionKind::Unsubscribe => format!("Unsubscribe from {subject}"),
        SuggestionKind::MuteThread => "Mute this thread".to_string(),
        SuggestionKind::AlwaysLabel => format!("Always label {subject}"),
    }
}

/// What accepting writes. Refused: a suggestion already accepted, an empty subject, an always-label with no label.
#[uniffi::export]
pub fn suggestion_effect(suggestion: Suggestion) -> Result<SuggestionEffect, CoreError> {
    if suggestion.accepted { return Err(invalid("suggestion", "this suggestion was already accepted")); }
    let s = suggestion.subject.trim().to_string();
    if s.is_empty() { return Err(invalid("suggestion", "a suggestion needs its subject")); }
    Ok(match suggestion.kind {
        SuggestionKind::BlockSender => SuggestionEffect::Block { target: BlockTarget::Sender { address: s.to_lowercase() } },
        SuggestionKind::AlwaysJunkDomain => SuggestionEffect::Block { target: BlockTarget::Domain { domain: s.trim_start_matches('@').to_lowercase() } },
        SuggestionKind::Unsubscribe => SuggestionEffect::Unsubscribe { subject: s },
        SuggestionKind::MuteThread => SuggestionEffect::MuteThread { thread_id: s },
        SuggestionKind::AlwaysLabel => match suggestion.label_id {
            Some(l) if !l.is_empty() => SuggestionEffect::LabelRule { address: s.to_lowercase(), label_id: l },
            _ => return Err(invalid("suggestion", "an always-label suggestion names its label")),
        },
    })
}

/// Whether a message from `address` is blocked by `blocks` (the sender, or its domain or a sub-domain of it).
#[uniffi::export]
pub fn is_blocked(blocks: Vec<Block>, address: String) -> bool {
    let a = address.trim().to_lowercase();
    let host = a.rsplit('@').next().unwrap_or("").to_string();
    blocks.iter().any(|b| match &b.target {
        BlockTarget::Sender { address } => address.to_lowercase() == a,
        BlockTarget::Domain { domain } => { let d = domain.to_lowercase(); host == d || host.ends_with(&format!(".{d}")) }
    })
}
