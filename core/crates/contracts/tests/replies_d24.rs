//! Delta 24: a mail folder removed on the server stays as a gone row and returns with the owner's last choice.
//! MUTANTS (_captures/replies-contract/mutants.py P01..P06; each must turn the test named red):
//!   P01 a folder that goes missing is not forced off                                  -> a_folder_that_goes_remembers_the_switch_and_comes_back_with_it
//!   P02 the owner's switch is not remembered when it goes                             -> a_folder_that_goes_remembers_the_switch_and_comes_back_with_it
//!   P03 a folder that comes back stays off                                            -> a_folder_that_goes_remembers_the_switch_and_comes_back_with_it
//!   P04 a second LIST that still misses it forgets the memory                         -> a_folder_that_stays_gone_keeps_its_memory_and_the_owner_may_change_it
//!   P05 the owner's switch on a gone folder turns it on                               -> a_folder_that_stays_gone_keeps_its_memory_and_the_owner_may_change_it
//!   P07 `reply/sources` drops the owner's remembered choice (null for a gone folder)    -> reply_sources_carries_the_owners_remembered_choice_of_a_gone_folder
//!   P08 the choice is attached to the wrong source (every source gets it)             -> reply_sources_carries_the_owners_remembered_choice_of_a_gone_folder
//!   P06 the row of a gone folder loses its last count                                 -> the_row_says_gone_and_keeps_its_count
use contracts::*;

fn p(enabled: bool, owner: Option<bool>, gone: bool) -> SourcePresence { SourcePresence { enabled, owner_enabled: owner, gone } }

#[test]
fn a_folder_that_goes_remembers_the_switch_and_comes_back_with_it() {
    for chosen in [true, false] {
        let gone = source_presence_after_list(p(chosen, None, false), false);
        assert_eq!(gone, p(false, Some(chosen), true), "forced off, the owner's choice kept apart");
        assert!(source_presence_is_legal(gone));
        let back = source_presence_after_list(gone, true);
        assert_eq!(back, p(chosen, None, false), "the folder returns with the switch the owner last chose");
    }
    assert_eq!(source_presence_after_list(p(true, None, false), true), p(true, None, false), "present stays as it is");
}

#[test]
fn a_folder_that_stays_gone_keeps_its_memory_and_the_owner_may_change_it() {
    let gone = p(false, Some(true), true);
    assert_eq!(source_presence_after_list(gone, false), gone, "a second LIST that still misses it changes nothing");
    let flipped = source_presence_after_owner_switch(gone, false);
    assert_eq!(flipped, p(false, Some(false), true), "the switch of a gone row is remembered, never applied");
    assert_eq!(source_presence_after_list(flipped, true), p(false, None, false), "and the folder returns off");
    assert_eq!(source_presence_after_owner_switch(p(false, None, false), true), p(true, None, false), "a present folder just switches");
    let on_while_gone = source_presence_after_owner_switch(gone, true);
    assert!(!on_while_gone.enabled && source_presence_is_legal(on_while_gone));
    assert!(!source_presence_is_legal(p(true, Some(true), true)) && !source_presence_is_legal(p(false, Some(true), false)), "illegal shapes");
}

#[test]
fn the_row_says_gone_and_keeps_its_count() {
    let row = ReplySource { source: ReplySourceRef::MailFolder { account_id: "a".into(), folder: "Old".into() }, enabled: true, item_count: 1_840, progress: ReplySyncProgress::UpToDate };
    let g = reply_source_with_presence(row.clone(), p(false, Some(true), true));
    assert_eq!((g.enabled, g.item_count, g.progress.clone()), (false, 1_840, ReplySyncProgress::Gone), "off, last count kept, Gone");
    assert!(reply_source_is_legal(g.clone()) && !reply_source_is_legal(ReplySource { enabled: true, ..g }));
    assert_eq!(reply_source_with_presence(row.clone(), p(true, None, false)), row, "a present folder reads as the crawl reports it");
    let j: serde_json::Value = serde_json::from_str(&reply_source_json(reply_source_with_presence(row, p(false, Some(true), true)))).unwrap();
    assert_eq!(j["progress"]["state"], "gone");
    assert_eq!(j["item_count"], 1_840);
}

#[test]
fn reply_sources_carries_the_owners_remembered_choice_of_a_gone_folder() {
    let mail = |f: &str| ReplySource { source: ReplySourceRef::MailFolder { account_id: "a".into(), folder: f.into() }, enabled: false, item_count: 3, progress: ReplySyncProgress::UpToDate };
    let gone = reply_source_with_presence(mail("Old"), p(false, Some(true), true));
    let key = reply_source_key(gone.source.clone());
    let pane = reply_sources_read(vec![reply_account_sources("a".into(), vec![gone, mail("INBOX")])], vec![], vec![], RetrievalScope::ThisAccount);
    let j: serde_json::Value = serde_json::from_str(&reply_sources_json_with_choices(pane.clone(), vec![SourceChoice { source_key: key.clone(), owner_enabled: true }])).unwrap();
    let f = &j["accounts"][0]["folders"];
    assert_eq!((f[0]["owner_enabled"].clone(), f[0]["progress"]["state"].clone()), (serde_json::json!(true), serde_json::json!("gone")), "the gone folder carries what the owner last chose");
    assert!(f[1]["owner_enabled"].is_null(), "a present folder carries null");
    let off: serde_json::Value = serde_json::from_str(&reply_sources_json_with_choices(pane.clone(), vec![SourceChoice { source_key: key, owner_enabled: false }])).unwrap();
    assert_eq!(off["accounts"][0]["folders"][0]["owner_enabled"], serde_json::json!(false), "false is a value, not null");
    let plain: serde_json::Value = serde_json::from_str(&reply_sources_json(pane)).unwrap();
    assert!(plain["accounts"][0]["folders"][0]["owner_enabled"].is_null(), "the old entry point reads null everywhere");
}
