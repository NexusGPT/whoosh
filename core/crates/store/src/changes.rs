//! The change feed. The writer connection reports every row change SQLite makes (the update hook);
//! at commit those collapse to ONE `TableChange` per touched table. The contract's streams are fed
//! from the same commit, so a write that touches three tables still emits one event per stream.
use contracts::{CoreEvent, EventSink, StreamKind};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

/// What one commit did to one table.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TableChange {
    pub table: String,
    pub inserts: u32,
    pub updates: u32,
    pub deletes: u32,
}

pub type TableSink = Arc<dyn Fn(&TableChange) + Send + Sync>;

/// Which streams a change in `table` can alter. The inbox, counts and thread streams are produced
/// here; the others are listed so the lanes that own them (sync, outbox) subscribe to the table feed.
pub fn streams_for_table(table: &str) -> &'static [StreamKind] {
    match table {
        "message" => &[StreamKind::Inbox, StreamKind::Counts, StreamKind::Thread],
        "message_category" | "snooze" | "split_tab" => &[StreamKind::Inbox, StreamKind::Counts],
        "pin" => &[StreamKind::Inbox, StreamKind::Counts],
        // a pinned owner label is a tab: pinning it, renaming it or filing a mail under it moves the counts' tabs (writer.rs COUNTS_TABLES lists both)
        "label_assignment" | "label" => &[StreamKind::Inbox, StreamKind::Counts],
        "attachment" => &[StreamKind::Inbox],
        "thread" => &[StreamKind::Inbox, StreamKind::Thread],
        "draft" => &[StreamKind::Counts],
        "outbox_entry" => &[StreamKind::Counts, StreamKind::Outbox],
        "account" | "account_sync_state" | "folder_sync_state" => &[StreamKind::SyncStatus],
        _ => &[],
    }
}

#[derive(Default)]
pub struct Bus {
    next: AtomicU64,
    sinks: Mutex<Vec<(u64, StreamKind, Arc<dyn EventSink>)>>,
    table_sinks: Mutex<Vec<(u64, TableSink)>>,
}

impl Bus {
    pub fn subscribe(&self, stream: StreamKind, sink: Arc<dyn EventSink>) -> u64 {
        let id = self.next.fetch_add(1, Ordering::Relaxed) + 1;
        self.sinks.lock().unwrap_or_else(|e| e.into_inner()).push((id, stream, sink));
        id
    }

    pub fn subscribe_tables(&self, sink: TableSink) -> u64 {
        let id = self.next.fetch_add(1, Ordering::Relaxed) + 1;
        self.table_sinks.lock().unwrap_or_else(|e| e.into_inner()).push((id, sink));
        id
    }

    pub fn unsubscribe(&self, id: u64) {
        self.sinks.lock().unwrap_or_else(|e| e.into_inner()).retain(|(i, _, _)| *i != id);
        self.table_sinks.lock().unwrap_or_else(|e| e.into_inner()).retain(|(i, _)| *i != id);
    }

    /// Deliver one event to every sink of `stream`. The lock is released before any sink runs.
    pub fn emit(&self, stream: StreamKind, event: CoreEvent) {
        let sinks: Vec<Arc<dyn EventSink>> = self
            .sinks
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
            .filter(|(_, s, _)| *s == stream)
            .map(|(_, _, sink)| sink.clone())
            .collect();
        for sink in sinks {
            sink.on_event(event.clone());
        }
    }

    pub fn emit_table(&self, change: &TableChange) {
        let sinks: Vec<TableSink> = self.table_sinks.lock().unwrap_or_else(|e| e.into_inner()).iter().map(|(_, s)| s.clone()).collect();
        for sink in sinks {
            sink(change);
        }
    }
}
