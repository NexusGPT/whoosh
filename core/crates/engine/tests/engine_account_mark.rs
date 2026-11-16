//! gap-engine-state, item 33270f65 (the account mark on every surface), the engine half the shell's surfaces draw from: the real core serves each account's colour
//! ("#RRGGBB", the store's palette in account order) and icon id (`None` until the owner picks one) through `WhooshCore::accounts()`, the one list the app's
//! AccountBook reads for the list row, the sidebar ring, the reader meta, the search result and the notch; every message names an account that list holds. A core that
//! served the same colour for two accounts, an unstable value between two reads, or a message of an account the list does not hold is red.
//! Each case prints one `PASS <case>` line (`-- --nocapture`).
//!
//! Broken copies (single edits in core/crates/engine/src/rust_core.rs `accounts`, each red in the named test):
//! - K1 `for a in &mut out { a.health = ...; }` also sets `a.colour = "#007AFF".into();` (the system blue for every account): `every_account_serves_its_own_colour_and_a_stable_icon`
//! - K2 the same loop sets `a.icon_id = Some("envelope".into());`: `every_account_serves_its_own_colour_and_a_stable_icon` (the owner picked no icon, the core must not invent one)
//! - K3 the same loop appends `a.id.push('x');` (the list names ids no message carries): `every_message_belongs_to_an_account_the_list_serves`
mod first_run_support;

use contracts::WhooshCore;
use first_run_support::*;

fn is_hex_colour(c: &str) -> bool { c.len() == 7 && c.starts_with('#') && c[1..].chars().all(|ch| ch.is_ascii_hexdigit()) }

#[test]
fn every_account_serves_its_own_colour_and_a_stable_icon() {
    let rig = Rig::first_sync(3);
    let first = rig.core.accounts();
    assert_eq!(first.len(), 2, "the rig adds two accounts");
    for a in &first {
        assert!(is_hex_colour(&a.colour), "{} serves the colour {:?}, not #RRGGBB", a.address, a.colour);
        assert_ne!(a.colour.to_ascii_uppercase(), "#007AFF", "{} serves the system blue, not its own colour", a.address);
        assert_eq!(a.icon_id, None, "{} serves the icon {:?} though the owner picked none", a.address, a.icon_id);
    }
    assert_ne!(first[0].colour, first[1].colour, "two accounts serve the same colour {}", first[0].colour);
    let again = rig.core.accounts();
    let pairs = |v: &[contracts::AccountSummary]| v.iter().map(|a| (a.id.clone(), a.colour.clone(), a.icon_id.clone())).collect::<Vec<_>>();
    assert_eq!(pairs(&first), pairs(&again), "a second read serves other marks");
    println!("PASS accounts-serve-own-colour-and-no-invented-icon ({} accounts: {} and {})", first.len(), first[0].colour, first[1].colour);
}

#[test]
fn every_message_belongs_to_an_account_the_list_serves() {
    let rig = Rig::first_sync(3);
    let ids: Vec<String> = rig.core.accounts().into_iter().map(|a| a.id).collect();
    let query = contracts::InboxQuery { tab_id: "all".into(), account_id: None, label_id: None, mailbox: None, account_ids: vec![], filters: vec![], smart_folder_id: None };
    let rows = rig.core.inbox(query);
    assert!(!rows.is_empty(), "the first account holds three messages");
    for r in &rows {
        let m = rig.core.message(r.message_id.clone()).unwrap_or_else(|| panic!("{} is listed and has no message", r.message_id));
        assert!(ids.contains(&m.account_id), "{} names the account {} which the accounts list does not hold", r.message_id, m.account_id);
    }
    println!("PASS every-listed-message-names-a-served-account ({} messages)", rows.len());
}
