//! The local engine: Laya's option budget (the authors' build_sequence arithmetic over the ledger's on-device numbers),
//! the state cut to the room left, the window never past ON_DEVICE_CONTEXT_TOKENS, a cut option mapped back.
mod support;
use contracts::{DecisionAnswer, DecisionPort, DecisionQuestion};
use decision::adapters::laya_budget::{estimate_tokens, fit, window, LayaShape};
use decision::decide::state_of;
use decision::tuning::{laya_min_head_tokens, laya_option_token_cap, on_device_context_tokens, LAYA_MIN_OPTION_TOKENS};
use serde_json::Value;
use support::*;

fn long_options(n: usize, chars: usize) -> Vec<String> {
    (0..n).map(|i| format!("option {i} {}", "x".repeat(chars))).collect()
}

#[test]
fn every_option_is_cut_to_the_cap_and_the_sequence_fits_the_window() {
    let f = fit("choice", "Which label fits this email?", &long_options(3, 400), TEST_SHAPE);
    for o in &f.options {
        assert!(estimate_tokens(&format!(" {o}")) <= laya_option_token_cap(), "{} tokens", estimate_tokens(o));
    }
    assert!(f.used_tokens + f.state_room < window(TEST_SHAPE));
    assert!(f.used_tokens <= TEST_SHAPE.head_max_len + 3, "head and options stay inside the head budget plus CLS and two SEP");
}

#[test]
fn too_many_options_share_the_head_and_the_instructions_keep_their_minimum() {
    let f = fit("choice", &"why ".repeat(200), &long_options(40, 400), TEST_SHAPE);
    for o in &f.options {
        assert!(estimate_tokens(o) + 1 <= LAYA_MIN_OPTION_TOKENS.max((TEST_SHAPE.head_max_len - decision::tuning::laya_option_budget_floor()) / 40));
    }
    assert!(estimate_tokens(&format!("choice question: {}", f.instructions)) >= laya_min_head_tokens() - 1, "the head keeps its floor");
}

#[test]
fn short_options_and_instructions_go_through_untouched() {
    let opts = vec!["Primary".to_string(), "Updates".to_string()];
    let f = fit("choice", "Which inbox tab should this email sit in?", &opts, TEST_SHAPE);
    assert_eq!(f.options, opts);
    assert_eq!(f.instructions, "Which inbox tab should this email sit in?");
}

#[test]
fn the_window_never_passes_the_on_device_context() {
    assert_eq!(window(LayaShape { max_len: 1_000_000, head_max_len: 128 }), on_device_context_tokens());
    assert_eq!(window(TEST_SHAPE), TEST_SHAPE.max_len);
}

#[test]
fn the_state_is_cut_to_the_room_and_a_cut_option_maps_back_to_the_full_one() {
    let long = format!("client > Provanta > {}", "Orbit ".repeat(80));
    let fake = FakeLocal::new(|_, q, _| {
        let first = q["criteria"].as_object().unwrap().keys().find(|k| k.starts_with("client")).unwrap().clone();
        choice(&first, 0.9)
    });
    let engine = local(fake.clone());
    let m = msg("m", "a@b-example.com", "kickoff", &"lorem ipsum ".repeat(2000));
    let q = DecisionQuestion::Choice { id: "labels/model".into(), prompt: "Which label?".into(), options: vec![long.clone(), "vendors".into()] };
    let r = engine.decide(state_of(&m), vec![q]).unwrap();
    let DecisionAnswer::Choice { choice, probabilities, .. } = &r.answers[0] else { panic!() };
    assert_eq!(choice, &long, "answered with the cut option, returned as the full one");
    assert_eq!(probabilities.get(&long), Some(&0.9));
    let (prompt, max_tokens) = fake.prompts.lock().unwrap()[0].clone();
    let sent: Value = serde_json::from_str(&prompt).unwrap();
    assert!(estimate_tokens(sent["state"].as_str().unwrap()) <= window(TEST_SHAPE), "the state fits the window");
    assert!(sent["state"].as_str().unwrap().len() < m.body.len());
    assert!(estimate_tokens(&prompt) <= window(TEST_SHAPE), "everything handed over fits the window");
    assert_eq!(max_tokens as usize, window(TEST_SHAPE) - estimate_tokens(&prompt));
}

#[test]
fn an_option_longer_than_the_cap_is_cut_even_when_the_head_has_room() {
    // a head budget wide enough that the overflow branch never runs: only the per-option cap cuts
    let wide = LayaShape { max_len: 2048, head_max_len: 400 };
    let f = fit("choice", "Which label?", &long_options(1, 200), wide);
    assert_eq!(estimate_tokens(&f.options[0]), laya_option_token_cap(), "the option keeps exactly the cap");
}
