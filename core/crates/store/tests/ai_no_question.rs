//! Lane fix-aisort round 4 (lead's ruling, 12 November): a changed QUESTION of the label AI clears its kept "no" (`ai_label_no`), as a
//! changed description does. The question is one line `<form>|<trust>` kept in a marker beside the database
//! (`Store::ai_no_clear_on_question`, called at each engine start by `engine/src/label_ai_failure.rs` `clear_on_question_change`). On the
//! owner's Mac: 54 mails judged under the one-choice question kept a "no" each, and the per-label build never asked them again.
//!
//! Run: cargo test -p store --test ai_no_question
//!
//! Broken copies (each one edit in core/crates/store/src/label_meta.rs `ai_no_clear_on_question`; each turns the named test red):
//! - Q1 `if before.as_deref() == Some(question) {` -> `if before.is_some() {` -> red: `a_changed_trust_level_clears_the_kept_no`.
//! - Q2 drop the line `std::fs::write(marker, ...)` (and its `map_err`) -> red: `the_same_question_at_the_next_start_clears_nothing`.
//! - Q3 `DELETE FROM ai_label_no WHERE decided_by IN ('laya', 'jev')` -> `DELETE FROM label_assignment` -> red:
//!   `the_first_start_writes_the_marker_and_clears_and_filings_are_never_touched` (the owner's and the rule's filings go, the no-rows stay).
mod common;
use common::*;
use contracts::*;

fn spec(name: &str) -> LabelSpec {
    LabelSpec { name: name.into(), parent_id: None, colour: None, description: "emails waiting for my answer".into(), scope: LabelScope::All,
        rules: RuleSet { conditions: vec![], match_mode: MatchMode::Any }, pinned: false }
}

/// A store with one label, a "no" of Laya on m000, a "no" of Jev on m001, and filings by the owner (m002) and by a rule (m001).
fn seeded(db: &TempDb) -> (store::Store, String) {
    let s = small_store(db, 3);
    let l = s.label_create_full(spec("Needs reply"), 1).unwrap().id;
    s.ai_no_put(vec![("m000".into(), l.clone())], DecidedBy::Laya, 2).unwrap();
    s.ai_no_put(vec![("m001".into(), l.clone())], DecidedBy::Jev, 2).unwrap();
    s.label_file(l.clone(), "m002".into(), DecidedBy::Owner, None, 3).unwrap();
    s.label_file(l.clone(), "m001".into(), DecidedBy::Rule { rule_id: "r1".into() }, None, 3).unwrap();
    (s, l)
}

fn marker(db: &TempDb) -> std::path::PathBuf { db.0.with_file_name("ai-label-question") }

#[test]
fn the_first_start_writes_the_marker_and_clears_and_filings_are_never_touched() {
    let db = TempDb::new("aino-q-first");
    let (s, l) = seeded(&db);
    assert_eq!(s.ai_no_clear_on_question("per-label|0.8", &marker(&db)).unwrap(), Some((None, 2)), "no marker yet: both models' no go");
    assert!(s.ai_no_messages(&l).unwrap().is_empty());
    assert_eq!(std::fs::read_to_string(marker(&db)).unwrap().trim(), "per-label|0.8", "the marker holds the question");
    assert_eq!(s.label_filing(&l, "m002").unwrap().map(|(by, _)| by), Some(DecidedBy::Owner), "the owner's filing stays");
    assert_eq!(s.label_filing(&l, "m001").unwrap().map(|(by, _)| by), Some(DecidedBy::Rule { rule_id: "r1".into() }), "a rule's filing stays");
}

#[test]
fn the_same_question_at_the_next_start_clears_nothing() {
    let db = TempDb::new("aino-q-same");
    let (s, l) = seeded(&db);
    s.ai_no_clear_on_question("per-label|0.8", &marker(&db)).unwrap();
    s.ai_no_put(vec![("m000".into(), l.clone())], DecidedBy::Laya, 4).unwrap();
    assert_eq!(s.ai_no_clear_on_question("per-label|0.8", &marker(&db)).unwrap(), None, "same question: nothing to clear");
    assert_eq!(s.ai_no_messages(&l).unwrap(), vec!["m000".to_string()], "the no kept under this question stays");
}

#[test]
fn a_changed_trust_level_clears_the_kept_no() {
    let db = TempDb::new("aino-q-trust");
    let (s, l) = seeded(&db);
    s.ai_no_clear_on_question("per-label|0.8", &marker(&db)).unwrap();
    s.ai_no_put(vec![("m000".into(), l.clone())], DecidedBy::Laya, 4).unwrap();
    assert_eq!(s.ai_no_clear_on_question("per-label|0.7", &marker(&db)).unwrap(), Some((Some("per-label|0.8".to_string()), 1)));
    assert!(s.ai_no_messages(&l).unwrap().is_empty());
    // and a changed form (the owner's case: choice -> per-label) the same way
    s.ai_no_put(vec![("m000".into(), l.clone())], DecidedBy::Laya, 5).unwrap();
    std::fs::write(marker(&db), "choice|0.8\n").unwrap();
    assert_eq!(s.ai_no_clear_on_question("per-label|0.8", &marker(&db)).unwrap(), Some((Some("choice|0.8".to_string()), 1)));
}

// ---------------------------------------------------------------- round 8, migration 0031: Apple's model is stored as 'laya' + decided_model 'apple'
// Broken copy Q4: core/crates/store/src/label_meta.rs `decided_by_from`: the arm `("laya", Some("apple")) => DecidedBy::Apple,` removed
// -> red: `an_apple_filing_writes_and_reads_back_as_apple_and_an_old_row_reads_as_before`.

#[test]
fn an_apple_filing_writes_and_reads_back_as_apple_and_an_old_row_reads_as_before() {
    let db = TempDb::new("aino-q-apple");
    let (s, l) = seeded(&db);
    s.label_file_entry(l.clone(), "m000".into(), DecidedBy::Apple, Some("labels/model".into()), Some(1.0), 4).expect("the CHECK allows it: stored as laya");
    assert_eq!(s.label_filing(&l, "m000").unwrap().map(|(by, _)| by), Some(DecidedBy::Apple), "reads back as Apple's");
    s.label_file_entry(l.clone(), "m000".into(), DecidedBy::Laya, Some("labels/model".into()), Some(0.9), 5).unwrap();
    assert_eq!(s.label_filing(&l, "m000").unwrap().map(|(by, _)| by), Some(DecidedBy::Laya), "a later Laya filing clears the model's name");
    assert_eq!(s.label_filing(&l, "m002").unwrap().map(|(by, _)| by), Some(DecidedBy::Owner), "an old row reads as before");
    // a no of Apple's model is kept, and cleared with every model's no by a changed question
    s.ai_no_put(vec![("m002".into(), l.clone())], DecidedBy::Apple, 6).expect("a no by Apple's model");
    assert!(s.ai_no_messages(&l).unwrap().contains(&"m002".to_string()));
    s.ai_no_clear_on_question("per-label|0.8", &marker(&db)).unwrap();
    assert!(s.ai_no_messages(&l).unwrap().is_empty(), "the clear by decider ('laya', 'jev') takes Apple's rows too: they are stored as laya");
}
