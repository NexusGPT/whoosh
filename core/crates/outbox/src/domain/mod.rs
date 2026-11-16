//! The outbox domain: one record per outgoing message and the only ways it may change state.
mod codec;
mod record;

pub use codec::{from_row, row_to_wire, to_row, to_wire_row, wire_to_row, RowData};
pub(crate) use codec::{draft_from_value, draft_to_value};
pub use record::{CopyPending, Record};
