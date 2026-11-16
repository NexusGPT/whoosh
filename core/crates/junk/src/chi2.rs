//! Chi-square combining of per-token probabilities (Gary Robinson's method, as SpamBayes runs it).
//!
//! If a message were random words, -2 * sum(ln p) would follow a chi-square law with 2n degrees of freedom. The
//! spam side `S` asks how unlikely the tokens' "ham-ness" is, the ham side `H` how unlikely their "spam-ness" is;
//! the score `(S - H + 1) / 2` is near 1 when only the spam side is surprised, near 0 when only the ham side is,
//! and near 0.5 when both or neither are.
use crate::tuning::PROB_EDGE;

/// The upper tail of the chi-square law for an even number of degrees of freedom `v`:
/// P(X >= x2) = e^-m * sum_{i < v/2} m^i / i!, with m = x2 / 2. Exact (a finite sum), no special function needed.
pub fn chi2q(x2: f64, v: usize) -> f64 {
    if x2 <= 0.0 {
        return 1.0;
    }
    let m = x2 / 2.0;
    let mut term = (-m).exp();
    let mut sum = term;
    for i in 1..(v / 2) {
        term *= m / (i as f64);
        sum += term;
    }
    if sum > 1.0 {
        1.0
    } else {
        sum
    }
}

/// One score in [0, 1] from the probabilities of the strongest tokens (each in (0, 1), spam-ward above 0.5).
/// No tokens: 0.5. A single token: its own probability.
pub fn combine(probs: &[f64]) -> f64 {
    let n = probs.len();
    if n == 0 {
        return 0.5;
    }
    let mut ln_f = 0.0f64;
    let mut ln_1mf = 0.0f64;
    for &p in probs {
        let f = p.clamp(PROB_EDGE, 1.0 - PROB_EDGE);
        ln_f += f.ln();
        ln_1mf += (1.0 - f).ln();
    }
    let s = 1.0 - chi2q(-2.0 * ln_1mf, 2 * n);
    let h = 1.0 - chi2q(-2.0 * ln_f, 2 * n);
    ((s - h) + 1.0) / 2.0
}
