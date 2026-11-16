//! Vectors filed under a TEXT key (an attachment's "<message_id>/<attachment_id>" plus a part suffix such as "#f3"),
//! beside the messages' numeric keys (contracts delta 10, hook). Each text key gets a slot in a reserved range of
//! message keys, so the index, its removals and its compaction treat it like a one-chunk message.
use crate::files::{io_err, write_atomic};
use contracts::CoreError;
use std::{collections::BTreeMap, fs, path::{Path, PathBuf}};

/// Message keys from here up hold text-keyed vectors; mail uses the keys below (a message key is under 2^47 in practice).
pub const TEXT_KEY_BASE: u64 = 1 << 47;
const FILE: &str = "vector-keys.tsv";

#[derive(Debug, Default)]
pub struct KeyTable {
    by_key: BTreeMap<String, u64>,
    next: u64,
}

pub fn table_path(dir: &Path) -> PathBuf { dir.join(FILE) }

impl KeyTable {
    /// Lines "<slot>\t<key>"; a missing file is an empty table.
    pub fn load(dir: &Path) -> Result<Self, CoreError> {
        let path = table_path(dir);
        if !path.exists() {
            return Ok(Self { by_key: BTreeMap::new(), next: TEXT_KEY_BASE });
        }
        let text = fs::read_to_string(&path).map_err(|e| io_err("read the key table", e))?;
        let mut t = Self { by_key: BTreeMap::new(), next: TEXT_KEY_BASE };
        for line in text.lines().filter(|l| !l.is_empty()) {
            let (slot, key) = line.split_once('\t').ok_or_else(|| io_err("read the key table", "a line has no tab"))?;
            let slot: u64 = slot.parse().map_err(|e| io_err("read the key table", e))?;
            t.next = t.next.max(slot + 1);
            t.by_key.insert(key.to_string(), slot);
        }
        Ok(t)
    }

    pub fn save(&self, dir: &Path) -> Result<(), CoreError> {
        let mut out = String::new();
        for (k, slot) in &self.by_key {
            out.push_str(&format!("{slot}\t{k}\n"));
        }
        write_atomic(&table_path(dir), out.as_bytes())
    }

    pub fn slot(&self, key: &str) -> Option<u64> { self.by_key.get(key).copied() }

    pub fn slot_or_new(&mut self, key: &str) -> u64 {
        if let Some(s) = self.by_key.get(key) {
            return *s;
        }
        let s = self.next;
        self.next += 1;
        self.by_key.insert(key.to_string(), s);
        s
    }

    pub fn key_of(&self, slot: u64) -> Option<&str> {
        self.by_key.iter().find(|(_, s)| **s == slot).map(|(k, _)| k.as_str())
    }

    /// Removes every key that starts with `prefix` and returns their slots.
    pub fn take_prefix(&mut self, prefix: &str) -> Vec<u64> {
        let keys: Vec<String> = self.by_key.keys().filter(|k| k.starts_with(prefix)).cloned().collect();
        keys.iter().filter_map(|k| self.by_key.remove(k)).collect()
    }

    pub fn slots(&self) -> Vec<u64> { self.by_key.values().copied().collect() }
}
