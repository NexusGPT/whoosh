//! The tokeniser: words, prefixes, links, scripts without spaces, distinct tokens, the per-message cap.
mod support;

use std::collections::HashSet;

use junk::tuning::MAX_TOKENS_PER_MESSAGE;
use junk::{tokenise, words};
use support::msg;

fn has(tokens: &[String], wanted: &str) -> bool {
    tokens.iter().any(|t| t.as_str() == wanted)
}

#[test]
fn words_are_lower_case_and_length_bounded() {
    let w = words("It is a BIG Day, don't stop $100 now");
    for want in ["big", "day", "don't", "stop", "$100", "now"] {
        assert!(has(&w, want), "{want} missing from {w:?}");
    }
    for dropped in ["it", "is", "a"] {
        assert!(!has(&w, dropped), "{dropped} is under three characters and must be dropped: {w:?}");
    }
}

#[test]
fn long_words_become_skip_tokens() {
    let w = words("extraordinarily supercalifragilistic international confirmation");
    assert!(has(&w, "skip:e:10"), "{w:?}"); // 15 letters
    assert!(has(&w, "skip:s:20"), "{w:?}"); // 20 letters
    assert!(has(&w, "skip:i:10"), "{w:?}"); // 13 letters
    assert!(has(&w, "confirmation"), "twelve letters is still a word: {w:?}");
    assert!(!has(&w, "extraordinarily"), "{w:?}");
}

#[test]
fn subject_sender_and_name_tokens_are_prefixed() {
    let m = msg(Some("Acme Support"), "Deals@Promo-Shop.EXAMPLE", "FREE Cash NOW!!!", "hello there friend");
    let t = tokenise(&m);
    for want in ["subject:free", "subject:cash", "subject:now", "from:promo-shop.example", "fromname:acme", "fromname:support", "hello", "there", "friend"] {
        assert!(has(&t, want), "{want} missing from {t:?}");
    }
    assert!(!has(&t, "free"), "a subject word is not a body word: {t:?}");
}

#[test]
fn links_and_urls_give_host_tokens() {
    let mut m = msg(None, "a@b.example", "", "visit http://cheap.example/p now");
    m.html = Some("<p><a href=\"http://tracking.spam-shop.example/x?id=1&amp;u=2\">hi</a></p><abbr title=\"x\">no</abbr>".to_string());
    let t = tokenise(&m);
    assert!(has(&t, "url:tracking.spam-shop.example"), "{t:?}");
    assert!(has(&t, "url:cheap.example"), "{t:?}");
}

#[test]
fn a_script_without_spaces_is_cut_into_pairs() {
    let t = tokenise(&msg(None, "a@b.example", "", "迷惑メール"));
    for want in ["迷惑", "惑メ", "メー", "ール"] {
        assert!(has(&t, want), "{want} missing from {t:?}");
    }
}

#[test]
fn a_token_counts_once_per_message() {
    let t = tokenise(&msg(None, "a@b.example", "", "free free free FREE free"));
    assert_eq!(t.iter().filter(|x| x.as_str() == "free").count(), 1, "{t:?}");
}

#[test]
fn a_message_gives_at_most_the_capped_number_of_distinct_tokens() {
    let text: String = (0..5000).map(|i| format!("w{i}")).collect::<Vec<String>>().join(" ");
    let t = tokenise(&msg(None, "a@b.example", "", &text));
    assert_eq!(t.len(), MAX_TOKENS_PER_MESSAGE, "the message holds thousands of distinct words");
    let distinct: HashSet<&String> = t.iter().collect();
    assert_eq!(distinct.len(), t.len(), "tokens are distinct");
    assert!(!has(&t, "w4999"), "the end of a long body is never read");
}

#[test]
fn an_empty_message_has_no_tokens() {
    assert!(tokenise(&msg(None, "", "", "")).is_empty());
}
