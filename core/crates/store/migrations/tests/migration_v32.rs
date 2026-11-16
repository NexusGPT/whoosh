//! Migration 0032 (owner's picture 212, 13 Nov 2026: "fantom attachment"): the calendar data of a meeting invitation stored before the parser's
//! rule (a calendar part the engine named attachment-N) is marked as a part shown with the text, so it stops reading as a file. No row leaves.
//! MUTANTS: M1 the UPDATE marks every calendar part -> a_stored_invitation_is_marked_and_a_named_ics_is_not
//!          M2 the reversal marks nothing back       -> the_reversal_reads_them_as_files_again
use rusqlite::Connection;
use store_migrations::{migrate, revert_to, MIGRATIONS};

fn at(version: usize) -> Connection {
    let c = Connection::open_in_memory().unwrap();
    c.pragma_update(None, "foreign_keys", false).unwrap();
    for (_, sql) in MIGRATIONS.iter().filter(|(v, _)| (*v as usize) <= version) { c.execute_batch(sql).unwrap(); }
    c.pragma_update(None, "user_version", version as i64).unwrap();
    c
}
fn part(c: &Connection, id: &str, name: &str, mime: &str, inline: i64) {
    c.execute("INSERT INTO attachment (id, message_id, file_name, mime_type, size_bytes, inline) VALUES (?1, 'm1', ?2, ?3, 10, ?4)", rusqlite::params![id, name, mime, inline]).unwrap();
}
fn flags(c: &Connection) -> Vec<(String, i64)> {
    c.prepare("SELECT id, inline FROM attachment ORDER BY id").unwrap().query_map([], |r| Ok((r.get(0)?, r.get(1)?))).unwrap().map(Result::unwrap).collect()
}
fn seeded() -> Connection {
    let c = at(31);
    part(&c, "a-invitation", "attachment-1", "text/calendar", 0);
    part(&c, "b-named-ics", "invite.ics", "text/calendar", 0);
    part(&c, "c-pdf", "attachment-2", "application/pdf", 0);
    part(&c, "d-picture", "logo.png", "image/png", 1);
    c
}

#[test]
fn a_stored_invitation_is_marked_and_a_named_ics_is_not() {
    let mut c = seeded();
    migrate(&mut c).unwrap();
    assert_eq!(flags(&c), vec![("a-invitation".into(), 1), ("b-named-ics".into(), 0), ("c-pdf".into(), 0), ("d-picture".into(), 1)]);
}

#[test]
fn the_reversal_reads_them_as_files_again() {
    let mut c = seeded();
    migrate(&mut c).unwrap();
    revert_to(&mut c, 31).unwrap();
    assert_eq!(flags(&c), vec![("a-invitation".into(), 0), ("b-named-ics".into(), 0), ("c-pdf".into(), 0), ("d-picture".into(), 1)]);
}
