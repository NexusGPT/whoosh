//! An in-memory adapter for other lanes' unit tests: same ports, no Keychain. The Keychain
//! adapter is the one the app ships.
use std::collections::HashMap;
use std::sync::Mutex;

use contracts::{CoreError, KeychainRef};

use crate::{DeleteSecret, Namespace, ReadSecret, Secret, SecretKind, StoreSecret};

pub struct MemoryKeychain {
    namespace: Namespace,
    items: Mutex<HashMap<(String, String), Secret>>,
}

impl MemoryKeychain {
    pub fn new(namespace: Namespace) -> Self { MemoryKeychain { namespace, items: Mutex::new(HashMap::new()) } }
    fn key(r: &KeychainRef) -> (String, String) { (r.service.clone(), r.account.clone()) }
}

impl StoreSecret for MemoryKeychain {
    fn execute(&self, kind: SecretKind, name: &str, secret: &Secret) -> Result<KeychainRef, CoreError> {
        let r = self.namespace.reference(kind, name)?;
        self.items.lock().expect("memory keychain lock").insert(Self::key(&r), secret.clone());
        Ok(r)
    }
}

impl ReadSecret for MemoryKeychain {
    fn execute(&self, r: &KeychainRef) -> Result<Secret, CoreError> {
        self.namespace.kind_of(r)?;
        self.items.lock().expect("memory keychain lock").get(&Self::key(r)).cloned()
            .ok_or_else(|| CoreError::NotFound { kind: "secret".into(), id: r.account.clone() })
    }
}

impl DeleteSecret for MemoryKeychain {
    fn execute(&self, r: &KeychainRef) -> Result<(), CoreError> {
        self.namespace.kind_of(r)?;
        self.items.lock().expect("memory keychain lock").remove(&Self::key(r));
        Ok(())
    }
}
