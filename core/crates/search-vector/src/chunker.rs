//! Splits a message body into overlapping token windows.
use contracts::CoreError;
use std::ops::Range;

/// Finds the token spans of a text. The real tokenizer lives with the embedder; the default
/// counts whitespace-separated words, so a caller with the model's tokenizer injects it.
pub trait Tokenizer: Send + Sync {
    /// Byte ranges of each token, in order, inside `text`.
    fn spans(&self, text: &str) -> Vec<Range<usize>>;
    /// pieces: tokens the model adds around every input (its begin and end marks); none for a word count.
    fn specials(&self) -> usize { 0 }
}

/// One token per whitespace-separated word.
pub struct WhitespaceTokenizer;

impl Tokenizer for WhitespaceTokenizer {
    fn spans(&self, text: &str) -> Vec<Range<usize>> {
        let mut spans = Vec::new();
        let mut start: Option<usize> = None;
        for (i, c) in text.char_indices() {
            match (c.is_whitespace(), start) {
                (true, Some(s)) => {
                    spans.push(s..i);
                    start = None;
                }
                (false, None) => start = Some(i),
                _ => {}
            }
        }
        if let Some(s) = start {
            spans.push(s..text.len());
        }
        spans
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chunk {
    pub index: usize,
    pub start_token: usize,
    pub end_token: usize,
    pub text: String,
}

/// How many chunks a body of `tokens` tokens yields: one window of `size`, then one per `size - overlap` more.
pub fn expected_chunk_count(tokens: usize, size: usize, overlap: usize) -> usize {
    if tokens == 0 {
        0
    } else if tokens <= size {
        1
    } else {
        1 + (tokens - size).div_ceil(size - overlap)
    }
}

pub fn chunk_text(text: &str, tokenizer: &dyn Tokenizer, size: usize, overlap: usize) -> Result<Vec<Chunk>, CoreError> {
    if size == 0 || overlap >= size {
        return Err(CoreError::Invalid { field: "chunking".into(), reason_words: "the overlap must be smaller than the chunk size".into() });
    }
    let spans = tokenizer.spans(text);
    let stride = size - overlap;
    let mut chunks = Vec::new();
    let mut start = 0;
    while start < spans.len() {
        let end = (start + size).min(spans.len());
        chunks.push(Chunk { index: chunks.len(), start_token: start, end_token: end, text: text[spans[start].start..spans[end - 1].end].to_string() });
        if end == spans.len() {
            break;
        }
        start += stride;
    }
    Ok(chunks)
}
