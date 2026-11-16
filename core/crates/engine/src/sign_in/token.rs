//! An OAuth2 account's access token (slice 2b): built from a finished sign-in when the account is added, and rebuilt from
//! the Keychain at each launch. The refresh token, the client ID and the authority's token URL are three Keychain items of
//! one service, named after the address (oauth::{refresh,client,authority}_secret_name); the store keeps the refresh
//! item's reference only.
use std::sync::{Arc, Mutex};

use contracts::{oauth_account_draft_id, CoreError, KeychainRef, MailProvider, OAuthProvider, OAuthSignInState};
use oauth::{authority_secret_name, client_secret_name, refresh_secret_name, RefreshingToken};
use secrets::ReadSecret;

use super::codes::{coded, refusal, refused_code, NO_CLIENT_ID, NO_REFRESH_TOKEN, SIGN_IN_NOT_DONE};
use super::SignIn;
use crate::rust_core::Keyring;
use crate::sync::SystemClock;

/// What Add account needs from a finished sign-in.
pub struct NewOAuth { pub token: Arc<RefreshingToken>, pub refresh_ref: KeychainRef, pub provider: MailProvider }

fn token(endpoint: Arc<dyn oauth::TokenEndpoint>, keychain: Arc<dyn Keyring>, token_url: String, client_id: String, refresh_ref: KeychainRef, refresh_name: String) -> Arc<RefreshingToken> {
    Arc::new(RefreshingToken {
        endpoint, token_url, client_id, refresh_ref, refresh_name, read: keychain.clone(), store: keychain,
        clock: Arc::new(SystemClock), margin_ms: oauth::ledger_margin_ms(), cached: Mutex::new(None),
    })
}

/// Add account on the sign-in of `address`: it must be Done, its refresh token readable in the Keychain. Each other state
/// is refused in words with its code.
pub fn for_new_account(sign_in: &SignIn, keychain: Arc<dyn Keyring>, address: &str) -> Result<NewOAuth, CoreError> {
    let draft = oauth_account_draft_id(address.to_string()).map_err(|e| coded(e, SIGN_IN_NOT_DONE, address))?;
    // the raw state: the words carry no code here, the refusal below sets it (consent required or refused)
    match sign_in.sign_ins.state(&draft) {
        OAuthSignInState::Done => {}
        OAuthSignInState::NeedsClientId => return Err(refusal("Paste your organisation's app ID and sign in first.", NO_CLIENT_ID, address)),
        OAuthSignInState::Ready => return Err(refusal("Sign in with the provider's page first.", SIGN_IN_NOT_DONE, address)),
        OAuthSignInState::Waiting { .. } => return Err(refusal("The sign-in in the browser is not finished yet.", SIGN_IN_NOT_DONE, address)),
        OAuthSignInState::Refused { words } => return Err(refusal(&words, refused_code(&words), address)),
    }
    let refresh_ref = sign_in.sign_ins.reference_of(&refresh_secret_name(&draft)).map_err(|e| coded(e, NO_REFRESH_TOKEN, address))?;
    ReadSecret::execute(&*keychain, &refresh_ref)
        .map_err(|e| coded(CoreError::Unexpected { reason_words: format!("the sign-in finished but its refresh token is not in the Keychain: {e}") }, NO_REFRESH_TOKEN, address))?;
    let client_id = sign_in.sign_ins.client_id_of(&draft).map_err(|e| coded(e, NO_CLIENT_ID, address))?;
    let authority = sign_in.sign_ins.authority_of(&draft).ok_or_else(|| refusal("Sign in with the provider's page first.", SIGN_IN_NOT_DONE, address))?;
    let provider = match sign_in.sign_ins.provider_of(&draft) { Some(OAuthProvider::Google) => MailProvider::Google, _ => MailProvider::Microsoft };
    let t = token(sign_in.endpoint.clone(), keychain, authority.token_url, client_id, refresh_ref.clone(), refresh_secret_name(&draft));
    Ok(NewOAuth { token: t, refresh_ref, provider })
}

fn read_text(keychain: &dyn Keyring, r: &KeychainRef) -> Result<String, String> {
    let s = ReadSecret::execute(keychain, r).map_err(|e| e.to_string())?;
    String::from_utf8(s.expose().to_vec()).map_err(|e| e.to_string())
}

/// At launch: the stored account's token, from its refresh item's reference and the two items beside it. Refused in words
/// when an item cannot be read (the caller puts the words on the account's sync state).
pub fn at_launch(endpoint: Arc<dyn oauth::TokenEndpoint>, keychain: Arc<dyn Keyring>, refresh_ref: KeychainRef) -> Result<Arc<RefreshingToken>, String> {
    let draft = refresh_ref.account.strip_suffix(":oauth-refresh").ok_or("the stored sign-in reference does not name a refresh token")?.to_string();
    let beside = |name: String| KeychainRef { service: refresh_ref.service.clone(), account: name };
    let client_id = read_text(&*keychain, &beside(client_secret_name(&draft))).map_err(|e| format!("the app ID of this account could not be read from the Keychain: {e}"))?;
    let token_url = read_text(&*keychain, &beside(authority_secret_name(&draft))).map_err(|e| format!("the sign-in server of this account could not be read from the Keychain: {e}"))?;
    Ok(token(endpoint, keychain, token_url, client_id, refresh_ref.clone(), refresh_secret_name(&draft)))
}
