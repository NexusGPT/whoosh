//! contracts delta 12 hook: the smart inbox's facts carry the sender's name and the unread bit, read from the store, so a
//! label rule on a sender's NAME and the live count's unread figure work at arrival and on a re-sort alike.
mod support;
use contracts::DecisionProviderKind;
use support::*;

#[test]
fn facts_read_back_carry_the_sender_name_and_unread() {
    let fx = fixture();
    let db = TempDb::new("facts-d12");
    let model = FixtureModel::new(&fx, DecisionProviderKind::Jev);
    let store = seeded(&db, &fx, &model);
    let conn = smart_inbox::load::open_reader(store.path()).unwrap();
    let ctx = smart_inbox::load::load_context(&conn).unwrap();
    let rows = fx["messages"].as_array().unwrap();
    let ids: Vec<String> = rows.iter().map(|m| s(m, "id")).collect();
    let facts = smart_inbox::load::facts_for(&conn, &ctx, &ids).unwrap();
    assert!(!facts.is_empty());
    for f in &facts {
        let m = rows.iter().find(|m| s(m, "id") == f.message_id).unwrap();
        assert_eq!(f.from_name, s(m, "from"), "{}: the sender's name", f.message_id);
        assert_eq!(f.unread, !m["read"].as_bool().unwrap(), "{}: unread", f.message_id);
    }
    assert!(facts.iter().any(|f| f.unread) && facts.iter().any(|f| !f.unread), "the fixture holds both, so the check can fail");
}
