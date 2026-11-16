//! Step 6, 7b and the report: the AI filter on and off, the writer's budget, the numbers that add up, determinism.
mod common;
use common::*;
use contracts::{ClashRule, ExcerptSize};
use reply_retrieval::usecases::ai_filter::{judge_prompt, read_yes_no};
use reply_retrieval::*;

fn three() -> Vec<FoundExcerpt> {
    vec![
        ex("c1", 0.9, Some(NOV_09), "", "Le prix est de 4 200 euros."),
        ex("c2", 0.8, Some(NOV_01), "", "La météo était belle."),
        ex("c3", 0.7, Some(OCT_01), "", "Le chantier commence lundi."),
    ]
}

#[test]
fn with_no_filter_question_the_judge_is_never_asked() {
    let req = request();
    let j = judge();
    let o = run(&req, &writer("Quel prix ?"), &index(three()), &j);
    assert_eq!(o.trace.ai_filter, AiFilterTrace::Off);
    assert!(j.prompts.lock().unwrap().is_empty());
    assert_eq!(kept_ids(&o), vec!["c1", "c2", "c3"]);
}

#[test]
fn with_a_filter_question_a_no_drops_and_an_unclear_answer_keeps() {
    let mut req = request();
    req.settings.ai_filter_question = Some("Cet extrait aide-t-il à répondre ?".into());
    let mut j = judge();
    j.no_when = vec!["météo".into()];
    j.unclear_when = vec!["chantier".into()];
    let o = run(&req, &writer("Quel prix ?"), &index(three()), &j);
    assert_eq!(kept_ids(&o), vec!["c1", "c3"]);
    assert_eq!(dropped_with(&o, &DropReason::AiFilter), vec!["c2"]);
    assert_eq!(o.trace.ai_filter, AiFilterTrace::On { question: "Cet extrait aide-t-il à répondre ?".into(), judged: 2, dropped: 1, unjudged: 1 });
    let prompts = j.prompts.lock().unwrap();
    assert_eq!(prompts.len(), 3, "one question per excerpt");
    assert!(prompts.iter().all(|p| p.starts_with("Cet extrait aide-t-il à répondre ?") && p.contains("Devis toiture")), "the question and the message reach the judge");
}

#[test]
fn the_judge_answer_is_read_by_its_first_word() {
    assert_eq!(read_yes_no("**Oui** — clairement"), Some(true));
    assert_eq!(read_yes_no("no."), Some(false));
    assert_eq!(read_yes_no("NON"), Some(false));
    assert_eq!(read_yes_no("I think yes"), None);
    assert_eq!(read_yes_no(""), None);
}

#[test]
fn the_judge_prompt_never_passes_its_budget_and_keeps_the_question_first() {
    let q = "Cet extrait aide-t-il ?";
    let long = "mot ".repeat(500);
    for budget in [0, 10, 60, 200, 900] {
        let p = judge_prompt(q, &long, &long, budget);
        assert!(p.chars().count() <= budget, "budget {budget}");
    }
    assert!(judge_prompt(q, &long, &long, 200).starts_with(q));
}

#[test]
fn the_writer_budget_drops_the_lowest_ranked_first_and_never_overflows() {
    let mut req = request();
    req.settings.attached_fields = contracts::ExcerptFields { date: false, sender: false, folder: false, subject: false };
    // "[1]\nLe prix est de 4 200 euros." = 31 characters; with the blank line and "[2]\nLa météo était belle." (25) = 58
    req.budgets.context_chars = 60;
    let o = run(&req, &writer("Quel prix ?"), &index(three()), &judge());
    assert_eq!(kept_ids(&o), vec!["c1", "c2"]);
    assert_eq!(dropped_with(&o, &DropReason::NoRoom), vec!["c3"]);
    assert!(o.context_text.chars().count() <= 60);
    assert!(!o.trace.context_cut);
}

#[test]
fn a_best_excerpt_bigger_than_the_whole_budget_is_cut_not_refused() {
    let mut req = request();
    req.settings.attached_fields = contracts::ExcerptFields { date: false, sender: false, folder: false, subject: false };
    req.budgets.context_chars = 14;
    let o = run(&req, &writer("Quel prix ?"), &index(three()), &judge());
    assert_eq!(o.context_text, "[1]\nLe prix es");
    assert!(o.trace.context_cut);
    assert_eq!(dropped_with(&o, &DropReason::NoRoom), vec!["c2", "c3"]);
}

#[test]
fn a_clash_partner_cut_for_room_leaves_no_dangling_number() {
    let mut req = request();
    req.settings.attached_fields = contracts::ExcerptFields { date: false, sender: false, folder: false, subject: false };
    let list = vec![ex("old", 0.9, Some(SEP_15), "Devis", "Trois mille."), ex("new", 0.8, Some(NOV_01), "Devis", "Quatre mille euros, la nouvelle offre.")];
    req.budgets.context_chars = 40;
    let o = run(&req, &writer("Quel prix ?"), &index(list), &judge());
    assert_eq!(kept_ids(&o), vec!["old"]);
    assert_eq!(o.context[0].label, "[1]");
    assert!(o.context[0].clashes_with.is_empty());
}

/// Every drop reason at once: a leak, a merge, a duplicate, an empty excerpt, one beyond the keep, a clash, a no from the judge.
fn everything() -> RetrievalOutcome {
    let mut req = request();
    req.settings.keep_n = 4;
    req.settings.clash_rule = ClashRule::KeepNewest;
    req.settings.excerpt_size = ExcerptSize::Section;
    req.settings.ai_filter_question = Some("Utile ?".into());
    let w = writer("Quel prix ?\nQuel délai ?");
    let i = index_by(vec![
        (
            "Quel prix ?",
            vec![
                ex_in(mail("a2", "INBOX"), "leak", 0.99, Some(NOV_09), "", "Autre compte."),
                ex("p1", 0.95, Some(NOV_09), "Devis", "Le prix est de 4 200 euros."),
                ex("p0", 0.94, Some(SEP_15), "Devis", "Le prix est de 3 900 euros."),
                ex("dup", 0.9, Some(NOV_01), "", "le prix est de 4 200 euros"),
                ex("quoted", 0.85, Some(NOV_01), "", "> seulement cité"),
            ],
        ),
        (
            "Quel délai ?",
            vec![
                ex("p1", 0.5, Some(NOV_09), "Devis", "Le prix est de 4 200 euros."),
                ex("d1", 0.8, Some(OCT_01), "", "Le délai est de trois semaines."),
                ex("meteo", 0.7, Some(OCT_01), "", "La météo."),
                ex("low", 0.1, Some(OCT_01), "", "Rien d'utile."),
            ],
        ),
    ]);
    let mut j = judge();
    j.no_when = vec!["météo".into()];
    run(&req, &w, &i, &j)
}

#[test]
fn the_reports_numbers_add_up_with_every_drop_reason() {
    let o = everything();
    let t = &o.trace;
    assert_eq!(t.hits, 9);
    assert_eq!(t.merged, 1);
    assert_eq!(dropped_with(&o, &DropReason::OutsideScope), vec!["leak"]);
    assert_eq!(dropped_with(&o, &DropReason::EmptyAfterSizing), vec!["quoted"]);
    assert_eq!(dropped_with(&o, &DropReason::Duplicate { of: "p1".into() }), vec!["dup"]);
    assert_eq!(dropped_with(&o, &DropReason::BeyondKeep), vec!["low"]);
    assert_eq!(dropped_with(&o, &DropReason::Clash { kept: "p1".into() }), vec!["p0"]);
    assert_eq!(dropped_with(&o, &DropReason::AiFilter), vec!["meteo"]);
    assert_eq!(kept_ids(&o), vec!["p1", "d1"]);
    assert_eq!(t.kept, 2);
    assert!(t.adds_up(), "9 hits = 1 merged + 6 dropped + 2 kept");
    let r = o.report();
    assert_eq!(r.questions, vec!["Quel prix ?", "Quel délai ?"]);
    assert_eq!(r.found, 7, "9 hits less the merge and the leak");
    assert_eq!(r.kept.iter().map(|e| e.chunk_id.as_str()).collect::<Vec<_>>(), vec!["p1", "d1"]);
    assert_eq!(r.dropped_by_ai_filter, 1);
}

#[test]
fn a_trace_that_lost_a_count_does_not_add_up() {
    let mut t = everything().trace;
    t.merged += 1;
    assert!(!t.adds_up());
    let mut t = everything().trace;
    t.questions[0].found -= 1;
    assert!(!t.adds_up());
}

#[test]
fn the_same_port_answers_give_the_same_outcome() {
    assert_eq!(everything(), everything());
}
