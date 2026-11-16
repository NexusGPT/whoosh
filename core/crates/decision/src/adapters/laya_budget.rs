//! Laya's option budget, ported from the authors' rl_common.py `build_sequence` (Apache-2.0, convaiinnovations/laya) by
//! way of the ondeviceai lane's Swift port (Sources/Features/OnDeviceAI/Core/LayaSequence.swift):
//! `[CLS] <type> question: instructions [SEP] [MASK] opt0 [MASK] opt1 … [SEP] state [SEP]`.
//! Rust has no tokenizer for the checkpoint, so token counts are estimated with ON_DEVICE_CHARS_PER_TOKEN_FLOOR (a
//! pessimistic chars-per-token ratio: the estimate counts more tokens than the model will, so a cut never overflows).
use crate::tuning::{laya_min_head_tokens, laya_option_budget_floor, laya_option_token_cap, on_device_chars_per_token_floor, on_device_context_tokens, LAYA_MIN_OPTION_TOKENS};

/// Read from the checkpoint's config by the runtime, never typed here (docs/lanes/ondeviceai.md).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LayaShape {
    pub max_len: usize,
    pub head_max_len: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Fitted {
    pub instructions: String,
    pub options: Vec<String>,
    /// Tokens of the sequence before the state: CLS, head, SEP, every [MASK]+option, SEP.
    pub used_tokens: usize,
    /// Tokens left for the state (one SEP kept for the end).
    pub state_room: usize,
}

pub fn estimate_tokens(text: &str) -> usize {
    (text.chars().count() as f64 / on_device_chars_per_token_floor()).ceil() as usize
}

/// The longest prefix whose estimate fits in `tokens`.
pub fn cut_to_tokens(text: &str, tokens: usize) -> String {
    let chars = (tokens as f64 * on_device_chars_per_token_floor()).floor() as usize;
    text.chars().take(chars).collect()
}

/// The window a sequence may use: the checkpoint's length, never past the on-device context.
pub fn window(shape: LayaShape) -> usize {
    shape.max_len.min(on_device_context_tokens())
}

pub fn fit(kind_word: &str, instructions: &str, options: &[String], shape: LayaShape) -> Fitted {
    let cap = laya_option_token_cap();
    let floor = laya_option_budget_floor() as i64;
    let head_max = shape.head_max_len as i64;
    // each option: [MASK] + its words, cut to the cap
    let mut opt: Vec<usize> = options.iter().map(|o| 1 + estimate_tokens(&format!(" {o}")).min(cap)).collect();
    let mut budget = head_max - opt.iter().sum::<usize>() as i64;
    if budget < floor {
        let per = LAYA_MIN_OPTION_TOKENS.max(((head_max - floor) / opt.len().max(1) as i64).max(0) as usize);
        opt = opt.into_iter().map(|t| t.min(per)).collect();
        budget = head_max - opt.iter().sum::<usize>() as i64;
    }
    let lead = format!("{kind_word} question: ");
    let head_tokens = estimate_tokens(&format!("{lead}{instructions}")).min((laya_min_head_tokens() as i64).max(budget) as usize);
    let ins_room = head_tokens.saturating_sub(estimate_tokens(&lead));
    let options: Vec<String> = options.iter().zip(&opt).map(|(o, t)| cut_to_tokens(o, t.saturating_sub(1))).collect();
    let used = 1 + head_tokens + 1 + opt.iter().sum::<usize>() + 1;
    Fitted { instructions: cut_to_tokens(instructions, ins_room), options, used_tokens: used, state_room: window(shape).saturating_sub(used + 1) }
}
