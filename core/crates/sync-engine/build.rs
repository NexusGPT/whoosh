//! Reads the numbers this crate needs from docs/CONSTANTS.md, the only place they live.
//! A missing row or a value that does not start with an integer fails the build: no fallback exists.
//! Rows may join several names with " · " (for example `GRAPH_CONCURRENT · GRAPH_REQ_PER_10MIN | 4 · 10,000`);
//! a short name such as `IPHONE` takes the prefix of the first name (`BACKFILL_CONCURRENT_IPHONE`).
use std::{env, fs, path::PathBuf};

fn lookup(table: &str, name: &str) -> Option<u64> {
    for line in table.lines() {
        let cells: Vec<&str> = line.split('|').map(str::trim).collect();
        if cells.len() < 3 {
            continue;
        }
        let names: Vec<&str> = cells[1].split('·').map(str::trim).collect();
        let values: Vec<&str> = cells[2].split('·').map(str::trim).collect();
        let prefix = names[0].rsplit_once('_').map(|(p, _)| format!("{p}_")).unwrap_or_default();
        for (i, n) in names.iter().enumerate() {
            let full = if n.contains('_') { n.to_string() } else { format!("{prefix}{n}") };
            if full == name {
                let digits: String = values.get(i)?.chars().take_while(|c| c.is_ascii_digit() || *c == ',').filter(|c| *c != ',').collect();
                return digits.parse().ok();
            }
        }
    }
    None
}

fn main() {
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("cargo sets it"));
    let path = std::env::var_os("WHOOSH_CONSTANTS_MD").map(std::path::PathBuf::from).unwrap_or_else(|| manifest.join("../../../docs/CONSTANTS.md"));
    // WHOOSH_CONSTANTS_MD: an absolute path to the ledger for a build outside this tree (cargo mutants copies core/ alone; round cell 03 died on the relative path, main52 to main55)
    println!("cargo:rerun-if-env-changed=WHOOSH_CONSTANTS_MD");
    println!("cargo:rerun-if-changed={}", path.display());
    let table = fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
    let mut out = String::new();
    for name in ["BACKFILL_CONCURRENT_MAC", "BACKFILL_CONCURRENT_IPHONE", "IMAP_CONN_PER_ACCOUNT", "GRAPH_CONCURRENT"] {
        let v = lookup(&table, name).unwrap_or_else(|| panic!("docs/CONSTANTS.md has no integer row named {name}"));
        out.push_str(&format!("pub const {name}: usize = {v};\n"));
    }
    let dest = PathBuf::from(env::var("OUT_DIR").expect("cargo sets it")).join("constants.rs");
    fs::write(dest, out).expect("write constants.rs");
}
