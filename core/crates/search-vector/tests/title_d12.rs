//! contracts delta 12: the message's subject reaches the embedder's title slot; no title or a blank one is "none".
mod common;
use common::vector_for;
use contracts::{CoreError, Embedder, MediaKind};
use search_vector::constants::EMBED_DIM;
use search_vector::VectorIndex;
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct Recording { titles: Mutex<Vec<Option<String>>> }
impl Embedder for Recording {
    fn dimension(&self) -> u32 { EMBED_DIM as u32 }
    fn embed(&self, texts: Vec<String>) -> Result<Vec<Vec<f32>>, CoreError> { self.embed_titled(None, texts) }
    fn embed_titled(&self, title: Option<String>, texts: Vec<String>) -> Result<Vec<Vec<f32>>, CoreError> {
        self.titles.lock().unwrap().push(title.clone());
        let t = title.unwrap_or_else(|| "none".into());
        Ok(texts.iter().map(|x| vector_for(&format!("title: {t} | text: {x}"), EMBED_DIM)).collect())
    }
    fn embed_query(&self, text: String) -> Result<Vec<f32>, CoreError> { Ok(vector_for(&text, EMBED_DIM)) }
    fn embed_media(&self, _k: MediaKind, path: String) -> Result<Vec<f32>, CoreError> { Ok(vector_for(&path, EMBED_DIM)) }
}

#[test]
fn the_subject_travels_in_the_title_slot_and_a_blank_one_is_none() {
    let e = Arc::new(Recording::default());
    let dir = tempfile::tempdir().unwrap();
    let ix = VectorIndex::open(dir.path(), e.clone()).unwrap();
    ix.index_message_titled(1, Some("Quote for the Q1 print run"), "twelve thousand units").unwrap();
    ix.index_message_titled(2, Some("   "), "no subject here").unwrap();
    ix.index_message(3, "old call site").unwrap();
    assert_eq!(*e.titles.lock().unwrap(), vec![Some("Quote for the Q1 print run".to_string()), None, None]);
}
