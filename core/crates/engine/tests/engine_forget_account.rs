//! fix-blockforget (docs/lanes/fix-blockforget.md; docs/lanes/real-accounts.md findings 2 and 5): RemoveAccount on the REAL engine takes the
//! account off the people list at once, the crawl and the two search indexes forget it (`forget_account` in people.rs, crawl.rs, indexer.rs),
//! a search after the removal finds none of its mail, and the undo entries of its messages go with it so Z reverses the newest entry of
//! another account. Every test runs a store on this Mac with no mail server and no socket; the secrets are the in-memory keychain's (never
//! read out or printed). Each test prints `PASS <case>` lines and `TIME <what> <ms> ms` for each command on a store of 2,000 mails (the bar
//! is 50 ms; `-- --nocapture`, release profile, one test at a time for the timed ones).
//!
//! BROKEN COPIES (each `find` occurs once in its file; the named test goes red):
//! - F1 core/crates/engine/src/people.rs `let from = p.from_per_account.remove(account_id).unwrap_or(0).min(n);` => `let from = 0;`:
//!   the_people_of_a_removed_account_leave_the_list_at_once_and_the_shared_ones_keep_the_other_accounts_counts
//! - F2 core/crates/engine/src/people.rs `            p.total() > 0` => `            true`: the same test (a person known only through the account stays, at 0)
//! - F3 core/crates/engine/src/account_cmds.rs `        self.people.forget_account(account_id);` => `` (deleted): the same test; red when the first
//!   read takes the book's lock before the rebuild thread replaces the book (the rebuild reads every row first, so nearly always)
//! - F4 core/crates/engine/src/indexer.rs `if vector.contains_message(key) && vector.remove_message(key).is_ok() { removed += 1; }` =>
//!   `if vector.contains_message(key) { removed += 1; }`: the_indexer_forgets_the_vectors_of_a_removed_accounts_chunks_and_keeps_the_others
//! - F5 core/crates/engine/src/account_cmds.rs `        self.deps.bodies.forget_account(account_id, &message_ids, &chunk_ids);` => `` (deleted):
//!   a_search_after_the_removal_finds_none_of_its_mail_and_its_vectors_are_gone (a second forget then still finds A's vectors)
//! - F6 core/crates/store/src/realcore.rs `DELETE FROM undo_entry WHERE EXISTS (SELECT 1 FROM json_tree(undo_entry.action_json) j WHERE j.type = 'text'`
//!   => `DELETE FROM undo_entry WHERE 0 AND EXISTS (SELECT 1 FROM json_tree(undo_entry.action_json) j WHERE j.type = 'text'`:
//!   the_undo_entries_of_a_removed_account_go_with_it_and_z_reverses_the_other_accounts_newest
//! - F7 (the search's own filter, not changed by this lane) core/crates/search-text/src/query.rs `let Some(body) = body_of(&id) else { continue };`
//!   => `let body = body_of(&id).unwrap_or_default();`: a_search_after_the_removal_finds_none_of_its_mail_and_its_vectors_are_gone
use std::sync::Arc;
use std::time::{Duration, Instant};

use contracts::*;
use engine::bodies::indexer::Indexer;
use engine::{RustCore, TestTransport};
use secrets::{MemoryKeychain, Namespace, Secret, SecretKind, StoreSecret};
use store::{NewMessage, Store};

#[allow(dead_code, unused_imports)]
#[path = "../../autoconfig/tests/support/mod.rs"]
mod detect_fakes;
use detect_fakes::{FakeDns, FakeHttp, FakeProbe};

const WAIT: Duration = Duration::from_secs(120);
const ADDR: [&str; 2] = ["alpha@example.test", "beta@example.test"];

fn detector() -> Box<dyn engine::Detect> { Box::new(autoconfig::Detector::new(FakeHttp::default(), FakeDns::default(), FakeProbe::default())) }
fn config(user: &str) -> AccountConfig {
    AccountConfig {
        imap_host: "127.0.0.1".into(), imap_port: 1, imap_security: ConnectionSecurity::Ssl, smtp_host: "127.0.0.1".into(), smtp_port: 1,
        smtp_security: ConnectionSecurity::Starttls, username: user.into(), smtp_username: user.into(), auth_method: AuthMethod::Password, source: DetectionSource::Manual,
    }
}
fn timed<T>(what: &str, f: impl FnOnce() -> T) -> T {
    let t0 = Instant::now();
    let r = f();
    let ms = t0.elapsed().as_secs_f64() * 1000.0;
    println!("TIME {what} {ms:.1} ms");
    assert!(ms < 50.0, "{what} took {ms:.1} ms, the bar is 50");
    r
}
fn wait_until(what: &str, mut ok: impl FnMut() -> bool) {
    let t0 = Instant::now();
    while t0.elapsed() < WAIT { if ok() { return; } std::thread::sleep(Duration::from_millis(20)); }
    panic!("never reached within {WAIT:?}: {what}");
}
fn mail(account: &str, id: &str, from: &str, to: &[&str], at: i64, body: Option<String>) -> NewMessage {
    NewMessage {
        message: Message {
            id: id.into(), thread_id: format!("{id}:thread"), account_id: account.into(), sender_name: "Sender".into(), from_address: from.into(),
            to: to.iter().map(|t| t.to_string()).collect(), subject: format!("Subject {id}"), preview: String::new(), received_at_ms: at, read: false, archived: false,
            folder: "INBOX".into(), attachment_names: vec![], login_code: None, trackers_blocked: 0, list_unsubscribe: vec![], list_unsubscribe_one_click: false,
            reply_to_name: None, cc: vec![], bcc: vec![], is_outgoing: false,
        },
        remote_id: Some(at.to_string()), body_text: body, body_html: None, has_list_header: false, size_bytes: None, tab_id: None,
    }
}

/// The test embedder: one direction per word (a hash), so a body's vector is close to a query holding its words. Dimension EMBED_DIM.
struct WordEmbedder;
fn vector(text: &str) -> Vec<f32> {
    let dim = search_vector::constants::EMBED_DIM;
    let mut v = vec![0.001f32; dim];
    for w in text.to_lowercase().split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()) {
        v[w.bytes().fold(7usize, |h, b| h.wrapping_mul(31).wrapping_add(b as usize)) % dim] += 1.0;
    }
    v
}
impl Embedder for WordEmbedder {
    fn dimension(&self) -> u32 { search_vector::constants::EMBED_DIM as u32 }
    fn embed(&self, texts: Vec<String>) -> Result<Vec<Vec<f32>>, CoreError> { Ok(texts.iter().map(|t| vector(t)).collect()) }
    fn embed_query(&self, text: String) -> Result<Vec<f32>, CoreError> { Ok(vector(&text)) }
    fn embed_titled(&self, _title: Option<String>, texts: Vec<String>) -> Result<Vec<Vec<f32>>, CoreError> { self.embed(texts) }
    fn embed_media(&self, _kind: MediaKind, path: String) -> Result<Vec<f32>, CoreError> { Ok(vector(&path)) }
}

/// Two accounts on one store. A (alpha) holds 2,000 mails: 1,996 from 50 senders of alpha-mail.test (the first 30 with the body
/// "zebracrossing alpha"), 3 from both@shared.test, and 1 the owner of A sent to only-a@x.test and to-shared@x.test. B (beta) holds 43: 40 from
/// 5 senders of beta-mail.test (the first 10 with the body "zebracrossing beta"), 2 from both@shared.test, and 1 the owner of B sent to
/// to-shared@x.test. Each account's password is in the in-memory keychain (the removal deletes it).
struct World { core: RustCore, kc: Arc<MemoryKeychain>, a: String, b: String, dir: tempfile::TempDir }

fn store_with_two_accounts(path: &std::path::Path, kc: &MemoryKeychain) -> (Store, String, String) {
    let store = Store::open(path).unwrap();
    let mut ids = vec![];
    for (i, addr) in ADDR.iter().enumerate() {
        let r = StoreSecret::execute(kc, SecretKind::AccountCredential, addr, &Secret::new(b"app-password".to_vec())).unwrap();
        ids.push(store.add_account(addr, &format!("Name {i}"), MailProvider::Imap, &format!("{}\u{1f}{}", r.service, r.account), false, config(addr), 10 + i as i64).unwrap());
    }
    let (a, b) = (ids[0].clone(), ids[1].clone());
    let mut batch = Vec::new();
    for i in 0..1996 {
        let body = (i < 30).then(|| format!("zebracrossing alpha number {i}"));
        batch.push(mail(&a, &format!("a{i}"), &format!("asender{}@alpha-mail.test", i % 50), &[ADDR[0]], 1_000 + i as i64, body));
    }
    for i in 0..3 { batch.push(mail(&a, &format!("as{i}"), "both@shared.test", &[ADDR[0]], 5_000 + i, None)); }
    batch.push(mail(&a, "a-sent", ADDR[0], &["Only A <only-a@x.test>", "Shared To <to-shared@x.test>"], 6_000, None));
    for i in 0..40 {
        let body = (i < 10).then(|| format!("zebracrossing beta number {i}"));
        batch.push(mail(&b, &format!("b{i}"), &format!("bsender{}@beta-mail.test", i % 5), &[ADDR[1]], 1_000 + i as i64, body));
    }
    for i in 0..2 { batch.push(mail(&b, &format!("bs{i}"), "both@shared.test", &[ADDR[1]], 5_000 + i, None)); }
    batch.push(mail(&b, "b-sent", ADDR[1], &["to-shared@x.test"], 6_001, None));
    store.insert_messages(batch).unwrap();
    (store, a, b)
}

fn world() -> World {
    let dir = tempfile::tempdir().unwrap();
    let kc = Arc::new(MemoryKeychain::new(Namespace::rig()));
    let (store, a, b) = store_with_two_accounts(&dir.path().join("whoosh.sqlite"), &kc);
    World { core: RustCore::with_parts(store, kc.clone(), detector(), TestTransport::PlainLoopback), kc, a, b, dir }
}
fn restart(w: World) -> World {
    let World { core, kc, a, b, dir } = w;
    drop(core);
    let store = Store::open(&dir.path().join("whoosh.sqlite")).unwrap();
    World { core: RustCore::with_parts(store, kc.clone(), detector(), TestTransport::PlainLoopback), kc, a, b, dir }
}
fn st(core: &RustCore, p: &str) -> Option<String> { core.state(p.to_string()) }
fn count(core: &RustCore, sql: &str, arg: &str) -> i64 {
    let (sql, arg) = (sql.to_string(), arg.to_string());
    core.store().write(move |c| Ok(c.tx.query_row(&sql, [&arg], |r| r.get::<_, i64>(0))?)).unwrap()
}
fn people_of(core: &RustCore, domain: &str) -> usize { core.contacts().iter().filter(|c| c.addresses.iter().any(|a| a.ends_with(domain))).count() }
fn remove(core: &RustCore, id: &str) { timed("RemoveAccount (2,000 mails)", || core.run(Command::RemoveAccount { account_id: id.into() })).expect("removed"); }

// ---------------------------------------------------------------- the people list (finding 2)

#[test]
fn the_people_of_a_removed_account_leave_the_list_at_once_and_the_shared_ones_keep_the_other_accounts_counts() {
    let w = world();
    wait_until("the people list is built", || st(&w.core, "people/ready").as_deref() == Some("true"));
    assert_eq!(people_of(&w.core, "@alpha-mail.test"), 50, "A's 50 senders are people before");
    assert!(st(&w.core, "people/address/both@shared.test").unwrap().contains("\"from_count\":5"), "{:?}", st(&w.core, "people/address/both@shared.test"));
    assert!(st(&w.core, "people/address/to-shared@x.test").unwrap().contains("\"to_count\":2"), "{:?}", st(&w.core, "people/address/to-shared@x.test"));
    assert!(st(&w.core, "people/address/only-a@x.test").unwrap().contains("\"to_count\":1"));
    assert_eq!(st(&w.core, "people/count").as_deref(), Some("58"), "50 + 5 senders, both, only-a, to-shared");

    remove(&w.core, &w.a);
    // the very next reads, no wait: none of A's people, the shared ones with B's counts only
    assert_eq!(people_of(&w.core, "@alpha-mail.test"), 0, "A's senders left the list at once");
    assert!(w.core.contact("asender0@alpha-mail.test".into()).is_none());
    assert_eq!(st(&w.core, "people/address/only-a@x.test").as_deref(), Some("null"), "a person A's owner wrote to, and nobody else, left");
    let both = st(&w.core, "people/address/both@shared.test").unwrap();
    assert!(both.contains("\"from_count\":2") && both.contains("\"to_count\":0"), "B's two mails only: {both}");
    let shared = st(&w.core, "people/address/to-shared@x.test").unwrap();
    assert!(shared.contains("\"to_count\":1") && shared.contains("\"from_count\":0"), "B's one sent mail only: {shared}");
    assert_eq!(st(&w.core, "people/count").as_deref(), Some("7"), "B's 5 senders, both, to-shared");
    assert!(w.core.contacts().iter().all(|c| !c.mail_count_per_account.contains_key(&w.a)), "no person counts mail of A");
    assert_eq!(people_of(&w.core, "@beta-mail.test"), 5, "B's people stay");
    println!("PASS people-of-a-removed-account-leave-at-once-shared-people-keep-the-other-accounts-counts");
    // the rebuild the removal started reads the store: the same answer
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(st(&w.core, "people/count").as_deref(), Some("7"), "after the rebuild");
    let w = restart(w);
    wait_until("the people list is built again", || st(&w.core, "people/ready").as_deref() == Some("true"));
    assert_eq!(people_of(&w.core, "@alpha-mail.test"), 0);
    assert_eq!(st(&w.core, "people/count").as_deref(), Some("7"), "after a restart");
    assert!(st(&w.core, "people/address/both@shared.test").unwrap().contains("\"from_count\":2"));
    println!("PASS people-of-a-removed-account-stay-gone-after-a-rebuild-and-a-restart");
}

// ---------------------------------------------------------------- the search and the two indexes (finding 2)

#[test]
fn a_search_after_the_removal_finds_none_of_its_mail_and_its_vectors_are_gone() {
    let w = world();
    let bg = w.core.background();
    bg.set_embedder(Some(Arc::new(WordEmbedder)));
    assert!(bg.settle_index(WAIT), "the indexer finished");
    let by_account = |hits: Vec<SearchHit>| -> (usize, usize) {
        (hits.iter().filter(|h| h.message_id.starts_with('a')).count(), hits.iter().filter(|h| h.message_id.starts_with('b')).count())
    };
    wait_until("the word index holds the 40 bodies", || by_account(bg.search_words("zebracrossing", 100)) == (30, 10));
    let meaning = bg.search_meaning("zebracrossing alpha", 40).expect("a meaning search");
    assert!(meaning.iter().any(|h| h.account_id == w.a), "A's pieces are found by meaning before");
    assert_eq!(w.core.rule_match_count("zebracrossing".into()), 40, "the owner's search counts 40 before");
    let (a_messages, a_chunks) = w.core.store().account_index_rows(&w.a).unwrap();
    assert!(a_chunks.len() >= 30, "A's 30 bodies have pieces ({})", a_chunks.len());

    remove(&w.core, &w.a);
    // the very next searches, no wait
    assert_eq!(by_account(bg.search_words("zebracrossing", 100)), (0, 10), "the word search finds B's ten only");
    let meaning = bg.search_meaning("zebracrossing alpha", 40).expect("a meaning search");
    assert!(meaning.iter().all(|h| h.account_id == w.b), "the meaning search finds none of A's pieces");
    assert_eq!(w.core.rule_match_count("zebracrossing".into()), 10, "the owner's search counts B's ten");
    // the removal already took A's vectors out of the core's meaning index: forgetting them again finds none (Bodies::forget_account
    // answers how many vectors it removed; the core's indexer is not reachable from a test otherwise)
    assert_eq!(bg.forget_account(&w.a, &a_messages, &a_chunks), 0, "no vector of A's pieces was left in the meaning index");
    println!("PASS a-search-after-the-removal-finds-none-of-its-mail-and-its-vectors-are-gone");
    // round 6: the first core's background (its indexer and word index writer, which holds the index folder's lock) goes before the
    // restart; a `bg` kept alive here made the reopened word index answer LockBusy
    drop(bg);
    let w = restart(w);
    let bg = w.core.background();
    bg.set_embedder(Some(Arc::new(WordEmbedder)));
    assert!(bg.settle_index(WAIT));
    assert!(bg.open_word_index(), "the word index opens from its folder (a restart with nothing new to index does not open it on its own)");
    wait_until("the reopened word index answers", || by_account(bg.search_words("zebracrossing", 100)) == (0, 10));
    assert_eq!(by_account(bg.search_words("zebracrossing", 100)), (0, 10), "after a restart");
    assert_eq!(w.core.rule_match_count("zebracrossing".into()), 10);
    println!("PASS a-search-after-the-removal-and-a-restart-finds-none-of-its-mail");
}

// ---------------------------------------------------------------- the indexer alone (finding 2)

#[test]
fn the_indexer_forgets_the_vectors_of_a_removed_accounts_chunks_and_keeps_the_others() {
    let dir = tempfile::tempdir().unwrap();
    let kc = MemoryKeychain::new(Namespace::rig());
    let (store, a, b) = store_with_two_accounts(&dir.path().join("whoosh.sqlite"), &kc);
    let store = Arc::new(store);
    let indexer = Indexer::start(store.clone(), dir.path().join("indexer.log"));
    indexer.set_embedder(Some(Arc::new(WordEmbedder)));
    assert!(indexer.settle(WAIT), "the indexer finished");
    let (a_messages, a_chunks) = store.account_index_rows(&a).unwrap();
    let (_, b_chunks) = store.account_index_rows(&b).unwrap();
    assert_eq!(a_messages.len(), 2_000, "A holds 2,000 mails");
    assert!(a_chunks.len() >= 30 && b_chunks.len() >= 10, "pieces: A {} B {}", a_chunks.len(), b_chunks.len());
    let keys = |ids: &[i64]| ids.iter().map(|c| chunk_vector_key(*c)).collect::<Vec<u64>>();
    let query = vector("zebracrossing");
    let held = |ids: &[i64]| indexer.chunk_cosines(&query, &keys(ids)).unwrap().expect("a meaning index").into_iter().filter(|c| c.is_some()).count();
    assert_eq!((held(&a_chunks), held(&b_chunks)), (a_chunks.len(), b_chunks.len()), "every piece has its vector before");

    let removed = timed("Indexer::forget_account (2,000 mails)", || indexer.forget_account(&a_messages, &a_chunks));
    assert_eq!(removed, a_chunks.len(), "one vector per piece of A");
    assert_eq!((held(&a_chunks), held(&b_chunks)), (0, b_chunks.len()), "A's vectors are gone, B's stay");
    assert_eq!(indexer.forget_account(&a_messages, &a_chunks), 0, "a second forget finds nothing");
    println!("PASS the-indexer-forgets-a-removed-accounts-vectors-and-keeps-the-others");
    // kept across a restart: the removals are written with the index (commit on drop)
    drop(indexer);
    let indexer = Indexer::start(store.clone(), dir.path().join("indexer.log"));
    indexer.set_embedder(Some(Arc::new(WordEmbedder)));
    assert!(indexer.settle(WAIT));
    let held = |ids: &[i64]| indexer.chunk_cosines(&query, &keys(ids)).unwrap().expect("a meaning index").into_iter().filter(|c| c.is_some()).count();
    assert_eq!((held(&a_chunks), held(&b_chunks)), (0, b_chunks.len()), "after a restart");
    println!("PASS the-indexer-keeps-the-removal-after-a-restart");
}

// ---------------------------------------------------------------- the undo entries (finding 5)

#[test]
fn the_undo_entries_of_a_removed_account_go_with_it_and_z_reverses_the_other_accounts_newest() {
    let w = world();
    let entry_b = w.core.run(Command::Archive { message_ids: vec!["b3".into()] }).expect("B archived").undo_entry_id.expect("an undo entry");
    std::thread::sleep(Duration::from_millis(5));
    let entry_a = w.core.run(Command::Archive { message_ids: vec!["a3".into()] }).expect("A archived").undo_entry_id.expect("an undo entry");
    assert_eq!(count(&w.core, "SELECT count(*) FROM undo_entry WHERE id = ?1", &entry_a), 1);

    remove(&w.core, &w.a);
    assert_eq!(count(&w.core, "SELECT count(*) FROM undo_entry WHERE id = ?1", &entry_a), 0, "A's entry went with A");
    assert_eq!(count(&w.core, "SELECT count(*) FROM undo_entry WHERE id = ?1", &entry_b), 1, "B's entry stays");
    let undone = w.core.run(Command::Undo).expect("Z answers, it does not stop on A's entry");
    assert_eq!(undone.undo_entry_id.as_deref(), Some(entry_b.as_str()), "Z reversed B's archive, the newest entry left");
    assert_eq!(w.core.message("b3".into()).map(|m| m.archived), Some(false), "b3 is back in the inbox");
    println!("PASS the-undo-entries-of-a-removed-account-go-and-z-reverses-the-other-accounts-newest");
    let w = restart(w);
    assert_eq!(count(&w.core, "SELECT count(*) FROM undo_entry WHERE id = ?1", &entry_a), 0, "after a restart");
    println!("PASS the-removed-accounts-undo-entries-stay-gone-after-a-restart");
}
