//! Steps 3, 4, 5 and 7a: the size of an excerpt, the keep, the clash rules, the label line.
mod common;
use common::*;
use contracts::{ClashRule, ExcerptFields, ExcerptSize, ReplyExcerpt};
use reply_retrieval::usecases::label_line::label_line;
use reply_retrieval::*;

const MAIL: &str = "Bonjour Marc.\n\nMerci pour votre visite. Nous avons bien noté la toiture.\nLe prix du devis est de 4 200 euros. Il comprend la pose.\n\n\
La pose prend trois semaines.\n\n> Le 2 nov., Marc a écrit :\n> Quel est le prix du devis ?";

fn sized(size: ExcerptSize) -> String {
    let mut req = request();
    req.settings.excerpt_size = size;
    let o = run(&req, &writer("Quel est le prix du devis ?"), &index(vec![ex("c1", 0.9, Some(NOV_09), "", MAIL)]), &judge());
    assert_eq!(o.context.len(), 1);
    o.context[0].excerpt.text.clone()
}

#[test]
fn a_sentence_is_the_sentence_holding_the_question_words() {
    assert_eq!(sized(ExcerptSize::Sentence), "Le prix du devis est de 4 200 euros.");
}

#[test]
fn a_paragraph_is_the_paragraph_holding_the_question_words() {
    assert_eq!(sized(ExcerptSize::Paragraph), "Merci pour votre visite. Nous avons bien noté la toiture.\nLe prix du devis est de 4 200 euros. Il comprend la pose.");
}

#[test]
fn a_section_is_the_whole_excerpt_without_the_quoted_history() {
    assert_eq!(
        sized(ExcerptSize::Section),
        "Bonjour Marc.\n\nMerci pour votre visite. Nous avons bien noté la toiture.\nLe prix du devis est de 4 200 euros. Il comprend la pose.\n\nLa pose prend trois semaines."
    );
}

#[test]
fn an_excerpt_that_is_only_quoted_history_is_dropped() {
    let req = request();
    let o = run(&req, &writer("Quel prix ?"), &index(vec![ex("q", 0.9, None, "", "> tout cité\n> encore")]), &judge());
    assert!(o.context.is_empty());
    assert_eq!(dropped_with(&o, &DropReason::EmptyAfterSizing), vec!["q"]);
}

#[test]
fn keep_n_keeps_the_best_and_drops_the_rest() {
    let mut req = request();
    req.settings.keep_n = 4;
    req.settings.excerpts_per_question = 20;
    let list: Vec<FoundExcerpt> = (0..10).map(|k| ex(&format!("c{k}"), k as f32, None, "", &format!("Texte {k}."))).collect();
    let o = run(&req, &writer("Quel prix ?"), &index(list), &judge());
    assert_eq!(kept_ids(&o), vec!["c9", "c8", "c7", "c6"], "best first");
    assert_eq!(dropped_with(&o, &DropReason::BeyondKeep).len(), 6);
    // the boundary: one more than keep_n
    let five: Vec<FoundExcerpt> = (0..5).map(|k| ex(&format!("c{k}"), k as f32, None, "", &format!("Texte {k}."))).collect();
    let o = run(&req, &writer("Quel prix ?"), &index(five), &judge());
    assert_eq!(kept_ids(&o), vec!["c4", "c3", "c2", "c1"]);
    assert_eq!(dropped_with(&o, &DropReason::BeyondKeep), vec!["c0"]);
}

/// Three excerpts on the same conversation found by the same question, on three dates, plus one on another subject.
fn clash_run(rule: ClashRule, fields: ExcerptFields) -> RetrievalOutcome {
    let mut req = request();
    req.settings.clash_rule = rule;
    req.settings.attached_fields = fields;
    let list = vec![
        ex("old", 0.9, Some(SEP_15), "Devis toiture", "Le prix est de 3 900 euros."),
        ex("new", 0.8, Some(NOV_01), "RE: devis  toiture", "Le prix est de 4 200 euros."),
        ex("other", 0.7, Some(OCT_01), "Facture", "La facture est payée."),
    ];
    run(&req, &writer("Quel prix ?"), &index(list), &judge())
}

const NO_FIELDS: ExcerptFields = ExcerptFields { date: false, sender: false, folder: false, subject: false };

#[test]
fn keep_both_dated_keeps_the_pair_and_each_label_carries_its_date_and_the_other() {
    let o = clash_run(ClashRule::KeepBothDated, NO_FIELDS);
    assert_eq!(kept_ids(&o), vec!["old", "new", "other"]);
    assert_eq!(o.trace.clashes, vec![("old".to_string(), "new".to_string())]);
    let labels: Vec<&str> = o.context.iter().map(|c| c.label.as_str()).collect();
    assert_eq!(labels, vec!["[1] · 2026-09-15 · ≠ [2]", "[2] · 2026-11-01 · ≠ [1]", "[3]"], "the date shows although its tick is off");
    assert_eq!(o.context[0].clashes_with, vec![2]);
}

#[test]
fn keep_newest_drops_the_older_side_and_names_the_kept_one() {
    let o = clash_run(ClashRule::KeepNewest, NO_FIELDS);
    assert_eq!(kept_ids(&o), vec!["new", "other"]);
    assert_eq!(dropped_with(&o, &DropReason::Clash { kept: "new".into() }), vec!["old"]);
    assert!(o.context.iter().all(|c| c.clashes_with.is_empty()));
}

#[test]
fn keep_oldest_drops_the_newer_side_and_names_the_kept_one() {
    let o = clash_run(ClashRule::KeepOldest, NO_FIELDS);
    assert_eq!(kept_ids(&o), vec!["old", "other"]);
    assert_eq!(dropped_with(&o, &DropReason::Clash { kept: "old".into() }), vec!["new"]);
}

#[test]
fn keep_newest_names_the_newest_of_a_chain() {
    // a and b share the first question, b and c the second; a and c share none, so a's newer partner is b, which c beats in turn
    let mut req = request();
    req.settings.clash_rule = ClashRule::KeepNewest;
    let (a, b, c) = (ex("a", 0.9, Some(SEP_15), "Devis", "Trois mille."), ex("b", 0.8, Some(OCT_01), "Devis", "Quatre mille."), ex("c", 0.7, Some(NOV_01), "Devis", "Cinq mille."));
    let i = index_by(vec![("Quel prix ?", vec![a, b.clone()]), ("Quel montant ?", vec![b, c])]);
    let o = run(&req, &writer("Quel prix ?\nQuel montant ?"), &i, &judge());
    assert_eq!(o.trace.clashes.len(), 2, "a with b, b with c, never a with c");
    assert_eq!(kept_ids(&o), vec!["c"]);
    assert_eq!(dropped_with(&o, &DropReason::Clash { kept: "c".into() }), vec!["a", "b"], "the trace names the excerpt really kept");
}

#[test]
fn no_clash_without_both_dates_a_same_subject_a_shared_question_and_two_times() {
    let mut req = request();
    req.settings.clash_rule = ClashRule::KeepNewest;
    let w = writer("Quel prix ?\nQuel délai ?");
    let i = index_by(vec![
        ("Quel prix ?", vec![ex("undated", 0.9, None, "Devis", "Sans date."), ex("dated", 0.8, Some(NOV_01), "Devis", "Avec date."), ex("same-time", 0.7, Some(NOV_01), "Devis", "Même moment.")]),
        ("Quel délai ?", vec![ex("other-question", 0.6, Some(SEP_15), "Devis", "Une autre question.")]),
    ]);
    let o = run(&req, &w, &i, &judge());
    assert_eq!(kept_ids(&o), vec!["undated", "dated", "same-time", "other-question"]);
    assert!(o.trace.clashes.is_empty());
}

fn sample() -> ReplyExcerpt {
    ReplyExcerpt {
        chunk_id: "c".into(),
        source: mail("a1", "INBOX"),
        date_ms: Some(NOV_09),
        sender: Some("Anna Martin".into()),
        folder: Some("INBOX".into()),
        subject: Some("Re: Devis".into()),
        text: "x".into(),
    }
}

#[test]
fn the_label_line_follows_every_combination_of_the_four_ticks() {
    let values = ["2026-11-09", "Anna Martin", "INBOX", "Re: Devis"];
    for mask in 0..16u8 {
        let f = ExcerptFields { date: mask & 1 != 0, sender: mask & 2 != 0, folder: mask & 4 != 0, subject: mask & 8 != 0 };
        let mut want = String::from("[3]");
        for (bit, v) in values.iter().enumerate() {
            if mask & (1 << bit) != 0 {
                want.push_str(" · ");
                want.push_str(v);
            }
        }
        assert_eq!(label_line(3, &sample(), f, false, &[]), want, "ticks {mask:04b}");
    }
}

#[test]
fn a_ticked_field_with_no_value_is_left_out_and_a_clash_ends_the_line() {
    let all = ExcerptFields { date: true, sender: true, folder: true, subject: true };
    let e = ReplyExcerpt { date_ms: None, sender: Some("  ".into()), subject: None, ..sample() };
    assert_eq!(label_line(1, &e, all, true, &[]), "[1] · INBOX");
    assert_eq!(label_line(2, &sample(), NO_FIELDS, true, &[1, 4]), "[2] · 2026-11-09 · ≠ [1] [4]");
}

#[test]
fn the_writer_reads_each_label_above_its_text() {
    let o = clash_run(ClashRule::KeepBothDated, ExcerptFields { date: true, sender: false, folder: false, subject: true });
    assert_eq!(
        o.context_text,
        "[1] · 2026-09-15 · Devis toiture · ≠ [2]\nLe prix est de 3 900 euros.\n\n[2] · 2026-11-01 · RE: devis  toiture · ≠ [1]\nLe prix est de 4 200 euros.\n\n[3] · 2026-10-01 · Facture\nLa facture est payée."
    );
}
