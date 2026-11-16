//! One list per page, rules and the model in it, first match wins; only switched-on pages are decided; every verdict
//! names its page and entry. The engine is the fake local runtime behind the real LocalDecision adapter.
mod support;
use contracts::DecisionProviderKind;
use decision::{DecideMessage, DecisionSettings, EntryKind, LabelNode, Outcome, Source, TabOption, Value};
use smart_inbox::TabRef;
use support::*;

fn tabs() -> Vec<TabOption> {
    ["Primary", "Updates", "Promotions", "Social"].iter().map(|n| TabOption { tab: TabRef::Builtin(n.to_string()), name: n.to_string() }).collect()
}

/// Asks: a "?" in the subject needs a reply (0.9); "receipt" is Updates (0.91); "PIN" carries a code (0.95).
fn fake() -> std::sync::Arc<FakeLocal> {
    FakeLocal::new(|id, _q, state| {
        let subject = subject_in(state);
        match id {
            "needs-reply/model" => noul(if subject.contains('?') { 0.9 } else { 0.1 }),
            "which-tab/model" if subject.contains("receipt") => choice("Updates", 0.91),
            "which-tab/model" => choice("Primary", 0.6),
            "sign-in-codes/model" => noul(if subject.contains("PIN") { 0.95 } else { 0.05 }),
            _ => None,
        }
    })
}

fn decide_all(settings: &DecisionSettings, engine: &dyn contracts::DecisionPort, m: &smart_inbox::MessageFacts) -> decision::Decisions {
    let t = tabs();
    DecideMessage { settings, engine: Some(engine), tabs: &t, labels: &[] }.execute(m)
}

#[test]
fn with_only_which_tab_on_no_other_page_is_decided_or_asked() {
    let settings = DecisionSettings::shipped(DecisionProviderKind::Local, &[Outcome::WhichTab]);
    let engine = Recording::new(local(fake()));
    for m in [msg("m1", "ana@client-example.com", "Can you send the deck?", "Hi"), msg("m2", "shop@store-example.com", "Your receipt", "Total"), msg("m3", "x@y-example.com", "Your PIN", "1234")] {
        let d = decide_all(&settings, &engine, &m);
        let outcomes: Vec<Outcome> = d.verdicts.iter().map(|v| v.outcome).collect();
        assert_eq!(outcomes, vec![Outcome::WhichTab], "{}: only the switched-on page is decided", m.message_id);
    }
    let ids = engine.ids();
    assert!(!ids.is_empty() && ids.iter().all(|i| i == "which-tab/model"), "only the Which tab question reaches the engine: {ids:?}");
}

#[test]
fn every_verdict_names_its_page_and_the_entry_that_decided() {
    let settings = DecisionSettings::shipped(DecisionProviderKind::Local, &Outcome::ALL);
    let engine = local(fake());
    let mut sources = std::collections::BTreeSet::new();
    for m in [msg("m1", "ana@client-example.com", "Can you send the deck?", "Hi"), msg("m2", "shop@store-example.com", "Your receipt", "Total"), msg("m3", "x@y-example.com", "Your verification code", "1234"), msg("m4", "x@y-example.com", "hello", "")] {
        let d = decide_all(&settings, &engine, &m);
        assert_eq!(d.verdicts.len(), Outcome::ALL.len(), "{}: one verdict per switched-on page", m.message_id);
        for v in &d.verdicts {
            let page = settings.page(v.outcome).unwrap();
            assert_eq!(v.decided_by.page, v.outcome);
            assert!(!v.decided_by.entry_id.is_empty(), "{:?} has no entry", v.outcome);
            assert_eq!(page.list.entries[v.decided_by.position - 1].id, v.decided_by.entry_id, "the position names the entry");
            assert!(v.decided_by.why().starts_with(&format!("{} · entry {}", v.outcome.words(), v.decided_by.position)));
            sources.insert(format!("{:?}", std::mem::discriminant(&v.decided_by.source)));
        }
    }
    assert_eq!(sources.len(), 3, "rules, the model and defaults all decided something");
}

#[test]
fn the_model_entry_decides_only_where_it_sits_and_at_its_trust_level() {
    let mut settings = DecisionSettings::shipped(DecisionProviderKind::Local, &[Outcome::WhichTab]);
    let engine = local(fake());
    // a list mail with "receipt": the list-header rule sits above the model and wins
    let mut m = msg("m1", "shop@store-example.com", "Your receipt", "Total");
    m.has_list_header = true;
    let v = decide_all(&settings, &engine, &m).verdicts.remove(0);
    assert_eq!((v.decided_by.entry_id.as_str(), v.decided_by.source.clone()), ("which-tab/list-header", Source::Rule));
    // the owner drags the model entry to the top: now the model decides the same message
    let list = &mut settings.page_mut(Outcome::WhichTab).unwrap().list;
    let at = list.entries.iter().position(|e| matches!(e.kind, EntryKind::Model { .. })).unwrap();
    let model = list.entries.remove(at);
    list.entries.insert(0, model);
    let v = decide_all(&settings, &engine, &m).verdicts.remove(0);
    assert_eq!(v.decided_by.position, 1);
    assert_eq!(v.value, Value::Tab(TabRef::Builtin("Updates".into())));
    assert!(matches!(v.decided_by.source, Source::Model { provider: DecisionProviderKind::Local, probability, .. } if (probability - 0.91).abs() < 1e-9));
    assert_eq!(v.decided_by.why(), "Which tab · entry 1 · the model said Updates (0.91)");
    // an answer under the trust level (0.6 < 0.80) falls through to the default
    let v = decide_all(&settings, &engine, &msg("m2", "a@b-example.com", "lunch", "")).verdicts.remove(0);
    assert_eq!((v.decided_by.entry_id.as_str(), v.decided_by.source.clone()), ("which-tab/default", Source::Default));
}

#[test]
fn engine_off_skips_every_model_entry() {
    let settings = DecisionSettings::shipped(DecisionProviderKind::Rules, &Outcome::ALL);
    let engine = Recording::new(local(fake()));
    let d = decide_all(&settings, &engine, &msg("m1", "ana@client-example.com", "Can you send the deck?", ""));
    assert!(engine.ids().is_empty() && d.questions_sent.is_empty());
    assert!(d.verdicts.iter().all(|v| !matches!(v.decided_by.source, Source::Model { .. })));
}

#[test]
fn one_engine_call_per_message_carries_every_reachable_question() {
    let settings = DecisionSettings::shipped(DecisionProviderKind::Local, &Outcome::ALL);
    let engine = Recording::new(local(fake()));
    decide_all(&settings, &engine, &msg("m1", "ana@client-example.com", "Can you send the deck?", ""));
    assert_eq!(engine.calls.lock().unwrap().len(), 1);
    let mut ids = engine.ids();
    ids.sort();
    assert_eq!(ids, vec!["needs-reply/model", "sign-in-codes/model", "which-tab/model"]);
}

#[test]
fn sign_in_codes_rule_then_model_at_ninety_percent_then_no() {
    let settings = DecisionSettings::shipped(DecisionProviderKind::Local, &[Outcome::SignInCodes]);
    let at = |subject: &str, p: f64| {
        let engine = local(FakeLocal::new(move |_, _, _| noul(p)));
        decide_all(&settings, &engine, &msg("m", "x@y-example.com", subject, "")).verdicts.remove(0)
    };
    let v = at("Your verification code", 0.0);
    assert_eq!((v.value, v.decided_by.entry_id.as_str()), (Value::Yes(true), "sign-in-codes/subject-words"));
    let v = at("Log in to your bank", 0.95);
    assert_eq!((v.value, v.decided_by.entry_id.as_str()), (Value::Yes(true), "sign-in-codes/model"));
    let v = at("Log in to your bank", 0.85);
    assert_eq!((v.value, v.decided_by.entry_id.as_str()), (Value::Yes(false), "sign-in-codes/default"));
}

#[test]
fn notify_me_reads_the_vip_and_needs_reply_pages() {
    let settings = DecisionSettings::shipped(DecisionProviderKind::Local, &[Outcome::Vips, Outcome::NeedsMyReply, Outcome::NotifyMe]);
    let engine = local(fake());
    let mut vip = msg("m1", "boss@client-example.com", "hello", "");
    vip.from_vip = true;
    let d = decide_all(&settings, &engine, &vip);
    assert_eq!(d.get(Outcome::NotifyMe).unwrap().decided_by.entry_id, "notify-me/vip");
    let d = decide_all(&settings, &engine, &msg("m2", "ana@client-example.com", "Can you send the deck?", ""));
    assert_eq!(d.get(Outcome::NotifyMe).unwrap().decided_by.entry_id, "notify-me/needs-reply");
    let d = decide_all(&settings, &engine, &msg("m3", "ana@client-example.com", "fyi", ""));
    assert_eq!((d.get(Outcome::NotifyMe).unwrap().value.clone(), d.get(Outcome::NotifyMe).unwrap().decided_by.entry_id.as_str()), (Value::Yes(false), "notify-me/default"));
    // with VIPs switched off, "if VIP" never fires
    let off = DecisionSettings::shipped(DecisionProviderKind::Local, &[Outcome::NotifyMe]);
    let d = decide_all(&off, &engine, &vip);
    assert_eq!(d.get(Outcome::NotifyMe).unwrap().decided_by.entry_id, "notify-me/default");
}

#[test]
fn the_labels_model_may_propose_a_new_child_under_an_existing_parent_and_never_writes_it() {
    // A140: `execute` runs the Labels page as the three-step pipeline, where a label with no description has no AI step: both are described
    let described = |id: &str, name: &str, parent: Option<&str>| LabelNode { description: format!("mail about {name}"), ..LabelNode::new(id, name, parent) };
    let labels = vec![described("l-client", "client", None), described("l-provanta", "Provanta", Some("l-client"))];
    let mut settings = DecisionSettings::shipped(DecisionProviderKind::Local, &[Outcome::Labels]);
    let list = &mut settings.page_mut(Outcome::Labels).unwrap().list;
    list.entries.insert(0, decision::Entry { id: "labels/model".into(), kind: EntryKind::Model { question: "Which label fits this email?".into(), trust_from: 0.7 } });
    // fix-aisort round 4: this fake is a free-text engine (as Jev): in per-label mode (LABEL_AI_QUESTION) no label reaches the trust level, so
    // the one choice is asked too and only its proposal is read; in choice mode the flag changes nothing. Broken copy P4 (decide.rs `walk`, the
    // per-label arm): `if found.is_none() && asked && self.settings.choice_is_open {` -> `if false {` -> red here with the row present.
    settings.choice_is_open = true;
    let run = |answer: &'static str| {
        let engine = local(FakeLocal::new(move |_, _, _| choice(answer, 0.88)));
        let t = tabs();
        DecideMessage { settings: &settings, engine: Some(&engine), tabs: &t, labels: &labels }.execute(&msg("m", "a@provanta-example.com", "Orbit kickoff", "")).verdicts.remove(0)
    };
    let v = run("client > Provanta > Orbit");
    assert_eq!(v.value, Value::Label(None), "a proposal adds no label");
    let p = v.proposal.expect("a proposal");
    assert_eq!((p.parent_id.as_str(), p.name.as_str()), ("l-provanta", "Orbit"));
    assert_eq!(v.decided_by.entry_id, "labels/model");
    let v = run("client > Provanta");
    assert_eq!((v.value, v.proposal), (Value::Label(Some("l-provanta".into())), None));
    let v = run("vendors > Acme");
    assert_eq!((v.value, v.decided_by.entry_id.as_str(), v.proposal), (Value::Label(None), "labels/default", None), "no existing parent: no proposal");
}
