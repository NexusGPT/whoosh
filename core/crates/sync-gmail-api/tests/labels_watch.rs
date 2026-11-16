mod support;
use support::*;
use sync_gmail_api::limits::*;
use sync_gmail_api::{LabelKind, Watch};

const HOUR: i64 = 3_600_000;

#[test]
fn labels_come_back_with_system_and_user_kinds() {
    let clock = Clock::new(T0);
    let replay = Replay::from_files(&clock, &["labels.json"]);
    let labels = open(&replay, &clock).labels().expect("labels");
    assert_eq!(labels.len(), 3);
    assert_eq!((labels[0].id.as_str(), labels[0].kind), ("INBOX", LabelKind::System));
    assert_eq!((labels[2].id.as_str(), labels[2].name.as_str(), labels[2].kind), ("Label_12", "Receipts", LabelKind::User));
    assert_eq!(replay.paths(), vec!["labels"]);
}

#[test]
fn watch_posts_the_topic_and_renews_on_the_ledger_cadence() {
    let renew_h = ledger_number("GMAIL_WATCH_RENEW_H") as i64;
    assert_eq!(GMAIL_WATCH_RENEW_H as i64, renew_h);
    let clock = Clock::new(T0);
    let replay = Replay::from_files(&clock, &["watch.json"]);
    let api = open(&replay, &clock);
    let watch = api.start_watch("projects/example-project/topics/whoosh-relay", &["INBOX".to_string()]).expect("watch");

    let log = replay.requests();
    assert_eq!(log[0].request.path, "watch");
    let body: serde_json::Value = serde_json::from_str(log[0].request.json_body.as_ref().unwrap()).unwrap();
    assert_eq!(body["topicName"], "projects/example-project/topics/whoosh-relay");
    assert_eq!(body["labelIds"], serde_json::json!(["INBOX"]));

    assert_eq!(watch, Watch { history_id: "1234".into(), expires_at_ms: 1_794_304_800_000, renew_at_ms: T0 + renew_h * HOUR });
    // The renewal is due well before the seven-day expiry.
    assert!(watch.renew_at_ms < watch.expires_at_ms);
    clock.advance(renew_h * HOUR - 1);
    assert!(!api.watch_renew_due(&watch));
    clock.advance(1);
    assert!(api.watch_renew_due(&watch));
}

#[test]
fn a_watch_that_expires_sooner_than_the_cadence_renews_at_expiry() {
    let clock = Clock::new(T0);
    let replay = Replay::new(&clock);
    replay.push(serde_json::json!({"method": "POST", "path": "watch", "body": {"historyId": "1", "expiration": (T0 + HOUR).to_string()}}));
    let api = open(&replay, &clock);
    let watch = api.start_watch("projects/example-project/topics/whoosh-relay", &[]).unwrap();
    assert_eq!(watch.renew_at_ms, T0 + HOUR);
}

#[test]
fn the_crate_reads_the_ledger_number_not_its_own_copy() {
    assert_eq!(ledger_number("GMAIL_UNITS_PER_MIN"), GMAIL_UNITS_PER_MIN);
}
