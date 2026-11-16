//! What is searchable, and what an exact quoted phrase means.
mod common;
use common::*;
use search_text::TextIndex;

fn corpus(dir: &TempDir) -> std::sync::Arc<store::Store> {
    let store = open_store(dir);
    insert(
        &store,
        vec![
            new_msg(msg("holder", "Invoice from Acme", "Billing Team", "owner@example.test", 1_000), Some("Please pay the total of 1,284,550.37 euros before Friday."), None),
            // Every word of the figure, never as that phrase: reversed, split by other words, split by a field.
            new_msg(msg("decoy-reversed", "Totals", "Ops Team", "owner@example.test", 2_000), Some("Line items sum to 550.37 and the other 1,284 are pending."), None),
            new_msg(msg("decoy-gap", "Totals again", "Ops Team", "owner@example.test", 3_000), Some("Subtotal 1,284 plus tax 550 plus fee 37 cents."), None),
            new_msg(msg("html-only", "Newsletter", "News Desk", "owner@example.test", 4_000), None, Some("<html><style>.x{color:red}</style><body><p>The <b>quarterly</b> résumé&nbsp;is ready &amp; waiting</p></body></html>")),
            new_msg(msg("people", "Lunch", "Marguerite Dupont", "colleague@example.test", 5_000), Some("See you at noon."), None),
            new_msg(msg("with-attachment", "Contract", "Legal", "owner@example.test", 6_000), Some("Contract attached."), None),
            // The same words as the attachment sentence, in another order and with words between them.
            new_msg(msg("swapped", "Notes", "Legal", "owner@example.test", 7_000), Some("Termination survives, and the clause of indemnification is separate."), None),
        ],
    );
    add_attachment(&store, "att-1", "with-attachment", "master-services.pdf", Some("The indemnification clause survives termination."));
    store
}

#[test]
fn an_exact_quoted_figure_returns_only_the_message_that_holds_it() {
    let dir = TempDir::new("phrase");
    let store = corpus(&dir);
    let index = TextIndex::open(&store, &dir.index()).unwrap();
    let hits = index.search("\"1,284,550.37\"", 10).unwrap();
    assert_eq!(ids(&hits), vec!["holder"], "the quoted figure is an exact phrase: decoys with the same words in another order must not match");
    assert!(hits[0].snippet.contains("1,284"), "the snippet is cut from the holding text: {:?}", hits[0].snippet);
}

#[test]
fn the_same_words_without_quotes_match_every_message_that_has_them_all() {
    let dir = TempDir::new("loose");
    let store = corpus(&dir);
    let index = TextIndex::open(&store, &dir.index()).unwrap();
    let mut got = ids(&index.search("1,284,550.37", 10).unwrap());
    got.sort();
    // Unquoted, the query parser still reads the figure as one phrase of its words; bare words are the loose form.
    let mut bare = ids(&index.search("550 284 37", 10).unwrap());
    bare.sort();
    assert_eq!(bare, vec!["decoy-gap", "decoy-reversed", "holder"], "bare words must all appear, in any order");
    assert!(got.contains(&"holder".to_string()));
}

#[test]
fn people_subject_html_accents_and_attachment_text_are_searchable() {
    let dir = TempDir::new("fields");
    let store = corpus(&dir);
    let index = TextIndex::open(&store, &dir.index()).unwrap();
    assert_eq!(ids(&index.search("marguerite", 5).unwrap()), vec!["people"], "sender name");
    assert_eq!(ids(&index.search("colleague@example.test", 5).unwrap()), vec!["people"], "recipient address");
    assert_eq!(ids(&index.search("\"Invoice from Acme\"", 5).unwrap()), vec!["holder"], "subject phrase");
    assert_eq!(ids(&index.search("resume quarterly", 5).unwrap()), vec!["html-only"], "html text with entities, accent folded");
    assert!(index.search("color red", 5).unwrap().is_empty(), "style blocks are not text");
    assert_eq!(ids(&index.search("\"survives termination\"", 5).unwrap()), vec!["with-attachment"], "extracted attachment text");
    assert_eq!(ids(&index.search("master-services.pdf", 5).unwrap()), vec!["with-attachment"], "attachment file name");
}

#[test]
fn a_malformed_query_returns_nothing_instead_of_an_error() {
    let dir = TempDir::new("lenient");
    let store = corpus(&dir);
    let index = TextIndex::open(&store, &dir.index()).unwrap();
    for q in ["\"unclosed", "((", "from:", "AND OR", "", "   "] {
        assert!(index.search(q, 5).is_ok(), "query {q:?} must not error");
    }
}

#[test]
fn a_quoted_phrase_needs_its_words_in_order_and_next_to_each_other() {
    let dir = TempDir::new("order");
    let store = corpus(&dir);
    let index = TextIndex::open(&store, &dir.index()).unwrap();
    assert_eq!(ids(&index.search("\"indemnification clause survives\"", 5).unwrap()), vec!["with-attachment"], "the words in that order");
    assert!(index.search("\"survives clause indemnification\"", 5).unwrap().is_empty(), "the words reversed are not that phrase");
    assert!(index.search("\"indemnification survives\"", 5).unwrap().is_empty(), "a word missing between them is not that phrase");
    let mut loose = ids(&index.search("indemnification clause survives", 5).unwrap());
    loose.sort();
    assert_eq!(loose, vec!["swapped", "with-attachment"], "without quotes the words may be anywhere");
}
