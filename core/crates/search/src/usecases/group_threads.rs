//! Fused messages to threads (C-J2.1: one row per thread, never one per message). A thread takes its best
//! member's score and position; its matches keep the fused order. Exact holders go first (C-J2.3).
use crate::domain::{MessageMatch, Segment, ThreadResult};
use crate::usecases::fuse_ranks::Fused;
use crate::usecases::highlight::mark_words;
use contracts::constants::SEARCH_SNIPPET_CHARS;
use contracts::Message;
use std::collections::{HashMap, HashSet};

/// `facts` holds only the messages that passed the filters; a fused id without facts is dropped.
/// `snippets` are the text index's snippets by id; a message without one shows the start of its preview.
pub fn group_threads(fused: &[Fused], facts: &HashMap<String, Message>, snippets: &HashMap<String, String>, exact: &HashSet<String>, terms: &[String]) -> Vec<ThreadResult> {
    let mut ordered: Vec<&Fused> = fused.iter().filter(|f| facts.contains_key(&f.message_id)).collect();
    // Stable: inside each tier the fused order stands.
    ordered.sort_by_key(|f| !exact.contains(&f.message_id));
    let mut threads: Vec<ThreadResult> = Vec::new();
    let mut slot: HashMap<String, usize> = HashMap::new();
    for f in ordered {
        let m = &facts[&f.message_id];
        let text = snippets.get(&m.id).filter(|s| !s.trim().is_empty()).cloned().unwrap_or_else(|| m.preview.chars().take(SEARCH_SNIPPET_CHARS as usize).collect());
        let matched = MessageMatch {
            message_id: m.id.clone(),
            account_id: m.account_id.clone(),
            sender_name: m.sender_name.clone(),
            received_at_ms: m.received_at_ms,
            score: f.score,
            exact: exact.contains(&m.id),
            snippet: mark_words(&text, terms),
            attachment: None,
        };
        match slot.get(&m.thread_id) {
            Some(&at) => threads[at].matches.push(matched),
            None => {
                slot.insert(m.thread_id.clone(), threads.len());
                threads.push(ThreadResult {
                    thread_id: m.thread_id.clone(),
                    best_message_id: m.id.clone(),
                    account_id: m.account_id.clone(),
                    subject: m.subject.clone(),
                    score: f.score,
                    matches: vec![matched],
                });
            }
        }
    }
    threads
}

/// The plain text of marked runs (a test and preview aid).
pub fn marked_words(snippet: &[Segment]) -> Vec<String> {
    snippet.iter().filter(|s| s.marked).map(|s| s.text.clone()).collect()
}
