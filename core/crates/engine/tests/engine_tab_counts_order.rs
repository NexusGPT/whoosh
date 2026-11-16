//! fix-statepaths (main88 wave): the counts' tabs are in the order of the strip, and a label write reaches the Counts stream through the table feed.
//! (1) `store::streams_for_table` lists Counts for the label tables, as writer.rs `COUNTS_TABLES` does, and a label write publishes a Counts event.
//! (2) `counts().tabs` (store/src/counts.rs `build`) follows the same order as the state path `inbox/tabs` (engine reads.rs): the views first, the pinned
//! labels in tree order with the tabs that have a `split_tab` row in the order of those rows, a hidden tab out; the enabled tabs that are no label
//! (shipped Social and VIPs are not pinned) come after. After ReorderSplitTabs both lists move alike.
//! Real core, real store on a temp folder, 2,000 stored mails, no network. Each timed step prints "<what>: N ms on 2000 mails" and is held under 50 ms
//! (the owner: "The objective is <50ms for any operation").
//!
//! Broken copies (one per behaviour; each is a single edit and names the test that then fails):
//! - T1 store/src/changes.rs `streams_for_table`: `"label_assignment" | "label" => &[StreamKind::Inbox, StreamKind::Counts],` -> `&[StreamKind::Inbox],`
//!   -> the_table_feed_lists_counts_for_every_table_the_writer_counts_from
//! - T2 store/src/writer.rs `COUNTS_TABLES`: drop "label" -> a_label_write_publishes_counts_through_the_path_the_table_feed_names
//! - T3 store/src/counts.rs `strip_order`: `if moved.len() == slots.len() {` -> `if false {` -> after_reorder_split_tabs_the_counts_tabs_follow_the_strip_and_a_hidden_tab_is_absent
//! - T4 store/src/counts.rs `build`: `if *enabled && inbox_view_from_id(id.clone()).is_none()` -> `if inbox_view_from_id(id.clone()).is_none()` (a hidden tab comes back)
//!   -> after_reorder_split_tabs_the_counts_tabs_follow_the_strip_and_a_hidden_tab_is_absent
//! - T5 store/src/counts.rs `build`: `strip_order(strip_labels(conn)?, &rows)` -> `strip_order(strip_labels(conn)?, &[])` (the rows no longer place the tabs)
//!   -> the_counts_tabs_keep_an_owner_label_in_its_place_on_the_strip
#[allow(dead_code, unused_imports)]
#[path = "../../autoconfig/tests/support/mod.rs"]
mod detect_fakes;

use std::sync::{Arc, Mutex};
use std::time::Instant;

use contracts::*;
use detect_fakes::{FakeDns, FakeHttp, FakeProbe};
use engine::{RustCore, TestTransport};
use secrets::{MemoryKeychain, Namespace};
use store::{NewMessage, Store};

const USER: &str = "owner@example.test";
const MAILS: i64 = 2_000;
const BUDGET_MS: f64 = 50.0;

fn detector() -> Box<dyn engine::Detect> { Box::new(autoconfig::Detector::new(FakeHttp::default(), FakeDns::default(), FakeProbe::default())) }
fn kc() -> Arc<MemoryKeychain> { Arc::new(MemoryKeychain::new(Namespace::rig())) }
fn config() -> AccountConfig {
    AccountConfig {
        imap_host: "127.0.0.1".into(), imap_port: 1, imap_security: ConnectionSecurity::Ssl, smtp_host: "127.0.0.1".into(), smtp_port: 1,
        smtp_security: ConnectionSecurity::Starttls, username: USER.into(), smtp_username: USER.into(), auth_method: AuthMethod::Password, source: DetectionSource::Manual,
    }
}

fn mail(account: &str, n: i64) -> NewMessage {
    let id = format!("{account}:INBOX:{n}");
    NewMessage {
        message: Message {
            id: id.clone(), thread_id: id, account_id: account.into(), sender_name: format!("Sender {n}"), from_address: format!("sender{n}@other.example"),
            to: vec![USER.into()], subject: format!("Subject {n}"), preview: format!("Preview {n}"), received_at_ms: 1_700_000_000_000 + n * 1000, read: n % 2 == 0, archived: false,
            folder: "INBOX".into(), attachment_names: vec![], login_code: None, trackers_blocked: 0, list_unsubscribe: vec![], list_unsubscribe_one_click: false,
            reply_to_name: None, cc: vec![], bcc: vec![], is_outgoing: false,
        },
        remote_id: Some(n.to_string()), body_text: None, body_html: None, has_list_header: false, size_bytes: Some(100), tab_id: None,
    }
}
fn mid(account: &str, n: i64) -> String { format!("{account}:INBOX:{n}") }

struct Fx { core: RustCore, account: String, _dir: tempfile::TempDir }

/// One account, MAILS inbox mails (odd numbers unread), the core over it.
fn setup() -> Fx {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(&dir.path().join("whoosh.sqlite")).unwrap();
    let account = store.add_account(USER, "Owner", MailProvider::Imap, "svc\u{1f}acct", false, config(), 1).unwrap();
    store.insert_messages((1..=MAILS).map(|n| mail(&account, n)).collect()).unwrap();
    Fx { core: RustCore::with_parts(store, kc(), detector(), TestTransport::PlainLoopback), account, _dir: dir }
}

/// Runs a command, prints its time, and holds it under the budget.
fn timed(core: &RustCore, what: &str, c: Command) -> Result<CommandOutcome, CoreError> {
    let t = Instant::now();
    let out = core.run(c);
    let ms = t.elapsed().as_secs_f64() * 1000.0;
    println!("{what}: {ms:.2} ms on {MAILS} mails");
    assert!(ms < BUDGET_MS, "{what} took {ms:.2} ms, not under {BUDGET_MS} ms");
    out
}

/// The tab ids of `counts()`, in order; the call is timed and held under the budget.
fn counted(core: &RustCore) -> Vec<String> {
    let t = Instant::now();
    let counts = core.counts();
    let ms = t.elapsed().as_secs_f64() * 1000.0;
    println!("counts(): {ms:.2} ms on {MAILS} mails");
    assert!(ms < BUDGET_MS, "counts() took {ms:.2} ms, not under {BUDGET_MS} ms");
    counts.tabs.into_iter().map(|t| t.tab_id).collect()
}

/// The strip, left to right: the ids `inbox/tabs` answers.
fn strip(core: &RustCore) -> Vec<String> {
    let json = core.state("inbox/tabs".to_string()).expect("inbox/tabs answers");
    let rows: serde_json::Value = serde_json::from_str(&json).expect("inbox/tabs is JSON");
    rows.as_array().expect("inbox/tabs is a list").iter().map(|t| t["id"].as_str().expect("a tab has an id").to_string()).collect()
}

fn v(list: &[&str]) -> Vec<String> { list.iter().map(|x| x.to_string()).collect() }

fn spec(name: &str, pinned: bool) -> LabelSpec {
    LabelSpec { name: name.into(), parent_id: None, colour: None, description: String::new(), scope: LabelScope::All, rules: RuleSet { conditions: vec![], match_mode: MatchMode::Any }, pinned }
}
fn create_full(core: &RustCore, s: LabelSpec) -> String { core.run(Command::CreateLabelFull { spec: s }).unwrap().created_id.expect("the new label's id") }

/// The counts' tabs must read like the strip: the strip first, in its order, then only the enabled tabs that are no label (never a hidden one).
fn assert_counts_follow_the_strip(core: &RustCore, after: &str, no_label_tabs: &[&str]) {
    let (counts, strip) = (counted(core), strip(core));
    assert_eq!(counts[..strip.len().min(counts.len())], strip[..], "{after}: counts().tabs begin with the strip's order\n counts: {counts:?}\n strip:  {strip:?}");
    assert_eq!(counts[strip.len()..], v(no_label_tabs)[..], "{after}: after the strip come the enabled tabs that are no label, in `split_tab` order\n counts: {counts:?}");
}

struct Heard(Mutex<Vec<CoreEvent>>);
impl EventSink for Heard { fn on_event(&self, event: CoreEvent) { self.0.lock().unwrap().push(event) } }
impl Heard { fn take(&self) -> Vec<CoreEvent> { std::mem::take(&mut *self.0.lock().unwrap()) } }
fn listen_counts(core: &RustCore) -> Arc<Heard> {
    let heard = Arc::new(Heard(Mutex::new(vec![])));
    core.subscribe(StreamKind::Counts, heard.clone());
    heard.take();
    heard
}
fn counts_events(heard: &[CoreEvent]) -> Vec<Counts> {
    heard.iter().filter_map(|e| match e { CoreEvent::CountsChanged { counts } => Some(counts.clone()), _ => None }).collect()
}

// ------------------------------------------------------------------------------------------------ the table feed

#[test]
fn the_table_feed_lists_counts_for_every_table_the_writer_counts_from() {
    // writer.rs `COUNTS_TABLES`, copied: the tables whose write makes the writer publish Counts. The table feed's documentation must say the same.
    let counts_tables = ["message", "message_category", "snooze", "pin", "split_tab", "draft", "outbox_entry", "label", "label_assignment"];
    for table in counts_tables {
        assert!(store::streams_for_table(table).contains(&StreamKind::Counts), "streams_for_table({table:?}) = {:?} lacks Counts", store::streams_for_table(table));
    }
    for table in ["label", "label_assignment"] {
        assert!(store::streams_for_table(table).contains(&StreamKind::Inbox), "a pinned label's tab rows are an Inbox list too: {table}");
    }
    assert!(!store::streams_for_table("attachment").contains(&StreamKind::Counts), "an attachment moves no figure");
    println!("PASS table-feed-counts-for-{}-tables", counts_tables.len());
}

#[test]
fn a_label_write_publishes_counts_through_the_path_the_table_feed_names() {
    let fx = setup();
    let core = &fx.core;
    let label = create_full(core, spec("Contracts", false));
    core.store().label_assign(label.clone(), vec![mid(&fx.account, 1), mid(&fx.account, 3)], 1).unwrap();
    let heard = listen_counts(core);
    // a write to the `label` table (the pin): one Counts event, and the label is a tab in it
    timed(core, "PinLabel on", Command::PinLabel { label_id: label.clone(), pinned: true }).unwrap();
    let events = counts_events(&heard.take());
    assert_eq!(events.len(), 1, "one Counts event for a write to the label table: {events:?}");
    assert!(events[0].tabs.iter().any(|t| t.tab_id == label && t.unread == 2), "the pinned label is a tab with its two unread mails: {:?}", events[0].tabs);
    // a write to the `label_assignment` table of a pinned label: one Counts event with the new figure
    let t = Instant::now();
    core.store().label_assign(label.clone(), vec![mid(&fx.account, 5)], 2).unwrap();
    let ms = t.elapsed().as_secs_f64() * 1000.0;
    println!("a filing under a pinned label: {ms:.2} ms on {MAILS} mails");
    assert!(ms < BUDGET_MS, "the filing took {ms:.2} ms, not under {BUDGET_MS} ms");
    let events = counts_events(&heard.take());
    assert_eq!(events.len(), 1, "one Counts event for a write to the label_assignment table: {events:?}");
    assert!(events[0].tabs.iter().any(|t| t.tab_id == label && t.unread == 3), "three unread mails now: {:?}", events[0].tabs);
    // and the table feed names both tables as Counts feeds
    assert!(store::streams_for_table("label").contains(&StreamKind::Counts) && store::streams_for_table("label_assignment").contains(&StreamKind::Counts));
}

// ------------------------------------------------------------------------------------------------ the order of the counts' tabs

#[test]
fn after_reorder_split_tabs_the_counts_tabs_follow_the_strip_and_a_hidden_tab_is_absent() {
    let fx = setup();
    let core = &fx.core;
    // the shipped strip: Primary, All, then the pinned Needs Reply, Updates, Promotions; Social and VIPs are enabled tabs that are no pinned label
    assert_eq!(strip(core), v(&["primary", "all", "needs-reply", "updates", "promotions"]));
    assert_counts_follow_the_strip(core, "before any change", &["social", "vips"]);
    let order = v(&["primary", "promotions", "vips", "needs-reply", "social", "updates"]);
    timed(core, "ReorderSplitTabs", Command::ReorderSplitTabs { ids: order }).unwrap();
    assert_eq!(strip(core), v(&["primary", "all", "promotions", "needs-reply", "updates"]), "the strip follows the new order");
    let after = counted(core);
    assert_eq!(after[..5], v(&["primary", "all", "promotions", "needs-reply", "updates"])[..], "the counts' tabs follow it: {after:?}");
    assert_counts_follow_the_strip(core, "after ReorderSplitTabs", &["vips", "social"]);
    // a hidden tab is no tab of the strip and none of the counts
    timed(core, "SetSplitTabEnabled off", Command::SetSplitTabEnabled { id: "updates".into(), on: false }).unwrap();
    assert!(!counted(core).contains(&"updates".to_string()), "a hidden tab leaves the counts");
    assert_counts_follow_the_strip(core, "after hiding Updates", &["vips", "social"]);
    timed(core, "SetSplitTabEnabled on", Command::SetSplitTabEnabled { id: "updates".into(), on: true }).unwrap();
    assert_counts_follow_the_strip(core, "after showing Updates again", &["vips", "social"]);
    assert_eq!(counted(core)[..5], v(&["primary", "all", "promotions", "needs-reply", "updates"])[..], "shown again, it comes back where the order put it");
}

#[test]
fn the_counts_tabs_keep_an_owner_label_in_its_place_on_the_strip() {
    let fx = setup();
    let core = &fx.core;
    // an owner label, pinned and moved to the top of the tree: the strip draws it before the built-in tabs; the counts used to list it last
    let alpha = create_full(core, spec("Alpha", true));
    core.store().label_assign(alpha.clone(), vec![mid(&fx.account, 1), mid(&fx.account, 3), mid(&fx.account, 4)], 1).unwrap();
    timed(core, "ReorderLabel to the top", Command::ReorderLabel { label_id: alpha.clone(), new_parent_id: None, new_index: 0 }).unwrap();
    let strip_now = strip(core);
    assert_eq!(strip_now[2], alpha, "the strip draws the owner label first among the labels: {strip_now:?}");
    assert_counts_follow_the_strip(core, "owner label at the top", &["social", "vips"]);
    // its figures are the unread mails filed under it, and the tab is the same row after a reorder of the split tabs
    let tabs = core.counts().tabs;
    let own = tabs.iter().find(|t| t.tab_id == alpha).expect("the owner label is a tab of the counts");
    assert_eq!(own.unread, 2, "mails 1 and 3 are unread, 4 is read");
    timed(core, "ReorderSplitTabs", Command::ReorderSplitTabs { ids: v(&["primary", "promotions", "vips", "needs-reply", "social", "updates"]) }).unwrap();
    assert_counts_follow_the_strip(core, "owner label, then ReorderSplitTabs", &["vips", "social"]);
    assert_eq!(strip(core)[2], alpha, "the owner label keeps its place on the strip, so in the counts");
}
