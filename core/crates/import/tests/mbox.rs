mod support;

use std::io::{BufRead, Cursor, Read};

use import::{flags_from_headers, walk_mbox, MboxMessages, IMPORT_MAX_MESSAGE_BYTES};
use support::{mbox, msg, Collect};

#[test]
fn an_escaped_from_line_is_unescaped_and_does_not_split_the_message() {
    let body = "hello\nFrom the depths of winter\n>From already quoted\nbye";
    let a = msg("m1@x.test", "a@x.test", "b@x.test", "one", body);
    let b = msg("m2@x.test", "a@x.test", "b@x.test", "two", "second");
    let bytes = mbox(&[&a, &b]);
    let items: Vec<_> = MboxMessages::new(Cursor::new(bytes), IMPORT_MAX_MESSAGE_BYTES).collect::<Result<_, _>>().unwrap();
    assert_eq!(items.len(), 2);
    let first = String::from_utf8(items[0].bytes.clone()).unwrap();
    assert!(first.contains("\nFrom the depths of winter\n"), "{first}");
    assert!(first.contains("\n>From already quoted\n"), "{first}");
    assert!(!first.contains(">From the depths"), "{first}");
    assert_eq!(first, a);
    assert_eq!(String::from_utf8(items[1].bytes.clone()).unwrap(), b);
}

#[test]
fn a_body_line_that_starts_with_from_but_has_no_date_is_not_a_separator() {
    let m = msg("m1@x.test", "a@x.test", "b@x.test", "one", "From scratch we built it\nand more");
    // written by a sloppy program that never escaped it
    let mut bytes = b"From MAILER-DAEMON Tue Jan  2 10:11:12 2024\n".to_vec();
    bytes.extend_from_slice(m.as_bytes());
    bytes.push(b'\n');
    let items: Vec<_> = MboxMessages::new(Cursor::new(bytes), IMPORT_MAX_MESSAGE_BYTES).collect::<Result<_, _>>().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(String::from_utf8(items[0].bytes.clone()).unwrap(), m);
}

#[test]
fn the_from_line_date_is_the_received_time() {
    let m = msg("m1@x.test", "a@x.test", "b@x.test", "one", "x");
    let items: Vec<_> = MboxMessages::new(Cursor::new(mbox(&[&m])), IMPORT_MAX_MESSAGE_BYTES).collect::<Result<_, _>>().unwrap();
    // 2024-01-02 10:11:12 UTC
    assert_eq!(items[0].received_ms, Some(1_704_190_272_000));
}

/// Counts the bytes taken from the file. A reader that held the whole file would take them all on the first message.
struct Counting<'a> {
    inner: Cursor<Vec<u8>>,
    consumed: &'a std::cell::Cell<usize>,
}
impl Read for Counting<'_> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.inner.read(buf)
    }
}
impl BufRead for Counting<'_> {
    fn fill_buf(&mut self) -> std::io::Result<&[u8]> {
        self.inner.fill_buf()
    }
    fn consume(&mut self, amt: usize) {
        self.consumed.set(self.consumed.get() + amt);
        self.inner.consume(amt)
    }
}

#[test]
fn the_file_is_read_message_by_message_not_whole() {
    let big = "x".repeat(2000);
    let msgs: Vec<String> = (0..20).map(|i| msg(&format!("m{i}@x.test"), "a@x.test", "b@x.test", "s", &big)).collect();
    let refs: Vec<&str> = msgs.iter().map(String::as_str).collect();
    let bytes = mbox(&refs);
    let total = bytes.len();
    let consumed = std::cell::Cell::new(0);
    let mut it = MboxMessages::new(Counting { inner: Cursor::new(bytes), consumed: &consumed }, IMPORT_MAX_MESSAGE_BYTES);
    assert!(it.next().unwrap().is_ok());
    assert!(consumed.get() < total / 4, "after one message the reader had taken {} of {total} bytes", consumed.get());
}

#[test]
fn a_message_over_the_limit_is_flagged_and_dropped_and_the_next_one_is_read() {
    let long = msg("m1@x.test", "a@x.test", "b@x.test", "one", &"y".repeat(500));
    let short = msg("m2@x.test", "a@x.test", "b@x.test", "two", "ok");
    let items: Vec<_> = MboxMessages::new(Cursor::new(mbox(&[&long, &short])), 200).collect::<Result<_, _>>().unwrap();
    assert!(items[0].oversize && items[0].bytes.is_empty());
    assert!(!items[1].oversize);
    assert_eq!(String::from_utf8(items[1].bytes.clone()).unwrap(), short);
}

#[test]
fn a_file_that_is_not_an_mbox_is_an_error_in_words() {
    let mut it = MboxMessages::new(Cursor::new(b"hello\nworld\n".to_vec()), IMPORT_MAX_MESSAGE_BYTES);
    let err = it.next().unwrap().unwrap_err();
    assert!(err.to_string().contains("not an mbox"), "{err}");
    assert!(it.next().is_none());
}

#[test]
fn read_state_comes_from_the_status_headers_and_defaults_to_read() {
    let with = |h: &str| flags_from_headers(format!("{h}\nFrom: a@x.test\n\nbody\n").as_bytes());
    assert!(with("Status: RO").read);
    assert!(!with("Status: O").read);
    assert!(!with("X-Mozilla-Status: 0000").read);
    assert!(with("X-Mozilla-Status: 0001").read);
    assert!(with("X-Mozilla-Status: 0005").flagged);
    assert!(with("X-Status: F").flagged);
    assert!(with("X-Status: D").deleted);
    assert!(with("Subject: no state headers").read);
}

#[test]
fn walking_a_file_gives_each_message_its_mailbox_flags_and_bytes() {
    let dir = tempfile::tempdir().unwrap();
    let a = msg("w1@x.test", "a@x.test", "b@x.test", "one", "first");
    let b = format!("X-Mozilla-Status: 0000\n{}", msg("w2@x.test", "a@x.test", "b@x.test", "two", "second"));
    let file = dir.path().join("Inbox");
    std::fs::write(&file, mbox(&[&a, &b])).unwrap();
    let mut got = Collect::default();
    walk_mbox(&file, &mut got).unwrap();
    assert_eq!(got.mails.len(), 2);
    assert!(got.mails.iter().all(|m| m.mailbox == "Inbox"));
    assert!(got.mails[0].flags.read);
    assert!(!got.mails[1].flags.read);
    assert_eq!(got.mails[0].bytes, a.as_bytes());
    assert!(got.skips.is_empty());
}
