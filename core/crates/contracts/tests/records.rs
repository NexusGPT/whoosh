//! fn-contract-types acceptance: one exported type per docs/DATA-MAP.md record, kinds agree,
//! the count definitions in docs/DIGEST.md match the contract word for word.
use contracts::{RecordKind, RECORDS, UNREAD_DEFINITION, WAITING_DEFINITION};
use std::collections::BTreeMap;

fn repo(rel: &str) -> String {
    let p = format!("{}/../../../{}", env!("CARGO_MANIFEST_DIR"), rel);
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("cannot read {p}: {e}"))
}

/// DATA-MAP rows: `| name | holds | kind | ...`, header and rule rows skipped.
fn data_map() -> BTreeMap<String, String> {
    repo("docs/DATA-MAP.md")
        .lines()
        .filter(|l| l.starts_with("| ") && !l.starts_with("| record ") && !l.starts_with("|---"))
        .map(|l| {
            let c: Vec<&str> = l.split('|').map(str::trim).collect();
            (c[1].to_string(), c[3].to_string())
        })
        .collect()
}

#[test]
fn one_type_per_data_map_record() {
    let map = data_map();
    let ours: BTreeMap<String, &str> = RECORDS.iter().map(|(n, t, _)| (n.to_string(), *t)).collect();
    let missing: Vec<&String> = map.keys().filter(|k| !ours.contains_key(*k)).collect();
    let extra: Vec<&String> = ours.keys().filter(|k| !map.contains_key(*k)).collect();
    assert!(missing.is_empty(), "DATA-MAP records with no contract type: {missing:?}");
    assert!(extra.is_empty(), "contract types with no DATA-MAP row: {extra:?}");
    assert_eq!(RECORDS.len(), map.len(), "a record registered twice");
    let types: std::collections::BTreeSet<&str> = RECORDS.iter().map(|(_, t, _)| *t).collect();
    assert_eq!(types.len(), RECORDS.len(), "two records share one type");
    println!("records: {} (DATA-MAP rows {})", RECORDS.len(), map.len());
}

#[test]
fn record_kinds_match_data_map() {
    let map = data_map();
    for (name, _, kind) in RECORDS {
        let want = match kind {
            RecordKind::State => "STATE",
            RecordKind::File => "FILE",
            RecordKind::Index => "INDEX",
            RecordKind::Credential => "CREDENTIAL",
        };
        // a row may carry two kinds ("STATE + CREDENTIAL"): the type names its primary one
        assert!(map[*name].split('+').map(str::trim).any(|k| k == want), "kind of {name}: DATA-MAP says {:?}, the type says {want}", map[*name]);
    }
}

#[test]
fn digest_count_definitions_match_contract_word_for_word() {
    let digest = repo("docs/DIGEST.md");
    let src = repo("core/crates/contracts/src/counts.rs");
    for def in [UNREAD_DEFINITION, WAITING_DEFINITION] {
        assert!(digest.contains(def), "docs/DIGEST.md lacks the definition {def:?}");
        assert!(src.contains(&format!("/// {def}")), "the doc comment differs from {def:?}");
    }
}
