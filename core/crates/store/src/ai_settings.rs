//! ai-settings-core: the AI settings the owner makes in Settings, AI and agents, over migration 0001's tables `ai_provider`,
//! `agent_config` and `speech_engine`. A provider row exists only for a provider whose key was accepted: it holds the provider's
//! model ids and the Keychain reference (`service` + U+001F + `account`), never the key. An agent row holds the owner's choice of
//! provider and model. No query here takes a tenant: the store is one owner's, on one Mac.
use crate::error::StoreError;
use crate::Store;
use contracts::{AgentConfig, AgentPermission};
use rusqlite::{params, OptionalExtension};

/// One `ai_provider` row as stored.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiProviderRow {
    pub id: String,
    pub name: String,
    pub models: Vec<String>,
    pub connected: bool,
    /// `service` U+001F `account`, the only shape a credential takes in the database.
    pub keychain_ref: Option<String>,
    pub updated_at_ms: i64,
}

/// The one `speech_engine` row: `cloud_provider_id` None = on this Mac.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpeechEngineRow {
    pub cloud_provider_id: Option<String>,
    pub ready: bool,
}

/// The one spelling of an allowed action in `agent_config.allowed_json` and in the `agent_config/<id>/allowed` state value: the
/// Swift case names the fixture core prints (`"\(permission)"`).
pub fn permission_key(p: &AgentPermission) -> &'static str {
    match p {
        AgentPermission::ReadMail => "readMail",
        AgentPermission::Search => "search",
        AgentPermission::Draft => "draft",
        AgentPermission::Label => "label",
        AgentPermission::SendWithConfirmation => "sendWithConfirmation",
    }
}

pub fn permission_from_key(key: &str) -> Option<AgentPermission> {
    [AgentPermission::ReadMail, AgentPermission::Search, AgentPermission::Draft, AgentPermission::Label, AgentPermission::SendWithConfirmation]
        .into_iter()
        .find(|p| permission_key(p) == key)
}

fn json_words(words: &[String]) -> String {
    let parts: Vec<String> = words.iter().map(|w| format!("\"{}\"", w.replace('\\', "\\\\").replace('"', "\\\""))).collect();
    format!("[{}]", parts.join(","))
}

/// Reads a JSON array of strings written by `json_words` (or by SQLite's json functions). Anything else reads as empty.
fn words_from_json(text: &str) -> Vec<String> {
    let t = text.trim();
    let Some(inner) = t.strip_prefix('[').and_then(|r| r.strip_suffix(']')) else { return Vec::new() };
    let mut out = Vec::new();
    let mut cur = String::new();
    let (mut inside, mut escaped) = (false, false);
    for c in inner.chars() {
        match (inside, escaped, c) {
            (true, true, _) => { cur.push(c); escaped = false; }
            (true, false, '\\') => escaped = true,
            (true, false, '"') => { out.push(std::mem::take(&mut cur)); inside = false; }
            (true, false, _) => cur.push(c),
            (false, _, '"') => inside = true,
            _ => {}
        }
    }
    out
}

impl Store {
    /// Every provider whose key was accepted, in id order.
    pub fn ai_provider_rows(&self) -> Result<Vec<AiProviderRow>, StoreError> {
        self.read(|c| {
            let mut st = c.prepare_cached("SELECT id, name, models_json, connected, keychain_ref, updated_at_ms FROM ai_provider ORDER BY id")?;
            let rows = st.query_map([], |r| {
                Ok(AiProviderRow { id: r.get(0)?, name: r.get(1)?, models: words_from_json(&r.get::<_, String>(2)?), connected: r.get::<_, i64>(3)? == 1, keychain_ref: r.get(4)?, updated_at_ms: r.get(5)? })
            })?;
            Ok(rows.collect::<Result<Vec<_>, _>>()?)
        })
    }

    pub fn ai_provider_row(&self, id: &str) -> Result<Option<AiProviderRow>, StoreError> {
        Ok(self.ai_provider_rows()?.into_iter().find(|r| r.id == id))
    }

    /// Keeps an accepted key's provider: connected, its model ids, the Keychain reference. Replaces the row of an earlier key.
    pub fn ai_provider_connect(&self, id: String, name: String, models: Vec<String>, keychain_ref: String, now_ms: i64) -> Result<(), StoreError> {
        if !keychain_ref.contains('\u{1f}') || keychain_ref.starts_with('\u{1f}') {
            return Err(StoreError::Invalid("a provider's key reference is service U+001F account".into()));
        }
        self.write(move |ctx| {
            ctx.tx.execute(
                "INSERT INTO ai_provider (id, name, models_json, connected, keychain_ref, updated_at_ms) VALUES (?1, ?2, ?3, 1, ?4, ?5) \
                 ON CONFLICT(id) DO UPDATE SET name = excluded.name, models_json = excluded.models_json, connected = 1, keychain_ref = excluded.keychain_ref, updated_at_ms = excluded.updated_at_ms",
                params![id, name, json_words(&models), keychain_ref, now_ms],
            )?;
            Ok(())
        })
    }

    /// Forgets a provider: its row goes (a speech engine on it falls back to none, by the table's ON DELETE SET NULL). Returns
    /// the Keychain reference it held, for the caller to delete the item. An unknown provider forgets nothing.
    pub fn ai_provider_forget(&self, id: String) -> Result<Option<String>, StoreError> {
        self.write(move |ctx| {
            let held: Option<Option<String>> = ctx.tx.query_row("SELECT keychain_ref FROM ai_provider WHERE id = ?1", params![id], |r| r.get(0)).optional()?;
            ctx.tx.execute("UPDATE speech_engine SET kind = 'local', cloud_provider_id = NULL, ready = 0 WHERE cloud_provider_id = ?1", params![id])?;
            ctx.tx.execute("DELETE FROM ai_provider WHERE id = ?1", params![id])?;
            Ok(held.flatten())
        })
    }

    /// Every saved agent, in id order.
    pub fn agent_config_rows(&self) -> Result<Vec<AgentConfig>, StoreError> {
        self.read(|c| {
            let mut st = c.prepare_cached("SELECT id, name, purpose, provider_id, model, instructions, allowed_json, built_in FROM agent_config ORDER BY id")?;
            let rows = st.query_map([], |r| {
                let allowed = words_from_json(&r.get::<_, String>(6)?).iter().filter_map(|w| permission_from_key(w)).collect();
                Ok(AgentConfig { id: r.get(0)?, name: r.get(1)?, purpose: r.get(2)?, provider_id: r.get(3)?, model: r.get(4)?, instructions: r.get(5)?, allowed, built_in: r.get::<_, i64>(7)? == 1 })
            })?;
            Ok(rows.collect::<Result<Vec<_>, _>>()?)
        })
    }

    /// Keeps an agent whole (a new one, or the row with its id replaced).
    pub fn agent_config_put(&self, a: AgentConfig, now_ms: i64) -> Result<(), StoreError> {
        if a.id.trim().is_empty() || a.name.trim().is_empty() {
            return Err(StoreError::Invalid("an agent is kept with its id and its name".into()));
        }
        self.write(move |ctx| {
            let allowed: Vec<String> = a.allowed.iter().map(|p| permission_key(p).to_string()).collect();
            ctx.tx.execute(
                "INSERT INTO agent_config (id, name, purpose, provider_id, model, instructions, allowed_json, built_in, updated_at_ms) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9) \
                 ON CONFLICT(id) DO UPDATE SET name = excluded.name, purpose = excluded.purpose, provider_id = excluded.provider_id, model = excluded.model, \
                 instructions = excluded.instructions, allowed_json = excluded.allowed_json, built_in = excluded.built_in, updated_at_ms = excluded.updated_at_ms",
                params![a.id, a.name, a.purpose, a.provider_id, a.model, a.instructions, json_words(&allowed), a.built_in as i64, now_ms],
            )?;
            Ok(())
        })
    }

    pub fn speech_engine_row(&self) -> Result<Option<SpeechEngineRow>, StoreError> {
        self.read(|c| {
            Ok(c.prepare_cached("SELECT cloud_provider_id, ready FROM speech_engine WHERE id = 1")?
                .query_row([], |r| Ok(SpeechEngineRow { cloud_provider_id: r.get(0)?, ready: r.get::<_, i64>(1)? == 1 }))
                .optional()?)
        })
    }

    /// Keeps the chosen speech engine: on this Mac (`cloud_provider_id` None) or a provider whose row exists.
    pub fn speech_engine_put(&self, row: SpeechEngineRow) -> Result<(), StoreError> {
        self.write(move |ctx| {
            let kind = if row.cloud_provider_id.is_some() { "cloud" } else { "local" };
            ctx.tx.execute(
                "INSERT INTO speech_engine (id, kind, cloud_provider_id, ready) VALUES (1, ?1, ?2, ?3) \
                 ON CONFLICT(id) DO UPDATE SET kind = excluded.kind, cloud_provider_id = excluded.cloud_provider_id, ready = excluded.ready",
                params![kind, row.cloud_provider_id, row.ready as i64],
            )?;
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn word_lists_round_trip_with_quotes_and_backslashes() {
        let words = vec!["claude-sonnet-5-5".to_string(), "a \"quoted\" \\ id".to_string(), String::new()];
        assert_eq!(words_from_json(&json_words(&words)), words);
        assert_eq!(words_from_json("[]"), Vec::<String>::new());
        assert_eq!(words_from_json("not json"), Vec::<String>::new());
    }

    #[test]
    fn every_permission_has_one_key_both_ways() {
        for p in [AgentPermission::ReadMail, AgentPermission::Search, AgentPermission::Draft, AgentPermission::Label, AgentPermission::SendWithConfirmation] {
            assert_eq!(permission_from_key(permission_key(&p)), Some(p));
        }
        assert_eq!(permission_from_key("read mail"), None);
    }
}
