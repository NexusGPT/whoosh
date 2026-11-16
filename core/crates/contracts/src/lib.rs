//! The Whoosh contract: the single source of every shared shape.
//!
//! Schema first, types derived: Swift gets these types from UniFFI (`scripts/gen-swift.sh`
//! writes `Sources/CoreBridge/Generated/`), never by hand. Invariants are encoded in the
//! types where a type can hold them; `tests/` proves the rest.
uniffi::setup_scaffolding!("contracts");

pub mod account_setup;
pub mod axid;
pub mod ai_provider_state;
pub mod command;
pub mod constants;
pub mod counts;
pub mod decision;
pub mod decision_consent;
pub mod error;
pub mod events;
pub mod foreign;
pub mod inbox_views;
pub mod label_flow;
pub mod label_tabs;
pub mod labels;
pub mod mail_actions;
pub mod mail_import;
pub mod notch_alerts;
pub mod notch_words;
pub mod oauth;
pub mod ports;
pub mod records;
pub mod reply;
pub mod reply_ai;
pub mod reply_ai_logic;
pub mod reply_ai_state;
pub mod reply_ai_d21;
pub mod reply_ai_d22;
pub mod reply_ai_d23;
pub mod reply_ai_d24;
pub mod views;

pub use account_setup::*;
pub use axid::*;
pub use ai_provider_state::*;
pub use command::*;
pub use counts::*;
pub use decision::*;
pub use decision_consent::*;
pub use error::*;
pub use events::*;
pub use foreign::*;
pub use inbox_views::*;
pub use label_flow::*;
pub use label_tabs::*;
pub use labels::*;
pub use mail_actions::*;
pub use mail_import::*;
pub use notch_alerts::*;
pub use notch_words::*;
pub use oauth::*;
pub use ports::*;
pub use records::*;
pub use reply::*;
pub use reply_ai::*;
pub use reply_ai_logic::*;
pub use reply_ai_state::*;
pub use reply_ai_d21::*;
pub use reply_ai_d22::*;
pub use reply_ai_d23::*;
pub use reply_ai_d24::*;
pub use views::*;
