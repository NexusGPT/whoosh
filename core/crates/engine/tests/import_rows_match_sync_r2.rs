//! Fix round r2 (docs/lanes/import-r2.md), the store side: a message the import stores must satisfy the inbox query exactly as a message
//! the sync stores does. The check's fixture tree is read by the import crate once; the converted batches are written twice: through the
//! core's own import run (production path: `Store::insert_imported`) and through the sync's `insert_messages`, into two stores that hold the
//! same Imported account. Every table of the two databases is then compared row by row; the only columns allowed to differ are the three
//! marks migration 0010 adds to an imported row. Then the inbox query (all accounts, the Imported account, the All view) is read from both.
#[allow(dead_code, unused_imports)]
#[path = "../../autoconfig/tests/support/mod.rs"]
mod detect_fakes;

use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use ::import as mail_import;
use contracts::*;
use detect_fakes::{FakeDns, FakeHttp, FakeProbe};
use engine::{RustCore, TestTransport};
use secrets::{MemoryKeychain, Namespace};
use store::{NewMessage, Store};

const APPLE: &str = "apple-mail:~/Library/Mail";
const MARKS: [&str; 3] = ["source", "import_source_id", "imported_at_ms"];

fn detector() -> Box<dyn engine::Detect> { Box::new(autoconfig::Detector::new(FakeHttp::default(), FakeDns::default(), FakeProbe::default())) }

fn fixture(home: &Path) {
    let dir = home.join("Library/Mail/V10/acct/Inbox.mbox/GUID/Data/1/Messages");
    fs::create_dir_all(&dir).unwrap();
    for (n, (subject, sender)) in ["Imported: quarterly report", "Imported: holiday photos", "Imported: invoice 4471"].iter().zip(["boss@example.test", "pat@example.test", "kim@example.test"]).enumerate() {
        let m = format!("Message-ID: <import-{}@whoosh-check.invalid>\nFrom: {sender}\nTo: stranger@whoosh-check.invalid\nSubject: {subject}\nDate: Tue, 02 Jan 2024 10:11:12 +0000\n\nBody of {subject}.\n", n + 1);
        let mut out = format!("{}\n", m.len()).into_bytes();
        out.extend_from_slice(m.as_bytes());
        out.extend_from_slice(b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<plist version=\"1.0\">\n<dict>\n\t<key>date-received</key>\n\t<integer>1704190272</integer>\n\t<key>flags</key>\n\t<integer>1</integer>\n</dict>\n</plist>\n");
        fs::write(dir.join(format!("{}.emlx", n + 1)), out).unwrap();
    }
}

/// The batches the import crate converts, as the sync would be handed them.
struct Recorder(Vec<NewMessage>);
impl mail_import::MessageSink for Recorder {
    fn insert(&mut self, batch: Vec<NewMessage>) -> Result<usize, mail_import::ImportError> {
        let n = batch.len();
        self.0.extend(batch);
        Ok(n)
    }
}

/// Every table of the database as sorted text rows (all columns, SQL `quote`d), the three import marks left out of `message`.
fn dump(store: &Store) -> Vec<(String, Vec<String>)> {
    store
        .write(|ctx| {
            let tables: Vec<String> = {
                let mut st = ctx.tx.prepare("SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' AND sql NOT LIKE 'CREATE VIRTUAL%' ORDER BY name")?;
                let rows = st.query_map([], |r| r.get::<_, String>(0))?;
                rows.collect::<Result<_, _>>()?
            };
            let mut out = Vec::new();
            for t in tables {
                let cols: Vec<String> = {
                    let mut st = ctx.tx.prepare(&format!("SELECT name FROM pragma_table_info('{t}')"))?;
                    let rows = st.query_map([], |r| r.get::<_, String>(0))?;
                    rows.collect::<Result<Vec<_>, _>>()?
                };
                let kept: Vec<String> = cols.into_iter().filter(|c| !(t == "message" && MARKS.contains(&c.as_str()))).collect();
                if kept.is_empty() { continue; }
                let expr = kept.iter().map(|c| format!("'{c}=' || quote(\"{c}\")")).collect::<Vec<_>>().join(" || ' | ' || ");
                let mut st = ctx.tx.prepare(&format!("SELECT {expr} FROM \"{t}\""))?;
                let mut rows: Vec<String> = st.query_map([], |r| r.get::<_, String>(0))?.collect::<Result<_, _>>()?;
                rows.sort();
                out.push((t, rows));
            }
            Ok(out)
        })
        .unwrap()
}

fn list(store: &Store, tab: &str, account: Option<&str>) -> Vec<String> {
    let q = InboxQuery { tab_id: tab.into(), account_id: account.map(String::from), label_id: None, mailbox: None, account_ids: vec![], filters: vec![], smart_folder_id: None };
    store.inbox(&q).unwrap().into_iter().map(|r| format!("{}|{}|{}|{}|{}|{}|{}", r.message_id, r.thread_id, r.account_id, r.subject, r.received_at_ms, r.unread, r.pinned)).collect()
}

#[test]
fn an_imported_row_is_the_row_a_synced_message_would_be_and_the_inbox_query_reads_both_alike() {
    // A: the core's own import run, the production path
    let (db_a, db_b, home) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    fixture(home.path());
    let core = RustCore::with_parts(Store::open(&db_a.path().join("a.sqlite")).unwrap(), Arc::new(MemoryKeychain::new(Namespace::rig())), detector(), TestTransport::PlainLoopback);
    core.set_import_home(home.path().to_path_buf());
    core.run(Command::ImportScan { picked_paths: vec![] }).unwrap();
    let run = core.run(Command::ImportStart { source_id: APPLE.into(), into: ImportInto::Imported }).unwrap().created_id.unwrap();
    let t0 = Instant::now();
    while core.state(format!("import/{run}/result")).is_none() {
        assert!(t0.elapsed() < Duration::from_secs(30), "the run never ended");
        std::thread::sleep(Duration::from_millis(20));
    }
    let a = core.store();
    let imported = a.account_summaries().unwrap().into_iter().find(|x| x.address == "imported@whoosh.local").expect("the Imported account");

    // B: the same account (same id), the same converted batches, written the way the sync writes a page
    let b = Store::open(&db_b.path().join("b.sqlite")).unwrap();
    b.seed_builtin_labels(0).unwrap(); // labels-r5 round 6: the core seeds the built-in labels when it opens (A did); B is the same store without the core
    let id = imported.id.clone();
    let copy = a.write(move |ctx| Ok(ctx.tx.query_row("SELECT address, provider, display_name, colour, sort_order, credential_kind, keychain_ref, updated_at_ms FROM account WHERE id = ?1", [&id], |r| {
        Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?, r.get::<_, String>(3)?, r.get::<_, i64>(4)?, r.get::<_, String>(5)?, r.get::<_, String>(6)?, r.get::<_, i64>(7)?))
    })?)).unwrap();
    let id = imported.id.clone();
    b.write(move |ctx| {
        ctx.tx.execute("INSERT INTO account (id, address, provider, display_name, colour, sort_order, credential_kind, keychain_ref, updated_at_ms) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            rusqlite::params![id, copy.0, copy.1, copy.2, copy.3, copy.4, copy.5, copy.6, copy.7])?;
        Ok(())
    }).unwrap();
    let mut rec = Recorder(Vec::new());
    let reader = mail_import::find_apple_mail(home.path()).unwrap().expect("the fixture tree");
    let ctx = mail_import::ImportContext { accounts: &[], imported_account_id: &imported.id };
    mail_import::import(&reader, &ctx, &mut rec, &mut |_| {}).unwrap();
    assert_eq!(rec.0.len(), 3, "the fixture tree converts to three messages");
    b.insert_messages(rec.0).unwrap();

    // every table, row by row (the three marks of an imported row are the only columns left out)
    let (da, db) = (dump(&a), dump(&b));
    for ((ta, ra), (tb, rb)) in da.iter().zip(db.iter()) {
        assert_eq!(ta, tb);
        if ta == "account" { continue; }
        assert_eq!(ra, rb, "table {ta} differs between an imported row and a synced one");
    }
    assert_eq!(da.len(), db.len());
    // and the question a screen asks
    for (tab, acc) in [("primary", None), ("all", None), ("primary", Some(imported.id.as_str())), ("all", Some(imported.id.as_str()))] {
        let (la, lb) = (list(&a, tab, acc), list(&b, tab, acc));
        assert_eq!(la.len(), 3, "{tab} {acc:?}: {la:?}");
        assert_eq!(la, lb, "{tab} {acc:?}");
    }
}
