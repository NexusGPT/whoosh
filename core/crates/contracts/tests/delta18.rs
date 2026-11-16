//! Delta 18 probes (docs/lanes/contracts.md "Delta 18"): the mail import contract, and the state words and rules for mute, unsubscribe and
//! remind-if-no-reply. Written uncompiled; never run. Each test names the rule it holds.
//! MUTANTS (the lead runs each at the build word; the test named must go red):
//!   M1  `import_source_id` ignores the location (two files share one id)                       -> a_source_id_is_stable_per_kind_and_location
//!   M2  `import_source_id` ignores the kind                                                     -> a_source_id_is_stable_per_kind_and_location
//!   M3  `check_import_start` lets a not-readable source through                                 -> a_start_is_refused_for_every_rule_in_its_order
//!   M4  `check_import_start` skips the account check for `ImportInto::Account`                  -> a_start_is_refused_for_every_rule_in_its_order
//!   M5  `check_import_start` lets a second run start                                            -> a_start_is_refused_for_every_rule_in_its_order
//!   M6  `check_import_start` checks "already running" before "unknown source"                   -> a_start_is_refused_for_every_rule_in_its_order
//!   M7  `import_access_words(NeedsFullDiskAccess)` loses the Skip sentence                       -> full_disk_access_is_refused_in_the_words_the_step_shows
//!   M8  `import_access_refusal(Readable)` returns a refusal                                     -> full_disk_access_is_refused_in_the_words_the_step_shows
//!   M9  `import_progress_fraction` divides by zero (total 0 gives Some) or lets done pass 1.0     -> the_bar_is_derived_once_and_never_guessed
//!   M10 `check_import_cancel` accepts a finished run                                            -> cancel_needs_a_running_run
//!   M11 `import_kind_of_pick` accepts `.txt`, or ignores case wrongly                           -> a_picked_file_is_an_olm_or_an_mbox_by_its_extension
//!   M12 `import_result_adds_up` ignores `already_there`                                         -> a_result_adds_up_and_never_counts_a_message_twice
//!   M13 `import_progress_is_legal` lets a Refused run carry no refusal                          -> a_progress_record_is_legal_only_when_its_refusal_matches_its_state
//!   M14 `import_run_is_over(Cancelled)` is false                                                -> a_run_is_over_in_every_state_but_running
//!   M15 `unsubscribe_state` lets a method win over `done`                                       -> unsubscribe_state_is_derived_and_done_wins
//!   M16 `unsubscribe_state` calls a mailto "link"                                               -> unsubscribe_state_is_derived_and_done_wins
//!   M17 `check_unsubscribe` lets a done one through                                             -> unsubscribe_is_refused_for_a_non_list_and_a_second_time
//!   M18 `arrival_in_thread(true)` leaves the mail unread or in the inbox                         -> a_muted_thread_receives_its_mail_read_and_out_of_the_inbox
//!   M19 `reply_reminder_state` counts a reply AFTER `at_ms` as answered                          -> a_reminder_fires_only_when_no_reply_came_by_its_time
//!   M20 `reply_reminder_state` makes a reply exactly at `at_ms` too late                         -> a_reminder_fires_only_when_no_reply_came_by_its_time
//!   M21 `reply_reminder_state` shows Waiting at `now == at_ms`                                  -> a_reminder_fires_only_when_no_reply_came_by_its_time
//!   M22 `reply_reminder_in_view` puts a Waiting or Answered reminder in the view                 -> only_a_fired_reminder_is_in_the_reminders_view
//!   M23 `check_remind_if_no_reply` lets `at_ms == now_ms` through                               -> a_reminder_in_the_past_is_refused
//!   M24 `check_clear_reply_reminder` refuses a fired reminder                                   -> clearing_needs_a_reminder_and_dismisses_a_fired_one
//!   M25 `mailbox_key(Reminders)` is "reminder" or collides with another key                      -> the_reminders_view_has_its_own_key
//!   M27 `import_store_failed_refusal` carries another code or empty words                         -> a_failed_run_says_why_in_one_place
//!   M28 `imported_row_is_legal` lets an imported row have no source or a synced row have one       -> the_stored_row_rule_is_the_same_as_the_trigger
//!   M26 the state keys of `unsubscribe_state_key` or `reply_reminder_state_key` drift             -> the_state_words_are_the_ones_the_screens_read
use contracts::*;

fn src(access: ImportAccess) -> ImportSource {
    ImportSource { id: import_source_id(ImportSourceKind::Olm, "~/Documents/a.olm".into()), kind: ImportSourceKind::Olm, name: "a.olm".into(),
        location: "~/Documents/a.olm".into(), message_count: Some(10), imported_count: 0, last_run_ms: None, access }
}
fn code(r: Result<(), CoreError>) -> String { match r { Err(CoreError::Invalid { field, .. }) => field, other => panic!("expected an Invalid refusal, got {other:?}") } }
fn progress(state: ImportRunState, done: u64, stored: u64, skipped: u64, refusal: Option<ImportRefusal>) -> ImportRunProgress {
    ImportRunProgress { run_id: "run-1".into(), source_id: "olm:x".into(), state, done, total: Some(100), current: "Inbox".into(), stored, skipped, refusal }
}

#[test]
fn a_source_id_is_stable_per_kind_and_location() {
    let a = import_source_id(ImportSourceKind::Olm, "~/a.olm".into());
    assert_eq!(a, import_source_id(ImportSourceKind::Olm, "  ~/a.olm ".into()), "a scan twice finds the same row");
    assert_ne!(a, import_source_id(ImportSourceKind::Olm, "~/b.olm".into()), "another file is another source");
    assert_ne!(a, import_source_id(ImportSourceKind::Mbox, "~/a.olm".into()), "the kind is part of the id");
    assert_eq!(import_source_id(ImportSourceKind::AppleMail, "~/Library/Mail".into()), "apple-mail:~/Library/Mail");
}

#[test]
fn a_start_is_refused_for_every_rule_in_its_order() {
    let accounts = vec!["acc-1".to_string()];
    let ok = || check_import_start(Some(src(ImportAccess::Readable)), ImportInto::Imported, accounts.clone(), None);
    assert!(ok().is_ok());
    assert!(check_import_start(Some(src(ImportAccess::Readable)), ImportInto::Account { account_id: "acc-1".into() }, accounts.clone(), None).is_ok());
    assert_eq!(code(check_import_start(None, ImportInto::Imported, accounts.clone(), None)), IMPORT_UNKNOWN_SOURCE_CODE);
    assert_eq!(code(check_import_start(Some(src(ImportAccess::NeedsFullDiskAccess)), ImportInto::Imported, accounts.clone(), None)), IMPORT_FULL_DISK_ACCESS_CODE);
    assert_eq!(code(check_import_start(Some(src(ImportAccess::NotFound)), ImportInto::Imported, accounts.clone(), None)), IMPORT_NOT_FOUND_CODE);
    assert_eq!(code(check_import_start(Some(src(ImportAccess::Unreadable)), ImportInto::Imported, accounts.clone(), None)), IMPORT_UNREADABLE_CODE);
    assert_eq!(code(check_import_start(Some(src(ImportAccess::Readable)), ImportInto::Account { account_id: "acc-9".into() }, accounts.clone(), None)), IMPORT_BAD_TARGET_CODE);
    assert_eq!(code(check_import_start(Some(src(ImportAccess::Readable)), ImportInto::Imported, accounts.clone(), Some("run-0".into()))), IMPORT_ALREADY_RUNNING_CODE);
    // order: an unknown source is named before a running import, an unreadable source before a bad target
    assert_eq!(code(check_import_start(None, ImportInto::Imported, accounts.clone(), Some("run-0".into()))), IMPORT_UNKNOWN_SOURCE_CODE);
    assert_eq!(code(check_import_start(Some(src(ImportAccess::NotFound)), ImportInto::Account { account_id: "acc-9".into() }, accounts, Some("run-0".into()))), IMPORT_NOT_FOUND_CODE);
}

#[test]
fn full_disk_access_is_refused_in_the_words_the_step_shows() {
    let words = import_access_words(ImportAccess::NeedsFullDiskAccess);
    for part in ["Full Disk Access", "System Settings", "Privacy & Security", "skip this step", "Settings, Accounts"] {
        assert!(words.contains(part), "the words name {part}: {words}");
    }
    assert_eq!(import_access_words(ImportAccess::Readable), "", "a readable source has nothing to say");
    assert_eq!(import_access_refusal(ImportAccess::Readable), None);
    let r = import_access_refusal(ImportAccess::NeedsFullDiskAccess).expect("a refusal");
    assert_eq!((r.code.as_str(), r.words.as_str()), (IMPORT_FULL_DISK_ACCESS_CODE, words.as_str()), "code and words come from one place");
    assert_eq!(import_access_code(ImportAccess::NotFound).as_deref(), Some(IMPORT_NOT_FOUND_CODE));
}

#[test]
fn the_bar_is_derived_once_and_never_guessed() {
    assert_eq!(import_progress_fraction(50, Some(200)), Some(0.25));
    assert_eq!(import_progress_fraction(0, Some(200)), Some(0.0));
    assert_eq!(import_progress_fraction(300, Some(200)), Some(1.0), "a total is an estimate: done past it is full, never more");
    assert_eq!(import_progress_fraction(5, Some(0)), None, "a zero total is not a bar");
    assert_eq!(import_progress_fraction(5, None), None, "an mbox has no total until it is read: the bar is indeterminate");
}

#[test]
fn cancel_needs_a_running_run() {
    assert!(check_import_cancel(Some(progress(ImportRunState::Running, 5, 5, 0, None))).is_ok());
    for s in [ImportRunState::Finished, ImportRunState::Cancelled] {
        assert_eq!(code(check_import_cancel(Some(progress(s, 5, 5, 0, None)))), IMPORT_NOT_RUNNING_CODE);
    }
    assert_eq!(code(check_import_cancel(None)), IMPORT_NOT_RUNNING_CODE, "an unknown run is not running");
}

#[test]
fn a_picked_file_is_an_olm_or_an_mbox_by_its_extension() {
    assert_eq!(import_kind_of_pick("~/Documents/Outlook.OLM".into()), Ok(ImportSourceKind::Olm));
    assert_eq!(import_kind_of_pick("/x/Inbox.mbox".into()), Ok(ImportSourceKind::Mbox));
    assert_eq!(import_kind_of_pick("/x/Inbox.MBX".into()), Ok(ImportSourceKind::Mbox));
    for bad in ["/x/notes.txt", "/x/mail", "/x/archive.olm.zip", ""] {
        match import_kind_of_pick(bad.into()) { Err(CoreError::Invalid { field, .. }) => assert_eq!(field, IMPORT_BAD_PICK_CODE, "{bad}"), other => panic!("{bad}: {other:?}") }
    }
}

#[test]
fn a_result_adds_up_and_never_counts_a_message_twice() {
    let mut r = ImportResult { run_id: "r".into(), source_id: "s".into(), state: ImportRunState::Finished, seen: 10, stored: 6, already_there: 3,
        skipped: vec![ImportSkipped { origin: "a.emlx".into(), words: "too large".into() }], per_account: vec![ImportPerAccount { account_id: "acc-1".into(), stored: 4 }, ImportPerAccount { account_id: "imported".into(), stored: 2 }], refusal: None };
    assert!(import_result_adds_up(r.clone()));
    r.already_there = 2;
    assert!(!import_result_adds_up(r.clone()), "10 seen is not 6 + 2 + 1");
    r.already_there = 3; r.per_account[1].stored = 1;
    assert!(!import_result_adds_up(r), "the per-account split must equal what was stored");
}

#[test]
fn a_progress_record_is_legal_only_when_its_refusal_matches_its_state() {
    let why = Some(ImportRefusal { code: IMPORT_FULL_DISK_ACCESS_CODE.into(), words: import_access_words(ImportAccess::NeedsFullDiskAccess) });
    assert!(import_progress_is_legal(progress(ImportRunState::Running, 10, 7, 3, None)));
    assert!(import_progress_is_legal(progress(ImportRunState::Refused, 0, 0, 0, why.clone())));
    assert!(import_progress_is_legal(progress(ImportRunState::Failed, 4, 4, 0, why.clone())));
    assert!(!import_progress_is_legal(progress(ImportRunState::Refused, 0, 0, 0, None)), "a refused run says why");
    assert!(!import_progress_is_legal(progress(ImportRunState::Running, 0, 0, 0, why)), "a running run has no refusal");
    assert!(!import_progress_is_legal(progress(ImportRunState::Running, 5, 4, 3, None)), "stored + skipped cannot pass done");
}

#[test]
fn a_run_is_over_in_every_state_but_running() {
    assert!(!import_run_is_over(ImportRunState::Running));
    for s in [ImportRunState::Finished, ImportRunState::Cancelled, ImportRunState::Refused, ImportRunState::Failed] { assert!(import_run_is_over(s), "{s:?}"); }
}

fn one_click() -> UnsubscribeMethod { UnsubscribeMethod::OneClick { url: "https://list.example/u".into() } }
fn mail() -> UnsubscribeMethod { UnsubscribeMethod::Mail { address: "unsub@list.example".into(), subject: None } }
fn link() -> UnsubscribeMethod { UnsubscribeMethod::Link { url: "https://list.example/page".into() } }

#[test]
fn unsubscribe_state_is_derived_and_done_wins() {
    assert_eq!(unsubscribe_state(None, false), UnsubscribeState::NoList);
    assert_eq!(unsubscribe_state(Some(one_click()), false), UnsubscribeState::OneClick);
    assert_eq!(unsubscribe_state(Some(mail()), false), UnsubscribeState::Mail, "a mailto is the mail, not a page");
    assert_eq!(unsubscribe_state(Some(link()), false), UnsubscribeState::Link);
    for m in [None, Some(one_click()), Some(mail()), Some(link())] { assert_eq!(unsubscribe_state(m, true), UnsubscribeState::Done, "once left, the offer is gone"); }
}

#[test]
fn unsubscribe_is_refused_for_a_non_list_and_a_second_time() {
    assert_eq!(check_unsubscribe(Some(one_click()), false), Ok(one_click()));
    match check_unsubscribe(None, false) { Err(CoreError::Invalid { field, .. }) => assert_eq!(field, UNSUBSCRIBE_NOT_A_LIST_CODE), o => panic!("{o:?}") }
    match check_unsubscribe(Some(link()), true) { Err(CoreError::Invalid { field, reason_words }) => { assert_eq!(field, UNSUBSCRIBE_ALREADY_DONE_CODE); assert_eq!(reason_words, "You already left this list."); } o => panic!("{o:?}") }
    // a message with no list AND marked done is "already done": the done mark is the first fact
    match check_unsubscribe(None, true) { Err(CoreError::Invalid { field, .. }) => assert_eq!(field, UNSUBSCRIBE_ALREADY_DONE_CODE), o => panic!("{o:?}") }
}

#[test]
fn a_muted_thread_receives_its_mail_read_and_out_of_the_inbox() {
    assert_eq!(arrival_in_thread(true), ArrivalTreatment { mark_read: true, in_inbox: false });
    assert_eq!(arrival_in_thread(false), ArrivalTreatment { mark_read: false, in_inbox: true }, "an unmuted thread arrives as it always did");
}

#[test]
fn a_reminder_fires_only_when_no_reply_came_by_its_time() {
    let at = 1_000;
    assert_eq!(reply_reminder_state(None, None, 5_000), ReplyReminderState::NoReminder, "no time set, no reminder");
    assert_eq!(reply_reminder_state(Some(at), None, 999), ReplyReminderState::Waiting { at_ms: at });
    assert_eq!(reply_reminder_state(Some(at), None, 1_000), ReplyReminderState::Fired { at_ms: at }, "at its time it has fired");
    assert_eq!(reply_reminder_state(Some(at), None, 9_999), ReplyReminderState::Fired { at_ms: at });
    assert_eq!(reply_reminder_state(Some(at), Some(500), 999), ReplyReminderState::Answered { at_ms: at }, "a reply before its time ends it early");
    assert_eq!(reply_reminder_state(Some(at), Some(500), 9_999), ReplyReminderState::Answered { at_ms: at }, "and it never fires after");
    assert_eq!(reply_reminder_state(Some(at), Some(1_000), 9_999), ReplyReminderState::Answered { at_ms: at }, "a reply AT its time was in time");
    assert_eq!(reply_reminder_state(Some(at), Some(1_001), 9_999), ReplyReminderState::Fired { at_ms: at }, "a reply after it does not unfire it");
}

#[test]
fn only_a_fired_reminder_is_in_the_reminders_view() {
    assert!(reply_reminder_in_view(ReplyReminderState::Fired { at_ms: 1 }));
    for s in [ReplyReminderState::NoReminder, ReplyReminderState::Waiting { at_ms: 1 }, ReplyReminderState::Answered { at_ms: 1 }] { assert!(!reply_reminder_in_view(s), "{s:?}"); }
}

#[test]
fn a_reminder_in_the_past_is_refused() {
    assert!(check_remind_if_no_reply(2_000, 1_000).is_ok());
    for at in [1_000, 999, 0, -5] {
        match check_remind_if_no_reply(at, 1_000) { Err(CoreError::Invalid { field, .. }) => assert_eq!(field, REMINDER_IN_THE_PAST_CODE, "{at}"), o => panic!("{at}: {o:?}") }
    }
}

#[test]
fn clearing_needs_a_reminder_and_dismisses_a_fired_one() {
    for s in [ReplyReminderState::Waiting { at_ms: 1 }, ReplyReminderState::Fired { at_ms: 1 }, ReplyReminderState::Answered { at_ms: 1 }] { assert!(check_clear_reply_reminder(s).is_ok(), "{s:?}"); }
    match check_clear_reply_reminder(ReplyReminderState::NoReminder) { Err(CoreError::Invalid { field, .. }) => assert_eq!(field, REMINDER_NOTHING_TO_CLEAR_CODE), o => panic!("{o:?}") }
}

#[test]
fn the_reminders_view_has_its_own_key() {
    assert_eq!(mailbox_key(Mailbox::Reminders), "reminders");
    let all = [Mailbox::Outbox, Mailbox::Vips, Mailbox::Pinned, Mailbox::Snoozed, Mailbox::Drafts, Mailbox::Sent, Mailbox::Junk, Mailbox::Trash, Mailbox::Archive, Mailbox::Starred, Mailbox::Muted, Mailbox::AllMail, Mailbox::Reminders];
    let keys: std::collections::HashSet<String> = all.iter().map(|m| mailbox_key(*m)).collect();
    assert_eq!(keys.len(), all.len(), "no two mailboxes share a key");
}

#[test]
fn the_state_words_are_the_ones_the_screens_read() {
    let words: Vec<String> = [UnsubscribeState::NoList, UnsubscribeState::OneClick, UnsubscribeState::Mail, UnsubscribeState::Link, UnsubscribeState::Done].into_iter().map(unsubscribe_state_key).collect();
    assert_eq!(words, ["none", "one_click", "mail", "link", "done"]);
    let r: Vec<String> = [ReplyReminderState::NoReminder, ReplyReminderState::Waiting { at_ms: 1 }, ReplyReminderState::Fired { at_ms: 1 }, ReplyReminderState::Answered { at_ms: 1 }].into_iter().map(reply_reminder_state_key).collect();
    assert_eq!(r, ["none", "waiting", "fired", "answered"]);
    assert_eq!(import_run_state_key(ImportRunState::Refused), "refused");
    assert_eq!(import_access_key(ImportAccess::NeedsFullDiskAccess), "needs_full_disk_access");
    assert_eq!(import_source_kind_key(ImportSourceKind::AppleMail), "apple-mail");
}

#[test]
fn a_failed_run_says_why_in_one_place() {
    let r = import_store_failed_refusal();
    assert_eq!(r.code, IMPORT_STORE_FAILED_CODE);
    assert!(r.words.contains("local database"), "{}", r.words);
    assert!(import_progress_is_legal(progress(ImportRunState::Failed, 3, 3, 0, Some(r))), "the refusal fits a Failed run");
}

#[test]
fn the_stored_row_rule_is_the_same_as_the_trigger() {
    assert!(imported_row_is_legal(true, Some("olm:x".into()), Some(5)));
    assert!(imported_row_is_legal(false, None, None));
    assert!(!imported_row_is_legal(true, None, Some(5)), "imported with no source");
    assert!(!imported_row_is_legal(true, Some("olm:x".into()), None), "imported with no time");
    assert!(!imported_row_is_legal(true, Some("  ".into()), Some(5)), "a blank source");
    assert!(!imported_row_is_legal(false, Some("olm:x".into()), None), "synced with a source");
    assert!(!imported_row_is_legal(false, None, Some(5)), "synced with a time");
}
