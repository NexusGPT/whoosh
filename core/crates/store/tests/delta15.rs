//! contracts delta 15 hook: an account's signature is kept in the store (the v1 `signature` table, no migration).
mod common;
use common::*;

#[test]
fn a_signature_is_kept_per_address_replaced_and_cleared() {
    let db = TempDb::new("d15-signature");
    let s = small_store(&db, 1);
    assert!(s.signature("owner0@example.test").unwrap().is_none());
    s.set_signature(" Owner0@Example.TEST ", "Best,\nAssem Chammah · A\n", 1).unwrap();
    let got = s.signature("owner0@example.test").unwrap().unwrap();
    assert_eq!((got.address.as_str(), got.text.as_str()), ("owner0@example.test", "Best,\nAssem Chammah · A"), "any case in, trailing blank line dropped");
    s.set_signature("owner0@example.test", "Bien à vous,\nAssem", 2).unwrap();
    assert_eq!(s.signature("OWNER0@example.test").unwrap().unwrap().text, "Bien à vous,\nAssem", "a second save replaces the first");
    assert!(s.signature("owner1@example.test").unwrap().is_none(), "another address is untouched");
    s.set_signature("owner0@example.test", "   ", 3).unwrap();
    assert!(s.signature("owner0@example.test").unwrap().is_none(), "a blank text clears it");
}
