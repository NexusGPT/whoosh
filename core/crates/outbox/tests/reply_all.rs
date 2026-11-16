//! real-compose: a reply-all draft (To the sender and the other recipients, Cc the rest, in the thread of the mail it answers, the shape
//! `ReplyAll` in the engine saves) leaves through the outbox like any reply: queued, held for the undo window, threaded from the stored
//! headers, delivered to EVERY recipient (the envelope, not only the header lines) and sent once. The SMTP server is the send lane's fake on
//! 127.0.0.1. Not run by this lane (no cargo here): the lead runs `cargo test -p outbox --test reply_all`.
//! Broken copies (one line each): outbox/src/adapters/reply/mod.rs `stored`: `parent_references: h.references` becomes `parent_references: vec![]`
//!   -> the References assertion; `execute`: `if let Some(found) = self.stored(thread_id, kind)? {` becomes `if let Some(found) = None::<ReplyContext> {`
//!   -> the stored-headers assertions (the server port has no answer and refuses).
mod support;

use outbox::DeviceRole;
use store::ThreadHeaders;
use support::*;

const T0: i64 = EIGHT - 3 * 60 * MINUTE;
const AFTER_WINDOW: i64 = T0 + 60_000;

fn headers(data: &str) -> Vec<(String, String)> {
    let head = data.split_once("\r\n\r\n").expect("header/body split").0;
    head.replace("\r\n ", " ").replace("\r\n\t", " ").lines().filter_map(|l| l.split_once(':').map(|(n, v)| (n.to_ascii_lowercase(), v.trim().to_string()))).collect()
}
fn get(h: &[(String, String)], name: &str) -> Vec<String> { h.iter().filter(|(n, _)| n == name).map(|(_, v)| v.clone()).collect() }

fn put_parent_mail(d: &Device) {
    d.store
        .write(|ctx| {
            ctx.tx.execute("INSERT INTO thread (id, account_id, subject) VALUES ('thr-ra', ?1, 'Q1 plan')", rusqlite::params![ACCOUNT_ID])?;
            ctx.tx.execute(
                "INSERT INTO message (id, thread_id, account_id, remote_id, sender_name, from_address, subject, received_at_ms, folder) \
                 VALUES ('thr-ra-m1', 'thr-ra', ?1, NULL, 'Ann', 'ann@linea-example.com', 'Q1 plan', 1000, 'INBOX')",
                rusqlite::params![ACCOUNT_ID],
            )?;
            Ok(())
        })
        .expect("insert the mail");
    d.store.set_thread_headers_quiet(vec![("thr-ra-m1".to_string(), Some(ThreadHeaders { message_id: "<ra-2@linea-example.com>".into(), references: vec!["<ra-1@linea-example.com>".into()] }))]).expect("store the headers");
}

/// A reply-all draft as the engine's `ReplyAll` leaves it: two To, two Cc, in the thread.
fn put_reply_all_draft(d: &Device) {
    d.put_draft_in("draft-ra", "Agreed, thanks all.", Some("thr-ra"));
    d.store
        .write(|ctx| {
            ctx.tx.execute(
                "UPDATE draft SET to_json = ?1, cc_json = ?2 WHERE id = 'draft-ra'",
                rusqlite::params![r#"["ann@linea-example.com","bob@linea-example.com"]"#, r#"["cy@linea-example.com","dee@linea-example.com"]"#],
            )?;
            Ok(())
        })
        .expect("recipients");
}

#[test]
fn a_reply_all_draft_is_held_for_the_undo_window_then_delivered_to_every_recipient_threaded_and_once() {
    let smtp = FakeSmtp::start(Mode::Accept);
    let d = Device::new("mac", DeviceRole::Primary, T0, smtp.port, None);
    put_parent_mail(&d);
    put_reply_all_draft(&d);
    d.outbox.send_draft("draft-ra").expect("queued");
    assert!(smtp.received().is_empty(), "nothing leaves inside the undo window");
    d.outbox.tick().unwrap();
    assert!(smtp.received().is_empty(), "the tick inside the window sends nothing");
    d.clock.set(AFTER_WINDOW);
    d.outbox.tick().unwrap();
    let got = smtp.received();
    assert_eq!(got.len(), 1, "one message at the server");
    let mut rcpt: Vec<String> = got[0].rcpt_to.iter().map(|a| a.trim_matches(|c| c == '<' || c == '>').to_ascii_lowercase()).collect();
    rcpt.sort();
    assert_eq!(rcpt, vec!["ann@linea-example.com", "bob@linea-example.com", "cy@linea-example.com", "dee@linea-example.com"], "the envelope holds every To and Cc: {:?}", got[0].rcpt_to);
    let h = headers(&got[0].data);
    assert!(get(&h, "to")[0].contains("ann@linea-example.com") && get(&h, "to")[0].contains("bob@linea-example.com"), "{h:?}");
    assert!(get(&h, "cc")[0].contains("cy@linea-example.com") && get(&h, "cc")[0].contains("dee@linea-example.com"), "{h:?}");
    assert_eq!(get(&h, "in-reply-to"), vec!["<ra-2@linea-example.com>".to_string()]);
    assert_eq!(get(&h, "references"), vec!["<ra-1@linea-example.com> <ra-2@linea-example.com>".to_string()]);
    d.clock.set(AFTER_WINDOW + 10 * MINUTE);
    d.outbox.tick().unwrap();
    assert_eq!(smtp.received().len(), 1, "a later tick does not send it again");
    assert!(d.parents.asked.lock().unwrap().is_empty(), "the stored headers answered: the server was not asked");
}
