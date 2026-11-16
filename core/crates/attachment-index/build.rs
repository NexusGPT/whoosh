//! Reads the numbers this crate depends on from docs/CONSTANTS.md so each number lives once.
use std::{env, fs, path::PathBuf};

fn leading_int(s: &str) -> Option<usize> {
    let digits: String = s.trim().chars().take_while(|c| c.is_ascii_digit() || *c == ',').filter(|c| *c != ',').collect();
    digits.parse().ok()
}

fn main() {
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("manifest dir"));
    let path = std::env::var_os("WHOOSH_CONSTANTS_MD").map(std::path::PathBuf::from).unwrap_or_else(|| manifest.join("../../../docs/CONSTANTS.md"));
    // WHOOSH_CONSTANTS_MD: an absolute path to the ledger for a build outside this tree (cargo mutants copies core/ alone; round cell 03 died on the relative path, main52 to main55)
    println!("cargo:rerun-if-env-changed=WHOOSH_CONSTANTS_MD");
    println!("cargo:rerun-if-changed={}", path.display());
    let text = fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
    let mut found: Vec<(String, usize)> = Vec::new();
    for line in text.lines().filter(|l| l.starts_with('|')) {
        let cells: Vec<&str> = line.split('|').map(str::trim).collect();
        if cells.len() < 3 {
            continue;
        }
        let names: Vec<&str> = cells[1].split(['·', '/']).map(str::trim).collect();
        let values: Vec<&str> = cells[2].split(['·', '/']).collect();
        for (name, value) in names.iter().zip(values.iter()) {
            if let Some(n) = leading_int(value) {
                found.push((name.to_string(), n));
            }
        }
    }
    let get = |name: &str| found.iter().find(|(n, _)| n == name).map(|(_, v)| *v);
    let need = |name: &str| get(name).unwrap_or_else(|| panic!("docs/CONSTANTS.md has no numeric row for {name}"));
    let mut out = String::new();
    for name in ["EMBED_DIM", "CHUNK_TOKENS", "CHUNK_OVERLAP", "VIDEO_FRAME_EVERY_S", "VIDEO_FRAMES_MAX", "ATTACHMENT_INDEX_BATTERY_MIN", "INDEX_LAG_MS"] {
        out.push_str(&format!("pub const {name}: usize = {};\n", need(name)));
    }
    // Rows asked of the lead (SendMessage 2026-11-08): absent = no cap until the row lands.
    for name in ["ATTACHMENT_TEXT_MAX_CHARS", "TRANSCRIPT_MAX_CHARS"] {
        match get(name) {
            Some(v) => out.push_str(&format!("pub const {name}: Option<usize> = Some({v});\n")),
            None => out.push_str(&format!("pub const {name}: Option<usize> = None;\n")),
        }
    }
    fs::write(PathBuf::from(env::var("OUT_DIR").expect("out dir")).join("constants.rs"), out).expect("write constants");
}
