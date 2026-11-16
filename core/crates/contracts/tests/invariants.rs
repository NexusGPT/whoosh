//! Logic probes for the invariants the types hold (see _captures/INVARIANT-PROBE-LOG.md).
use contracts::*;

#[test]
fn ax_ids_the_journeys_use_parse() {
    for s in ["mac-main.list", "mac-main.list.row.0", "dock.whoosh", "mac-main.tabs.edit.rename.needs-reply",
              "mac-settings-accounts.colour.2F6FED", "mac-main.accounts.acc-04", "mac-main.list.row.1.archive"] {
        let id = parse_ax_id(s.into()).unwrap_or_else(|e| panic!("{s}: {e}"));
        assert_eq!(format_ax_id(id), s, "round trip of {s}");
    }
    let id = parse_ax_id("mac-main.list.row.0".into()).unwrap();
    assert_eq!((id.surface.as_str(), id.region.as_str(), id.index), ("mac-main", "list", Some(0)));
}

#[test]
fn ax_ids_that_break_the_grammar_are_refused() {
    for s in ["", "mac-main", "Mac-main.list", "mac-main..row", "mac-main.list.", ".list", "mac main.list", "mac-main.-x"] {
        assert!(parse_ax_id(s.into()).is_err(), "{s:?} should be refused");
    }
}

#[test]
fn action_failed_names_account_and_reason() {
    let e = CoreError::Action { failure: ActionFailed { account_name: "Provanta RFP".into(), reason_words: "needs you to sign in again".into(), retryable: true, code: None } };
    assert_eq!(e.to_string(), "Provanta RFP: needs you to sign in again");
}

#[test]
fn a_label_path_holds_up_to_the_depth_limit() {
    let five = "Clients > Provanta > RFP > 2026 > Q4";
    assert_eq!(parse_label_path(five.into()).unwrap().len(), 5);
    assert_eq!(parse_label_path("Clients>Provanta".into()).unwrap(), vec!["Clients", "Provanta"]);
    assert_eq!(label_path(vec!["Clients".into(), "Provanta".into()]), "Clients > Provanta");
    let six = format!("{five} > Week 47");
    let e = parse_label_path(six).unwrap_err();
    assert!(e.to_string().contains("at most 5 levels"), "{e}");
    assert!(parse_label_path("Clients > > RFP".into()).is_err());
    assert!(parse_label_path("".into()).is_err());
}

#[test]
fn a_throttle_names_its_provider_and_wait() {
    let e = CoreError::Throttled { provider: "Gmail".into(), retry_after_ms: Some(1500) };
    assert_eq!(e.to_string(), "Gmail asks us to slow down; trying again in 2 s");
    match SyncError::from(e) {
        SyncError::Throttled { provider, retry_after_ms, .. } => assert_eq!((provider.as_str(), retry_after_ms), ("Gmail", Some(1500))),
        other => panic!("a throttle became {other:?}"),
    }
}
