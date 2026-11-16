//! The trained table: per token, in how many junk and how many not-junk messages it was seen, plus the number of
//! messages learned per class. Scoring is the SpamBayes method: Robinson's smoothed per-token probability, the
//! strongest tokens kept, chi-square combining. The table is capped (rare tokens are pruned first) so it stays small.
use std::collections::HashMap;

use crate::chi2::combine;
use crate::tuning::*;

/// In how many junk and how many not-junk messages one token was seen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Counts {
    pub spam: u32,
    pub ham: u32,
}

/// The learned state. `to_bytes` / `from_bytes` keep it in the store.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Model {
    tokens: HashMap<String, Counts>,
    n_spam: u32,
    n_ham: u32,
}

/// Why a stored table could not be read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelError {
    BadMagic,
    Truncated,
    BadVarint,
    BadToken,
    TooLarge,
    TrailingBytes,
}

impl std::fmt::Display for ModelError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let words = match self {
            ModelError::BadMagic => "this is not a stored junk table",
            ModelError::Truncated => "the stored junk table ends too early",
            ModelError::BadVarint => "the stored junk table holds a number that is too long",
            ModelError::BadToken => "the stored junk table holds a word that is not text or is too long",
            ModelError::TooLarge => "the stored junk table claims more words than the cap allows",
            ModelError::TrailingBytes => "the stored junk table has bytes after its end",
        };
        write!(f, "{words}")
    }
}

impl std::error::Error for ModelError {}

const MAGIC: [u8; 4] = *b"WJB1";

impl Model {
    pub fn new() -> Model {
        Model::default()
    }

    /// Messages learned as (junk, not junk).
    pub fn trained(&self) -> (u32, u32) {
        (self.n_spam, self.n_ham)
    }

    /// Distinct tokens held.
    pub fn len(&self) -> usize {
        self.tokens.len()
    }

    pub fn is_empty(&self) -> bool {
        self.tokens.is_empty()
    }

    /// The counts of one token.
    pub fn counts(&self, token: &str) -> Option<Counts> {
        self.tokens.get(token).copied()
    }

    /// True once the model has learned enough of both classes to give an opinion.
    pub fn is_ready(&self) -> bool {
        self.n_spam >= MODEL_MIN_PER_CLASS && self.n_ham >= MODEL_MIN_PER_CLASS
    }

    /// Learns one message (its distinct tokens) as junk (`spam`) or not junk. Prunes the table when it passes its cap.
    pub fn train(&mut self, tokens: &[String], spam: bool) {
        for t in tokens {
            let c = self.tokens.entry(t.clone()).or_default();
            if spam {
                c.spam = c.spam.saturating_add(1);
            } else {
                c.ham = c.ham.saturating_add(1);
            }
        }
        if spam {
            self.n_spam = self.n_spam.saturating_add(1);
        } else {
            self.n_ham = self.n_ham.saturating_add(1);
        }
        self.prune();
    }

    /// Takes one earlier lesson back (the message was learned as `spam` before). A token pruned since is left alone.
    pub fn untrain(&mut self, tokens: &[String], spam: bool) {
        for t in tokens {
            let gone = match self.tokens.get_mut(t.as_str()) {
                Some(c) => {
                    if spam {
                        c.spam = c.spam.saturating_sub(1);
                    } else {
                        c.ham = c.ham.saturating_sub(1);
                    }
                    c.spam == 0 && c.ham == 0
                }
                None => false,
            };
            if gone {
                self.tokens.remove(t.as_str());
            }
        }
        if spam {
            self.n_spam = self.n_spam.saturating_sub(1);
        } else {
            self.n_ham = self.n_ham.saturating_sub(1);
        }
    }

    /// Robinson's probability that a message holding this token is junk: the token's own spam share, pulled toward
    /// `UNKNOWN_WORD_PROB` by the prior (worth `UNKNOWN_WORD_STRENGTH` observations), so a token seen once is never decisive.
    fn token_prob(&self, c: Counts) -> f64 {
        let n = c.spam as f64 + c.ham as f64;
        if n == 0.0 {
            return UNKNOWN_WORD_PROB;
        }
        let spam_ratio = c.spam as f64 / self.n_spam.max(1) as f64;
        let ham_ratio = c.ham as f64 / self.n_ham.max(1) as f64;
        let raw = spam_ratio / (spam_ratio + ham_ratio);
        (UNKNOWN_WORD_STRENGTH * UNKNOWN_WORD_PROB + n * raw) / (UNKNOWN_WORD_STRENGTH + n)
    }

    /// The probability the model gives one token; None when the token is unknown.
    pub fn probability_of(&self, token: &str) -> Option<f64> {
        self.tokens.get(token).map(|c| self.token_prob(*c))
    }

    /// The model's score for a message's tokens, in [0, 1]. None while the model has not learned enough
    /// (`is_ready`); 0.5 when it is ready but knows none of the tokens well enough to say anything.
    pub fn score(&self, tokens: &[String]) -> Option<f64> {
        if !self.is_ready() {
            return None;
        }
        let mut strong: Vec<(f64, f64)> = Vec::new();
        for t in tokens {
            if let Some(c) = self.tokens.get(t.as_str()) {
                let f = self.token_prob(*c);
                let strength = (f - 0.5).abs();
                if strength >= MIN_PROB_STRENGTH {
                    strong.push((strength, f));
                }
            }
        }
        if strong.is_empty() {
            return Some(0.5);
        }
        strong.sort_by(|a, b| b.0.total_cmp(&a.0));
        strong.truncate(MAX_DISCRIMINATORS);
        let probs: Vec<f64> = strong.iter().map(|x| x.1).collect();
        Some(combine(&probs))
    }

    /// Past `TOKEN_TABLE_MAX` tokens, drops the rarest (fewest messages) down to `TOKEN_TABLE_PRUNE_TO`; ties go in token order.
    fn prune(&mut self) {
        if self.tokens.len() <= TOKEN_TABLE_MAX {
            return;
        }
        let drop = self.tokens.len() - TOKEN_TABLE_PRUNE_TO;
        let mut entries: Vec<(String, u32)> = self.tokens.iter().map(|(k, c)| (k.clone(), c.spam.saturating_add(c.ham))).collect();
        entries.sort_by(|a, b| a.1.cmp(&b.1).then_with(|| a.0.cmp(&b.0)));
        for (token, _) in entries.into_iter().take(drop) {
            self.tokens.remove(&token);
        }
    }

    /// The table as bytes for the store: magic "WJB1", messages learned (junk, not junk) and token count as u32 little
    /// endian, then per token in sorted order: its length, its UTF-8 bytes, its junk count, its not-junk count (LEB128 varints).
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut keys: Vec<&String> = self.tokens.keys().collect();
        keys.sort();
        let mut out: Vec<u8> = Vec::with_capacity(16 + keys.len() * 16);
        out.extend_from_slice(&MAGIC);
        out.extend_from_slice(&self.n_spam.to_le_bytes());
        out.extend_from_slice(&self.n_ham.to_le_bytes());
        out.extend_from_slice(&(keys.len() as u32).to_le_bytes());
        for k in keys {
            let c = self.tokens[k];
            put_varint(&mut out, k.len() as u32);
            out.extend_from_slice(k.as_bytes());
            put_varint(&mut out, c.spam); put_varint(&mut out, c.ham);
        }
        out
    }

    /// Reads what `to_bytes` wrote. Any damage is an error; nothing is guessed.
    pub fn from_bytes(bytes: &[u8]) -> Result<Model, ModelError> {
        let mut r = Reader { b: bytes, i: 0 };
        if r.take(4)? != &MAGIC[..] {
            return Err(ModelError::BadMagic);
        }
        let n_spam = r.u32()?;
        let n_ham = r.u32()?;
        let count = r.u32()? as usize;
        if count > TOKEN_TABLE_MAX * 2 {
            return Err(ModelError::TooLarge);
        }
        let mut tokens: HashMap<String, Counts> = HashMap::with_capacity(count);
        for _ in 0..count {
            let len = r.varint()? as usize;
            if len > TOKEN_BYTES_MAX {
                return Err(ModelError::BadToken);
            }
            let raw = r.take(len)?;
            let token = std::str::from_utf8(raw).map_err(|_| ModelError::BadToken)?.to_string();
            let spam = r.varint()?;
            let ham = r.varint()?;
            tokens.insert(token, Counts { spam, ham });
        }
        if r.i != bytes.len() {
            return Err(ModelError::TrailingBytes);
        }
        Ok(Model { tokens, n_spam, n_ham })
    }
}

fn put_varint(out: &mut Vec<u8>, mut v: u32) {
    loop {
        let b = (v & 0x7f) as u8;
        v >>= 7;
        if v == 0 {
            out.push(b);
            return;
        }
        out.push(b | 0x80);
    }
}

struct Reader<'a> {
    b: &'a [u8],
    i: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], ModelError> {
        let end = self.i.checked_add(n).ok_or(ModelError::Truncated)?;
        if end > self.b.len() {
            return Err(ModelError::Truncated);
        }
        let s = &self.b[self.i..end];
        self.i = end;
        Ok(s)
    }

    fn u32(&mut self) -> Result<u32, ModelError> {
        let s = self.take(4)?;
        Ok(u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
    }

    fn varint(&mut self) -> Result<u32, ModelError> {
        let mut v: u32 = 0;
        for shift in (0..35u32).step_by(7) {
            let byte = self.take(1)?[0];
            v |= ((byte & 0x7f) as u32) << shift;
            if byte & 0x80 == 0 {
                return Ok(v);
            }
        }
        Err(ModelError::BadVarint)
    }
}
