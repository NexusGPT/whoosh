//! Sending safety, pieces 2 and 3, the IMAP half: the Sent copy (role, flag, date, Gmail skip, the server's
//! refusal in words) and the headers of the message a reply answers. Fake server on the loopback only.
mod support;

use contracts::{AppendSentPort, AppendSentRequest, CoreError};
use send::{ParentHeadersPort, ParentRef};
use support::*;
use sync_imap::{parent_headers_from, AccountAdapters, Auth, ImapAdapter, ImapMailAdapter, ImapParentHeaders, ImapSentAppender, Provider};

fn sent_message() -> Vec<u8> {
    b"From: owner@example.test\r\nTo: orders@linea-example.test\r\nSubject: Re: Quote\r\nMessage-ID: <draft-1@example.test>\r\nDate: Sun, 08 Nov 2026 10:30:15 +0100\r\n\r\nYes.\r\n".to_vec()
}

#[tokio::test(flavor = "multi_thread")]
async fn the_sent_copy_goes_to_the_special_use_sent_folder_flagged_seen_and_dated_with_the_send_date() {
    let srv = FakeImap::start().await;
    // A name no heuristic knows: only the RFC 6154 \Sent attribute can find it.
    srv.with(|s| s.boxes[1].name = "Gesendet".into());
    adapter(&srv, "work").append_sent(&sent_message()).await.unwrap();

    assert_eq!(srv.uids("Gesendet").len(), 1);
    assert_eq!(srv.flags_of("Gesendet", 1), vec!["\\Seen"]);
    let log = srv.log();
    assert!(
        log.iter().any(|l| l.contains("APPEND \"Gesendet\" (\\Seen) \"08-Nov-2026 09:30:15 +0000\"")),
        "the APPEND carries the flag and the send date (the Date header, in UTC):\n{}",
        log.join("\n")
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn without_special_use_the_names_hosts_use_find_the_sent_folder() {
    let srv = FakeImap::start_with(false).await;
    srv.with(|s| s.cap_special_use = false);
    adapter(&srv, "work").append_sent(&sent_message()).await.unwrap();
    assert_eq!(srv.uids("Sent Messages").len(), 1);
    assert_eq!(srv.flags_of("Sent Messages", 1), vec!["\\Seen"]);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_gmail_account_appends_nothing_because_gmail_saves_the_copy_itself() {
    let srv = FakeImap::start().await;
    let gmail = ImapAdapter::with_provider(config(&srv, "g", Auth::Password("app-password".into())), Provider::Gmail);
    gmail.append_sent(&sent_message()).await.unwrap();
    assert!(srv.uids("Sent").is_empty(), "no copy appended for Gmail");
    assert!(srv.log().is_empty(), "not even a connection was opened: {:?}", srv.log());

    // The same server as a host with no special rule: the copy is appended.
    let other = ImapAdapter::with_provider(config(&srv, "o", Auth::Password("app-password".into())), Provider::Other);
    other.append_sent(&sent_message()).await.unwrap();
    assert_eq!(srv.uids("Sent").len(), 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_server_with_no_sent_folder_says_so_in_words() {
    let srv = FakeImap::start().await;
    srv.with(|s| s.boxes.retain(|b| b.name != "Sent"));
    match adapter(&srv, "work").append_sent(&sent_message()).await {
        Err(CoreError::Action { failure }) => assert!(failure.reason_words.contains("no Sent folder"), "{}", failure.reason_words),
        other => panic!("expected words, got {other:?}"),
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn the_synchronous_port_appends_through_the_account_adapter_and_returns_the_servers_refusal() {
    let srv = FakeImap::start().await;
    let adapters = AccountAdapters::new(tokio::runtime::Handle::current()).with("acc-1", adapter(&srv, "work"));
    let port = std::sync::Arc::new(ImapSentAppender(adapters));
    let request = AppendSentRequest { account_id: "acc-1".into(), rfc822: sent_message() };

    let p = port.clone();
    let r = request.clone();
    tokio::task::spawn_blocking(move || p.execute(&r)).await.unwrap().unwrap();
    assert_eq!(srv.uids("Sent").len(), 1);

    srv.with(|s| s.refuse_append = true);
    let p = port.clone();
    let r = request.clone();
    match tokio::task::spawn_blocking(move || p.execute(&r)).await.unwrap() {
        Err(CoreError::Action { failure }) => assert!(failure.reason_words.contains("Quota exceeded"), "the server's own words: {}", failure.reason_words),
        other => panic!("expected the refusal, got {other:?}"),
    }
    assert_eq!(srv.uids("Sent").len(), 1, "nothing was stored by the refused append");

    let p = port.clone();
    let unknown = AppendSentRequest { account_id: "acc-9".into(), rfc822: sent_message() };
    match tokio::task::spawn_blocking(move || p.execute(&unknown)).await.unwrap() {
        Err(e) => assert!(e.to_string().contains("acc-9"), "{e}"),
        Ok(()) => panic!("an account with no IMAP connection must not report success"),
    }
}

// ------------------------------------------------------------------------------------ parent headers

#[test]
fn the_parents_message_id_and_references_are_read_from_its_header_block_folded_lines_included() {
    let block = b"From: a@x\r\nMessage-ID: <c-3@x>\r\nReferences: <a-1@x>\r\n <b-2@x>\r\n\t<b-2b@x>\r\nSubject: s\r\n\r\n";
    let h = parent_headers_from(block).unwrap();
    assert_eq!(h.message_id, "<c-3@x>");
    assert_eq!(h.references, vec!["<a-1@x>", "<b-2@x>", "<b-2b@x>"]);

    // RFC 5322 3.6.4: with no References the chain is the In-Reply-To.
    let h = parent_headers_from(b"Message-ID: <c@x>\r\nIn-Reply-To: <a@x>\r\n\r\n").unwrap();
    assert_eq!(h.references, vec!["<a@x>"]);

    match parent_headers_from(b"Subject: no id\r\n\r\n") {
        Err(CoreError::Invalid { reason_words, .. }) => assert!(reason_words.contains("no Message-ID"), "{reason_words}"),
        other => panic!("expected words, got {other:?}"),
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn the_parent_headers_port_asks_the_server_for_the_header_block_of_that_uid() {
    let srv = FakeImap::start().await;
    srv.seed("INBOX", 3);
    srv.with(|s| s.boxes[0].msgs[2].raw = b"From: a@x\r\nMessage-ID: <c-3@example.test>\r\nReferences: <a-1@example.test>\r\n <b-2@example.test>\r\nSubject: Quote\r\n\r\nBody.\r\n".to_vec());
    let adapters = AccountAdapters::new(tokio::runtime::Handle::current()).with("acc-1", adapter(&srv, "work"));
    let port = std::sync::Arc::new(ImapParentHeaders(adapters));

    let p = port.clone();
    let got = tokio::task::spawn_blocking(move || p.execute(&ParentRef { account_id: "acc-1".into(), folder: "INBOX".into(), remote_id: "3".into() })).await.unwrap().unwrap();
    assert_eq!(got.message_id, "<c-3@example.test>");
    assert_eq!(got.references, vec!["<a-1@example.test>", "<b-2@example.test>"]);
    assert!(srv.log().iter().any(|l| l.contains("UID FETCH 3 (UID BODY.PEEK[HEADER])")), "only the header block was asked for: {:?}", srv.log());

    let p = port.clone();
    let bad = tokio::task::spawn_blocking(move || p.execute(&ParentRef { account_id: "acc-1".into(), folder: "INBOX".into(), remote_id: "not-a-uid".into() })).await.unwrap();
    assert!(bad.is_err());
}
