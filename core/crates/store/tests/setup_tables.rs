//! Migration 0003's tables through the store's additive functions: one test per table (account columns,
//! detection_cache, message_location, attachment_content), each with the case that breaks it.
mod common;
use common::*;
use contracts::{AccountConfig, AttachmentContent, AuthMethod, ConnectionSecurity, DetectionSource};
use store::{CachedDetection, MessageLocation};

fn icloud() -> AccountConfig {
    AccountConfig {
        imap_host: "imap.mail.me.com".into(), imap_port: 993, imap_security: ConnectionSecurity::Ssl,
        smtp_host: "smtp.mail.me.com".into(), smtp_port: 587, smtp_security: ConnectionSecurity::Starttls,
        username: "assem.fixture".into(), smtp_username: "assem.fixture@icloud.com".into(), auth_method: AuthMethod::Password, source: DetectionSource::Ispdb,
    }
}

#[test]
fn an_account_keeps_its_server_config_and_an_older_account_has_none() {
    let db = TempDb::new("setup-account");
    let store = small_store(&db, 0);
    assert_eq!(store.account_config("acc-a").unwrap(), None, "an account added before 0003 has no server config");
    store.set_account_config("acc-a".into(), icloud()).unwrap();
    assert_eq!(store.account_config("acc-a").unwrap(), Some(icloud()));
    assert_eq!(store.account_config("acc-b").unwrap(), None, "the other account is untouched");
    let mut oauth = icloud();
    oauth.auth_method = AuthMethod::Oauth2;
    oauth.source = DetectionSource::Manual;
    store.set_account_config("acc-a".into(), oauth.clone()).unwrap();
    assert_eq!(store.account_config("acc-a").unwrap(), Some(oauth), "a second save replaces the first");
    assert!(store.set_account_config("acc-zz".into(), icloud()).unwrap_err().to_string().contains("no account acc-zz"));
    // the schema refuses a port outside 1..=65535 and a security word it does not know
    let bad = store.write(|ctx| Ok(ctx.tx.execute("UPDATE account SET imap_port = 70000 WHERE id = 'acc-a'", [])?));
    assert!(bad.is_err(), "a port above 65535 must be refused by the schema");
    // the password has no column: no account column holds anything that looks like one
    let cols: Vec<String> = store.write(|ctx| {
        let mut s = ctx.tx.prepare("SELECT name FROM pragma_table_info('account')")?;
        let rows = s.query_map([], |r| r.get::<_, String>(0))?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }).unwrap();
    assert!(!cols.iter().any(|c| c.contains("pass") || c.contains("secret") || c.contains("token")), "account columns: {cols:?}");
}

#[test]
fn the_detection_cache_keeps_the_last_result_per_domain_and_manual_setup_clears_it() {
    let db = TempDb::new("setup-detect");
    let store = small_store(&db, 0);
    assert_eq!(store.detection("icloud.com").unwrap(), None);
    store.put_detection("iCloud.com".into(), CachedDetection::Found { config: icloud() }, 1000).unwrap();
    assert_eq!(store.detection("ICLOUD.COM").unwrap(), Some((CachedDetection::Found { config: icloud() }, 1000)), "the domain is matched ignoring case");
    store.put_detection("icloud.com".into(), CachedDetection::Refused { words: "No mail settings were found. Use Manual setup.".into() }, 2000).unwrap();
    assert_eq!(store.detection("icloud.com").unwrap(), Some((CachedDetection::Refused { words: "No mail settings were found. Use Manual setup.".into() }, 2000)), "the second result replaces the first");
    let n: i64 = store.write(|ctx| Ok(ctx.tx.query_row("SELECT count(*) FROM detection_cache", [], |r| r.get(0))?)).unwrap();
    assert_eq!(n, 1);
    // a config with awkward characters survives the JSON round trip
    let mut odd = icloud();
    odd.username = "o\"brien\\x\n@host".into();
    odd.source = DetectionSource::Guess;
    store.put_detection("odd.test".into(), CachedDetection::Found { config: odd.clone() }, 5).unwrap();
    assert_eq!(store.detection("odd.test").unwrap().unwrap().0, CachedDetection::Found { config: odd });
    assert!(store.clear_detection("icloud.com".into()).unwrap());
    assert!(!store.clear_detection("icloud.com".into()).unwrap(), "clearing twice reports nothing was there");
    assert_eq!(store.detection("icloud.com").unwrap(), None);
    // the table refuses a refusal that carries a config, and a result with no source
    let bad = store.write(|ctx| Ok(ctx.tx.execute("INSERT INTO detection_cache (domain, outcome, config_json, refusal_words, detected_at_ms) VALUES ('x.test','refused','{}','w',1)", [])?));
    assert!(bad.is_err());
}

#[test]
fn a_gmail_message_sits_once_with_a_row_per_label_folder() {
    let db = TempDb::new("setup-location");
    let store = small_store(&db, 2);
    let loc = |folder: &str, uid: i64| MessageLocation { account_id: "acc-a".into(), folder: folder.into(), uid_validity: 7, uid, message_id: "m000".into() };
    store.add_message_location(loc("INBOX", 11)).unwrap();
    store.add_message_location(loc("[Gmail]/All Mail", 90)).unwrap();
    store.add_message_location(loc("Clients", 3)).unwrap();
    store.add_message_location(loc("Clients", 3)).unwrap(); // the same copy again changes nothing
    let all = store.message_locations("m000").unwrap();
    assert_eq!(all.iter().map(|l| l.folder.as_str()).collect::<Vec<_>>(), ["Clients", "INBOX", "[Gmail]/All Mail"]);
    let messages: i64 = store.write(|ctx| Ok(ctx.tx.query_row("SELECT count(*) FROM message WHERE id = 'm000'", [], |r| r.get(0))?)).unwrap();
    assert_eq!(messages, 1, "one message row however many folders");
    // a UIDVALIDITY change on one folder drops that folder's rows; the message stays where its other copies are
    assert_eq!(store.forget_folder_locations("acc-a".into(), "INBOX".into()).unwrap(), Vec::<String>::new());
    assert_eq!(store.message_locations("m000").unwrap().len(), 2);
    store.forget_folder_locations("acc-a".into(), "Clients".into()).unwrap();
    assert_eq!(store.forget_folder_locations("acc-a".into(), "[Gmail]/All Mail".into()).unwrap(), vec!["m000".to_string()], "the last copy gone reports the message");
    // a location for a message that does not exist is refused
    assert!(store.add_message_location(MessageLocation { message_id: "m999".into(), ..loc("INBOX", 1) }).is_err());
}

#[test]
fn what_was_read_out_of_an_attachment_is_kept_replaced_and_dropped_with_the_attachment() {
    let db = TempDb::new("setup-content");
    let store = small_store(&db, 2);
    store.write(|ctx| {
        ctx.tx.execute("INSERT INTO attachment (id, message_id, file_name, mime_type, size_bytes) VALUES ('att-1','m000','warehouse-photo.jpg','image/jpeg',120000)", [])?;
        Ok(())
    }).unwrap();
    assert_eq!(store.attachment_content("att-1").unwrap(), None);
    let c = AttachmentContent { attachment_id: "att-1".into(), text: None, ocr_text: Some("Q1 brochure · 12,000 · 0.84 EUR".into()),
        labels: vec!["pallets".into(), "warehouse \"north\"".into(), "".into()], transcript: None, embedded: false, vector_key: None, updated_at_ms: 10 };
    store.put_attachment_content(c.clone()).unwrap();
    assert_eq!(store.attachment_content("att-1").unwrap(), Some(c.clone()));
    let d = AttachmentContent { embedded: true, labels: vec![], transcript: Some("the pallets arrive on Thursday".into()), updated_at_ms: 20, ..c };
    store.put_attachment_content(d.clone()).unwrap();
    assert_eq!(store.attachment_content("att-1").unwrap(), Some(d), "a second write replaces the first");
    assert!(store.put_attachment_content(AttachmentContent { attachment_id: "att-9".into(), text: None, ocr_text: None, labels: vec![], transcript: None, embedded: false, vector_key: None, updated_at_ms: 1 }).is_err(), "no attachment, no content");
    store.write(|ctx| { ctx.tx.execute("DELETE FROM attachment WHERE id = 'att-1'", [])?; Ok(()) }).unwrap();
    assert_eq!(store.attachment_content("att-1").unwrap(), None, "the content goes with the attachment");
}

#[test]
fn migration_0003_reverses_and_reapplies_without_touching_older_rows() {
    let db = TempDb::new("setup-revert");
    {
        let store = small_store(&db, 2);
        store.set_account_config("acc-a".into(), icloud()).unwrap();
    }
    let mut c = rusqlite::Connection::open(&db.0).unwrap();
    let before_accounts: i64 = c.query_row("SELECT count(*) FROM account", [], |r| r.get(0)).unwrap();
    let a = store::migrations::revert_to(&mut c, 2).unwrap();
    assert_eq!((a.from_version, a.to_version), (store::migrations::latest_version(), 2), "the reversal runs every newer migration's reversal too");
    let tables = store::migrations::table_names(&c).unwrap();
    assert!(!tables.iter().any(|t| ["detection_cache", "message_location", "attachment_content"].contains(&t.as_str())), "{tables:?}");
    assert_eq!(c.query_row::<i64, _, _>("SELECT count(*) FROM account", [], |r| r.get(0)).unwrap(), before_accounts);
    assert_eq!(c.query_row::<i64, _, _>("SELECT count(*) FROM message", [], |r| r.get(0)).unwrap(), 2);
    store::migrations::migrate(&mut c).unwrap();
    assert_eq!(c.query_row::<i64, _, _>("SELECT count(*) FROM account WHERE imap_host IS NOT NULL", [], |r| r.get(0)).unwrap(), 0, "the config does not survive a reversal");
}
