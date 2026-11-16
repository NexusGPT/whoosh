//! The voice of a draft: the tone choice, the same-person examples, match-thread.
mod common;

use common::*;
use contracts::{CoreError, ReplyTone, ReplyUse, ReplyUseKind, ReplyVoice};
use reply_voice::{voice_for_draft, DraftContext, ExamplesFrom, SentQuery, VoiceLimits};

fn use_of(tone_id: &str, match_thread: bool) -> ReplyUse {
    ReplyUse { kind: ReplyUseKind::Auto, enabled: true, tone_id: tone_id.into(), match_thread, prompt_id: "replies".into() }
}

fn voice_of(text: &str, examples: u32) -> ReplyVoice {
    ReplyVoice { tone_text: text.into(), sample_size: 100, analysed_at_ms: Some(1), same_person_examples: examples }
}

const CTX: DraftContext<'static> = DraftContext { thread_id: "t-now", recipient: "claire@example.com" };

fn tones() -> Vec<ReplyTone> {
    vec![ReplyTone { id: "tone-formal".into(), name: "Formel".into(), text: "  Vouvoiement, phrases complètes.  ".into() }]
}

fn texts(v: &reply_voice::DraftVoice) -> Vec<&str> {
    v.examples.iter().map(|e| e.text.as_str()).collect()
}

// ---- the tone

#[test]
fn the_voice_tone_is_the_voices_text() {
    let out = voice_for_draft(&use_of("voice", false), &voice_of("  Je suis bref.\n", 0), &tones(), &CTX, &FakeSent::honest(vec![]), &VoiceLimits::default()).unwrap();
    assert_eq!((out.tone_id.as_str(), out.tone_text.as_str()), ("voice", "Je suis bref."));
}

#[test]
fn another_tone_is_that_tones_text() {
    let out = voice_for_draft(&use_of("tone-formal", false), &voice_of("Je suis bref.", 0), &tones(), &CTX, &FakeSent::honest(vec![]), &VoiceLimits::default()).unwrap();
    assert_eq!((out.tone_id.as_str(), out.tone_text.as_str()), ("tone-formal", "Vouvoiement, phrases complètes."));
}

#[test]
fn a_voice_never_analysed_gives_an_empty_tone_not_an_invented_one() {
    let out = voice_for_draft(&use_of("voice", false), &voice_of("", 0), &tones(), &CTX, &FakeSent::honest(vec![]), &VoiceLimits::default()).unwrap();
    assert_eq!(out.tone_text, "");
}

#[test]
fn an_unknown_tone_is_not_found() {
    let err = voice_for_draft(&use_of("tone-gone", false), &voice_of("x", 0), &tones(), &CTX, &FakeSent::honest(vec![]), &VoiceLimits::default()).unwrap_err();
    assert_eq!(err, CoreError::NotFound { kind: "tone".into(), id: "tone-gone".into() });
}

// ---- same-person examples

fn folder() -> Vec<reply_voice::SentMessage> {
    vec![
        msg("t-1", "claire@example.com", 1000, "Oldest to Claire."),
        msg("t-2", "claire@example.com", 3000, "Newest to Claire."),
        msg("t-3", "claire@example.com", 2000, "Middle to Claire.\n-- \nAssem"),
        msg("t-4", "bob@example.com", 4000, "To Bob, not Claire."),
    ]
}

#[test]
fn same_person_examples_are_the_newest_replies_to_that_person_up_to_the_setting() {
    let sent = FakeSent::honest(folder());
    let out = voice_for_draft(&use_of("voice", false), &voice_of("v", 2), &[], &CTX, &sent, &VoiceLimits::default()).unwrap();
    assert_eq!(out.examples_from, ExamplesFrom::SamePerson);
    assert_eq!(texts(&out), vec!["Newest to Claire.", "Middle to Claire."]);
    assert_eq!(sent.asked(), vec![(SentQuery::ToRecipient { recipient: "claire@example.com".into() }, 2)]);
}

#[test]
fn a_careless_store_that_gives_more_than_asked_is_cut_to_the_setting() {
    let sent = FakeSent::careless(folder());
    let out = voice_for_draft(&use_of("voice", false), &voice_of("v", 2), &[], &CTX, &sent, &VoiceLimits::default()).unwrap();
    assert_eq!(texts(&out), vec!["Newest to Claire.", "Middle to Claire."]);
}

#[test]
fn a_setting_of_zero_asks_the_store_for_nothing() {
    let sent = FakeSent::honest(folder());
    let out = voice_for_draft(&use_of("voice", true), &voice_of("v", 0), &[], &CTX, &sent, &VoiceLimits::default()).unwrap();
    assert_eq!((out.examples_from, out.examples.len()), (ExamplesFrom::Nothing, 0));
    assert!(sent.asked().is_empty());
}

#[test]
fn a_careless_store_cannot_put_another_persons_replies_in() {
    let sent = FakeSent::careless(folder());
    let out = voice_for_draft(&use_of("voice", false), &voice_of("v", 10), &[], &CTX, &sent, &VoiceLimits::default()).unwrap();
    assert_eq!(texts(&out), vec!["Newest to Claire.", "Middle to Claire.", "Oldest to Claire."]);
}

#[test]
fn the_recipient_is_matched_in_any_case_and_among_several_recipients() {
    let mut m = msg("t-1", "x@example.com", 1000, "Sent to two people.");
    m.recipients.push("  Claire@Example.COM ".into());
    let sent = FakeSent::careless(vec![m]);
    let out = voice_for_draft(&use_of("voice", false), &voice_of("v", 3), &[], &CTX, &sent, &VoiceLimits::default()).unwrap();
    assert_eq!(texts(&out), vec!["Sent to two people."]);
}

#[test]
fn each_example_is_stripped_and_cut_to_its_budget_and_an_empty_one_is_dropped() {
    let sent = FakeSent::honest(vec![
        msg("t-1", "claire@example.com", 3000, "On Mon, Nov 3, 2026 at 10:14 AM Claire <claire@example.com> wrote:\n> only a quote\n"),
        msg("t-2", "claire@example.com", 2000, "one two three four five six seven eight nine ten\n\n> quoted\n"),
        msg("t-3", "claire@example.com", 1000, "short"),
    ]);
    let limits = VoiceLimits { analysis_chars: 1_000, example_chars: 20 };
    let out = voice_for_draft(&use_of("voice", false), &voice_of("v", 5), &[], &CTX, &sent, &limits).unwrap();
    assert_eq!(texts(&out), vec!["one two three four…", "short"]);
    assert!(out.examples.iter().all(|e| e.text.chars().count() <= 20));
}

#[test]
fn never_wrote_to_that_person_gives_no_examples() {
    let sent = FakeSent::honest(vec![msg("t-4", "bob@example.com", 4000, "To Bob.")]);
    let out = voice_for_draft(&use_of("voice", false), &voice_of("v", 3), &[], &CTX, &sent, &VoiceLimits::default()).unwrap();
    assert_eq!((out.examples_from, out.examples.len()), (ExamplesFrom::Nothing, 0));
}

// ---- match thread

fn thread_folder() -> Vec<reply_voice::SentMessage> {
    vec![
        msg("t-now", "claire@example.com", 5000, "My second reply in this thread.\n\nOn Mon Claire <claire@example.com> wrote:\n> hi\n"),
        msg("t-now", "claire@example.com", 4000, "My first reply in this thread."),
        msg("t-old", "claire@example.com", 3000, "An older thread, same person."),
    ]
}

#[test]
fn match_thread_on_and_the_owner_wrote_in_the_thread_uses_those_replies_instead() {
    let sent = FakeSent::honest(thread_folder());
    let out = voice_for_draft(&use_of("voice", true), &voice_of("v", 3), &[], &CTX, &sent, &VoiceLimits::default()).unwrap();
    assert_eq!(out.examples_from, ExamplesFrom::Thread);
    assert_eq!(texts(&out), vec!["My second reply in this thread.", "My first reply in this thread."]);
    // instead: the same-person query is never made
    assert_eq!(sent.asked(), vec![(SentQuery::InThread { thread_id: "t-now".into() }, 3)]);
}

#[test]
fn match_thread_off_ignores_the_thread_and_uses_the_same_person() {
    let sent = FakeSent::honest(thread_folder());
    let out = voice_for_draft(&use_of("voice", false), &voice_of("v", 3), &[], &CTX, &sent, &VoiceLimits::default()).unwrap();
    assert_eq!(out.examples_from, ExamplesFrom::SamePerson);
    assert_eq!(out.examples.len(), 3);
    assert!(sent.asked().iter().all(|(q, _)| matches!(q, SentQuery::ToRecipient { .. })));
}

#[test]
fn match_thread_on_but_the_owner_never_wrote_in_it_falls_back_to_the_same_person() {
    let sent = FakeSent::honest(vec![msg("t-old", "claire@example.com", 3000, "An older thread, same person.")]);
    let out = voice_for_draft(&use_of("voice", true), &voice_of("v", 3), &[], &CTX, &sent, &VoiceLimits::default()).unwrap();
    assert_eq!(out.examples_from, ExamplesFrom::SamePerson);
    assert_eq!(texts(&out), vec!["An older thread, same person."]);
    assert_eq!(sent.asked().len(), 2);
}

#[test]
fn a_careless_store_cannot_put_another_threads_replies_in_the_thread_examples() {
    let sent = FakeSent::careless(thread_folder());
    let out = voice_for_draft(&use_of("voice", true), &voice_of("v", 10), &[], &CTX, &sent, &VoiceLimits::default()).unwrap();
    assert_eq!(out.examples_from, ExamplesFrom::Thread);
    assert_eq!(out.examples.len(), 2);
}

#[test]
fn thread_examples_are_cut_to_the_budget_too() {
    let sent = FakeSent::honest(vec![msg("t-now", "other@example.com", 4000, "alpha beta gamma delta epsilon zeta eta theta")]);
    let limits = VoiceLimits { analysis_chars: 1_000, example_chars: 15 };
    let out = voice_for_draft(&use_of("voice", true), &voice_of("v", 3), &[], &CTX, &sent, &limits).unwrap();
    assert_eq!(texts(&out), vec!["alpha beta…"]);
}

#[test]
fn a_store_that_fails_fails_the_voice_with_its_real_reason() {
    let mut sent = FakeSent::honest(folder());
    sent.fail_with = Some(CoreError::Unexpected { reason_words: "database is locked".into() });
    let err = voice_for_draft(&use_of("voice", false), &voice_of("v", 3), &[], &CTX, &sent, &VoiceLimits::default()).unwrap_err();
    assert_eq!(err, CoreError::Unexpected { reason_words: "database is locked".into() });
}
