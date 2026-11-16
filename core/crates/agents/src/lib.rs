//! Whoosh agents: the runtime behind "Ask my inbox" (J5.4-J5.6), the built-in Writer (C-J3.3) and approved proposals.
//!
//! An agent is defined by what it may do, in words (read mail, search, draft, label, send with your confirmation).
//! The model is offered the tools those words name and no other; a call to any other tool is refused and does nothing.
//! Answers cite the messages they rest on (account, sender, date, message id), and only messages the agent was shown.
//! A label or a send is a PROPOSAL: it is stored, shown, and applied once, only after the owner approves it.
//! Before any mail text goes to a provider off the device, the owner is asked once per provider.
//! A run stops at AGENT_MAX_STEPS model calls (docs/CONSTANTS.md).
//!
//! Open-source start: Zero (Mail-0/Zero, MIT; research/ai-providers.md:36,47,49,52) for the pattern only: an editable
//! prompt with a default when the stored one is empty, and a mail tool set (labels, read state, send). Zero has no confirm
//! step and no step limit (research/ai-providers.md:49,53); both are this crate's additions. No code or prompt text is copied.

pub mod adapters;
pub mod builtin;
pub mod domain;
pub mod ports;
pub mod runtime;
pub mod usecases;

pub use contracts::AnswerKind;
pub use domain::answer::{AgentAnswer, CitedMessage};
pub use domain::event::AgentEvent;
pub use domain::permission::{allowed_words, offered, words, Tool};
pub use domain::proposal::StoredProposal;
pub use runtime::{AgentPorts, AgentRuntime};
pub use usecases::ask::{AskOutcome, AskRequest, AskTurn};
