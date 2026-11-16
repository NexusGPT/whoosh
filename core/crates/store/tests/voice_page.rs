//! The voice profile, and a page of messages written together with its folder cursor.
mod common;
use common::*;
use contracts::SyncCursor;
use store::FolderProgress;

const DAY_MS: i64 = 86_400_000;

#[test]
fn a_voice_profile_is_kept_replaced_found_by_any_case_and_fresh_for_the_ledgers_days() {
    let db = TempDb::new("voice");
    let store = small_store(&db, 0);
    assert!(store.voice_profile("owner@example.test").unwrap().is_none());
    let built = 1_700_000_000_000;
    let first = store.set_voice_profile("Owner@Example.test".into(), "Short, warm, no sign-off.".into(), built, built + 5).unwrap();
    assert_eq!(first.updated_at_ms, built + 5);
    let found = store.voice_profile("owner@EXAMPLE.test").unwrap().expect("the match ignores case");
    assert_eq!((found.summary_words.as_str(), found.built_at_ms), ("Short, warm, no sign-off.", built));
    let days = constant("VOICE_REFRESH_DAYS").expect("the ledger has VOICE_REFRESH_DAYS") as i64;
    assert_eq!(store::voice_refresh_days().unwrap(), days);
    assert!(found.is_fresh(built + days * DAY_MS - 1).unwrap());
    assert!(!found.is_fresh(built + days * DAY_MS).unwrap(), "at the refresh time the profile is due");
    // A rewrite replaces; it does not add a second row, even when the address is spelled differently.
    store.set_voice_profile("owner@example.test".into(), "Longer sentences now.".into(), built + days * DAY_MS, built + days * DAY_MS).unwrap();
    let rows: i64 = store.write(|ctx| Ok(ctx.tx.query_row("SELECT count(*) FROM voice_profile", [], |r| r.get(0))?)).unwrap();
    assert_eq!(rows, 1);
    assert_eq!(store.voice_profile("OWNER@example.test").unwrap().unwrap().summary_words, "Longer sentences now.");
}

#[test]
fn a_voice_profile_with_no_words_or_no_address_is_refused_and_keeps_the_old_one() {
    let db = TempDb::new("voice-bad");
    let store = small_store(&db, 0);
    store.set_voice_profile("owner@example.test".into(), "Plain.".into(), 1, 1).unwrap();
    assert!(store.set_voice_profile("owner@example.test".into(), "   ".into(), 2, 2).unwrap_err().to_string().contains("needs words"));
    assert!(store.set_voice_profile(" ".into(), "Words".into(), 2, 2).unwrap_err().to_string().contains("needs the address"));
    assert_eq!(store.voice_profile("owner@example.test").unwrap().unwrap().summary_words, "Plain.");
}

fn folder_row(store: &store::Store, account: &str, folder: &str) -> Option<(Option<i64>, Option<i64>, Option<String>, Option<i64>, Option<String>)> {
    let (a, f) = (account.to_string(), folder.to_string());
    store
        .write(move |ctx| {
            Ok(ctx
                .tx
                .query_row(
                    "SELECT last_uid, last_full_pass_ms, cursor_kind, imap_uid_validity, cursor_value FROM folder_sync_state WHERE account_id = ?1 AND folder = ?2",
                    [&a, &f],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
                )
                .ok())
        })
        .unwrap()
}

fn indexed(store: &store::Store, account: &str) -> (i64, Option<i64>) {
    let a = account.to_string();
    store.write(move |ctx| Ok(ctx.tx.query_row("SELECT messages_indexed, last_synced_ms FROM account_sync_state WHERE account_id = ?1", [&a], |r| Ok((r.get(0)?, r.get(1)?)))?)).unwrap()
}

fn page(from: usize, to: usize) -> Vec<store::NewMessage> {
    (from..to).map(|i| new_msg(msg(&format!("p{i:03}"), &format!("pt{:03}", i / 2), "acc-a", "x@ext.test", 1_700_000_000_000 + i as i64), Some("primary"))).collect()
}

fn progress(account: &str, oldest: Option<i64>, done: bool, cursor: Option<SyncCursor>, now: i64) -> FolderProgress {
    FolderProgress { account_id: account.into(), folder: "INBOX".into(), oldest, done, cursor, now_ms: now }
}

#[test]
fn a_page_and_its_cursor_land_in_one_commit() {
    let db = TempDb::new("page");
    let store = small_store(&db, 0);
    let imap = SyncCursor::Imap { uid_validity: 7, highest_mod_seq: 99 };
    let n = store.write_page(page(0, 10), progress("acc-a", Some(41), false, Some(imap), 1000)).unwrap();
    assert_eq!(n, 10);
    assert_eq!(folder_row(&store, "acc-a", "INBOX"), Some((Some(41), None, Some("imap".into()), Some(7), None)));
    assert_eq!(indexed(&store, "acc-a"), (10, Some(1000)));
    // The next page ends the folder and hands a Graph delta token: the cursor kind is replaced, the full-pass time set.
    let n = store.write_page(page(10, 14), progress("acc-a", Some(3), true, Some(SyncCursor::GraphDelta { token: "tok-1".into() }), 2000)).unwrap();
    assert_eq!(n, 4);
    assert_eq!(folder_row(&store, "acc-a", "INBOX"), Some((Some(3), Some(2000), Some("graph_delta".into()), None, Some("tok-1".into()))));
    assert_eq!(indexed(&store, "acc-a"), (14, Some(2000)));
    // The same page again: nothing is new, the count does not grow, the fetch time still moves.
    assert_eq!(store.write_page(page(10, 14), progress("acc-a", Some(3), true, None, 3000)).unwrap(), 0);
    assert_eq!(indexed(&store, "acc-a"), (14, Some(3000)));
    assert_eq!(store.counts().unwrap(), store.recount().unwrap());
}

#[test]
fn a_page_whose_cursor_cannot_be_stored_leaves_no_message_behind() {
    let db = TempDb::new("page-atomic");
    let store = small_store(&db, 0);
    // An account the store does not know: the folder row is refused by its foreign key AFTER the messages were inserted.
    let err = store.write_page(page(0, 6), progress("acc-ghost", Some(1), false, None, 1000)).unwrap_err();
    assert!(err.to_string().contains("FOREIGN KEY"), "the real cause is kept: {err}");
    let messages: i64 = store.write(|ctx| Ok(ctx.tx.query_row("SELECT count(*) FROM message", [], |r| r.get(0))?)).unwrap();
    let threads: i64 = store.write(|ctx| Ok(ctx.tx.query_row("SELECT count(*) FROM thread", [], |r| r.get(0))?)).unwrap();
    assert_eq!((messages, threads), (0, 0), "the messages went back with the cursor");
    assert_eq!(folder_row(&store, "acc-ghost", "INBOX"), None);
}
