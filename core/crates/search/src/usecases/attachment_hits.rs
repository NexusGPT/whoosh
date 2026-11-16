//! The attachment hit lists fused into one (contracts delta 10 hook): words read out of the files, the files' names, and
//! the meaning of their parts (the vector index's text keys), one row per attachment, a content hit before a name hit.
use attachment_index::{hits_for_name, hits_for_text, hits_for_vectors, merge_attachment_hits, CatalogRow, VectorMatch};
use contracts::SearchHit;

pub fn attachment_hits(words: &str, catalog: &[CatalogRow], vectors: &[VectorMatch]) -> Vec<SearchHit> {
    merge_attachment_hits(vec![hits_for_text(words, catalog), hits_for_name(words, catalog), hits_for_vectors(vectors, catalog)])
}
