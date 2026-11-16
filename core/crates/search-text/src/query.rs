//! Turning the owner's words into a query, and a query into hits.
//!
//! A bare word must appear somewhere in the message (subject, people, body or attachments); several
//! words must all appear. A phrase in double quotes is an EXACT phrase: its words, in that order, with
//! nothing between them, inside one field.
use crate::error::Error;
use crate::limits::SNIPPET_CHARS;
use crate::schema::Fields;
use contracts::{MatchedIn, SearchHit, TextRange};
use tantivy::collector::TopDocs;
use tantivy::query::{Query, QueryParser};
use tantivy::schema::Field;
use tantivy::snippet::SnippetGenerator;
use tantivy::{Index, IndexReader};

pub fn parse(index: &Index, fields: &Fields, text: &str) -> Box<dyn Query> {
    let mut parser = QueryParser::for_index(index, fields.searchable());
    parser.set_conjunction_by_default();
    // Lenient: a stray quote or colon in a typed query must narrow nothing and break nothing.
    let (query, _errors) = parser.parse_query_lenient(text);
    query
}

/// Run `text` and return up to `limit` hits, best first. `body_of` supplies the text each snippet is cut from.
pub fn run(index: &Index, fields: &Fields, reader: &IndexReader, text: &str, limit: usize, body_of: &dyn Fn(&str) -> Option<String>) -> Result<Vec<SearchHit>, Error> {
    if text.trim().is_empty() || limit == 0 {
        return Ok(Vec::new());
    }
    let query = parse(index, fields, text);
    let searcher = reader.searcher();
    let top = searcher.search(&query, &TopDocs::with_limit(limit).order_by_score())?;
    let mut snippets = SnippetGenerator::create(&searcher, &*query, fields.body)?;
    snippets.set_max_num_chars(SNIPPET_CHARS);
    let mut hits = Vec::with_capacity(top.len());
    for (score, address) in top {
        let doc: tantivy::TantivyDocument = searcher.doc(address)?;
        let Some(id) = crate::engine::text_of(&doc, fields.id) else { continue };
        // The table is the truth: a hit whose message is gone (deleted, index not yet reconciled) is dropped.
        let Some(body) = body_of(&id) else { continue };
        let snippet = {
            let s = snippets.snippet(&body);
            if s.is_empty() {
                body.chars().take(SNIPPET_CHARS).collect()
            } else {
                s.fragment().to_string()
            }
        };
        // matched_in is a placeholder until this lane reads the matching field (delta 8, mechanical edit by contracts)
        hits.push(SearchHit { message_id: id, score: score as f64, snippet, matched_in: contracts::MatchedIn::MeaningOnly, attachment_name: None, attachment_note: None });
    }
    Ok(hits)
}

// ---- match-inside (SPEC A149): the owner's search on the real engine ----
//
// A typed word matches every mail that HOLDS its letters: a typed word of SUBSTRING_MIN_CHARS letters or more stands for every
// index word that is it, starts with it, or holds it inside ("consultant" finds "consultants" and the "brightconsultants" of
// brightconsultants.example; "consultancy" does not hold those letters); a shorter word stands only for itself. The index's analyzer
// splits on anything that is not a letter or digit, lower-cases and folds accents, so a run of typed letters inside a stored
// word is a run inside one index word: reading the index's word lists finds exactly the mails whose subject, sender (name and
// address), text (body, attachment names and text) hold the letters, case and accent blind. The recipients keep the rule before
// A149 (the word and the words that start with it): A149 names no recipient, and an address the owner shares with every mail would
// make every mail match a few letters of it. Several typed words: every one must be held (the conjunction of `run`); a quoted
// phrase stays an exact phrase. No query is re-indexed: the index and its schema are unchanged.
//
// Order (SPEC A149), first to last: the typed word itself, a word that starts with it, a word that holds it inside; in each, a
// match in the subject or the sender before a match in the text only. With several typed words a mail ranks by its weakest word
// (each word at its best place). The score says that rank (6 the best, 1 the least), so the engine's sort (score, then newest)
// gives the owner's order. Each hit says where it matched, with the letters to show in bold: `SubjectWords` (byte ranges in the
// subject), `SenderWords` (byte ranges in the sender's name and address), an attachment's name or text, else `TextWords` (byte
// ranges in `snippet`, the passage of the text around the first occurrence). A word matched whole or by its start is shown whole
// (the rule before A149: "Consultants" for "consultant"); letters inside a longer word are shown alone.

/// What a ranked hit's `matched_in` and snippet are read from: `body` is the text the passage is cut from (the mail's text when a
/// typed word was found in it, else its preview), the subject, the sender, and each attachment (file name, extracted text).
pub struct HitFacts {
    pub body: String,
    pub subject: String,
    pub sender_name: String,
    pub from_address: String,
    pub attachments: Vec<(String, String)>,
}

/// Where a typed word sits in an index word, best first: the word itself, a word that starts with it, a word that holds it inside.
const WHOLE: u8 = 0;
const START: u8 = 1;
const INSIDE: u8 = 2;
/// Where an index word sits in the mail: the subject or the sender, else the text (body, attachments, recipients).
const HEAD: u8 = 0;
const TEXT: u8 = 1;

/// The tier of the stored word `term` for the typed word `word`: None when it does not stand for it. `wide` false: only the word itself.
fn token_tier(term: &str, word: &str, wide: bool) -> Option<u8> {
    if term == word {
        Some(WHOLE)
    } else if !wide {
        None
    } else if term.starts_with(word) {
        Some(START)
    } else if term.contains(word) {
        Some(INSIDE)
    } else {
        None
    }
}

/// A bare typed word (already through the analyzer) is wide when it has SUBSTRING_MIN_CHARS letters or more.
fn is_wide(word: &str) -> bool {
    word.chars().count() >= crate::limits::SUBSTRING_MIN_CHARS
}

/// One stored word that stands for a typed word, where the index holds it: the segment, the field (its place in
/// `Fields::searchable`), the word's tier, and its entry in that word list.
struct Located {
    seg: usize,
    field: usize,
    term: String,
    tier: u8,
    info: tantivy::postings::TermInfo,
}

/// The stored words one typed word stands for, in every segment and field: the word itself and the words that start with it (a
/// walk from the word, at most PREFIX_SCAN_MAX per field and segment), then, not in the recipients, the words that hold it inside
/// (the word lists read whole, at most SUBSTRING_SCAN_MAX words per typed word); at most SUBSTRING_EXPANSIONS_MAX distinct words
/// are kept, the best tier first, then the shortest.
fn forms_of(searcher: &tantivy::Searcher, fields: &Fields, word: &str) -> Result<Vec<Located>, Error> {
    use crate::limits::{PREFIX_SCAN_MAX, SUBSTRING_EXPANSIONS_MAX, SUBSTRING_SCAN_MAX};
    let wide = is_wide(word);
    let w = word.as_bytes();
    let searchable: Vec<Field> = fields.searchable();
    let mut located: Vec<Located> = Vec::new();
    let mut scanned = 0usize;
    for (s, seg) in searcher.segment_readers().iter().enumerate() {
        for (f, field) in searchable.iter().enumerate() {
            let inverted = seg.inverted_index(*field)?;
            let dict = inverted.terms();
            if !wide {
                if let Some(info) = dict.get(w)? {
                    located.push(Located { seg: s, field: f, term: word.to_string(), tier: WHOLE, info });
                }
                continue;
            }
            let mut starts = dict.range().ge(w).into_stream()?;
            let mut read = 0usize;
            while read < PREFIX_SCAN_MAX && starts.advance() {
                read += 1;
                let key = starts.key();
                if !key.starts_with(w) {
                    break;
                }
                if let Ok(k) = std::str::from_utf8(key) {
                    located.push(Located { seg: s, field: f, term: k.to_string(), tier: if k == word { WHOLE } else { START }, info: starts.value().clone() });
                }
            }
            if *field == fields.to {
                continue;
            }
            let mut all = dict.stream()?;
            while scanned < SUBSTRING_SCAN_MAX && all.advance() {
                scanned += 1;
                let key = all.key();
                if key.len() <= w.len() || key.starts_with(w) || !key.windows(w.len()).any(|x| x == w) {
                    continue;
                }
                if let Ok(k) = std::str::from_utf8(key) {
                    located.push(Located { seg: s, field: f, term: k.to_string(), tier: INSIDE, info: all.value().clone() });
                }
            }
        }
    }
    let kept: std::collections::HashSet<String> = {
        let mut words: Vec<(u8, &str)> = located.iter().map(|l| (l.tier, l.term.as_str())).collect();
        words.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.len().cmp(&b.1.len())).then(a.1.cmp(b.1)));
        words.dedup();
        words.into_iter().take(SUBSTRING_EXPANSIONS_MAX).map(|(_, t)| t.to_string()).collect()
    };
    located.retain(|l| kept.contains(&l.term));
    Ok(located)
}

/// Per indexed mail (segment, doc): its best (tier, place) for one typed word, and a bit per searchable field the word was found in.
type Found = std::collections::HashMap<(u32, u32), ((u8, u8), u8)>;

/// The mails holding one typed word through any of its stored words, each at its best (tier, place).
fn docs_of(searcher: &tantivy::Searcher, fields: &Fields, located: &[Located]) -> Result<Found, Error> {
    use tantivy::schema::IndexRecordOption;
    use tantivy::{DocSet, TERMINATED};
    let searchable: Vec<Field> = fields.searchable();
    let readers = searcher.segment_readers();
    let mut out: Found = Found::new();
    for l in located {
        let seg = &readers[l.seg];
        let field = searchable[l.field];
        let place = if field == fields.subject || field == fields.from { HEAD } else { TEXT };
        let inverted = seg.inverted_index(field)?;
        let mut postings = inverted.read_postings_from_terminfo(&l.info, IndexRecordOption::Basic)?;
        let mut doc = postings.doc();
        while doc != TERMINATED {
            if !seg.is_deleted(doc) {
                let e = out.entry((l.seg as u32, doc)).or_insert(((l.tier, place), 0));
                if (l.tier, place) < e.0 {
                    e.0 = (l.tier, place);
                }
                e.1 |= 1u8 << l.field;
            }
            doc = postings.advance();
        }
    }
    Ok(out)
}

/// Keeps the mails both lists hold, each at the weaker of its two ranks, with the fields of both.
fn meet(found: Found, matched: &mut Option<Found>) {
    *matched = Some(match matched.take() {
        None => found,
        Some(prev) => prev.into_iter().filter_map(|(d, (k, m))| found.get(&d).map(|(k2, m2)| (d, (k.max(*k2), m | *m2)))).collect(),
    });
}

/// The analyzer's words of `text` with their byte ranges in `text`.
fn words_of(index: &Index, text: &str) -> Vec<(String, usize, usize)> {
    use tantivy::tokenizer::TokenStream;
    let Some(mut analyzer) = index.tokenizers().get(crate::schema::ANALYZER) else { return Vec::new() };
    let mut stream = analyzer.token_stream(text);
    let mut out = Vec::new();
    while stream.advance() {
        let t = stream.token();
        out.push((t.text.clone(), t.offset_from, t.offset_to));
    }
    out
}

/// The quoted phrases of `text` and the text outside them (an unclosed quote runs to the end).
fn split_phrases(text: &str) -> (Vec<String>, String) {
    let mut phrases = Vec::new();
    let mut bare = String::new();
    let mut inside: Option<String> = None;
    for c in text.chars() {
        if c == '"' {
            match inside.take() {
                None => inside = Some(String::new()),
                Some(p) => { if !p.trim().is_empty() { phrases.push(p); } bare.push(' '); }
            }
        } else if let Some(p) = inside.as_mut() {
            p.push(c);
        } else {
            bare.push(c);
        }
    }
    if let Some(p) = inside { if !p.trim().is_empty() { phrases.push(p); } }
    (phrases, bare)
}

/// The mails holding a quoted phrase (exact, as `run`), each as the typed word itself, in the subject or sender when the phrase is
/// there, with every field bit set (the text is read for the passage).
fn docs_of_phrase(index: &Index, fields: &Fields, searcher: &tantivy::Searcher, phrase: &str) -> Result<Found, Error> {
    use tantivy::collector::DocSetCollector;
    let quoted = format!("\"{}\"", phrase.replace('"', " "));
    let all = searcher.search(&*parse(index, fields, &quoted), &DocSetCollector)?;
    let head = {
        let mut parser = QueryParser::for_index(index, vec![fields.subject, fields.from]);
        parser.set_conjunction_by_default();
        let (query, _errors) = parser.parse_query_lenient(&quoted);
        searcher.search(&*query, &DocSetCollector)?
    };
    Ok(all.iter().map(|a| ((a.segment_ord, a.doc_id), ((WHOLE, if head.contains(a) { HEAD } else { TEXT }), u8::MAX))).collect())
}

/// One char through the index's analyzer (lower case, accents folded); empty when the analyzer keeps nothing of it.
fn fold_char(analyzer: &mut tantivy::tokenizer::TextAnalyzer, ch: char) -> String {
    use tantivy::tokenizer::TokenStream;
    let one = ch.to_string();
    let mut stream = analyzer.token_stream(&one);
    let mut out = String::new();
    while stream.advance() {
        out.push_str(&stream.token().text);
    }
    out
}

/// The byte range in `s` to show in bold for a stored word at `from..to` (analyzed as `folded`) that stands for `word` at `tier`:
/// the whole word for the word itself or a word that starts with it; the letters alone for a word that holds it inside (the
/// whole word when its chars do not fold one by one to the same letters).
fn bold_range(folder: &mut tantivy::tokenizer::TextAnalyzer, s: &str, folded: &str, from: usize, to: usize, word: &str, tier: u8) -> (usize, usize) {
    if tier != INSIDE {
        return (from, to);
    }
    let Some(at) = folded.find(word) else { return (from, to) };
    let mut concat = String::new();
    let mut pieces: Vec<(usize, usize)> = Vec::new(); // (start in `concat`, start in `s`)
    for (i, ch) in s[from..to].char_indices() {
        pieces.push((concat.len(), from + i));
        concat.push_str(&fold_char(folder, ch));
    }
    if concat != folded {
        return (from, to);
    }
    let end_at = at + word.len();
    let start = pieces.iter().rev().find(|p| p.0 <= at).map(|p| p.1).unwrap_or(from);
    let end = pieces.iter().find(|p| p.0 >= end_at).map(|p| p.1).unwrap_or(to);
    (start, end)
}

/// The byte ranges in `s` of the stored words that stand for a typed word (`typed`: the word, and whether it is wide), in order,
/// at most `max`; each range as `bold_range` says.
fn occurrences(index: &Index, s: &str, typed: &[(String, bool)], max: usize) -> Vec<(usize, usize)> {
    use tantivy::tokenizer::TokenStream;
    let mut out = Vec::new();
    if s.is_empty() || max == 0 {
        return out;
    }
    let (Some(mut analyzer), Some(mut folder)) = (index.tokenizers().get(crate::schema::ANALYZER), index.tokenizers().get(crate::schema::ANALYZER)) else { return out };
    let mut stream = analyzer.token_stream(s);
    while stream.advance() {
        let t = stream.token();
        if let Some((word, tier)) = typed.iter().find_map(|(w, wide)| token_tier(&t.text, w, *wide).map(|tier| (w, tier))) {
            out.push(bold_range(&mut folder, s, &t.text, t.offset_from, t.offset_to, word, tier));
            if out.len() >= max {
                break;
            }
        }
    }
    out
}

fn ranges(found: Vec<(usize, usize)>) -> Vec<TextRange> {
    found.into_iter().map(|(a, b)| TextRange { start: a as u32, end: b as u32 }).collect()
}

/// The passage of `text` around the first occurrence of a typed word: about SEARCH_PASSAGE_CHARS characters, a third of the room
/// before the occurrence, cut at word boundaries, runs of white space (line breaks included) as one space; with the occurrence's
/// byte range in the passage. No occurrence: the start of the text, cut the same way, and no range.
fn passage(index: &Index, text: &str, typed: &[(String, bool)]) -> (String, Vec<TextRange>) {
    use crate::limits::SEARCH_PASSAGE_CHARS;
    let first = occurrences(index, text, typed, 1).into_iter().next();
    let (os, oe) = first.unwrap_or((0, 0));
    let lead = SEARCH_PASSAGE_CHARS.saturating_sub(text[os..oe].chars().count()) / 3;
    let mut start = if lead == 0 { os } else { text[..os].char_indices().rev().take(lead).last().map(|(i, _)| i).unwrap_or(os) };
    if start > 0 && !text[..start].ends_with(char::is_whitespace) {
        if let Some((p, c)) = text[start..os].char_indices().find(|(_, c)| c.is_whitespace()) {
            start += p + c.len_utf8();
        }
    }
    let mut end = text[start..].char_indices().nth(SEARCH_PASSAGE_CHARS).map(|(i, _)| start + i).unwrap_or(text.len()).max(oe);
    if end < text.len() && !text[end..].starts_with(char::is_whitespace) {
        if let Some((p, _)) = text[oe..end].char_indices().rev().find(|(_, c)| c.is_whitespace()) {
            end = oe + p;
        }
    }
    let mut out = String::with_capacity(end - start);
    let (mut ns, mut ne) = (None, None);
    let mut gap = false;
    for (i, c) in text[start..end].char_indices() {
        let at = start + i;
        if at == oe {
            ne = Some(out.len());
        }
        if c.is_whitespace() {
            gap = true;
            continue;
        }
        if gap && !out.is_empty() {
            out.push(' ');
        }
        gap = false;
        if at == os {
            ns = Some(out.len());
        }
        out.push(c);
    }
    let found = match (first, ns) {
        (Some(_), Some(a)) => vec![TextRange { start: a as u32, end: ne.unwrap_or(out.len()).min(out.len()) as u32 }],
        _ => Vec::new(),
    };
    (out, found)
}

/// Where a hit matched, its attachment's name, and its snippet (the passage). The subject first, then the sender, then an
/// attachment's name, then an attachment's text, else the text.
fn evidence(index: &Index, typed: &[(String, bool)], facts: &HitFacts) -> (MatchedIn, Option<String>, String) {
    let (snippet, text_ranges) = passage(index, &facts.body, typed);
    let subject = occurrences(index, &facts.subject, typed, usize::MAX);
    if !subject.is_empty() {
        return (MatchedIn::SubjectWords { ranges: ranges(subject) }, None, snippet);
    }
    let (name, address) = (occurrences(index, &facts.sender_name, typed, usize::MAX), occurrences(index, &facts.from_address, typed, usize::MAX));
    if !name.is_empty() || !address.is_empty() {
        return (MatchedIn::SenderWords { name_ranges: ranges(name), address_ranges: ranges(address) }, None, snippet);
    }
    if let Some((file, _)) = facts.attachments.iter().find(|(file, _)| !occurrences(index, file, typed, 1).is_empty()) {
        return (MatchedIn::AttachmentName, Some(file.clone()), snippet);
    }
    if let Some((file, _)) = facts.attachments.iter().find(|(_, text)| !occurrences(index, text, typed, 1).is_empty()) {
        return (MatchedIn::AttachmentContent, Some(file.clone()), snippet);
    }
    // the body, or the recipients (no letters to show there: the passage is the start of the text)
    (MatchedIn::TextWords { ranges: text_ranges }, None, snippet)
}

/// The owner's search on the word index: up to `limit` hits in the order of SPEC A149 (module note above), each with where it
/// matched and its passage. Empty when a typed word is held by no message. `facts_of(id, with_text)` reads a hit's message (its
/// text only when a typed word was found in the text); a hit whose message is gone is dropped. When more than `limit` mails match,
/// the kept ones are the best ranks, ties by the index's own order (the newest indexed first), and the caller sorts them by date.
pub fn run_ranked(index: &Index, fields: &Fields, reader: &IndexReader, text: &str, limit: usize, facts_of: &dyn Fn(&str, bool) -> Option<HitFacts>) -> Result<Vec<SearchHit>, Error> {
    if text.trim().is_empty() || limit == 0 {
        return Ok(Vec::new());
    }
    let searcher = reader.searcher();
    let (phrases, bare) = split_phrases(text);
    let mut typed: Vec<(String, bool)> = Vec::new();
    let mut matched: Option<Found> = None;
    for (word, _, _) in words_of(index, &bare) {
        if typed.iter().any(|(w, _)| *w == word) {
            continue;
        }
        let forms = forms_of(&searcher, fields, &word)?;
        let found = if forms.is_empty() { Found::new() } else { docs_of(&searcher, fields, &forms)? };
        if found.is_empty() {
            // a typed word no message holds: the conjunction matches nothing
            return Ok(Vec::new());
        }
        typed.push((word.clone(), is_wide(&word)));
        meet(found, &mut matched);
    }
    for phrase in &phrases {
        let found = docs_of_phrase(index, fields, &searcher, phrase)?;
        if found.is_empty() {
            return Ok(Vec::new());
        }
        for (w, _, _) in words_of(index, phrase) {
            if !typed.iter().any(|(t, _)| *t == w) {
                typed.push((w, false));
            }
        }
        meet(found, &mut matched);
    }
    let Some(matched) = matched else { return Ok(Vec::new()) };
    let mut ranked: Vec<((u32, u32), ((u8, u8), u8))> = matched.into_iter().collect();
    ranked.sort_by(|a, b| (a.1).0.cmp(&(b.1).0).then(b.0.cmp(&a.0)));
    ranked.truncate(limit);
    let body_bit: u8 = 1u8 << fields.searchable().iter().position(|f| *f == fields.body).unwrap_or(0);
    let mut hits = Vec::with_capacity(ranked.len());
    for ((seg, doc), ((tier, place), bits)) in ranked {
        let stored: tantivy::TantivyDocument = searcher.doc(tantivy::DocAddress::new(seg, doc))?;
        let Some(id) = crate::engine::text_of(&stored, fields.id) else { continue };
        let Some(facts) = facts_of(&id, bits & body_bit != 0) else { continue };
        let (matched_in, attachment_name, snippet) = evidence(index, &typed, &facts);
        let score = f64::from(6 - (i32::from(tier) * 2 + i32::from(place)));
        hits.push(SearchHit { message_id: id, score, snippet, matched_in, attachment_name, attachment_note: None });
    }
    Ok(hits)
}
