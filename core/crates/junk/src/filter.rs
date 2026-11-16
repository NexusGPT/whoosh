//! The signals in one decision: the person's hard rules, the learned word model, the header rules, and (only for a suspect message,
//! when a model on this Mac is ready) the on-device model's opinion.
use crate::message::{Context, HardSource, Message};
use crate::model::Model;
use crate::rules::{fired_rules, RuleId};
use crate::tokens::tokenise;
use crate::tuning::{BOOTSTRAP_MAX_PER_CLASS, JUNK_MOVE_THRESHOLD, JUNK_REVIEW_THRESHOLD};

/// What the engine does with the message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    /// Leave it where it is.
    Keep,
    /// Leave it, but mark it "suspected" for the person to review.
    Suspect,
    /// IMAP MOVE to the account's \Junk folder.
    Move,
}

/// The two cut-offs the person tunes in Settings. `move_at` is inclusive: a score equal to it moves the message.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Thresholds {
    pub move_at: f64,
    pub review_at: f64,
}

/// A cut-off pair that is not 0 < review <= move <= 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThresholdError;

impl std::fmt::Display for ThresholdError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "the review cut-off must be above 0 and not above the move cut-off, which must not be above 1")
    }
}

impl std::error::Error for ThresholdError {}

impl Default for Thresholds {
    fn default() -> Thresholds {
        Thresholds { move_at: JUNK_MOVE_THRESHOLD, review_at: JUNK_REVIEW_THRESHOLD }
    }
}

impl Thresholds {
    pub fn new(move_at: f64, review_at: f64) -> Result<Thresholds, ThresholdError> {
        let ok = move_at.is_finite() && review_at.is_finite() && review_at > 0.0 && review_at <= move_at && move_at <= 1.0;
        if ok {
            Ok(Thresholds { move_at, review_at })
        } else {
            Err(ThresholdError)
        }
    }

    /// Whether the score is in the suspect band: at or above the review cut-off and below the move cut-off. Only there can an
    /// on-device opinion change what happens, so only there is it asked (never on every arrival: speed, RAM and CPU).
    pub fn in_suspect_band(&self, score: f64) -> bool {
        score >= self.review_at && score < self.move_at
    }

    pub fn decide(&self, score: f64) -> Decision {
        if score >= self.move_at {
            Decision::Move
        } else if score >= self.review_at {
            Decision::Suspect
        } else {
            Decision::Keep
        }
    }
}

/// What the person did with a message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Label {
    Junk,
    NotJunk,
}

/// An optional second reader of one message: the chance, in [0, 1], that it is junk. It must run on this Mac (nothing leaves it).
/// `None` = no opinion (no model ready, or it failed): the score is left exactly as the other signals made it.
pub trait ModelOpinion {
    fn chance_junk(&self, msg: &Message) -> Option<f64>;
}

/// No on-device model: never an opinion.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoOpinion;

impl ModelOpinion for NoOpinion {
    fn chance_junk(&self, _msg: &Message) -> Option<f64> {
        None
    }
}

impl<F: Fn(&Message) -> Option<f64>> ModelOpinion for F {
    fn chance_junk(&self, msg: &Message) -> Option<f64> {
        self(msg)
    }
}

/// The answer for one message, with the reasons behind it.
#[derive(Debug, Clone, PartialEq)]
pub struct Verdict {
    /// 1.0 for a hard rule, else the combined score in [0, 1].
    pub score: f64,
    pub decision: Decision,
    /// The word model's own score; None when it has not learned enough yet (or a hard rule decided first).
    pub bayes: Option<f64>,
    /// The header rules tripped.
    pub rules: Vec<RuleId>,
    /// The on-device model's chance of junk, when it was asked (suspect band only) and answered; None otherwise.
    pub opinion: Option<f64>,
    /// The block or "always junk" rule that decided, when one did.
    pub hard: Option<HardSource>,
}

/// How much a first-run training took in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BootstrapReport {
    pub junk_learned: usize,
    pub good_learned: usize,
}

/// The combined score of the word model, the header rules and the on-device opinion. Each signal is read as the chance the message
/// is junk and the signals are taken as independent: the message is clean only if every signal lets it through.
/// A model that has not learned enough, or an on-device model that is absent, counts as 0 (no opinion), so day one is never decided by a guess.
pub fn combine_signals(bayes: Option<f64>, rules: &[RuleId], opinion: Option<f64>) -> f64 {
    let mut clean = (1.0 - bayes.unwrap_or(0.0)) * (1.0 - opinion.unwrap_or(0.0).clamp(0.0, 1.0));
    for r in rules {
        clean *= 1.0 - r.weight();
    }
    (1.0 - clean).clamp(0.0, 1.0)
}

/// The filter: the learned model and the cut-offs.
#[derive(Debug, Clone, Default)]
pub struct Filter {
    pub model: Model,
    pub thresholds: Thresholds,
}

impl Filter {
    pub fn new(model: Model, thresholds: Thresholds) -> Filter {
        Filter { model, thresholds }
    }

    /// Scores one message. The person's hard rules come first and are certain; then the model and the header rules.
    pub fn assess(&self, msg: &Message, ctx: &Context) -> Verdict {
        self.assess_with(msg, ctx, &NoOpinion)
    }

    /// `assess`, and when the score of the other signals falls in the suspect band, one question to the on-device model (`opinion`),
    /// folded in as one more independent signal. A message outside the band never asks; a hard rule decides before anything is asked.
    pub fn assess_with(&self, msg: &Message, ctx: &Context, opinion: &dyn ModelOpinion) -> Verdict {
        if let Some(rule) = ctx.hard_rules.iter().find(|r| r.matches(&msg.from_email)) {
            return Verdict { score: 1.0, decision: Decision::Move, bayes: None, rules: Vec::new(), opinion: None, hard: Some(rule.source) };
        }
        let tokens = tokenise(msg);
        let bayes = self.model.score(&tokens);
        let rules = fired_rules(msg, ctx);
        let base = combine_signals(bayes, &rules, None);
        let asked = if self.thresholds.in_suspect_band(base) { opinion.chance_junk(msg).filter(|p| p.is_finite()) } else { None };
        let score = combine_signals(bayes, &rules, asked);
        Verdict { score, decision: self.thresholds.decide(score), bayes, rules, opinion: asked, hard: None }
    }

    /// A late opinion for a message already scored: the stored score folded with the opinion, as one more independent chance.
    /// The score never goes down, and an opinion that is not a number changes nothing.
    pub fn with_late_opinion(&self, stored_score: f64, opinion: Option<f64>) -> (f64, Decision) {
        let score = match opinion.filter(|p| p.is_finite()) {
            Some(p) => (1.0 - (1.0 - stored_score.clamp(0.0, 1.0)) * (1.0 - p.clamp(0.0, 1.0))).clamp(0.0, 1.0),
            None => stored_score,
        };
        (score, self.thresholds.decide(score))
    }

    /// The person marked the message junk or not junk: learn it.
    pub fn learn(&mut self, msg: &Message, label: Label) {
        self.model.train(&tokenise(msg), label == Label::Junk);
    }

    /// Take an earlier lesson back.
    pub fn unlearn(&mut self, msg: &Message, label: Label) {
        self.model.untrain(&tokenise(msg), label == Label::Junk);
    }

    /// The person changed their mind (junk to not junk, or back): take the old lesson out, put the new one in.
    pub fn relearn(&mut self, msg: &Message, from: Label, to: Label) {
        let tokens = tokenise(msg);
        self.model.untrain(&tokens, from == Label::Junk);
        self.model.train(&tokens, to == Label::Junk);
    }

    /// First run on an account: learn the server's Junk folder as junk and the inbox as not junk, at most
    /// `BOOTSTRAP_MAX_PER_CLASS` of each (the engine hands the newest first). Messages with no tokens are skipped.
    pub fn bootstrap(&mut self, junk: &[Message], good: &[Message]) -> BootstrapReport {
        let mut junk_learned = 0usize;
        for m in junk.iter().take(BOOTSTRAP_MAX_PER_CLASS) {
            let tokens = tokenise(m);
            if tokens.is_empty() {
                continue;
            }
            self.model.train(&tokens, true);
            junk_learned += 1;
        }
        let mut good_learned = 0usize;
        for m in good.iter().take(BOOTSTRAP_MAX_PER_CLASS) {
            let tokens = tokenise(m);
            if tokens.is_empty() {
                continue;
            }
            self.model.train(&tokens, false);
            good_learned += 1;
        }
        BootstrapReport { junk_learned, good_learned }
    }
}
