//! Delta 16 (docs/PRD.md "A91 shape", owner card 2026-11-09 16:30): Primary and All are VIEWS of the inbox, not labels.
//! "Main is primary, so keep primary, just don't make it a Label because it is confusing ... we should be able to have
//! 'All' the mail (except spam) too ... (again not a label)."
//!
//! A view has no label row, no label chip, and cannot be applied, removed, renamed, moved, pinned, unpinned, deleted or
//! re-run. It is NOT stored: membership is derived at read from what the message already carries (`view_contains`), so
//! no migration and no second copy of a derived value. The tab list the UI draws is `inbox_tabs`: Primary, All, then the
//! pinned labels in tree order. Unread is not a tab: it is `InboxFilter::Unread` (delta 10), the chip on any tab.
//!
//! Tab ids stay what they were: Primary keeps "primary" (the id of the old tab and of the decision model's default
//! category, so no stored tab id, rule or query changes) and All is "all". `InboxQuery.tab_id` carries either one, exactly
//! as it carries a label id; `Counts.tabs` carries one `TabCounts` per tab of the list, views first.
use crate::{
    pinned_in_tree_order, Command, CoreError, LabelKind, LabelSpec, MailLabel, SplitTabRule, TabCounts,
};

/// The stable code of every refusal that says "a view is not a label". It is the `field` of the `CoreError::Invalid`
/// (the sentence is `reason_words`), so the UI can tell this refusal from a bad name without parsing words.
pub const VIEW_REFUSAL_CODE: &str = "view_not_a_label";

/// The two views. Closed on purpose: a third view is a contract change with its own decision.
#[derive(uniffi::Enum, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InboxView {
    /// inbox mail the sorting model filed into no other category
    Primary,
    /// every inbox message of every category, Junk and Trash excluded
    All,
}

/// What a tab of the strip is: a view (derived, fixed) or a pinned label (the owner's).
#[derive(uniffi::Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum InboxTabKind { View, Label }

/// One tab of the strip, left to right. `id` is the `InboxQuery.tab_id` and the `TabCounts.tab_id` of that tab.
#[derive(uniffi::Record, Debug, Clone, PartialEq, Eq)]
pub struct InboxTab {
    pub id: String,
    pub name: String,
    pub kind: InboxTabKind,
}

/// What a view needs to know about one message. The core fills it from its own columns; `in_inbox` is the same test the
/// inbox list uses (not archived, not snoozed, not muted, not outgoing).
#[derive(uniffi::Record, Debug, Clone, PartialEq, Eq)]
pub struct ViewFacts {
    /// the category the sorting model (or the owner, or a rule) filed it in; "" = not filed yet, which reads as primary
    pub category_id: String,
    pub in_inbox: bool,
    pub junk: bool,
    pub trashed: bool,
}

/// One message as the view counts read it.
#[derive(uniffi::Record, Debug, Clone, PartialEq, Eq)]
pub struct ViewMessage {
    pub facts: ViewFacts,
    pub unread: bool,
    /// an inbound thread awaiting the owner's reply (the `waiting` definition)
    pub awaiting_reply: bool,
}

/// What the owner does with a label that a view refuses. Names the verb in the refusal's sentence.
#[derive(uniffi::Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewRefusedAction { Apply, Remove, Rename, Move, Pin, Unpin, Delete, Rerun, Reorder, Edit, AddUnder, Hide }

const PRIMARY_ID: &str = "primary";
const ALL_ID: &str = "all";

/// "primary" / "all": the tab id of a view.
#[uniffi::export]
pub fn inbox_view_id(view: InboxView) -> String {
    match view { InboxView::Primary => PRIMARY_ID, InboxView::All => ALL_ID }.to_string()
}

/// "Primary" / "All": the name on the tab and in the refusals (the strings table owns the translations; these are the keys' English).
#[uniffi::export]
pub fn inbox_view_name(view: InboxView) -> String {
    match view { InboxView::Primary => "Primary", InboxView::All => "All" }.to_string()
}

/// The view a tab id names; None for a label id. Exact match: ids are lower case.
#[uniffi::export]
pub fn inbox_view_from_id(id: String) -> Option<InboxView> {
    match id.as_str() { PRIMARY_ID => Some(InboxView::Primary), ALL_ID => Some(InboxView::All), _ => None }
}

/// The views in tab order: Primary, then All.
#[uniffi::export]
pub fn inbox_views() -> Vec<InboxView> { vec![InboxView::Primary, InboxView::All] }

/// Whether a message is in a view. Junk and Trash are in neither. Primary = in the inbox and filed in no other category
/// (an owner label does not move a message out of Primary: labels are a second axis, views read the category only). A category
/// that is not "primary" is another category, whatever its id: the store's `COALESCE(category, 'primary') = 'primary'`.
#[uniffi::export]
pub fn view_contains(view: InboxView, facts: ViewFacts) -> bool {
    if !facts.in_inbox || facts.junk || facts.trashed { return false; }
    match view {
        InboxView::All => true,
        InboxView::Primary => {
            let c = facts.category_id.trim();
            c.is_empty() || c == PRIMARY_ID
        }
    }
}

/// The counts of one view over a corpus, in the shape `Counts.tabs` carries: unread = unread members, waiting = members
/// awaiting the owner's reply. Same membership as the list, so the badge and the rows cannot disagree.
#[uniffi::export]
pub fn count_view(view: InboxView, messages: Vec<ViewMessage>) -> TabCounts {
    let members: Vec<&ViewMessage> = messages.iter().filter(|m| view_contains(view, m.facts.clone())).collect();
    TabCounts {
        tab_id: inbox_view_id(view),
        unread: members.iter().filter(|m| m.unread).count() as u32,
        waiting: members.iter().filter(|m| m.awaiting_reply).count() as u32,
    }
}

/// True when this label row is a legacy copy of a view (a store seeded before delta 16 holds a built-in "primary" label).
fn is_view_row(l: &MailLabel) -> bool { l.kind == LabelKind::BuiltIn && l.parent_id.is_none() && inbox_view_from_id(l.id.clone()).is_some() }

/// The label list without any row that is really a view: what `labels()` and `pinned_labels()` serve, so a store seeded
/// before delta 16 shows no "Primary" label in the sidebar and no chip. Order and every other row are untouched.
#[uniffi::export]
pub fn labels_without_views(labels: Vec<MailLabel>) -> Vec<MailLabel> {
    labels.into_iter().filter(|l| !is_view_row(l)).collect()
}

/// The tab strip, left to right: Primary, All, then the pinned labels in tree order. `labels` is every label (view rows
/// are dropped, never shown twice). The list is never empty, so "the last pinned label cannot be unpinned" is no longer a
/// reason to refuse an unpin.
#[uniffi::export]
pub fn inbox_tabs(labels: Vec<MailLabel>) -> Vec<InboxTab> {
    let mut tabs: Vec<InboxTab> = inbox_views().into_iter()
        .map(|v| InboxTab { id: inbox_view_id(v), name: inbox_view_name(v), kind: InboxTabKind::View }).collect();
    tabs.extend(pinned_in_tree_order(labels_without_views(labels)).into_iter()
        .map(|l| InboxTab { id: l.id, name: l.name, kind: InboxTabKind::Label }));
    tabs
}

// ---------------------------------------------------------------- refusals

fn refuse(words: String) -> CoreError { CoreError::Invalid { field: VIEW_REFUSAL_CODE.into(), reason_words: words } }

fn verb(a: ViewRefusedAction) -> &'static str {
    match a {
        ViewRefusedAction::Apply => "applied to a message",
        ViewRefusedAction::Remove => "removed from a message",
        ViewRefusedAction::Rename => "renamed",
        ViewRefusedAction::Move => "moved",
        ViewRefusedAction::Pin => "pinned: the views are always the first tabs",
        ViewRefusedAction::Unpin => "unpinned: the views are always the first tabs",
        ViewRefusedAction::Delete => "deleted",
        ViewRefusedAction::Rerun => "re-run",
        ViewRefusedAction::Reorder => "reordered: the views stay first",
        ViewRefusedAction::Edit => "edited",
        ViewRefusedAction::AddUnder => "a parent: a label cannot go under a view",
        ViewRefusedAction::Hide => "hidden",
    }
}

/// Refuses an action on a label id when that id is a view, in words, with `VIEW_REFUSAL_CODE`. A real label passes.
#[uniffi::export]
pub fn check_not_a_view(label_id: String, action: ViewRefusedAction) -> Result<(), CoreError> {
    let Some(v) = inbox_view_from_id(label_id) else { return Ok(()) };
    let name = inbox_view_name(v);
    Err(refuse(format!("{name} is a view of the inbox, not a label, so it cannot be {}.", verb(action))))
}

/// Refuses a TOP-LEVEL label named like a view ("Primary", "All", any case): the strip would show two tabs of one name.
/// Under a parent the name is free ("Clients > All" is an ordinary label). `parent_id` is the parent's id, None = top level.
#[uniffi::export]
pub fn check_not_a_view_name(name: String, parent_id: Option<String>) -> Result<(), CoreError> {
    if let Some(p) = &parent_id { check_not_a_view(p.clone(), ViewRefusedAction::AddUnder)?; return Ok(()); }
    let n = name.trim();
    if let Some(v) = inbox_views().into_iter().find(|v| inbox_view_name(*v).eq_ignore_ascii_case(n)) {
        return Err(refuse(format!("{} is the name of a view of the inbox, so a top-level label cannot use it. Pick another name.", inbox_view_name(v))));
    }
    Ok(())
}

fn first_segment(path: &str) -> &str { path.split('>').next().unwrap_or("").trim() }

fn spec_ok(spec: &LabelSpec) -> Result<(), CoreError> { check_not_a_view_name(spec.name.clone(), spec.parent_id.clone()) }

/// A split-tab order may name a view (the order lists every tab, Primary is one), but the views stay first, in the views' order: a view id after
/// a label id, or before a view that comes earlier in `inbox_views`, is a move and is refused in the views' words. A view listed twice is not a
/// move: the store answers it ("the order must list every tab once").
fn check_tab_order(ids: &[String]) -> Result<(), CoreError> {
    let views = inbox_views();
    let place = |id: &str| inbox_view_from_id(id.to_string()).and_then(|v| views.iter().position(|x| *x == v));
    let (mut last, mut leading) = (0usize, true);
    for id in ids {
        match place(id) {
            Some(p) => {
                if !leading || p < last { return check_not_a_view(id.clone(), ViewRefusedAction::Reorder); }
                last = p;
            }
            None => leading = false,
        }
    }
    Ok(())
}

/// The one door: refuses, in words with `VIEW_REFUSAL_CODE`, every command that would treat a view as a label or a tab the
/// owner can edit: apply (`Label` by name, `MoveToLabel`), remove (`RemoveLabel`), rename, move, reorder (a split-tab order that moves a view), pin, unpin,
/// delete, re-run, update, create a label under a view or named like one, and the split-tab commands naming a view id.
/// Every other command passes. Both cores call it first in `run`, so a refusal is the same in the Rust core and the fixture.
#[uniffi::export]
pub fn check_view_command(command: Command) -> Result<(), CoreError> {
    use ViewRefusedAction as A;
    match command {
        Command::Label { label_name, .. } => check_not_a_view_name(first_segment(&label_name).to_string(), None)
            .map_err(|_| refuse(format!("{} is a view of the inbox, not a label, so it cannot be applied to a message.", first_segment(&label_name)))),
        Command::MoveToLabel { label_id, .. } => check_not_a_view(label_id, A::Apply),
        Command::RemoveLabel { label_id, .. } => check_not_a_view(label_id, A::Remove),
        Command::RenameLabel { label_id, .. } => check_not_a_view(label_id, A::Rename),
        Command::MoveLabel { label_id, parent_id } => { check_not_a_view(label_id, A::Move)?; match parent_id { Some(p) => check_not_a_view(p, A::AddUnder), None => Ok(()) } }
        Command::CreateLabel { name, parent_id } => check_not_a_view_name(name, parent_id),
        Command::CreateLabelFull { spec } => spec_ok(&spec),
        Command::UpdateLabel { label_id, spec } => { check_not_a_view(label_id, A::Edit)?; spec_ok(&spec) }
        Command::ReorderLabel { label_id, new_parent_id, .. } => { check_not_a_view(label_id, A::Reorder)?; match new_parent_id { Some(p) => check_not_a_view(p, A::AddUnder), None => Ok(()) } }
        Command::PinLabel { label_id, pinned } => check_not_a_view(label_id, if pinned { A::Pin } else { A::Unpin }),
        Command::DeleteLabel { label_id } => check_not_a_view(label_id, A::Delete),
        Command::RerunLabel { label_id } => check_not_a_view(label_id, A::Rerun),
        // delta 17: the flow's commands edit a label; a view has no flow, no card, no chain
        Command::SetLabelCard { label_id, .. } | Command::SetLabelAiReads { label_id, .. } | Command::SetLabelExitFallback { label_id, .. }
        | Command::AddLabelChain { label_id, .. } | Command::UpdateLabelChain { label_id, .. } | Command::RemoveLabelChain { label_id, .. } => check_not_a_view(label_id, A::Edit),
        Command::RenameSplitTab { tab_id, .. } => check_not_a_view(tab_id, A::Rename),
        Command::SetSplitTabEnabled { id, on } => check_not_a_view(id, if on { A::Pin } else { A::Hide }),
        Command::RemoveSplitTab { id } => check_not_a_view(id, A::Delete),
        Command::ReorderSplitTabs { ids } => check_tab_order(&ids),
        Command::AddSplitTab { rule: SplitTabRule::Label { label_id }, .. } => check_not_a_view(label_id, A::AddUnder),
        _ => Ok(()),
    }
}
