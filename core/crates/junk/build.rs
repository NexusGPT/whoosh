//! Reads the numbers this crate needs from the JUNK_* rows of docs/CONSTANTS.md, the only place they live.
//! A row missing from the ledger fails the build. The crate keeps its own const names: the ledger's JUNK_ prefix
//! stays on JUNK_MOVE_THRESHOLD and JUNK_REVIEW_THRESHOLD and drops from every other name.
use std::{env, fs, path::PathBuf};

fn row_value(table: &str, name: &str) -> Option<String> {
    table.lines().find_map(|line| {
        let cells: Vec<&str> = line.split('|').map(str::trim).collect();
        (cells.len() > 2 && cells[1] == name).then(|| cells[2].replace(',', ""))
    })
}

fn main() {
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("cargo sets it"));
    let ledger_path = std::env::var_os("WHOOSH_CONSTANTS_MD").map(std::path::PathBuf::from).unwrap_or_else(|| manifest.join("../../../docs/CONSTANTS.md"));
    // WHOOSH_CONSTANTS_MD: an absolute path to the ledger for a build outside this tree (cargo mutants copies core/ alone; round cell 03 died on the relative path, main52 to main55)
    println!("cargo:rerun-if-env-changed=WHOOSH_CONSTANTS_MD");
    println!("cargo:rerun-if-changed={}", ledger_path.display());
    let ledger = fs::read_to_string(&ledger_path).unwrap_or_else(|e| panic!("cannot read {}: {e}", ledger_path.display()));
    // (const name in this crate, ledger row, type, doc line)
    let rows = [
        ("JUNK_MOVE_THRESHOLD", "JUNK_MOVE_THRESHOLD", "f64", "A message moves to the server's Junk folder at or above this score."),
        ("JUNK_REVIEW_THRESHOLD", "JUNK_REVIEW_THRESHOLD", "f64", "A message is marked \"suspected\" (stays where it is) at or above this score and below the move threshold."),
        ("UNKNOWN_WORD_PROB", "JUNK_UNKNOWN_WORD_PROB", "f64", "Probability given to a token the model has never seen (Robinson's x)."),
        ("UNKNOWN_WORD_STRENGTH", "JUNK_UNKNOWN_WORD_STRENGTH", "f64", "How many observations the prior is worth (Robinson's s)."),
        ("MIN_PROB_STRENGTH", "JUNK_MIN_PROB_STRENGTH", "f64", "A token whose probability is closer than this to 0.5 says nothing and is left out of the combining."),
        ("MAX_DISCRIMINATORS", "JUNK_MAX_DISCRIMINATORS", "usize", "At most this many of the strongest tokens are combined."),
        ("PROB_EDGE", "JUNK_PROB_EDGE", "f64", "Probabilities are held away from 0 and 1 by this much before the logarithm is taken."),
        ("MODEL_MIN_PER_CLASS", "JUNK_MODEL_MIN_PER_CLASS", "u32", "The model gives no opinion until it has learned from at least this many junk AND this many not-junk messages."),
        ("TOKEN_TABLE_MAX", "JUNK_TOKEN_TABLE_MAX", "usize", "The token table is pruned when it holds more than this many tokens (rare tokens go first)."),
        ("TOKEN_TABLE_PRUNE_TO", "JUNK_TOKEN_TABLE_PRUNE_TO", "usize", "A prune brings the table down to this many tokens, so it does not run again at once."),
        ("TOKEN_BYTES_MAX", "JUNK_TOKEN_BYTES_MAX", "usize", "Longest token the serialised table accepts, in bytes."),
        ("MAX_TOKENS_PER_MESSAGE", "JUNK_MAX_TOKENS_PER_MESSAGE", "usize", "Distinct tokens taken from one message (subject, sender and links first, then body words)."),
        ("BODY_CHARS_MAX", "JUNK_BODY_CHARS_MAX", "usize", "Characters of body text read per message."),
        ("HTML_CHARS_MAX", "JUNK_HTML_CHARS_MAX", "usize", "Characters of HTML read per message for the links."),
        ("SUBJECT_CHARS_MAX", "JUNK_SUBJECT_CHARS_MAX", "usize", "Characters of the subject read for tokens."),
        ("FROMNAME_CHARS_MAX", "JUNK_FROMNAME_CHARS_MAX", "usize", "Characters of the sender's display name read for tokens."),
        ("WORD_MIN_CHARS", "JUNK_WORD_MIN_CHARS", "usize", "Shortest word, in characters, that is a token (shorter words are dropped)."),
        ("WORD_MAX_CHARS", "JUNK_WORD_MAX_CHARS", "usize", "Longest word, in characters, kept whole as a token (longer words are bucketed)."),
        ("LINKS_SCANNED_MAX", "JUNK_LINKS_SCANNED_MAX", "usize", "Links examined per message."),
        ("BOOTSTRAP_MAX_PER_CLASS", "JUNK_BOOTSTRAP_MAX_PER_CLASS", "usize", "Messages learned per class at first run (the newest first, as the engine hands them over)."),
        ("RULE_SPF_FAIL", "JUNK_RULE_SPF_FAIL", "f64", "Header rule: the chance, taken alone, that a message failing SPF is junk."),
        ("RULE_SPF_SOFTFAIL", "JUNK_RULE_SPF_SOFTFAIL", "f64", "Header rule: SPF softfail."),
        ("RULE_DKIM_FAIL", "JUNK_RULE_DKIM_FAIL", "f64", "Header rule: no signature of the message passes DKIM."),
        ("RULE_DMARC_FAIL", "JUNK_RULE_DMARC_FAIL", "f64", "Header rule: DMARC fails."),
        ("RULE_REPLY_TO_MISMATCH", "JUNK_RULE_REPLY_TO_MISMATCH", "f64", "Header rule: Reply-To points to another site than From (mailing lists trip it, hence the low score)."),
        ("RULE_DISPLAY_NAME_IMPERSONATION", "JUNK_RULE_DISPLAY_NAME_IMPERSONATION", "f64", "Header rule: the display name claims to be someone else."),
        ("RULE_LINK_MISMATCH", "JUNK_RULE_LINK_MISMATCH", "f64", "Header rule: a link shows one site to the reader and goes to another."),
    ];
    let mut out = String::new();
    for (name, row, ty, doc) in rows {
        let v = row_value(&ledger, row).unwrap_or_else(|| panic!("no row named {row} in docs/CONSTANTS.md"));
        let literal = if ty == "f64" {
            let f: f64 = v.parse().unwrap_or_else(|_| panic!("{row} is {v:?}, not a number"));
            format!("{f:?}")
        } else {
            let n: u64 = v.parse().unwrap_or_else(|_| panic!("{row} is {v:?}, not an integer"));
            n.to_string()
        };
        out.push_str(&format!("/// {doc}\npub const {name}: {ty} = {literal};\n"));
    }
    let dest = PathBuf::from(env::var("OUT_DIR").expect("cargo sets it")).join("tuning.rs");
    fs::write(dest, out).expect("write tuning.rs");
}
