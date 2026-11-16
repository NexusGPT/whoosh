//! What the index stores for a source other than mail: excerpts of an item (a file, a calendar event), and the item's stamp, by which a
//! later sync knows the item has not changed.
use contracts::{reply_source_key, ReplyExcerpt, ReplySourceRef};

/// One excerpt of a folder of this Mac (a paragraph of a file) or of a calendar (one event).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceExcerpt {
    /// stable across syncs: the source key, the item key and the ordinal (`excerpt_id`)
    pub id: String,
    pub source: ReplySourceRef,
    /// the file's path inside the folder ("Contrats/2026/acme.pdf"), or the event's id and start ("<event id>@<start ms>")
    pub item_key: String,
    /// 0 for the first excerpt of the item
    pub ordinal: u32,
    /// the file's title (a markdown "# " heading, an HTML <title>, else the file name without its extension), or the event's title
    pub title: String,
    /// where it lives as the owner reads it: the file's whole path ("~/Documents/Contrats/acme.pdf"), or the calendar's name
    pub folder: String,
    /// the file's modified date, or the event's start
    pub date_ms: Option<i64>,
    pub text: String,
}

impl SourceExcerpt {
    /// What a reader of a draft or a try sees (the contract's `ReplyExcerpt`): the title in the subject slot, the path or calendar in
    /// the folder slot, no sender.
    pub fn to_reply_excerpt(&self) -> ReplyExcerpt {
        ReplyExcerpt {
            chunk_id: self.id.clone(),
            source: self.source.clone(),
            date_ms: self.date_ms,
            sender: None,
            folder: Some(self.folder.clone()),
            subject: Some(self.title.clone()),
            text: self.text.clone(),
        }
    }
}

/// The id of an excerpt: the source key (the contract's `reply_source_key`), the item key, the ordinal, joined by U+001F.
pub fn excerpt_id(source: &ReplySourceRef, item_key: &str, ordinal: u32) -> String {
    format!("{}\u{1f}{item_key}\u{1f}{ordinal}", reply_source_key(source.clone()))
}

/// What the index remembers of one item beside its excerpts. `version` is "<size>:<modified ms>" for a file and a hash of the text for
/// an event: the same version means the item is not read again. `excerpt_count` 0 = the item is known and holds no text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemStamp {
    pub item_key: String,
    pub version: String,
    pub excerpt_count: u32,
}
