//! fix-taborder: the owner changes the order of the split tabs and a renamed tab shows its new name on the strip, in the real core.
//! Every case runs `RustCore` over a real migrated store (a fresh one: the six shipped tabs, Needs Reply / Updates / Promotions pinned) and a fake secret store.
//! Nothing here opens a socket. Each test ends with its BROKEN COPY: one edit that turns the test red.
#[allow(dead_code, unused_imports)]
#[path = "../../autoconfig/tests/support/mod.rs"]
mod detect_fakes;

use std::path::Path;
use std::sync::Arc;

use contracts::*;
use detect_fakes::{FakeDns, FakeHttp, FakeProbe};
use engine::{RustCore, TestTransport};
use secrets::{MemoryKeychain, Namespace};
use store::Store;

const NOT_ONCE: &str = "the order must list every tab once";
const VIEWS_FIRST: &str = "Primary is a view of the inbox, not a label, so it cannot be reordered: the views stay first.";

fn open_core(path: &Path, keys: Arc<MemoryKeychain>) -> RustCore {
    let detector: Box<dyn engine::Detect> = Box::new(autoconfig::Detector::new(FakeHttp::default(), FakeDns::default(), FakeProbe::default()));
    RustCore::with_parts(Store::open(path).unwrap(), keys, detector, TestTransport::Real)
}

struct Rig { core: RustCore, keys: Arc<MemoryKeychain>, dir: tempfile::TempDir }

impl Rig {
    fn new() -> Rig {
        let dir = tempfile::tempdir().unwrap();
        let keys = Arc::new(MemoryKeychain::new(Namespace::rig()));
        let core = open_core(&dir.path().join("whoosh.sqlite"), keys.clone());
        Rig { core, keys, dir }
    }
    fn run(&self, c: Command) { self.core.run(c).unwrap_or_else(|e| panic!("refused: {e:?}")) ; }
    /// The app is closed and opened again over the same store.
    fn restart(self) -> Rig {
        let Rig { core, keys, dir } = self;
        drop(core);
        let core = open_core(&dir.path().join("whoosh.sqlite"), keys.clone());
        Rig { core, keys, dir }
    }
    /// The strip the app draws, left to right: (id, name), read through the state path `inbox/tabs`.
    fn strip(&self) -> Vec<(String, String)> {
        let json = self.core.state("inbox/tabs".to_string()).expect("inbox/tabs has a value");
        json.split(r#"{"id":""#).skip(1).map(|row| {
            let id = row.split('"').next().unwrap().to_string();
            let name = row.split(r#""name":""#).nth(1).unwrap().split('"').next().unwrap().to_string();
            (id, name)
        }).collect()
    }
    fn strip_ids(&self) -> Vec<String> { self.strip().into_iter().map(|t| t.0).collect() }
}

fn ids(core: &RustCore) -> Vec<String> { core.split_tabs().into_iter().map(|t| t.id).collect() }
fn v(list: &[&str]) -> Vec<String> { list.iter().map(|x| x.to_string()).collect() }
fn refusal(r: Result<CommandOutcome, CoreError>) -> (String, String) {
    match r { Err(CoreError::Invalid { field, reason_words }) => (field, reason_words), other => panic!("expected an Invalid refusal, got {other:?}") }
}

#[test]
fn the_door_takes_an_order_that_keeps_the_views_first_and_the_store_writes_it_and_it_survives_a_restart() {
    let rig = Rig::new();
    assert_eq!(ids(&rig.core), v(&["primary", "needs-reply", "updates", "promotions", "social", "vips"]));
    assert_eq!(rig.strip_ids(), v(&["primary", "all", "needs-reply", "updates", "promotions"]));
    let order = v(&["primary", "promotions", "vips", "needs-reply", "social", "updates"]);
    assert!(check_view_command(Command::ReorderSplitTabs { ids: order.clone() }).is_ok(), "the views first, the others in any order: the door lets it by");
    rig.run(Command::ReorderSplitTabs { ids: order.clone() });
    assert_eq!(ids(&rig.core), order);
    assert_eq!(rig.state_of("split_tab/promotions/order"), Some("1".to_string()));
    assert_eq!(rig.strip_ids(), v(&["primary", "all", "promotions", "needs-reply", "updates"]), "the pinned tabs of the strip follow the new order, the views stay first");
    let rig = rig.restart();
    assert_eq!(ids(&rig.core), order, "the order is on disk");
    assert_eq!(rig.strip_ids(), v(&["primary", "all", "promotions", "needs-reply", "updates"]), "and the strip after the restart");
}
// broken copy: engine/src/rust_core.rs line 577 `check_view_command(command.clone())?;` becomes `check_view_command(command.clone())?; let _ = command; return Err(CoreError::Invalid { field: "ids".into(), reason_words: "no".into() });`
//   (or: contracts/src/inbox_views.rs `check_tab_order` returns `check_not_a_view(ids[0].clone(), ViewRefusedAction::Reorder)`): this test fails at its `rig.run(ReorderSplitTabs)` line.
// broken copy: store/src/app_settings.rs `split_tab_reorder`: `params![id, i as i64, now_ms]` becomes `params![id, 0i64, now_ms]`: fails at `assert_eq!(ids(&rig.core), order)`.
// broken copy: engine/src/reads.rs `inbox/tabs`: `if moved.len() == slots.len()` becomes `if false`: fails at the first strip assertion after the reorder.

#[test]
fn a_hidden_tab_stays_off_the_strip_and_the_counts_and_the_order_still_holds() {
    let rig = Rig::new();
    rig.run(Command::SetSplitTabEnabled { id: "updates".into(), on: false });
    let counted = |c: &RustCore| c.counts().tabs.into_iter().map(|t| t.tab_id).collect::<Vec<_>>();
    assert!(!counted(&rig.core).contains(&"updates".to_string()), "a hidden tab leaves the counts");
    assert_eq!(rig.strip_ids(), v(&["primary", "all", "needs-reply", "promotions"]), "and the strip");
    rig.run(Command::ReorderSplitTabs { ids: v(&["primary", "promotions", "updates", "needs-reply", "social", "vips"]) });
    assert_eq!(rig.strip_ids(), v(&["primary", "all", "promotions", "needs-reply"]), "the hidden tab is not drawn though it has a place in the order");
    rig.run(Command::SetSplitTabEnabled { id: "updates".into(), on: true });
    assert_eq!(rig.strip_ids(), v(&["primary", "all", "promotions", "updates", "needs-reply"]), "shown again, it comes back where the order put it");
}
// broken copy: store/src/app_settings.rs `split_tab_set_enabled`: the `UPDATE label SET pinned` line is removed (a hidden tab stays on the strip): fails at the first strip assertion.

#[test]
fn the_refusals_of_an_order_have_their_sentences_and_write_nothing() {
    let rig = Rig::new();
    let before = rig.core.store().split_tab_rows().unwrap();
    // a view moved: Primary not first
    let moved = v(&["needs-reply", "primary", "updates", "promotions", "social", "vips"]);
    assert_eq!(refusal(rig.core.run(Command::ReorderSplitTabs { ids: moved })), ("view_not_a_label".to_string(), VIEWS_FIRST.to_string()));
    let last = v(&["needs-reply", "updates", "promotions", "social", "vips", "primary"]);
    assert_eq!(refusal(rig.core.run(Command::ReorderSplitTabs { ids: last })), ("view_not_a_label".to_string(), VIEWS_FIRST.to_string()));
    // All is a view too: after a label, or before Primary
    let all_late = v(&["primary", "needs-reply", "all", "updates", "promotions", "social", "vips"]);
    assert_eq!(refusal(rig.core.run(Command::ReorderSplitTabs { ids: all_late })).1, "All is a view of the inbox, not a label, so it cannot be reordered: the views stay first.");
    assert!(check_view_command(Command::ReorderSplitTabs { ids: v(&["all", "primary", "updates"]) }).is_err(), "the views in the wrong order are a move");
    // a tab twice (another missing), a tab missing, a tab that does not exist
    assert_eq!(refusal(rig.core.run(Command::ReorderSplitTabs { ids: v(&["primary", "updates", "updates", "promotions", "social", "vips"]) })), ("ids".to_string(), NOT_ONCE.to_string()));
    assert_eq!(refusal(rig.core.run(Command::ReorderSplitTabs { ids: v(&["primary", "primary", "needs-reply", "updates", "promotions", "social"]) })), ("ids".to_string(), NOT_ONCE.to_string()));
    assert_eq!(refusal(rig.core.run(Command::ReorderSplitTabs { ids: v(&["primary", "needs-reply", "updates", "promotions", "social"]) })), ("ids".to_string(), NOT_ONCE.to_string()));
    assert_eq!(refusal(rig.core.run(Command::ReorderSplitTabs { ids: v(&["primary", "needs-reply", "updates", "promotions", "social", "nope"]) })), ("ids".to_string(), NOT_ONCE.to_string()));
    assert_eq!(rig.core.store().split_tab_rows().unwrap(), before, "no refusal wrote anything");
}
// broken copy: contracts/src/inbox_views.rs `check_tab_order`: `if !leading || p < last` becomes `if p < last` (a view after a label is let by): fails at the `moved`-last case (the store then answers "ids").
// broken copy: contracts/src/inbox_views.rs `check_tab_order`: `if !leading || p < last` becomes `if !leading` (views in the wrong order let by): fails at the `all, primary, updates` assertion.
// broken copy: store/src/app_settings.rs `split_tab_reorder`: the `want != have` test is removed: fails at the first `NOT_ONCE` case.

#[test]
fn a_renamed_tab_shows_its_new_name_on_the_strip_and_a_restore_gives_the_old_one_back() {
    let rig = Rig::new();
    assert_eq!(rig.strip()[3], ("updates".to_string(), "Updates".to_string()));
    rig.run(Command::RenameSplitTab { tab_id: "updates".into(), name: "  Receipts ".into() });
    assert_eq!(rig.state_of("split_tab/updates/name"), Some("\"Receipts\"".to_string()));
    assert_eq!(rig.strip()[3], ("updates".to_string(), "Receipts".to_string()), "the strip row has the new name");
    assert_eq!(rig.strip()[2], ("needs-reply".to_string(), "Needs Reply".to_string()), "a tab nobody renamed keeps its name");
    assert_eq!(rig.strip()[0], ("primary".to_string(), "Primary".to_string()), "a view keeps its name");
    let rig = rig.restart();
    assert_eq!(rig.strip()[3], ("updates".to_string(), "Receipts".to_string()), "the name is on disk");
    rig.run(Command::RestoreDefaultSplitTabs);
    assert_eq!(rig.strip()[3], ("updates".to_string(), "Updates".to_string()), "restore: the shipped name");
}
// broken copy: engine/src/reads.rs `inbox/tabs`: `t.name = r.tab.name.clone();` is removed: fails at "the strip row has the new name".
// broken copy: engine/src/reads.rs `inbox/tabs`: `&& r.updated_at_ms > 0` is removed: no failure here by design (a shipped name equals the label's name), the test does not claim it.

impl Rig {
    fn state_of(&self, path: &str) -> Option<String> { self.core.state(path.to_string()) }
}
