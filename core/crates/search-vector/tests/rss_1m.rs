//! Resident memory of a mapped one-million-message index. Slow, so ignored by default; run:
//! cargo test -p search-vector --release --test rss_1m -- --ignored --nocapture
//! The parent builds and saves the index; a fresh child process maps it and is the one measured,
//! so the build's own memory never counts.
mod common;
use common::{body, FakeEmbedder};
use search_vector::constants::{CHUNK_OVERLAP as O, CHUNK_TOKENS as T, MEM_MAC_MB, VECTOR_RSS_MB};
use search_vector::{expected_chunk_count, BaseBuilder, VectorIndex};
use std::{process::Command, sync::Arc, time::Instant};

const MESSAGES: u64 = 1_000_000;

/// Fixture shape: one message in ten is long (two or three chunks), the rest are short mail.
fn tokens_for(i: u64) -> usize {
    if i % 10 == 0 { T + 100 + (i % 7) as usize * 50 } else { 30 + (i * 31 % 200) as usize }
}

fn rss_mb(pid: u32) -> f64 {
    let out = Command::new("ps").args(["-o", "rss=", "-p", &pid.to_string()]).output().expect("ps runs");
    String::from_utf8_lossy(&out.stdout).trim().parse::<f64>().expect("ps prints kilobytes") / 1024.0
}

#[test]
#[ignore = "builds a one-million-message index; run on purpose"]
fn one_million_messages_stay_under_the_memory_budget() {
    let dir = tempfile::tempdir().unwrap();
    let total: usize = (1..=MESSAGES).map(|i| expected_chunk_count(tokens_for(i), T, O)).sum();
    let started = Instant::now();
    let builder = BaseBuilder::new(Arc::new(FakeEmbedder::new()), total).unwrap();
    let threads = std::thread::available_parallelism().map_or(2, |n| n.get().min(8)) as u64;
    std::thread::scope(|s| {
        for t in 0..threads {
            let b = &builder;
            s.spawn(move || {
                let mut i = 1 + t;
                while i <= MESSAGES {
                    b.add_message(i, &body(tokens_for(i), i)).unwrap();
                    i += threads;
                }
            });
        }
    });
    let ix = builder.finish(dir.path(), 1).unwrap();
    assert_eq!(ix.chunk_count(), total, "every chunk of every message is in the index");
    drop(ix);
    let file_mb = std::fs::metadata(dir.path().join("base-1.usearch")).unwrap().len() as f64 / 1048576.0;
    println!("BUILD messages={MESSAGES} chunks={total} threads={threads} seconds={:.0} file_mb={file_mb:.0}", started.elapsed().as_secs_f64());

    let out = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "measure_in_a_fresh_process", "--ignored", "--nocapture"])
        .env("WHOOSH_VECTOR_DIR", dir.path())
        .env("WHOOSH_VECTOR_CHUNKS", total.to_string())
        .output()
        .unwrap();
    println!("{}", String::from_utf8_lossy(&out.stdout));
    assert!(out.status.success(), "the measuring process failed:\n{}", String::from_utf8_lossy(&out.stderr));
}

#[test]
#[ignore = "child of the test above; does nothing without WHOOSH_VECTOR_DIR"]
fn measure_in_a_fresh_process() {
    let Ok(dir) = std::env::var("WHOOSH_VECTOR_DIR") else { return };
    let total: usize = std::env::var("WHOOSH_VECTOR_CHUNKS").unwrap().parse().unwrap();
    let pid = std::process::id();
    let ix = VectorIndex::open(std::path::Path::new(&dir), Arc::new(FakeEmbedder::new())).unwrap();
    assert!(ix.is_mapped());
    assert_eq!(ix.chunk_count(), total, "the relaunched index holds every chunk");
    let after_open = rss_mb(pid);
    let queries = 200u64;
    let mut found = 0;
    for q in 0..queries {
        let m = 1 + q * (MESSAGES / queries);
        let hit = ix.search(&body(tokens_for(m).min(T), m), 1).unwrap();
        found += usize::from(hit.first().is_some_and(|h| h.message_key == m && h.chunk == 0));
    }
    let after_search = rss_mb(pid);
    let footprint = Command::new("footprint").args(["-p", &pid.to_string()]).output().expect("footprint runs");
    for line in String::from_utf8_lossy(&footprint.stdout).lines().filter(|l| l.contains("Footprint") || l.contains("Dirty") || l.contains("Clean") || l.contains("MAPPED") || l.contains("mapped file")) {
        println!("FOOTPRINT {}", line.trim());
    }
    let (budget, source) = match VECTOR_RSS_MB {
        Some(v) => (v as f64, "VECTOR_RSS_MB"),
        None => (MEM_MAC_MB as f64, "MEM_MAC_MB (the whole app's ceiling; docs/CONSTANTS.md has no VECTOR_RSS_MB row)"),
    };
    println!("RSS pid={pid} after_open_mb={after_open:.1} after_{queries}_searches_mb={after_search:.1} budget_mb={budget:.0} budget_from={source}");
    println!("RECALL self-query top1 {found}/{queries}");
    assert!(found * 10 >= (queries as usize) * 9, "self-queries found their own message {found}/{queries}");
    assert!(after_search < budget, "resident {after_search:.1} MB is not under {budget} MB");
}
