//! Slice 2b, the use case: an OAuth2 account added on a finished browser sign-in, and rebuilt at launch. The account is
//! checked by XOAUTH2 (IMAP login and list, SMTP auth) with a fresh access token, stored with the reference of its refresh
//! token, and synced with a token source that refreshes before expiry (OAUTH_REFRESH_MARGIN_S).
use std::sync::Arc;

use contracts::{Account, AccountConfig, CommandOutcome, CoreError};

use crate::rust_core::{now_ms, RustCore};
use crate::sign_in::codes::{coded, refusal, REFUSED};
use crate::sign_in::{token, OAuthMutant};

impl RustCore {
    /// The IMAP sign-in for an access token: XOAUTH2 (the mutant `oauth-no-xoauth2-imap`: the token as a LOGIN password).
    fn token_auth(&self, token: &Arc<oauth::RefreshingToken>, access: &str) -> sync_imap::Auth {
        if self.sign_in.mutant == OAuthMutant::NoXoauth2OnImap { return sync_imap::Auth::Password(access.to_string()); }
        sync_imap::Auth::XOAuth2(Arc::new(sync_imap::OAuthTokenSource(token.clone())))
    }

    /// `AddAccountConfigured` with `auth_method` Oauth2 (the password is not used): the sign-in of the address must be Done.
    pub(crate) fn add_oauth_account(&self, address: String, display_name: Option<String>, config: AccountConfig) -> Result<CommandOutcome, CoreError> {
        let fail = |words: String| CoreError::Action { failure: contracts::ActionFailed { account_name: address.clone(), reason_words: words, retryable: false, code: None } };
        if self.sign_in.mutant == OAuthMutant::AddRefusesOauth2 {
            return Err(fail("This account signs in with its provider's page; the local engine adds password accounts only for now.".into()));
        }
        if self.store.account_summaries().map_err(|e| fail(e.to_string()))?.iter().any(|a| a.address.eq_ignore_ascii_case(&address)) {
            return Err(fail(format!("{address} is already an account on this Mac.")));
        }
        let new = token::for_new_account(&self.sign_in, self.keychain.clone(), &address)?;
        let t = new.token.clone();
        // the refresh runs off the caller's thread: a caller inside an async runtime never blocks it
        let access = std::thread::scope(|s| s.spawn(|| t.access_token()).join())
            .map_err(|_| refusal("The sign-in check stopped unexpectedly. Try again.", REFUSED, &address))?
            .map_err(|e| coded(e, REFUSED, &address))?;
        let cred = if self.sign_in.mutant == OAuthMutant::NoXoauth2OnImap { sync_imap::Credential::Password(access.clone()) } else { sync_imap::Credential::Token(access.clone()) };
        let report = self.validate_with(&config, &cred);
        if !report.ok { return Err(refusal(&report.summary_words, REFUSED, &address)); }
        let keychain_ref = format!("{}\u{1f}{}", new.refresh_ref.service, new.refresh_ref.account);
        let name = display_name.filter(|n| !n.trim().is_empty()).unwrap_or_else(|| address.clone());
        let id = self.store.add_account(&address, &name, new.provider, &keychain_ref, true, config.clone(), now_ms()).map_err(|e| fail(e.to_string()))?;
        match self.store.sync_accounts().ok().and_then(|all| all.into_iter().find(|a| a.account.id == id)) {
            Some(a) => { let auth = self.token_auth(&new.token, &access); self.start_sync(a.account, &config, auth) }
            None => eprintln!("add {id}: the new account was not read back, so its sync did not start"),
        }
        Ok(CommandOutcome { undo_entry_id: None, created_id: Some(id) })
    }

    /// At launch: the stored OAuth2 account's token rebuilt from the Keychain, then its sync. The first refresh happens when
    /// the sync signs in, so a launch never waits on the provider; a refusal lands on the account's sync state in its words.
    pub(crate) fn start_stored_oauth(&self, account: Account, keychain_ref: &str, config: &AccountConfig) {
        let built = keychain_ref.split_once('\u{1f}')
            .map(|(service, account)| contracts::KeychainRef { service: service.to_string(), account: account.to_string() })
            .ok_or_else(|| "the stored sign-in reference has no separator".to_string())
            .and_then(|r| token::at_launch(self.sign_in.endpoint.clone(), self.keychain.clone(), r));
        match built {
            Ok(t) => {
                let auth = if self.sign_in.mutant == OAuthMutant::NoXoauth2OnImap { sync_imap::Auth::Password(String::new()) } else { sync_imap::Auth::XOAuth2(Arc::new(sync_imap::OAuthTokenSource(t))) };
                self.start_sync(account, config, auth)
            }
            Err(why) => self.record_failure(&account, format!("The app could not rebuild this account's sign-in from the Keychain: {why}")),
        }
    }
}
