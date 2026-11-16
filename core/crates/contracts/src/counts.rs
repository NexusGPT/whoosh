//! The two counts the app shows (docs/PRD.md:33: "no aggregates beyond counts (unread, waiting)").
//! One query each, defined here once; docs/DIGEST.md repeats these words and a test holds them equal.
use std::collections::HashMap;

pub const UNREAD_DEFINITION: &str = "unread = unread messages in the tab across the included accounts";
pub const WAITING_DEFINITION: &str = "waiting = inbound threads awaiting the owner's reply";

/// The counts of one split tab.
#[derive(uniffi::Record, Debug, Clone, PartialEq, Eq)]
pub struct TabCounts {
    pub tab_id: String,
    /// unread = unread messages in the tab across the included accounts
    pub unread: u32,
    /// waiting = inbound threads awaiting the owner's reply
    pub waiting: u32,
}

/// Every count the inbox shows at one moment: per split tab, and per mailbox (Snoozed, Drafts, ...).
#[derive(uniffi::Record, Debug, Clone, PartialEq, Eq)]
pub struct Counts {
    pub tabs: Vec<TabCounts>,
    pub mailboxes: HashMap<String, u32>,
}
