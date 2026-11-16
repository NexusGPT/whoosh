//! par-smartinbox: the four features of the todo, each with one core-level check:
//! smart inbox by AI with the user's provider · reminder if no reply · notifications limited to Primary · VIP rules.
//! (The app-visible half of each is a gap: no surface is locked-on-build; see docs/lanes/core-smart-inbox.md.)
mod support;
use contracts::{DecidedBy, DecisionProviderKind, DecisionQuestion};
use smart_inbox::{should_notify, NotifyPolicy, NoModel, PlaceNewMessages, RemindIfNoReply, ViaDecisionPort};
use support::*;

#[test]
fn feature_smart_inbox_by_ai_with_the_users_provider() {
    let fx = fixture();
    let db = TempDb::new("ai");
    let store = store::Store::open(&db.0).unwrap();
    seed_accounts_and_vips(&store, &fx);
    let plain = fx["messages"].as_array().unwrap().iter().find(|m| s(m, "from") == "person5@private5-example.net").unwrap().clone();
    assert_eq!(s(&plain, "expected_tab"), "primary");
    for (kind, by) in [(DecisionProviderKind::Jev, DecidedBy::Jev), (DecisionProviderKind::Local, DecidedBy::Laya)] {
        let port = RecordingPort { kind, calls: Default::default(), answer_choice: Some(("Updates".into(), 0.93)) };
        let model = ViaDecisionPort(&port);
        let mut inc = incoming(&plain);
        inc.new.message.id = format!("ai-{kind:?}");
        inc.new.message.thread_id = format!("t-ai-{kind:?}");
        let p = PlaceNewMessages { store: &store, model: &model }.execute(vec![inc], 9).unwrap().remove(0);
        assert_eq!((p.tab_id.as_str(), &p.decided_by, p.probability), ("updates", &by, Some(0.93)), "the user's provider {kind:?} placed it");
        let calls = port.calls.lock().unwrap();
        let (state, questions) = calls.iter().find(|(_, qs)| qs.iter().any(|q| matches!(q, DecisionQuestion::Choice { .. }))).expect("a Which tab question was sent");
        let DecisionQuestion::Choice { prompt, options, .. } = &questions[0] else { panic!() };
        assert_eq!(prompt, smart_inbox::list::WHICH_TAB_QUESTION);
        assert_eq!(options, &["Primary", "Updates", "Promotions", "Social"].map(String::from));
        let mut sent: Vec<&str> = state.fields.keys().map(String::as_str).collect();
        sent.sort();
        assert_eq!(sent, ["cc", "from", "list_header", "subject", "to"], "only the fields DECISION-MODEL.md lists leave for a provider");
        assert!(state.text.chars().count() <= smart_inbox::tuning::body_chars());
    }
    // Engine off: no provider is called and the list falls through.
    let port = RecordingPort { kind: DecisionProviderKind::Jev, calls: Default::default(), answer_choice: None };
    let mut inc = incoming(&plain);
    inc.new.message.id = "ai-off".into();
    inc.new.message.thread_id = "t-ai-off".into();
    let p = PlaceNewMessages { store: &store, model: &NoModel }.execute(vec![inc], 9).unwrap().remove(0);
    assert_eq!(p.decided_by, DecidedBy::Default);
    assert!(port.calls.lock().unwrap().is_empty());
}

#[test]
fn feature_reminder_if_no_reply() {
    let fx = fixture();
    let db = TempDb::new("remind");
    let model = FixtureModel::new(&fx, DecisionProviderKind::Jev);
    let store = seeded(&db, &fx, &model);
    let wait = fx["reminder_wait_ms_for_test"].as_i64().unwrap();
    let now = fx["now_ms"].as_i64().unwrap();
    let uc = RemindIfNoReply { store: &store };
    let due: Vec<String> = uc.execute(now, wait).unwrap().into_iter().map(|d| d.message_id).collect();
    let mut sorted = due.clone();
    sorted.sort();
    assert_eq!(sorted, strs(&fx["expected"], "reminders_due"));
    assert!(!due.contains(&"m110".to_string()) && fx["messages"].as_array().unwrap().iter().any(|m| s(m, "thread") == "tc-noreply-fresh"), "the 5-hour-old message is not due yet");
    // A reminder already set removes it from the list; a reply removes it too.
    let first = due[0].clone();
    let f2 = first.clone();
    store.write(move |ctx| { ctx.tx.execute("INSERT INTO reminder (id, message_id, when_ms, note, updated_at_ms) VALUES ('rem1', ?1, 0, '', 0)", [&f2])?; Ok(()) }).unwrap();
    let after: Vec<String> = uc.execute(now, wait).unwrap().into_iter().map(|d| d.message_id).collect();
    assert_eq!(after.len(), due.len() - 1);
    assert!(!after.contains(&first));
    let last_due = due.last().unwrap().clone();
    let thread = fx["messages"].as_array().unwrap().iter().find(|m| s(m, "id") == last_due).map(|m| s(m, "thread")).unwrap();
    let mut reply = incoming(&fx["messages"].as_array().unwrap()[0].clone());
    reply.new.message.id = "late-reply".into();
    reply.new.message.thread_id = thread;
    reply.new.message.received_at_ms = now;
    PlaceNewMessages { store: &store, model: &NoModel }.execute(vec![reply], 9).unwrap();
    let after: Vec<String> = uc.execute(now, wait).unwrap().into_iter().map(|d| d.message_id).collect();
    assert!(!after.contains(&last_due), "an answered message no longer needs a reminder");
}

#[test]
fn feature_notifications_limited_to_primary() {
    let fx = fixture();
    let db = TempDb::new("notify");
    let model = FixtureModel::new(&fx, DecisionProviderKind::Jev);
    let store = seeded(&db, &fx, &model);
    let conn = smart_inbox::load::open_reader(store.path()).unwrap();
    let ctx = smart_inbox::load::load_context(&conn).unwrap();
    let placer = smart_inbox::Placer::new(&ctx.tabs, &ctx.rules, &model);
    let ids: Vec<String> = fx["messages"].as_array().unwrap().iter().filter(|m| s(m, "folder") == "INBOX").map(|m| s(m, "id")).collect();
    let facts = smart_inbox::load::facts_for(&conn, &ctx, &ids).unwrap();
    let limited = NotifyPolicy { limit_to_primary: true };
    let mut rang = std::collections::BTreeMap::<String, usize>::new();
    for f in &facts {
        let m = fx["messages"].as_array().unwrap().iter().find(|m| s(m, "id") == f.message_id).unwrap();
        let mut f = f.clone();
        smart_inbox::load::apply_headers(&mut f, &smart_inbox::header_facts(&raw_headers(m)));
        let p = placer.place(&f);
        let rings = should_notify(true, &limited, &f, &p, &ctx.tabs);
        if rings {
            *rang.entry(p.tab_id.clone()).or_default() += 1;
            assert!(["primary", "vips", "needs-reply"].contains(&p.tab_id.as_str()), "{}: a {} message rang", f.message_id, p.tab_id);
            assert!(f.from_vip || !f.has_list_header, "list mail is silent unless the sender is a VIP");
        }
        if ["updates", "promotions", "social"].contains(&p.tab_id.as_str()) && !f.from_vip {
            assert!(!rings, "{} in {} must be silent", f.message_id, p.tab_id);
        }
        assert!(!should_notify(false, &limited, &f, &p, &ctx.tabs), "a quiet account never rings");
    }
    assert!(rang.get("primary").copied().unwrap_or(0) > 10 && rang["vips"] == 5 && rang["needs-reply"] == 5, "{rang:?}");
    // Switched off, social mail without a list header rings too.
    let open = NotifyPolicy { limit_to_primary: false };
    let social = facts.iter().find(|f| f.from_address.ends_with("linkedin.com")).unwrap();
    let p = placer.place(social);
    assert!(should_notify(true, &open, social, &p, &ctx.tabs) && !should_notify(true, &limited, social, &p, &ctx.tabs));
}

#[test]
fn feature_vip_rules() {
    let fx = fixture();
    let db = TempDb::new("vip");
    let model = FixtureModel::new(&fx, DecisionProviderKind::Jev);
    let store = seeded(&db, &fx, &model);
    let cats = categories(&store);
    let vip_ids: Vec<String> = fx["messages"].as_array().unwrap().iter().filter(|m| s(m, "expected_tab") == "vips").map(|m| s(m, "id")).collect();
    assert_eq!(vip_ids.len(), 5);
    for (id, why) in vip_ids.iter().zip(["address", "address in another case", "address", "domain", "subdomain, list header"]) {
        let id = id.as_str();
        assert_eq!((cats[id].tab.as_str(), cats[id].rule_id.as_deref()), ("vips", Some("which-tab/vip")), "{id} ({why})");
    }
    // The VIP entry is the first rule: a VIP who also matches a list header, a Gmail category or a promo word still lands in VIPs.
    assert_eq!(fx["vips"]["addresses"].as_array().unwrap().len(), 2);
    let on_vips = cats.values().filter(|r| r.tab == "vips").count();
    assert_eq!(on_vips, 5 + 0, "the VIPs tab holds the five VIP messages and nothing else (the model never sends mail there)");
    assert_eq!(store.counts().unwrap().tabs.iter().find(|t| t.tab_id == "vips").unwrap().unread, 5);
}
