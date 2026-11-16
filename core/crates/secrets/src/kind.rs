//! The three item kinds and the namespace every item lives under.
use contracts::{CoreError, KeychainRef};

/// What a secret is for. Each kind is its own Keychain `service`, so one listing by service
/// never mixes an account password with an AI key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SecretKind {
    /// An app password or sign-in token for one mail account (`account.keychain_ref`).
    AccountCredential,
    /// An AI-provider or decision-model API key (`ai_provider.keychain_ref`, `decision_model`).
    ProviderCredential,
    /// The single-use Mac to iPhone pairing token (`device_link`).
    DeviceLink,
}

impl SecretKind {
    pub const ALL: [SecretKind; 3] =
        [SecretKind::AccountCredential, SecretKind::ProviderCredential, SecretKind::DeviceLink];

    pub fn slug(self) -> &'static str {
        match self {
            SecretKind::AccountCredential => "account-credential",
            SecretKind::ProviderCredential => "provider-credential",
            SecretKind::DeviceLink => "device-link",
        }
    }

    /// Device links never leave the Mac that made them (the Mac wins and deletes it on use or
    /// expiry); the other two ride iCloud Keychain to the iPhone (docs/DATA-MAP.md).
    pub fn synchronizable(self) -> bool {
        !matches!(self, SecretKind::DeviceLink)
    }
}

/// The reverse-DNS prefix of every service this crate writes. The app uses `PRODUCTION`; every
/// test uses `RIG`, so a test can only ever touch items it made itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Namespace(String);

impl Namespace {
    pub const PRODUCTION_PREFIX: &'static str = "io.nexus.whoosh.";
    pub const RIG_PREFIX: &'static str = "io.nexus.whoosh.rig.";

    pub fn production() -> Self { Namespace(Self::PRODUCTION_PREFIX.to_string()) }
    pub fn rig() -> Self { Namespace(Self::RIG_PREFIX.to_string()) }

    /// A rig namespace narrowed by a run tag (`io.nexus.whoosh.rig.<tag>.`), so two test runs
    /// never see each other's items.
    pub fn rig_run(tag: &str) -> Result<Self, CoreError> {
        check_part("rig tag", tag)?;
        Ok(Namespace(format!("{}{}.", Self::RIG_PREFIX, tag)))
    }

    pub fn prefix(&self) -> &str { &self.0 }

    pub fn reference(&self, kind: SecretKind, name: &str) -> Result<KeychainRef, CoreError> {
        check_part("secret name", name)?;
        Ok(KeychainRef { service: format!("{}{}", self.0, kind.slug()), account: name.to_string() })
    }

    /// The kind a reference names, or an error when the reference is not ours. Every read and
    /// delete goes through here: an item outside the namespace is never touched.
    pub fn kind_of(&self, r: &KeychainRef) -> Result<SecretKind, CoreError> {
        let slug = r.service.strip_prefix(&self.0).ok_or_else(|| CoreError::Invalid {
            field: "keychain_ref.service".into(),
            reason_words: format!("{} is outside {}", r.service, self.0),
        })?;
        SecretKind::ALL.into_iter().find(|k| k.slug() == slug).ok_or_else(|| CoreError::Invalid {
            field: "keychain_ref.service".into(),
            reason_words: format!("{slug} is not an item kind of this app"),
        })
    }
}

fn check_part(field: &str, value: &str) -> Result<(), CoreError> {
    let ok = !value.is_empty()
        && value.len() <= 128
        && value.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '@' | '+' | ':'));
    if ok { Ok(()) } else {
        Err(CoreError::Invalid { field: field.into(), reason_words: "use 1 to 128 letters, digits and - _ . @ + :".into() })
    }
}
