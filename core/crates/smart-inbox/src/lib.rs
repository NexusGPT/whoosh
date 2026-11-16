//! The smart inbox: which split tab a message sits in, and why.
//!
//! One ordered list per outcome (docs/DECISION-MODEL.md version 2): rule entries first, the "ask the model" entry
//! where the owner placed it, the default last, first match wins. A placement always carries its `decided_by`.
//!
//! - `facts` what a placement reads (headers, labels, sender history); `list` the entries; `place` the first-match walk;
//! - `ask` the one port a model entry plugs into (the decision lane implements it);
//! - `tabs` split tabs as data; `load` reads them and the facts from the store;
//! - one file per use case: `place_new`, `edit_tabs`, `edit_vips`, `resort`, `counts`, `notify`, `remind`;
//! - `corpus` the live count of a rule set over the stored mail; `thread` the rest of a message's thread (labels-r5).
//!
//! Third-party code: rusqlite (MIT), serde_json (MIT or Apache-2.0). Nothing is copied.
pub mod ask;
pub mod cond;
pub mod corpus;
pub mod counts;
pub mod edit_tabs;
pub mod edit_vips;
pub mod error;
pub mod facts;
pub mod list;
pub mod load;
pub mod notify;
pub mod place;
pub mod place_new;
pub mod remind;
pub mod resort;
pub mod tabs;
pub mod thread;
pub mod tuning;

pub use ask::{AskKind, AskModel, ModelAnswer, ModelAsk, ModelVerdict, NoModel, ViaDecisionPort};
pub use counts::{account_counts, awaiting_reply_threads, AccountTabCounts};
pub use edit_tabs::{EditSplitTabs, TabEdit};
pub use edit_vips::{EditVips, VipEdit};
pub use error::SmartInboxError;
pub use facts::{header_facts, GmailCategory, HeaderFacts, MessageFacts, SenderHistory, ThreadPart};
pub use list::{Cond, DecisionList, DecisionPage, Entry, EntryKind, TabRef, Then};
pub use notify::{should_notify, NotifyPolicy};
pub use place::{Placement, Placer};
pub use place_new::{Incoming, PlaceNewMessages};
pub use remind::{DueReminder, RemindIfNoReply};
pub use resort::{resort, ResortReport, Scope};
pub use tabs::TabSet;
