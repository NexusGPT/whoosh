//! The tally the writer keeps equals a from-scratch count after every kind of write, in a long mixed run.
mod common;
use common::*;

struct Lcg(u64);
impl Lcg {
    fn next(&mut self, n: usize) -> usize {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((self.0 >> 33) as usize) % n
    }
}

#[test]
fn tally_equals_recount_after_every_write_of_a_mixed_run() {
    let db = TempDb::new("tally");
    let store = small_store(&db, 60);
    assert_eq!(store.counts().unwrap(), store.recount().unwrap());
    let mut rng = Lcg(7);
    let mut next_new = 0usize;
    for step in 0..400 {
        let id = format!("m{:03}", rng.next(60));
        match rng.next(7) {
            0 => drop(store.set_read(vec![id], rng.next(2) == 0)),
            1 => drop(store.set_archived(vec![id], rng.next(2) == 0)),
            2 => store.snooze(vec![id], 9_000_000_000_000, 1).unwrap(),
            3 => drop(store.unsnooze(vec![id])),
            4 => store.set_pinned(id, rng.next(2) == 0, step).unwrap(),
            5 => {
                // an inbound reply in an existing thread: it becomes the thread's newest
                let thread = format!("t{:03}", rng.next(30));
                next_new += 1;
                let m = msg(&format!("new{next_new}"), &thread, "acc-a", "late@ext.test", 1_900_000_000_000 + next_new as i64 * 1000);
                store.insert_messages(vec![new_msg(m, Some("primary"))]).unwrap();
            }
            _ => {
                // the owner answers: a message from the owner's own address in the Sent folder ends the thread's "waiting"
                let thread = format!("t{:03}", rng.next(30));
                next_new += 1;
                let mut m = msg(&format!("sent{next_new}"), &thread, "acc-a", "owner0@example.test", 1_950_000_000_000 + next_new as i64 * 1000);
                m.folder = "Sent".into();
                m.read = true;
                store.insert_messages(vec![new_msg(m, None)]).unwrap();
            }
        }
        assert_eq!(store.counts().unwrap(), store.recount().unwrap(), "after step {step}");
    }
    let c = store.counts().unwrap();
    let (primary, every) = (&c.tabs[0], &c.tabs[1]);
    assert_eq!((primary.tab_id.as_str(), every.tab_id.as_str()), ("primary", "all"), "the views come first: {c:?}");
    assert!(every.unread >= primary.unread && every.waiting >= primary.waiting, "All >= Primary: {c:?}");
    assert!(c.tabs.iter().map(|t| t.waiting).sum::<u32>() > 0 && c.tabs.iter().map(|t| t.unread).sum::<u32>() > 0, "the run exercised both counts: {c:?}");
}

#[test]
fn the_owner_answering_ends_waiting() {
    let db = TempDb::new("answer");
    let store = small_store(&db, 2); // m000 (acc-a) and m001 (acc-b), threads t000 for both
    let before = store.counts().unwrap();
    // delta 16: the All row is every category summed, so it is the inbox's own figure (a sum over the tabs would count twice)
    let all = |c: &contracts::Counts| c.tabs.iter().find(|t| t.tab_id == "all").map(|t| t.waiting).expect("the All row");
    let waiting_before: u32 = all(&before);
    assert_eq!(waiting_before, 1, "the newest inbound message of the thread is waiting: {before:?}");
    let mut reply = msg("r1", "t000", "acc-a", "owner0@example.test", 1_800_000_000_000);
    reply.folder = "Sent".into();
    store.insert_messages(vec![new_msg(reply, None)]).unwrap();
    let after = store.counts().unwrap();
    assert_eq!(all(&after), 0, "{after:?}");
    assert_eq!(after, store.recount().unwrap());
}

#[test]
fn all_inboxes_holds_the_all_views_unread_and_follows_a_read() {
    let db = TempDb::new("allbox");
    let store = small_store(&db, 6);
    let all = |c: &contracts::Counts| c.tabs.iter().find(|t| t.tab_id == "all").map(|t| t.unread).expect("the All row");
    let boxed = |c: &contracts::Counts| c.mailboxes.get(contracts::MAILBOX_ALL_KEY).copied();
    let before = store.counts().unwrap();
    assert!(all(&before) > 0, "the store holds unread mail: {before:?}");
    assert_eq!(boxed(&before), Some(all(&before)), "All inboxes is the All view's unread: {before:?}");
    store.set_read((0..6).map(|i| format!("m{i:03}")).collect(), true).unwrap();
    let after = store.counts().unwrap();
    assert_eq!((boxed(&after), all(&after)), (Some(0), 0), "every mail read: {after:?}");
    assert_eq!(after, store.recount().unwrap());
}
