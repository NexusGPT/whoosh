//! fn-actions-contact-merge: merging two addresses yields one contact id carrying both; the per-account mail count equals the sum.
mod common;
use actions::{BuildContacts, ContactsDirectory, DeviceContact, Label, LabelContact, MergeContact, PermissionDenied};
use common::*;
use std::sync::Arc;

/// Two senders: ann@one.test writes twice to Alpha, ann@two.test writes once to Alpha and once to Beta.
fn mail() -> Vec<store::NewMessage> {
    let mut v = Vec::new();
    for (i, (account, from)) in [("acc-a", "ann@one.test"), ("acc-a", "ann@one.test"), ("acc-a", "ann@two.test"), ("acc-b", "ann@two.test"), ("acc-b", "bob@three.test")].iter().enumerate() {
        v.push(new_msg(msg(&format!("c{i}"), &format!("ct{i}"), account, from, NOW - 100_000 + i as i64 * 1000)));
    }
    v
}

fn counts_of(r: &Rig, id: &str) -> std::collections::BTreeMap<String, i64> {
    let id = id.to_string();
    r.store
        .write(move |ctx| {
            let mut st = ctx.tx.prepare("SELECT key, value FROM contact, json_each(contact.mail_count_json) WHERE contact.id = ?1")?;
            let rows = st.query_map([&id], |x| Ok((x.get::<_, String>(0)?, x.get::<_, i64>(1)?)))?;
            Ok(rows.collect::<Result<_, _>>()?)
        })
        .unwrap()
}

#[test]
fn merging_two_addresses_yields_one_contact_carrying_both_and_the_counts_add_up() {
    let r = rig("merge", mail());
    BuildContacts::new(r.env.clone()).from_headers().unwrap();
    assert_eq!(scalar(&r, "SELECT count(*) FROM contact"), 3, "ann@one, ann@two and bob");
    let id = MergeContact::new(r.env.clone()).execute("ann@one.test".into(), "ANN@two.test".into()).unwrap();
    assert_eq!(scalar(&r, "SELECT count(*) FROM contact"), 2);
    assert_eq!(scalar(&r, "SELECT count(*) FROM contact, json_each(contact.addresses_json) WHERE contact.name LIKE 'Name of ann%'"), 2, "one contact, both addresses");
    let counts = counts_of(&r, &id);
    assert_eq!(counts.get("acc-a"), Some(&3), "Alpha: two from one.test plus one from two.test");
    assert_eq!(counts.get("acc-b"), Some(&1));
    let total: i64 = counts.values().sum();
    assert_eq!(total, scalar(&r, "SELECT count(*) FROM message WHERE from_address IN ('ann@one.test', 'ann@two.test')"), "the per-account count equals the sum of both addresses' mail");
}

#[test]
fn merging_without_contact_rows_builds_them_from_the_headers_first() {
    let r = rig("merge-fresh", mail());
    let id = MergeContact::new(r.env.clone()).execute("ann@one.test".into(), "ann@two.test".into()).unwrap();
    assert_eq!(scalar(&r, "SELECT count(*) FROM contact"), 1);
    assert_eq!(counts_of(&r, &id).values().sum::<i64>(), 4);
}

#[test]
fn merging_an_address_with_itself_changes_nothing() {
    let r = rig("merge-self", mail());
    let a = MergeContact::new(r.env.clone()).execute("ann@one.test".into(), "ann@one.test".into()).unwrap();
    let b = MergeContact::new(r.env.clone()).execute("ann@one.test".into(), "Ann@One.test".into()).unwrap();
    assert_eq!(a, b);
    assert_eq!(scalar(&r, "SELECT count(*) FROM contact"), 1);
}

#[test]
fn an_address_without_an_at_is_refused_in_words() {
    let r = rig("merge-bad", mail());
    let e = MergeContact::new(r.env.clone()).execute("ann".into(), "ann@two.test".into()).unwrap_err();
    assert!(matches!(e, contracts::CoreError::Invalid { .. }));
    assert_eq!(scalar(&r, "SELECT count(*) FROM contact"), 0);
}

#[test]
fn contacts_never_include_the_owner_own_addresses() {
    let mut v = mail();
    v.push(new_msg(msg("own", "ot", "acc-a", OWN_A, NOW)));
    let r = rig("own", v);
    BuildContacts::new(r.env.clone()).from_headers().unwrap();
    assert_eq!(scalar(&r, "SELECT count(*) FROM contact, json_each(contact.addresses_json) WHERE json_each.value = 'owner@alpha.test'"), 0);
}

struct Device(Result<Vec<DeviceContact>, PermissionDenied>);
impl ContactsDirectory for Device {
    fn execute(&self) -> Result<Vec<DeviceContact>, PermissionDenied> {
        self.0.clone()
    }
}

#[test]
fn device_contacts_join_an_existing_contact_by_a_shared_address_and_their_name_wins() {
    let r = rig("device", mail());
    let b = BuildContacts::new(r.env.clone());
    b.from_headers().unwrap();
    let person = DeviceContact { name: "Ann Example".into(), addresses: vec!["ann@one.test".into(), "ann@two.test".into(), "ann@home.test".into()] };
    let n = b.from_device(Arc::new(Device(Ok(vec![person])))).unwrap();
    assert_eq!(n, Some(1));
    assert_eq!(scalar(&r, "SELECT count(*) FROM contact WHERE name = 'Ann Example'"), 1);
    assert_eq!(scalar(&r, "SELECT count(*) FROM contact"), 2, "Ann (merged) and Bob");
    assert_eq!(scalar(&r, "SELECT json_array_length(addresses_json) FROM contact WHERE name = 'Ann Example'"), 3);
}

#[test]
fn without_the_owners_permission_the_device_contacts_are_not_read() {
    let r = rig("device-denied", mail());
    let b = BuildContacts::new(r.env.clone());
    b.from_headers().unwrap();
    assert_eq!(b.from_device(Arc::new(Device(Err(PermissionDenied)))).unwrap(), None);
    assert_eq!(scalar(&r, "SELECT count(*) FROM contact"), 3);
}

#[test]
fn label_contact_labels_every_message_from_all_its_addresses() {
    let r = rig("label-contact", mail());
    BuildContacts::new(r.env.clone()).from_headers().unwrap();
    let id = MergeContact::new(r.env.clone()).execute("ann@one.test".into(), "ann@two.test".into()).unwrap();
    let out = LabelContact::new(r.env.clone()).execute(id, "Friends".into(), true).unwrap();
    assert!(out.created_id.is_some());
    assert_eq!(scalar(&r, "SELECT count(*) FROM label_assignment"), 4, "all four messages of both addresses");
    // The same names again do not duplicate.
    LabelContact::new(r.env.clone()).execute(scalar_id(&r), "friends".into(), false).unwrap();
    assert_eq!(scalar(&r, "SELECT count(*) FROM label_assignment"), 4);
    let _ = Label::new(r.env.clone());
}

fn scalar_id(r: &Rig) -> String {
    text(r, "SELECT id FROM contact WHERE name LIKE 'Name of ann%'")
}

#[test]
fn label_contact_for_an_unknown_contact_is_not_found() {
    let r = rig("label-contact-missing", mail());
    let e = LabelContact::new(r.env.clone()).execute("contact-zzz".into(), "Friends".into(), true).unwrap_err();
    assert!(matches!(e, contracts::CoreError::NotFound { .. }));
}
