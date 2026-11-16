//! The decision: the person's hard rules override everything, the thresholds cut the score into keep / suspect /
//! move, and the signals combine as independent chances of junk.
mod support;

use junk::tuning::*;
use junk::{combine_signals, Contact, Context, Decision, Filter, HardRule, HardSource, RuleId, Thresholds};
use support::{ham_probe, spam_probe};

fn bootstrapped() -> Filter {
    let mut f = Filter::default();
    f.bootstrap(&support::spam(), &support::ham());
    f
}

fn blocked_context(rule: HardRule) -> Context {
    Context { contacts: Vec::new(), hard_rules: vec![rule] }
}

#[test]
fn a_blocked_sender_moves_whatever_the_model_thinks() {
    let f = bootstrapped();
    let mut m = ham_probe();
    m.from_email = "blocked@evil.example".to_string();
    assert_eq!(f.assess(&m, &Context::default()).decision, Decision::Keep, "without the block this is ordinary mail");
    let v = f.assess(&m, &blocked_context(HardRule::block_sender("Blocked@Evil.example")));
    assert_eq!(v.decision, Decision::Move);
    assert_eq!(v.score, 1.0);
    assert_eq!(v.hard, Some(HardSource::Block));
    assert_eq!(v.bayes, None);
}

#[test]
fn an_always_junk_domain_covers_its_subdomains_and_nothing_that_merely_ends_alike() {
    let f = Filter::default();
    let ctx = blocked_context(HardRule::always_junk_domain("@Promo.example"));
    for (from, moved) in [("x@promo.example", true), ("x@mail.promo.example", true), ("x@notpromo.example", false), ("x@promo.example.org", false)] {
        let mut m = ham_probe();
        m.from_email = from.to_string();
        let v = f.assess(&m, &ctx);
        assert_eq!(v.decision == Decision::Move, moved, "{from}");
        if moved {
            assert_eq!(v.hard, Some(HardSource::AlwaysJunk));
        }
    }
}

#[test]
fn a_block_beats_a_known_contact() {
    let f = Filter::default();
    let mut m = ham_probe();
    m.from_email = "anna@work.example".to_string();
    let ctx = Context {
        contacts: vec![Contact { name: "Anna Smith".into(), emails: vec!["anna@work.example".into()] }],
        hard_rules: vec![HardRule::block_sender("anna@work.example")],
    };
    assert_eq!(f.assess(&m, &ctx).decision, Decision::Move);
}

#[test]
fn the_default_cut_offs_are_the_ledger_numbers() {
    let t = Thresholds::default();
    assert_eq!(t.move_at, JUNK_MOVE_THRESHOLD);
    assert_eq!(t.review_at, JUNK_REVIEW_THRESHOLD);
    assert_eq!((t.move_at, t.review_at), (0.90, 0.70));
}

#[test]
fn the_move_cut_off_is_inclusive_and_the_review_cut_off_too() {
    let t = Thresholds::default();
    assert_eq!(t.decide(1.0), Decision::Move);
    assert_eq!(t.decide(0.90), Decision::Move, "equal to the move cut-off moves");
    assert_eq!(t.decide(0.8999999), Decision::Suspect);
    assert_eq!(t.decide(0.70), Decision::Suspect, "equal to the review cut-off is suspected");
    assert_eq!(t.decide(0.6999999), Decision::Keep);
    assert_eq!(t.decide(0.0), Decision::Keep);
    let tuned = Thresholds::new(0.99, 0.5).expect("a valid pair");
    assert_eq!(tuned.decide(0.95), Decision::Suspect, "the person raised the bar");
}

#[test]
fn cut_offs_that_do_not_make_sense_are_refused() {
    assert!(Thresholds::new(0.9, 0.95).is_err(), "review above move");
    assert!(Thresholds::new(1.1, 0.5).is_err(), "above one");
    assert!(Thresholds::new(0.9, 0.0).is_err(), "review at zero would suspect everything");
    assert!(Thresholds::new(f64::NAN, 0.5).is_err());
    assert!(Thresholds::new(0.8, 0.8).is_ok(), "no suspect band is allowed");
}

#[test]
fn keep_suspect_and_move_through_real_messages() {
    // Keep: nothing learned, no header trips: no opinion, nothing moves.
    let untrained = Filter::default();
    let plain = untrained.assess(&spam_probe(), &Context::default());
    assert_eq!(plain.bayes, None, "day one: the model has no opinion");
    assert_eq!(plain.score, 0.0);
    assert_eq!(plain.decision, Decision::Keep);

    // Suspect: three failed authentication verdicts are 0.755 on their own: above 0.70, below 0.90.
    let mut suspect = ham_probe();
    suspect.authentication_results = vec!["mx.example.net; spf=fail smtp.mailfrom=a.example; dkim=fail header.d=a.example; dmarc=fail header.from=a.example".to_string()];
    let v = untrained.assess(&suspect, &Context::default());
    let expect = 1.0 - (1.0 - RULE_SPF_FAIL) * (1.0 - RULE_DKIM_FAIL) * (1.0 - RULE_DMARC_FAIL);
    assert!((v.score - expect).abs() < 1e-9, "{} against {expect}", v.score);
    assert_eq!(v.rules, vec![RuleId::SpfFail, RuleId::DkimFail, RuleId::DmarcFail]);
    assert_eq!(v.decision, Decision::Suspect);

    // Move: a trained model recognises junk.
    let trained = bootstrapped();
    let moved = trained.assess(&spam_probe(), &Context::default());
    assert!(moved.bayes.expect("trained") > 0.9);
    assert_eq!(moved.decision, Decision::Move);
    let kept = trained.assess(&ham_probe(), &Context::default());
    assert_eq!(kept.decision, Decision::Keep);
    let neutral = trained.assess(&support::unknown_probe(), &Context::default());
    assert_eq!(neutral.bayes, Some(0.5));
    assert_eq!(neutral.decision, Decision::Keep, "a message the model knows nothing about stays");
}

#[test]
fn header_rules_cannot_overrule_a_confident_model_alone() {
    let trained = bootstrapped();
    let mut m = ham_probe();
    m.authentication_results = vec!["mx.example.net; dmarc=fail header.from=a.example".to_string()];
    let v = trained.assess(&m, &Context::default());
    assert!(v.score < JUNK_REVIEW_THRESHOLD, "one failed verdict on mail the model is sure about: {}", v.score);
    assert_eq!(v.decision, Decision::Keep);
}

#[test]
fn signals_combine_as_independent_chances() {
    assert_eq!(combine_signals(None, &[], None), 0.0);
    assert!((combine_signals(Some(0.5), &[], None) - 0.5).abs() < 1e-12);
    assert!((combine_signals(None, &[RuleId::DmarcFail], None) - RULE_DMARC_FAIL).abs() < 1e-12);
    assert!((combine_signals(Some(0.5), &[RuleId::DmarcFail], None) - 0.75).abs() < 1e-12);
    assert!((combine_signals(Some(0.02), &[RuleId::DmarcFail], None) - (1.0 - 0.98 * 0.5)).abs() < 1e-12);
    let all = [RuleId::SpfFail, RuleId::SpfSoftfail, RuleId::DkimFail, RuleId::DmarcFail, RuleId::ReplyToMismatch, RuleId::DisplayNameImpersonation, RuleId::LinkMismatch];
    let mut last = 0.0;
    for n in 0..=all.len() {
        let s = combine_signals(Some(0.3), &all[..n], None);
        assert!(s >= last, "one more rule never lowers the score");
        assert!((0.0..=1.0).contains(&s));
        last = s;
    }
    assert_eq!(combine_signals(Some(1.0), &all, None), 1.0);
}
