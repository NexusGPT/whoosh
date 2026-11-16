//! The Keychain adapter (macOS and iOS share the Security framework; this lane builds and
//! tests the macOS side). Built on security-framework (MIT OR Apache-2.0), the open-source start.
//!
//! One generic-password item per secret: `service` = `<namespace><kind slug>`, `account` = the
//! name the database already holds. Account and provider credentials are written with
//! `kSecAttrSynchronizable = true` so iCloud Keychain carries them to the iPhone; a device link
//! is single use and stays on the Mac that made it.
use contracts::{ActionFailed, CoreError, KeychainRef};
use core_foundation::base::{CFType, TCFType};
use core_foundation::boolean::CFBoolean;
use core_foundation::dictionary::CFDictionary;
use core_foundation::string::CFString;
use security_framework::base::Error as OsError;
use security_framework::passwords::{
    delete_generic_password_options, generic_password, set_generic_password_options, PasswordOptions,
};
use security_framework_sys::base::{errSecItemNotFound, SecCopyErrorMessageString};
use security_framework_sys::item::{
    kSecAttrAccount, kSecAttrService, kSecAttrSynchronizable, kSecAttrSynchronizableAny, kSecClass,
    kSecClassGenericPassword, kSecReturnAttributes,
};
use security_framework_sys::keychain_item::SecItemCopyMatching;

use crate::{DeleteSecret, Namespace, ReadSecret, Secret, SecretKind, StoreSecret};

/// What the Keychain says about one stored item. Only the facts the tests and Settings need.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemAttributes {
    pub synchronizable: bool,
}

/// Whether kinds that ride iCloud Keychain are written as synchronizable items.
///
/// `ICloud` is the shipped mode. macOS only accepts a synchronizable item from a process signed
/// with a keychain-access-groups entitlement (the signed app); an unsigned process such as
/// `cargo test` gets status -34018. `LocalOnly` writes the same items without the flag, and is
/// for unsigned processes only.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncMode { ICloud, LocalOnly }

pub struct Keychain {
    namespace: Namespace,
    sync: SyncMode,
}

impl Keychain {
    /// The shipped adapter: account and provider credentials synchronizable.
    pub fn new(namespace: Namespace) -> Self { Keychain { namespace, sync: SyncMode::ICloud } }
    pub fn with_sync(namespace: Namespace, sync: SyncMode) -> Self { Keychain { namespace, sync } }
    pub fn namespace(&self) -> &Namespace { &self.namespace }

    fn synchronizable(&self, kind: SecretKind) -> bool {
        self.sync == SyncMode::ICloud && kind.synchronizable()
    }

    /// The query every store, read and delete is built from. Public so a test can read the
    /// attributes the adapter asks the OS for.
    pub fn options(&self, reference: &KeychainRef, kind: SecretKind) -> PasswordOptions {
        let mut o = PasswordOptions::new_generic_password(&reference.service, &reference.account);
        o.set_access_synchronized(Some(self.synchronizable(kind)));
        o
    }

    /// The attributes the Keychain holds for this item (not a copy the crate kept).
    pub fn attributes(&self, reference: &KeychainRef) -> Result<ItemAttributes, CoreError> {
        self.namespace.kind_of(reference)?;
        let pairs: Vec<(CFString, CFType)> = unsafe {
            vec![
                (CFString::wrap_under_get_rule(kSecClass), CFString::wrap_under_get_rule(kSecClassGenericPassword).into_CFType()),
                (CFString::wrap_under_get_rule(kSecAttrService), CFString::from(reference.service.as_str()).into_CFType()),
                (CFString::wrap_under_get_rule(kSecAttrAccount), CFString::from(reference.account.as_str()).into_CFType()),
                (CFString::wrap_under_get_rule(kSecAttrSynchronizable), CFString::wrap_under_get_rule(kSecAttrSynchronizableAny).into_CFType()),
                (CFString::wrap_under_get_rule(kSecReturnAttributes), CFBoolean::true_value().into_CFType()),
            ]
        };
        let query = CFDictionary::from_CFType_pairs(&pairs);
        let mut found: core_foundation::base::CFTypeRef = std::ptr::null();
        let status = unsafe { SecItemCopyMatching(query.as_concrete_TypeRef(), &mut found) };
        if status != 0 {
            return Err(map_status(status, &reference.account));
        }
        let dict = unsafe { CFDictionary::<CFString, CFType>::wrap_under_create_rule(found as _) };
        let key = unsafe { CFString::wrap_under_get_rule(kSecAttrSynchronizable) };
        let synchronizable = dict
            .find(&key)
            .and_then(|v| v.downcast::<CFBoolean>())
            .map(bool::from)
            .unwrap_or(false);
        Ok(ItemAttributes { synchronizable })
    }
}

impl StoreSecret for Keychain {
    fn execute(&self, kind: SecretKind, name: &str, secret: &Secret) -> Result<KeychainRef, CoreError> {
        let reference = self.namespace.reference(kind, name)?;
        set_generic_password_options(secret.expose(), self.options(&reference, kind))
            .map_err(|e| map_error(&e, name))?;
        Ok(reference)
    }
}

impl ReadSecret for Keychain {
    fn execute(&self, reference: &KeychainRef) -> Result<Secret, CoreError> {
        let kind = self.namespace.kind_of(reference)?;
        generic_password(self.options(reference, kind))
            .map(Secret::new)
            .map_err(|e| map_error(&e, &reference.account))
    }
}

impl DeleteSecret for Keychain {
    fn execute(&self, reference: &KeychainRef) -> Result<(), CoreError> {
        let kind = self.namespace.kind_of(reference)?;
        match delete_generic_password_options(self.options(reference, kind)) {
            Ok(()) => Ok(()),
            Err(e) if e.code() == errSecItemNotFound => Ok(()),
            Err(e) => Err(map_error(&e, &reference.account)),
        }
    }
}

fn map_error(e: &OsError, account: &str) -> CoreError { map_status(e.code(), account) }

fn map_status(status: i32, account: &str) -> CoreError {
    if status == errSecItemNotFound {
        return CoreError::NotFound { kind: "secret".into(), id: account.to_string() };
    }
    let words = os_message(status);
    // -128 user cancelled, -25308 interaction not allowed, -25293 auth failed: the owner can retry.
    let retryable = matches!(status, -128 | -25308 | -25293);
    CoreError::Action { failure: ActionFailed { account_name: account.to_string(), reason_words: words, retryable, code: None } }
}

fn os_message(status: i32) -> String {
    let s = unsafe { SecCopyErrorMessageString(status, std::ptr::null_mut()) };
    if s.is_null() {
        return format!("the Keychain refused (status {status})");
    }
    let text = unsafe { CFString::wrap_under_create_rule(s) }.to_string();
    format!("the Keychain refused: {text} (status {status})")
}
