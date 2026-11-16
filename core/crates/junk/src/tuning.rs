//! Every number of the junk crate. build.rs reads them from the JUNK_* rows of docs/CONSTANTS.md, the only place
//! they live; nothing here is typed by hand. All are Cue's defaults and unmeasured (no real mailbox was available).
//!
//! The SpamBayes numbers are the classifier options `unknown_word_prob`, `unknown_word_strength`,
//! `minimum_prob_strength` and `max_discriminators` (defaults 0.5, 0.45, 0.1, 150). The header rules are fixed
//! scores in the SpamAssassin spirit.
include!(concat!(env!("OUT_DIR"), "/tuning.rs"));
