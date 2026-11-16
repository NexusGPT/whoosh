//! A host without JMAP answers FallbackToImap in the contract's words; real trouble stays an error.
mod common;

use common::*;
use contracts::CoreError;
use sync_jmap::{Discovery, JmapAdapter, JmapAuth, FALLBACK_WORDS};

async fn probe(mode: Mode) -> Result<Discovery<JmapAdapter>, CoreError> {
    let fake = Fake::start(mode, vec![FEmail::inbox(1)], 500).await;
    let out = JmapAdapter::connect(fake.config()).await;
    fake.shutdown().await;
    out
}

fn fallback_words(d: Result<Discovery<JmapAdapter>, CoreError>) -> String {
    match d.expect("a fallback is an answer, not an error") {
        Discovery::FallbackToImap { reason_words } => reason_words,
        Discovery::Jmap(_) => panic!("expected the fallback"),
    }
}

#[test]
fn the_contracts_words_are_the_journeys_words() {
    assert_eq!(FALLBACK_WORDS, "no JMAP: falls back to IMAP");
}

#[tokio::test]
async fn a_host_with_no_jmap_falls_back_to_imap() {
    let words = fallback_words(probe(Mode::NoJmap404).await);
    assert!(words.starts_with(FALLBACK_WORDS), "{words}");
    assert!(words.contains("404"), "the words say what the host answered: {words}");
}

#[tokio::test]
async fn a_web_page_instead_of_a_session_falls_back_to_imap() {
    assert!(fallback_words(probe(Mode::NotJson).await).starts_with(FALLBACK_WORDS));
}

#[tokio::test]
async fn a_session_without_mail_falls_back_to_imap() {
    let words = fallback_words(probe(Mode::NoMail).await);
    assert!(words.starts_with(FALLBACK_WORDS) && words.contains("no mail account"), "{words}");
}

#[tokio::test]
async fn nothing_listening_falls_back_to_imap() {
    let fake = Fake::start(Mode::Jmap, vec![], 500).await;
    let url = fake.base_url();
    fake.shutdown().await; // the port is closed again: a host that runs no JMAP service at all
    let out = JmapAdapter::connect(config_for(&url, JmapAuth::Bearer(TOKEN.into()))).await;
    assert!(fallback_words(out).starts_with(FALLBACK_WORDS));
}

#[tokio::test]
async fn a_refused_sign_in_is_an_error_naming_the_account_not_a_fallback() {
    let fake = Fake::start(Mode::Jmap, vec![FEmail::inbox(1)], 500).await;
    let out = JmapAdapter::connect(config_for(&fake.base_url(), JmapAuth::Bearer("wrong".into()))).await;
    fake.shutdown().await;
    let Err(CoreError::Action { failure }) = out else { panic!("expected an Action error, got {out:?}") };
    assert_eq!(failure.account_name, "Fastmail test");
    assert!(failure.reason_words.contains("401"), "{}", failure.reason_words);
    assert!(!failure.retryable, "the same secret will be refused again");
}

#[tokio::test]
async fn a_busy_host_is_a_retryable_error_not_a_fallback() {
    let Err(CoreError::Action { failure }) = probe(Mode::Busy).await else { panic!("expected an Action error") };
    assert!(failure.retryable);
    assert!(failure.reason_words.contains("503"), "{}", failure.reason_words);
}

#[tokio::test]
async fn a_host_that_offers_jmap_connects() {
    let d = probe(Mode::Jmap).await.expect("connect");
    assert!(matches!(d, Discovery::Jmap(_)));
}
