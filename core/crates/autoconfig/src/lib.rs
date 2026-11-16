//! Mail provider autodetection for adding an account by IMAP: the Mozilla Autoconfig flow in the owner's
//! order (docs/PRD.md "## Add account by IMAP with provider autodetection"). Every network call goes through
//! a port (HttpPort, DnsPort, ProbePort); the real implementations are in `adapters`.
pub mod adapters;
pub mod consts;
pub mod domain;
pub mod parse;
pub mod ports;
mod steps;
pub mod usecases;
mod xml;

pub use domain::address::EmailAddress;
pub use domain::config::{AuthMethod, DetectionResult, ImapOnlyConfig, MailConfig, Outcome, Provider, Refusal, Security, Source, Step};
pub use ports::*;
pub use usecases::detect_provider::{user_agent, Detector, STEP_ORDER};
