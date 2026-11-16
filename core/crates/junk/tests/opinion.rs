//! The on-device model as one more signal (`ModelOpinion`): asked only for a suspect message (score from the other signals at or above the
//! review cut-off and below the move cut-off), absent changes nothing, a hard rule of the person wins before anything is asked, and the
//! opinion never reaches beyond this Mac (that guard is in core/crates/engine/tests/engine_junk_opinion.rs: this crate has no dependency).
//!
//! BROKEN COPIES (one edit each in core/crates/junk/src/filter.rs; each must turn the named tests red):
//!   O1 combine_signals ignores the opinion: `(1.0 - opinion.unwrap_or(0.0).clamp(0.0, 1.0))` -> `1.0`
//!        -> an_opinion_of_095_alone_moves_a_suspect_message, an_opinion_folds_in_as_one_more_independent_chance
//!   O2 the band check removed: `if self.thresholds.in_suspect_band(base) {` -> `if true {`
//!        -> a_message_outside_the_band_never_asks, the_suspect_band_is_review_inclusive_and_move_exclusive_from_the_ledger_numbers
//!           (the second needs `in_suspect_band` itself broken: `score >= self.review_at && score < self.move_at` -> `true`)
//!   O3 the hard rule no longer first: the `ctx.hard_rules` early return removed -> a_hard_rule_still_wins_and_nothing_is_asked
mod support;

use std::cell::Cell;

use junk::tuning::*;
use junk::{combine_signals, Context, Decision, Filter, HardRule, HardSource, Message, ModelOpinion, NoOpinion, RuleId, Thresholds};
use support::{ham_probe, spam_probe};

// ---------------------------------------------------------------- fixtures

/// Three failed authentication verdicts: 0.755 on their own, inside the suspect band [0.70, 0.90).
fn suspect_message() -> Message {
    let mut m = ham_probe();
    m.authentication_results = vec!["mx.example.net; spf=fail smtp.mailfrom=a.example; dkim=fail header.d=a.example; dmarc=fail header.from=a.example".to_string()];
    m
}

/// An opinion that counts how often it is asked and answers a fixed chance.
struct Counting {
    asked: Cell<usize>,
    answer: Option<f64>,
}
impl Counting {
    fn new(answer: Option<f64>) -> Counting { Counting { asked: Cell::new(0), answer } }
}
impl ModelOpinion for Counting {
    fn chance_junk(&self, _msg: &Message) -> Option<f64> {
        self.asked.set(self.asked.get() + 1);
        self.answer
    }
}

fn trained() -> Filter {
    let mut f = Filter::default();
    f.bootstrap(&support::spam(), &support::ham());
    f
}

// ---------------------------------------------------------------- the tests

#[test]
fn an_opinion_of_095_alone_moves_a_suspect_message() {
    let f = Filter::default();
    let m = suspect_message();
    let without = f.assess(&m, &Context::default());
    assert_eq!(without.decision, Decision::Suspect, "the fixture is suspect on the other signals");
    let op = Counting::new(Some(0.95));
    let v = f.assess_with(&m, &Context::default(), &op);
    assert_eq!(v.decision, Decision::Move, "score {}", v.score);
    assert_eq!(v.opinion, Some(0.95));
    assert_eq!(op.asked.get(), 1, "one question for one suspect message");
    assert!(v.score >= JUNK_MOVE_THRESHOLD && v.score > without.score);
}

#[test]
fn an_opinion_of_one_half_does_not_move_a_suspect_message() {
    let f = Filter::default();
    let v = f.assess_with(&suspect_message(), &Context::default(), &Counting::new(Some(0.5)));
    assert_eq!(v.decision, Decision::Suspect, "score {}", v.score);
    assert_eq!(v.opinion, Some(0.5));
    assert!(v.score < JUNK_MOVE_THRESHOLD);
}

#[test]
fn an_absent_opinion_changes_nothing() {
    let f = trained();
    for m in [suspect_message(), spam_probe(), ham_probe(), support::unknown_probe()] {
        let plain = f.assess(&m, &Context::default());
        let absent = Counting::new(None);
        let with_none = f.assess_with(&m, &Context::default(), &absent);
        assert_eq!(with_none, plain, "an opinion that answers None leaves the verdict as it was");
        assert_eq!(f.assess_with(&m, &Context::default(), &NoOpinion), plain, "no provider is the same as None");
        assert_eq!(plain.opinion, None);
    }
    // a provider that answers something that is not a chance is no opinion either
    let nan = Counting::new(Some(f64::NAN));
    let v = f.assess_with(&suspect_message(), &Context::default(), &nan);
    assert_eq!(v, f.assess(&suspect_message(), &Context::default()));
}

#[test]
fn a_hard_rule_still_wins_and_nothing_is_asked() {
    let f = trained();
    let mut m = suspect_message();
    m.from_email = "blocked@evil.example".to_string();
    let ctx = Context { contacts: Vec::new(), hard_rules: vec![HardRule::block_sender("blocked@evil.example")] };
    // an opinion that says "clean" cannot undo a block, and a block never asks
    let op = Counting::new(Some(0.0));
    let v = f.assess_with(&m, &ctx, &op);
    assert_eq!((v.decision, v.score, v.hard, v.opinion), (Decision::Move, 1.0, Some(HardSource::Block), None));
    assert_eq!(op.asked.get(), 0, "a hard rule decides before the model is asked");
}

#[test]
fn an_opinion_cannot_clear_a_message_the_others_moved_and_only_ever_raises() {
    let f = trained();
    let v = f.assess_with(&spam_probe(), &Context::default(), &Counting::new(Some(0.0)));
    assert_eq!(v.decision, Decision::Move);
    let base = f.assess(&suspect_message(), &Context::default());
    let low = f.assess_with(&suspect_message(), &Context::default(), &Counting::new(Some(0.0)));
    assert_eq!(low.score, base.score, "an opinion of 0 lowers nothing");
}

#[test]
fn a_message_outside_the_band_never_asks() {
    let f = trained();
    let op = Counting::new(Some(0.95));
    // below the band: clean mail, and mail nothing is known about
    let kept = f.assess_with(&ham_probe(), &Context::default(), &op);
    let neutral = f.assess_with(&support::unknown_probe(), &Context::default(), &op);
    assert_eq!((kept.decision, neutral.decision), (Decision::Keep, Decision::Keep), "an opinion of 0.95 did not move mail below the band");
    // at or above the move cut-off: already moved
    let moved = f.assess_with(&spam_probe(), &Context::default(), &op);
    assert_eq!(moved.decision, Decision::Move);
    assert_eq!(op.asked.get(), 0, "no ask for a message below the band or at the move cut-off");
    // inside the band: exactly one ask per message
    f.assess_with(&suspect_message(), &Context::default(), &op);
    f.assess_with(&suspect_message(), &Context::default(), &op);
    assert_eq!(op.asked.get(), 2);
}

#[test]
fn the_suspect_band_is_review_inclusive_and_move_exclusive_from_the_ledger_numbers() {
    let t = Thresholds::default();
    assert_eq!((t.review_at, t.move_at), (JUNK_REVIEW_THRESHOLD, JUNK_MOVE_THRESHOLD), "the band is the ledger's, not typed here");
    assert!(t.in_suspect_band(JUNK_REVIEW_THRESHOLD), "the review cut-off itself is in the band");
    assert!(!t.in_suspect_band(JUNK_MOVE_THRESHOLD), "the move cut-off itself is out: it moves already");
    assert!(t.in_suspect_band((JUNK_REVIEW_THRESHOLD + JUNK_MOVE_THRESHOLD) / 2.0));
    assert!(!t.in_suspect_band(JUNK_REVIEW_THRESHOLD - 1e-9));
    assert!(!t.in_suspect_band(1.0) && !t.in_suspect_band(0.0));
    // a person's own cut-offs move the band with them; no band at all when review == move
    let tuned = Thresholds::new(0.99, 0.5).expect("valid");
    assert!(tuned.in_suspect_band(0.95) && !tuned.in_suspect_band(0.4));
    assert!(!Thresholds::new(0.8, 0.8).expect("valid").in_suspect_band(0.8));
}

#[test]
fn an_opinion_folds_in_as_one_more_independent_chance() {
    assert_eq!(combine_signals(None, &[], None), 0.0);
    assert!((combine_signals(None, &[], Some(0.4)) - 0.4).abs() < 1e-12);
    assert!((combine_signals(Some(0.5), &[], Some(0.5)) - 0.75).abs() < 1e-12);
    assert!((combine_signals(None, &[RuleId::DmarcFail], Some(0.5)) - (1.0 - (1.0 - RULE_DMARC_FAIL) * 0.5)).abs() < 1e-12);
    assert_eq!(combine_signals(Some(0.3), &[], Some(1.0)), 1.0);
    let mut last = 0.0;
    for p in [0.0, 0.2, 0.5, 0.8, 1.0] {
        let s = combine_signals(Some(0.3), &[RuleId::SpfFail], Some(p));
        assert!(s >= last && (0.0..=1.0).contains(&s), "a higher opinion never lowers the score");
        last = s;
    }
}

#[test]
fn a_late_opinion_re_scores_one_stored_message() {
    let f = Filter::default();
    let stored = f.assess(&suspect_message(), &Context::default());
    let (score, decision) = f.with_late_opinion(stored.score, Some(0.95));
    let direct = f.assess_with(&suspect_message(), &Context::default(), &Counting::new(Some(0.95)));
    assert!((score - direct.score).abs() < 1e-12, "a late opinion gives the score an early one would have");
    assert_eq!((decision, direct.decision), (Decision::Move, Decision::Move));
    assert_eq!(f.with_late_opinion(stored.score, None), (stored.score, Decision::Suspect), "absent: nothing changes");
    assert_eq!(f.with_late_opinion(stored.score, Some(0.5)).1, Decision::Suspect);
}
