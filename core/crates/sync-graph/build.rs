//! Reads the Graph limits out of docs/CONSTANTS.md so the numbers live once (rule: every number
//! lives once, in docs/CONSTANTS.md). A missing row fails the build with the row's name.
use std::{env, fs, path::PathBuf};

fn cells(line: &str) -> Vec<String> {
    line.trim().trim_matches('|').split('|').map(|c| c.trim().to_string()).collect()
}

/// Every run of digits in `s`; commas inside a run are thousands separators ("10,000").
fn numbers(s: &str) -> Vec<u64> {
    let mut out = Vec::new();
    let mut cur = String::new();
    for ch in s.chars() {
        if ch.is_ascii_digit() {
            cur.push(ch);
        } else if ch == ',' && !cur.is_empty() {
            continue;
        } else if !cur.is_empty() {
            out.push(cur.parse().expect("digits parse"));
            cur.clear();
        }
    }
    if !cur.is_empty() {
        out.push(cur.parse().expect("digits parse"));
    }
    out
}

fn row(text: &str, first_name: &str) -> (Vec<String>, String) {
    let line = text
        .lines()
        .find(|l| l.trim_start().starts_with(&format!("| {first_name}")))
        .unwrap_or_else(|| panic!("docs/CONSTANTS.md has no row starting with {first_name}: request it under '## Constants to add' in the lane notes"));
    let c = cells(line);
    (c.clone(), c[1].clone())
}

fn main() {
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("manifest dir"));
    let path = std::env::var_os("WHOOSH_CONSTANTS_MD").map(std::path::PathBuf::from).unwrap_or_else(|| manifest.join("../../../docs/CONSTANTS.md"));
    // WHOOSH_CONSTANTS_MD: an absolute path to the ledger for a build outside this tree (cargo mutants copies core/ alone; round cell 03 died on the relative path, main52 to main55)
    println!("cargo:rerun-if-env-changed=WHOOSH_CONSTANTS_MD");
    println!("cargo:rerun-if-changed={}", path.display());
    let text = fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));

    let (names, values) = row(&text, "GRAPH_CONCURRENT");
    let nums = numbers(&values);
    assert!(nums.len() == 2, "GRAPH_CONCURRENT row should hold two numbers, found {values:?}");
    let window_min = *numbers(&names[0]).first().expect("GRAPH_REQ_PER_10MIN names its window in minutes");

    let (_, sub) = row(&text, "GRAPH_SUB_RENEW_MIN");
    let sub_nums = numbers(&sub);
    assert!(sub_nums.len() == 2, "GRAPH_SUB_RENEW_MIN row should hold the plain and the rich ceiling, found {sub:?}");

    let out = format!(
        "pub const GRAPH_CONCURRENT: usize = {};\n\
         pub const GRAPH_REQ_PER_10MIN: usize = {};\n\
         pub const GRAPH_WINDOW_MS: i64 = {};\n\
         pub const GRAPH_SUB_CEILING_MIN: i64 = {};\n\
         pub const GRAPH_SUB_CEILING_RICH_MIN: i64 = {};\n",
        nums[0], nums[1], window_min * 60_000, sub_nums[0], sub_nums[1]
    );
    fs::write(PathBuf::from(env::var("OUT_DIR").expect("out dir")).join("limits.rs"), out).expect("write limits.rs");
}
