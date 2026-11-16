//! Lane decided-model (SPEC A133, board item abda53d1): a filing made by an on-device model other than Laya names that model. The store keeps
//! it as the on-device decider `decided_by 'laya'` (the CHECK's word) with `decided_model` holding the catalogue id, as migration 0031 keeps
//! Apple's model ('laya' + 'apple'). No migration: the column exists. The two reserved words of `decided_model` that are no model,
//! `SET_ASIDE_MODEL` ('apple-refused') and `WAITING_MODEL` ('ai-waiting'), and Apple's 'apple', are never read as a model.
//!
//! Run: cargo test -p store --test decided_model
//!
//! Broken copy M1: core/crates/store/src/label_meta.rs `decided_by_from`: the arm
//! `("laya", Some(id)) if id != SET_ASIDE_MODEL && id != WAITING_MODEL && !id.is_empty() => DecidedBy::Model { model: id.to_string() },` removed
//! -> red: `a_model_round_trips_through_the_columns`, `a_model_filing_reads_back_as_that_model`.
//! Broken copy M2: the same arm without its guard, `("laya", Some(id)) if id != SET_ASIDE_MODEL && id != WAITING_MODEL && !id.is_empty() =>`
//! -> `("laya", Some(id)) =>` -> red: `apple_and_the_reserved_words_are_never_read_as_a_model`.
mod common;
use common::*;
use contracts::*;
use store::{decided_by_columns, decided_by_from, SET_ASIDE_MODEL, WAITING_MODEL};

fn knowline() -> DecidedBy { DecidedBy::Model { model: "knowline-0.8b".into() } }

#[test]
fn a_model_round_trips_through_the_columns() {
    let (word, rule, model) = decided_by_columns(&knowline());
    assert_eq!((word, rule.clone(), model.clone()), ("laya", None, Some("knowline-0.8b".to_string())), "stored as the on-device decider with the id");
    assert_eq!(decided_by_from(word, rule, model.as_deref()), knowline(), "and read back as that model");
    // every other decider round-trips as before
    for by in [DecidedBy::Laya, DecidedBy::Apple, DecidedBy::Jev, DecidedBy::Owner, DecidedBy::Default, DecidedBy::Rule { rule_id: "r1".into() }] {
        let (w, r, m) = decided_by_columns(&by);
        assert_eq!(decided_by_from(w, r, m.as_deref()), by, "{by:?}");
    }
}

#[test]
fn apple_and_the_reserved_words_are_never_read_as_a_model() {
    assert_eq!(decided_by_from("laya", None, Some("apple")), DecidedBy::Apple);
    for word in [SET_ASIDE_MODEL, WAITING_MODEL] {
        let by = decided_by_from("laya", None, Some(word));
        assert!(!matches!(by, DecidedBy::Model { .. }), "{word} is no model: {by:?}");
        assert_eq!(by, DecidedBy::Laya, "{word}");
    }
    assert_eq!(decided_by_from("laya", None, None), DecidedBy::Laya, "no id: Laya, as before");
    assert_eq!(decided_by_from("jev", None, Some("knowline-0.8b")), DecidedBy::Jev, "only the on-device decider carries a model");
}

#[test]
fn a_model_filing_reads_back_as_that_model() {
    let db = TempDb::new("decided-model-filing");
    let s = small_store(&db, 2);
    let l = s.label_create_full(LabelSpec { name: "To read".into(), parent_id: None, colour: None, description: "mail I still have to read".into(), scope: LabelScope::All,
        rules: RuleSet { conditions: vec![], match_mode: MatchMode::Any }, pinned: false }, 1).unwrap().id;
    s.label_file_entry(l.clone(), "m000".into(), knowline(), Some("labels/model".into()), Some(0.9), 2).expect("the CHECK allows it: stored as laya");
    assert_eq!(s.label_filing(&l, "m000").unwrap().map(|(by, _)| by), Some(knowline()), "reads back as the model");
    // a model's "no" is kept under the on-device decider and read like any model's
    s.ai_no_put(vec![("m001".into(), l.clone())], knowline(), 3).expect("a model may rule a mail out");
    assert_eq!(s.ai_no_messages(&l).unwrap(), vec!["m001".to_string()]);
}
