//! fixtures/search-queries.json over the J2 corpus (fixtures/tests-j2.json), through the public API only.
use contracts::constants::RRF_K;
use contracts::Message;
use search::adapters::memory::MemoryCorpus;
use search::usecases::group_threads::marked_words;
use search::*;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;

fn repo_file(rel: &str) -> Value {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..").join(rel);
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("parse {}: {e}", path.display()))
}

fn s(v: &Value) -> String {
    v.as_str().unwrap_or_else(|| panic!("not a string: {v}")).to_string()
}

fn strings(v: &Value) -> Vec<String> {
    v.as_array().unwrap_or_else(|| panic!("not a list: {v}")).iter().map(s).collect()
}

fn ms_of(stamp: &str) -> i64 {
    let (date, time) = stamp.split_once('T').expect("date T time");
    let day = CivilDate::parse_iso(date).expect("iso date");
    let hms: Vec<i64> = time.split(':').map(|p| p.parse().expect("time part")).collect();
    day.start_ms(0) + ((hms[0] * 60 + hms[1]) * 60 + hms[2]) * 1000
}

struct World {
    fx: Value,
    corpus: Value,
    searcher: Searcher,
    store: Arc<MemoryCorpus>,
    today: CivilDate,
}

fn world() -> World {
    let fx = repo_file("fixtures/search-queries.json");
    let corpus = repo_file(fx["corpus"].as_str().expect("corpus path"));
    let mut mem = MemoryCorpus::default();
    for m in corpus["messages"].as_array().expect("messages") {
        let id = s(&m["id"]);
        mem.bodies.insert(id.clone(), s(&m["body"]));
        mem.messages.push(Message { list_unsubscribe: vec![], list_unsubscribe_one_click: false,
            id,
            thread_id: s(&m["thread"]),
            account_id: s(&m["account"]),
            sender_name: s(&m["from_name"]),
            from_address: s(&m["from"]),
            to: vec![],
            subject: s(&m["subject"]),
            preview: s(&m["body"]),
            received_at_ms: ms_of(&s(&m["date"])),
            read: false,
            archived: false,
            folder: "INBOX".into(),
            attachment_names: strings(&m["attachments"]),
            login_code: None,
            trackers_blocked: 0,
            reply_to_name: None,
            cc: vec![], bcc: vec![], is_outgoing: false,
        });
    }
    for a in fx["accounts"].as_array().expect("accounts") {
        mem.accounts.push(AccountState { id: s(&a["id"]), name: s(&a["name"]), indexed: a["indexed"].as_bool().expect("indexed") });
    }
    for (words, ids) in fx["meaning"].as_object().expect("meaning") {
        mem.meaning.insert(words.clone(), strings(ids));
    }
    let store = Arc::new(mem);
    let today = CivilDate::parse_iso(fx["today"].as_str().expect("today")).expect("today date");
    let searcher = Searcher { text: store.clone(), meaning: store.clone(), facts: store.clone(), scan: store.clone(), accounts: store.clone(), attachments: None };
    World { fx, corpus, searcher, store, today }
}

impl World {
    fn req<'a>(&self, text: &'a str) -> SearchRequest<'a> {
        SearchRequest { text, today: self.today, utc_offset_min: self.fx["utc_offset_min"].as_i64().expect("offset") as i32 }
    }
    fn run(&self, text: &str) -> SearchOutcome {
        self.searcher.run(&self.req(text)).unwrap_or_else(|e| panic!("search {text:?}: {e}"))
    }
}

fn thread_ids(o: &SearchOutcome) -> Vec<String> {
    o.threads.iter().map(|t| t.thread_id.clone()).collect()
}

fn message(o: &SearchOutcome, id: &str) -> MessageMatch {
    o.threads.iter().flat_map(|t| t.matches.iter()).find(|m| m.message_id == id).unwrap_or_else(|| panic!("{id} not in the results")).clone()
}

#[test]
fn filter_words_become_chips_and_only_matching_threads_come_back() {
    let w = world();
    for case in w.fx["filters"].as_array().expect("filters") {
        let text = s(&case["text"]);
        let out = w.run(&text);
        let want = case["chips"].as_array().expect("chips");
        assert_eq!(out.query.chips.len(), want.len(), "{text}: chips {:?}", out.query.chips);
        for (chip, exp) in out.query.chips.iter().zip(want) {
            assert_eq!(chip.kind.word(), exp["kind"].as_str().expect("kind"), "{text}");
            if let Some(v) = exp["value"].as_str() {
                assert_eq!(chip.value.as_deref(), Some(v), "{text}");
            }
            if let Some(bound) = exp["bound"].as_str() {
                let ms = CivilDate::parse_iso(bound).expect("bound").start_ms(0);
                let got = if chip.kind == ChipKind::Before { out.query.filters.before_ms } else { out.query.filters.after_ms };
                assert_eq!(got, Some(ms), "{text}: bound of {}", chip.words);
            }
        }
        assert_eq!(out.query.residual, s(&case["residual"]), "{text}");
        assert_eq!(thread_ids(&out), strings(&case["threads"]), "{text}");
        if case["messages"].is_array() {
            let got: HashSet<String> = out.threads.iter().flat_map(|t| t.matches.iter().map(|m| m.message_id.clone())).collect();
            let want: HashSet<String> = strings(&case["messages"]).into_iter().collect();
            assert_eq!(got, want, "{text}");
        }
        if let Some(marks) = case["marked"].as_object() {
            for (id, words) in marks {
                assert_eq!(marked_words(&message(&out, id).snippet), strings(words), "{text}: marks in {id}");
            }
        }
    }
}

#[test]
fn the_three_word_filter_line_yields_three_chips() {
    let w = world();
    // SPEC A142: the sender filter is the typed form from:"…"; the word "from" in a sentence stays a word
    let out = w.run("from:\"Linea Supply\" before June has attachment");
    let kinds: Vec<&str> = out.query.chips.iter().map(|c| c.kind.word()).collect();
    assert_eq!(kinds, ["from", "before", "attachment"]);
    // Removing a chip gives back the line without its words.
    let rest = out.query.without_chip(1);
    assert_eq!(rest, "from:\"Linea Supply\" has attachment");
    assert_eq!(w.run(&rest).query.chips.len(), 2);
}

#[test]
fn the_j2_filter_cases_read_the_same_way() {
    let w = world();
    for case in w.corpus["filter_cases"].as_array().expect("filter_cases") {
        let text = s(&case["text"]);
        let q = w.searcher.parse(&w.req(&text));
        let want = case["chips"].as_array().expect("chips");
        let kinds: Vec<&str> = q.chips.iter().map(|c| c.kind.word()).collect();
        let want_kinds: Vec<&str> = want.iter().map(|c| c["kind"].as_str().expect("kind")).collect();
        assert_eq!(kinds, want_kinds, "{text}");
        for (chip, exp) in q.chips.iter().zip(want) {
            if let Some(v) = exp["value"].as_str() {
                assert_eq!(chip.value.as_deref(), Some(v), "{text}");
            }
        }
        assert_eq!(q.residual, s(&case["residual"]), "{text}");
        assert_eq!(!q.not_understood.is_empty(), case["bar_says"].as_str() == Some("not understood"), "{text}: {:?}", q.not_understood);
    }
}

#[test]
fn fusion_order_equals_the_hand_computed_order() {
    let w = world();
    let f = &w.fx["fusion"];
    assert_eq!(f["k_constant"].as_str(), Some("RRF_K"));
    assert_eq!(f["k"].as_u64(), Some(RRF_K), "the fixture's hand sums assume the ledger's k");
    let fused = fuse_ranks(&strings(&f["text"]), &strings(&f["meaning"]));
    let order: Vec<String> = fused.iter().map(|x| x.message_id.clone()).collect();
    assert_eq!(order, strings(&f["order"]));
    for (got, hand) in fused.iter().zip(f["hand"].as_array().expect("hand")) {
        assert_eq!(got.message_id, s(&hand["id"]));
        let want = hand["score"].as_f64().expect("score");
        assert!((got.score - want).abs() < 1e-8, "{}: {} vs hand {want} ({})", got.message_id, got.score, s(&hand["shares"]));
    }
}

#[test]
fn fused_messages_group_by_thread_in_fused_order() {
    let w = world();
    let f = &w.fx["fusion"];
    let fused = fuse_ranks(&strings(&f["text"]), &strings(&f["meaning"]));
    let facts: HashMap<String, Message> = w.store.messages.iter().map(|m| (m.id.clone(), m.clone())).collect();
    let threads = group_threads(&fused, &facts, &HashMap::new(), &HashSet::new(), &[]);
    let want = f["threads"].as_array().expect("threads");
    assert_eq!(threads.len(), want.len());
    for (t, exp) in threads.iter().zip(want) {
        assert_eq!(t.thread_id, s(&exp["thread"]));
        let ids: Vec<String> = t.matches.iter().map(|m| m.message_id.clone()).collect();
        assert_eq!(ids, strings(&exp["matches"]), "{}", t.thread_id);
        assert_eq!(t.best_message_id, ids[0]);
        assert!(t.matches.iter().all(|m| !m.snippet.is_empty()), "every match carries a snippet");
    }
}

#[test]
fn meaning_alone_finds_a_thread_sharing_no_word() {
    let w = world();
    let q = &w.fx["meaning_query"];
    let out = w.run(q["text"].as_str().expect("text"));
    assert_eq!(thread_ids(&out), strings(&q["threads"]));
    for (thread, ids) in q["thread_matches"].as_object().expect("thread_matches") {
        let t = out.threads.iter().find(|t| &t.thread_id == thread).expect("thread present");
        assert_eq!(t.matches.iter().map(|m| m.message_id.clone()).collect::<Vec<_>>(), strings(ids));
    }
}

#[test]
fn an_exact_figure_outranks_meaning_and_is_marked() {
    let w = world();
    let e = &w.fx["exact"];
    let out = w.run(e["text"].as_str().expect("text"));
    assert!(!out.no_exact_match);
    assert_eq!(thread_ids(&out), strings(&e["threads"]));
    assert_eq!(out.threads[0].thread_id, s(&e["first_thread"]));
    for id in strings(&e["exact_messages"]) {
        assert!(message(&out, &id).exact, "{id} holds the phrase");
    }
    for id in strings(&e["not_exact"]) {
        assert!(!message(&out, &id).exact, "{id} must not count as exact");
    }
    for (id, words) in e["marked"].as_object().expect("marked") {
        assert_eq!(marked_words(&message(&out, id).snippet), strings(words));
    }
    let tier = &w.fx["exact_tier"];
    let out = w.run(tier["text"].as_str().expect("text"));
    assert_eq!(thread_ids(&out), strings(&tier["threads"]), "exact holders go before meaning-only hits");
    for (thread, ids) in tier["thread_matches"].as_object().expect("thread_matches") {
        let t = out.threads.iter().find(|t| &t.thread_id == thread).expect("thread present");
        assert_eq!(t.matches.iter().map(|m| m.message_id.clone()).collect::<Vec<_>>(), strings(ids));
    }
    let none = &w.fx["no_exact"];
    let out = w.run(none["text"].as_str().expect("text"));
    assert_eq!(out.no_exact_match, none["no_exact_match"].as_bool().expect("flag"));
    assert_eq!(thread_ids(&out), strings(&none["threads"]), "meaning results are kept");
}

#[test]
fn partial_results_name_the_accounts_still_loading() {
    let w = world();
    let p = &w.fx["partial"];
    let indexed = w.store.accounts.iter().filter(|a| a.indexed).count();
    assert_eq!(indexed as u64, p["indexed_count"].as_u64().expect("count"));
    let out = w.run(p["text"].as_str().expect("text"));
    assert_eq!(out.accounts_still_loading, strings(&p["loading"]));
    assert_eq!(out.offer_server_search, p["offer_server_search"].as_bool().expect("flag"));
    assert!(!out.threads.is_empty(), "indexed results are shown while the rest loads");
}

#[test]
fn a_saved_smart_folder_lists_the_same_threads_as_its_query() {
    let w = world();
    let sf = &w.fx["smart_folder"];
    let query = s(&sf["query"]);
    let parent = Some(s(&sf["parent"]));
    let folder = new_smart_folder(&w.searcher, "sf-1".into(), &s(&sf["name"]), parent.clone(), &w.req(&query), &[]).expect("saved");
    let listed = run_smart_folder(&w.searcher, &folder, &w.req("")).expect("listed");
    assert_eq!(thread_ids(&listed), thread_ids(&w.run(&query)));
    assert_eq!(thread_ids(&listed), strings(&sf["threads"]));
    let again = new_smart_folder(&w.searcher, "sf-2".into(), &s(&sf["existing_name_refused"]), parent, &w.req(&query), &[folder]);
    assert!(matches!(again, Err(contracts::CoreError::Invalid { ref field, .. }) if field == "name"), "{again:?}");
}

#[test]
fn adversarial_lines_never_break_the_search() {
    let w = world();
    for case in w.corpus["adversarial_queries"].as_array().expect("adversarial_queries") {
        let text = match case["repeat"].as_str() {
            Some(r) => r.repeat(case["times"].as_u64().expect("times") as usize),
            None => s(&case["text"]),
        };
        let out = w.run(&text);
        if text.trim().is_empty() {
            assert!(out.threads.is_empty() && out.query.chips.is_empty(), "{:?}", case["name"]);
        }
    }
}
