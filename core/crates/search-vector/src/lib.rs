//! Vector index over message chunks: usearch, i8 quantised, memory-mapped, fed through the Embedder.
mod chunker;
mod files;
mod index;
mod keys;
/// pieces (SPEC A151): the meaning model's text, cleaned and cut in the model's own tokens.
pub mod pieces;

pub mod constants {
    include!(concat!(env!("OUT_DIR"), "/constants.rs"));
}

pub use chunker::{chunk_text, expected_chunk_count, Chunk, Tokenizer, WhitespaceTokenizer};
pub use index::{BaseBuilder, Hit, KeyHit, VectorIndex, MAX_CHUNKS_PER_MESSAGE};
pub use keys::TEXT_KEY_BASE;
