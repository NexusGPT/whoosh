mod support;

use import::parse_emlx;
use support::{emlx, msg};

const M: &str = "Message-ID: <e1@x.test>\nFrom: a@x.test\nTo: b@x.test\nSubject: s\n\nbody\n";

#[test]
fn the_message_ends_where_the_byte_count_says_and_the_plist_is_not_part_of_it() {
    let parsed = parse_emlx(emlx(M, 1, 1_700_000_000)).unwrap();
    assert_eq!(parsed.message, M.as_bytes());
    assert!(!String::from_utf8_lossy(&parsed.message).contains("plist"));
}

#[test]
fn flag_bits_become_read_deleted_answered_flagged_draft() {
    let read = parse_emlx(emlx(M, 1, 1)).unwrap().flags;
    assert!(read.read && !read.deleted && !read.flagged && !read.answered && !read.draft);
    let unread = parse_emlx(emlx(M, 0, 1)).unwrap().flags;
    assert!(!unread.read);
    let all = parse_emlx(emlx(M, 1 | 2 | 4 | 16 | 64, 1)).unwrap().flags;
    assert!(all.read && all.deleted && all.answered && all.flagged && all.draft);
    // Mail keeps more state in the higher bits; a large value still reads its low bits
    let big = parse_emlx(emlx(M, 8_623_750_144 | 16, 1)).unwrap().flags;
    assert!(!big.read && big.flagged);
}

#[test]
fn the_arrival_time_is_seconds_in_the_plist_and_milliseconds_in_the_result() {
    assert_eq!(parse_emlx(emlx(M, 1, 1_700_000_000)).unwrap().received_ms, Some(1_700_000_000_000));
}

#[test]
fn a_file_with_no_plist_keeps_the_message_and_reads_as_read_with_no_date() {
    let mut bytes = format!("{}\n", M.len()).into_bytes();
    bytes.extend_from_slice(M.as_bytes());
    let parsed = parse_emlx(bytes).unwrap();
    assert_eq!(parsed.message, M.as_bytes());
    assert!(parsed.flags.read);
    assert_eq!(parsed.received_ms, None);
}

#[test]
fn a_byte_count_larger_than_the_file_is_refused_in_words() {
    let err = parse_emlx(b"999\nshort".to_vec()).unwrap_err();
    assert!(err.contains("999"), "{err}");
    let err = parse_emlx(b"not a number\nbody".to_vec()).unwrap_err();
    assert!(err.contains("byte count"), "{err}");
}

#[test]
fn a_message_with_non_ascii_text_keeps_its_bytes_exactly() {
    let m = msg("u1@x.test", "a@x.test", "b@x.test", "Réunion", "À demain, 日本語");
    assert_eq!(parse_emlx(emlx(&m, 1, 1)).unwrap().message, m.as_bytes());
}
