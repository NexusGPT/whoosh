//! What an attachment CONTAINS, ready for the word and meaning indexes (docs/PRD.md "Search covers everything a message carries").
//! PDF and document text, OCR text and scene labels of images, speech of audio and video, and one vector per image, audio file,
//! sampled video frame and text chunk, embedded through the contract `Embedder` (EmbeddingGemma 2). The OS pieces (PDFKit, Vision,
//! Speech, AVFoundation) are ports in `ports`, answered by Sources/Features/AttachmentIndex; `twins` holds the fixture answers.
pub mod constants {
    include!(concat!(env!("OUT_DIR"), "/constants.rs"));
}

mod chunk;
mod entry;
mod hits;
mod indexer;
mod indexer_media;
mod kind;
mod pdf;
mod plan;
mod ports;
mod rows;
pub mod twins;

pub use entry::{AttachmentIndexEntry, EntryVector, IndexState, NOT_DOWNLOADED_WORDS, UNREADABLE_WORDS};
pub use hits::{hits_for_name, hits_for_text, hits_for_vectors, merge_attachment_hits, CatalogRow, VectorMatch};
pub use indexer::{AttachmentIndexer, AttachmentInput, Limits, Readers};
pub use kind::AttachmentKind;
pub use pdf::extract_pdf_text;
pub use plan::{frame_times, video_plan, MediaEmbedRequest, MediaPart, TranscriptRequest};
pub use ports::{ImageReader, MediaProbe, TextReader, Transcriber, TranscriptOutcome};
pub use rows::{AttachmentRows, SqliteRows};

/// True when indexing may run: on battery under ATTACHMENT_INDEX_BATTERY_MIN percent it waits (docs/CONSTANTS.md).
pub fn may_index_now(battery_percent: Option<u8>, charging: bool) -> bool {
    charging || battery_percent.map_or(true, |p| p as usize >= constants::ATTACHMENT_INDEX_BATTERY_MIN)
}
