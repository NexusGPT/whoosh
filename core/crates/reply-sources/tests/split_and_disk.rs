//! The excerpt cut, and the real disk adapter on a real folder with a real link out of it.
use reply_sources::adapters::DiskTree;
use reply_sources::{split_excerpts, walk_folder, FileTree, TreeEntryKind};
use std::path::Path;

#[test]
fn a_long_paragraph_is_cut_at_sentence_ends_within_the_maximum() {
    let sentence = "Le client confirme la date de livraison du lot. ";
    let text = sentence.repeat(10);
    let parts = split_excerpts(&text, 20, 120);
    assert!(parts.len() >= 4, "{parts:?}");
    for p in &parts {
        assert!(p.chars().count() <= 120, "{p}");
        assert!(p.ends_with('.'), "cut at a sentence end: {p}");
    }
    assert_eq!(parts.join(" ").split_whitespace().count(), text.split_whitespace().count(), "no word lost");
}

#[test]
fn a_paragraph_with_no_sentence_end_is_cut_at_a_space_and_one_with_no_space_at_the_maximum() {
    let words = "mot ".repeat(50);
    for p in split_excerpts(&words, 0, 30) {
        assert!(p.chars().count() <= 30 && !p.starts_with(' ') && !p.ends_with(' '), "{p:?}");
    }
    let blob = "é".repeat(70);
    let parts = split_excerpts(&blob, 0, 30);
    assert_eq!(parts.iter().map(|p| p.chars().count()).collect::<Vec<_>>(), vec![30, 30, 10], "cut on characters, never inside one");
}

#[test]
fn short_paragraphs_join_until_the_minimum_and_long_ones_stand_alone() {
    let text = "Bonjour,\n\nMerci pour votre message.\n\nVoici le point sur le contrat, signé hier par les deux parties.\n\nCordialement";
    assert_eq!(split_excerpts(text, 30, 200), vec!["Bonjour,\n\nMerci pour votre message.", "Voici le point sur le contrat, signé hier par les deux parties.", "Cordialement"]);
    assert_eq!(split_excerpts(text, 0, 200).len(), 4, "a minimum of 0 joins nothing");
    assert!(split_excerpts("  \n\n \t ", 10, 100).is_empty());
}

#[test]
fn the_disk_adapter_reports_a_link_as_a_link_and_the_walk_never_enters_it() {
    let outside = tempfile::tempdir().unwrap();
    std::fs::write(outside.path().join("secret.txt"), "hunter2").unwrap();
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("note.md"), "# Note\n\nUn texte.").unwrap();
    std::fs::create_dir(root.path().join("sous")).unwrap();
    std::fs::write(root.path().join("sous/b.txt"), "B").unwrap();
    std::os::unix::fs::symlink(outside.path(), root.path().join("dehors")).unwrap();
    std::os::unix::fs::symlink(outside.path().join("secret.txt"), root.path().join("lien.txt")).unwrap();
    let tree = DiskTree { home: None };
    let entries = tree.read_dir(root.path()).unwrap();
    let kind = |n: &str| entries.iter().find(|e| e.name == n).map(|e| e.kind.clone()).unwrap();
    assert_eq!(kind("dehors"), TreeEntryKind::Link);
    assert_eq!(kind("lien.txt"), TreeEntryKind::Link);
    assert_eq!(kind("sous"), TreeEntryKind::Dir);
    assert!(matches!(kind("note.md"), TreeEntryKind::File { size: 17, modified_ms } if modified_ms > 0));
    let walk = walk_folder(&tree, root.path()).unwrap();
    let rels: Vec<&str> = walk.files.iter().map(|f| f.rel.as_str()).collect();
    assert_eq!(rels, vec!["note.md", "sous/b.txt"]);
    assert_eq!(walk.skips.links, 2);
    assert!(tree.read(&root.path().join("lien.txt"), 100).is_err(), "a read of a link is refused too");
    assert_eq!(tree.read(&root.path().join("note.md"), 6).unwrap(), b"# Note", "a read stops at max_bytes");
}

#[test]
fn the_disk_adapter_reads_a_home_path_under_home() {
    let home = tempfile::tempdir().unwrap();
    std::fs::create_dir(home.path().join("Documents")).unwrap();
    std::fs::write(home.path().join("Documents/a.txt"), "A").unwrap();
    let tree = DiskTree { home: Some(home.path().to_path_buf()) };
    let entries = tree.read_dir(Path::new("~/Documents")).unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(tree.read(Path::new("~/Documents/a.txt"), 10).unwrap(), b"A");
}
