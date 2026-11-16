//! The index: a memory-mapped i8 base for the bulk, an in-memory delta for new mail, tombstones for removals.
use crate::chunker::{chunk_text, Tokenizer, WhitespaceTokenizer};
use crate::constants::{CHUNK_OVERLAP, CHUNK_TOKENS, EMBED_DIM};
use crate::files::*;
use crate::keys::{KeyTable, TEXT_KEY_BASE};
use contracts::{CoreError, Embedder};
use std::{collections::HashSet, fs, path::{Path, PathBuf}, sync::{Arc, Mutex, RwLock}};
use usearch::{ffi::{IndexOptions, MetricKind, ScalarKind}, Index};

/// Key layout: the message key above the low 16 bits, the chunk number in them.
pub const MAX_CHUNKS_PER_MESSAGE: usize = 1 << 16;
const MAX_MESSAGE_KEY: u64 = (1 << 48) - 1;

/// A hit on a text-keyed vector (an attachment part): the key it was filed under and its cosine (contracts delta 10).
#[derive(Debug, Clone, PartialEq)]
pub struct KeyHit {
    pub key: String,
    pub score: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Hit {
    pub message_key: u64,
    pub chunk: u16,
    /// Cosine similarity, 1.0 for the same direction.
    pub score: f32,
}

/// Search effort. The library default (64) finds a chunk's own vector 53 times in 200 at a million chunks of random
/// vectors; 512 finds it 200 of 200 at 200,000 (tests/recall_probe.rs). Needs a docs/CONSTANTS.md row (root-owned).
const SEARCH_EFFORT: usize = 512;

fn options() -> IndexOptions {
    // Zero connectivity and expansion_add mean the library defaults.
    IndexOptions { dimensions: EMBED_DIM, metric: MetricKind::Cos, quantization: ScalarKind::I8, connectivity: 0, expansion_add: 0, expansion_search: SEARCH_EFFORT, multi: false }
}
fn map_base(path: &Path) -> Result<Index, CoreError> {
    let base = Index::restore_view(path_str(path)?).map_err(|e| io_err("map the base", e))?;
    base.change_expansion_search(SEARCH_EFFORT);
    Ok(base)
}
fn new_index() -> Result<Index, CoreError> { Index::new(&options()).map_err(|e| io_err("create an index", e)) }
fn chunk_key(message: u64, chunk: usize) -> u64 { (message << 16) | chunk as u64 }
fn path_str(p: &Path) -> Result<&str, CoreError> { p.to_str().ok_or_else(|| io_err("use a path", "not valid text")) }

fn invalid(field: &str, words: impl Into<String>) -> CoreError { CoreError::Invalid { field: field.into(), reason_words: words.into() } }

/// Chunks a body, asks the embedder for the vectors and returns unit-length vectors under their keys.
fn embed_message(embedder: &dyn Embedder, tokenizer: &dyn Tokenizer, message: u64, title: Option<&str>, body: &str) -> Result<Vec<(u64, Vec<f32>)>, CoreError> {
    if message > MAX_MESSAGE_KEY {
        return Err(invalid("message_key", "the key does not fit in 48 bits"));
    }
    let chunks = chunk_text(body, tokenizer, CHUNK_TOKENS, CHUNK_OVERLAP)?;
    if chunks.len() > MAX_CHUNKS_PER_MESSAGE {
        return Err(invalid("body", "the message has more chunks than the key layout can number"));
    }
    if chunks.is_empty() {
        return Ok(Vec::new());
    }
    // delta 12: the subject travels in the title slot; a missing or blank title is the model's "none"
    let title = title.map(str::trim).filter(|t| !t.is_empty()).map(str::to_string);
    let vectors = embedder.embed_titled(title, chunks.iter().map(|c| c.text.clone()).collect())?;
    if vectors.len() != chunks.len() {
        return Err(invalid("embedder", "it returned a different number of vectors than chunks"));
    }
    chunks.iter().zip(vectors).map(|(c, v)| Ok((chunk_key(message, c.index), unit(v)?))).collect()
}

fn check_dimension(embedder: &dyn Embedder) -> Result<(), CoreError> {
    if embedder.dimension() as usize != EMBED_DIM {
        return Err(invalid("embedder", format!("it produces {} numbers per chunk, the index holds {EMBED_DIM}", embedder.dimension())));
    }
    Ok(())
}

fn unit(mut v: Vec<f32>) -> Result<Vec<f32>, CoreError> {
    if v.len() != EMBED_DIM {
        return Err(invalid("embedder", format!("a vector has {} numbers, the index holds {EMBED_DIM}", v.len())));
    }
    let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if !norm.is_finite() || norm == 0.0 {
        return Err(invalid("embedder", "a vector is empty or not a number"));
    }
    v.iter_mut().for_each(|x| *x /= norm);
    Ok(v)
}

fn search_one(index: &Index, query: &[f32], k: usize, skip: Option<&HashSet<u64>>) -> Result<Vec<Hit>, CoreError> {
    if index.size() == 0 {
        return Ok(Vec::new());
    }
    // Over-ask when tombstones may hide hits.
    let ask = k + skip.map_or(0, |s| s.len().min(k * 4));
    let m = index.search::<f32>(query, ask).map_err(|e| io_err("search", e))?;
    Ok(m.keys.iter().zip(&m.distances)
        .map(|(key, d)| Hit { message_key: key >> 16, chunk: (key & 0xFFFF) as u16, score: 1.0 - d })
        .filter(|h| skip.is_none_or(|s| !s.contains(&h.message_key)))
        .collect())
}

/// Fills a new base in parallel (backfill and compaction), then opens it mapped.
pub struct BaseBuilder {
    index: Index,
    embedder: Arc<dyn Embedder>,
    tokenizer: Arc<dyn Tokenizer>,
}

impl BaseBuilder {
    /// `capacity_chunks` is reserved up front; adding past it fails.
    pub fn new(embedder: Arc<dyn Embedder>, capacity_chunks: usize) -> Result<Self, CoreError> {
        check_dimension(embedder.as_ref())?;
        let index = new_index()?;
        index.reserve(capacity_chunks).map_err(|e| io_err("reserve room", e))?;
        Ok(Self { index, embedder, tokenizer: Arc::new(WhitespaceTokenizer) })
    }

    /// Safe to call from several threads.
    pub fn add_message(&self, message_key: u64, body: &str) -> Result<usize, CoreError> {
        self.add_message_titled(message_key, None, body)
    }

    /// `add_message` with the mail's subject in the embedder's title slot (delta 12).
    pub fn add_message_titled(&self, message_key: u64, title: Option<&str>, body: &str) -> Result<usize, CoreError> {
        let entries = embed_message(self.embedder.as_ref(), self.tokenizer.as_ref(), message_key, title, body)?;
        for (key, v) in &entries {
            self.index.add(*key, v).map_err(|e| io_err("add a chunk", e))?;
        }
        Ok(entries.len())
    }

    /// Writes generation `generation` into `dir` and returns the index that maps it.
    pub fn finish(self, dir: &Path, generation: u64) -> Result<VectorIndex, CoreError> {
        fs::create_dir_all(dir).map_err(|e| io_err("create its folder", e))?;
        let tmp = base_path(dir, generation).with_extension("tmp");
        self.index.save(path_str(&tmp)?).map_err(|e| io_err("save the base", e))?;
        fs::rename(&tmp, base_path(dir, generation)).map_err(|e| io_err("publish the base", e))?;
        VectorIndex::open(dir, self.embedder)
    }
}

pub struct VectorIndex {
    dir: PathBuf,
    generation: RwLock<u64>,
    base: RwLock<Option<Index>>,
    delta: RwLock<Index>,
    dead: RwLock<HashSet<u64>>,
    write: Mutex<()>,
    embedder: Arc<dyn Embedder>,
    tokenizer: Arc<dyn Tokenizer>,
    keys: RwLock<KeyTable>,
}

impl VectorIndex {
    /// Opens the newest generation in `dir` (creating the folder). The base is mapped, never read in.
    pub fn open(dir: &Path, embedder: Arc<dyn Embedder>) -> Result<Self, CoreError> {
        check_dimension(embedder.as_ref())?;
        fs::create_dir_all(dir).map_err(|e| io_err("create its folder", e))?;
        let latest = latest_generation(dir)?;
        let generation = latest.unwrap_or(0);
        sweep_other_generations(dir, generation)?;
        let base = match latest {
            Some(g) => Some(map_base(&base_path(dir, g))?),
            None => None,
        };
        let delta = new_index()?;
        if delta_path(dir, generation).exists() {
            delta.load(path_str(&delta_path(dir, generation))?).map_err(|e| io_err("load the recent mail", e))?;
        }
        let dead = read_dead(&dead_path(dir, generation))?;
        let keys = KeyTable::load(dir)?;
        Ok(Self { dir: dir.to_path_buf(), generation: RwLock::new(generation), base: RwLock::new(base), delta: RwLock::new(delta), dead: RwLock::new(dead), write: Mutex::new(()), embedder, tokenizer: Arc::new(WhitespaceTokenizer), keys: RwLock::new(keys) })
    }

    /// Replaces the default whitespace tokenizer (use the embedding model's own).
    pub fn with_tokenizer(mut self, tokenizer: Arc<dyn Tokenizer>) -> Self {
        self.tokenizer = tokenizer;
        self
    }

    pub fn is_mapped(&self) -> bool { self.base.read().expect("lock").is_some() }

    /// Chunks in the base (minus none removed) plus the delta; removed messages still count until compaction.
    pub fn chunk_count(&self) -> usize {
        self.base.read().expect("lock").as_ref().map_or(0, |b| b.size()) + self.delta.read().expect("lock").size()
    }

    /// Indexes one message, replacing any earlier version of it. Returns its chunk count.
    pub fn index_message(&self, message_key: u64, body: &str) -> Result<usize, CoreError> {
        self.index_message_titled(message_key, None, body)
    }

    /// `index_message` with the mail's subject in the embedder's title slot, so the index stops being "title: none"
    /// (delta 12). A missing or blank title is "none".
    pub fn index_message_titled(&self, message_key: u64, title: Option<&str>, body: &str) -> Result<usize, CoreError> {
        let entries = embed_message(self.embedder.as_ref(), self.tokenizer.as_ref(), message_key, title, body)?;
        let _w = self.write.lock().expect("lock");
        self.remove_locked(message_key)?;
        let delta = self.delta.write().expect("lock");
        let wanted = delta.size() + entries.len();
        if wanted > delta.capacity() {
            delta.reserve(wanted.next_power_of_two()).map_err(|e| io_err("reserve room", e))?;
        }
        for (key, v) in &entries {
            delta.add(*key, v).map_err(|e| io_err("add a chunk", e))?;
        }
        Ok(entries.len())
    }

    /// sync-full-r1: true when the index holds this message (its first chunk, in the delta or in the base and not removed). A
    /// message with no text has no chunk and is never held.
    pub fn contains_message(&self, message_key: u64) -> bool {
        let key = chunk_key(message_key, 0);
        if self.delta.read().expect("lock").contains(key) {
            return true;
        }
        self.base.read().expect("lock").as_ref().is_some_and(|b| b.contains(key)) && !self.dead.read().expect("lock").contains(&message_key)
    }

    /// sync-full-r1 round 2: files one vector the caller already has under `key` as a one-chunk entry (the real core's key is a
    /// chunk's own id, contracts `chunk_vector_key`), replacing what was filed under it. Normalised like a chunk; `remove_message(key)`
    /// removes it, a hit carries it as `message_key`.
    pub fn index_vector(&self, key: u64, vector: &[f32]) -> Result<(), CoreError> {
        if key > MAX_MESSAGE_KEY || key >= TEXT_KEY_BASE {
            return Err(invalid("key", "the key is past the range of filed entries"));
        }
        let v = unit(vector.to_vec())?;
        let _w = self.write.lock().expect("lock");
        // an older copy in the base stays hidden (dead); the delta's copy is the one searched and kept by `compact`
        self.remove_locked(key)?;
        let delta = self.delta.write().expect("lock");
        if delta.size() + 1 > delta.capacity() {
            delta.reserve((delta.size() + 1).next_power_of_two()).map_err(|e| io_err("reserve room", e))?;
        }
        delta.add(chunk_key(key, 0), &v).map_err(|e| io_err("add a vector", e))
    }

    /// sync-full-r1: chunks held in memory (the delta), which `compact` folds into the mapped base.
    pub fn delta_chunks(&self) -> usize { self.delta.read().expect("lock").size() }

    pub fn remove_message(&self, message_key: u64) -> Result<(), CoreError> {
        let _w = self.write.lock().expect("lock");
        self.remove_locked(message_key)
    }

    fn remove_locked(&self, message_key: u64) -> Result<(), CoreError> {
        let delta = self.delta.write().expect("lock");
        let mut chunk = 0;
        while delta.contains(chunk_key(message_key, chunk)) {
            delta.remove(chunk_key(message_key, chunk)).map_err(|e| io_err("remove a chunk", e))?;
            chunk += 1;
        }
        if self.base.read().expect("lock").as_ref().is_some_and(|b| b.contains(chunk_key(message_key, 0))) {
            self.dead.write().expect("lock").insert(message_key);
        }
        Ok(())
    }

    /// Files one vector the caller already has (an attachment part's embedding) under a text key, replacing any vector
    /// filed under that key before (contracts delta 10). Normalised like a chunk.
    pub fn add_vector(&self, key: &str, vector: &[f32]) -> Result<(), CoreError> {
        if key.is_empty() {
            return Err(invalid("key", "a vector needs a key"));
        }
        let v = unit(vector.to_vec())?;
        let _w = self.write.lock().expect("lock");
        let slot = self.keys.write().expect("lock").slot_or_new(key);
        self.remove_locked(slot)?;
        self.dead.write().expect("lock").remove(&slot);
        let delta = self.delta.write().expect("lock");
        if delta.size() + 1 > delta.capacity() {
            delta.reserve((delta.size() + 1).next_power_of_two()).map_err(|e| io_err("reserve room", e))?;
        }
        delta.add(chunk_key(slot, 0), &v).map_err(|e| io_err("add a vector", e))
    }

    /// Removes every vector whose key starts with `prefix` (an attachment's base key removes all its parts).
    /// Returns how many keys went.
    pub fn remove_prefix(&self, prefix: &str) -> Result<usize, CoreError> {
        let _w = self.write.lock().expect("lock");
        let slots = self.keys.write().expect("lock").take_prefix(prefix);
        for s in &slots {
            self.remove_locked(*s)?;
        }
        Ok(slots.len())
    }

    /// The `k` closest text-keyed vectors to the query, best first, with their keys. Mail chunks are not returned here.
    pub fn search_keys(&self, query: &str, k: usize) -> Result<Vec<KeyHit>, CoreError> {
        let vector = unit(self.embedder.embed_query(query.to_string())?)?;
        // ask wider: mail chunks share the index and are filtered out below
        let hits = self.search_vector(&vector, k.saturating_mul(8).max(k))?;
        let keys = self.keys.read().expect("lock");
        let mut out: Vec<KeyHit> = hits
            .iter()
            .filter(|h| h.message_key >= TEXT_KEY_BASE)
            .filter_map(|h| keys.key_of(h.message_key).map(|key| KeyHit { key: key.to_string(), score: h.score }))
            .collect();
        out.truncate(k);
        Ok(out)
    }

    /// The text key a hit's message key stands for, when it is a text-keyed vector.
    pub fn key_of(&self, hit: &Hit) -> Option<String> {
        (hit.message_key >= TEXT_KEY_BASE).then(|| self.keys.read().expect("lock").key_of(hit.message_key).map(str::to_string)).flatten()
    }

    /// The `k` closest chunks to the query text, best first.
    pub fn search(&self, query: &str, k: usize) -> Result<Vec<Hit>, CoreError> {
        // The query form (model's QUERY prefix), not the document form the chunks were stored with (contracts delta 8).
        let vector = self.embedder.embed_query(query.to_string())?;
        self.search_vector(&unit(vector)?, k)
    }

    pub fn search_vector(&self, query: &[f32], k: usize) -> Result<Vec<Hit>, CoreError> {
        let dead = self.dead.read().expect("lock");
        let mut hits = Vec::new();
        if let Some(base) = self.base.read().expect("lock").as_ref() {
            hits.extend(search_one(base, query, k, Some(&dead))?);
        }
        hits.extend(search_one(&self.delta.read().expect("lock"), query, k, None)?);
        hits.sort_by(|a, b| b.score.total_cmp(&a.score));
        hits.truncate(k);
        Ok(hits)
    }

    /// search-forms (SPEC A141): the exact cosine of `query` to the vector filed under each key (`index_vector`'s one-chunk entries,
    /// the real core's chunk ids), in the order given; None for a key the index does not hold (never filed, removed). Every key is
    /// read, none is cut by a score, so a caller can rank EVERY message down to no likeness; the graph is not walked, so the answer
    /// is the same at each call over the same index (a relaunch included). A query of the wrong length or no length answers all None.
    pub fn cosines(&self, query: &[f32], keys: &[u64]) -> Vec<Option<f32>> {
        let qn = query.iter().map(|x| x * x).sum::<f32>().sqrt();
        if query.len() != EMBED_DIM || !qn.is_finite() || qn == 0.0 {
            return vec![None; keys.len()];
        }
        let base = self.base.read().expect("lock");
        let delta = self.delta.read().expect("lock");
        let dead = self.dead.read().expect("lock");
        let mut row = vec![0i8; EMBED_DIM];
        keys.iter().map(|key| {
            let k = chunk_key(*key, 0);
            let source = if delta.contains(k) { Some(&*delta) } else { base.as_ref().filter(|b| !dead.contains(key) && b.contains(k)) };
            let source = source?;
            source.get::<i8>(k, &mut row).ok().filter(|n| *n > 0)?;
            let (dot, rn) = row.iter().zip(query).fold((0f32, 0f32), |(d, n), (r, q)| { let r = f32::from(*r); (d + r * q, n + r * r) });
            let rn = rn.sqrt();
            (rn > 0.0).then(|| dot / (rn * qn))
        }).collect()
    }

    /// Makes the delta and the removals survive a relaunch. The removal list goes first: a crash between the two
    /// files can lose a re-indexed message until it is indexed again, never show its stale version.
    pub fn commit(&self) -> Result<(), CoreError> {
        let _w = self.write.lock().expect("lock");
        let g = *self.generation.read().expect("lock");
        write_dead(&dead_path(&self.dir, g), &self.dead.read().expect("lock"))?;
        self.keys.read().expect("lock").save(&self.dir)?;
        let tmp = delta_path(&self.dir, g).with_extension("tmp");
        self.delta.read().expect("lock").save(path_str(&tmp)?).map_err(|e| io_err("save the recent mail", e))?;
        fs::rename(&tmp, delta_path(&self.dir, g)).map_err(|e| io_err("publish the recent mail", e))
    }

    /// Folds the delta and the removals into a new mapped base. `live_messages` is every message that should remain.
    pub fn compact(&self, live_messages: impl IntoIterator<Item = u64>) -> Result<(), CoreError> {
        let _w = self.write.lock().expect("lock");
        let new = new_index()?;
        let old_base = self.base.read().expect("lock");
        let delta = self.delta.read().expect("lock");
        let dead = self.dead.read().expect("lock");
        new.reserve((old_base.as_ref().map_or(0, |b| b.size()) + delta.size()).max(1)).map_err(|e| io_err("reserve room", e))?;
        let mut row = vec![0i8; EMBED_DIM];
        // text-keyed vectors stay as long as their key is in the table (remove_prefix takes them out)
        let text_slots = self.keys.read().expect("lock").slots();
        for message in live_messages.into_iter().filter(|m| *m < TEXT_KEY_BASE).chain(text_slots) {
            let mut chunk = 0;
            loop {
                let key = chunk_key(message, chunk);
                let source = if delta.contains(key) { Some(&*delta) } else { old_base.as_ref().filter(|b| !dead.contains(&message) && b.contains(key)) };
                let Some(source) = source else { break };
                source.get::<i8>(key, &mut row).map_err(|e| io_err("read a stored chunk", e))?;
                new.add(key, &row[..]).map_err(|e| io_err("add a chunk", e))?;
                chunk += 1;
            }
        }
        let next = *self.generation.read().expect("lock") + 1;
        let tmp = base_path(&self.dir, next).with_extension("tmp");
        new.save(path_str(&tmp)?).map_err(|e| io_err("save the base", e))?;
        fs::rename(&tmp, base_path(&self.dir, next)).map_err(|e| io_err("publish the base", e))?;
        drop((old_base, delta, dead));
        *self.base.write().expect("lock") = Some(map_base(&base_path(&self.dir, next))?);
        *self.delta.write().expect("lock") = new_index()?;
        self.dead.write().expect("lock").clear();
        *self.generation.write().expect("lock") = next;
        sweep_other_generations(&self.dir, next)
    }
}
