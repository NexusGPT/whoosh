//! lane fix-suggest (docs/lanes/fix-suggest.md): the REAL engine offers the follow-ups the test engine offers (FixtureLabels.swift
//! `offerAfter*Locked`): junk => Block sender, archive of a list mail => Unsubscribe, the third snooze of one thread => Mute thread, the same
//! label put twice on one sender => Always label. Per act: the act repeated as SPEC and the test engine say gives exactly ONE offer, read
//! back through the paths the app reads (`suggestions()`, `suggestion/count`, `suggestion/last`, `suggestion/<id>/<field>`); accepting runs
//! its effect; a dismissal keeps it from coming back; a restart keeps the offers; each act with its offer stays under 50 ms on a store of
//! 2,000 mails (`TIME <act> <ms> ms`). Each test prints `PASS <case>` lines.
//! BROKEN COPIES (one per behaviour, the exact `find` and `replace`, and the case that then fails): docs/lanes/fix-suggest.md, table.
#[allow(dead_code, unused_imports)]
#[path = "../../autoconfig/tests/support/mod.rs"]
mod detect_fakes;

use std::sync::Arc;
use std::time::Instant;

use contracts::*;
use detect_fakes::{FakeDns, FakeHttp, FakeProbe};
use engine::{RustCore, TestTransport};
use secrets::{MemoryKeychain, Namespace};
use store::{NewMessage, Store};

const USER: &str = "owner@example.test";

fn detector() -> Box<dyn engine::Detect> { Box::new(autoconfig::Detector::new(FakeHttp::default(), FakeDns::default(), FakeProbe::default())) }
fn kc() -> Arc<MemoryKeychain> { Arc::new(MemoryKeychain::new(Namespace::rig())) }
fn config() -> AccountConfig {
    AccountConfig {
        imap_host: "127.0.0.1".into(), imap_port: 1, imap_security: ConnectionSecurity::Ssl, smtp_host: "127.0.0.1".into(), smtp_port: 1,
        smtp_security: ConnectionSecurity::Starttls, username: USER.into(), smtp_username: USER.into(), auth_method: AuthMethod::Password, source: DetectionSource::Manual,
    }
}
fn msg(account: &str, id: &str, thread: &str, from: &str, at: i64) -> NewMessage {
    NewMessage {
        message: Message {
            id: id.into(), thread_id: thread.into(), account_id: account.into(), sender_name: "Someone".into(), from_address: from.into(), to: vec![USER.into()],
            subject: format!("Subject {id}"), preview: format!("Preview {id}"), received_at_ms: at, read: false, archived: false, folder: "INBOX".into(),
            attachment_names: vec![], login_code: None, trackers_blocked: 0, list_unsubscribe: vec![], list_unsubscribe_one_click: false,
            reply_to_name: None, cc: vec![], bcc: vec![], is_outgoing: false,
        },
        remote_id: None, body_text: None, body_html: None, has_list_header: false, size_bytes: Some(100), tab_id: None,
    }
}
/// A mail of a mailing list: its List-Unsubscribe is a web page (no one-click), so leaving it sends nothing anywhere.
fn list_msg(account: &str, id: &str, thread: &str, from: &str, at: i64) -> NewMessage {
    let mut m = msg(account, id, thread, from, at);
    m.message.list_unsubscribe = vec![format!("<https://{}/leave>", from.rsplit('@').next().unwrap_or("list.test"))];
    m.has_list_header = true;
    m
}
fn open_core(path: &std::path::Path) -> (RustCore, String) {
    let store = Store::open(path).unwrap();
    let id = match store.account_summaries().unwrap().first() {
        Some(a) => a.id.clone(),
        None => store.add_account(USER, "Owner", MailProvider::Imap, "svc\u{1f}acct", false, config(), 1).unwrap(),
    };
    (RustCore::with_parts(store, kc(), detector(), TestTransport::PlainLoopback), id)
}
/// 2,000 mails: t1 holds m1 and m2 (ann), t2 holds m3 (bob), t3 m4 (promo), t4 m5 (promo), t5 m6 (promo), t6 m7 (x@blocked.test),
/// n1, n2 and n5 (news@letter.test, three threads), n3 (left@letter2.test), n4 (news@other.test): 12, and 1,988 more.
fn big() -> (RustCore, String, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let (core, id) = open_core(&dir.path().join("whoosh.sqlite"));
    let mut all = vec![
        msg(&id, "m1", "t1", "ann@ext.test", 100), msg(&id, "m2", "t1", "Ann@Ext.test", 110), msg(&id, "m3", "t2", "bob@ext.test", 120),
        msg(&id, "m4", "t3", "promo@spam.test", 130), msg(&id, "m5", "t4", "promo@spam.test", 140), msg(&id, "m6", "t5", "promo@spam.test", 150),
        msg(&id, "m7", "t6", "x@blocked.test", 160),
        list_msg(&id, "n1", "tn1", "news@letter.test", 170), list_msg(&id, "n2", "tn2", "news@letter.test", 180),
        list_msg(&id, "n3", "tn3", "left@letter2.test", 190), list_msg(&id, "n4", "tn4", "news@other.test", 200),
        list_msg(&id, "n5", "tn5", "news@letter.test", 210),
    ];
    let more: Vec<NewMessage> = (0..1988).map(|i| msg(&id, &format!("b{i}"), &format!("bt{}", i / 3), &format!("sender{}@bulk{}.test", i % 40, i % 7), 1000 + i as i64)).collect();
    all.extend(more);
    assert_eq!(all.len(), 2000, "a store of 2,000 mails");
    core.store().insert_messages(all).unwrap();
    (core, id, dir)
}
fn reopen(core: RustCore, dir: &tempfile::TempDir) -> RustCore {
    drop(core);
    open_core(&dir.path().join("whoosh.sqlite")).0
}
fn timed<T>(what: &str, f: impl FnOnce() -> T) -> T {
    let t0 = Instant::now();
    let r = f();
    let ms = t0.elapsed().as_millis();
    println!("TIME {what} {ms} ms");
    assert!(ms < 50, "{what} took {ms} ms, the bar is 50");
    r
}
fn now() -> i64 { std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as i64 }
fn st(core: &RustCore, p: &str) -> Option<String> { core.state(p.to_string()) }
fn q(s: &str) -> String { format!("{s:?}") }
fn ids(v: &[&str]) -> Vec<String> { v.iter().map(|s| s.to_string()).collect() }
fn count(core: &RustCore) -> String { st(core, "suggestion/count").unwrap_or_default() }
/// The one offer on the list, checked on every path the app reads: the trait read, the count, the newest, the row's fields.
fn the_offer(core: &RustCore, kind: &str, subject: &str) -> String {
    let list = core.suggestions();
    assert_eq!(list.len(), 1, "exactly one offer on the list: {list:?}");
    let id = list[0].id.clone();
    assert_eq!(count(core), "1");
    assert_eq!(st(core, "suggestion/last"), Some(q(&id)));
    assert_eq!(st(core, &format!("suggestion/{id}/kind")), Some(q(kind)));
    assert_eq!(st(core, &format!("suggestion/{id}/subject")), Some(q(subject)));
    assert_eq!(st(core, &format!("suggestion/{id}/accepted")).as_deref(), Some("false"));
    id
}

#[test]
fn junk_offers_block_sender_once_accepting_blocks_and_a_restart_keeps_the_offer() {
    let (core, _acc, dir) = big();
    assert_eq!(count(&core), "0");
    let out = timed("MarkSpam+offer", || core.run(Command::MarkSpam { message_ids: ids(&["m4"]) })).expect("junked");
    let id = the_offer(&core, "blockSender", "promo@spam.test");
    assert_eq!(out.created_id.as_deref(), Some(id.as_str()), "the act answers the offer's id, as the test engine does");
    println!("PASS junk-offers-block-sender");
    // the same sender junked again: still the one offer
    let again = timed("MarkSpam+offer(again)", || core.run(Command::MarkSpam { message_ids: ids(&["m5"]) })).expect("junked");
    assert_eq!(again.created_id, None, "no second offer");
    assert_eq!(the_offer(&core, "blockSender", "promo@spam.test"), id);
    println!("PASS junk-offers-once-per-sender");
    // a restart keeps the offer
    let core = reopen(core, &dir);
    assert_eq!(the_offer(&core, "blockSender", "promo@spam.test"), id);
    println!("PASS junk-offer-survives-a-restart");
    // accepting runs its effect: the block the junk filter reads
    assert_eq!(st(&core, "block/promo@spam.test/blocked").as_deref(), Some("false"));
    core.run(Command::AcceptSuggestion { suggestion_id: id.clone() }).expect("accepted");
    assert_eq!(st(&core, "block/promo@spam.test/blocked").as_deref(), Some("true"));
    assert_eq!(count(&core), "0");
    println!("PASS junk-offer-accepted-blocks-the-sender");
    // an accepted offer is never made again, and a sender already blocked (here by its domain) is offered nothing
    core.run(Command::MarkSpam { message_ids: ids(&["m6"]) }).expect("junked");
    assert_eq!(count(&core), "0", "accepted: never offered again");
    core.run(Command::BlockDomain { domain: "blocked.test".into() }).expect("blocked");
    let blocked = core.run(Command::MarkSpam { message_ids: ids(&["m7"]) }).expect("junked");
    assert_eq!((count(&core).as_str(), blocked.created_id), ("0", None), "a blocked sender is offered nothing");
    println!("PASS junk-offers-nothing-for-a-blocked-sender");
}

#[test]
fn archive_of_a_list_mail_offers_unsubscribe_once_and_a_dismissal_keeps_it_away_after_a_restart() {
    let (core, _acc, dir) = big();
    // a mail with no list header, and a list the owner already left: nothing
    core.run(Command::Archive { message_ids: ids(&["m3"]) }).expect("archived");
    assert_eq!(count(&core), "0", "a mail of no list is offered nothing");
    core.store().mark_unsubscribed("n3".into(), now()).unwrap();
    core.run(Command::Archive { message_ids: ids(&["n3"]) }).expect("archived");
    assert_eq!(count(&core), "0", "a list already left is offered nothing");
    println!("PASS archive-offers-nothing-without-a-list-to-leave");
    let out = timed("Archive+offer", || core.run(Command::Archive { message_ids: ids(&["n1"]) })).expect("archived");
    let id = the_offer(&core, "unsubscribe", "news@letter.test");
    assert_eq!(out.created_id.as_deref(), Some(id.as_str()));
    println!("PASS archive-offers-unsubscribe");
    // dismissed: gone, and the next issue of the same list offers nothing, before and after a restart
    core.run(Command::DismissSuggestion { suggestion_id: id.clone() }).expect("dismissed");
    assert_eq!((count(&core).as_str(), st(&core, &format!("suggestion/{id}/kind"))), ("0", None));
    assert!(core.suggestions().is_empty());
    let next = timed("Archive+offer(dismissed)", || core.run(Command::Archive { message_ids: ids(&["n2"]) })).expect("archived");
    assert_eq!((count(&core).as_str(), next.created_id), ("0", None), "a dismissed offer never comes back");
    let core = reopen(core, &dir);
    core.run(Command::Archive { message_ids: ids(&["n5"]) }).expect("archived");
    assert_eq!(count(&core), "0", "nor after a restart");
    println!("PASS dismissed-offer-never-comes-back");
    // accepting runs its effect: the list is left
    core.run(Command::Archive { message_ids: ids(&["n4"]) }).expect("archived");
    let id = the_offer(&core, "unsubscribe", "news@other.test");
    let core = reopen(core, &dir);
    assert_eq!(the_offer(&core, "unsubscribe", "news@other.test"), id, "a restart keeps the offer");
    assert_eq!(st(&core, "message/n4/unsubscribed").as_deref(), Some("false"));
    core.run(Command::AcceptSuggestion { suggestion_id: id }).expect("accepted");
    assert_eq!(st(&core, "message/n4/unsubscribed").as_deref(), Some("true"));
    assert_eq!(count(&core), "0");
    println!("PASS unsubscribe-offer-accepted-leaves-the-list");
}

#[test]
fn the_third_snooze_of_a_thread_offers_mute_once_and_accepting_mutes_it() {
    let (core, _acc, dir) = big();
    let later = |h: i64| SnoozeWhen::At { at_ms: now() + h * 3_600_000 };
    let first = timed("Snooze+offer(1)", || core.run(Command::Snooze { message_ids: ids(&["m1"]), when: later(1) })).expect("snoozed");
    core.run(Command::Snooze { message_ids: ids(&["m2"]), when: later(2) }).expect("snoozed");
    assert_eq!((count(&core).as_str(), first.created_id), ("0", None), "two snoozes offer nothing");
    let third = timed("Snooze+offer(3)", || core.run(Command::Snooze { message_ids: ids(&["m1"]), when: later(3) })).expect("snoozed");
    let id = the_offer(&core, "muteThread", "t1");
    assert_eq!(third.created_id.as_deref(), Some(id.as_str()));
    println!("PASS third-snooze-offers-mute-thread");
    let fourth = core.run(Command::Snooze { message_ids: ids(&["m2"]), when: later(4) }).expect("snoozed");
    assert_eq!(fourth.created_id, None);
    assert_eq!(the_offer(&core, "muteThread", "t1"), id, "the fourth snooze offers nothing more");
    println!("PASS mute-offered-once-per-thread");
    let core = reopen(core, &dir);
    assert_eq!(the_offer(&core, "muteThread", "t1"), id);
    println!("PASS mute-offer-survives-a-restart");
    core.run(Command::AcceptSuggestion { suggestion_id: id }).expect("accepted");
    assert_eq!((st(&core, "message/m1/muted").as_deref(), st(&core, "message/m2/muted").as_deref()), (Some("true"), Some("true")));
    assert_eq!(count(&core), "0");
    println!("PASS mute-offer-accepted-mutes-the-thread");
    // a thread already muted is offered nothing, however often it is snoozed
    core.run(Command::Mute { message_ids: ids(&["m3"]), muted: true }).expect("muted");
    for h in 1..=4 { core.run(Command::Snooze { message_ids: ids(&["m3"]), when: later(h) }).expect("snoozed"); }
    assert_eq!(count(&core), "0", "a muted thread is offered nothing");
    println!("PASS muted-thread-offered-nothing");
}

#[test]
fn the_same_label_twice_on_one_sender_offers_always_label_and_accepting_writes_the_rule() {
    let (core, _acc, dir) = big();
    let label = |m: &str| Command::Label { message_ids: ids(&[m]), label_name: "Clients".into(), create_if_missing: true };
    timed("Label+offer(1)", || core.run(label("m1"))).expect("labelled");
    core.run(label("m3")).expect("labelled");
    assert_eq!(count(&core), "0", "once per sender offers nothing");
    let second = timed("Label+offer(2)", || core.run(label("m2"))).expect("labelled");
    let id = the_offer(&core, "alwaysLabel", "ann@ext.test");
    assert_ne!(second.created_id.as_deref(), Some(id.as_str()), "a label act keeps its own answer");
    let label_id = core.labels().into_iter().find(|l| l.name == "Clients").expect("the label").id;
    assert_eq!(st(&core, &format!("suggestion/{id}/label_id")), Some(q(&label_id)));
    println!("PASS second-label-on-a-sender-offers-always-label");
    let core = reopen(core, &dir);
    assert_eq!(the_offer(&core, "alwaysLabel", "ann@ext.test"), id);
    println!("PASS always-label-offer-survives-a-restart");
    core.run(Command::AcceptSuggestion { suggestion_id: id }).expect("accepted");
    assert!(core.store().label_rules(&label_id).unwrap().conditions.contains(&RuleCondition::From { words: "ann@ext.test".into() }));
    assert_eq!(count(&core), "0");
    println!("PASS always-label-offer-accepted-writes-the-rule");
    // the third label on the same sender: accepted, never offered again
    core.run(Command::RemoveLabel { message_ids: ids(&["m1"]), label_id: label_id.clone() }).expect("removed");
    core.run(label("m1")).expect("labelled");
    assert_eq!(count(&core), "0", "accepted: never offered again");
    println!("PASS always-label-offered-once-per-sender");
}
