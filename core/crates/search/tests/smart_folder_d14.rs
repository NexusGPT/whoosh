//! contracts delta 14: a smart folder opens its list in the real core: the store's saved words, searched, then the store's
//! own rows, one per thread, newest first, with the unread count from the same rows.
use contracts::{CoreError, MatchedIn, Message, SearchHit};
use search::*;
use std::sync::Arc;

struct Quotes;
fn hit(id: &str) -> SearchHit { SearchHit { message_id: id.into(), score: 1.0, snippet: String::new(), matched_in: MatchedIn::MeaningOnly, attachment_name: None, attachment_note: None } }
impl TextPort for Quotes { fn execute(&self, w: &str, _l: usize) -> Result<Vec<SearchHit>, CoreError> { Ok(if w.contains("quote") { vec![hit("m1"), hit("m2")] } else { vec![] }) } }
impl MeaningPort for Quotes { fn execute(&self, _w: &str, _l: usize) -> Result<Vec<SearchHit>, CoreError> { Ok(vec![]) } }
impl FilterScanPort for Quotes { fn execute(&self, _f: &Filters, _l: usize) -> Result<Vec<String>, CoreError> { Ok(vec![]) } }
impl AccountsPort for Quotes { fn execute(&self) -> Result<Vec<AccountState>, CoreError> { Ok(vec![]) } }
impl FactsPort for Quotes {
    fn execute(&self, ids: &[String]) -> Result<Vec<Message>, CoreError> {
        Ok(ids.iter().map(|id| Message { id: id.clone(), thread_id: format!("t-{id}"), account_id: "acc-a".into(), sender_name: "S".into(), from_address: "s@x.test".into(),
            to: vec![], subject: "A quote".into(), preview: "p".into(), received_at_ms: 1, read: false, archived: false, folder: "INBOX".into(), attachment_names: vec![],
            login_code: None, trackers_blocked: 0, reply_to_name: None, list_unsubscribe: vec![], list_unsubscribe_one_click: false, cc: vec![], bcc: vec![], is_outgoing: false }).collect())
    }
}

#[test]
fn a_smart_folder_lists_its_saved_search_newest_first_with_the_same_unread_count() {
    let dir = tempfile::tempdir().unwrap();
    let s = Arc::new(store::Store::open(&dir.path().join("w.db")).unwrap());
    s.write(|c| {
        c.tx.execute("INSERT INTO account (id, address, provider, display_name, colour, sort_order, credential_kind, keychain_ref, updated_at_ms) VALUES ('acc-a','o@x.test','imap','A','#336699',0,'manual','svc' || char(31) || 'acc-a',0)", [])?;
        for (m, t, at) in [("m1", "t1", 10), ("m2", "t2", 20), ("m3", "t3", 30)] {
            c.tx.execute("INSERT INTO thread (id, account_id, subject, participants_json, last_message_at_ms) VALUES (?1,'acc-a','s','[]',?2)", rusqlite::params![t, at])?;
            c.tx.execute("INSERT INTO message (id, thread_id, account_id, sender_name, from_address, to_json, cc_json, subject, preview, received_at_ms, read, archived, folder) VALUES (?1,?2,'acc-a','S','s@x.test','[]','[]','A quote','p',?3,0,0,'INBOX')", rusqlite::params![m, t, at])?;
        }
        c.tx.execute("INSERT INTO smart_folder (id, name, rule_words, parent_id, updated_at_ms) VALUES ('sf-1','Quotes','quote',NULL,1)", [])?;
        Ok(())
    }).unwrap();
    let q = Arc::new(Quotes);
    let searcher = Searcher { text: q.clone(), meaning: q.clone(), facts: q.clone(), scan: q.clone(), accounts: q, attachments: None };
    let req = SearchRequest { text: "", today: CivilDate::new(2026, 11, 9).unwrap(), utc_offset_min: 0 };
    let v = smart_folder_rows(&searcher, &s, "sf-1", &req).unwrap();
    assert_eq!(v.rows.iter().map(|r| r.message_id.as_str()).collect::<Vec<_>>(), ["m2", "m1"], "the saved search's rows, newest first; m3 is not a match");
    assert_eq!(v.message_ids, ["m2", "m1"]);
    assert_eq!((v.count, v.unread), (2, 2));
    s.write(|c| { c.tx.execute("UPDATE message SET read = 1 WHERE id = 'm2'", [])?; Ok(()) }).unwrap();
    assert_eq!(smart_folder_rows(&searcher, &s, "sf-1", &req).unwrap().unread, 1, "unread follows the same rows");
    s.write(|c| { c.tx.execute("UPDATE message SET archived = 1 WHERE id = 'm2'", [])?; Ok(()) }).unwrap();
    let after = smart_folder_rows(&searcher, &s, "sf-1", &req).unwrap();
    assert_eq!((after.count, after.message_ids.clone()), (1, vec!["m1".to_string()]), "an archive takes the row out of the folder and its count");
    assert!(matches!(smart_folder_rows(&searcher, &s, "sf-none", &req), Err(CoreError::NotFound { .. })));
}
