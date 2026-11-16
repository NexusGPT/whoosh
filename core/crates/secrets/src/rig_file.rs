//! The hidden test copy's keyring (rig-core, docs/lanes/realcore-firstrun.testplan.md R5): the same ports as `MemoryKeychain`, with
//! every item also written to ONE file in the run's own folder, so a relaunch of the same test copy on the same folder keeps the key a
//! check pasted. TEST DATA ONLY: it refuses any namespace that is not the rig's (`io.nexus.whoosh.rig.`), so the owner's secrets can
//! never be kept here, and it never touches the Keychain. The file is written only once an item exists (an untouched run leaves no file).
//!
//! File shape: one JSON object, `{"items": [{"service": .., "account": .., "secret_hex": ..}, ..]}`, written whole through a temporary
//! file and a rename (a crash leaves the old file or the new one, never half of one).
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use contracts::{CoreError, KeychainRef};

use crate::{DeleteSecret, Namespace, ReadSecret, Secret, SecretKind, StoreSecret};

pub struct RigFileKeychain {
    namespace: Namespace,
    path: PathBuf,
    items: Mutex<BTreeMap<(String, String), Secret>>,
}

fn invalid(words: String) -> CoreError { CoreError::Invalid { field: "rig_keys".into(), reason_words: words } }

fn hex(bytes: &[u8]) -> String { bytes.iter().map(|b| format!("{b:02x}")).collect() }

fn unhex(s: &str) -> Option<Vec<u8>> {
    if s.len() % 2 != 0 { return None; }
    (0..s.len()).step_by(2).map(|i| u8::from_str_radix(s.get(i..i + 2)?, 16).ok()).collect()
}

/// Service and account names are the namespace's (letters, digits and `- _ . @ + :`), so they need no escaping; a name with a quote or
/// a backslash is refused rather than written.
fn plain(s: &str) -> Result<&str, CoreError> {
    if s.contains(['"', '\\']) || s.chars().any(char::is_control) { Err(invalid(format!("{s:?} cannot be written to the test keyring"))) } else { Ok(s) }
}

/// The value of `"key": "<value>"` in one item's text.
fn field<'a>(item: &'a str, key: &str) -> Option<&'a str> {
    let rest = item.split(&format!("\"{key}\"")).nth(1)?;
    let rest = rest.trim_start().strip_prefix(':')?.trim_start().strip_prefix('"')?;
    rest.split('"').next()
}

impl RigFileKeychain {
    /// The keyring of a rig run, with the items `path` already holds (none when the file does not exist). Refused: a namespace that
    /// is not the rig's, or a file that is not one this keyring wrote.
    pub fn open(namespace: Namespace, path: impl AsRef<Path>) -> Result<Self, CoreError> {
        if !namespace.prefix().starts_with(Namespace::RIG_PREFIX) {
            return Err(invalid(format!("the test keyring keeps rig items only, not {}", namespace.prefix())));
        }
        let path = path.as_ref().to_path_buf();
        let mut items = BTreeMap::new();
        match std::fs::read_to_string(&path) {
            Ok(text) => {
                let body = text.split_once("\"items\"").map(|(_, r)| r).ok_or_else(|| invalid(format!("{} is not a test keyring file", path.display())))?;
                for item in body.split('{').skip(1) {
                    let item = item.split('}').next().unwrap_or("");
                    let (Some(service), Some(account), Some(secret)) = (field(item, "service"), field(item, "account"), field(item, "secret_hex").and_then(unhex)) else {
                        return Err(invalid(format!("{} holds an item this keyring cannot read", path.display())));
                    };
                    namespace.kind_of(&KeychainRef { service: service.into(), account: account.into() })?;
                    items.insert((service.to_string(), account.to_string()), Secret::new(secret));
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(invalid(format!("{} could not be read: {e}", path.display()))),
        }
        Ok(RigFileKeychain { namespace, path, items: Mutex::new(items) })
    }

    pub fn path(&self) -> &Path { &self.path }

    fn save(&self, items: &BTreeMap<(String, String), Secret>) -> Result<(), CoreError> {
        let mut rows = Vec::new();
        for ((service, account), secret) in items {
            rows.push(format!("{{\"service\": \"{}\", \"account\": \"{}\", \"secret_hex\": \"{}\"}}", plain(service)?, plain(account)?, hex(secret.expose())));
        }
        let text = format!("{{\"items\": [{}]}}\n", rows.join(", "));
        let tmp = self.path.with_extension("json.tmp");
        std::fs::write(&tmp, text).and_then(|_| std::fs::rename(&tmp, &self.path))
            .map_err(|e| invalid(format!("{} could not be written: {e}", self.path.display())))
    }

    fn items(&self) -> std::sync::MutexGuard<'_, BTreeMap<(String, String), Secret>> { self.items.lock().unwrap_or_else(|e| e.into_inner()) }
}

impl StoreSecret for RigFileKeychain {
    fn execute(&self, kind: SecretKind, name: &str, secret: &Secret) -> Result<KeychainRef, CoreError> {
        let r = self.namespace.reference(kind, name)?;
        let mut items = self.items();
        let old = items.insert((r.service.clone(), r.account.clone()), secret.clone());
        if let Err(e) = self.save(&items) {
            match old { Some(o) => { items.insert((r.service.clone(), r.account.clone()), o); } None => { items.remove(&(r.service.clone(), r.account.clone())); } }
            return Err(e);
        }
        Ok(r)
    }
}

impl ReadSecret for RigFileKeychain {
    fn execute(&self, r: &KeychainRef) -> Result<Secret, CoreError> {
        self.namespace.kind_of(r)?;
        self.items().get(&(r.service.clone(), r.account.clone())).cloned().ok_or_else(|| CoreError::NotFound { kind: "secret".into(), id: r.account.clone() })
    }
}

impl DeleteSecret for RigFileKeychain {
    fn execute(&self, r: &KeychainRef) -> Result<(), CoreError> {
        self.namespace.kind_of(r)?;
        let mut items = self.items();
        if items.remove(&(r.service.clone(), r.account.clone())).is_some() { self.save(&items)?; }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_round_trips() {
        assert_eq!(unhex(&hex(b"sk-rig-good \x00\xff")).as_deref(), Some(&b"sk-rig-good \x00\xff"[..]));
        assert_eq!(unhex("abc"), None);
        assert_eq!(unhex("zz"), None);
    }
}
