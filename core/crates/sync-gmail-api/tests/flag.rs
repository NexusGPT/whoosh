mod support;
use std::sync::atomic::{AtomicUsize, Ordering};
use support::*;
use sync_gmail_api::{GmailApi, GmailError, Parts, FLAG_NAME};

fn parts_counting(built: &'static AtomicUsize, replay: Replay, clock: Clock) -> impl FnOnce() -> Parts {
    move || {
        built.fetch_add(1, Ordering::SeqCst);
        Parts { transport: Box::new(replay), tokens: Box::new(FixedToken), pacer: Box::new(clock), account_name: "Fixture account".into() }
    }
}

#[test]
fn the_flag_has_the_name_the_signin_lane_and_the_plan_use() {
    assert_eq!(FLAG_NAME, "gmail_api_verified");
}

#[test]
fn flag_off_the_adapter_is_never_constructed() {
    static BUILT: AtomicUsize = AtomicUsize::new(0);
    let clock = Clock::new(T0);
    let replay = Replay::new(&clock);
    let result = GmailApi::open(&Flags(&[]), parts_counting(&BUILT, replay.clone(), clock.clone()));
    assert!(matches!(result, Err(GmailError::FlagOff)), "got an adapter with the flag off");
    assert_eq!(BUILT.load(Ordering::SeqCst), 0, "the transport, token source and pacer were made with the flag off");
    assert_eq!(replay.count(), 0, "a request was sent with the flag off");
}

#[test]
fn another_flag_on_does_not_switch_it_on() {
    static BUILT: AtomicUsize = AtomicUsize::new(0);
    let clock = Clock::new(T0);
    let replay = Replay::new(&clock);
    let result = GmailApi::open(&Flags(&["gmail_api_verified_staging", "graph_api"]), parts_counting(&BUILT, replay, clock));
    assert!(matches!(result, Err(GmailError::FlagOff)));
    assert_eq!(BUILT.load(Ordering::SeqCst), 0);
}

#[test]
fn flag_on_the_adapter_is_built_once() {
    static BUILT: AtomicUsize = AtomicUsize::new(0);
    let clock = Clock::new(T0);
    let replay = Replay::new(&clock);
    let api = GmailApi::open(&Flags(&["gmail_api_verified"]), parts_counting(&BUILT, replay.clone(), clock)).expect("on");
    assert_eq!(BUILT.load(Ordering::SeqCst), 1);
    assert_eq!(api.account_name(), "Fixture account");
    assert_eq!(replay.count(), 0, "building the adapter sends nothing");
}

#[test]
fn flag_off_error_reads_as_words_and_is_not_retryable() {
    let e = GmailError::FlagOff;
    let core = e.to_core_error("Fixture account");
    let text = core.to_string();
    assert!(text.starts_with("Fixture account: "), "{text}");
    assert!(!e.retryable());
}
