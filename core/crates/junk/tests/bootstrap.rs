//! First run from the account's Junk folder and inbox, and learning from the person's junk / not-junk actions.
mod support;

use junk::tuning::BOOTSTRAP_MAX_PER_CLASS;
use junk::{Context, Decision, Filter, Label};
use support::{ham_probe, msg, spam, spam_probe, unknown_probe};

fn bootstrapped() -> Filter {
    let mut f = Filter::default();
    f.bootstrap(&spam(), &support::ham());
    f
}

#[test]
fn bootstrap_from_a_junk_set_and_an_inbox_set() {
    let mut f = Filter::default();
    let ctx = Context::default();
    assert_eq!(f.assess(&spam_probe(), &ctx).decision, Decision::Keep, "before the first run nothing moves");
    let report = f.bootstrap(&spam(), &support::ham());
    assert_eq!((report.junk_learned, report.good_learned), (36, 36));
    assert_eq!(f.model.trained(), (36, 36));
    assert_eq!(f.assess(&spam_probe(), &ctx).decision, Decision::Move);
    assert_eq!(f.assess(&ham_probe(), &ctx).decision, Decision::Keep);
    assert_eq!(f.assess(&unknown_probe(), &ctx).bayes, Some(0.5));
}

#[test]
fn a_junk_folder_alone_gives_no_opinion() {
    let mut f = Filter::default();
    f.bootstrap(&spam(), &[]);
    let v = f.assess(&spam_probe(), &Context::default());
    assert_eq!(v.bayes, None, "one class is not enough to tell the two apart");
    assert_eq!(v.decision, Decision::Keep);
}

#[test]
fn bootstrap_reads_a_bounded_number_of_messages_and_skips_empty_ones() {
    let first = spam()[0].clone();
    let many: Vec<_> = std::iter::repeat(first.clone()).take(BOOTSTRAP_MAX_PER_CLASS + 100).collect();
    let mut f = Filter::default();
    let r = f.bootstrap(&many, &[]);
    assert_eq!(r.junk_learned, BOOTSTRAP_MAX_PER_CLASS);
    assert_eq!(f.model.trained().0 as usize, BOOTSTRAP_MAX_PER_CLASS);

    let mut g = Filter::default();
    let empty = msg(None, "", "", "");
    let r2 = g.bootstrap(&[empty, first], &[]);
    assert_eq!(r2.junk_learned, 1, "a message with no tokens teaches nothing");
}

#[test]
fn a_junk_action_moves_the_next_similar_message_and_a_change_of_mind_flips_it_back() {
    let mut f = bootstrapped();
    let ctx = Context::default();
    let probe = msg(None, "x@fresh.example", "", "zeppelin");
    assert_eq!(f.assess(&probe, &ctx).decision, Decision::Keep);
    assert_eq!(f.assess(&probe, &ctx).bayes, Some(0.5), "the model has never seen the word");

    let marked = msg(None, "y@zep-seller.example", "", "zeppelin");
    for _ in 0..3 {
        f.learn(&marked, Label::Junk);
    }
    let after = f.assess(&probe, &ctx);
    assert!(after.bayes.expect("ready") > 0.9, "three junk actions on one word: {:?}", after.bayes);
    assert_eq!(after.decision, Decision::Move);

    for _ in 0..3 {
        f.relearn(&marked, Label::Junk, Label::NotJunk);
    }
    let flipped = f.assess(&probe, &ctx);
    assert!(flipped.bayes.expect("ready") < 0.1, "{:?}", flipped.bayes);
    assert_eq!(flipped.decision, Decision::Keep);

    let mut direct = bootstrapped();
    for _ in 0..3 {
        direct.learn(&marked, Label::NotJunk);
    }
    assert_eq!(f.model, direct.model, "changing one's mind leaves the table as if the first action never happened");
}

#[test]
fn unlearning_takes_a_lesson_back() {
    let mut f = bootstrapped();
    let before = f.model.clone();
    let m = msg(None, "y@zep-seller.example", "Hello", "zeppelin tickets");
    f.learn(&m, Label::Junk);
    assert_ne!(f.model, before);
    f.unlearn(&m, Label::Junk);
    assert_eq!(f.model, before);
}
