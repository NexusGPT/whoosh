//! contracts delta 10 hook, addition 13: V moves to a label (label + archive) with ONE undo entry; Z takes the label off
//! only where the move added it and unarchives only what the move archived.
mod common;
use actions::{Archive, Label, MoveToLabel, Undo};
use common::*;

fn label_id(r: &Rig, name: &'static str) -> String {
    r.store.write(move |c| Ok(c.tx.query_row("SELECT id FROM label WHERE name = ?1", [name], |x| x.get::<_, String>(0))?)).unwrap()
}

#[test]
fn a_move_is_one_undo_entry_and_z_restores_exactly_what_it_changed() {
    let r = rig("move-to-label", four());
    Label::new(r.env.clone()).execute(vec!["m2".into()], "Suppliers".into(), true).unwrap();
    Archive::new(r.env.clone()).execute(vec!["m3".into()]).unwrap();
    let id = label_id(&r, "Suppliers");
    let entries = scalar(&r, "SELECT count(*) FROM undo_entry");
    let out = MoveToLabel::new(r.env.clone()).execute(vec!["m1".into(), "m2".into(), "m3".into()], id.clone()).unwrap();
    assert!(out.undo_entry_id.is_some());
    assert_eq!(scalar(&r, "SELECT count(*) FROM undo_entry") - entries, 1, "one entry for label + archive");
    assert_eq!(inbox_ids(&r), vec!["m4".to_string()]);
    Undo::new(r.env.clone()).execute().unwrap();
    let mut back = inbox_ids(&r);
    back.sort();
    assert_eq!(back, vec!["m1".to_string(), "m2".into(), "m4".into()], "m3 was archived before the move and stays archived");
    assert_eq!(scalar(&r, "SELECT count(*) FROM label_assignment WHERE message_id = 'm1'"), 0, "the move's label leaves m1");
    assert_eq!(scalar(&r, "SELECT count(*) FROM label_assignment WHERE message_id = 'm2'"), 1, "m2 carried it before");
}

#[test]
fn a_move_to_an_unknown_label_is_refused() {
    let r = rig("move-unknown", four());
    assert!(MoveToLabel::new(r.env.clone()).execute(vec!["m1".into()], "nope".into()).is_err());
    assert_eq!(inbox_ids(&r).len(), 4);
}
