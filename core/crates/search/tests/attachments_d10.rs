//! contracts delta 10 hook: words inside attachments and their names join the search, ranked with the mail and named.
use attachment_index::{CatalogRow, IndexState, VectorMatch};
use contracts::{CoreError, MatchedIn, Message, SearchHit};
use search::*;
use std::sync::Arc;

struct Mail(Vec<Message>);
impl TextPort for Mail {
    fn execute(&self, words: &str, _l: usize) -> Result<Vec<SearchHit>, CoreError> {
        Ok(self.0.iter().filter(|m| m.preview.contains(words)).map(|m| SearchHit { message_id: m.id.clone(), score: 1.0, snippet: m.preview.clone(), matched_in: MatchedIn::MeaningOnly, attachment_name: None, attachment_note: None }).collect())
    }
}
impl MeaningPort for Mail { fn execute(&self, _w: &str, _l: usize) -> Result<Vec<SearchHit>, CoreError> { Ok(vec![]) } }
impl FactsPort for Mail { fn execute(&self, ids: &[String]) -> Result<Vec<Message>, CoreError> { Ok(self.0.iter().filter(|m| ids.contains(&m.id)).cloned().collect()) } }
impl FilterScanPort for Mail { fn execute(&self, _f: &Filters, _l: usize) -> Result<Vec<String>, CoreError> { Ok(vec![]) } }
impl AccountsPort for Mail { fn execute(&self) -> Result<Vec<AccountState>, CoreError> { Ok(vec![]) } }

struct Files(Vec<CatalogRow>);
impl AttachmentPort for Files {
    fn execute(&self, words: &str, _l: usize) -> Result<Vec<SearchHit>, CoreError> { Ok(attachment_hits(words, &self.0, &[])) }
}

fn msg(id: &str, preview: &str) -> Message {
    Message { id: id.into(), thread_id: format!("t-{id}"), account_id: "a".into(), sender_name: "S".into(), from_address: "s@x.test".into(), to: vec![],
        subject: format!("Subject {id}"), preview: preview.into(), received_at_ms: 1, read: false, archived: false, folder: "INBOX".into(), attachment_names: vec![],
        login_code: None, trackers_blocked: 0, reply_to_name: None, list_unsubscribe: vec![], list_unsubscribe_one_click: false, cc: vec![], bcc: vec![], is_outgoing: false }
}

fn row(m: &str, a: &str, name: &str, state: IndexState, text: &str) -> CatalogRow {
    CatalogRow { message_id: m.into(), attachment_id: a.into(), file_name: name.into(), state, text: text.into() }
}

fn searcher(files: Option<Vec<CatalogRow>>) -> Searcher {
    let mail = Arc::new(Mail(vec![msg("m1", "the invoice for the print run"), msg("m2", "photos from the warehouse"), msg("m3", "see the scan")]));
    Searcher { text: mail.clone(), meaning: mail.clone(), facts: mail.clone(), scan: mail.clone(), accounts: mail, attachments: files.map(|f| Arc::new(Files(f)) as Arc<dyn AttachmentPort>) }
}

fn run(s: &Searcher, q: &str) -> SearchOutcome { s.run(&SearchRequest { text: q, today: CivilDate { year: 2026, month: 11, day: 8 }, utc_offset_min: 60 }).unwrap() }

fn catalog() -> Vec<CatalogRow> {
    vec![row("m2", "a1", "warehouse-photo.jpg", IndexState::Indexed, "pallets of brochures, a forklift in the aisle"), row("m3", "a2", "scan-notes.pdf", IndexState::NotDownloaded, "")]
}

#[test]
fn a_word_inside_an_attachment_finds_its_message_and_names_the_file() {
    let out = run(&searcher(Some(catalog())), "forklift");
    let m = out.threads.iter().flat_map(|t| &t.matches).find(|m| m.message_id == "m2").expect("the photo's message is found");
    let a = m.attachment.as_ref().expect("the attachment is named");
    assert_eq!((a.matched_in.clone(), a.attachment_name.as_deref()), (MatchedIn::AttachmentContent, Some("warehouse-photo.jpg")));
}

#[test]
fn a_name_hit_on_a_file_not_downloaded_is_found_by_name_only() {
    let out = run(&searcher(Some(catalog())), "scan notes");
    let m = out.threads.iter().flat_map(|t| &t.matches).find(|m| m.message_id == "m3").expect("found by the file name");
    assert_eq!(m.attachment.as_ref().map(|a| a.matched_in.clone()), Some(MatchedIn::AttachmentName));
}

#[test]
fn without_the_attachment_port_the_search_reads_mail_only_and_a_phrase_skips_attachments() {
    assert!(run(&searcher(None), "forklift").threads.is_empty());
    assert!(run(&searcher(Some(catalog())), "\"forklift\"").threads.is_empty(), "an exact phrase is the text index's alone");
    assert_eq!(run(&searcher(Some(catalog())), "invoice").threads.len(), 1, "mail hits still come");
}

#[test]
fn an_attachment_hit_adds_to_a_mail_hit_on_the_same_message() {
    let fused = add_ranked_list(fuse_ranks(&["m1".to_string(), "m2".to_string()], &[]), &["m2".to_string()]);
    assert_eq!(fused[0].message_id, "m2", "text rank 2 plus attachment rank 1 outranks text rank 1 alone");
    let _ = VectorMatch { key: "m2/a1#i".into(), score: 0.9 };
}
