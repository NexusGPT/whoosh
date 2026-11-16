//! contracts delta 10 hook: deleting a selection is ONE undo entry; one Z brings the whole batch back.
mod common;
use actions::{Trash, Undo};
use common::*;

#[test]
fn deleting_three_is_undone_by_one_z() {
    let r = rig("delete-batch", four());
    let ids: Vec<String> = vec!["m1".into(), "m2".into(), "m3".into()];
    let out = Trash::new(r.env.clone()).execute(ids.clone()).unwrap();
    assert!(out.undo_entry_id.is_some(), "a delete writes an undo entry");
    assert_eq!(scalar(&r, "SELECT count(*) FROM undo_entry"), 1, "one entry for the three");
    assert_eq!(inbox_ids(&r), vec!["m4".to_string()]);
    Undo::new(r.env.clone()).execute().unwrap();
    let mut back = inbox_ids(&r);
    back.sort();
    assert_eq!(back, vec!["m1", "m2", "m3", "m4"].into_iter().map(String::from).collect::<Vec<_>>());
    assert_eq!(scalar(&r, "SELECT count(*) FROM message WHERE folder = 'Trash'"), 0);
}

#[test]
fn z_returns_each_message_to_the_folder_it_came_from_with_its_archived_mark() {
    let r = rig("delete-origin", four());
    r.store.write(|c| { c.tx.execute("UPDATE message SET folder = 'Spam' WHERE id = 'm2'", [])?; c.tx.execute("UPDATE message SET archived = 1 WHERE id = 'm3'", [])?; Ok(()) }).unwrap();
    Trash::new(r.env.clone()).execute(vec!["m1".into(), "m2".into(), "m3".into()]).unwrap();
    assert_eq!(scalar(&r, "SELECT count(*) FROM message WHERE folder = 'Trash'"), 3);
    Undo::new(r.env.clone()).execute().unwrap();
    assert_eq!(scalar(&r, "SELECT count(*) FROM message WHERE id = 'm1' AND folder = 'INBOX' AND archived = 0"), 1);
    assert_eq!(scalar(&r, "SELECT count(*) FROM message WHERE id = 'm2' AND folder = 'Spam'"), 1, "back to Spam, not the inbox");
    assert_eq!(scalar(&r, "SELECT count(*) FROM message WHERE id = 'm3' AND archived = 1"), 1, "still archived");
}
