//! Whoosh decisions (docs/DECISION-MODEL.md version 2, signed "Yes, build it this way").
//!
//! Settings › Decisions holds one page per thing Whoosh decides about a message. Each page is ONE ordered list:
//! rule entries and "ask the model" entries share it, first match wins, a default closes it. Only the pages the owner
//! switched on are decided. Every verdict names the page and the entry that decided it (`DecidedByEntry`).
//!
//! - `outcome` the six pages; `entry` the list grammar and the shipped lists; `verdict` what a walk returns;
//! - `settings` which pages are on, which engine answers model entries; `consent` the once-per-provider gate;
//! - one file per use case: `decide` (one message, one engine call), `preview` (20 recent messages, never writes),
//!   `rerun` ("Re-run on N filed messages" after a label changed, never writes);
//! - `labels` the Labels page from the owner's label records (delta 9): fixed rules first, then the label's meaning;
//! - `flow` delta 17's label flow (entry rules, AI sorting, exit filter with its fallback) on the Labels verdict; `flip` the
//!   re-filing when a fact flips (replied, read, starred...): entry and exit again, never the model;
//! - `thread_context` the rest of the thread as a fourth input of a label's AI card, cut to each model's size budget, oldest first;
//! - `engine` plugs the engine into the smart inbox's "ask the model" hook (`smart_inbox::AskModel`);
//! - adapters: `adapters::jev` (TypeSafe's classifier, the owner's key), `adapters::local` (a downloaded model such as
//!   Laya, through `contracts::LocalModel`, with the authors' option budget).
//!
//! Started from: the smart-inbox lane's conditions and its two shipped lists (reused, not retyped), the ai-providers
//! lane's HTTP seam, the secrets lane's ReadSecret port, the ondeviceai lane's port of Laya's `build_sequence`
//! (Apache-2.0, convaiinnovations/laya), the org's Jev client request shape (retrieval-dossier-jev.md).
pub mod adapters;
pub mod consent;
pub mod decide;
pub mod engine;
pub mod entry;
pub mod flip;
pub mod flow;
pub mod labels;
pub mod outcome;
pub mod preview;
pub mod rerun;
pub mod settings;
pub mod test_answers;
pub mod thread_context;
pub mod tuning;
pub mod verdict;

pub use consent::{HasConsent, MemoryConsent};
pub use decide::{DecideMessage, Decisions};
pub use engine::EngineAsk;
pub use entry::{Entry, EntryKind, LabelNode, NodeChain, OutcomeList, TabOption, Value, When};
pub use flip::{CurrentFiling, FlipAction, RefileOnFact};
pub use flow::{FlowPlace, FlowVerdict};
pub use labels::{label_rule_count, meaning_prompt, rule_facts, LABELS_MODEL_ID, LABELS_QUESTION};
pub use outcome::Outcome;
pub use preview::{Preview, PreviewDecisions, PreviewInput, PreviewRow};
pub use rerun::{FileWrite, Rerun, RerunAction, RerunLabel, RerunRow};
pub use settings::{DecisionSettings, Page};
pub use verdict::{DecidedByEntry, LabelProposal, Source, Verdict};
