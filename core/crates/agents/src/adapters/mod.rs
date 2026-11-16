//! Adapters this crate ships: in-memory consent and proposal books (tests, and the core until the store binds the
//! agent-proposal and consent rows), and the system clock.
pub mod memory;
pub mod system_clock;
