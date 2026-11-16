//! search-forms (SPEC A141): `search_ranked` widens a typed word of PREFIX_MIN_CHARS letters or more to the words that start with
//! it, keeps the conjunction and quoted phrases of `search`, and says where each hit matched. A new entry point: it has no red run
//! on 399d0d59 (the engine's tests in engine_search_forms.rs carry the red runs).
mod common;
use common::*;
use contracts::{MatchedIn, TextRange};
use search_text::TextIndex;

fn corpus(dir: &TempDir) -> std::sync::Arc<store::Store> {
    let store = open_store(dir);
    insert(
        &store,
        vec![
            new_msg(msg("plural-subject", "Consultants on site", "Ops", "owner@example.test", 1_000), Some("They arrive Monday."), None),
            new_msg(msg("exact-body", "Weekly notes", "Ops", "owner@example.test", 2_000), Some("Our consultant team met."), None),
            new_msg(msg("unrelated", "Lunch", "Ops", "owner@example.test", 3_000), Some("Salmon and rice."), None),
            new_msg(msg("with-file", "Files", "Ops", "owner@example.test", 4_000), Some("See the file."), None),
        ],
    );
    add_attachment(&store, "att-c", "with-file", "consultants-terms.pdf", Some("Terms of the engagement."));
    store
}

#[test]
fn a_typed_word_finds_the_words_that_start_with_it_and_says_where() {
    let dir = TempDir::new("prefix");
    let store = corpus(&dir);
    let index = TextIndex::open(&store, &dir.index()).unwrap();
    let hits = index.search_ranked("consultant", 10).unwrap();
    let got: std::collections::HashSet<String> = hits.iter().map(|h| h.message_id.clone()).collect();
    assert_eq!(got, ["plural-subject", "exact-body", "with-file"].iter().map(|s| s.to_string()).collect(), "{hits:?}");
    let plural = hits.iter().find(|h| h.message_id == "plural-subject").unwrap();
    assert_eq!(plural.matched_in, MatchedIn::SubjectWords { ranges: vec![TextRange { start: 0, end: 11 }] });
    let body = hits.iter().find(|h| h.message_id == "exact-body").unwrap();
    // SPEC A149 changes this expectation: "Every match SHOWS why: the letters in bold in the subject or the sender, or one line of
    // the text around them" (was `SubjectWords { ranges: vec![] }`, the contract then having no text value)
    assert_eq!(body.matched_in, MatchedIn::TextWords { ranges: vec![TextRange { start: 4, end: 14 }] }, "a body match is a word match shown in its passage, never meaning");
    assert_eq!(body.snippet, "Our consultant team met.", "the passage around the word");
    let file = hits.iter().find(|h| h.message_id == "with-file").unwrap();
    assert_eq!((file.matched_in.clone(), file.attachment_name.as_deref()), (MatchedIn::AttachmentName, Some("consultants-terms.pdf")));
    assert!(index.search("consultant", 10).unwrap().iter().all(|h| h.message_id != "plural-subject"), "the old search is unchanged: whole words only");
}

#[test]
fn a_short_word_matches_only_itself_and_every_word_must_still_appear() {
    let dir = TempDir::new("prefix-short");
    let store = corpus(&dir);
    let index = TextIndex::open(&store, &dir.index()).unwrap();
    assert!(index.search_ranked("co", 10).unwrap().is_empty(), "two letters are not widened");
    let both = index.search_ranked("consultant monday", 10).unwrap();
    assert_eq!(both.iter().map(|h| h.message_id.as_str()).collect::<Vec<_>>(), vec!["plural-subject"], "every typed word must appear");
    assert!(index.search_ranked("zebra", 10).unwrap().is_empty(), "a word in no message matches nothing");
    let phrase = index.search_ranked("\"consultant team\"", 10).unwrap();
    assert_eq!(phrase.iter().map(|h| h.message_id.as_str()).collect::<Vec<_>>(), vec!["exact-body"], "a quoted phrase stays exact");
}
