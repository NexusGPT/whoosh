//! One test crate for the relay: every test spawns the real binary (and the local APNs stub).
mod apns_send;
mod common;
mod inbound_reply;
mod registry_api;
mod schema;
mod tracking;
