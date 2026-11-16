//! The adapters of the draft engine's ports, one file per outside system: the model (the providers table), the mail database (its own read
//! connection), the indexes (words, meaning, the in-memory excerpts), the progress of a source.
pub mod calendars;
pub mod excerpt_index;
pub mod mail_reads;
pub mod memory_excerpts;
pub mod model_ports;
pub mod progress;
pub mod provider;
pub mod stored_excerpts;
