//! fn-smart-inbox-rules: counts per tab and per account, and the waiting threads; each equals the hand-labelled value
//! (computed in Python from the labels) AND the store's own recount.
mod support;
use contracts::DecisionProviderKind;
use smart_inbox::{account_counts, awaiting_reply_threads};
use std::collections::BTreeMap;
use support::*;

fn setup() -> (TempDb, serde_json::Value, store::Store) {
    let fx = fixture();
    let db = TempDb::new("counts");
    let model = FixtureModel::new(&fx, DecisionProviderKind::Jev);
    let store = seeded(&db, &fx, &model);
    (db, fx, store)
}

#[test]
fn per_account_and_tab_counts_equal_the_hand_labelled_ones() {
    let (_db, fx, store) = setup();
    let conn = smart_inbox::load::open_reader(store.path()).unwrap();
    let got: Vec<(String, String, u32, u32)> = account_counts(&conn).unwrap().into_iter().map(|c| (c.account_id, c.tab_id, c.unread, c.waiting)).collect();
    let want: Vec<(String, String, u32, u32)> = fx["expected"]["account_tab"].as_array().unwrap().iter().map(|r| (s(r, "account"), s(r, "tab"), r["unread"].as_u64().unwrap() as u32, r["waiting"].as_u64().unwrap() as u32)).collect();
    assert_eq!(got, want);
}

#[test]
fn tab_counts_equal_the_hand_labelled_ones_and_the_stores_select() {
    let (_db, fx, store) = setup();
    let counts = store.counts().unwrap();
    assert_eq!(counts, store.recount().unwrap(), "counts_stream value equals the from-scratch SELECT");
    let conn = smart_inbox::load::open_reader(store.path()).unwrap();
    let mut by_tab: BTreeMap<String, (u32, u32)> = BTreeMap::new();
    for c in account_counts(&conn).unwrap() {
        let e = by_tab.entry(c.tab_id).or_default();
        e.0 += c.unread;
        e.1 += c.waiting;
    }
    // Delta 16: "all" is a view, not a tab of the fixture. Its figure is every category summed (the labels' own totals), so it
    // has no entry of its own in the hand-labelled maps and no row of its own in the per-account counts.
    let sum_map = |k: &str| fx["expected"][k].as_object().unwrap().values().map(|v| v.as_u64().unwrap() as u32).sum::<u32>();
    let all = counts.tabs.iter().find(|t| t.tab_id == "all").expect("the All view row");
    assert_eq!((all.unread, all.waiting), (sum_map("tab_unread"), sum_map("tab_waiting")), "All is every category of the labels summed");
    assert_eq!(by_tab.values().fold((0, 0), |a, v| (a.0 + v.0, a.1 + v.1)), (all.unread, all.waiting), "All is the sum of every per-account count");
    for t in counts.tabs.iter().filter(|t| t.tab_id != "all") {
        let want_u = fx["expected"]["tab_unread"][&t.tab_id].as_u64().unwrap_or(0) as u32;
        let want_w = fx["expected"]["tab_waiting"][&t.tab_id].as_u64().unwrap_or(0) as u32;
        assert_eq!((t.unread, t.waiting), (want_u, want_w), "tab {} against the labels", t.tab_id);
        assert_eq!(by_tab.get(&t.tab_id).copied().unwrap_or((0, 0)), (t.unread, t.waiting), "tab {}: the sum of the per-account counts is the tab count", t.tab_id);
    }
    assert_eq!(counts.tabs.iter().find(|t| t.tab_id == "needs-reply").unwrap().waiting, 5, "five fixture messages are placed in Needs Reply by the model, each unanswered");
}

#[test]
fn waiting_is_the_inbound_threads_whose_last_message_is_unanswered() {
    let (_db, fx, store) = setup();
    let conn = smart_inbox::load::open_reader(store.path()).unwrap();
    let got = awaiting_reply_threads(&conn).unwrap();
    let mut want = strs(&fx["expected"], "awaiting_threads");
    want.sort();
    assert_eq!(got, want);
    let counts = store.counts().unwrap();
    // Delta 16: the All row is the inbox's own figure; a sum over every row would count each thread twice (its tab and All).
    let all = counts.tabs.iter().find(|t| t.tab_id == "all").expect("the All view row").waiting;
    assert_eq!(all as usize, got.len(), "one waiting count per waiting thread: the All view");
    let over_tabs: u32 = counts.tabs.iter().filter(|t| t.tab_id != "all").map(|t| t.waiting).sum();
    assert_eq!(over_tabs, all, "the tabs' waiting counts add up to the All view");
    for t in ["tc-answered", "tc-ownersent", "tc-ping-pong", "tc-archived", "tc-snoozed"] {
        assert!(!got.contains(&t.to_string()), "{t} is not waiting");
    }
    assert!(got.contains(&"tc-waiting".to_string()));
}

#[test]
fn the_counts_follow_a_new_reply() {
    let (_db, fx, store) = setup();
    let before = store.counts().unwrap();
    // The owner answers the waiting conversation: it stops waiting, the unread count does not move.
    let mut reply = incoming(&fx["messages"].as_array().unwrap().iter().find(|m| s(m, "thread") == "tc-waiting" && s(m, "folder") == "Sent").unwrap().clone());
    reply.new.message.id = "reply-1".into();
    reply.new.message.received_at_ms = fx["now_ms"].as_i64().unwrap();
    smart_inbox::PlaceNewMessages { store: &store, model: &smart_inbox::NoModel }.execute(vec![reply], 9).unwrap();
    let after = store.counts().unwrap();
    assert_eq!(after, store.recount().unwrap());
    // Delta 16: the All row, the inbox's own waiting figure (a sum over every row would count the thread in its tab and in All).
    let w = |c: &contracts::Counts| c.tabs.iter().find(|t| t.tab_id == "all").expect("the All view row").waiting;
    assert_eq!(w(&after), w(&before) - 1);
}
