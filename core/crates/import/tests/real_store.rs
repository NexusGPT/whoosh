mod support;

use import::{import, AccountAddress, ImportContext, ImportProgress, Source, StoreSink};
use store::Store;
use support::{mbox, msg, IMPORTED};

fn add_account(store: &Store, id: &str, address: &str, order: i64) {
    let sql = format!(
        "INSERT INTO account (id, address, provider, display_name, colour, sort_order, credential_kind, keychain_ref, updated_at_ms) \
         VALUES ('{id}', '{address}', 'imap', '{id}', '#336699', {order}, 'manual', 'svc' || char(31) || '{id}', 0)"
    );
    store
        .write(move |ctx| {
            ctx.tx.execute(&sql, [])?;
            Ok(())
        })
        .unwrap();
}

#[test]
fn imported_mail_lands_in_the_same_database_under_the_matching_account_and_a_second_run_adds_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(&dir.path().join("whoosh.db")).unwrap();
    add_account(&store, "acc-a", "anna@example.test", 0);
    add_account(&store, "acc-b", "ben@example.test", 1);
    add_account(&store, IMPORTED, "imported@whoosh.local", 2);
    // the accounts as the real core lists them (the Imported one is not a receiving address)
    let accounts: Vec<AccountAddress> = store
        .account_summaries()
        .unwrap()
        .into_iter()
        .filter(|a| a.id != IMPORTED)
        .map(|a| AccountAddress { id: a.id, address: a.address })
        .collect();
    assert_eq!(accounts.len(), 2);

    let m1 = msg("st1@x.test", "carol@far.test", "anna@example.test", "for Anna", "hello Anna");
    let m2 = msg("st2@x.test", "carol@far.test", "ben@example.test", "for Ben", "hello Ben");
    let m3 = msg("st3@x.test", "carol@far.test", "stranger@far.test", "for nobody here", "hello");
    let file = dir.path().join("Inbox");
    std::fs::write(&file, mbox(&[&m1, &m2, &m3])).unwrap();
    let source = Source::Mbox { file };
    let ctx = ImportContext { accounts: &accounts, imported_account_id: IMPORTED };

    let mut sink = StoreSink { store: &store };
    let first = import(&source, &ctx, &mut sink, &mut |_: &ImportProgress| {}).unwrap();
    assert_eq!((first.seen, first.stored, first.already_there), (3, 3, 0));
    for (id, account) in [("imp:acc-a:st1@x.test", "acc-a"), ("imp:acc-b:st2@x.test", "acc-b"), ("imp:acc-imported:st3@x.test", IMPORTED)] {
        let m = store.message(id).unwrap().unwrap_or_else(|| panic!("{id} is not in the database"));
        assert_eq!((m.account_id.as_str(), m.folder.as_str()), (account, "INBOX"));
    }
    let m = store.message("imp:acc-a:st1@x.test").unwrap().unwrap();
    assert_eq!((m.subject.as_str(), m.from_address.as_str(), m.read), ("for Anna", "carol@far.test", true));

    let second = import(&source, &ctx, &mut sink, &mut |_: &ImportProgress| {}).unwrap();
    assert_eq!((second.seen, second.stored, second.already_there), (3, 0, 3));
}
