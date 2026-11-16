//! Drives the crate the way the one search does, over the J2 corpus (checks/attachment-index-check.py writes the spec).
//! usage: j2_corpus <spec.tsv> <query>...    spec line: attachment_id TAB message_id TAB file TAB mime TAB state TAB text
//! state: indexed | not-downloaded | unreadable. Prints one JSON object per query: its hits as the one search would show them.
use attachment_index::twins::*;
use attachment_index::{hits_for_name, hits_for_text, hits_for_vectors, merge_attachment_hits, AttachmentIndexer, AttachmentInput, AttachmentRows, Limits, Readers, SqliteRows, VectorMatch};
use contracts::{Embedder, MatchedIn};
use rusqlite::Connection;
use std::sync::Arc;

fn js(s: &str) -> String {
    let mut o = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            c if (c as u32) < 0x20 => o.push(' '),
            c => o.push(c),
        }
    }
    o + "\""
}

fn cosine(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let spec = std::fs::read_to_string(&args[1]).expect("spec");
    let dir = std::env::temp_dir().join(format!("att-j2-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();

    let migrations = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../store/migrations/sql");
    let conn = Connection::open_in_memory().unwrap();
    for f in ["0001_v1.sql", "0002_nested_labels.sql", "0003_account_setup_search.sql", "0004_labels_tabs_blocks.sql"] {
        conn.execute_batch(&std::fs::read_to_string(migrations.join(f)).unwrap()).unwrap();
    }
    conn.execute_batch("PRAGMA foreign_keys = OFF;").unwrap();

    // The twin model "sees" a picture as its file name words plus what the reading returned: wiring proof, not retrieval quality.
    let (mut model, mut image) = (TwinEmbedder::default(), TwinImage::default());
    let rows_spec: Vec<Vec<&str>> = spec.lines().map(|l| l.split('\t').collect()).collect();
    for r in &rows_spec {
        let (file, text) = (r[2], r[5]);
        model.captions.insert(file.to_string(), format!("{} {}", file.replace(['-', '.'], " "), text));
        if !text.is_empty() {
            image.ocr.insert(file.to_string(), text.to_string());
        }
    }
    let model: Arc<dyn Embedder> = Arc::new(model);
    let indexer = AttachmentIndexer::new(
        model.clone(),
        Readers { text: Arc::new(TwinText::default()), image: Arc::new(image), speech: Arc::new(TwinSpeech::default()), probe: Arc::new(TwinProbe { duration_s: 1.0, has_sound: false }) },
        Limits::from_constants(),
        dir.join("scratch"),
    );

    let mut vectors: Vec<(String, Vec<f32>)> = Vec::new();
    for r in &rows_spec {
        let (att, msg, file, mime, state) = (r[0], r[1], r[2], r[3], r[4]);
        let local = dir.join(file);
        let have = state != "not-downloaded";
        if have {
            std::fs::write(&local, include_bytes!("../tests/fixtures/tiny.png")).unwrap();
        }
        conn.execute("INSERT INTO attachment (id, message_id, file_name, mime_type, size_bytes, local_path) VALUES (?1, ?2, ?3, ?4, 1, ?5)", rusqlite::params![att, msg, file, mime, have.then(|| local.to_string_lossy().into_owned())]).unwrap();
    }
    let store = SqliteRows::new(conn);
    for r in &rows_spec {
        let (att, msg, file, mime, state) = (r[0], r[1], r[2], r[3], r[4]);
        let local = dir.join(file);
        let input = AttachmentInput { message_id: msg, attachment_id: att, file_name: file, mime_type: mime, local_path: (state != "not-downloaded").then_some(local.as_path()), now_ms: 1 };
        let entry = indexer.index(&input);
        store.save(&entry).unwrap();
        vectors.extend(entry.vectors.iter().map(|v| (v.key.clone(), v.vector.clone())));
    }
    let messages: Vec<String> = rows_spec.iter().map(|r| r[1].to_string()).collect();
    let catalog = store.catalog(&messages).unwrap();

    for q in &args[2..] {
        let qv = model.embed_query(q.clone()).unwrap();
        let mut scored: Vec<VectorMatch> = vectors.iter().map(|(k, v)| VectorMatch { key: k.clone(), score: cosine(&qv, v) }).filter(|m| m.score > 0.0).collect();
        scored.sort_by(|a, b| b.score.total_cmp(&a.score));
        let (meaning, words, names) = (hits_for_vectors(&scored, &catalog), hits_for_text(q, &catalog), hits_for_name(q, &catalog));
        let merged = merge_attachment_hits(vec![meaning.clone(), words.clone(), names.clone()]);
        let list = |hits: &[contracts::SearchHit]| -> String {
            let items: Vec<String> = hits
                .iter()
                .map(|h| {
                    let m = match &h.matched_in { MatchedIn::AttachmentContent => "attachment-content", MatchedIn::AttachmentName => "attachment-name", MatchedIn::MeaningOnly => "meaning-only", MatchedIn::SubjectWords { .. } => "subject-words", MatchedIn::SenderWords { .. } => "sender-words", MatchedIn::TextWords { .. } => "text-words", MatchedIn::Related => "related" };
                    format!("{{\"message\":{},\"matched_in\":{},\"attachment\":{},\"note\":{},\"score\":{:.3}}}", js(&h.message_id), js(m), js(h.attachment_name.as_deref().unwrap_or("")), js(&h.snippet), h.score)
                })
                .collect();
            format!("[{}]", items.join(","))
        };
        println!("{{\"query\":{},\"hits\":{},\"meaning\":{},\"words\":{},\"names\":{}}}", js(q), list(&merged), list(&meaning), list(&words), list(&names));
    }
    let _ = std::fs::remove_dir_all(&dir);
}
