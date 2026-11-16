//! fix-realcore round 12 (main111 `--slice-crawl --paused`: "11 whole-message fetches in 20s, 8 on a folder other than INBOX; the shell's own row says
//! crawl 'running', reason '', conditions it sent 100%"; main88 the same with 68%): the hidden check puts a rig core on battery at 15 %
//! (`WHOOSH_CORE_CHECK_CONDITIONS=battery:15`), then the shell reports the Mac's own state at start (on its charger, 100 %), and that report replaced the
//! check's: the crawl never held. Now the check's conditions are pinned for the core's life: the shell's report changes nothing on that core.
//! One test (the variable is the process's own).
//! Broken copies (one edit each, red first):
//!   crawl.rs `shell_conditions`: `if self.crawl.pinned.load(Ordering::SeqCst) { return; }` -> deleted (the shell's report wins again: red)
//!   rust_core.rs `open_production`: `core.deps.bodies.pin_conditions(c);` -> `core.deps.bodies.set_conditions(c);` (not pinned: red)
use engine::bodies::crawl::{set_background_conditions, BackgroundPause};
use engine::RustCore;

#[test]
fn the_checks_battery_holds_the_crawl_whatever_the_shell_reports_after() {
    std::env::set_var("WHOOSH_CORE_CHECK_CONDITIONS", "battery:15");
    let dir = tempfile::tempdir().unwrap();
    let core = RustCore::open_production(dir.path().to_str().unwrap(), true).expect("the rig core opens");
    std::env::remove_var("WHOOSH_CORE_CHECK_CONDITIONS");
    assert_eq!(core.background().paused(), Some(BackgroundPause::Battery), "the check's battery holds the background");
    // the shell, at start and on a change: the Mac is on its charger at 100 %, not hot
    set_background_conditions(Some(100), true, false);
    assert_eq!(core.background().paused(), Some(BackgroundPause::Battery), "the shell's report does not replace the check's conditions");
    set_background_conditions(None, false, false);
    assert_eq!(core.background().paused(), Some(BackgroundPause::Battery), "nor does a desktop Mac's report");
    println!("PASS check-conditions-pinned: battery 15 % holds after two reports of the shell");
}
