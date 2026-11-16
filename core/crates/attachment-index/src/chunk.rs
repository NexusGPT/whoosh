//! Text of an attachment cut into pieces for the meaning index by THE rule of the mail bodies (search-vector `pieces`, SPEC A151):
//! cleaned first (links to their site, codes and quoted mail out), cut at white space, sized in the model's tokens. This crate has no
//! model tokenizer: it counts with `ByteBound` (one token per byte, never fewer than the model makes), so each piece fits the model's
//! input under any title of at most PIECE_TITLE_TOKENS; the engine's indexer cuts a file's text again with the model's own tokenizer.
use search_vector::pieces::{clean_for_meaning, pieces_of, ByteBound};

pub fn chunks(text: &str) -> Vec<String> {
    pieces_of(&ByteBound, None, &clean_for_meaning(text))
}
