//! The small typed ops the actions use cases switched to: pin time, snooze time, the list-header flag, a draft from a sent
//! message, and contacts with their recounted mail counts.
mod common;
use common::*;

#[test]
fn pin_and_snooze_times_are_read_replaced_and_lifted_with_the_counts_kept_equal_to_a_recount() {
    let db = TempDb::new("small-pin");
    let store = small_store(&db, 12);
    store.counts().unwrap();
    store
        .write(|ctx| {
            assert_eq!(ctx.pinned_at("m000")?, None);
            ctx.set_pinned_at("m000", Some(10))?;
            ctx.set_pinned_at("m000", Some(20))?;
            assert_eq!(ctx.pinned_at("m000")?, Some(20), "a second pin replaces the time");
            ctx.set_snoozed_until("m006", Some(9_000_000_000_000), 1)?;
            ctx.set_snoozed_until("m006", Some(9_100_000_000_000), 2)?;
            assert_eq!(ctx.snoozed_until("m006")?, Some(9_100_000_000_000));
            Ok(())
        })
        .unwrap();
    assert_eq!(store.counts().unwrap(), store.recount().unwrap(), "the snooze left the unread tally");
    store.write(|ctx| ctx.set_snoozed_until("m006", None, 3)).unwrap();
    store.write(|ctx| ctx.set_pinned_at("m000", None)).unwrap();
    assert_eq!(store.counts().unwrap(), store.recount().unwrap());
    assert!(store.snoozed().unwrap().is_empty() && store.pinned().unwrap().is_empty());
}

#[test]
fn a_sent_message_becomes_a_draft_and_a_received_one_does_not() {
    let db = TempDb::new("small-draft");
    let store = small_store(&db, 0);
    let mut sent = msg("s1", "ts1", "acc-a", "owner0@example.test", 1_700_000_000_000);
    sent.folder = "Sent".into();
    store.insert_messages(vec![new_msg(sent, None), new_msg(msg("r1", "tr1", "acc-a", "friend@ext.test", 1_700_000_001_000), None)]).unwrap();
    let id = store.write(|ctx| ctx.draft_from_sent("s1", "draft-1", 5)).unwrap();
    assert_eq!(id, "draft-1");
    let (account, subject): (String, String) = store.write(|ctx| Ok(ctx.tx.query_row("SELECT from_account_id, subject FROM draft WHERE id = 'draft-1'", [], |r| Ok((r.get(0)?, r.get(1)?)))?)).unwrap();
    assert_eq!((account.as_str(), subject.as_str()), ("acc-a", "Subject s1"));
    let refused = store.write(|ctx| ctx.draft_from_sent("r1", "draft-2", 5)).unwrap_err();
    assert!(refused.to_string().contains("only a message you sent"), "{refused}");
    let drafts: i64 = store.write(|ctx| Ok(ctx.tx.query_row("SELECT count(*) FROM draft", [], |r| r.get(0))?)).unwrap();
    assert_eq!(drafts, 1);
    assert!(store.write(|ctx| ctx.has_list_header("s1")).unwrap() == false);
    assert!(store.write(|ctx| ctx.has_list_header("nope")).unwrap() == false);
}

#[test]
fn contacts_are_found_by_any_case_saved_with_recounted_mail_and_folded() {
    let db = TempDb::new("small-contacts");
    let store = small_store(&db, 0);
    store
        .insert_messages(vec![
            new_msg(msg("a1", "ta", "acc-a", "Ann@Ext.test", 1_700_000_000_000), None),
            new_msg(msg("a2", "tb", "acc-b", "ann@ext.test", 1_700_000_001_000), None),
            new_msg(msg("a3", "tc", "acc-a", "ann.work@ext.test", 1_700_000_002_000), None),
            new_msg(msg("o1", "td", "acc-a", "owner0@example.test", 1_700_000_003_000), None),
        ])
        .unwrap();
    let senders = store.write(|ctx| ctx.outside_senders()).unwrap();
    let mut addresses: Vec<&str> = senders.iter().map(|(a, _)| a.as_str()).collect();
    addresses.sort();
    assert_eq!(addresses, ["ann.work@ext.test", "ann@ext.test"], "the owner's own address is never a contact; case is folded");
    store
        .write(|ctx| {
            ctx.contact_save("c1", "Ann", &["ann@ext.test".to_string()], 1)?;
            ctx.contact_save("c2", "Ann (work)", &["ann.work@ext.test".to_string()], 1)?;
            assert_eq!(ctx.contact_by_address("ann@ext.test")?.unwrap().id, "c1");
            assert!(ctx.contact_by_address("nobody@ext.test")?.is_none());
            assert_eq!(ctx.sender_name_of("ann@ext.test")?.as_deref(), Some("ann@ext.test"), "the name on the newest message");
            assert_eq!(ctx.contact_message_ids("c1")?.unwrap(), ["a2", "a1"], "newest first, any case");
            assert!(ctx.contact_message_ids("ghost")?.is_none());
            let both = ["ann@ext.test".to_string(), "ann.work@ext.test".to_string()];
            ctx.contact_save("c1", "Ann", &both, 2)?;
            assert!(ctx.contact_delete("c2")?);
            assert!(!ctx.contact_delete("c2")?);
            let counts = ctx.mail_counts(&both)?;
            assert_eq!(counts.values().sum::<i64>(), 3, "the per-account counts add up to the messages from those addresses");
            assert_eq!(ctx.contacts_all()?.len(), 1);
            let stored: String = ctx.tx.query_row("SELECT mail_count_json FROM contact WHERE id = 'c1'", [], |r| r.get(0))?;
            assert_eq!(stored, r#"{"acc-a":2,"acc-b":1}"#);
            Ok(())
        })
        .unwrap();
}
