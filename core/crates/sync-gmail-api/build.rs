//! Reads the numbers this crate depends on from docs/CONSTANTS.md, the only place a Whoosh number lives.
//! GMAIL_UNITS_PER_MIN and GMAIL_WATCH_RENEW_H are required: the build fails when either row is missing.
//! The per-call costs and the retry numbers are used from the ledger when it has the row. Until the root adds
//! the rows (docs/lanes/core-sync-gmail-api.md "## Constants to add"), the fallbacks below are their only home.
use std::{env, fs, path::PathBuf};

/// (name, fallback). None = required in the ledger.
const NAMES: &[(&str, Option<u64>)] = &[
    ("GMAIL_UNITS_PER_MIN", None),
    ("GMAIL_WATCH_RENEW_H", None),
    // research/mail-sync.md:9
    ("GMAIL_COST_HISTORY_LIST", Some(2)),
    ("GMAIL_COST_MESSAGES_LIST", Some(5)),
    ("GMAIL_COST_MESSAGES_GET_RAW", Some(60)),
    ("GMAIL_COST_WATCH", Some(100)),
    // not in the research file: Gmail quota reference values, unverified in this session
    ("GMAIL_COST_LABELS_LIST", Some(1)),
    ("GMAIL_COST_PROFILE", Some(1)),
    // Gmail API page-size maximum for history.list and messages.list, unverified in this session
    ("GMAIL_PAGE_SIZE", Some(500)),
    ("GMAIL_RETRY_MAX", Some(5)),
    ("GMAIL_BACKOFF_BASE_MS", Some(1000)),
];

fn cell_number(text: &str, name: &str) -> Option<u64> {
    for line in text.lines() {
        let cells: Vec<&str> = line.split('|').map(str::trim).collect();
        if cells.len() >= 3 && cells[1] == name {
            let digits: String = cells[2]
                .chars()
                .take_while(|c| c.is_ascii_digit() || *c == ',')
                .filter(|c| c.is_ascii_digit())
                .collect();
            return digits.parse().ok();
        }
    }
    None
}

fn main() {
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let ledger = std::env::var_os("WHOOSH_CONSTANTS_MD").map(std::path::PathBuf::from).unwrap_or_else(|| manifest.join("../../../docs/CONSTANTS.md"));
    // WHOOSH_CONSTANTS_MD: an absolute path to the ledger for a build outside this tree (cargo mutants copies core/ alone; round cell 03 died on the relative path, main52 to main55)
    println!("cargo:rerun-if-env-changed=WHOOSH_CONSTANTS_MD");
    println!("cargo:rerun-if-changed={}", ledger.display());
    println!("cargo:rerun-if-changed=build.rs");
    let text = fs::read_to_string(&ledger)
        .unwrap_or_else(|e| panic!("cannot read the constants ledger {}: {e}", ledger.display()));
    let mut out = String::new();
    for (name, fallback) in NAMES {
        let value = match (cell_number(&text, name), fallback) {
            (Some(v), _) => v,
            (None, Some(f)) => *f,
            (None, None) => panic!("docs/CONSTANTS.md has no numeric row for {name}"),
        };
        out.push_str(&format!("pub const {name}: u64 = {value};\n"));
    }
    let dest = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR")).join("constants.rs");
    fs::write(&dest, out).expect("write constants.rs");
}
