use search_vector::constants::{CHUNK_OVERLAP as O, CHUNK_TOKENS as T};
use search_vector::{chunk_text, expected_chunk_count, WhitespaceTokenizer};

/// Counts windows by walking the text with the constants, not by the crate's formula.
fn walked_count(n: usize) -> usize {
    let (mut count, mut start) = (0, 0);
    while start < n {
        count += 1;
        if start + T >= n { break; }
        start += T - O;
    }
    count
}

fn sizes() -> Vec<usize> {
    vec![0, 1, T - 1, T, T + 1, T + (T - O), T + (T - O) + 1, 2 * T, 3 * T + 5, 10 * T]
}

#[test]
fn chunk_counts_match_the_constants() {
    for n in sizes() {
        let chunks = chunk_text(&body(n, 1), &WhitespaceTokenizer, T, O).unwrap();
        assert_eq!(chunks.len(), walked_count(n), "{n} tokens");
        assert_eq!(chunks.len(), expected_chunk_count(n, T, O), "{n} tokens, formula");
    }
}

#[test]
fn neighbours_share_exactly_the_overlap_and_nothing_is_lost() {
    for n in sizes().into_iter().filter(|n| *n > T) {
        let chunks = chunk_text(&body(n, 2), &WhitespaceTokenizer, T, O).unwrap();
        assert_eq!(chunks[0].start_token, 0);
        assert_eq!(chunks.last().unwrap().end_token, n, "{n} tokens: the tail is covered");
        for pair in chunks.windows(2) {
            let (a, b) = (&pair[0], &pair[1]);
            assert!(a.end_token - a.start_token <= T);
            assert_eq!(a.end_token - b.start_token, O, "{n} tokens: overlap between chunk {} and {}", a.index, b.index);
            let tail: Vec<&str> = a.text.split(' ').rev().take(O).collect();
            let head: Vec<&str> = b.text.split(' ').take(O).collect::<Vec<_>>().into_iter().rev().collect();
            assert_eq!(tail, head, "{n} tokens: the shared words are the same words");
        }
    }
}

#[test]
fn a_short_body_is_one_chunk_and_an_empty_one_is_none() {
    assert_eq!(chunk_text("", &WhitespaceTokenizer, T, O).unwrap().len(), 0);
    assert_eq!(chunk_text("  \n ", &WhitespaceTokenizer, T, O).unwrap().len(), 0);
    let one = chunk_text("hello  there\nworld", &WhitespaceTokenizer, T, O).unwrap();
    assert_eq!((one.len(), one[0].text.as_str()), (1, "hello  there\nworld"), "the original spacing is kept");
}

#[test]
fn an_overlap_that_is_not_smaller_than_the_chunk_is_refused() {
    assert!(chunk_text("a b c", &WhitespaceTokenizer, 4, 4).is_err());
    assert!(chunk_text("a b c", &WhitespaceTokenizer, 0, 0).is_err());
}

/// A body of exactly `n` whitespace tokens, each unique per `salt`.
fn body(n: usize, salt: u64) -> String {
    (0..n).map(|i| format!("w{salt}x{i}")).collect::<Vec<_>>().join(" ")
}
