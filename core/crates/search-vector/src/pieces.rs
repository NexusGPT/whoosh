//! pieces (SPEC A151, A152): the text the meaning model reads, cut so that the model reads ALL of it.
//!
//! A piece is sized in the MODEL's tokens (the `Tokenizer` given: the model's own tokenizer.json through `ModelTokenizer`, or the
//! `ByteBound` fallback, which can only over-count), never by words or characters alone: the owner's mail has 4.62 tokens per word
//! (links, codes), so 512 words were about 1,128 tokens and the model kept the first 512. The rule, after DigUp's
//! (Extractors.swift:107-131: clean the text first, cut at a space, the next piece starts on a word start, a small overlap):
//! 1. `clean_for_meaning`: quoted earlier mail leaves (lines starting with ">", the block under a reply header in English, French, Dutch,
//!    German and Spanish, and under an "Original Message" line), a link becomes its site name, long tracking codes and query strings
//!    leave, runs of white space become one space (one line break when the run holds one, so a passage stays readable).
//! 2. `cut_pieces`: words in order, at most PIECE_TEXT_TOKENS tokens of text per piece, PIECE_OVERLAP_TOKENS shared with the next;
//!    a single word longer than half a piece (a run of text with no space) is cut at token starts.
//! 3. Each piece is checked whole, in the model's document form `title: <title> | text: <piece>` with the model's special tokens:
//!    it fits MODEL_INPUT_TOKENS (a piece that does not, by a boundary effect, gives up its last word until it does).
//! 4. A piece that is mostly digits or codes (fewer letters than half its non-space characters) gets no vector.
//!
//! The word index never reads this: it keeps the stored text whole.
use std::ops::Range;
use std::path::Path;

use crate::chunker::Tokenizer;
use crate::constants::CHUNK_TOKENS;

// Sizes this file needs that have no row in docs/CONSTANTS.md yet (proposed in docs/lanes/pieces.md); each is named here once.
/// The model's input limit in tokens, special tokens included: the cut GemmaEmbedder keeps (`.prefix(CHUNK_TOKENS)`).
pub const MODEL_INPUT_TOKENS: usize = CHUNK_TOKENS;
/// Tokens of text in one piece at most (DigUp: about 450 under the same 512 cap).
pub const PIECE_TEXT_TOKENS: usize = crate::constants::PIECE_TEXT_TOKENS;
/// Tokens two neighbouring pieces share (about one sentence or two).
pub const PIECE_OVERLAP_TOKENS: usize = crate::constants::PIECE_OVERLAP_TOKENS;
/// Tokens of the title (the subject, or "<subject> · <file name>") at most; the title is cut beyond.
pub const PIECE_TITLE_TOKENS: usize = crate::constants::PIECE_TITLE_TOKENS;
/// A word this long or longer, made only of letters, digits and `+/=_-%.~`, with a digit in it, is a code (a tracking id, a hash).
pub const CODE_MIN_CHARS: usize = crate::constants::CODE_MIN_CHARS;
/// A word this long or longer of base64 letters with upper and lower case inside it is a code even without a digit.
pub const BASE64_MIN_CHARS: usize = crate::constants::BASE64_MIN_CHARS;
/// A line longer than this many characters is never a reply header (the ledger's REPLY_HEADER_LINE_MAX_CHARS; not the piece size).
pub const REPLY_HEADER_LINE_MAX_CHARS: usize = contracts::constants::REPLY_HEADER_LINE_MAX_CHARS as usize;

/// The model's document form, as Sources/Features/Embedding/DocumentForm.swift writes it: `title: <title or none> | text: <text>`.
pub fn document_form(title: Option<&str>, text: &str) -> String {
    format!("title: {} | text: {}", clean_title(title), text)
}

/// DocumentForm.cleanTitle: one line, no " | " inside, trimmed; empty gives "none".
pub fn clean_title(title: Option<&str>) -> String {
    let Some(t) = title else { return "none".into() };
    let one_line = t.lines().collect::<Vec<_>>().join(" ").replace(" | ", " / ");
    let trimmed = one_line.trim();
    if trimmed.is_empty() { "none".into() } else { trimmed.to_string() }
}

/// Tokens the model reads for `text`, its special tokens included.
pub fn count_tokens(tok: &dyn Tokenizer, text: &str) -> usize { tok.spans(text).len() + tok.specials() }

/// True when the piece with its title fits the model's input.
pub fn fits(tok: &dyn Tokenizer, title: Option<&str>, piece: &str) -> bool {
    count_tokens(tok, &document_form(title, piece)) <= MODEL_INPUT_TOKENS
}

/// The title cut to at most `max` tokens (on one line), at the last space before the limit when there is one.
pub fn cut_title(tok: &dyn Tokenizer, title: &str, max: usize) -> String {
    let one = title.lines().collect::<Vec<_>>().join(" ");
    let one = one.trim();
    let spans = tok.spans(one);
    if spans.len() <= max { return one.to_string(); }
    let limit = spans[max].start;
    let cut = match one[..limit].rfind(char::is_whitespace) { Some(i) if i > 0 => &one[..i], _ => &one[..limit] };
    cut.trim_end().to_string()
}

/// The title of a file's pieces: "<subject> · <file name>", the subject cut first so the file name stays.
pub fn attachment_title(tok: &dyn Tokenizer, subject: &str, file_name: &str, max: usize) -> String {
    let name = cut_title(tok, file_name, max);
    let subject = subject.lines().collect::<Vec<_>>().join(" ");
    let subject = subject.trim();
    if subject.is_empty() { return name; }
    let whole = format!("{subject} · {name}");
    if tok.spans(&whole).len() <= max { return whole; }
    let room = max.saturating_sub(tok.spans(&format!(" · {name}")).len());
    let s = cut_title(tok, subject, room);
    if s.is_empty() { name } else { format!("{s} · {name}") }
}

/// What the meaning index files for one document: its title (cut) and its pieces.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pieces { pub title: Option<String>, pub texts: Vec<String> }

/// A mail body's pieces under its subject. Every piece with the title fits the model; a piece that is mostly digits or codes is left
/// out; when that leaves none, the subject alone is the one piece (a mail of numbers is still found by its subject).
pub fn body_pieces(tok: &dyn Tokenizer, text: &str, subject: &str) -> Pieces {
    let title = Some(cut_title(tok, subject, PIECE_TITLE_TOKENS)).filter(|t| !t.is_empty());
    let mut texts = pieces_of(tok, title.as_deref(), &clean_for_meaning(text));
    if texts.is_empty() {
        if let Some(t) = &title { texts.push(t.clone()); }
    }
    Pieces { title, texts }
}

/// A file's pieces under "<subject> · <file name>" (same rule as a body; no fallback piece: a file of numbers has no vector).
pub fn attachment_pieces(tok: &dyn Tokenizer, text: &str, subject: &str, file_name: &str) -> Pieces {
    let title = Some(attachment_title(tok, subject, file_name, PIECE_TITLE_TOKENS)).filter(|t| !t.is_empty());
    let texts = pieces_of(tok, title.as_deref(), &clean_for_meaning(text));
    Pieces { title, texts }
}

/// Cleaned text into pieces that fit under `title`; mostly-code pieces are left out.
pub fn pieces_of(tok: &dyn Tokenizer, title: Option<&str>, clean: &str) -> Vec<String> {
    cut_pieces(tok, title, clean, PIECE_TEXT_TOKENS, PIECE_OVERLAP_TOKENS).into_iter().filter(|p| !mostly_codes(p)).collect()
}

/// The rule for "no vector": fewer letters than half the piece's non-space characters (a table of amounts, a list of numbers or codes).
pub fn mostly_codes(piece: &str) -> bool {
    let (mut letters, mut all) = (0usize, 0usize);
    for c in piece.chars().filter(|c| !c.is_whitespace()) {
        all += 1;
        if c.is_alphabetic() { letters += 1; }
    }
    letters * 2 < all || all == 0
}

/// Cuts `clean` into pieces of at most `size` tokens of text, `overlap` tokens shared by neighbours, at white space; each piece with
/// `title` fits MODEL_INPUT_TOKENS (checked whole). Pieces are slices of `clean`, so every word of `clean` is in at least one piece.
pub fn cut_pieces(tok: &dyn Tokenizer, title: Option<&str>, clean: &str, size: usize, overlap: usize) -> Vec<String> {
    cut_ranges(tok, title, clean, size, overlap).into_iter().map(|r| clean[r].to_string()).collect()
}

/// The pieces of `cut_pieces` as byte ranges of `clean` (the count of words in no piece reads them; a piece's text can occur more than once in a
/// mail that repeats itself, so a range, never a search for the text, says where a piece is).
pub fn cut_ranges(tok: &dyn Tokenizer, title: Option<&str>, clean: &str, size: usize, overlap: usize) -> Vec<Range<usize>> {
    let size = size.max(1);
    let units = units_of(tok, clean, size);
    let mut out = Vec::new();
    let mut start = 0;
    while start < units.len() {
        let mut end = start;
        let mut sum = 0;
        while end < units.len() && (end == start || sum + units[end].1 <= size) {
            sum += units[end].1;
            end += 1;
        }
        // the whole piece, in the model's form, fits; a boundary effect gives back words from the end (never the first)
        while end > start + 1 && !fits(tok, title, &clean[units[start].0.start..units[end - 1].0.end]) { end -= 1; }
        out.push(units[start].0.start..units[end - 1].0.end);
        if end == units.len() { break; }
        // the next piece starts on a word start, `overlap` tokens back at most, and always after this one's start
        let mut next = end;
        let mut back = 0;
        while next > start + 1 && back + units[next - 1].1 <= overlap {
            back += units[next - 1].1;
            next -= 1;
        }
        start = next;
    }
    out
}

/// The words-in-no-piece count of item c1436f49 (the engine read `index/pieces`, checks/ownercopy-pieces.py): the words of `clean` (split at
/// white space) with a byte in none of `pieces`. Each piece is located in `clean` in order (cut_pieces makes slices of it, neighbours sharing an
/// overlap); the located ranges are joined where they touch or overlap, so a word cut INSIDE (a run with no space longer than half a piece,
/// `units_of`) is held when its parts are, whatever piece holds each part. A piece that is not a slice of `clean` (the subject standing in for a
/// mail of numbers) covers nothing. Returns (the number of words, the byte start of each word in no piece). Counts and offsets only.
pub fn words_outside(clean: &str, pieces: &[&str]) -> (usize, Vec<usize>) {
    let mut ranges: Vec<Range<usize>> = Vec::new();
    let mut from = 0usize;
    for p in pieces {
        if p.is_empty() { continue; }
        // the next piece starts strictly after the previous one's start (cut_ranges: `next` is always past `start`), so the search starts one byte
        // further: a mail that repeats a passage (row 32 of main112's run, "lost by the cut 50") once found every later copy of a piece at the first,
        // and the words of the later copies counted as lost
        let at = clean.get(from..).and_then(|rest| rest.find(p)).map(|i| from + i).or_else(|| clean.find(p));
        if let Some(s) = at {
            ranges.push(s..s + p.len());
            from = s + clean[s..].chars().next().map_or(1, char::len_utf8);
        }
    }
    words_outside_ranges(clean, &ranges)
}

/// The pieces left after `pieces_of`'s rule (mostly-code pieces out), as byte ranges of `clean`: what a mail's stored body pieces cover when they are
/// today's cut of its text.
pub fn piece_ranges(tok: &dyn Tokenizer, title: Option<&str>, clean: &str) -> Vec<Range<usize>> {
    cut_ranges(tok, title, clean, PIECE_TEXT_TOKENS, PIECE_OVERLAP_TOKENS).into_iter().filter(|r| !mostly_codes(&clean[r.clone()])).collect()
}

/// `words_outside` on byte ranges of `clean` (no search for any text): the words with a byte in none of them, the ranges joined where they touch.
pub fn words_outside_ranges(clean: &str, pieces: &[Range<usize>]) -> (usize, Vec<usize>) {
    let mut covered: Vec<(usize, usize)> = pieces.iter().map(|r| (r.start, r.end)).collect();
    covered.sort();
    let mut merged: Vec<(usize, usize)> = Vec::new();
    for (s, e) in covered {
        if let Some(last) = merged.last_mut() {
            if s <= last.1 { last.1 = last.1.max(e); continue; }
        }
        merged.push((s, e));
    }
    let held = |s: usize, e: usize| merged.iter().any(|&(a, b)| a <= s && e <= b);
    let (mut words, mut outside) = (0usize, Vec::new());
    let mut at: Option<usize> = None;
    for (i, c) in clean.char_indices() {
        match (c.is_whitespace(), at) {
            (true, Some(s)) => { words += 1; if !held(s, i) { outside.push(s); } at = None; }
            (false, None) => at = Some(i),
            _ => {}
        }
    }
    if let Some(s) = at { words += 1; if !held(s, clean.len()) { outside.push(s); } }
    (words, outside)
}

/// The words of `clean` (byte ranges) with their token counts; a word over half a piece is split at token starts.
fn units_of(tok: &dyn Tokenizer, clean: &str, size: usize) -> Vec<(Range<usize>, usize)> {
    let mut words: Vec<Range<usize>> = Vec::new();
    let mut at: Option<usize> = None;
    for (i, c) in clean.char_indices() {
        match (c.is_whitespace(), at) {
            (true, Some(s)) => { words.push(s..i); at = None; }
            (false, None) => at = Some(i),
            _ => {}
        }
    }
    if let Some(s) = at { words.push(s..clean.len()); }
    if words.is_empty() { return Vec::new(); }
    // each token belongs to the word its start falls in; one starting in the space before a word (the model's "▁word") to that word
    let starts: Vec<usize> = tok.spans(clean).into_iter().map(|r| r.start).collect();
    let mut cost = vec![0usize; words.len()];
    let mut per_word: Vec<Vec<usize>> = vec![Vec::new(); words.len()];
    for s in &starts {
        let j = words.partition_point(|w| w.start <= *s);
        let k = if j == 0 { 0 } else if *s >= words[j - 1].end && j < words.len() { j } else { j - 1 };
        cost[k] += 1;
        per_word[k].push(*s);
    }
    let half = (size / 2).max(1);
    let mut units = Vec::new();
    for (k, w) in words.into_iter().enumerate() {
        if cost[k] <= half {
            units.push((w, cost[k].max(1)));
            continue;
        }
        // a run with no space (a long word, text with no spaces): cut at token starts inside it, `half` tokens a part (byte-fallback
        // tokens share a start: a part never ends inside one character)
        let mut from = w.start;
        let mut n = 0;
        for s in per_word[k].iter().copied() {
            if n >= half && s > from && s < w.end && clean.is_char_boundary(s) { units.push((from..s, n)); from = s; n = 0; }
            n += 1;
        }
        units.push((from..w.end, n.max(1)));
    }
    units
}

// --- cleaning -------------------------------------------------------------------------------------------------------------------

/// The text the meaning model reads (see the header). A mail that is only a quote keeps its quote (it would read as nothing).
pub fn clean_for_meaning(text: &str) -> String {
    let unquoted = drop_quoted(text);
    let base = if unquoted.split_whitespace().next().is_some() { unquoted } else { text.to_string() };
    let mut out = String::with_capacity(base.len());
    for line in base.lines() {
        let mut first = true;
        for word in line.split_whitespace() {
            let Some(w) = clean_word(word) else { continue };
            if !out.is_empty() { out.push(if first { '\n' } else { ' ' }); }
            out.push_str(&w);
            first = false;
        }
    }
    out
}

/// One word: a link becomes its site name, a code or a query string leaves (None), anything else stays as it is.
pub fn clean_word(word: &str) -> Option<String> {
    let lower = word.to_ascii_lowercase();
    let at = ["https://", "http://", "www."].iter().filter_map(|p| lower.find(p)).min();
    if let Some(i) = at {
        let before = word[..i].trim_end_matches(['<', '(', '[', '"', '\'']);
        let host = host_of(&word[i..]);
        let joined = format!("{before}{host}");
        return if joined.is_empty() { None } else { Some(joined) };
    }
    if let Some(rest) = lower.strip_prefix("mailto:") { return Some(word[word.len() - rest.len()..].to_string()).filter(|s| !s.is_empty()); }
    let core = word.trim_matches(|c: char| "\"'()[]<>{},;:.!?".contains(c));
    if is_code(core) || is_query(core) { None } else { Some(word.to_string()) }
}

/// The site of a link: what follows the scheme up to its path, query, port or end, without "www." and without a user part.
fn host_of(link: &str) -> String {
    let lower = link.to_ascii_lowercase();
    let rest = if lower.starts_with("https://") { &link[8..] } else if lower.starts_with("http://") { &link[7..] } else { link };
    let end = rest.find(['/', '?', '#', ':', '>', ')', ']', '"', '\'', ',']).unwrap_or(rest.len());
    let host = &rest[..end];
    let host = host.rsplit('@').next().unwrap_or(host);
    let host = host.trim_end_matches('.');
    let host = if host.to_ascii_lowercase().starts_with("www.") { &host[4..] } else { host };
    host.to_string()
}

/// A tracking id, a hash, a base64 or hex run: long, of code characters only, with a digit (or base64 with case changes inside).
pub fn is_code(w: &str) -> bool {
    let n = w.chars().count();
    if n < CODE_MIN_CHARS || !w.chars().all(|c| c.is_ascii_alphanumeric() || "+/=_-%.~".contains(c)) { return false; }
    if w.chars().any(|c| c.is_ascii_digit()) { return true; }
    let inner_upper = w.chars().skip(1).filter(|c| c.is_ascii_uppercase()).count();
    n >= BASE64_MIN_CHARS && w.chars().any(|c| c.is_ascii_lowercase()) && inner_upper >= 4
}

/// A query string: `?a=b`, or `a=b&c=d`.
pub fn is_query(w: &str) -> bool { w.contains('=') && (w.contains('&') || w.starts_with('?')) }

/// The text without quoted earlier mail: lines starting with ">", and the block under a reply header or an "Original Message" line.
pub fn drop_quoted(text: &str) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let mut keep: Vec<&str> = Vec::with_capacity(lines.len());
    let mut i = 0;
    while i < lines.len() {
        let t = lines[i].trim_start();
        if t.starts_with('>') { i += 1; continue; }
        if is_separator(t) { break; }
        if let Some(len) = reply_header_at(&lines, i) {
            // the block under it: quoted lines (">", skipped one by one, so an answer written below them stays), or, when the old mail
            // follows unmarked (Outlook, a converted html quote), everything to the end
            let mut j = i + len;
            while j < lines.len() && lines[j].trim().is_empty() { j += 1; }
            if j < lines.len() && lines[j].trim_start().starts_with('>') { i += len; continue; }
            break;
        }
        keep.push(lines[i]);
        i += 1;
    }
    keep.join("\n")
}

/// "-----Original Message-----" and its French, Dutch, German and Spanish forms (any number of dashes, spaces allowed).
pub fn is_separator(line: &str) -> bool {
    let t = line.trim();
    if !t.starts_with("--") { return false; }
    let inner = t.trim_matches(|c: char| c == '-' || c.is_whitespace()).to_lowercase();
    ["original message", "message d'origine", "message d\u{2019}origine", "oorspronkelijk bericht", "ursprüngliche nachricht", "mensaje original"]
        .contains(&inner.as_str())
}

/// The number of lines (1 or 2: mail programs wrap a long header) of a reply header starting at line `i`, if one does.
fn reply_header_at(lines: &[&str], i: usize) -> Option<usize> {
    if is_reply_header(lines[i]) { return Some(1); }
    let opener = norm(lines[i]);
    let opens = ["on ", "le ", "op ", "am ", "el "].iter().any(|o| opener.starts_with(o));
    if opens && i + 1 < lines.len() && opener.chars().count() < 300 && is_reply_header(&format!("{} {}", lines[i].trim(), lines[i + 1].trim())) {
        return Some(2);
    }
    None
}

/// Lower case, no-break spaces as spaces, trimmed.
fn norm(s: &str) -> String { s.replace(['\u{a0}', '\u{202f}'], " ").trim().to_lowercase() }

/// One reply header line: "On … wrote:", "Le … a écrit :", "Op … schreef …:", "Am … schrieb …:", "El … escribió:".
pub fn is_reply_header(line: &str) -> bool {
    let s = norm(line);
    if s.chars().count() > REPLY_HEADER_LINE_MAX_CHARS { return false; }
    (s.starts_with("on ") && s.ends_with("wrote:"))
        || (s.starts_with("le ") && (s.ends_with("a écrit :") || s.ends_with("a écrit:")))
        || (s.starts_with("op ") && s.contains(" schreef") && s.ends_with(':'))
        || (s.starts_with("am ") && s.contains(" schrieb") && s.ends_with(':'))
        || (s.starts_with("el ") && s.ends_with("escribió:"))
}

// --- counters ---------------------------------------------------------------------------------------------------------------------

/// The fallback counter when the model's tokenizer.json is not known: one token per BYTE of UTF-8 (the model's tokenizer is a BPE
/// with byte fallback, so it never makes more tokens than bytes), plus the model's two special tokens. It can only over-count: a
/// piece it cuts always fits; the pieces are smaller than they need be (about 400 bytes of text).
pub struct ByteBound;

impl Tokenizer for ByteBound {
    fn spans(&self, text: &str) -> Vec<Range<usize>> {
        let mut out = Vec::with_capacity(text.len());
        for (i, c) in text.char_indices() {
            for _ in 0..c.len_utf8() { out.push(i..i + c.len_utf8()); }
        }
        out
    }
    fn specials(&self) -> usize { 2 }
}

/// The model's own tokenizer, read from its tokenizer.json (the Hugging Face `tokenizers` library, the format the Swift side's
/// swift-transformers reads too). Built only while the indexer works, dropped when it is idle.
pub struct ModelTokenizer { inner: tokenizers::Tokenizer, specials: usize }

impl ModelTokenizer {
    pub fn from_file(path: &Path) -> Result<ModelTokenizer, String> {
        let mut inner = tokenizers::Tokenizer::from_file(path).map_err(|e| format!("the model's tokenizer did not load: {e}"))?;
        // a tokenizer.json may carry a truncation or padding setting: counting must see every token
        let _ = inner.with_truncation(None);
        let _ = inner.with_padding(None);
        let with = inner.encode("a", true).map_err(|e| format!("the model's tokenizer did not run: {e}"))?.len();
        let without = inner.encode("a", false).map_err(|e| format!("the model's tokenizer did not run: {e}"))?.len();
        Ok(ModelTokenizer { inner, specials: with.saturating_sub(without) })
    }

    /// `<folder>/tokenizer.json` when the folder holds one.
    pub fn file_in(folder: &Path) -> Option<std::path::PathBuf> {
        let f = folder.join("tokenizer.json");
        f.is_file().then_some(f)
    }
}

impl Tokenizer for ModelTokenizer {
    fn spans(&self, text: &str) -> Vec<Range<usize>> {
        match self.inner.encode(text, false) {
            Ok(enc) => enc.get_offsets().iter().map(|(a, b)| *a..*b).collect(),
            // never under-count on a failure: the byte bound
            Err(_) => ByteBound.spans(text),
        }
    }
    fn specials(&self) -> usize { self.specials }
}
