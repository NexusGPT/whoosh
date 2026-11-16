//! A fake Embedder: the vector is a pure function of the text, so equal text finds itself.
use contracts::{CoreError, Embedder};
use search_vector::constants::EMBED_DIM;

pub struct FakeEmbedder { pub dim: u32 }
impl FakeEmbedder { pub fn new() -> Self { Self { dim: EMBED_DIM as u32 } } }

fn mix(mut x: u64) -> u64 {
    x ^= x >> 30; x = x.wrapping_mul(0xbf58476d1ce4e5b9); x ^= x >> 27; x = x.wrapping_mul(0x94d049bb133111eb); x ^ (x >> 31)
}

pub fn vector_for(text: &str, dim: usize) -> Vec<f32> {
    let seed = text.bytes().fold(0xcbf29ce484222325u64, |h, b| (h ^ b as u64).wrapping_mul(0x100000001b3));
    (0..dim as u64).map(|i| (mix(seed ^ mix(i + 1)) >> 40) as f32 / (1u64 << 23) as f32 - 1.0).collect()
}

impl Embedder for FakeEmbedder {
    fn dimension(&self) -> u32 { self.dim }
    fn embed(&self, texts: Vec<String>) -> Result<Vec<Vec<f32>>, CoreError> {
        Ok(texts.iter().map(|t| vector_for(t, self.dim as usize)).collect())
    }
    fn embed_titled(&self, _title: Option<String>, texts: Vec<String>) -> Result<Vec<Vec<f32>>, CoreError> { self.embed(texts) } // mechanical (contracts delta 12)
    // mechanical (contracts delta 8): the fake has one prefix-free space, so a query embeds like a document
    fn embed_query(&self, text: String) -> Result<Vec<f32>, CoreError> { Ok(vector_for(&text, self.dim as usize)) }
    // mechanical (contracts delta 9): the fake embeds the file's path as its text
    fn embed_media(&self, _kind: contracts::MediaKind, path: String) -> Result<Vec<f32>, CoreError> { Ok(vector_for(&path, self.dim as usize)) }
}

/// A body of exactly `n` whitespace tokens, each unique per `salt`.
pub fn body(n: usize, salt: u64) -> String {
    (0..n).map(|i| format!("w{salt}x{i}")).collect::<Vec<_>>().join(" ")
}
