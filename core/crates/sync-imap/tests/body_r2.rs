//! body-r2: the body fetch path. On the owner's Mac, one large newsletter (the sweetgreen mail, UID 143411) was reported again and again
//! as "the server sent no body for UID 143411". A body fetch can end in that sentence in more than one way, and the sentence said
//! which of them it was in none. Here each answer shape a server can give to `UID FETCH <uids> (UID RFC822.SIZE BODY.PEEK[])` is played
//! by a small scripted server (tests/body_r2_server: it listens on 127.0.0.1 only, nothing leaves this machine): the shapes a real server may legally use must give the
//! body, the ones that hold none must say which case it was (what came back, the size the server named).
mod support;
#[path = "body_r2_server/mod.rs"]
mod server;

use server::{raw, Server, Shape};
use sync_imap::*;

fn adapter(srv: &Server) -> ImapAdapter { server::adapter(srv, support::limits()) }

/// Every shape a server may legally answer with must hand over the exact bytes it sent: a message of 600 KB included.
#[tokio::test]
async fn a_600_kb_body_comes_back_whole_in_every_legal_answer_shape() {
    for shape in [Shape::Whole, Shape::Chunked, Shape::Origin, Shape::Rfc822, Shape::UidLast, Shape::Split] {
        let srv = Server::start(&[(143411, shape)], 600).await;
        let a = adapter(&srv);
        let got = a.fetch_raw_many("INBOX", &[143411]).await.unwrap_or_else(|e| panic!("{shape:?}: {e:?}"));
        assert_eq!(got.len(), 1, "{shape:?}: the body is in the answer");
        assert_eq!(got[0].0, 143411);
        assert_eq!(got[0].1, raw(143411, 600), "{shape:?}: the 600 KB are the bytes the server sent");
        assert!(a.body_miss("INBOX", 143411).is_none(), "{shape:?}: nothing missed");
        let (one, _, _) = a.fetch_raw("INBOX", 143411).await.unwrap_or_else(|e| panic!("{shape:?} alone: {e:?}"));
        assert_eq!(one, raw(143411, 600), "{shape:?}: the single fetch too");
    }
}

/// A batch: the message in the odd shape is the middle one; the other two come back untouched.
#[tokio::test]
async fn a_batch_keeps_the_messages_around_the_odd_one() {
    let srv = Server::start(&[(10, Shape::Whole), (11, Shape::Split), (12, Shape::Origin)], 60).await;
    let got = adapter(&srv).fetch_raw_many("INBOX", &[10, 11, 12]).await.unwrap();
    assert_eq!(got.iter().map(|(u, b)| (*u, b.len())).collect::<Vec<_>>(), vec![(10, raw(10, 60).len()), (11, raw(11, 60).len()), (12, raw(12, 60).len())]);
}

/// The answers that hold no body: the missing UID is absent from the result and the case is kept, in the words the core's log
/// prints: what the server sent back (its keys), and the size it named.
#[tokio::test]
async fn a_body_the_server_does_not_send_says_which_case_it_was() {
    let n = raw(143411, 4).len();
    let size = format!("the server named size {n}");
    let cases: [(Shape, Vec<String>); 4] = [
        (Shape::Nil, vec!["BODY[] came back NIL".into(), size.clone(), "keys UID, RFC822.SIZE, BODY[]".into()]),
        (Shape::Empty, vec!["BODY[] came back as an empty literal, 0 bytes".into(), size.clone(), "keys UID, RFC822.SIZE, BODY[]".into()]),
        (Shape::NoBodyKey, vec!["no BODY[] in the answer".into(), size.clone(), "keys UID, RFC822.SIZE)".into()]),
        (Shape::NoAnswer, vec!["no FETCH for this UID (answered 0 of the 1 UIDs asked)".into()]),
    ];
    for (shape, expect) in cases {
        let srv = Server::start(&[(143411, shape)], 4).await;
        let a = adapter(&srv);
        let got = a.fetch_raw_many("INBOX", &[143411]).await.unwrap();
        assert!(got.is_empty(), "{shape:?}: no body is not made up");
        let why = a.body_miss("INBOX", 143411).unwrap_or_else(|| panic!("{shape:?}: the case is kept"));
        for part in &expect { assert!(why.contains(part.as_str()), "{shape:?}: {part:?} is in {why:?}"); }
        // a UID asked alone is asked once more by its two sections; the case says both asks
        assert!(why.contains("asked alone; tried by sections"), "{shape:?}: {why:?}");
        assert!(a.body_miss("INBOX", 143411).is_none(), "{shape:?}: a case is read once");
    }
}

/// A server that leaves a large message out of a batch but gives it when it is asked for alone: the body arrives, no miss is kept.
#[tokio::test]
async fn a_message_dropped_from_a_batch_is_asked_again_alone() {
    let srv = Server::start(&[(20, Shape::Whole), (21, Shape::DropsFromBatch), (22, Shape::Whole)], 30).await;
    let a = adapter(&srv);
    let got = a.fetch_raw_many("INBOX", &[20, 21, 22]).await.unwrap();
    assert_eq!(got.iter().map(|(u, _)| *u).collect::<Vec<_>>(), vec![20, 21, 22]);
    assert!(a.body_miss("INBOX", 21).is_none());
    let fetches = srv.fetches();
    assert_eq!(fetches.len(), 2, "one batch, one ask for the dropped UID alone: {fetches:?}");
    assert!(fetches[1].contains("UID FETCH 21 "), "{fetches:?}");
    let _ = srv.shapes.lock().unwrap().len();
}

/// A server that has no whole-message answer for a large message but gives its header and its text: the two put together are the message.
#[tokio::test]
async fn a_message_the_server_gives_only_by_sections_is_put_together() {
    let srv = Server::start(&[(143411, Shape::SectionsOnly)], 600).await;
    let a = adapter(&srv);
    let got = a.fetch_raw_many("INBOX", &[143411]).await.unwrap();
    assert_eq!(got, vec![(143411, raw(143411, 600))]);
    assert!(a.body_miss("INBOX", 143411).is_none());
}

/// body-r3: one numbered part of a message is fetched by its number (`BODY.PEEK[2]`, never `BODY[2]`, so \Seen stays as it was), in its
/// transfer encoding; decoded as the message says, it is the original file. A part the server does not hold is an error in words.
#[tokio::test]
async fn a_part_is_fetched_by_its_number_and_decodes_to_the_original_file() {
    let (fx, files) = server::files_mail();
    let srv = Server::start(&[], 1).await;
    srv.put(7, fx.clone());
    let a = adapter(&srv);
    for f in &files {
        let raw_part = a.fetch_part("INBOX", 7, f.section).await.unwrap_or_else(|e| panic!("part {}: {e:?}", f.section));
        assert_eq!(raw_part, fx.sections.iter().find(|(k, _)| k == f.section).unwrap().1, "part {}: the bytes the server sent, still encoded", f.section);
        let enc = parse::attachments_of(&fx.raw).unwrap().into_iter().find(|x| x.part == f.section).unwrap().encoding;
        assert_eq!(parse::decode_transfer(&enc, &raw_part), f.bytes, "part {}", f.section);
    }
    let asked = srv.part_fetches();
    assert_eq!(asked.len(), 3, "{asked:?}");
    assert!(asked.iter().all(|l| l.contains("BODY.PEEK[")), "{asked:?}");
    let err = format!("{:?}", a.fetch_part("INBOX", 7, "9").await.unwrap_err());
    assert!(err.contains("the server sent no part 9 of UID 7") && err.contains("BODY[9]"), "{err}");
}
