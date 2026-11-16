mod common;
use common::{body, FakeEmbedder};
use contracts::{CoreError, Embedder};
use search_vector::constants::{CHUNK_OVERLAP as O, CHUNK_TOKENS as T, EMBED_DIM};
use search_vector::{expected_chunk_count, VectorIndex};
use std::sync::Arc;

fn open(dir: &std::path::Path) -> VectorIndex { VectorIndex::open(dir, Arc::new(FakeEmbedder::new())).unwrap() }
fn top(ix: &VectorIndex, text: &str) -> Option<(u64, u16)> { ix.search(text, 1).unwrap().first().map(|h| (h.message_key, h.chunk)) }

#[test]
fn a_message_is_stored_as_the_chunks_the_constants_give() {
    let dir = tempfile::tempdir().unwrap();
    let ix = open(dir.path());
    let n = 3 * T + 5;
    assert_eq!(ix.index_message(7, &body(n, 7)).unwrap(), expected_chunk_count(n, T, O));
    assert_eq!(ix.chunk_count(), expected_chunk_count(n, T, O));
    assert_eq!(ix.index_message(8, "").unwrap(), 0);
}

#[test]
fn the_index_file_survives_a_relaunch() {
    let dir = tempfile::tempdir().unwrap();
    let ix = open(dir.path());
    for m in 1..=20u64 { ix.index_message(m, &body(T + 30, m)).unwrap(); }
    ix.compact(1..=20).unwrap();
    let before = ix.chunk_count();
    drop(ix);
    let again = open(dir.path());
    assert!(again.is_mapped(), "the base is mapped, not read in");
    assert_eq!(again.chunk_count(), before);
    for m in [1u64, 10, 20] {
        let chunks = chunk_text_of(m);
        assert_eq!(top(&again, &chunks[1]), Some((m, 1)), "message {m}, second chunk, after the relaunch");
    }
}

fn chunk_text_of(m: u64) -> Vec<String> {
    search_vector::chunk_text(&body(T + 30, m), &search_vector::WhitespaceTokenizer, T, O).unwrap().into_iter().map(|c| c.text).collect()
}

#[test]
fn new_mail_and_removals_survive_a_relaunch_without_a_compaction() {
    let dir = tempfile::tempdir().unwrap();
    let ix = open(dir.path());
    for m in 1..=5u64 { ix.index_message(m, &body(40, m)).unwrap(); }
    ix.compact(1..=5).unwrap();
    ix.index_message(6, &body(40, 6)).unwrap();
    ix.remove_message(2).unwrap();
    ix.commit().unwrap();
    drop(ix);
    let again = open(dir.path());
    assert_eq!(top(&again, &body(40, 6)), Some((6, 0)), "mail that was only in the delta");
    assert_ne!(top(&again, &body(40, 2)).map(|t| t.0), Some(2), "a removed message is gone from the mapped base");
    assert_eq!(top(&again, &body(40, 3)), Some((3, 0)), "the rest of the base answers");
}

#[test]
fn indexing_a_message_again_replaces_the_old_version() {
    let dir = tempfile::tempdir().unwrap();
    let ix = open(dir.path());
    ix.index_message(1, &body(40, 100)).unwrap();
    ix.compact([1]).unwrap();
    ix.index_message(1, &body(40, 200)).unwrap();
    assert_eq!(top(&ix, &body(40, 200)), Some((1, 0)));
    assert!(ix.search(&body(40, 100), 5).unwrap().iter().all(|h| h.score < 0.99), "the old text no longer matches itself");
}

#[test]
fn compaction_drops_removed_messages_and_keeps_the_rest() {
    let dir = tempfile::tempdir().unwrap();
    let ix = open(dir.path());
    for m in 1..=4u64 { ix.index_message(m, &body(40, m)).unwrap(); }
    ix.compact(1..=4).unwrap();
    ix.remove_message(3).unwrap();
    ix.compact([1, 2, 4]).unwrap();
    assert_eq!(ix.chunk_count(), 3);
    assert_eq!(top(&ix, &body(40, 4)), Some((4, 0)));
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1, "only the new generation's base is left on disk");
}

struct Wrong(u32);
impl Embedder for Wrong {
    fn dimension(&self) -> u32 { self.0 }
    fn embed(&self, t: Vec<String>) -> Result<Vec<Vec<f32>>, CoreError> { Ok(t.iter().map(|_| vec![f32::NAN; EMBED_DIM]).collect()) }
    fn embed_titled(&self, _title: Option<String>, texts: Vec<String>) -> Result<Vec<Vec<f32>>, CoreError> { self.embed(texts) } // mechanical (contracts delta 12)
    fn embed_query(&self, _t: String) -> Result<Vec<f32>, CoreError> { Ok(vec![f32::NAN; EMBED_DIM]) } // mechanical (contracts delta 8)
    fn embed_media(&self, _k: contracts::MediaKind, _p: String) -> Result<Vec<f32>, CoreError> { Ok(vec![f32::NAN; EMBED_DIM]) } // mechanical (contracts delta 9)
}

#[test]
fn an_embedder_that_does_not_fit_is_refused_in_words() {
    let dir = tempfile::tempdir().unwrap();
    assert!(matches!(VectorIndex::open(dir.path(), Arc::new(Wrong(EMBED_DIM as u32 + 1))), Err(CoreError::Invalid { .. })));
    let ix = VectorIndex::open(dir.path(), Arc::new(Wrong(EMBED_DIM as u32))).unwrap();
    assert!(matches!(ix.index_message(1, "some words"), Err(CoreError::Invalid { .. })), "NaN vectors never enter the index");
    assert!(matches!(ix.index_message(1 << 48, "x"), Err(CoreError::Invalid { .. })));
}
