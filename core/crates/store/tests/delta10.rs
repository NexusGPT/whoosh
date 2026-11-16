//! contracts delta 10 hook: the store side of RemoveAllLabels, star and mute, List-Unsubscribe, remind-if-no-reply, the
//! attachments read and the attachment index's AttachmentRows (migration 0005).
mod common;
use attachment_index::{AttachmentIndexEntry, AttachmentKind, AttachmentRows, IndexState};
use common::*;
use contracts::*;

fn ids(v: &[&str]) -> Vec<String> { v.iter().map(|s| s.to_string()).collect() }
fn carries(s: &store::Store, label: &str, message: &str) -> bool {
    let (l, m) = (label.to_string(), message.to_string());
    s.write(move |c| Ok(c.tx.query_row("SELECT EXISTS (SELECT 1 FROM label_assignment WHERE label_id = ?1 AND message_id = ?2)", rusqlite::params![l, m], |r| r.get::<_, bool>(0))?)).unwrap()
}

#[test]
fn remove_all_labels_returns_what_it_took_and_restore_puts_it_back() {
    let db = TempDb::new("d10-unlabel");
    let s = small_store(&db, 4);
    let a = s.label_ensure_path("Clients > Provanta".into(), vec![], 1).unwrap();
    let b = s.label_ensure_path("Suppliers".into(), vec![], 1).unwrap();
    s.label_assign(a.label.id.clone(), ids(&["m000", "m001"]), 2).unwrap();
    s.label_assign(b.label.id.clone(), ids(&["m000"]), 2).unwrap();
    let removed = s.labels_remove_all(ids(&["m000"])).unwrap();
    assert_eq!(removed.len(), 2);
    assert!(!carries(&s, &a.label.id, "m000"));
    assert!(carries(&s, &a.label.id, "m001"), "another message keeps its label");
    assert_eq!(s.labels_restore(removed, 3).unwrap(), 2);
    assert!(carries(&s, &b.label.id, "m000") && carries(&s, &a.label.id, "m000"));
}

#[test]
fn star_mute_and_the_list_header_are_stored() {
    let db = TempDb::new("d10-flags");
    let s = small_store(&db, 2);
    assert_eq!(s.set_starred(ids(&["m000"]), true).unwrap(), 1);
    assert_eq!(s.set_muted(ids(&["m001"]), true).unwrap(), 1);
    let (starred, muted): (i64, i64) = s.write(|c| Ok(c.tx.query_row("SELECT (SELECT starred FROM message WHERE id='m000'), (SELECT muted FROM message WHERE id='m001')", [], |r| Ok((r.get(0)?, r.get(1)?)))?)).unwrap();
    assert_eq!((starred, muted), (1, 1));
    s.set_list_unsubscribe("m000".into(), ids(&["<https://l.example/u>", "<mailto:x@l.example>"]), true).unwrap();
    let (urls, one_click) = s.list_unsubscribe("m000").unwrap();
    assert_eq!(unsubscribe_method_from(urls, one_click), Some(UnsubscribeMethod::OneClick { url: "https://l.example/u".into() }));
    assert_eq!(s.list_unsubscribe("m001").unwrap(), (vec![], false));
    assert!(s.mark_unsubscribed("m000".into(), 5).unwrap());
    assert!(!s.mark_unsubscribed("m001".into(), 5).unwrap(), "a message from no list cannot be unsubscribed");
}

#[test]
fn a_reply_reminder_fires_only_when_no_reply_came() {
    let db = TempDb::new("d10-remind");
    let s = small_store(&db, 0);
    s.write(|c| {
        for d in ["d1", "d2"] {
            c.tx.execute("INSERT INTO draft (id, from_account_id, updated_at_ms) VALUES (?1, 'acc-a', 0)", rusqlite::params![d])?;
        }
        Ok(())
    }).unwrap();
    assert!(s.draft_remind_if_no_reply("d1".into(), 50, 100).is_err(), "a past time is refused");
    s.draft_remind_if_no_reply("d1".into(), 1_000, 100).unwrap();
    s.draft_remind_if_no_reply("d2".into(), 1_000, 100).unwrap();
    let mut sent1 = msg("s1", "t1", "acc-a", "owner0@example.test", 200); sent1.folder = "Sent".into();
    let mut sent2 = msg("s2", "t2", "acc-a", "owner0@example.test", 200); sent2.folder = "Sent".into();
    s.insert_messages(vec![new_msg(sent1, None), new_msg(sent2, None), new_msg(msg("r2", "t2", "acc-a", "them@ext.test", 500), Some("primary"))]).unwrap();
    s.reply_reminder_on_sent("d1".into(), "s1".into(), 200).unwrap();
    s.reply_reminder_on_sent("d2".into(), "s2".into(), 200).unwrap();
    assert!(s.reply_reminders_fire(900).unwrap().is_empty(), "not due yet");
    assert_eq!(s.reply_reminders_fire(1_000).unwrap(), ids(&["s1"]), "t2 got a reply, t1 did not");
    assert!(s.reply_reminders_fire(2_000).unwrap().is_empty(), "a reminder fires once");
}

#[test]
fn attachments_read_and_the_index_rows_go_through_the_store() {
    let db = TempDb::new("d10-attach");
    let s = small_store(&db, 1);
    s.write(|c| {
        c.tx.execute("INSERT INTO attachment (id, message_id, file_name, mime_type, size_bytes, local_path) VALUES ('a1','m000','quote.pdf','application/pdf',2048,'/tmp/quote.pdf')", [])?;
        c.tx.execute("INSERT INTO attachment (id, message_id, file_name, mime_type, size_bytes) VALUES ('a2','m000','plan.dwg','image/vnd.dwg',10)", [])?;
        Ok(())
    }).unwrap();
    let a = s.attachments("m000").unwrap();
    assert_eq!(a.iter().map(|x| (x.file_name.as_str(), x.size_bytes, x.local_path.is_some())).collect::<Vec<_>>(), vec![("quote.pdf", 2048, true), ("plan.dwg", 10, false)]);
    let entry = AttachmentIndexEntry {
        attachment_id: "a1".into(), message_id: "m000".into(), name: "quote.pdf".into(), kind: AttachmentKind::Pdf,
        state: IndexState::Indexed, text: Some("unit price 0.84 EUR".into()), ocr_text: None, labels: vec!["document".into()],
        transcript: None, vectors: vec![], notes: vec![], updated_at_ms: 7,
    };
    s.save(&entry).unwrap();
    let back = s.load("a1").unwrap().expect("filed");
    assert_eq!((back.text.as_deref(), back.labels.clone()), (Some("unit price 0.84 EUR"), vec!["document".to_string()]));
    let cat = s.catalog(&ids(&["m000"])).unwrap();
    assert_eq!(cat.iter().map(|c| (c.attachment_id.as_str(), c.state.clone())).collect::<Vec<_>>(), vec![("a1", IndexState::Indexed), ("a2", IndexState::Unreadable)]);
    assert!(cat[0].text.contains("0.84"));
}
