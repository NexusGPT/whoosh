//! real-accounts: the account commands of Settings > Accounts in the real core (they answered "The local engine does not do this yet.").
//!
//! - RenameAccount, SetAccountColour, SetAccountIcon (library glyph or the owner's own image), ReorderAccounts: one store transaction each
//!   (`store/src/realcore.rs`, "real-accounts"), nothing on the network, so each answers in a few milliseconds. The words of a refusal are the
//!   test engine's (`Sources/CoreBridge/FixtureAccounts.swift`, `FixtureSettings.swift`), the unknown account is `NotFound`.
//! - PauseAccount: the flag is stored (it survives a restart: `start_stored_account` reads it), the account's sync task is stopped, its IMAP
//!   adapter is let go (the write-back worker then answers "not connected yet" and keeps every pending operation) and its body source refuses.
//!   Its outgoing queue is held by `outbox_tick::StoreSendAccounts` (a send of a paused account stays in the Outbox, failed with
//!   `held_failure`). Nothing is deleted. Resume starts the sync again and sends what was held, on a thread of its own.
//! - RemoveAccount: refused in words while one of the account's messages is being sent; else the sync stops, ONE store transaction takes the
//!   account's mail, drafts, queue, pending operations and labels off this Mac, then its Keychain items are deleted (never read out, never
//!   printed). Nothing of another account is touched.
//! - AddAccount is the test engine's address-only shape (command.rs: "`AddAccount` (address and provider only) stays for callers that have no
//!   configuration"; AddAccountOAuth.swift:172 sends it under the fixture only). The real core adds an account with `AddAccountConfigured`
//!   (rust_core.rs `add_account`); this file serves a refusal that says so and builds no second add path.
//! - The state paths of the same feature (`state`): account/count, account/order, account/<id>/{name,address,provider,colour,paused,
//!   <server settings>,password_stored}, account_icon/<id>, account_credential/<id>/stored, sync_state/<id>/*. `account/<id>/password` is never served.
//!
//! This module is declared inside rust_core.rs (`#[path]`), as people.rs and search_cmd.rs are, so it reads the core's private fields.
use std::sync::Arc;

use contracts::constants::ACCOUNT_ICON_MAX_BYTES;
use contracts::WhooshCore;
use contracts::{AccountIconSource, ActionFailed, AuthMethod, Command, CommandOutcome, ConnectionSecurity, CoreError, CoreEvent, DetectionSource, KeychainRef, OutboxState, StreamKind};
use secrets::{DeleteSecret, ReadSecret};
use store::StoreError;

use super::{now_ms, RustCore};
use crate::bodies::BodySource;
use crate::reads::json_text;
use crate::sign_in::codes;
use crate::sync::emit;

/// The refusal code of AddAccount on this engine: the sentence names the command to use.
pub const USE_CONFIGURED_CODE: &str = "account.use_configured";
/// The refusal code of RemoveAccount while a message of the account is being sent.
pub const SEND_IN_FLIGHT_CODE: &str = "account.send_in_flight";
/// The code of a send held by a paused account (the entry stays in the Outbox).
pub const PAUSED_CODE: &str = "account.paused";
/// The words on the account's sync status while it is paused.
pub const PAUSED_STATUS: &str = "Paused. Nothing is fetched or sent until you resume.";

/// The failure a send of a paused account stays in the Outbox with (retryable: resume sends it).
pub fn held_failure(account_name: &str) -> ActionFailed {
    ActionFailed { account_name: account_name.to_string(), reason_words: "This account is paused, so this message waits. It is sent when you resume the account.".into(), retryable: true, code: Some(PAUSED_CODE.into()) }
}

pub(crate) fn is_command(c: &Command) -> bool {
    matches!(c, Command::RenameAccount { .. } | Command::PauseAccount { .. } | Command::RemoveAccount { .. } | Command::SetAccountIcon { .. }
        | Command::SetAccountColour { .. } | Command::ReorderAccounts { .. } | Command::AddAccount { .. })
}

fn nothing() -> Result<CommandOutcome, CoreError> { Ok(CommandOutcome { undo_entry_id: None, created_id: None }) }
fn gone(id: &str) -> CoreError { CoreError::NotFound { kind: "account".into(), id: id.to_string() } }
fn invalid(field: &str, words: &str) -> CoreError { CoreError::Invalid { field: field.into(), reason_words: words.into() } }
fn unsaved(e: StoreError) -> CoreError { CoreError::Unexpected { reason_words: format!("the account change was not saved: {e}") } }

/// "#RRGGBB" (any case) as the store keeps it: upper case.
fn colour_of(raw: &str) -> Result<String, CoreError> {
    let c = raw.trim();
    let hex = c.strip_prefix('#').unwrap_or("");
    if hex.len() == 6 && hex.chars().all(|ch| ch.is_ascii_hexdigit()) { Ok(c.to_ascii_uppercase()) } else { Err(invalid("colour", "a colour is written #RRGGBB, for example #2B8A3E")) }
}

fn mime_of(path: &std::path::Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase).as_deref() {
        Some("png") => "image/png", Some("jpg") | Some("jpeg") => "image/jpeg", Some("gif") => "image/gif", Some("heic") => "image/heic",
        Some("webp") => "image/webp", Some("tif") | Some("tiff") => "image/tiff", Some("svg") => "image/svg+xml", _ => "application/octet-stream",
    }
}

/// The body source of a paused or removed account: it refuses, so nothing is fetched for the account (the crawl and the reader's asks stop).
struct HeldSource;
#[async_trait::async_trait]
impl BodySource for HeldSource {
    async fn fetch_bodies(&self, _folder: &str, _uids: &[u32]) -> Result<Vec<(u32, Vec<u8>)>, CoreError> {
        Err(CoreError::Action { failure: ActionFailed { account_name: String::new(), reason_words: "This account is paused, so no mail is fetched for it.".into(), retryable: true, code: Some(PAUSED_CODE.into()) } })
    }
}

pub(crate) fn run(core: &RustCore, command: Command) -> Result<CommandOutcome, CoreError> {
    let at = now_ms();
    match command {
        Command::RenameAccount { account_id, name } => {
            if !core.store.account_exists(&account_id).map_err(unsaved)? { return Err(gone(&account_id)); }
            let n = name.trim().to_string();
            if n.is_empty() { return Err(invalid("name", "an account needs a name")); }
            core.store.rename_account(&account_id, &n, at).map_err(unsaved)?;
            nothing()
        }
        Command::SetAccountColour { account_id, colour } => {
            if !core.store.account_exists(&account_id).map_err(unsaved)? { return Err(gone(&account_id)); }
            let c = colour_of(&colour)?;
            core.store.set_account_colour(&account_id, &c, at).map_err(unsaved)?;
            nothing()
        }
        Command::SetAccountIcon { account_id, source } => {
            if !core.store.account_exists(&account_id).map_err(unsaved)? { return Err(gone(&account_id)); }
            match source {
                AccountIconSource::Library { icon_id } => {
                    let id = icon_id.trim();
                    if id.is_empty() { return Err(invalid("icon", "pick an icon from the library")); }
                    core.store.set_account_library_icon(&account_id, id, at).map_err(unsaved)?;
                }
                AccountIconSource::Uploaded { file_path } => {
                    let path = std::path::Path::new(&file_path);
                    let size = match std::fs::metadata(path) { Ok(m) if m.is_file() && m.len() > 0 => m.len(), _ => return Err(invalid("icon", "that image could not be read")) };
                    if size > ACCOUNT_ICON_MAX_BYTES { return Err(invalid("icon", &format!("that image is too big: the limit is {} MB", ACCOUNT_ICON_MAX_BYTES / 1_048_576))); }
                    let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("icon").to_string();
                    core.store.set_account_uploaded_icon(&account_id, &file_name, mime_of(path), size, &file_path, at).map_err(unsaved)?;
                }
            }
            nothing()
        }
        Command::ReorderAccounts { ids } => {
            core.store.reorder_accounts(&ids, at).map_err(|e| match e { StoreError::Invalid(w) => invalid("ids", &w), other => unsaved(other) })?;
            nothing()
        }
        Command::PauseAccount { account_id, paused } => { core.pause_account(&account_id, paused, at)?; nothing() }
        Command::RemoveAccount { account_id } => { core.remove_account(&account_id)?; nothing() }
        // by design the test engine's shape: the refusal names the command this engine takes
        Command::AddAccount { address, .. } => Err(codes::refusal(
            "This engine adds an account with its server settings and its password, from the Add account sheet: that is the command AddAccountConfigured. The address-only AddAccount is the test engine's.",
            USE_CONFIGURED_CODE, address.trim())),
        _ => Err(codes::refusal("This is not an account command.", codes::NOT_BUILT_YET, "")),
    }
}

impl RustCore {
    /// Stops what the account's sync holds: its task, its IMAP adapter (the write-back worker and the outbox then find the account "not
    /// connected yet" and keep what they were asked) and its body source.
    fn stop_account_sync(&self, account_id: &str) {
        if let Some(stop) = self.stops.lock().unwrap_or_else(|e| e.into_inner()).remove(account_id) { stop.notify_waiters(); stop.notify_one(); }
        self.deps.adapters.lock().unwrap_or_else(|e| e.into_inner()).remove(account_id);
        self.deps.phases.lock().unwrap_or_else(|e| e.into_inner()).remove(account_id);
        self.deps.bodies.set_source(account_id, Arc::new(HeldSource));
    }

    /// Writes the status words on the account's sync state and tells the SyncStatus stream.
    fn say_status(&self, account_id: &str, words: Option<&str>) {
        let patch = sync_engine::StatePatch { provider_status_words: Some(words.map(str::to_string)), ..Default::default() };
        match sync_engine::state::write_state(&self.store, account_id, patch) {
            Ok(state) => emit(&self.sinks, StreamKind::SyncStatus, CoreEvent::SyncStateChanged { state }),
            Err(e) => eprintln!("account {account_id}: the status was not written: {e}"),
        }
    }

    fn pause_account(&self, account_id: &str, paused: bool, at: i64) -> Result<(), CoreError> {
        if !self.store.set_account_paused(account_id, paused, at).map_err(unsaved)? { return Err(gone(account_id)); }
        if paused {
            self.stop_account_sync(account_id);
            self.say_status(account_id, Some(PAUSED_STATUS));
            return Ok(());
        }
        self.say_status(account_id, None);
        if let Some(a) = self.store.sync_accounts().unwrap_or_default().into_iter().find(|a| a.account.id == account_id) { self.start_stored_account(a); }
        self.catch_up_inbox(account_id);
        // what the Outbox held while the account was paused leaves now, on a thread of its own (the command answers at once)
        let held: Vec<String> = self.act.mail.open_entries().into_iter()
            .filter(|e| e.draft.from_account_id == account_id && matches!(&e.state, OutboxState::Failed { failure, .. } if failure.code.as_deref() == Some(PAUSED_CODE)))
            .map(|e| e.id).collect();
        if !held.is_empty() {
            let mail = self.act.mail.clone();
            let spawned = std::thread::Builder::new().name("whoosh-resume-send".into()).spawn(move || { for id in held { if let Err(e) = mail.send_now(&id) { eprintln!("resume: {id} was not sent: {e}"); } } });
            if let Err(e) = spawned { eprintln!("resume: the held messages leave on the next send ({e})"); }
        }
        self.act.worker.kick();
        Ok(())
    }

    /// reds-account: a resumed account's restarted sync finds its history stored and goes straight to listening, and a listener that starts
    /// does not read the folder: mail that reached the server while the account was paused would wait for the next push. So, once the
    /// restarted sync listens, INBOX is brought up to date once (`sync::apply_changes`, the call the listener makes after a reconnect).
    /// On the core's runtime, never on the caller's thread; it ends at once when the account was paused or removed again meanwhile.
    fn catch_up_inbox(&self, account_id: &str) {
        let Some(rt) = self.runtime.as_ref() else { return };
        let (d, id) = (self.deps.clone(), account_id.to_string());
        rt.spawn(async move {
            for _ in 0..600 {
                let phase = d.phases.lock().unwrap_or_else(|e| e.into_inner()).get(&id).cloned();
                match phase {
                    Some(crate::sync::SyncPhase::Listening) => break,
                    Some(crate::sync::SyncPhase::Fetching) | None => {}
                    Some(crate::sync::SyncPhase::Failed(_)) => return,
                }
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            }
            let adapter = d.adapters.lock().unwrap_or_else(|e| e.into_inner()).get(&id).cloned();
            let Some(adapter) = adapter else { return };
            if let Err(words) = crate::sync::apply_changes(&d, &id, &adapter, "INBOX").await { eprintln!("account {id}: mail that arrived while paused was not fetched: {words}"); }
        });
    }

    /// The Keychain items an account owns: its credential, and for a sign-in by OAuth the two items beside the refresh token.
    fn account_secret_refs(&self, reference: &str, is_oauth: bool) -> Vec<KeychainRef> {
        let Some((service, account)) = reference.split_once('\u{1f}') else { return vec![] };
        let mut refs = vec![KeychainRef { service: service.to_string(), account: account.to_string() }];
        if is_oauth || account.ends_with(":oauth-refresh") {
            if let Some(draft) = account.strip_suffix(":oauth-refresh") {
                for name in [oauth::client_secret_name(draft), oauth::authority_secret_name(draft)] { refs.push(KeychainRef { service: service.to_string(), account: name }); }
            }
        }
        refs
    }

    fn remove_account(&self, account_id: &str) -> Result<(), CoreError> {
        let Some(facts) = self.store.account_removal(account_id).map_err(unsaved)? else { return Err(gone(account_id)) };
        if self.act.mail.open_entries().iter().any(|e| e.draft.from_account_id == account_id && matches!(e.state, OutboxState::Sending)) {
            return Err(codes::refusal(&format!("{} is sending a message right now. Remove the account when the message has left.", facts.account_name), SEND_IN_FLIGHT_CODE, &facts.account_name));
        }
        let credential = self.store.account_credential_ref(account_id).map_err(unsaved)?;
        // fix-blockforget: what the search indexes hold of the account, read before its rows go with it
        let (message_ids, chunk_ids) = self.store.account_index_rows(account_id).map_err(unsaved)?;
        self.stop_account_sync(account_id);
        match self.store.remove_account(account_id) {
            Ok(true) => {}
            Ok(false) => return Err(gone(account_id)),
            Err(e) => {
                // nothing was removed: the sync it had starts again
                self.start_stored_account_by_id(account_id);
                return Err(unsaved(e));
            }
        }
        // fix-blockforget (real-accounts finding 2): the people list, the crawl and the two search indexes forget the account now, in memory
        // (the store transaction above took its undo entries too: finding 5)
        self.people.forget_account(account_id);
        self.deps.bodies.forget_account(account_id, &message_ids, &chunk_ids);
        let mut left: Vec<String> = Vec::new();
        if let Some((reference, oauth)) = credential {
            for r in self.account_secret_refs(&reference, oauth) {
                if let Err(e) = DeleteSecret::execute(&*self.keychain, &r) { left.push(e.to_string()); }
            }
        }
        if left.is_empty() { return Ok(()); }
        Err(codes::refusal(&format!("{} was removed from this Mac, but its sign-in could not be taken out of the Keychain: {}", facts.account_name, left.join("; ")), "account.keychain_left", &facts.account_name))
    }

    fn start_stored_account_by_id(&self, account_id: &str) {
        if let Some(a) = self.store.sync_accounts().unwrap_or_default().into_iter().find(|a| a.account.id == account_id) { self.start_stored_account(a); }
    }

    /// Whether the account's credential can be read from the Keychain now (the secret itself is dropped unread).
    fn credential_stored(&self, account_id: &str) -> Option<(bool, bool)> {
        let (reference, oauth) = self.store.account_credential_ref(account_id).ok().flatten()?;
        let (service, account) = reference.split_once('\u{1f}')?;
        let r = KeychainRef { service: service.to_string(), account: account.to_string() };
        Some((ReadSecret::execute(&*self.keychain, &r).is_ok(), oauth))
    }
}

fn security_word(s: ConnectionSecurity) -> &'static str { match s { ConnectionSecurity::Ssl => "ssl", ConnectionSecurity::Starttls => "starttls" } }
fn source_word(s: DetectionSource) -> &'static str {
    match s { DetectionSource::Ispdb => "ispdb", DetectionSource::Autoconfig => "autoconfig", DetectionSource::Srv => "srv", DetectionSource::Autodiscover => "autodiscover",
        DetectionSource::MailRecord => "mx", DetectionSource::Guess => "guess", DetectionSource::Manual => "manual" }
}

/// The account state paths (the test engine's `state(path:)` for "account", `account_icon`, `account_credential`, `sync_state`), as JSON text.
/// An unknown account or field answers None.
pub(crate) fn state(core: &RustCore, path: &str) -> Option<String> {
    let p: Vec<&str> = path.split('/').collect();
    match p.as_slice() {
        ["account", "count"] => Some(core.store.account_ids().ok()?.len().to_string()),
        ["account", "order"] => Some(format!("[{}]", core.store.account_ids().ok()?.iter().map(|i| json_text(i)).collect::<Vec<_>>().join(","))),
        ["account", _, "password"] => None,
        ["account", id, "password_stored"] => { let (stored, oauth) = core.credential_stored(id)?; Some((stored && !oauth).to_string()) }
        ["account", id, field @ ("name" | "address" | "provider" | "colour" | "paused")] => {
            let a = core.store.account_summaries().ok()?.into_iter().find(|a| a.id == *id)?;
            Some(match *field {
                "name" => json_text(&a.name), "address" => json_text(&a.address), "colour" => json_text(&a.colour), "paused" => a.paused.to_string(),
                _ => json_text(&format!("{:?}", a.provider).to_ascii_lowercase()),
            })
        }
        ["account", id, field @ ("imap_host" | "imap_port" | "imap_security" | "smtp_host" | "smtp_port" | "smtp_security" | "username" | "smtp_username" | "auth_method" | "detection_source")] => {
            let c = core.store.account_config(id).ok()??;
            Some(match *field {
                "imap_host" => json_text(&c.imap_host), "imap_port" => c.imap_port.to_string(), "imap_security" => json_text(security_word(c.imap_security)),
                "smtp_host" => json_text(&c.smtp_host), "smtp_port" => c.smtp_port.to_string(), "smtp_security" => json_text(security_word(c.smtp_security)),
                "username" => json_text(&c.username), "smtp_username" => json_text(&c.smtp_username),
                "auth_method" => json_text(if c.auth_method == AuthMethod::Password { "password" } else { "oauth2" }),
                _ => json_text(source_word(c.source)),
            })
        }
        // the library glyph, or "uploaded" while the owner's own image is the mark; None until the owner picks one (SPEC: the provider default is the app's drawing of none)
        ["account_icon", id] => {
            let a = core.store.account_summaries().ok()?.into_iter().find(|a| a.id == *id)?;
            if a.icon_file.is_some() { Some(json_text("uploaded")) } else { a.icon_id.map(|i| json_text(&i)) }
        }
        ["account_credential", id, "stored"] => core.credential_stored(id).map(|(stored, _)| stored.to_string()),
        ["sync_state", id, leaf] => {
            let s = core.sync_states().into_iter().find(|s| s.account_id == *id)?;
            match *leaf {
                "healthy" => Some(s.last_error.is_none().to_string()),
                "history_fetched" => Some(s.history_fetched.to_string()),
                "messages_indexed" => Some(s.messages_indexed.to_string()),
                "status" => s.provider_status_words.map(|w| json_text(&w)),
                "error" => s.last_error.map(|f| json_text(&f.reason_words)),
                "last_synced_ms" => s.last_synced_ms.map(|m| m.to_string()),
                _ => None,
            }
        }
        _ => None,
    }
}
