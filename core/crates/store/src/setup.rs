//! Migration 0003's tables: an account's server config, the last detection per domain, the Gmail message-in-several-folders
//! rows, and the text read out of attachments. Additive reads and writes; nothing here touches an older table's rows
//! except the new nullable columns of `account`.
use crate::error::StoreError;
use crate::Store;
use contracts::{AccountConfig, AttachmentContent, AuthMethod, ConnectionSecurity, DetectionSource};
use rusqlite::{params, OptionalExtension};

fn security_word(s: ConnectionSecurity) -> &'static str { match s { ConnectionSecurity::Ssl => "ssl", ConnectionSecurity::Starttls => "starttls" } }
fn security_from(w: &str) -> Result<ConnectionSecurity, StoreError> {
    match w { "ssl" => Ok(ConnectionSecurity::Ssl), "starttls" => Ok(ConnectionSecurity::Starttls), other => Err(StoreError::Invalid(format!("unknown connection security {other:?} in the store"))) }
}
/// The store writes 'basic' for a mail or app password, so the schema text holds no secret-sounding word.
fn auth_word(a: AuthMethod) -> &'static str { match a { AuthMethod::Password => "basic", AuthMethod::Oauth2 => "oauth2" } }
fn auth_from(w: &str) -> Result<AuthMethod, StoreError> {
    match w { "basic" => Ok(AuthMethod::Password), "oauth2" => Ok(AuthMethod::Oauth2), other => Err(StoreError::Invalid(format!("unknown auth method {other:?} in the store"))) }
}
fn source_word(s: DetectionSource) -> &'static str {
    match s { DetectionSource::Ispdb => "ispdb", DetectionSource::Autoconfig => "autoconfig", DetectionSource::Srv => "srv", DetectionSource::Autodiscover => "autodiscover", DetectionSource::MailRecord => "mx", DetectionSource::Guess => "guess", DetectionSource::Manual => "manual" }
}
fn source_from(w: &str) -> Result<DetectionSource, StoreError> {
    Ok(match w { "ispdb" => DetectionSource::Ispdb, "autoconfig" => DetectionSource::Autoconfig, "srv" => DetectionSource::Srv, "autodiscover" => DetectionSource::Autodiscover, "mx" => DetectionSource::MailRecord, "guess" => DetectionSource::Guess, "manual" => DetectionSource::Manual,
        other => return Err(StoreError::Invalid(format!("unknown detection source {other:?} in the store"))) })
}

fn json_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c { '"' => out.push_str("\\\""), '\\' => out.push_str("\\\\"), '\n' => out.push_str("\\n"), '\r' => out.push_str("\\r"), '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)), c => out.push(c) }
    }
    out.push('"');
    out
}

/// What the last detection of a domain concluded, as kept.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CachedDetection {
    /// the source is the config's own (`config.source`), kept in its column for the cache listing
    Found { config: AccountConfig },
    Refused { words: String },
}

pub use contracts::MessageLocation;

impl Store {
    /// Keeps the server config of an account. The password is not part of it: it lives in the Keychain, referenced by `keychain_ref`.
    pub fn set_account_config(&self, account_id: String, config: AccountConfig) -> Result<(), StoreError> {
        self.write(move |ctx| {
            let n = ctx.tx.execute(
                "UPDATE account SET imap_host = ?2, imap_port = ?3, imap_security = ?4, smtp_host = ?5, smtp_port = ?6, smtp_security = ?7, \
                 username = ?8, smtp_username = ?9, auth_method = ?10, detection_source = ?11 WHERE id = ?1",
                params![account_id, config.imap_host, config.imap_port, security_word(config.imap_security), config.smtp_host, config.smtp_port,
                        security_word(config.smtp_security), config.username, config.smtp_username, auth_word(config.auth_method), source_word(config.source)],
            )?;
            if n == 0 { return Err(StoreError::Invalid(format!("no account {account_id}"))); }
            Ok(())
        })
    }

    /// The server config of an account; None when it has none (added before 0003, or by Graph or JMAP).
    pub fn account_config(&self, account_id: &str) -> Result<Option<AccountConfig>, StoreError> {
        self.read(|c| {
            let row = c.prepare_cached("SELECT imap_host, imap_port, imap_security, smtp_host, smtp_port, smtp_security, username, smtp_username, auth_method, detection_source FROM account WHERE id = ?1")?
                .query_row(params![account_id], |r| Ok((r.get::<_, Option<String>>(0)?, r.get::<_, Option<u16>>(1)?, r.get::<_, Option<String>>(2)?, r.get::<_, Option<String>>(3)?,
                    r.get::<_, Option<u16>>(4)?, r.get::<_, Option<String>>(5)?, r.get::<_, Option<String>>(6)?, r.get::<_, Option<String>>(7)?, r.get::<_, Option<String>>(8)?, r.get::<_, Option<String>>(9)?)))
                .optional()?;
            let Some((Some(ih), Some(ip), Some(is), Some(sh), Some(sp), Some(ss), Some(u), Some(su), Some(am), Some(ds))) = row else { return Ok(None) };
            Ok(Some(AccountConfig { imap_host: ih, imap_port: ip, imap_security: security_from(&is)?, smtp_host: sh, smtp_port: sp, smtp_security: security_from(&ss)?,
                username: u, smtp_username: su, auth_method: auth_from(&am)?, source: source_from(&ds)? }))
        })
    }

    /// Keeps the last detection of a domain (the domain is lowercased); a second call replaces the first.
    pub fn put_detection(&self, domain: String, detection: CachedDetection, at_ms: i64) -> Result<(), StoreError> {
        let domain = domain.trim().to_lowercase();
        if domain.is_empty() { return Err(StoreError::Invalid("a detection needs the domain it describes".into())); }
        self.write(move |ctx| {
            let (outcome, source, config_json, words): (&str, Option<&str>, Option<String>, Option<String>) = match &detection {
                CachedDetection::Found { config } => ("found", Some(source_word(config.source)), Some(format!(
                    "{{\"imap_host\":{},\"imap_port\":{},\"imap_security\":{},\"smtp_host\":{},\"smtp_port\":{},\"smtp_security\":{},\"username\":{},\"smtp_username\":{},\"auth_method\":{},\"source\":{}}}",
                    json_string(&config.imap_host), config.imap_port, json_string(security_word(config.imap_security)), json_string(&config.smtp_host), config.smtp_port,
                    json_string(security_word(config.smtp_security)), json_string(&config.username), json_string(&config.smtp_username), json_string(auth_word(config.auth_method)),
                    json_string(source_word(config.source)))), None),
                CachedDetection::Refused { words } => ("refused", None, None, Some(words.clone())),
            };
            ctx.tx.execute(
                "INSERT INTO detection_cache (domain, outcome, source, config_json, refusal_words, detected_at_ms) VALUES (?1, ?2, ?3, ?4, ?5, ?6) \
                 ON CONFLICT(domain) DO UPDATE SET outcome = excluded.outcome, source = excluded.source, config_json = excluded.config_json, refusal_words = excluded.refusal_words, detected_at_ms = excluded.detected_at_ms",
                params![domain, outcome, source, config_json, words, at_ms],
            )?;
            Ok(())
        })
    }

    /// The last detection of a domain with the time it ran; None when none is kept.
    pub fn detection(&self, domain: &str) -> Result<Option<(CachedDetection, i64)>, StoreError> {
        let domain = domain.trim().to_lowercase();
        self.read(|c| {
            let row = c.prepare_cached(
                "SELECT outcome, source, json_extract(config_json,'$.imap_host'), json_extract(config_json,'$.imap_port'), json_extract(config_json,'$.imap_security'), \
                 json_extract(config_json,'$.smtp_host'), json_extract(config_json,'$.smtp_port'), json_extract(config_json,'$.smtp_security'), json_extract(config_json,'$.username'), \
                 json_extract(config_json,'$.smtp_username'), json_extract(config_json,'$.auth_method'), refusal_words, detected_at_ms FROM detection_cache WHERE domain = ?1")?
                .query_row(params![domain], |r| Ok((r.get::<_, String>(0)?, r.get::<_, Option<String>>(1)?, r.get::<_, Option<String>>(2)?, r.get::<_, Option<u16>>(3)?, r.get::<_, Option<String>>(4)?,
                    r.get::<_, Option<String>>(5)?, r.get::<_, Option<u16>>(6)?, r.get::<_, Option<String>>(7)?, r.get::<_, Option<String>>(8)?, r.get::<_, Option<String>>(9)?, r.get::<_, Option<String>>(10)?,
                    r.get::<_, Option<String>>(11)?, r.get::<_, i64>(12)?))).optional()?;
            let Some((outcome, source, ih, ip, is, sh, sp, ss, u, su, am, words, at)) = row else { return Ok(None) };
            let missing = || StoreError::Invalid("a cached detection is missing a field".into());
            let det = match outcome.as_str() {
                "refused" => CachedDetection::Refused { words: words.ok_or_else(missing)? },
                _ => {
                    let source = source_from(&source.ok_or_else(missing)?)?;
                    CachedDetection::Found { config: AccountConfig {
                        imap_host: ih.ok_or_else(missing)?, imap_port: ip.ok_or_else(missing)?, imap_security: security_from(&is.ok_or_else(missing)?)?,
                        smtp_host: sh.ok_or_else(missing)?, smtp_port: sp.ok_or_else(missing)?, smtp_security: security_from(&ss.ok_or_else(missing)?)?,
                        username: u.ok_or_else(missing)?, smtp_username: su.ok_or_else(missing)?, auth_method: auth_from(&am.ok_or_else(missing)?)?, source } }
                }
            };
            Ok(Some((det, at)))
        })
    }

    /// Manual setup clears the cache entry of its domain. Returns whether one existed.
    pub fn clear_detection(&self, domain: String) -> Result<bool, StoreError> {
        let domain = domain.trim().to_lowercase();
        self.write(move |ctx| Ok(ctx.tx.execute("DELETE FROM detection_cache WHERE domain = ?1", params![domain])? > 0))
    }

    /// Records one copy of a message in a label-folder (the same copy again changes nothing).
    pub fn add_message_location(&self, loc: MessageLocation) -> Result<(), StoreError> {
        self.write(move |ctx| {
            ctx.tx.execute(
                "INSERT INTO message_location (account_id, folder, uid_validity, uid, message_id) VALUES (?1, ?2, ?3, ?4, ?5) \
                 ON CONFLICT(account_id, folder, uid_validity, uid) DO UPDATE SET message_id = excluded.message_id",
                params![loc.account_id, loc.folder, loc.uid_validity, loc.uid, loc.message_id],
            )?;
            Ok(())
        })
    }

    /// Every label-folder a message sits in, folder order.
    pub fn message_locations(&self, message_id: &str) -> Result<Vec<MessageLocation>, StoreError> {
        self.read(|c| {
            let mut s = c.prepare_cached("SELECT account_id, folder, uid_validity, uid, message_id FROM message_location WHERE message_id = ?1 ORDER BY folder, uid_validity, uid")?;
            let rows = s.query_map(params![message_id], |r| Ok(MessageLocation { account_id: r.get(0)?, folder: r.get(1)?, uid_validity: r.get(2)?, uid: r.get(3)?, message_id: r.get(4)? }))?;
            Ok(rows.collect::<Result<Vec<_>, _>>()?)
        })
    }

    /// A UIDVALIDITY change: drops a folder's rows. Returns the messages that now sit in no folder at all (the caller decides what to do with them).
    pub fn forget_folder_locations(&self, account_id: String, folder: String) -> Result<Vec<String>, StoreError> {
        self.write(move |ctx| {
            let ids: Vec<String> = {
                let mut s = ctx.tx.prepare("SELECT DISTINCT message_id FROM message_location WHERE account_id = ?1 AND folder = ?2")?;
                let rows = s.query_map(params![account_id, folder], |r| r.get::<_, String>(0))?;
                rows.collect::<Result<Vec<_>, _>>()?
            };
            ctx.tx.execute("DELETE FROM message_location WHERE account_id = ?1 AND folder = ?2", params![account_id, folder])?;
            let mut orphans = Vec::new();
            for id in ids {
                let left: i64 = ctx.tx.query_row("SELECT count(*) FROM message_location WHERE message_id = ?1", params![id], |r| r.get(0))?;
                if left == 0 { orphans.push(id); }
            }
            Ok(orphans)
        })
    }

    /// Keeps what was read out of an attachment; a second call replaces the first. The attachment must exist.
    pub fn put_attachment_content(&self, content: AttachmentContent) -> Result<(), StoreError> {
        let labels = format!("[{}]", content.labels.iter().map(|l| json_string(l)).collect::<Vec<_>>().join(","));
        self.write(move |ctx| {
            ctx.tx.execute(
                "INSERT INTO attachment_content (attachment_id, text, ocr_text, labels_json, transcript, embedded, vector_key, updated_at_ms) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?8, ?7) \
                 ON CONFLICT(attachment_id) DO UPDATE SET text = excluded.text, ocr_text = excluded.ocr_text, labels_json = excluded.labels_json, transcript = excluded.transcript, embedded = excluded.embedded, vector_key = excluded.vector_key, updated_at_ms = excluded.updated_at_ms",
                params![content.attachment_id, content.text, content.ocr_text, labels, content.transcript, content.embedded, content.updated_at_ms, content.vector_key],
            )?;
            Ok(())
        })
    }

    pub fn attachment_content(&self, attachment_id: &str) -> Result<Option<AttachmentContent>, StoreError> {
        self.read(|c| {
            let row = c.prepare_cached("SELECT text, ocr_text, transcript, embedded, updated_at_ms, vector_key FROM attachment_content WHERE attachment_id = ?1")?
                .query_row(params![attachment_id], |r| Ok((r.get::<_, Option<String>>(0)?, r.get::<_, Option<String>>(1)?, r.get::<_, Option<String>>(2)?, r.get::<_, bool>(3)?, r.get::<_, i64>(4)?, r.get::<_, Option<String>>(5)?)))
                .optional()?;
            let Some((text, ocr_text, transcript, embedded, updated_at_ms, vector_key)) = row else { return Ok(None) };
            let mut s = c.prepare_cached("SELECT j.value FROM attachment_content a, json_each(a.labels_json) j WHERE a.attachment_id = ?1 ORDER BY j.key")?;
            let labels = s.query_map(params![attachment_id], |r| r.get::<_, String>(0))?.collect::<Result<Vec<_>, _>>()?;
            Ok(Some(AttachmentContent { attachment_id: attachment_id.to_string(), text, ocr_text, labels, transcript, embedded, vector_key, updated_at_ms }))
        })
    }
}
