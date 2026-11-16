//! Delta 10 pure rules: filters, the account set, the mailbox count rule, the unsubscribe method.
use contracts::*;

fn facts(unread: bool, starred: bool, important: bool, awaiting_reply: bool) -> ListFilterFacts {
    ListFilterFacts { unread, starred, important, awaiting_reply }
}

#[test]
fn every_filter_must_pass() {
    assert!(passes_filters(facts(false, false, false, false), vec![]));
    assert!(passes_filters(facts(true, true, false, false), vec![InboxFilter::Unread, InboxFilter::Starred]));
    assert!(!passes_filters(facts(true, false, false, false), vec![InboxFilter::Unread, InboxFilter::Starred]));
    assert!(!passes_filters(facts(true, true, true, false), vec![InboxFilter::NoReply]));
    assert!(passes_filters(facts(false, false, true, false), vec![InboxFilter::Important]));
}

fn q(account_id: Option<&str>, account_ids: &[&str]) -> InboxQuery {
    InboxQuery { tab_id: "primary".into(), account_id: account_id.map(String::from), label_id: None, mailbox: None,
                 account_ids: account_ids.iter().map(|s| s.to_string()).collect(), filters: vec![], smart_folder_id: None }
}

#[test]
fn the_account_set_is_canonical_and_the_single_field_is_its_fallback() {
    assert_eq!(query_accounts(q(None, &[])), None);
    assert_eq!(query_accounts(q(Some("acc-01"), &[])), Some(vec!["acc-01".to_string()]));
    assert_eq!(query_accounts(q(Some("acc-01"), &["acc-02", "acc-03"])), Some(vec!["acc-02".to_string(), "acc-03".to_string()]));
    assert!(account_in(&q(None, &["acc-02"]), "acc-02") && !account_in(&q(None, &["acc-02"]), "acc-01"));
    assert!(account_in(&q(None, &[]), "acc-09"));
}

#[test]
fn junk_trash_and_archive_carry_no_count_at_zero() {
    for m in [Mailbox::Junk, Mailbox::Trash, Mailbox::Archive] {
        assert_eq!(mailbox_count_shown(m, 0), None, "{m:?}");
        assert_eq!(mailbox_count_shown(m, 3), Some(3));
    }
    assert_eq!(mailbox_count_shown(Mailbox::Drafts, 0), Some(0));
    assert_eq!(mailbox_key(Mailbox::AllMail), "all mail");
}

#[test]
fn one_click_wins_then_mail_then_the_page() {
    let urls = vec!["<mailto:leave@list.example?subject=unsubscribe>".to_string(), "<https://list.example/u/1>".to_string()];
    assert_eq!(unsubscribe_method_from(urls.clone(), true), Some(UnsubscribeMethod::OneClick { url: "https://list.example/u/1".into() }));
    assert_eq!(unsubscribe_method_from(urls, false), Some(UnsubscribeMethod::Mail { address: "leave@list.example".into(), subject: Some("unsubscribe".into()) }));
    assert_eq!(unsubscribe_method_from(vec!["<http://list.example/prefs>".into()], true), Some(UnsubscribeMethod::Link { url: "http://list.example/prefs".into() }));
    assert_eq!(unsubscribe_method_from(vec![], true), None);
}

#[test]
fn an_account_removal_says_what_goes_and_what_stays() {
    assert_eq!(account_removal_words("Provanta RFP".into(), 12, 1, 0, 0), "Removes Provanta RFP from this Mac: 12 messages, 1 draft and its sign-in. Nothing is deleted on the server.");
    assert_eq!(account_removal_words("Side".into(), 0, 0, 0, 0), "Removes Side from this Mac: its sign-in. Nothing is deleted on the server.");
    assert_eq!(account_removal_words("W".into(), 1, 0, 2, 3), "Removes W from this Mac: 1 message, 2 waiting to send, 3 labels of its own and its sign-in. Nothing is deleted on the server.");
}

#[test]
fn a_label_the_owner_removed_is_never_filed_back_by_a_rule_or_the_model() {
    for by in [DecidedBy::Laya, DecidedBy::Jev, DecidedBy::Default, DecidedBy::Rule { rule_id: "r".into() }] {
        assert!(!may_file_label(by.clone(), true), "{by:?}");
        assert!(may_file_label(by, false));
    }
    assert!(may_file_label(DecidedBy::Owner, true), "the owner may put it back");
}

#[test]
fn the_list_unsubscribe_header_reads_in_order_and_one_click_needs_the_post_header() {
    let (u, one) = list_unsubscribe_from_headers(Some("<mailto:leave@l.example?subject=unsubscribe>,\r\n <https://l.example/u/1>"), Some("List-Unsubscribe=One-Click"));
    assert_eq!(u, vec!["<mailto:leave@l.example?subject=unsubscribe>".to_string(), "<https://l.example/u/1>".into()]);
    assert!(one);
    assert!(!list_unsubscribe_from_headers(Some("<https://l.example/u/1>"), None).1, "no post header, no one-click");
    assert_eq!(list_unsubscribe_from_headers(None, Some("List-Unsubscribe=One-Click")), (vec![], false));
    assert_eq!(unsubscribe_method_from(u, one), Some(UnsubscribeMethod::OneClick { url: "https://l.example/u/1".into() }));
}
