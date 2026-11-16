//! The one adapter this crate ships: the disk for `FileTree`. The calendars (EventKit), the document readers and the index are the
//! app's and the engine's.
mod disk;

pub use disk::DiskTree;
