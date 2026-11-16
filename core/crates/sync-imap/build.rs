//! Reads the two numbers this crate needs from docs/CONSTANTS.md, the only place they live.
//! The build fails when a row is missing or is not a plain integer: no fallback value exists.
use std::{env, fs, path::PathBuf};

fn row_value(table: &str, name: &str) -> String {
    for line in table.lines() {
        let cells: Vec<&str> = line.split('|').map(str::trim).collect();
        if cells.len() > 2 && cells[1] == name {
            return cells[2].to_string();
        }
    }
    panic!("docs/CONSTANTS.md has no row named {name}");
}

fn main() {
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("cargo sets it"));
    let path = std::env::var_os("WHOOSH_CONSTANTS_MD").map(std::path::PathBuf::from).unwrap_or_else(|| manifest.join("../../../docs/CONSTANTS.md"));
    // WHOOSH_CONSTANTS_MD: an absolute path to the ledger for a build outside this tree (cargo mutants copies core/ alone; round cell 03 died on the relative path, main52 to main55)
    println!("cargo:rerun-if-env-changed=WHOOSH_CONSTANTS_MD");
    println!("cargo:rerun-if-changed={}", path.display());
    let table = fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
    let mut out = String::new();
    for (name, ty) in [("IMAP_IDLE_REISSUE_MIN", "u64"), ("IMAP_CONN_PER_ACCOUNT", "usize"), ("IMAP_COMMAND_TIMEOUT_MS", "u64")] {
        let v = row_value(&table, name);
        let n: u64 = v.parse().unwrap_or_else(|_| panic!("{name} is {v:?} in docs/CONSTANTS.md, not an integer"));
        out.push_str(&format!("pub const {name}: {ty} = {n};\n"));
    }
    let dest = PathBuf::from(env::var("OUT_DIR").expect("cargo sets it")).join("constants.rs");
    fs::write(dest, out).expect("write constants.rs");
}
