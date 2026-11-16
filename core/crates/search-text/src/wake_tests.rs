//! indexer-race: a `reindex` that lands while a reopened index runs its exact comparison (WAKE_REPAIR, raised by every open of an
//! existing index) is served, never dropped. The worker's step is driven by hand on this thread, so the order is fixed: the open's
//! flags were taken, the ids arrive with their flag before the comparison's window ends.
//! Red-first broken copy: delete the line `if more & WAKE_IDS != 0 { reindex_pending(engine, shared)?; }` in `step` (lib.rs): the
//! comparison's `take_now` swallows WAKE_IDS, the ids stay in `pending` with no flag left, and this test fails at its assertion.
use super::*;
use contracts::Message;
use std::sync::atomic::AtomicBool;
use store::NewMessage;

fn store_with_one_mail(dir: &Path) -> Arc<Store> {
    let store = Arc::new(Store::open(&dir.join("whoosh.db")).unwrap());
    store
        .write(|ctx| {
            ctx.tx.execute(
                "INSERT INTO account (id, address, provider, display_name, colour, sort_order, credential_kind, keychain_ref, updated_at_ms) \
                 VALUES ('acc-a', 'owner@example.test', 'imap', 'A', '#336699', 0, 'manual', 'svc' || char(31) || 'a', 0)",
                [],
            )?;
            Ok(())
        })
        .unwrap();
    let message = Message {
        list_unsubscribe: vec![], list_unsubscribe_one_click: false, id: "m1".into(), thread_id: "t-m1".into(), account_id: "acc-a".into(),
        sender_name: "Sender".into(), from_address: "sender@ext.test".into(), to: vec!["owner@example.test".into()], subject: "Notes".into(),
        preview: "Preview".into(), received_at_ms: 1_700_000_000_000, read: false, archived: false, folder: "INBOX".into(), attachment_names: vec![],
        login_code: None, trackers_blocked: 0, reply_to_name: None, cc: vec![], bcc: vec![], is_outgoing: false,
    };
    let new = NewMessage { message, remote_id: None, body_text: Some("Body words.".into()), body_html: None, has_list_header: false, size_bytes: None, tab_id: Some("primary".into()) };
    store.insert_messages(vec![new]).unwrap();
    store
}

/// The worker's parts over the index an earlier open left in `folder` (what `open_with` builds for an existing index).
fn reopened(store: &Arc<Store>, folder: &Path) -> (Engine, Shared, Wake) {
    let (schema, fields) = schema::build();
    let (index, marks) = open_existing(folder, &schema).expect("the first open left a whole index");
    let reader: tantivy::IndexReader = index.reader_builder().reload_policy(ReloadPolicy::Manual).try_into().unwrap();
    let engine = Engine::new_with(&index, fields, reader.clone(), source::open_reader(store.path()).unwrap(), marks, limits::WRITER_THREADS, limits::WRITER_HEAP_BYTES).unwrap();
    let shared = Shared {
        index, fields, reader, snippets: Mutex::new(source::open_reader(store.path()).unwrap()), last_error: Mutex::new(None), commits: AtomicU64::new(0),
        pending: Mutex::new(Vec::new()), busy: AtomicBool::new(false),
    };
    (engine, shared, Wake { flags: Mutex::new(0), cv: Condvar::new() })
}

#[test]
fn ids_named_during_the_comparison_of_a_reopened_index_are_served() {
    let dir = tempfile::tempdir().unwrap();
    let store = store_with_one_mail(dir.path());
    let folder = dir.path().join("text");
    drop(TextIndex::open(&store, &folder).unwrap());
    let (mut engine, shared, wake) = reopened(&store, &folder);
    // the worker took the open's own flags; `TextIndex::reindex` (pending, then WAKE_IDS) lands before the window's `take_now`
    shared.pending.lock().unwrap().push("m1".to_string());
    wake.raise(WAKE_IDS);
    step(&mut engine, &shared, &wake, WAKE_INSERT | WAKE_REPAIR).unwrap();
    let left = shared.pending.lock().unwrap().len();
    let still_signalled = wake.take_now() & WAKE_IDS != 0;
    assert!(left == 0 || still_signalled, "{left} id(s) wait in pending with no WAKE_IDS left for the worker: pending() never reaches 0");
}
