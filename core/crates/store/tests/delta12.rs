//! contracts delta 12 hook: a message's Cc and Bcc are stored from the contract's Message (0007's bcc_json).
mod common;
use common::*;

#[test]
fn cc_and_bcc_are_kept_with_the_message() {
    let db = TempDb::new("d12-cc-bcc");
    let s = small_store(&db, 2);
    let mut m = msg("m-cc", "t-cc", "acc-a", "owner0@example.test", 1_700_000_100_000);
    m.cc = vec!["tom@y.example".into(), "sara@z.example".into()];
    m.bcc = vec!["boss@x.example".into()];
    s.insert_messages(vec![new_msg(m, Some("primary"))]).unwrap();
    let row = s
        .write(|c| Ok(c.tx.query_row("SELECT cc_json, bcc_json FROM message WHERE id = 'm-cc'", [], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?))
        .unwrap();
    assert_eq!(row, (r#"["tom@y.example","sara@z.example"]"#.to_string(), r#"["boss@x.example"]"#.to_string()));
    let plain = s.write(|c| Ok(c.tx.query_row("SELECT cc_json, bcc_json FROM message WHERE id = 'm000'", [], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?)).unwrap();
    assert_eq!(plain, ("[]".to_string(), "[]".to_string()));
}

#[test]
fn a_threads_drafts_come_newest_first_with_their_header() {
    let db = TempDb::new("d12-draft-header");
    let s = small_store(&db, 2);
    s.write(|c| {
        c.tx.execute_batch(
            "INSERT INTO draft (id, from_account_id, to_json, cc_json, bcc_json, subject, body, reply_to_thread_id, updated_at_ms)
               VALUES ('d1', 'acc-a', '[\"a@x.example\"]', '[]', '[]', 'Re: first', '\n  Hello Tom.\n\nSecond line\nThird', 't000', 10);
             INSERT INTO draft (id, from_account_id, to_json, cc_json, bcc_json, subject, body, reply_to_thread_id, updated_at_ms)
               VALUES ('d2', 'acc-b', '[\"a@x.example\",\"b@x.example\"]', '[\"c@x.example\"]', '[\"boss@x.example\"]', 'Re: the quote', 'Edited', 't000', 20);
             INSERT INTO draft (id, from_account_id, subject, body, reply_to_thread_id, updated_at_ms) VALUES ('d3', 'acc-a', 's', 'elsewhere', 't999', 30);",
        )?;
        Ok(())
    })
    .unwrap();
    let d = s.draft_summaries("t000").unwrap();
    assert_eq!(d.iter().map(|x| x.id.as_str()).collect::<Vec<_>>(), ["d2", "d1"]);
    assert_eq!((d[0].to.clone(), d[0].cc.clone(), d[0].bcc.clone(), d[0].subject.as_str()),
               (vec!["a@x.example".to_string(), "b@x.example".into()], vec!["c@x.example".to_string()], vec!["boss@x.example".to_string()], "Re: the quote"));
    assert_eq!((d[0].account_id.as_str(), d[0].saved_at), ("acc-b", 20));
    assert_eq!(d[1].first_lines, "Hello Tom.\nSecond line");
    assert!(s.draft_summaries("t-none").unwrap().is_empty());
}

#[test]
fn a_model_filing_keeps_its_page_entry_and_an_owner_filing_never_does() {
    use contracts::DecidedBy;
    let db = TempDb::new("d12-entry");
    let s = small_store(&db, 4);
    let l = s.label_ensure_path("Clients".into(), vec![], 1).unwrap().label.id;
    s.label_file_entry(l.clone(), "m000".into(), DecidedBy::Laya, Some("labels/model".into()), Some(0.9), 2).unwrap();
    s.label_file_entry(l.clone(), "m001".into(), DecidedBy::Owner, Some("labels/model".into()), None, 2).unwrap();
    s.label_file(l.clone(), "m002".into(), DecidedBy::Jev, Some(0.9), 2).unwrap();
    let entry = |id: &'static str| { let l2 = l.clone(); s.write(move |c| Ok(c.tx.query_row("SELECT decided_entry_id FROM label_assignment WHERE label_id = ?1 AND message_id = ?2", rusqlite::params![l2, id], |r| r.get::<_, Option<String>>(0))?)).unwrap() };
    assert_eq!(entry("m000").as_deref(), Some("labels/model"));
    assert_eq!(entry("m001"), None, "the owner's filing names no model entry");
    assert_eq!(entry("m002"), None, "the old call writes none");
}

#[test]
fn a_smart_folder_is_read_back_and_its_rows_come_from_ids_not_a_tab() {
    let db = TempDb::new("d14-smart");
    let s = small_store(&db, 4);
    s.write(|c| { c.tx.execute("INSERT INTO smart_folder (id, name, rule_words, parent_id, updated_at_ms) VALUES ('sf-1', 'Quotes', 'quote', NULL, 1)", [])?; Ok(()) }).unwrap();
    let f = s.smart_folder("sf-1").unwrap().unwrap();
    assert_eq!((f.name.as_str(), f.rule_words.as_str()), ("Quotes", "quote"));
    assert!(s.smart_folder("sf-none").unwrap().is_none());
    let rows = s.rows_for(&["m002".into(), "gone".into(), "m000".into()]).unwrap();
    assert_eq!(rows.iter().map(|r| r.message_id.as_str()).collect::<Vec<_>>(), ["m002", "m000"], "in the order given, a gone id left out");
    s.write(|c| { c.tx.execute("UPDATE message SET archived = 1 WHERE id = 'm002'", [])?; Ok(()) }).unwrap();
    assert_eq!(s.rows_for(&["m002".into(), "m000".into()]).unwrap().len(), 1, "an archived message leaves the folder");
    let q = contracts::InboxQuery { tab_id: "primary".into(), account_id: None, label_id: None, mailbox: None, account_ids: vec![], filters: vec![], smart_folder_id: Some("sf-1".into()) };
    assert!(s.inbox(&q).is_err(), "a smart folder is never answered as a tab");
}

#[test]
fn an_account_and_a_detection_found_by_the_mail_record_are_kept_as_mx() {
    use contracts::{AccountConfig, AuthMethod, ConnectionSecurity, DetectionSource};
    let db = TempDb::new("d14-mx");
    let s = small_store(&db, 1);
    let cfg = AccountConfig { imap_host: "outlook.office365.com".into(), imap_port: 993, imap_security: ConnectionSecurity::Ssl, smtp_host: "smtp.office365.com".into(),
        smtp_port: 587, smtp_security: ConnectionSecurity::Starttls, username: "a@gpt.example".into(), smtp_username: "a@gpt.example".into(),
        auth_method: AuthMethod::Oauth2, source: DetectionSource::MailRecord };
    s.set_account_config("acc-a".into(), cfg.clone()).unwrap();
    assert_eq!(s.account_config("acc-a").unwrap(), Some(cfg.clone()), "0008 lets the store keep a mail-record account");
    let word: String = s.write(|c| Ok(c.tx.query_row("SELECT detection_source FROM account WHERE id = 'acc-a'", [], |r| r.get(0))?)).unwrap();
    assert_eq!(word, "mx");
    s.put_detection("gpt.example".into(), store::CachedDetection::Found { config: cfg.clone() }, 5).unwrap();
    assert_eq!(s.detection("gpt.example").unwrap().unwrap().0, store::CachedDetection::Found { config: cfg });
}
