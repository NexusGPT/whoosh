//! A folder of this Mac read into excerpts and kept up to date, against fake ports.
mod common;
use attachment_index::AttachmentKind;
use common::*;
use contracts::{ReplySourceRef, ReplySyncProgress};
use reply_sources::{fresh_source, sync_local_folder, FolderPorts, FolderSyncReport, SyncOutcome, REPLY_FILE_MAX_BYTES};

const ROOT: &str = "/Users/me/Documents/Contrats";

struct Rig {
    tree: FakeTree,
    reader: FakeReader,
    sink: FakeSink,
    progress: FakeProgress,
}

impl Rig {
    fn new() -> Rig {
        let r = Rig { tree: FakeTree::default(), reader: FakeReader::default(), sink: FakeSink::default(), progress: FakeProgress::default() };
        for d in ["/Users", "/Users/me", "/Users/me/Documents", ROOT] {
            r.tree.dir(d);
        }
        r
    }
    fn sync(&self) -> FolderSyncReport {
        self.sync_while(&|| true)
    }
    fn sync_while(&self, keep_going: &dyn Fn() -> bool) -> FolderSyncReport {
        let ports = FolderPorts { tree: &self.tree, reader: &self.reader, sink: &self.sink, progress: &self.progress };
        sync_local_folder(&ports, ROOT, keep_going)
    }
}

fn source() -> ReplySourceRef {
    ReplySourceRef::LocalFolder { path: ROOT.to_string() }
}

fn f(name: &str) -> String {
    format!("{ROOT}/{name}")
}

fn para(word: &str, n: usize) -> String {
    vec![word; n].join(" ")
}

/// a.txt holds two long paragraphs (two excerpts); the others one each; a picture and an empty file add none.
fn mixed(r: &Rig) {
    r.tree.file(&f("a.txt"), &format!("{}.\n\n{}.", para("alpha", 60), para("beta", 60)), 1_000);
    r.tree.file(&f("plan.md"), "# Plan 2026\n\nLe budget est validé.", 2_000);
    r.tree.file(&f("offre.html"), "<html><head><title>Offre Lorvan</title></head><body><p>Prix: 12 000 euros</p><script>x()</script></body></html>", 3_000);
    r.tree.file(&f("contrat.docx"), "PK-binary", 4_000);
    r.reader.answer(&f("contrat.docx"), "Clause 4: résiliation à 30 jours.");
    r.tree.file(&f("scan.pdf"), "not a pdf the core can read", 5_000);
    r.reader.answer(&f("scan.pdf"), "Facture 2026-114, total 980 euros.");
    r.tree.file(&f("photo.jpg"), "jpeg", 6_000);
    r.tree.file(&f("vide.txt"), "   \n\n  ", 7_000);
}

#[test]
fn a_folder_of_mixed_files_gives_the_right_excerpts_and_count() {
    let r = Rig::new();
    mixed(&r);
    let rep = r.sync();
    assert_eq!(rep.outcome, SyncOutcome::UpToDate);
    assert_eq!(rep.item_count, 5, "a, plan, offre, contrat, scan hold text; the picture and the empty file do not");
    assert_eq!(rep.skips.not_text, 1);
    let ex = r.sink.excerpts(&source());
    let by = |key: &str| ex.iter().filter(|e| e.item_key == key).cloned().collect::<Vec<_>>();
    let a = by("a.txt");
    assert_eq!(a.len(), 2, "two long paragraphs, two excerpts");
    assert!(a[0].text.starts_with("alpha") && a[1].text.starts_with("beta"));
    assert_eq!((a[0].ordinal, a[1].ordinal), (0, 1));
    assert_eq!(a[0].title, "a");
    assert_eq!(a[0].folder, f("a.txt"), "the excerpt names the file's whole path");
    assert_eq!(a[0].date_ms, Some(1_000), "the excerpt carries the modified date");
    let plan = by("plan.md");
    assert_eq!(plan.len(), 1, "a short title paragraph joins the next one");
    assert_eq!(plan[0].title, "Plan 2026", "markdown title from its heading");
    assert!(plan[0].text.contains("budget est validé"));
    let offre = by("offre.html");
    assert_eq!(offre[0].title, "Offre Lorvan", "HTML title from <title>");
    assert!(offre[0].text.contains("Prix: 12 000 euros"));
    assert!(!offre[0].text.contains("<p>") && !offre[0].text.contains("x()"), "HTML read as text, no tags, no script");
    assert_eq!(by("contrat.docx")[0].text, "Clause 4: résiliation à 30 jours.");
    assert_eq!(by("scan.pdf")[0].text, "Facture 2026-114, total 980 euros.", "a PDF the core cannot read goes to PDFKit");
    let calls = r.reader.calls.lock().unwrap().clone();
    assert!(calls.contains(&(f("scan.pdf"), AttachmentKind::Pdf)));
    assert!(calls.contains(&(f("contrat.docx"), AttachmentKind::Document)));
    assert!(by("photo.jpg").is_empty() && by("vide.txt").is_empty());
    assert_eq!(ex.len(), 6);
    assert!(ex.iter().all(|e| e.source == source()));
    let ids: std::collections::HashSet<&str> = ex.iter().map(|e| e.id.as_str()).collect();
    assert_eq!(ids.len(), ex.len(), "every excerpt id is distinct");
}

#[test]
fn files_in_sub_folders_are_read_with_their_path() {
    let r = Rig::new();
    r.tree.dir(&f("2026"));
    r.tree.dir(&f("2026/Oranta"));
    r.tree.file(&f("2026/Oranta/devis.txt"), "Devis Oranta signé.", 1);
    let rep = r.sync();
    assert_eq!(rep.item_count, 1);
    let ex = r.sink.excerpts(&source());
    assert_eq!(ex[0].item_key, "2026/Oranta/devis.txt");
    assert_eq!(ex[0].folder, f("2026/Oranta/devis.txt"));
    assert_eq!(ex[0].to_reply_excerpt().subject.as_deref(), Some("devis"));
    assert_eq!(ex[0].to_reply_excerpt().folder.as_deref(), Some(f("2026/Oranta/devis.txt").as_str()));
}

#[test]
fn a_changed_file_is_read_again_and_an_unchanged_one_is_not() {
    let r = Rig::new();
    r.tree.file(&f("a.txt"), "Version une.", 1_000);
    r.tree.file(&f("b.txt"), "Rien ne change.", 1_000);
    r.sync();
    r.tree.take_reads();
    r.sink.take_puts();
    r.tree.file(&f("a.txt"), "Version deux, plus longue.", 2_000);
    let rep = r.sync();
    assert_eq!(r.tree.take_reads(), vec![f("a.txt")], "only the changed file is read");
    assert_eq!(r.sink.take_puts(), vec!["a.txt".to_string()], "only the changed file is written");
    assert_eq!((rep.read, rep.unchanged, rep.item_count), (1, 1, 2));
    let ex = r.sink.excerpts(&source());
    assert!(ex.iter().any(|e| e.text == "Version deux, plus longue."));
    assert!(!ex.iter().any(|e| e.text == "Version une."), "the old text is gone");
}

#[test]
fn the_same_size_with_a_new_modified_time_is_a_change() {
    let r = Rig::new();
    r.tree.file(&f("a.txt"), "aaaa", 1_000);
    r.sync();
    r.tree.take_reads();
    r.tree.file(&f("a.txt"), "bbbb", 1_001);
    r.sync();
    assert_eq!(r.tree.take_reads(), vec![f("a.txt")]);
}

#[test]
fn a_removed_file_takes_its_excerpts_with_it() {
    let r = Rig::new();
    r.tree.file(&f("a.txt"), "Garde.", 1);
    r.tree.file(&f("b.txt"), "Supprime.", 1);
    r.sync();
    r.tree.remove(&f("b.txt"));
    let rep = r.sync();
    assert_eq!(rep.removed, 1);
    assert_eq!(rep.item_count, 1);
    assert_eq!(r.sink.item_keys(&source()), vec!["a.txt".to_string()]);
    assert!(!r.sink.excerpts(&source()).iter().any(|e| e.text == "Supprime."));
}

#[test]
fn hidden_files_caches_packages_and_big_files_are_skipped() {
    let r = Rig::new();
    r.tree.file(&f(".secret.txt"), "hidden", 1);
    r.tree.dir(&f(".git"));
    r.tree.file(&f(".git/HEAD.txt"), "ref", 1);
    r.tree.dir(&f("node_modules"));
    r.tree.file(&f("node_modules/readme.md"), "dep", 1);
    r.tree.dir(&f("Outil.app"));
    r.tree.file(&f("Outil.app/info.txt"), "bundle", 1);
    r.tree.sized(&f("enorme.txt"), "big", REPLY_FILE_MAX_BYTES + 1, 1);
    r.tree.sized(&f("limite.txt"), "juste assez", REPLY_FILE_MAX_BYTES, 1);
    let rep = r.sync();
    assert_eq!(rep.skips.hidden, 2, ".secret.txt and .git");
    assert_eq!(rep.skips.skipped_dirs, 2, "node_modules and Outil.app");
    assert_eq!(rep.skips.too_big, 1);
    assert_eq!(r.tree.take_reads(), vec![f("limite.txt")], "nothing skipped is read; a file at the cap is");
    assert_eq!(r.sink.item_keys(&source()), vec!["limite.txt".to_string()]);
}

#[test]
fn a_file_that_grows_past_the_cap_leaves_the_index() {
    let r = Rig::new();
    r.tree.file(&f("journal.txt"), "petit", 1);
    r.sync();
    r.tree.sized(&f("journal.txt"), "grand", REPLY_FILE_MAX_BYTES + 1, 2);
    let rep = r.sync();
    assert_eq!(rep.removed, 1);
    assert!(r.sink.item_keys(&source()).is_empty());
}

#[test]
fn a_link_out_of_the_folder_is_not_followed() {
    let r = Rig::new();
    r.tree.dir("/private");
    r.tree.file("/private/passwords.txt", "hunter2", 1);
    r.tree.link(&f("raccourci"), "/private");
    r.tree.link(&f("mots.txt"), "/private/passwords.txt");
    r.tree.file(&f("ok.txt"), "Le contrat.", 1);
    let rep = r.sync();
    assert_eq!(rep.skips.links, 2);
    assert_eq!(rep.item_count, 1);
    assert!(!r.tree.take_reads().iter().any(|p| p.starts_with("/private")), "nothing outside the folder is read");
    assert!(!r.sink.excerpts(&source()).iter().any(|e| e.text.contains("hunter2")));
}

#[test]
fn progress_goes_not_started_then_syncing_then_up_to_date() {
    let r = Rig::new();
    assert_eq!(fresh_source(source()).progress, ReplySyncProgress::NotStarted, "a folder just added has not started");
    for (i, n) in ["a", "b", "c", "d"].iter().enumerate() {
        r.tree.file(&f(&format!("{n}.txt")), "Texte.", i as i64);
    }
    let rep = r.sync();
    let states = r.progress.states();
    let want: Vec<ReplySyncProgress> = [0, 25, 50, 75, 100].iter().map(|&p| ReplySyncProgress::Syncing { percent: p }).chain([ReplySyncProgress::UpToDate]).collect();
    assert_eq!(states, want);
    assert_eq!(r.progress.log.lock().unwrap().last().unwrap().0, 4, "the final event carries the item count");
    assert_eq!(rep.last, ReplySyncProgress::UpToDate);
}

#[test]
fn progress_sends_a_change_only_when_the_percent_moves() {
    let r = Rig::new();
    for i in 0..250 {
        r.tree.file(&f(&format!("n{i:03}.txt")), "Texte.", 1);
    }
    r.sync();
    let states = r.progress.states();
    assert_eq!(states.len(), 102, "0 to 100 once each, then UpToDate: 250 files do not send 250 events");
    assert_eq!(states.last(), Some(&ReplySyncProgress::UpToDate));
}

#[test]
fn a_folder_that_will_not_open_fails_with_words() {
    let r = Rig::new();
    r.tree.remove(ROOT);
    let rep = r.sync();
    match &rep.outcome {
        SyncOutcome::Failed { reason_words } => assert!(reason_words.contains(ROOT), "the words name the folder: {reason_words}"),
        o => panic!("expected Failed, got {o:?}"),
    }
    match r.progress.states().as_slice() {
        [ReplySyncProgress::Failed { reason_words }] => assert!(!reason_words.is_empty()),
        s => panic!("expected one Failed event, got {s:?}"),
    }
}

#[test]
fn an_index_that_will_not_answer_fails_with_words() {
    let r = Rig::new();
    r.tree.file(&f("a.txt"), "x", 1);
    *r.sink.fail_stamps.lock().unwrap() = true;
    let rep = r.sync();
    assert!(matches!(&rep.outcome, SyncOutcome::Failed { reason_words } if reason_words.contains("disk full")));
    assert!(matches!(r.progress.states().last(), Some(ReplySyncProgress::Failed { .. })));
}

#[test]
fn a_file_whose_reader_fails_is_kept_empty_and_not_read_again_until_it_changes() {
    let r = Rig::new();
    r.tree.file(&f("casse.docx"), "PK", 1);
    r.reader.refuse(&f("casse.docx"), "the document is damaged");
    r.tree.file(&f("ok.txt"), "Bon.", 1);
    let rep = r.sync();
    assert_eq!(rep.outcome, SyncOutcome::UpToDate, "one bad file does not fail the folder");
    assert_eq!((rep.unreadable, rep.item_count), (1, 1));
    r.reader.calls.lock().unwrap().clear();
    let rep2 = r.sync();
    assert!(r.reader.calls.lock().unwrap().is_empty(), "the damaged file is not read again while unchanged");
    assert_eq!(rep2.item_count, 1);
}

#[test]
fn a_sync_told_to_stop_writes_nothing_more() {
    let r = Rig::new();
    r.tree.file(&f("a.txt"), "x", 1);
    r.tree.file(&f("b.txt"), "y", 1);
    let rep = r.sync_while(&|| false);
    assert_eq!(rep.outcome, SyncOutcome::Stopped);
    assert!(r.sink.take_puts().is_empty());
    assert!(!r.progress.states().contains(&ReplySyncProgress::UpToDate));
}
