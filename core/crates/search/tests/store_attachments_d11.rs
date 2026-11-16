//! contracts-d11 hook: the real core's searcher takes the attachment port over the store and the vector index.
use contracts::{CoreError, Embedder, MatchedIn, MediaKind, Message, SearchHit};
use search::*;
use search_vector::{constants::EMBED_DIM, VectorIndex};
use std::sync::Arc;

struct Axis;
fn axis(i: usize) -> Vec<f32> { let mut v = vec![0.0; EMBED_DIM]; v[i] = 1.0; v }
impl Embedder for Axis {
    fn dimension(&self) -> u32 { EMBED_DIM as u32 }
    fn embed(&self, t: Vec<String>) -> Result<Vec<Vec<f32>>, CoreError> { Ok(t.iter().map(|_| axis(1)).collect()) }
    fn embed_titled(&self, _title: Option<String>, texts: Vec<String>) -> Result<Vec<Vec<f32>>, CoreError> { self.embed(texts) } // mechanical (contracts delta 12)
    fn embed_query(&self, text: String) -> Result<Vec<f32>, CoreError> { Ok(axis(if text.contains("pallets") { 3 } else { 4 })) }
    fn embed_media(&self, _k: MediaKind, _p: String) -> Result<Vec<f32>, CoreError> { Ok(axis(3)) }
}

struct Nothing(Arc<store::Store>);
impl TextPort for Nothing { fn execute(&self, _w: &str, _l: usize) -> Result<Vec<SearchHit>, CoreError> { Ok(vec![]) } }
impl MeaningPort for Nothing { fn execute(&self, _w: &str, _l: usize) -> Result<Vec<SearchHit>, CoreError> { Ok(vec![]) } }
impl FilterScanPort for Nothing { fn execute(&self, _f: &Filters, _l: usize) -> Result<Vec<String>, CoreError> { Ok(vec![]) } }
impl AccountsPort for Nothing { fn execute(&self) -> Result<Vec<AccountState>, CoreError> { Ok(vec![]) } }
impl FactsPort for Nothing {
    fn execute(&self, ids: &[String]) -> Result<Vec<Message>, CoreError> {
        Ok(ids.iter().map(|id| Message { id: id.clone(), thread_id: format!("t-{id}"), account_id: "acc-a".into(), sender_name: "S".into(), from_address: "s@x.test".into(),
            to: vec![], subject: format!("Subject {id}"), preview: "p".into(), received_at_ms: 1, read: false, archived: false, folder: "INBOX".into(), attachment_names: vec![],
            login_code: None, trackers_blocked: 0, reply_to_name: None, list_unsubscribe: vec![], list_unsubscribe_one_click: false, cc: vec![], bcc: vec![], is_outgoing: false }).collect())
    }
}

fn store_with_attachments(dir: &std::path::Path) -> Arc<store::Store> {
    let s = Arc::new(store::Store::open(&dir.join("w.db")).unwrap());
    s.write(|c| {
        c.tx.execute("INSERT INTO account (id, address, provider, display_name, colour, sort_order, credential_kind, keychain_ref, updated_at_ms) VALUES ('acc-a','o@x.test','imap','A','#336699',0,'manual','svc' || char(31) || 'acc-a',0)", [])?;
        c.tx.execute("INSERT INTO thread (id, account_id, subject, participants_json, last_message_at_ms) VALUES ('t1','acc-a','s','[]',1)", [])?;
        for m in ["m1", "m2"] {
            c.tx.execute("INSERT INTO message (id, thread_id, account_id, sender_name, from_address, to_json, cc_json, subject, preview, received_at_ms, read, archived, folder) VALUES (?1,'t1','acc-a','S','s@x.test','[]','[]','s','p',1,0,0,'INBOX')", [m])?;
        }
        c.tx.execute("INSERT INTO attachment (id, message_id, file_name, mime_type, size_bytes, local_path, extracted_text) VALUES ('a1','m1','warehouse-photo.jpg','image/jpeg',1,'/x','a forklift in the aisle')", [])?;
        c.tx.execute("INSERT INTO attachment_content (attachment_id, text, embedded, updated_at_ms) VALUES ('a1','a forklift in the aisle',1,1)", [])?;
        c.tx.execute("INSERT INTO attachment (id, message_id, file_name, mime_type, size_bytes, local_path) VALUES ('a2','m2','scan.jpg','image/jpeg',1,'/y')", [])?;
        c.tx.execute("INSERT INTO attachment_content (attachment_id, ocr_text, embedded, updated_at_ms) VALUES ('a2','',1,1)", [])?;
        Ok(())
    }).unwrap();
    s
}

fn searcher(s: Arc<store::Store>, v: Option<Arc<VectorIndex>>) -> Searcher {
    let n = Arc::new(Nothing(s.clone()));
    Searcher { text: n.clone(), meaning: n.clone(), facts: n.clone(), scan: n.clone(), accounts: n, attachments: Some(Arc::new(StoreAttachments { store: s, vectors: v })) }
}

fn run(s: &Searcher, q: &str) -> SearchOutcome { s.run(&SearchRequest { text: q, today: CivilDate { year: 2026, month: 11, day: 8 }, utc_offset_min: 60 }).unwrap() }
fn hit<'a>(o: &'a SearchOutcome, id: &str) -> Option<&'a MessageMatch> { o.threads.iter().flat_map(|t| &t.matches).find(|m| m.message_id == id) }

#[test]
fn a_word_inside_a_stored_attachment_finds_its_message_through_the_real_port() {
    let dir = tempfile::tempdir().unwrap();
    let s = store_with_attachments(dir.path());
    let out = run(&searcher(s, None), "forklift");
    let m = hit(&out, "m1").expect("found through the store's catalog");
    let a = m.attachment.as_ref().expect("named");
    assert_eq!((a.matched_in.clone(), a.attachment_name.as_deref()), (MatchedIn::AttachmentContent, Some("warehouse-photo.jpg")));
    assert!(hit(&out, "m2").is_none());
}

#[test]
fn a_meaning_match_under_a_text_key_joins_by_its_message() {
    let dir = tempfile::tempdir().unwrap();
    let s = store_with_attachments(dir.path());
    let v = Arc::new(VectorIndex::open(&dir.path().join("vec"), Arc::new(Axis)).unwrap());
    v.add_vector("m2/a2#i", &axis(3)).unwrap();
    let out = run(&searcher(s, Some(v)), "pallets");
    assert!(hit(&out, "m2").and_then(|m| m.attachment.as_ref()).is_some(), "the picture found by what it shows: {:?}", out.threads);
}
