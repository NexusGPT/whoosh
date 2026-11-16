//! Contract delta 12: the server code beside a failed send's words.
use contracts::smtp_failure_code;

#[test]
fn a_reply_code_and_its_enhanced_status_read_as_one_code() {
    assert_eq!(smtp_failure_code("451", "4.7.1 Try again later").as_deref(), Some("451 4.7.1"));
    assert_eq!(smtp_failure_code("550", "550-5.1.1 The email account that you tried to reach does not exist").as_deref(), Some("550 5.1.1"));
    assert_eq!(smtp_failure_code("554", "554 5.7.1 Message rejected").as_deref(), Some("554 5.7.1"));
}

#[test]
fn without_an_enhanced_status_only_the_reply_code_shows() {
    assert_eq!(smtp_failure_code("421", "Service not available").as_deref(), Some("421"));
    assert_eq!(smtp_failure_code("451", "").as_deref(), Some("451"));
    // an enhanced status of another class is not this reply's
    assert_eq!(smtp_failure_code("451", "5.7.1 nope").as_deref(), Some("451"));
    assert_eq!(smtp_failure_code("451", "4.7 short").as_deref(), Some("451"));
}

#[test]
fn a_success_or_a_malformed_reply_code_gives_no_code() {
    assert_eq!(smtp_failure_code("250", "2.0.0 OK"), None);
    assert_eq!(smtp_failure_code("45", "4.7.1"), None);
    assert_eq!(smtp_failure_code("4x1", "4.7.1"), None);
}

#[test]
fn an_action_failure_without_a_code_is_still_whole() {
    let f = contracts::ActionFailed { account_name: "Nordlicht".into(), reason_words: "the server is not reachable".into(), retryable: true, code: None };
    assert!(f.code.is_none() && !f.reason_words.is_empty());
}
