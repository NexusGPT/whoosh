//! Whoosh hybrid search (work item fn-search-hybrid).
//!
//! The owner types one line. `parse_query` turns the filter words in it (from, before, after, has attachment,
//! unread, account) into chips; the rest is searched twice, as exact words (the text index, crate `search-text`)
//! and as meaning (the vector index, crate `search-vector`). `fuse_ranks` merges the two ranked lists by
//! reciprocal rank with k = `contracts::constants::RRF_K`; `group_threads` folds the fused messages into
//! threads with snippets and marked words. `Searcher::run` orchestrates the ports; it never touches an index
//! itself, so the FFI lane binds `Searcher` with adapters over the two sibling crates.
//!
//! Third-party code: none. The ports carry `contracts::SearchHit`, `contracts::Message` and `contracts::CoreError`.

pub mod adapters;
pub mod domain;
pub mod ports;
pub mod usecases;

pub use domain::dates::CivilDate;
pub use domain::{AccountState, Chip, ChipKind, Filters, MessageMatch, ParsedQuery, SearchOutcome, Segment, ThreadResult};
pub use adapters::store_attachments::StoreAttachments;
pub use ports::{AccountsPort, AttachmentPort, FactsPort, FilterScanPort, MeaningPort, TextPort};
pub use usecases::attachment_hits::attachment_hits;
pub use usecases::fuse_ranks::{add_ranked_list, fuse_ranks, Fused};
pub use usecases::group_threads::group_threads;
pub use usecases::highlight::mark_words;
pub use usecases::parse_query::{parse_query, ParseContext};
pub use usecases::run_search::{SearchRequest, Searcher};
pub use usecases::smart_folder::{new_smart_folder, run_smart_folder, smart_folder_rows, SmartFolderView};
