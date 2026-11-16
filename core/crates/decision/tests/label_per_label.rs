//! Lane fix-aisort round 2: the number compared with MODEL_TRUST_LABEL must be the probability that THIS label applies (docs/CONSTANTS.md
//! MODEL_TRUST_LABEL: "the decision model files a message under a label by its meaning only at or above it"). Today's Labels question is ONE
//! choice among the offered labels: its number is the label's share of a forced choice (no "none" option), measured on the owner's Mac at
//! 0.58 at best over 50 mails. The per-label reading (`tuning::labels_per_label`, ledger row LABEL_AI_QUESTION = per-label, proposed) asks
//! one yes/no per label and reads the yes-probability. These tests pin the mapping; no model, no store.
//!
//! Run: cargo test -p decision --test label_per_label
//!
//! Broken copies (each one edit in core/crates/decision/src/decide.rs; each turns the named test red):
//! - P1 `label_applies`: `DecisionAnswer::Noul { probability, .. } => Some(*probability),` -> `DecisionAnswer::Noul { probability, yes, .. } => Some(if *yes { *probability } else { 1.0 - probability }),`
//!   -> red: `a_no_answer_reads_as_its_low_yes_probability_not_as_the_confidence_of_the_no`.
//! - P2 `read_per_label`: `return Err(why);` -> `continue;` -> red: `one_unreadable_label_answer_makes_the_whole_reading_a_failure`.
//! - P3 `entry_of_question`: `.map_or(question_id, |(e, _)| e)` -> `.map_or(question_id, |(_, l)| l)` -> red: `a_per_label_question_belongs_to_its_entry`.
use std::collections::HashMap;

use contracts::{DecisionAnswer, DecisionQuestion};
use decision::decide::{entry_of_question, label_applies, label_question_id, label_statement, read_per_label};

fn noul(id: &str, yes_probability: f64) -> DecisionAnswer {
    DecisionAnswer::Noul { id: id.into(), yes: yes_probability >= 0.5, probability: yes_probability }
}
fn asked(id: &str) -> DecisionQuestion {
    DecisionQuestion::Noul { id: id.into(), prompt: "statement".into() }
}

#[test]
fn a_no_answer_reads_as_its_low_yes_probability_not_as_the_confidence_of_the_no() {
    // Laya answers a noul with its yes-probability (LayaDecisionPort.swift: `.noul(id, yes: yes >= 0.5, probability: yes)`)
    assert_eq!(label_applies(&noul("q", 0.9)), Some(0.9));
    assert_eq!(label_applies(&noul("q", 0.1)), Some(0.1), "a confident no is a LOW probability that the label applies, never 0.9");
    assert_eq!(label_applies(&DecisionAnswer::Choice { id: "q".into(), choice: "Needs Reply".into(), probabilities: HashMap::from([("Needs Reply".to_string(), 0.58)]) }), None,
               "a share of a choice is not a yes-probability");
}

#[test]
fn a_per_label_question_belongs_to_its_entry() {
    let qid = label_question_id("labels/model", "needs-reply");
    assert_eq!(qid, "labels/model#needs-reply");
    assert_eq!(entry_of_question(&qid), "labels/model");
    assert_eq!(entry_of_question("labels/model"), "labels/model", "the one-choice question keeps its id");
}

#[test]
fn the_statement_carries_the_label_and_its_description() {
    assert_eq!(label_statement("Needs Reply", " mail that waits for my answer "), "This email belongs under the label \"Needs Reply\": mail that waits for my answer");
}

#[test]
fn each_label_gets_its_own_yes_probability_in_path_order() {
    let paths = HashMap::from([("Needs Reply".to_string(), "needs-reply".to_string()), ("Updates".to_string(), "updates".to_string())]);
    let (a, b) = (label_question_id("labels/model", "needs-reply"), label_question_id("labels/model", "updates"));
    let answers = HashMap::from([(a.clone(), noul(&a, 0.86)), (b.clone(), noul(&b, 0.07))]);
    let read = read_per_label("labels/model", &paths, &[asked(&a), asked(&b)], &answers).unwrap();
    assert_eq!(read, vec![("needs-reply".to_string(), "Needs Reply".to_string(), 0.86), ("updates".to_string(), "Updates".to_string(), 0.07)]);
    // a question that was never sent (no engine, or the label is not offered) is not read
    assert_eq!(read_per_label("labels/model", &paths, &[], &answers).unwrap(), vec![]);
}

#[test]
fn one_unreadable_label_answer_makes_the_whole_reading_a_failure() {
    let paths = HashMap::from([("Needs Reply".to_string(), "needs-reply".to_string()), ("Updates".to_string(), "updates".to_string())]);
    let (a, b) = (label_question_id("labels/model", "needs-reply"), label_question_id("labels/model", "updates"));
    let missing = HashMap::from([(a.clone(), noul(&a, 0.86))]);
    assert!(read_per_label("labels/model", &paths, &[asked(&a), asked(&b)], &missing).unwrap_err().contains("no answer"));
    let nan = HashMap::from([(a.clone(), noul(&a, 0.86)), (b.clone(), noul(&b, f64::NAN))]);
    assert!(read_per_label("labels/model", &paths, &[asked(&a), asked(&b)], &nan).unwrap_err().contains("probability from 0 to 1"));
}
