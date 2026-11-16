//! The tokeniser: a message becomes a set of distinct tokens, each counted once per message (Graham's and SpamBayes'
//! rule). Subject words, the sender's domain and display-name words, link hosts and body words are kept apart by a
//! prefix, so "free" in the subject is not the same evidence as "free" in the body. Mail in any script is read: a
//! run of Han, kana or Hangul characters has no spaces, so it is cut into overlapping pairs.
use std::collections::HashSet;

use crate::domain::{domain_of, head, host_of_url};
use crate::links::extract_links;
use crate::message::Message;
use crate::tuning::{BODY_CHARS_MAX, FROMNAME_CHARS_MAX, LINKS_SCANNED_MAX, MAX_TOKENS_PER_MESSAGE, SUBJECT_CHARS_MAX, WORD_MAX_CHARS, WORD_MIN_CHARS};

#[derive(Default)]
struct TokenSet {
    seen: HashSet<String>,
    order: Vec<String>,
}

impl TokenSet {
    fn add(&mut self, token: String) {
        if self.order.len() >= MAX_TOKENS_PER_MESSAGE {
            return;
        }
        if self.seen.insert(token.clone()) {
            self.order.push(token);
        }
    }
}

/// The distinct tokens of a message, in the order met (subject, sender, links, then body), at most `MAX_TOKENS_PER_MESSAGE`.
pub fn tokenise(msg: &Message) -> Vec<String> {
    let mut set = TokenSet::default();
    for w in words(head(&msg.subject, SUBJECT_CHARS_MAX)) {
        set.add(format!("subject:{w}"));
    }
    if let Some(d) = domain_of(&msg.from_email) {
        set.add(format!("from:{d}"));
    }
    if let Some(n) = &msg.from_name {
        for w in words(head(n, FROMNAME_CHARS_MAX)) {
            set.add(format!("fromname:{w}"));
        }
    }
    if let Some(html) = &msg.html {
        for l in extract_links(html, LINKS_SCANNED_MAX) {
            if let Some(h) = host_of_url(&l.href) {
                set.add(format!("url:{h}"));
            }
        }
    }
    let body = head(&msg.text, BODY_CHARS_MAX);
    for piece in body.split_whitespace() {
        if piece.contains("//") {
            let clean = piece.trim_matches(|c: char| c == '<' || c == '>' || c == '(' || c == ')' || c == '"');
            if let Some(h) = host_of_url(clean) {
                set.add(format!("url:{h}"));
            }
        }
    }
    for w in words(body) {
        set.add(w);
    }
    set.order
}

fn is_cjk(c: char) -> bool {
    let u = c as u32;
    (0x3040..=0x30FF).contains(&u) || (0x3400..=0x9FFF).contains(&u) || (0xAC00..=0xD7AF).contains(&u)
}

/// Lower-case words of `text`: letters, digits, an apostrophe or a dollar sign make a word. A word of
/// `WORD_MIN_CHARS` to `WORD_MAX_CHARS` characters is a token; a longer one becomes "skip:<first letter>:<length to the ten>"
/// (SpamBayes' trick: a long word is rarely seen twice, its first letter and size still say something); a shorter one is dropped.
pub fn words(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut word = String::new();
    let mut run: Vec<char> = Vec::new();
    for c in text.chars() {
        if is_cjk(c) {
            flush_word(&mut word, &mut out);
            run.push(c);
            continue;
        }
        flush_run(&mut run, &mut out);
        if c.is_alphanumeric() || c == '\'' || c == '$' {
            for l in c.to_lowercase() {
                word.push(l);
            }
        } else {
            flush_word(&mut word, &mut out);
        }
    }
    flush_word(&mut word, &mut out);
    flush_run(&mut run, &mut out);
    out
}

fn flush_word(word: &mut String, out: &mut Vec<String>) {
    let w = word.trim_matches('\'');
    let n = w.chars().count();
    if n >= WORD_MIN_CHARS && n <= WORD_MAX_CHARS {
        out.push(w.to_string());
    } else if n > WORD_MAX_CHARS {
        let first = w.chars().next().unwrap_or('?');
        out.push(format!("skip:{}:{}", first, (n / 10) * 10));
    }
    word.clear();
}

fn flush_run(run: &mut Vec<char>, out: &mut Vec<String>) {
    if run.len() == 1 {
        out.push(run[0].to_string());
    } else {
        for pair in run.windows(2) {
            out.push(pair.iter().collect::<String>());
        }
    }
    run.clear();
}
