//! engine-act-r4: which message a SECOND reply answers. After the first reply is sent, the thread's newest message by time is the owner's
//! own sent copy, which has no place on the mail server yet (no `message_location` row, no remote id). `parent_of` must pick the newest
//! message that HAS a place, so the reply is not refused; In-Reply-To and References then name that server message (the headers are read
//! from the server for the parent). Each test was seen red with the place filter removed from `parent_of`'s query (the logs are in
//! _captures/engine-act-r4/).
mod support;

use outbox::DeviceRole;
use send::ParentHeaders;
use support::*;

const T0: i64 = EIGHT - 3 * 60 * MINUTE;

fn device() -> (FakeSmtp, Device) {
    let smtp = FakeSmtp::start(Mode::Accept);
    let d = Device::new("mac", DeviceRole::Primary, T0, smtp.port, None);
    (smtp, d)
}
fn headers_of_c3() -> ParentHeaders {
    ParentHeaders { message_id: "<c-3@linea-example.com>".into(), references: vec!["<a-1@linea-example.com>".into()] }
}
/// The owner's own copy of the first reply: newest by time, stored locally, no remote id and no location row.
fn put_local_sent_copy(d: &Device, thread: &str, at: i64) {
    let t = thread.to_string();
    d.store
        .write(move |ctx| {
            ctx.tx.execute(
                "INSERT INTO message (id, thread_id, account_id, remote_id, sender_name, from_address, subject, received_at_ms, folder) \
                 VALUES (?1, ?2, ?3, NULL, 'Me', ?4, 'Re: Quote for the Q1 print run', ?5, 'Sent')",
                rusqlite::params![format!("{t}-sent"), t, ACCOUNT_ID, ADDRESS, at],
            )?;
            Ok(())
        })
        .expect("insert the local sent copy");
}

#[test]
fn a_second_reply_answers_the_newest_message_the_server_holds_not_the_local_sent_copy() {
    let (_smtp, d) = device();
    d.put_parent("thr-1", "INBOX", 42, headers_of_c3());
    put_local_sent_copy(&d, "thr-1", 5000);
    d.put_draft_in("draft-2", "Thanks, one more thing.", Some("thr-1"));
    d.outbox.send_draft("draft-2").expect("a second reply is queued, not refused");
    let asked = d.parents.asked.lock().unwrap().clone();
    assert_eq!((asked.len(), asked[0].folder.as_str(), asked[0].remote_id.as_str()), (1, "INBOX", "42"), "the server was asked for the located message: {asked:?}");
}
// mutant "parent-of-ignores-place": core/crates/outbox/src/adapters/reply/mod.rs, the string of `let placed = "AND ((m.remote_id IS NOT NULL ..."` is emptied (`let placed = "";`).

#[test]
fn a_message_with_only_a_location_row_counts_as_placed_and_its_newest_copy_is_asked() {
    let (_smtp, d) = device();
    d.put_parent("thr-1", "INBOX", 42, headers_of_c3());
    // the synced parent has no remote id of its own: it sits under two label folders (Gmail over IMAP)
    d.store
        .write(|ctx| {
            ctx.tx.execute("UPDATE message SET remote_id = NULL WHERE id = 'thr-1-m1'", [])?;
            ctx.tx.execute("INSERT INTO message_location (account_id, folder, uid_validity, uid, message_id) VALUES (?1, 'Clients', 1, 7, 'thr-1-m1')", [ACCOUNT_ID])?;
            ctx.tx.execute("INSERT INTO message_location (account_id, folder, uid_validity, uid, message_id) VALUES (?1, 'Quotes', 1, 9, 'thr-1-m1')", [ACCOUNT_ID])?;
            Ok(())
        })
        .unwrap();
    put_local_sent_copy(&d, "thr-1", 5000);
    d.put_draft_in("draft-2", "Thanks.", Some("thr-1"));
    d.outbox.send_draft("draft-2").expect("queued");
    let asked = d.parents.asked.lock().unwrap().clone();
    assert_eq!((asked.len(), asked[0].folder.as_str(), asked[0].remote_id.as_str()), (1, "Quotes", "9"), "the location with the highest uid: {asked:?}");
}

#[test]
fn of_two_placed_messages_the_newest_is_answered() {
    let (_smtp, d) = device();
    d.put_parent("thr-1", "INBOX", 42, headers_of_c3());
    d.store
        .write(|ctx| {
            ctx.tx.execute(
                "INSERT INTO message (id, thread_id, account_id, remote_id, sender_name, from_address, subject, received_at_ms, folder) \
                 VALUES ('thr-1-m2', 'thr-1', ?1, '77', 'Linea Supply', 'orders@linea-example.com', 'Re: Quote', 3000, 'INBOX')",
                [ACCOUNT_ID],
            )?;
            Ok(())
        })
        .unwrap();
    put_local_sent_copy(&d, "thr-1", 5000);
    d.put_draft_in("draft-2", "Thanks.", Some("thr-1"));
    d.outbox.send_draft("draft-2").expect("queued");
    let asked = d.parents.asked.lock().unwrap().clone();
    assert_eq!((asked[0].folder.as_str(), asked[0].remote_id.as_str()), ("INBOX", "77"), "{asked:?}");
}

#[test]
fn a_thread_with_only_local_messages_is_still_refused_in_words() {
    let (_smtp, d) = device();
    d.put_parent("thr-1", "INBOX", 42, headers_of_c3());
    d.store.write(|ctx| { ctx.tx.execute("UPDATE message SET remote_id = NULL WHERE id = 'thr-1-m1'", [])?; Ok(()) }).unwrap();
    put_local_sent_copy(&d, "thr-1", 5000);
    d.put_draft_in("draft-2", "Thanks.", Some("thr-1"));
    let err = d.outbox.send_draft("draft-2").expect_err("no message of the thread has a place on the server");
    assert!(err.to_string().contains("no place on the mail server yet"), "{err}");
    assert!(d.parents.asked.lock().unwrap().is_empty(), "the server was not asked");
}
