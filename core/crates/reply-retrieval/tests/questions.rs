//! Step 1: the questions, their cap, the prompt's budget, and the fallback to the subject.
mod common;
use common::*;
use contracts::CoreError;
use reply_retrieval::usecases::write_questions::{parse_questions, question_prompt};
use reply_retrieval::*;

#[test]
fn the_questions_are_capped_at_questions_max_and_each_is_searched() {
    let mut req = request();
    req.settings.questions_max = 3;
    let w = writer("1. Quel prix ?\n2) Quel délai ?\n- Quelle garantie ?\n* Quel acompte ?\nQ5: Quelle couleur ?");
    let i = index(vec![]);
    let o = run(&req, &w, &i, &judge());
    let qs: Vec<String> = o.trace.questions.iter().map(|q| q.question.clone()).collect();
    assert_eq!(qs, vec!["Quel prix ?", "Quel délai ?", "Quelle garantie ?"]);
    let searched: Vec<String> = i.queries.lock().unwrap().iter().map(|q| q.text.clone()).collect();
    assert_eq!(searched, qs, "one search per question, no more");
    assert_eq!(o.trace.origin, QuestionOrigin::Model);
}

#[test]
fn the_model_answer_is_read_as_lines_or_a_list_each_question_once() {
    assert_eq!(parse_questions("Here are the questions:\n\"Quel prix ?\"\nquel prix ?\n**Quel délai ?**", 10).unwrap(), vec!["Quel prix ?", "Quel délai ?"]);
    assert_eq!(parse_questions("[\"Quel prix ?\", \"Quel délai ?\"]", 10).unwrap(), vec!["Quel prix ?", "Quel délai ?"]);
    assert_eq!(parse_questions("Quel prix ?\nQuel délai ?", 1).unwrap(), vec!["Quel prix ?"]);
}

fn fallback_case(w: Writer, why: FallbackWhy) {
    let req = request();
    let i = index(vec![ex("c1", 0.9, Some(NOV_09), "Devis toiture", "Le prix est de 4 200 euros.")]);
    let o = run(&req, &w, &i, &judge());
    assert_eq!(o.trace.origin, QuestionOrigin::SubjectFallback { why });
    let searched: Vec<String> = i.queries.lock().unwrap().iter().map(|q| q.text.clone()).collect();
    assert_eq!(searched, vec!["Devis toiture"], "one search on the subject, without Re:");
    assert_eq!(kept_ids(&o), vec!["c1"], "the fallback search still feeds the draft");
}

#[test]
fn a_model_that_returns_nothing_falls_back_to_the_subject() {
    fallback_case(writer("   \n  "), FallbackWhy::ModelSaidNothing);
}

#[test]
fn a_model_that_returns_garbage_falls_back_to_the_subject() {
    let rambling = "x".repeat(400);
    fallback_case(writer(&format!("Voici :\n---\n{rambling}\n12345")), FallbackWhy::ModelGarbage);
}

#[test]
fn a_model_that_fails_falls_back_to_the_subject_and_keeps_its_words() {
    let w = Writer { answer: Err(CoreError::Unexpected { reason_words: "model offline".into() }), prompts: log() };
    fallback_case(w, FallbackWhy::ModelFailed { reason_words: "model offline".into() });
}

#[test]
fn the_question_prompt_fills_both_variables_and_never_passes_its_budget() {
    let mut req = request();
    req.settings.retrieval_prompt = "Q from {{message}} / {{instruction}}\n// a note the model never reads".into();
    let (p, cut) = question_prompt(&req.settings, &req.message, Some("dis oui"), BIG);
    assert_eq!(p, "Q from Re: Devis toiture\n\nBonjour, pouvez-vous confirmer le prix du devis ? / dis oui");
    assert!(!cut);
    // a budget that holds the fixed part, the instruction and 10 characters of the message: the message is cut, the instruction kept whole
    let fixed = "Q from  / ".chars().count();
    let (p, cut) = question_prompt(&req.settings, &req.message, Some("dis oui"), fixed + 7 + 10);
    assert_eq!(p, "Q from Re: Devis  / dis oui");
    assert!(cut);
    for budget in [0, 3, 12, 40] {
        let (p, _) = question_prompt(&req.settings, &req.message, Some("dis oui"), budget);
        assert!(p.chars().count() <= budget, "budget {budget}: {p:?}");
    }
}

#[test]
fn the_writer_is_handed_the_cut_prompt_and_the_trace_says_it_was_cut() {
    let mut req = request();
    req.budgets.question_prompt_chars = 60;
    let w = writer("Quel prix ?");
    let o = run(&req, &w, &index(vec![]), &judge());
    let prompts = w.prompts.lock().unwrap();
    assert_eq!(prompts.len(), 1);
    assert!(prompts[0].chars().count() <= 60);
    assert!(o.trace.question_prompt_cut);
}
