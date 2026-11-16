//! The counts' "vips" mailbox figure: the unread rows the VIPs view lists (inbox mail from a VIP address or a VIP domain), kept in the writer's tally.
mod common;
use common::*;
use store::{Store, StoredView};

fn vips(s: &Store) -> Option<u32> { s.counts().unwrap().mailboxes.get("vips").copied() }

/// The unread rows the VIPs view itself lists: what the figure must equal.
fn view_unread(s: &Store) -> u32 { s.view_rows(StoredView::Vips, None, 500).unwrap().iter().filter(|r| r.unread).count() as u32 }

fn mark(s: &Store, kind: &'static str, value: &'static str) {
    s.write(move |ctx| { ctx.tx.execute("INSERT INTO vip (kind, value, updated_at_ms) VALUES (?1, ?2, 0)", rusqlite::params![kind, value])?; Ok(()) }).unwrap();
}

/// Two accounts and the mail: v1, v2 (VIP address vip@partner.test), d1 (domain board.test), o1 (nobody special).
fn store_with_mail(db: &TempDb) -> Store {
    let s = small_store(db, 0);
    let mail = [("v1", "VIP@Partner.test"), ("v2", "vip@partner.test"), ("d1", "ceo@board.test"), ("o1", "other@ext.test")];
    s.insert_messages(mail.iter().enumerate().map(|(i, (id, from))| new_msg(msg(id, &format!("t-{id}"), "acc-a", from, 1_700_000_000_000 + i as i64 * 1000), Some("primary"))).collect()).unwrap();
    s
}

fn check(s: &Store, want: u32) {
    assert_eq!(vips(s), Some(want), "figure: {:?}", s.counts().unwrap().mailboxes);
    assert_eq!(view_unread(s), want, "the VIPs view lists the same unread rows");
    assert_eq!(s.counts().unwrap(), s.recount().unwrap(), "the tally equals the recount");
}

#[test]
fn a_vip_address_counts_its_unread_inbox_mail() {
    let db = TempDb::new("vipaddr");
    let s = store_with_mail(&db);
    check(&s, 0);
    mark(&s, "address", "vip@partner.test");
    check(&s, 2); // v1 (mixed case sender) and v2
}

#[test]
fn a_vip_domain_counts_its_unread_inbox_mail() {
    let db = TempDb::new("vipdomain");
    let s = store_with_mail(&db);
    mark(&s, "domain", "board.test");
    check(&s, 1);
    mark(&s, "address", "vip@partner.test");
    check(&s, 3);
}

#[test]
fn the_figure_goes_down_when_one_of_those_mails_is_read() {
    let db = TempDb::new("vipread");
    let s = store_with_mail(&db);
    mark(&s, "address", "vip@partner.test");
    mark(&s, "domain", "board.test");
    check(&s, 3);
    s.set_read(vec!["v1".into()], true).unwrap();
    check(&s, 2);
    s.set_read(vec!["d1".into()], true).unwrap();
    check(&s, 1);
    s.set_read(vec!["d1".into()], false).unwrap();
    check(&s, 2);
}

#[test]
fn the_figure_goes_up_when_a_sender_is_added_as_a_vip() {
    let db = TempDb::new("vipadd");
    let s = store_with_mail(&db);
    assert_eq!(vips(&s), Some(0));
    mark(&s, "address", "ceo@board.test");
    check(&s, 1);
    mark(&s, "address", "other@ext.test");
    check(&s, 2);
    s.write(|ctx| { ctx.tx.execute("DELETE FROM vip WHERE value = 'other@ext.test'", [])?; Ok(()) }).unwrap();
    check(&s, 1);
}

#[test]
fn read_mail_and_mail_outside_the_inbox_are_ignored() {
    let db = TempDb::new("vipignore");
    let s = store_with_mail(&db);
    mark(&s, "address", "vip@partner.test");
    let mut sent = msg("x-sent", "t-x1", "acc-a", "vip@partner.test", 1_800_000_000_000);
    sent.folder = "Sent".into();
    let mut read = msg("x-read", "t-x2", "acc-a", "vip@partner.test", 1_800_000_001_000);
    read.read = true;
    let mut junk = msg("x-junk", "t-x3", "acc-a", "vip@partner.test", 1_800_000_002_000);
    junk.folder = "Junk".into();
    s.insert_messages(vec![new_msg(sent, None), new_msg(read, Some("primary")), new_msg(junk, None)]).unwrap();
    check(&s, 2); // only v1 and v2
    // archived, snoozed and muted mail leave the VIPs view (VISIBLE), so they leave the figure
    s.set_archived(vec!["v1".into()], true).unwrap();
    check(&s, 1);
    s.snooze(vec!["v2".into()], 9_000_000_000_000, 1).unwrap();
    check(&s, 0);
    s.unsnooze(vec!["v2".into()]).unwrap();
    s.set_muted(vec!["v2".into()], true).unwrap();
    check(&s, 0);
    s.set_muted(vec!["v2".into()], false).unwrap();
    check(&s, 1);
}
