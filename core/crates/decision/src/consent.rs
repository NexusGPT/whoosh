//! The consent-once-per-provider gate (C-J5.4): no mail reaches Jev before the owner agreed to it once. The app persists
//! the answer; this crate only asks.
use contracts::DecisionProviderKind;
use std::collections::HashSet;
use std::sync::Mutex;

pub trait HasConsent: Send + Sync {
    fn execute(&self, provider: DecisionProviderKind) -> bool;
}

/// In-memory consent for tests and previews.
#[derive(Default)]
pub struct MemoryConsent(Mutex<HashSet<DecisionProviderKindKey>>);

#[derive(Hash, PartialEq, Eq, Clone, Copy)]
struct DecisionProviderKindKey(u8);

fn key(p: DecisionProviderKind) -> DecisionProviderKindKey {
    DecisionProviderKindKey(match p {
        DecisionProviderKind::Local => 0,
        DecisionProviderKind::Jev => 1,
        DecisionProviderKind::Rules => 2,
        DecisionProviderKind::Apple => 3,
    })
}

impl MemoryConsent {
    pub fn give(&self, provider: DecisionProviderKind) {
        self.0.lock().expect("consent lock").insert(key(provider));
    }
}

impl HasConsent for MemoryConsent {
    fn execute(&self, provider: DecisionProviderKind) -> bool {
        self.0.lock().expect("consent lock").contains(&key(provider))
    }
}
