//! The word model: train, score, the gate, smoothing, chi-square bounds, untraining.
mod support;

use junk::tuning::MODEL_MIN_PER_CLASS;
use junk::{chi2q, combine, tokenise, Model};

fn trained() -> Model {
    let mut m = Model::new();
    for s in support::spam() {
        m.train(&tokenise(&s), true);
    }
    for h in support::ham() {
        m.train(&tokenise(&h), false);
    }
    m
}

#[test]
fn train_and_score_round_trip_on_the_fixture_corpora() {
    let m = trained();
    assert_eq!(m.trained(), (36, 36));
    assert!(m.is_ready());
    let spam = m.score(&tokenise(&support::spam_probe())).expect("model is ready");
    let ham = m.score(&tokenise(&support::ham_probe())).expect("model is ready");
    assert!(spam > 0.95, "a held-out junk message scores {spam}");
    assert!(ham < 0.05, "a held-out not-junk message scores {ham}");
    for s in [spam, ham] {
        assert!((0.0..=1.0).contains(&s), "a score is a probability: {s}");
    }
}

#[test]
fn words_never_seen_say_nothing() {
    let m = trained();
    assert_eq!(m.score(&tokenise(&support::unknown_probe())), Some(0.5));
}

#[test]
fn the_model_gives_no_opinion_until_it_has_learned_enough_of_both_classes() {
    let mut m = Model::new();
    let probe = tokenise(&support::spam_probe());
    assert_eq!(m.score(&probe), None, "nothing learned");
    for s in support::spam().iter().take(MODEL_MIN_PER_CLASS as usize) {
        m.train(&tokenise(s), true);
    }
    assert_eq!(m.score(&probe), None, "junk only: one class is not an opinion");
    for h in support::ham().iter().take(MODEL_MIN_PER_CLASS as usize - 1) {
        m.train(&tokenise(h), false);
    }
    assert_eq!(m.score(&probe), None, "one message short of the minimum on the not-junk side");
    m.train(&tokenise(&support::ham()[0]), false);
    assert!(m.score(&probe).is_some(), "both classes at the minimum");
}

#[test]
fn a_token_seen_once_is_never_decisive() {
    let mut m = Model::new();
    let common = vec!["common".to_string()];
    for _ in 0..36 {
        m.train(&common, false);
    }
    for i in 0..36 {
        let mut t = common.clone();
        if i == 0 {
            t.push("rare".to_string());
        }
        m.train(&t, true);
    }
    let rare = m.probability_of("rare").expect("seen once");
    assert!((rare - 1.225 / 1.45).abs() < 1e-9, "Robinson: (s*x + n*p) / (s + n) with s 0.45, x 0.5, n 1, p 1 gives {rare}");
    assert!(rare > 0.8 && rare < 0.9, "{rare}");
    let both = m.probability_of("common").expect("seen in every message");
    assert!((both - 0.5).abs() < 1e-9, "a word in every message of both classes is neutral: {both}");
    assert_eq!(m.probability_of("never-seen"), None);
}

#[test]
fn a_token_seen_in_many_junk_messages_and_no_other_is_strong() {
    let m = trained();
    let free = m.probability_of("free").expect("in the corpus");
    // 36 junk lines and a Robinson prior of 0.5 with strength 1 put a never-ham token near 0.94, not above 0.95 (measured 2026-11-09)
    assert!(free > 0.9, "{free}");
    let notes = m.probability_of("notes").expect("in the corpus");
    assert!(notes < 0.05, "{notes}");
}

#[test]
fn chi2q_matches_its_closed_forms() {
    assert!((chi2q(2.0, 2) - (-1.0f64).exp()).abs() < 1e-12, "df 2: e^-1");
    assert!((chi2q(4.0, 4) - 3.0 * (-2.0f64).exp()).abs() < 1e-12, "df 4: 3 e^-2");
    assert_eq!(chi2q(0.0, 10), 1.0);
    let far = chi2q(2000.0, 20);
    assert!((0.0..1e-300).contains(&far), "the far tail is zero, not NaN: {far}");
}

#[test]
fn combining_stays_in_bounds_and_is_symmetric() {
    assert_eq!(combine(&[]), 0.5);
    assert!((combine(&[0.935]) - 0.935).abs() < 1e-9, "one token is its own probability");
    assert!(combine(&[0.9; 10]) > 0.99, "ten junk-ward tokens");
    assert!(combine(&[0.1; 10]) < 0.01, "ten not-junk-ward tokens");
    assert!((combine(&[0.9, 0.1]) - 0.5).abs() < 1e-9, "evidence that cancels");
    assert!((combine(&[0.5; 4]) - 0.5).abs() < 1e-12, "no evidence");
    let sets: [&[f64]; 4] = [&[0.9, 0.8, 0.7, 0.3], &[0.99, 0.01, 0.6], &[0.95; 3], &[0.2, 0.3, 0.4, 0.45, 0.55]];
    for set in sets {
        let s = combine(set);
        assert!((0.0..=1.0).contains(&s), "{set:?} gives {s}");
        let mirror: Vec<f64> = set.iter().map(|p| 1.0 - p).collect();
        let m = combine(&mirror);
        assert!((s + m - 1.0).abs() < 1e-9, "{set:?}: {s} and its mirror {m} must add up to one");
    }
}

#[test]
fn untraining_gives_back_the_exact_table() {
    let before = trained();
    let mut m = before.clone();
    let tokens = tokenise(&support::spam_probe());
    m.train(&tokens, true);
    assert_ne!(m, before);
    m.untrain(&tokens, true);
    assert_eq!(m, before);
}
