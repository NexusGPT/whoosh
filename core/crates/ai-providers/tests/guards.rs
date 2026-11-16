//! Greps over the crate's own source: what the brief asks to hold by construction, held by a test.
use std::fs;
use std::path::{Path, PathBuf};

use ai_providers::table::{info, Auth, ProviderId, PROVIDERS, SPEECH};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn sources(dir: &Path, out: &mut Vec<PathBuf>) {
    for e in fs::read_dir(dir).unwrap() {
        let p = e.unwrap().path();
        if p.is_dir() { sources(&p, out) } else if p.extension().is_some_and(|x| x == "rs") { out.push(p) }
    }
}

fn all_src() -> Vec<(PathBuf, String)> {
    let mut files = Vec::new();
    sources(&root().join("src"), &mut files);
    files.into_iter().map(|p| { let t = fs::read_to_string(&p).unwrap(); (p, t) }).collect()
}

/// Words that would mean a consumer-account sign-in route.
const SIGN_IN_WORDS: [&str; 8] = ["oauth", "subscription", "sign-in", "sign in", "signin", "claude.ai", "bearer", "authorization"];

#[test]
fn the_anthropic_adapter_has_no_subscription_sign_in_path() {
    let src = all_src();
    let (_, adapter) = src.iter().find(|(p, _)| p.ends_with("providers/anthropic.rs")).expect("the Anthropic adapter file");
    for w in SIGN_IN_WORDS {
        assert!(!adapter.to_lowercase().contains(w), "providers/anthropic.rs mentions '{w}'");
    }
    // No line anywhere in the crate ties the name to a sign-in route.
    for (path, text) in &src {
        for (n, line) in text.lines().enumerate() {
            let l = line.to_lowercase();
            if l.contains("anthropic") {
                for w in SIGN_IN_WORDS {
                    assert!(!l.contains(w), "{}:{} ties Anthropic to '{w}'", path.display(), n + 1);
                }
            }
        }
    }
    let row = info(ProviderId::Anthropic);
    assert_eq!(row.auth, Auth::ApiKeyHeader);
    assert!(!row.sign_in);
}

#[test]
fn every_endpoint_lives_in_the_providers_table_and_nowhere_else() {
    for (path, text) in all_src() {
        if path.ends_with("table.rs") { continue; }
        for (n, line) in text.lines().enumerate() {
            assert!(!line.contains("http://") && !line.contains("https://"), "{}:{} holds an address outside table.rs", path.display(), n + 1);
        }
    }
}

#[test]
fn no_socket_code_outside_the_live_feature() {
    for (path, text) in all_src() {
        if path.ends_with("live.rs") { continue; }
        for w in ["reqwest", "TcpStream", "std::net"] {
            assert!(!text.contains(w), "{} uses {w}", path.display());
        }
    }
}

#[test]
fn one_adapter_file_and_one_fixture_file_per_provider() {
    for p in PROVIDERS.iter() {
        assert!(root().join(format!("src/providers/{}.rs", p.slug)).is_file(), "no source file for {}", p.name);
        assert!(root().join(format!("fixtures/providers/{}.json", p.slug)).is_file(), "no fixture for {}", p.name);
    }
    for s in SPEECH.iter() {
        assert!(root().join(format!("src/speech/{}.rs", s.slug)).is_file(), "no source file for {}", s.name);
        assert!(root().join(format!("fixtures/speech/{}.json", s.slug)).is_file(), "no fixture for {}", s.name);
    }
}

#[test]
fn the_fixture_keys_are_invented() {
    for kind in ["providers", "speech"] {
        for e in fs::read_dir(root().join("fixtures").join(kind)).unwrap() {
            let p = e.unwrap().path();
            if p.extension().is_some_and(|x| x == "json") {
                let v: serde_json::Value = serde_json::from_str(&fs::read_to_string(&p).unwrap()).unwrap();
                for k in ["key", "bad_key"] {
                    if let Some(s) = v[k].as_str() {
                        assert!(s.contains("FIXTURE"), "{} holds a key that is not marked FIXTURE", p.display());
                    }
                }
            }
        }
    }
}
