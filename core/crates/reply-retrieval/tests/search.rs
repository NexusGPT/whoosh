//! Step 2 and 4: the search per question, the scope behind the account wall, the merge and the de-duplication.
mod common;
use common::*;
use contracts::{ReplySourceRef, RetrievalScope};
use reply_retrieval::*;

fn many(prefix: &str, n: usize) -> Vec<FoundExcerpt> {
    // scores rise with the index, so the best come LAST from this index: the crate must sort, not trust the order
    (0..n).map(|k| ex(&format!("{prefix}{k}"), k as f32 / 10.0, Some(NOV_09 - k as i64), "", &format!("Texte {prefix} numéro {k}."))).collect()
}

#[test]
fn each_question_keeps_at_most_excerpts_per_question_the_best_scored() {
    let mut req = request();
    req.settings.excerpts_per_question = 5;
    req.settings.keep_n = 50;
    let w = writer("Quel prix ?\nQuel délai ?");
    let i = index_by(vec![("Quel prix ?", many("p", 8)), ("Quel délai ?", many("d", 2))]);
    let o = run(&req, &w, &i, &judge());
    assert!(i.queries.lock().unwrap().iter().all(|q| q.limit == 5), "the index is asked for 5");
    let found: Vec<u32> = o.trace.questions.iter().map(|q| q.found).collect();
    assert_eq!(found, vec![5, 2]);
    let mut kept = kept_ids(&o);
    kept.sort();
    assert_eq!(kept, vec!["d0", "d1", "p3", "p4", "p5", "p6", "p7"], "the 5 best of 8, never the first 5 returned");
}

#[test]
fn a_chunk_found_by_two_questions_is_one_excerpt_and_a_repeated_text_is_dropped() {
    let mut req = request();
    req.settings.excerpt_size = contracts::ExcerptSize::Section;
    let w = writer("Quel prix ?\nQuel délai ?");
    let i = index_by(vec![
        ("Quel prix ?", vec![ex("c1", 0.9, Some(NOV_09), "", "Le prix est de 4 200 euros."), ex("c2", 0.5, Some(NOV_01), "", "  le PRIX est de 4 200   euros ")]),
        ("Quel délai ?", vec![ex("c1", 0.4, Some(NOV_09), "", "Le prix est de 4 200 euros."), ex("c3", 0.3, Some(NOV_01), "", "Le délai est de trois semaines.")]),
    ]);
    let o = run(&req, &w, &i, &judge());
    assert_eq!(o.trace.hits, 4);
    assert_eq!(o.trace.merged, 1, "c1 twice is one excerpt");
    assert_eq!(kept_ids(&o), vec!["c1", "c3"]);
    assert_eq!(dropped_with(&o, &DropReason::Duplicate { of: "c1".into() }), vec!["c2"], "the same words, case and spacing aside");
}

#[test]
fn this_account_searches_its_enabled_folders_and_the_explicit_sources_only() {
    let req = request();
    let i = index(vec![]);
    run(&req, &writer("Quel prix ?"), &i, &judge());
    let q = i.queries.lock().unwrap()[0].clone();
    assert_eq!(
        q.sources,
        vec![mail("a1", "INBOX"), mail("a1", "Clients"), ReplySourceRef::LocalFolder { path: "~/Documents/Devis".into() }, ReplySourceRef::Calendar { calendar_id: "cal-1".into() }]
    );
}

#[test]
fn all_accounts_adds_the_other_accounts_enabled_folders() {
    let mut req = request();
    req.settings.scope = RetrievalScope::AllAccounts;
    let i = index(vec![]);
    run(&req, &writer("Quel prix ?"), &i, &judge());
    let q = i.queries.lock().unwrap()[0].clone();
    assert_eq!(
        q.sources,
        vec![mail("a1", "INBOX"), mail("a1", "Clients"), mail("a2", "INBOX"), ReplySourceRef::LocalFolder { path: "~/Documents/Devis".into() }, ReplySourceRef::Calendar { calendar_id: "cal-1".into() }]
    );
}

#[test]
fn an_excerpt_from_outside_the_scope_never_reaches_the_writer() {
    let req = request();
    let i = index(vec![
        ex_in(mail("a2", "INBOX"), "other-account", 0.99, Some(NOV_09), "", "Le prix chez l'autre compte."),
        ex_in(mail("a1", "Archive"), "switched-off", 0.98, Some(NOV_09), "", "Un dossier éteint."),
        ex("mine", 0.5, Some(NOV_09), "", "Le prix est de 4 200 euros."),
    ]);
    let o = run(&req, &writer("Quel prix ?"), &i, &judge());
    assert_eq!(kept_ids(&o), vec!["mine"]);
    assert_eq!(dropped_with(&o, &DropReason::OutsideScope), vec!["other-account", "switched-off"]);
    assert!(!o.context_text.contains("autre compte"));
}

#[test]
fn with_no_source_in_scope_nothing_is_asked_and_nothing_searched() {
    let mut req = request();
    for a in &mut req.sources.accounts {
        for f in &mut a.folders {
            f.enabled = false;
        }
    }
    req.sources.local_folders.clear();
    req.sources.calendars.clear();
    let (w, i) = (writer("Quel prix ?"), index(vec![ex("c1", 0.9, None, "", "x")]));
    let o = run(&req, &w, &i, &judge());
    assert_eq!(o.trace.origin, QuestionOrigin::NoSourceInScope);
    assert!(w.prompts.lock().unwrap().is_empty() && i.queries.lock().unwrap().is_empty());
    assert!(o.context.is_empty() && o.trace.adds_up());
}

#[test]
fn a_failed_search_costs_its_question_only() {
    let req = request();
    let mut i = index_by(vec![("Quel délai ?", vec![ex("c3", 0.3, Some(NOV_01), "", "Le délai est de trois semaines.")])]);
    i.fail = vec!["Quel prix ?".into()];
    let o = run(&req, &writer("Quel prix ?\nQuel délai ?"), &i, &judge());
    assert_eq!(o.trace.questions[0].search_failed.as_deref(), Some("index unavailable"));
    assert_eq!(o.trace.questions[1].search_failed, None);
    assert_eq!(kept_ids(&o), vec!["c3"]);
}
