//! A search embeds the typed question with the QUERY form. The fake model has two forms like EmbeddingGemma 2:
//! a document is placed by the words it contains, a question is placed by what it asks about. The question's words
//! are not in the document, so only the query form finds it; the document form of a question is noise.
use contracts::{CoreError, Embedder};
use search_vector::constants::EMBED_DIM;
use search_vector::VectorIndex;
use std::sync::Arc;

const DOCS: [&str; 3] = [
    "invoice 4411 payment due friday bank transfer",
    "lisbon offsite agenda flights hotel booking",
    "warehouse delivery pallets shipped carrier",
];
const QUESTIONS: [(&str, usize); 3] = [
    ("what do I owe the supplier?", 0),
    ("where is the team going in the spring?", 1),
    ("did my parcel leave yet?", 2),
];

fn noise(text: &str) -> Vec<f32> {
    let seed = text.bytes().fold(0xcbf29ce484222325u64, |h, b| (h ^ b as u64).wrapping_mul(0x100000001b3));
    (0..EMBED_DIM as u64).map(|i| { let x = (seed ^ (i + 1)).wrapping_mul(0x9e3779b97f4a7c15); ((x >> 40) as f32 / (1u64 << 24) as f32) - 0.5 }).collect()
}
fn with_topic(text: &str, topic: Option<usize>) -> Vec<f32> {
    let mut v: Vec<f32> = noise(text).iter().map(|x| x * 0.02).collect();
    if let Some(t) = topic { v[t] += 1.0; }
    v
}

struct TwoForms;
impl Embedder for TwoForms {
    fn dimension(&self) -> u32 { EMBED_DIM as u32 }
    fn embed(&self, texts: Vec<String>) -> Result<Vec<Vec<f32>>, CoreError> {
        Ok(texts.iter().map(|t| with_topic(t, DOCS.iter().position(|d| d == t).or_else(|| ["invoice", "lisbon", "warehouse"].iter().position(|w| t.contains(w))))).collect())
    }
    fn embed_titled(&self, _title: Option<String>, texts: Vec<String>) -> Result<Vec<Vec<f32>>, CoreError> { self.embed(texts) } // mechanical (contracts delta 12)
    fn embed_query(&self, text: String) -> Result<Vec<f32>, CoreError> {
        Ok(with_topic(&text, QUESTIONS.iter().find(|(q, _)| *q == text).map(|(_, t)| *t)))
    }    fn embed_media(&self, _k: contracts::MediaKind, _p: String) -> Result<Vec<f32>, CoreError> { Ok(vec![f32::NAN; EMBED_DIM as usize]) } // mechanical (contracts delta 10; main was red here)
}

fn index() -> (tempfile::TempDir, VectorIndex) {
    let dir = tempfile::tempdir().unwrap();
    let ix = VectorIndex::open(dir.path(), Arc::new(TwoForms)).unwrap();
    for (n, d) in DOCS.iter().enumerate() { ix.index_message(n as u64 + 1, d).unwrap(); }
    (dir, ix)
}

#[test]
fn a_question_ranks_the_matching_passage_first() {
    let (_d, ix) = index();
    for (q, want) in QUESTIONS {
        let hits = ix.search(q, 3).unwrap();
        assert_eq!(hits[0].message_key, want as u64 + 1, "{q}: {hits:?}");
        assert!(hits[0].score > hits[1].score + 0.3, "{q}: a clear margin, {hits:?}");
    }
}

#[test]
fn the_document_form_of_the_same_questions_would_not() {
    let (_d, ix) = index();
    let right = QUESTIONS.iter().filter(|(q, want)| {
        let v = TwoForms.embed(vec![q.to_string()]).unwrap().remove(0);
        let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        let v: Vec<f32> = v.iter().map(|x| x / norm).collect();
        ix.search_vector(&v, 3).unwrap()[0].message_key == *want as u64 + 1
    }).count();
    assert!(right < QUESTIONS.len(), "the document form got {right} of 3 right; the fixture would not tell the forms apart");
}
