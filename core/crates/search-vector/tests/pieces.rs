//! pieces (SPEC A151, A152): the meaning model's text is cleaned and cut in the model's tokens so the model reads ALL of it.
//! Each test names its one-line broken copy (the line in core/crates/search-vector/src/pieces.rs that, changed so, turns it red).
//! The counter is injected (`FiveTok`: 5 tokens a word, 9 for a word with a dot or "://", 2 special tokens), except the one test
//! that runs only when WHOOSH_EMBED_WEIGHTS names a folder with tokenizer.json (the model's own tokenizer). No mail text is printed:
//! the assertion messages carry counts and indexes only.
use std::ops::Range;

use search_vector::pieces::{
    attachment_pieces, body_pieces, clean_for_meaning, count_tokens, cut_pieces, document_form, drop_quoted, fits, is_code, is_query,
    is_reply_header, is_separator, mostly_codes, words_outside, words_outside_ranges, cut_ranges, ByteBound, ModelTokenizer, MODEL_INPUT_TOKENS, PIECE_OVERLAP_TOKENS, PIECE_TEXT_TOKENS,
    PIECE_TITLE_TOKENS,
};
use search_vector::Tokenizer;

/// 5 tokens per whitespace word, 9 for a word that looks like a link or a host (a dot or "://"), plus 2 special tokens per input.
struct FiveTok;
impl Tokenizer for FiveTok {
    fn spans(&self, text: &str) -> Vec<Range<usize>> {
        let mut out = Vec::new();
        let mut at: Option<usize> = None;
        let push = |w: Range<usize>, out: &mut Vec<Range<usize>>| {
            let n = if text[w.clone()].contains('.') || text[w.clone()].contains("://") { 9 } else { 5 };
            for _ in 0..n { out.push(w.clone()); }
        };
        for (i, c) in text.char_indices() {
            match (c.is_whitespace(), at) {
                (true, Some(s)) => { push(s..i, &mut out); at = None; }
                (false, None) => at = Some(i),
                _ => {}
            }
        }
        if let Some(s) = at { push(s..text.len(), &mut out); }
        out
    }
    fn specials(&self) -> usize { 2 }
}

/// `n` words of prose, unique per index.
fn prose(n: usize, salt: &str) -> String { (0..n).map(|i| format!("word{salt}{}", letters(i))).collect::<Vec<_>>().join(" ") }
fn letters(i: usize) -> String { i.to_string().chars().map(|c| (b'a' + c.to_digit(10).unwrap() as u8) as char).collect() }

/// Asserts the pieces are slices of `clean` that cover it from its first word to its last, each piece starting on a word start.
fn assert_covers(clean: &str, pieces: &[String]) {
    assert!(!pieces.is_empty(), "no piece");
    let mut covered_to = 0usize;
    let mut from = 0usize;
    for (i, p) in pieces.iter().enumerate() {
        let at = clean[from..].find(p.as_str()).map(|x| x + from).unwrap_or_else(|| panic!("piece {i} is not a slice of the cleaned text in order"));
        assert!(at <= covered_to, "piece {i} starts at byte {at}, after the end of the previous piece ({covered_to}): words in no piece");
        assert!(at == 0 || clean[..at].ends_with(char::is_whitespace), "piece {i} does not start on a word start");
        covered_to = covered_to.max(at + p.len());
        from = at + 1;
    }
    assert_eq!(covered_to, clean.len(), "the last words of the cleaned text are in no piece");
}

/// (1) A mail with links and a quoted reply: the links become their sites, the reply leaves, every word of the cleaned text is in at
/// least one piece, and every piece with its title is within the model's input by the counter.
/// Broken copy: in `cut_pieces`, `if end == units.len() { break; }` -> `if end + 1 >= units.len() { break; }` (the last word is lost).
#[test]
fn a_mail_with_links_and_a_quoted_reply_is_read_whole_within_the_limit() {
    let mut body = String::new();
    for k in 0..12 {
        body.push_str(&prose(37, &letters(k)));
        body.push_str(&format!(" see https://www.shop{}.example/track?id=8f3a9c2e7b1d4f60a9c2e7b1d4f60a9c&utm_source=mail now.\n", letters(k)));
    }
    body.push_str("\nOn Mon, 10 Nov 2026 at 09:00, Ann <ann@example.test> wrote:\n> the earlier mail\n> more of it\n");
    let clean = clean_for_meaning(&body);
    assert!(!clean.contains("://") && !clean.contains("utm_source") && !clean.contains("earlier mail") && !clean.contains("wrote:"));
    assert!(clean.contains("shopa.example"), "a link becomes its site");
    let p = body_pieces(&FiveTok, &body, "Order update");
    assert_eq!(p.title.as_deref(), Some("Order update"));
    assert!(p.texts.len() > 1, "a long mail is more than one piece: {}", p.texts.len());
    assert_covers(&clean, &p.texts);
    for (i, t) in p.texts.iter().enumerate() {
        let n = count_tokens(&FiveTok, &document_form(p.title.as_deref(), t));
        assert!(n <= MODEL_INPUT_TOKENS, "piece {i} is {n} tokens with its title, over {MODEL_INPUT_TOKENS}");
        assert!(count_tokens(&FiveTok, t) - FiveTok.specials() <= PIECE_TEXT_TOKENS, "piece {i} holds more text than a piece");
    }
}

/// (2) Neighbouring pieces share words, at most PIECE_OVERLAP_TOKENS tokens of them, and the next piece starts on a word.
/// Broken copy: in `cut_pieces`, `back + units[next - 1].1 <= overlap` -> `back + units[next - 1].1 <= 0` (no overlap).
#[test]
fn neighbouring_pieces_share_the_overlap() {
    let clean = prose(500, "o");
    let pieces = cut_pieces(&FiveTok, Some("t"), &clean, PIECE_TEXT_TOKENS, PIECE_OVERLAP_TOKENS);
    assert!(pieces.len() >= 3, "{} pieces", pieces.len());
    for (i, w) in pieces.windows(2).enumerate() {
        let a: Vec<&str> = w[0].split_whitespace().collect();
        let b: Vec<&str> = w[1].split_whitespace().collect();
        let shared = (1..=a.len().min(b.len())).rev().find(|k| a[a.len() - k..] == b[..*k]).unwrap_or(0);
        assert!(shared >= 1, "pieces {i} and {} share no word", i + 1);
        assert!(shared * 5 <= PIECE_OVERLAP_TOKENS, "pieces {i} and {} share {shared} words, over the overlap", i + 1);
    }
    assert_covers(&clean, &pieces);
}

/// (2b) Whatever the length of the mail, the last word is in the last piece: a piece that ends one word before the end of the text must
/// not be taken for the last one. (1) and (2) each cut one length of text; this one cuts every length from 1 to 300 words.
/// Broken copy (c1436f49 m0): in `cut_ranges`, `if end == units.len() { break; }` -> `if end + 1 >= units.len() { break; }`.
#[test]
fn every_length_of_mail_keeps_its_last_word_in_the_last_piece() {
    for n in 1..=300 {
        let clean = prose(n, "z");
        let pieces = cut_pieces(&FiveTok, Some("t"), &clean, PIECE_TEXT_TOKENS, PIECE_OVERLAP_TOKENS);
        assert_covers(&clean, &pieces);
        let last_word = clean.rsplit(' ').next().unwrap();
        assert!(pieces.last().unwrap().ends_with(last_word), "{n} words: the last piece ends with the last word {last_word}");
    }
}

/// (3) A long subject is cut to PIECE_TITLE_TOKENS tokens at a word end, and every piece still fits with it.
/// Broken copy: in `body_pieces`, `Some(cut_title(tok, subject, PIECE_TITLE_TOKENS))` -> `Some(subject.to_string())`.
#[test]
fn a_long_subject_is_cut_and_every_piece_still_fits() {
    let subject = prose(60, "s");
    let p = body_pieces(&FiveTok, &prose(300, "b"), &subject);
    let title = p.title.clone().expect("a title");
    assert!(FiveTok.spans(&title).len() <= PIECE_TITLE_TOKENS, "the title is {} tokens", FiveTok.spans(&title).len());
    assert!(subject.starts_with(&title) && subject[title.len()..].starts_with(' '), "cut at a word end");
    for (i, t) in p.texts.iter().enumerate() { assert!(fits(&FiveTok, Some(&title), t), "piece {i} does not fit with the cut title"); }
}

/// (4) A piece that is mostly digits gets no vector; a mail of numbers only is filed under its subject alone.
/// Broken copy: in `mostly_codes`, `letters * 2 < all || all == 0` -> `all == 0`.
#[test]
fn a_piece_of_digits_gets_no_vector() {
    let numbers = (0..400).map(|i| format!("{}", 1000 + i)).collect::<Vec<_>>().join(" ");
    let body = format!("{}\n{numbers}", prose(70, "d"));
    let p = body_pieces(&FiveTok, &body, "Statement");
    assert!(!p.texts.is_empty());
    assert!(p.texts.iter().all(|t| !mostly_codes(t)), "a piece of digits was kept");
    assert!(mostly_codes("4711 0815 2026 13.50 99"));
    assert!(!mostly_codes("Invoice 4711 for March"));
    let only = body_pieces(&FiveTok, &numbers, "Statement");
    assert_eq!(only.texts, vec!["Statement".to_string()], "a mail of numbers is found by its subject");
}

/// (5) A file's pieces carry "<subject> · <file name>"; with a long subject the subject is cut and the file name stays.
/// Broken copy: in `attachment_title`, `let whole = format!("{subject} · {name}");` -> `let whole = subject.to_string();`.
#[test]
fn a_file_piece_carries_the_subject_and_the_file_name() {
    let p = attachment_pieces(&FiveTok, "Heavy matte stock, two colours", "Quote for May", "devis.pdf");
    assert_eq!(p.title.as_deref(), Some("Quote for May · devis.pdf"));
    assert_eq!(p.texts, vec!["Heavy matte stock, two colours".to_string()]);
    let long = attachment_pieces(&FiveTok, &prose(200, "f"), &prose(40, "l"), "devis.pdf");
    let title = long.title.expect("a title");
    assert!(title.ends_with(" · devis.pdf"), "the file name stays at the end");
    assert!(FiveTok.spans(&title).len() <= PIECE_TITLE_TOKENS, "the title is {} tokens", FiveTok.spans(&title).len());
    for (i, t) in long.texts.iter().enumerate() { assert!(fits(&FiveTok, Some(&title), t), "file piece {i} does not fit"); }
}

/// (6a) A link becomes its site name (no "www.", no path, no query).
/// Broken copy: in `clean_word`, `let host = host_of(&word[i..]);` -> `let host = word[i..].to_string();`.
#[test]
fn clean_a_link_becomes_its_site() {
    assert_eq!(clean_for_meaning("read https://www.example.com/a/b?utm_source=x#top today"), "read example.com today");
    assert_eq!(clean_for_meaning("(http://shop.example.org:8080/x)"), "shop.example.org");
    assert_eq!(clean_for_meaning("<https://news.example.net/?a=1>"), "news.example.net");
    assert_eq!(clean_for_meaning("write to mailto:ann@example.test"), "write to ann@example.test");
}

/// (6b) Long tracking codes leave: hex runs, base64, long ids with digits; ordinary long words stay.
/// Broken copy: in `is_code`, `if w.chars().any(|c| c.is_ascii_digit()) { return true; }` -> `if false { return true; }`.
#[test]
fn clean_tracking_codes_leave() {
    assert!(is_code("8f3a9c2e7b1d4f60a9c2e7b1d4f60a9c"));
    assert!(is_code("aGVsbG8gd29ybGQgdGhpcyBpcyBhIHRlc3Q0"));
    assert!(is_code("AB12-CD34-EF56-GH78"));
    assert!(!is_code("internationalization"));
    assert!(!is_code("2026-11-11"));
    assert_eq!(clean_for_meaning("code 8f3a9c2e7b1d4f60a9c2e7b1d4f60a9c here"), "code here");
}

/// (6c) Query strings leave.
/// Broken copy: in `is_query`, `w.contains('=') && (w.contains('&') || w.starts_with('?'))` -> `false`.
#[test]
fn clean_query_strings_leave() {
    assert!(is_query("?utm_source=mail"));
    assert!(is_query("a=1&b=2"));
    assert!(!is_query("x=1"));
    assert_eq!(clean_for_meaning("see ?utm_source=mail&utm_medium=x below"), "see below");
}

/// (6d) Lines starting with ">" leave; an answer written below a quote stays.
/// Broken copy: in `drop_quoted`, `if t.starts_with('>') { i += 1; continue; }` -> delete the line.
#[test]
fn clean_quoted_lines_leave() {
    assert_eq!(drop_quoted("Yes, Friday works.\n> Can we meet Friday?\n>> Earlier\nSee you."), "Yes, Friday works.\nSee you.");
    assert_eq!(clean_for_meaning("Top\n> old\n  > older"), "Top");
}

/// (6e) English reply header: "On … wrote:" and the block under it leave, on one line or wrapped over two.
/// Broken copy: in `is_reply_header`, `(s.starts_with("on ") && s.ends_with("wrote:"))` -> `false`.
#[test]
fn clean_english_reply_header() {
    assert!(is_reply_header("On Mon, 10 Nov 2026 at 09:00, Ann Lee <ann@example.test> wrote:"));
    assert_eq!(drop_quoted("Thanks!\n\nOn Mon, 10 Nov 2026 at 09:00, Ann <a@x.test> wrote:\nold text\nmore old").trim_end(), "Thanks!");
    assert_eq!(drop_quoted("Thanks!\nOn Mon, 10 Nov 2026 at 09:00, Ann Lee\n<a@x.test> wrote:\n\n> old").trim_end(), "Thanks!");
}

/// (6f) French reply header: "Le … a écrit :" (with a narrow no-break space before the colon, as Gmail writes it).
/// Broken copy: in `is_reply_header`, `(s.ends_with("a écrit :") || s.ends_with("a écrit:"))` -> `s.ends_with("a écrit:")`.
#[test]
fn clean_french_reply_header() {
    assert!(is_reply_header("Le lun. 10 nov. 2026 à 09:00, Anne <a@x.test> a écrit\u{202f}:"));
    assert!(is_reply_header("Le 10/11/2026 à 09:00, Anne a écrit :"));
    assert_eq!(drop_quoted("Merci.\nLe 10/11/2026 à 09:00, Anne a écrit :\nancien texte"), "Merci.");
}

/// (6g) Dutch reply header: "Op … schreef …:".
/// Broken copy: in `is_reply_header`, `(s.starts_with("op ") && s.contains(" schreef") && s.ends_with(':'))` -> `false`.
#[test]
fn clean_dutch_reply_header() {
    assert!(is_reply_header("Op wo 11 nov. 2026 om 10:00 schreef Jan Jansen <jan@x.test>:"));
    assert_eq!(drop_quoted("Dank je.\nOp wo 11 nov. 2026 om 10:00 schreef Jan <jan@x.test>:\noude tekst"), "Dank je.");
}

/// (6h) German reply header: "Am … schrieb …:".
/// Broken copy: in `is_reply_header`, `(s.starts_with("am ") && s.contains(" schrieb") && s.ends_with(':'))` -> `false`.
#[test]
fn clean_german_reply_header() {
    assert!(is_reply_header("Am Mi., 11. Nov. 2026 um 10:00 Uhr schrieb Hans Meier <h@x.test>:"));
    assert_eq!(drop_quoted("Danke.\nAm Mi., 11. Nov. 2026 um 10:00 Uhr schrieb Hans <h@x.test>:\nalter Text"), "Danke.");
}

/// (6i) Spanish reply header: "El … escribió:".
/// Broken copy: in `is_reply_header`, `(s.starts_with("el ") && s.ends_with("escribió:"))` -> `false`.
#[test]
fn clean_spanish_reply_header() {
    assert!(is_reply_header("El mié, 11 nov 2026 a las 10:00, Juan (<j@x.test>) escribió:"));
    assert_eq!(drop_quoted("Gracias.\nEl mié, 11 nov 2026 a las 10:00, Juan escribió:\ntexto viejo"), "Gracias.");
}

/// (6j) "-----Original Message-----" and its French, Dutch, German and Spanish forms: the rest leaves.
/// Broken copy: in `is_separator`, `if !t.starts_with("--") { return false; }` -> `return false;`.
#[test]
fn clean_original_message_lines() {
    for line in ["-----Original Message-----", "----- Message d'origine -----", "-----Oorspronkelijk bericht-----", "-----Ursprüngliche Nachricht-----", "-----Mensaje original-----"] {
        assert!(is_separator(line), "a separator is not seen");
        assert_eq!(drop_quoted(&format!("New text.\n{line}\nFrom: x\nold")), "New text.");
    }
    assert!(!is_separator("-- \nsignature"));
}

/// (6k) Runs of white space become one space (one line break when the run holds one).
/// Broken copy: in `clean_for_meaning`, `out.push(if first { '\n' } else { ' ' });` -> `out.push_str("  ");`.
#[test]
fn clean_white_space_runs_become_one() {
    assert_eq!(clean_for_meaning("a  b\t\tc\n\n\n  d   e"), "a b c\nd e");
    assert_eq!(clean_for_meaning("Quarterly brochure quote\nHeavy matte stock, 170 g, two colours"), "Quarterly brochure quote\nHeavy matte stock, 170 g, two colours");
}

/// (6l) A mail that is only a quote keeps its text (it would read as nothing).
/// Broken copy: in `clean_for_meaning`, `if unquoted.split_whitespace().next().is_some() { unquoted } else { text.to_string() }` -> `unquoted`.
#[test]
fn clean_a_mail_that_is_only_a_quote_keeps_it() {
    assert_eq!(clean_for_meaning("> only the earlier mail"), "> only the earlier mail");
}

/// (7) The byte bound never counts fewer tokens than bytes (the model's BPE with byte fallback makes at most one token per byte).
/// Broken copy: in `ByteBound::spans`, `for _ in 0..c.len_utf8()` -> `for _ in 0..1`.
#[test]
fn the_byte_bound_counts_every_byte() {
    assert_eq!(ByteBound.spans("abc").len(), 3);
    assert_eq!(ByteBound.spans("é€").len(), 5);
    assert_eq!(count_tokens(&ByteBound, "abc"), 5);
    let p = body_pieces(&ByteBound, &prose(400, "y"), "Subject");
    for (i, t) in p.texts.iter().enumerate() { assert!(fits(&ByteBound, p.title.as_deref(), t), "byte-bound piece {i} does not fit"); }
}

/// (8) Real token counts, only when WHOOSH_EMBED_WEIGHTS names a folder with tokenizer.json (the lead sets it): every piece of a
/// link-heavy mail with its title is within the model's input by the model's OWN tokenizer, the byte bound is never below the model's
/// count, and the model makes more tokens than words of such a mail (why words could not size a piece).
/// Broken copy: in `ModelTokenizer::spans`, `Ok(enc) => enc.get_offsets()...` -> `Ok(_) => Vec::new(),` (counts nothing).
#[test]
fn real_tokenizer_counts_when_the_weights_are_given() {
    let Some(folder) = std::env::var_os("WHOOSH_EMBED_WEIGHTS") else { eprintln!("WHOOSH_EMBED_WEIGHTS unset: skipped"); return };
    let Some(file) = ModelTokenizer::file_in(std::path::Path::new(&folder)) else { eprintln!("no tokenizer.json in WHOOSH_EMBED_WEIGHTS: skipped"); return };
    let tok = ModelTokenizer::from_file(&file).expect("the model's tokenizer loads");
    assert!(tok.specials() >= 1, "the model adds its begin mark");
    let mut body = String::new();
    for k in 0..40 {
        body.push_str(&prose(30, &letters(k)));
        body.push_str(" ref AB12-CD34-EF56 Zürich 2026-11-11 https://example.com/x?y=1 ");
    }
    let words = body.split_whitespace().count();
    assert!(count_tokens(&tok, &body) > words, "the model makes more tokens than words here");
    let p = body_pieces(&tok, &body, "Weekly figures, Zürich office");
    assert!(p.texts.len() > 1);
    assert_covers(&clean_for_meaning(&body), &p.texts);
    for (i, t) in p.texts.iter().enumerate() {
        let form = document_form(p.title.as_deref(), t);
        let n = count_tokens(&tok, &form);
        assert!(n <= MODEL_INPUT_TOKENS, "piece {i}: {n} model tokens with its title");
        assert!(count_tokens(&ByteBound, &form) >= n, "piece {i}: the byte bound is below the model's count");
    }
}

/// One token per character: a run with no space is cut inside by `units_of` (a word over half a piece), as a long code or a text with no spaces is
/// by the model's own tokenizer.
struct CharTok;
impl Tokenizer for CharTok {
    fn spans(&self, text: &str) -> Vec<Range<usize>> { text.char_indices().filter(|(_, c)| !c.is_whitespace()).map(|(i, c)| i..i + c.len_utf8()).collect() }
    fn specials(&self) -> usize { 2 }
}

/// (c1436f49, main111: "words in no stored piece 4 of 72809") A word cut inside, its parts in two pieces, is read whole by the model: the count of
/// words in no piece says 0, where a count by whole words found in the pieces (the engine read before round 20) says 1.
/// Broken copy: in `words_outside`, `if s <= last.1 { last.1 = last.1.max(e); continue; }` -> `if false && s <= last.1 { last.1 = last.1.max(e); continue; }`
/// (the pieces are not joined, so the cut word sits in no single one).
#[test]
fn a_word_cut_inside_is_in_the_pieces_that_hold_its_parts() {
    let run: String = (0..40).map(|i| (b'a' + (i % 26) as u8) as char).collect();
    let clean = format!("short {run} tail");
    let pieces = cut_pieces(&CharTok, None, &clean, 10, 2);
    assert!(pieces.len() > 2, "the run is cut into several pieces: {}", pieces.len());
    let whole: std::collections::HashSet<&str> = pieces.iter().flat_map(|p| p.split_whitespace()).collect();
    assert_eq!(clean.split_whitespace().filter(|w| !whole.contains(w)).count(), 1, "the old count by whole words misses the cut word");
    let refs: Vec<&str> = pieces.iter().map(String::as_str).collect();
    let (words, outside) = words_outside(&clean, &refs);
    assert_eq!((words, outside.len()), (3, 0), "every word, the cut one included, is in the pieces");
}

/// (c1436f49) A word that no piece holds is counted, by its offset: a piece left out (a piece of codes, or a missing last piece) leaves its words outside.
/// Broken copy: in `words_outside`, `if !held(s, i) { outside.push(s); }` -> `if false { outside.push(s); }`.
#[test]
fn a_word_no_piece_holds_is_counted_by_its_offset() {
    let clean = "alpha beta gamma delta";
    assert_eq!(words_outside(clean, &["alpha beta", "beta gamma"]), (4, vec![17]), "delta is in no piece");
    assert_eq!(words_outside(clean, &["alpha beta", "beta gamma", "gamma delta"]), (4, vec![]), "every word is held");
    assert_eq!(words_outside(clean, &["Subject standing in"]), (4, vec![0, 6, 11, 17]), "a piece that is not a slice of the text holds nothing");
}

/// (c1436f49, main112: row 32 "116 cut", "lost by the cut 50") A mail that repeats a passage: its later pieces have the same text as earlier ones. Read as byte
/// ranges of the cut, every word is in a piece; found again by their text from the PREVIOUS piece's start, every later copy landed on the first and its words
/// counted as lost though the model reads them. The cut keeps every byte; the count was wrong.
/// Broken copy: in `words_outside`, `from = s + clean[s..].chars().next().map_or(1, char::len_utf8);` -> `from = s;`.
#[test]
fn a_mail_that_repeats_a_passage_loses_no_word() {
    let clean = "alpha beta alpha beta alpha beta alpha beta";
    let ranges = cut_ranges(&FiveTok, None, clean, 10, 0);
    assert_eq!(ranges.len(), 4, "two words a piece, no overlap: {} pieces", ranges.len());
    assert_eq!(words_outside_ranges(clean, &ranges), (8, vec![]), "the ranges of the cut cover every word");
    let pieces: Vec<&str> = ranges.iter().map(|r| &clean[r.clone()]).collect();
    assert!(pieces.iter().all(|p| *p == "alpha beta"), "every piece has the same text");
    assert_eq!(words_outside(clean, &pieces), (8, vec![]), "found again by their text, in order, the pieces still cover every word");
}
