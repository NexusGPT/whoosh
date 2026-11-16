//! engine-act-r5: the server place of a moved message (`message_location`, store/src/place.rs). Recorded from the server's COPYUID answer,
//! replaced by the next move, dropped when the message is back in its INBOX under the INBOX UID; a UID of another folder is cleared
//! before a move so it cannot collide with the target's. Seen red on the mutants named in docs/lanes/engine-act-r5.md.
use store::{ServerPlace, Store};

mod common;
use common::{small_store, TempDb};

fn remote_of(store: &Store, id: &str) -> Option<String> {
    let id = id.to_string();
    store.write(move |c| Ok(c.tx.query_row("SELECT remote_id FROM message WHERE id = ?1", [&id], |r| r.get::<_, Option<String>>(0))?)).unwrap()
}
fn with_uid(store: &Store, id: &str, folder: &str, uid: &str) {
    let (id, folder, uid) = (id.to_string(), folder.to_string(), uid.to_string());
    store.write(move |c| Ok(c.tx.execute("UPDATE message SET folder = ?2, remote_id = ?3 WHERE id = ?1", rusqlite::params![id, folder, uid])?)).unwrap();
}

#[test]
fn a_place_is_recorded_replaced_read_and_dropped_when_the_message_is_back_in_its_inbox() {
    let db = TempDb::new("place-a");
    let store = small_store(&db, 4);
    with_uid(&store, "m000", "INBOX", "3");
    assert_eq!(store.message_place("m000").unwrap(), None, "no move, no place");
    assert!(store.write(|c| c.place_record("m000", "acc-a", "Archive", 77, 1)).unwrap());
    assert_eq!(store.message_place("m000").unwrap(), Some(ServerPlace { folder: "Archive".into(), uid: 1 }));
    assert!(store.write(|c| c.place_record("m000", "acc-a", "Trash", 78, 9)).unwrap(), "a second move replaces the place");
    assert_eq!(store.message_place("m000").unwrap(), Some(ServerPlace { folder: "Trash".into(), uid: 9 }));
    assert!(!store.write(|c| c.place_record("nope", "acc-a", "Archive", 77, 2)).unwrap(), "a message the store does not hold has no place");
    // back in the INBOX under the INBOX UID the server named: the place goes, the row's server id is that UID (the old one is replaced)
    assert!(store.write(|c| c.place_back_in_inbox("m000", 12)).unwrap());
    assert_eq!(store.message_place("m000").unwrap(), None);
    assert_eq!(remote_of(&store, "m000").as_deref(), Some("12"));
}

#[test]
fn an_inbox_uid_another_row_holds_is_refused_not_forced() {
    let db = TempDb::new("place-b");
    let store = small_store(&db, 4);
    with_uid(&store, "m000", "INBOX", "5");
    with_uid(&store, "m002", "INBOX", "6");
    assert!(!store.write(|c| c.place_back_in_inbox("m000", 6)).unwrap(), "the unique key (account, folder, server id) holds");
    assert_eq!(remote_of(&store, "m000").as_deref(), Some("5"));
}

#[test]
fn clearing_a_server_id_lets_a_message_with_the_same_number_move_in() {
    let db = TempDb::new("place-c");
    let store = small_store(&db, 4);
    with_uid(&store, "m000", "INBOX", "1");
    with_uid(&store, "m002", "Junk", "1");
    // the move of m002 to INBOX with its Junk UID collides with m000's INBOX UID 1 (the finding of engine-act-r4)
    assert!(store.write(|c| c.set_folder(&["m002".to_string()], "INBOX")).is_err());
    store.write(|c| { c.clear_remote_ids(&["m002".to_string()])?; c.set_folder(&["m002".to_string()], "INBOX") }).unwrap();
    assert_eq!(remote_of(&store, "m002"), None);
    assert_eq!(remote_of(&store, "m000").as_deref(), Some("1"));
}
