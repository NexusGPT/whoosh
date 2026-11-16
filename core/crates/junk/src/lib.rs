//! Whoosh junk crate: the local spam filter behind the Spam folder. Pure functions over one message and one trained
//! table: no network, no store, no clock, no dependency beyond std. The Spam folder itself is the server's Junk
//! folder (IMAP \Junk); this crate decides which incoming mail belongs there and learns from the person's actions.
//!
//! Three signals and an optional fourth, one score in [0, 1] (`Filter::assess`, `Filter::assess_with`):
//! 1. The person's blocks and "always junk" rules: certain (score 1.0, always a move).
//! 2. A word-probability model of the SpamBayes family: tokeniser, Robinson's smoothed per-token probability,
//!    chi-square combining of the strongest tokens (`tokens`, `model`, `chi2`). It learns from junk / not-junk
//!    actions (`Filter::learn`, `relearn`) and starts from the account's Junk folder and inbox (`Filter::bootstrap`).
//! 3. Cheap header rules in the spirit of SpamAssassin: SPF/DKIM/DMARC verdicts already written into the headers,
//!    Reply-To mismatch, display-name impersonation, link-text mismatch (`rules`), each a fixed score.
//! 4. The on-device model's opinion (`ModelOpinion`), asked only for a message whose score from the others is in the suspect band
//!    (`Thresholds::in_suspect_band`), absent when no model on this Mac is ready (absent changes nothing). It runs on this Mac only.
//!
//! Method sources (read for the method; no code copied, SpamAssassin and bogofilter are ideas only):
//! - SpamBayes project and its classifier notes: <https://spambayes.sourceforge.io/>
//! - Gary Robinson, "A Statistical Approach to the Spam Problem", Linux Journal 2003: <https://www.linuxjournal.com/article/6467>
//! - Paul Graham, "A Plan for Spam": <https://www.paulgraham.com/spam.html>
#![forbid(unsafe_code)]

mod chi2;
mod domain;
mod filter;
mod links;
mod message;
mod model;
mod rules;
mod tokens;
pub mod tuning;

pub use chi2::{chi2q, combine};
pub use domain::{domain_of, host_of_url, registrable_domain, visible_host};
pub use filter::{combine_signals, BootstrapReport, Decision, Filter, Label, ModelOpinion, NoOpinion, ThresholdError, Thresholds, Verdict};
pub use links::{extract_links, Link};
pub use message::{parse_address, unfold_headers, Contact, Context, HardRule, HardSource, Matcher, Message};
pub use model::{Counts, Model, ModelError};
pub use rules::{auth_verdicts, display_name_impersonation, fired_rules, link_mismatch, reply_to_mismatch, AuthVerdicts, RuleId};
pub use tokens::{tokenise, words};
