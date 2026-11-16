//! Ports: one method each, named `execute`. The core wiring binds them: search to the search crate, mail to the store,
//! apply to `actions::Actions::run`, consent and proposals to the store's agent-proposal and settings rows.
pub mod agent_events;
pub mod apply_command;
pub mod clock;
pub mod consent_given;
pub mod decide_proposal;
pub mod read_mail;
pub mod record_consent;
pub mod resolve_provider;
pub mod save_proposal;
pub mod search_mail;

pub use agent_events::AgentEvents;
pub use apply_command::ApplyCommand;
pub use clock::Clock;
pub use consent_given::ConsentGiven;
pub use decide_proposal::DecideProposal;
pub use read_mail::{MailExcerpt, ReadMail};
pub use record_consent::RecordConsent;
pub use resolve_provider::{BoundProvider, ResolveProvider};
pub use save_proposal::SaveProposal;
pub use search_mail::SearchMail;
