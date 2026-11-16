//! match-inside (SPEC A149): `search_ranked` matches a typed word of SUBSTRING_MIN_CHARS letters or more wherever its letters sit,
//! also inside a longer word or an address ("consultant" finds brightconsultants.example), in the subject, the sender's name and
//! address, and the text; a shorter word matches whole words only; the order is the word itself, then a word that starts with
//! it, then a word that holds it, the subject or sender before the text; each hit says where, with the letters to show, and a
//! text match's snippet is the passage around the first occurrence. Each test names its broken copy: the one-line change to
//! core/crates/search-text/src/query.rs that turns it red. No test prints a mail's text or a query.
mod common;
use common::*;
use contracts::{MatchedIn, SearchHit, TextRange};
use search_text::TextIndex;
use std::time::{Duration, Instant};

/// The owner's search asks this many rows (the engine passes SEARCH_RANKED_MAX).
fn rows() -> usize {
    constant("SEARCH_RANKED_MAX") as usize
}

fn hit<'a>(hits: &'a [SearchHit], id: &str) -> &'a SearchHit {
    hits.iter().find(|h| h.message_id == id).unwrap_or_else(|| panic!("{id} is a match: {:?}", ids(hits)))
}

fn sender(id: &str, subject: &str, name: &str, address: &str, at: i64) -> contracts::Message {
    let mut m = msg(id, subject, name, "owner@example.test", at);
    m.from_address = address.into();
    m
}

/// The owner's mail of 2026-11-11 in counts: one mail holds a word that starts with "consultant", two hold it only inside an
/// address-like word of 17 and 20 letters, three hold "consultancy" or "consulting" (not the letters "consultant").
/// Broken copy (red): in `forms_of`, `while scanned < SUBSTRING_SCAN_MAX && all.advance() {` -> `while false && all.advance() {`
/// (no word holding the letters inside is read: the two addresses are lost).
#[test]
fn a_consultant_finds_the_three_mails_that_hold_its_letters_and_only_those() {
    let dir = TempDir::new("inside-owner");
    let store = open_store(&dir);
    insert(&store, vec![
        new_msg(msg("plural", "Meet our consultants", "Ops", "owner@example.test", 1_000), Some("Hello."), None),
        new_msg(msg("addr17", "Your quote from brightconsultants.example", "Ops", "owner@example.test", 2_000), Some("Hello."), None),
        new_msg(msg("addr20", "Reply from brightconsultantsusa.example", "Ops", "owner@example.test", 3_000), Some("Hello."), None),
        new_msg(msg("consultancy", "Consultancy rates for next year", "Ops", "owner@example.test", 4_000), Some("Hello."), None),
        new_msg(msg("consulting", "Consulting services offer", "Ops", "owner@example.test", 5_000), Some("Our consulting team."), None),
        new_msg(msg("digest", "Daily digest", "Ops", "owner@example.test", 6_000), Some("Sources and journalists."), None),
    ]);
    let index = TextIndex::open(&store, &dir.index()).unwrap();
    let hits = index.search_ranked("consultant", rows()).unwrap();
    let got: std::collections::HashSet<String> = ids(&hits).into_iter().collect();
    assert_eq!(got, ["plural", "addr17", "addr20"].iter().map(|s| s.to_string()).collect(), "the three mails holding the letters");
    // the letters alone in bold inside the address: "Your quote from " is 16 bytes, "bright" 6
    assert_eq!(hit(&hits, "addr17").matched_in, MatchedIn::SubjectWords { ranges: vec![TextRange { start: 22, end: 32 }] });
    assert!(hits.iter().all(|h| h.matched_in != MatchedIn::MeaningOnly), "a word match is never meaning");
}

/// "consultancy" and "consulting" do not hold the letters "consultant": no match.
/// Broken copy (red): in `forms_of`, `let w = word.as_bytes();` -> `let w = &word.as_bytes()[..word.len().min(7)];` (the first
/// seven letters, "consult", stand for the word).
#[test]
fn b_consultancy_is_not_a_match_for_consultant() {
    let dir = TempDir::new("inside-consultancy");
    let store = open_store(&dir);
    insert(&store, vec![
        new_msg(msg("consultancy", "Consultancy rates", "Ops", "owner@example.test", 1_000), Some("The consultancy fee."), None),
        new_msg(msg("consulting", "Consulting", "Ops", "owner@example.test", 2_000), Some("Consulting work."), None),
    ]);
    let index = TextIndex::open(&store, &dir.index()).unwrap();
    assert!(index.search_ranked("consultant", rows()).unwrap().is_empty(), "neither mail holds the letters");
}

/// The order: the word itself, a word that starts with it, a word that holds it; in each, the subject or sender before the text.
/// Broken copy (red): in `docs_of`, `let place = if field == fields.subject || field == fields.from { HEAD } else { TEXT };` ->
/// `let place = TEXT;` (the subject no longer comes before the text).
#[test]
fn c_the_order_is_whole_then_start_then_inside_and_the_subject_before_the_text() {
    let dir = TempDir::new("inside-order");
    let store = open_store(&dir);
    insert(&store, vec![
        new_msg(msg("inside-text", "Hello", "Ops", "owner@example.test", 6_000), Some("Write to brightconsultantsusa.example today."), None),
        new_msg(msg("inside-head", "Quote from brightconsultants.example", "Ops", "owner@example.test", 5_000), Some("Hello."), None),
        new_msg(msg("start-text", "Hello", "Ops", "owner@example.test", 4_000), Some("The consultants arrive."), None),
        new_msg(msg("start-head", "Consultants visit", "Ops", "owner@example.test", 3_000), Some("Hello."), None),
        new_msg(msg("whole-text", "Hello", "Ops", "owner@example.test", 2_000), Some("Our consultant arrives."), None),
        new_msg(msg("whole-head", "Consultant visit", "Ops", "owner@example.test", 1_000), Some("Hello."), None),
    ]);
    let index = TextIndex::open(&store, &dir.index()).unwrap();
    let hits = index.search_ranked("consultant", rows()).unwrap();
    let want = ["whole-head", "whole-text", "start-head", "start-text", "inside-head", "inside-text"];
    assert_eq!(ids(&hits), want.iter().map(|s| s.to_string()).collect::<Vec<_>>(), "the order of SPEC A149");
    assert!(hits.windows(2).all(|p| p[0].score > p[1].score), "the score says the rank: {:?}", hits.iter().map(|h| h.score).collect::<Vec<_>>());
}

/// The rank of a word that starts with the typed word is under the word itself and over a word that holds it.
/// Broken copy (red): in `run_ranked`, `let score = f64::from(6 - (i32::from(tier) * 2 + i32::from(place)));` ->
/// `let score = f64::from(6 - i32::from(place));` (the tiers are no longer told apart).
#[test]
fn d_the_three_tiers_have_three_scores_in_the_subject() {
    let dir = TempDir::new("inside-tiers");
    let store = open_store(&dir);
    insert(&store, vec![
        new_msg(msg("whole", "Consultant visit", "Ops", "owner@example.test", 1_000), Some("Hello."), None),
        new_msg(msg("start", "Consultants visit", "Ops", "owner@example.test", 2_000), Some("Hello."), None),
        new_msg(msg("inside", "Quote from brightconsultants.example", "Ops", "owner@example.test", 3_000), Some("Hello."), None),
    ]);
    let index = TextIndex::open(&store, &dir.index()).unwrap();
    let hits = index.search_ranked("consultant", rows()).unwrap();
    let (w, s, i) = (hit(&hits, "whole").score, hit(&hits, "start").score, hit(&hits, "inside").score);
    assert!(w > s && s > i, "whole {w} > start {s} > inside {i}");
}

/// The sender's display name and the sender's address are searched, and the hit says which, with the letters.
/// Broken copy (red): in `evidence`, `if !name.is_empty() || !address.is_empty() {` -> `if false {` (a sender match is said as
/// a text match).
#[test]
fn e_the_sender_name_and_address_match_and_say_so() {
    let dir = TempDir::new("inside-sender");
    let store = open_store(&dir);
    insert(&store, vec![
        new_msg(sender("by-name", "Hello", "Consultant Desk", "desk@ext.test", 1_000), Some("Hello."), None),
        new_msg(sender("by-address", "Hello", "News", "news@brightconsultants.example", 2_000), Some("Hello."), None),
        new_msg(sender("neither", "Hello", "News", "news@ext.test", 3_000), Some("Hello."), None),
    ]);
    let index = TextIndex::open(&store, &dir.index()).unwrap();
    let hits = index.search_ranked("consultant", rows()).unwrap();
    assert_eq!(ids(&hits).len(), 2, "the two senders that hold the letters: {:?}", ids(&hits));
    assert_eq!(hit(&hits, "by-name").matched_in, MatchedIn::SenderWords { name_ranges: vec![TextRange { start: 0, end: 10 }], address_ranges: vec![] });
    // "news@" is 5 bytes, "bright" 6
    assert_eq!(hit(&hits, "by-address").matched_in, MatchedIn::SenderWords { name_ranges: vec![], address_ranges: vec![TextRange { start: 11, end: 21 }] });
}

/// Case and accents are folded, in the mail and in the typed word; the bold range covers the stored letters, accents included.
/// Broken copy (red): in `bold_range`, `pieces.push((concat.len(), from + i));` -> `pieces.push((concat.len(), from + concat.len()));`
/// (the range is counted in folded letters, off by one byte after an accented letter).
#[test]
fn f_case_and_accents_are_folded_both_ways() {
    let dir = TempDir::new("inside-accents");
    let store = open_store(&dir);
    insert(&store, vec![
        new_msg(msg("upper", "Réunion CONSULTÄNTS demain", "Ops", "owner@example.test", 1_000), Some("Hello."), None),
        new_msg(msg("inside", "Équipe brightconsultänts", "Ops", "owner@example.test", 2_000), Some("Hello."), None),
    ]);
    let index = TextIndex::open(&store, &dir.index()).unwrap();
    for typed in ["consultant", "CONSULTÄNT"] {
        let hits = index.search_ranked(typed, rows()).unwrap();
        assert_eq!(ids(&hits).len(), 2, "both mails, typed form {}: {:?}", if typed.is_ascii() { "plain" } else { "accented" }, ids(&hits));
        // "Réunion " is 9 bytes; "CONSULTÄNTS" 12 (Ä is 2): the whole word, it starts with the typed word
        assert_eq!(hit(&hits, "upper").matched_in, MatchedIn::SubjectWords { ranges: vec![TextRange { start: 9, end: 21 }] });
        // "Équipe " is 8 bytes, "bright" 6; "consultänt" 11 (ä is 2): the letters alone
        assert_eq!(hit(&hits, "inside").matched_in, MatchedIn::SubjectWords { ranges: vec![TextRange { start: 14, end: 25 }] });
    }
}

/// A typed word of two letters matches whole words only, never letters inside a word.
/// Broken copy (red): in `is_wide`, `>= crate::limits::SUBSTRING_MIN_CHARS` -> `>= 2`.
#[test]
fn g_a_two_letter_word_matches_whole_words_only() {
    let dir = TempDir::new("inside-short");
    let store = open_store(&dir);
    insert(&store, vec![
        new_msg(msg("whole", "Tools", "Ops", "owner@example.test", 1_000), Some("New ai tools for you."), None),
        new_msg(msg("inside", "Quote from brightconsultants.example", "Ops", "owner@example.test", 2_000), Some("Hello."), None),
        new_msg(msg("start", "Aims for the year", "Ops", "owner@example.test", 3_000), Some("Hello."), None),
    ]);
    let index = TextIndex::open(&store, &dir.index()).unwrap();
    assert_eq!(ids(&index.search_ranked("ai", rows()).unwrap()), vec!["whole".to_string()], "the word itself only");
    assert_eq!(ids(&index.search_ranked("AI", rows()).unwrap()), vec!["whole".to_string()], "case folded");
}

/// A text match shows the passage of the text around the first occurrence: about SEARCH_PASSAGE_CHARS characters, cut at word
/// boundaries, line breaks as spaces, with the letters' range in the passage; never the mail's first line.
/// Broken copy (red): in `passage`, `let first = occurrences(index, text, typed, 1).into_iter().next();` ->
/// `let first: Option<(usize, usize)> = None;` (the start of the text).
#[test]
fn h_a_text_match_shows_the_passage_around_the_first_occurrence() {
    let dir = TempDir::new("inside-passage");
    let store = open_store(&dir);
    let opening = "Opening words of this mail that the row must not show. ".repeat(5);
    let closing = " The rest of the mail goes on about other things for a while.".repeat(5);
    let body = format!("{opening}Last week we met the brightconsultants\nteam on Monday to talk.{closing}");
    insert(&store, vec![new_msg(msg("long", "Hello", "Ops", "owner@example.test", 1_000), Some(body.as_str()), None)]);
    let index = TextIndex::open(&store, &dir.index()).unwrap();
    let hits = index.search_ranked("consultant", rows()).unwrap();
    let h = hit(&hits, "long");
    let MatchedIn::TextWords { ranges } = &h.matched_in else { panic!("a text match says TextWords: {:?}", h.matched_in) };
    assert_eq!(ranges.len(), 1, "the first occurrence");
    let r = &ranges[0];
    assert_eq!(&h.snippet[r.start as usize..r.end as usize], "consultant", "the range is the letters, in the snippet");
    assert!(!h.snippet.starts_with("Opening"), "never the mail's first line");
    assert!(!h.snippet.contains('\n'), "line breaks are spaces");
    let limit = search_text::limits::SEARCH_PASSAGE_CHARS;
    assert!(h.snippet.chars().count() <= limit, "about {limit} characters: {}", h.snippet.chars().count());
    assert!(h.snippet.chars().count() >= limit / 2, "a passage, not a word: {}", h.snippet.chars().count());
    let flat = body.split_whitespace().collect::<Vec<_>>().join(" ");
    let at = flat.find(&h.snippet).expect("the passage is a run of the text");
    assert!(at > 0 && flat.as_bytes()[at - 1] == b' ', "it starts at a word");
    let end = at + h.snippet.len();
    assert!(end == flat.len() || flat.as_bytes()[end] == b' ', "it ends at a word");
}

// ---- speed ----

/// A deterministic mailbox of `n` mails with realistic word counts: subjects of 4 to 9 words, bodies of 80 to 400 words, drawn
/// with a skew (a few words common, most rare) from 6000 made-up words, some holding the letters "con"; one mail in 20 holds a
/// form of "consultant".
pub fn mailbox(n: usize) -> Vec<store::NewMessage> {
    const SYLLABLES: [&str; 24] = ["con", "sul", "tant", "ma", "ri", "no", "ve", "lo", "pa", "ter", "in", "for", "ion", "de", "ble", "ex", "pro", "ment", "al", "ly", "sta", "gu", "ken", "bor"];
    let mut seed: u64 = 0x5eed_1234_abcd_0001;
    let mut next = move || {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (seed >> 33) as usize
    };
    let vocabulary: Vec<String> = (0..6000).map(|_| {
        let parts = 2 + next() % 3;
        (0..parts).map(|_| SYLLABLES[next() % SYLLABLES.len()]).collect::<String>()
    }).collect();
    let word = |next: &mut dyn FnMut() -> usize| -> String {
        let r = (next() % 10_000) as f64 / 10_000.0;
        vocabulary[((r * r * r) * vocabulary.len() as f64) as usize].clone()
    };
    let forms = ["consultant", "consultants", "brightconsultants.example", "consultantsusa"];
    (0..n).map(|i| {
        let subject_words = 4 + next() % 6;
        let mut subject: Vec<String> = (0..subject_words).map(|_| word(&mut next)).collect();
        let body_words = 80 + next() % 321;
        let mut body: Vec<String> = (0..body_words).map(|_| word(&mut next)).collect();
        if i % 20 == 0 {
            let f = forms[(i / 20) % forms.len()].to_string();
            if i % 40 == 0 { subject.push(f) } else { body.insert(body.len() / 2, f) }
        }
        let name = format!("{} {}", word(&mut next), word(&mut next));
        let address = format!("{}@{}.com", word(&mut next), word(&mut next));
        let mut m = msg(&format!("m{i}"), &subject.join(" "), &name, "owner@example.test", 1_700_000_000_000 + i as i64 * 60_000);
        m.from_address = address;
        let text = body.join(" ");
        new_msg(m, Some(text.as_str()), None)
    }).collect()
}

/// The word part of the first list stays inside the owner's 50 ms ("The objective is <50ms for any operation") on 2000 mails, for a
/// 3-letter and a 10-letter word. Prints each time (never the word).
/// Broken copy (red): in `run_ranked`, after `let searcher = reader.searcher();` add
/// `std::thread::sleep(std::time::Duration::from_millis(60));`.
#[test]
fn i_a_three_and_a_ten_letter_search_take_under_50_ms_on_2000_mails() {
    const BUDGET: Duration = Duration::from_millis(50);
    let dir = TempDir::new("inside-speed");
    let store = open_store(&dir);
    insert(&store, mailbox(2000));
    let index = TextIndex::open(&store, &dir.index()).unwrap();
    assert_eq!(index.documents(), 2000, "every mail indexed");
    for (label, typed) in [("3-letter word", "con"), ("10-letter word", "consultant")] {
        let t = Instant::now();
        let hits = index.search_ranked(typed, rows()).unwrap();
        let took = t.elapsed();
        println!("search_ranked, {label}: {} hits in {:.2} ms on 2000 mails", hits.len(), took.as_secs_f64() * 1000.0);
        assert!(!hits.is_empty(), "{label}: the mailbox holds it");
        assert!(took < BUDGET, "{label}: {took:?} is over {BUDGET:?}");
    }
}
