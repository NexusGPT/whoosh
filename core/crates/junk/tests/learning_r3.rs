//! junk-r3: what the owner's two acts do to the score of a look-alike, on the filter alone (the engine's own tests, engine/tests/junk_r3.rs,
//! prove the same through the sync). The vocabulary below is invented: no word of it is in the support corpora, so the model has no opinion
//! of it until the owner teaches one.
mod support;

use junk::{Context, Decision, Filter, Label, Message};
use support::msg;

fn z(i: u32) -> Message {
    msg(Some("Zorbix"), "promo@zorbix.example", &format!("Zorbix gizmo {i}"), "Zorbix gizmo quartz voucher plinth")
}

fn bootstrapped() -> Filter {
    let mut f = Filter::default();
    f.bootstrap(&support::spam(), &support::ham());
    f
}

#[test]
fn a_word_the_model_never_met_gives_no_opinion() {
    let v = bootstrapped().assess(&z(1), &Context::default());
    assert_eq!((v.bayes, v.decision), (Some(0.5), Decision::Keep), "an unknown vocabulary is 0.5, below both cut-offs");
}

#[test]
fn a_junk_lesson_moves_a_look_alike_and_taking_it_back_stops_it() {
    let ctx = Context::default();
    let mut f = bootstrapped();
    f.learn(&z(1), Label::Junk);
    let v = f.assess(&z(2), &ctx);
    assert_eq!(v.decision, Decision::Move, "one junk lesson on a fresh vocabulary is enough: {v:?}");
    f.relearn(&z(1), Label::Junk, Label::NotJunk);
    let v = f.assess(&z(3), &ctx);
    assert_eq!(v.decision, Decision::Keep, "the owner changed their mind: the look-alike stays: {v:?}");
    assert!(v.score < 0.1, "{v:?}");
}

#[test]
fn three_junk_lessons_move_the_fourth_alike_and_a_not_junk_lesson_lowers_it() {
    let ctx = Context::default();
    let mut f = bootstrapped();
    for i in 1..=3 { f.learn(&z(i), Label::Junk); }
    let high = f.assess(&z(4), &ctx);
    assert_eq!(high.decision, Decision::Move, "{high:?}");
    f.learn(&z(4), Label::NotJunk);
    let lower = f.assess(&z(5), &ctx);
    assert!(lower.score < high.score, "a not-junk lesson on the same vocabulary lowers the score: {} then {}", high.score, lower.score);
}
