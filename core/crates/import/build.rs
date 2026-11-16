//! Reads the numbers this crate needs from docs/CONSTANTS.md, the only place they live.
//! A row missing from the ledger fails the build.
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
    let rows = [("IMPORT_BATCH_MESSAGES", "usize"), ("IMPORT_BATCH_BYTES", "usize"), ("IMPORT_MAX_MESSAGE_BYTES", "u64")];
    let mut out = String::new();
    for (name, ty) in rows {
        let v = row_value(&ledger, name).unwrap_or_else(|| panic!("no row named {name} in docs/CONSTANTS.md"));
        let n: u64 = v.parse().unwrap_or_else(|_| panic!("{name} is {v:?}, not an integer"));
        out.push_str(&format!("pub const {name}: {ty} = {n};\n"));
    }
    let dest = PathBuf::from(env::var("OUT_DIR").expect("cargo sets it")).join("constants.rs");
    fs::write(dest, out).expect("write constants.rs");
}
