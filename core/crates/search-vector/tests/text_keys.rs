//! contracts delta 10 hook: vectors filed under text keys (attachment parts), removed by prefix, found with their key.
use contracts::{CoreError, Embedder, MediaKind};
use search_vector::{constants::EMBED_DIM, VectorIndex};
use std::sync::Arc;

struct Axis;
fn axis(i: usize) -> Vec<f32> { let mut v = vec![0.0; EMBED_DIM]; v[i] = 1.0; v }
impl Embedder for Axis {
    fn dimension(&self) -> u32 { EMBED_DIM as u32 }
    fn embed(&self, texts: Vec<String>) -> Result<Vec<Vec<f32>>, CoreError> { Ok(texts.iter().map(|t| axis(if t.contains("invoice") { 1 } else { 2 })).collect()) }
    fn embed_titled(&self, _title: Option<String>, texts: Vec<String>) -> Result<Vec<Vec<f32>>, CoreError> { self.embed(texts) } // mechanical (contracts delta 12)
    fn embed_query(&self, text: String) -> Result<Vec<f32>, CoreError> { Ok(axis(if text.contains("forklift") { 3 } else if text.contains("invoice") { 1 } else { 4 })) }
    fn embed_media(&self, _k: MediaKind, _p: String) -> Result<Vec<f32>, CoreError> { Ok(axis(3)) }
}

fn open(dir: &std::path::Path) -> VectorIndex { VectorIndex::open(dir, Arc::new(Axis)).unwrap() }

#[test]
fn a_text_keyed_vector_is_found_with_its_key_and_mail_is_not_returned_as_one() {
    let dir = tempfile::tempdir().unwrap();
    let ix = open(dir.path());
    ix.index_message(7, "the invoice").unwrap();
    ix.add_vector("msg-s02/att-1#i", &axis(3)).unwrap();
    let hits = ix.search_keys("forklift", 5).unwrap();
    assert_eq!(hits.first().map(|h| h.key.as_str()), Some("msg-s02/att-1#i"));
    assert!(hits[0].score > 0.99);
    assert!(ix.search_keys("invoice", 5).unwrap().iter().all(|h| h.key.starts_with("msg-s02")), "mail never comes back as a key");
    let mail = ix.search("invoice", 1).unwrap();
    assert_eq!(mail[0].message_key, 7);
    assert_eq!(ix.key_of(&mail[0]), None);
}

#[test]
fn remove_prefix_takes_every_part_of_an_attachment_and_nothing_else() {
    let dir = tempfile::tempdir().unwrap();
    let ix = open(dir.path());
    ix.add_vector("m1/a1#f0", &axis(3)).unwrap();
    ix.add_vector("m1/a1#f1", &axis(3)).unwrap();
    ix.add_vector("m1/a2#i", &axis(3)).unwrap();
    let before = ix.chunk_count();
    assert_eq!(ix.remove_prefix("m1/a1").unwrap(), 2);
    assert_eq!(ix.chunk_count(), before - 2, "the two vectors leave the index, not only the key table");
    let keys: Vec<String> = ix.search_keys("forklift", 10).unwrap().into_iter().map(|h| h.key).collect();
    assert_eq!(keys, vec!["m1/a2#i".to_string()]);
}

#[test]
fn re_adding_a_key_replaces_its_vector_and_keys_survive_a_relaunch_and_a_compaction() {
    let dir = tempfile::tempdir().unwrap();
    {
        let ix = open(dir.path());
        ix.add_vector("m1/a1#i", &axis(4)).unwrap();
        ix.add_vector("m1/a1#i", &axis(3)).unwrap();
        assert_eq!(ix.search_keys("forklift", 10).unwrap().len(), 1, "one key, one vector");
        ix.commit().unwrap();
    }
    let ix = open(dir.path());
    assert_eq!(ix.search_keys("forklift", 1).unwrap()[0].key, "m1/a1#i");
    ix.compact(std::iter::empty()).unwrap();
    assert_eq!(ix.search_keys("forklift", 1).unwrap()[0].key, "m1/a1#i", "compaction keeps text-keyed vectors");
}
