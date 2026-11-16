//! The analysis: the cut to the budget and the count read, the refusal on an empty Sent folder, the failures in words, the language.
mod common;

use common::*;
use contracts::{CoreError, ReplyVoice, ReplyVoiceAnalysis, REPLY_BAD_SAMPLE_CODE};
use reply_voice::{analyse_voice, prepare_analysis, voice_instruction, AnalysisRequest, VoiceFailureKind};

fn req(sample_size: u32, budget_chars: usize) -> AnalysisRequest {
    AnalysisRequest { sample_size, language: "French".into(), budget_chars }
}

fn sent_of(texts: &[&str]) -> FakeSent {
    // date 1000 for the first, 2000 for the second ...: the LAST text is the newest
    FakeSent::honest(texts.iter().enumerate().map(|(i, t)| msg("t", "c@example.com", (i as i64 + 1) * 1000, t)).collect())
}

#[test]
fn an_analysis_returns_the_models_text_and_the_count_read() {
    let sent = sent_of(&["Bonjour Claire, merci.", "Salut, ok pour demain.", "Bien à vous."]);
    let writer = FakeWriter::answering("  Phrases courtes. Je vouvoie les clients.\n");
    let out = analyse_voice(&sent, &writer, &req(100, 1_000)).expect("analysis");
    assert_eq!(out.tone_text, "Phrases courtes. Je vouvoie les clients.");
    assert_eq!((out.read, out.of), (3, 3));
    assert_eq!(writer.calls(), 1);
}

#[test]
fn the_cut_takes_the_newest_whole_messages_and_reports_how_many_were_read() {
    // newest first: "cccccccccc"(10), "bbbbbbbbbb"(10), "aaaaaaaaaa"(10); room for two
    let sent = sent_of(&["aaaaaaaaaa", "bbbbbbbbbb", "cccccccccc"]);
    let writer = FakeWriter::answering("ok");
    let out = analyse_voice(&sent, &writer, &req(100, 25)).unwrap();
    assert_eq!((out.read, out.of), (2, 3));
    let texts: Vec<String> = writer.last().samples.into_iter().map(|s| s.text).collect();
    assert_eq!(texts, vec!["cccccccccc", "bbbbbbbbbb"]);
}

#[test]
fn a_budget_that_exactly_fits_reads_everything() {
    let sent = sent_of(&["aaaaaaaaaa", "bbbbbbbbbb"]);
    let out = analyse_voice(&sent, &FakeWriter::answering("ok"), &req(100, 20)).unwrap();
    assert_eq!((out.read, out.of), (2, 2));
}

#[test]
fn a_message_longer_than_what_is_left_is_skipped_whole_never_cut() {
    // newest first: 10, 50, 10; budget 25: the 50 does not fit, the older 10 does
    let sent = sent_of(&["aaaaaaaaaa", &"b".repeat(50), "cccccccccc"]);
    let writer = FakeWriter::answering("ok");
    let out = analyse_voice(&sent, &writer, &req(100, 25)).unwrap();
    assert_eq!((out.read, out.of), (2, 3));
    let texts: Vec<String> = writer.last().samples.into_iter().map(|s| s.text).collect();
    assert_eq!(texts, vec!["cccccccccc", "aaaaaaaaaa"]);
}

#[test]
fn when_every_message_is_longer_than_the_room_the_run_is_refused_and_the_model_not_called() {
    let sent = sent_of(&[&"a".repeat(50), &"b".repeat(60)]);
    let writer = FakeWriter::answering("ok");
    let err = analyse_voice(&sent, &writer, &req(100, 20)).unwrap_err();
    assert_eq!(err.kind, VoiceFailureKind::NothingFits);
    assert_eq!(writer.calls(), 0);
}

#[test]
fn the_newest_are_kept_whatever_order_the_store_answers_in() {
    let mut sent = sent_of(&["aaaaaaaaaa", "bbbbbbbbbb", "cccccccccc"]);
    sent.honest = false;
    sent.messages.sort_by_key(|m| m.date_ms); // a careless adapter hands them back oldest first
    let writer = FakeWriter::answering("ok");
    analyse_voice(&sent, &writer, &req(100, 25)).unwrap();
    let texts: Vec<String> = writer.last().samples.into_iter().map(|s| s.text).collect();
    assert_eq!(texts, vec!["cccccccccc", "bbbbbbbbbb"]);
}

#[test]
fn no_more_than_the_sample_size_is_taken_even_when_the_store_gives_more() {
    let mut sent = sent_of(&["a1", "a2", "a3", "a4", "a5"]);
    sent.honest = false;
    let writer = FakeWriter::answering("ok");
    let out = analyse_voice(&sent, &writer, &req(3, 1_000)).unwrap();
    assert_eq!((out.read, out.of), (3, 3));
    // the store was asked for the sample size
    assert_eq!(sent.asked()[0].1, 3);
}

#[test]
fn quotes_and_signatures_never_reach_the_model_and_a_bare_quote_is_not_counted() {
    let sent = FakeSent::honest(vec![
        msg("t", "c@example.com", 3000, "Thanks, will do.\n\nOn Mon, Nov 3, 2026 at 10:14 AM Claire <c@example.com> wrote:\n> long history here\n"),
        msg("t", "c@example.com", 2000, "Okay.\n-- \nAssem\nFounder\n"),
        msg("t", "c@example.com", 1000, "> only a quote\n"),
    ]);
    let writer = FakeWriter::answering("ok");
    let out = analyse_voice(&sent, &writer, &req(100, 1_000)).unwrap();
    assert_eq!((out.read, out.of), (2, 2));
    let texts: Vec<String> = writer.last().samples.into_iter().map(|s| s.text).collect();
    assert_eq!(texts, vec!["Thanks, will do.", "Okay."]);
}

#[test]
fn an_empty_sent_folder_is_refused_in_words_and_the_model_is_never_asked() {
    let sent = FakeSent::honest(vec![]);
    let writer = FakeWriter::answering("An invented voice.");
    let err = analyse_voice(&sent, &writer, &req(100, 1_000)).unwrap_err();
    assert_eq!(err.kind, VoiceFailureKind::NoSentMail);
    assert!(err.reason_words.contains("no sent mail"), "{}", err.reason_words);
    assert_eq!(writer.calls(), 0);
    assert_eq!(err.kind.code(), "voice_no_sent_mail");
    // the state the pane reads
    assert_eq!(ReplyVoiceAnalysis::from(err.clone()), ReplyVoiceAnalysis::Failed { reason_words: err.reason_words });
}

#[test]
fn a_sent_folder_of_nothing_but_quotes_is_as_empty_as_none() {
    let sent = FakeSent::honest(vec![msg("t", "c@example.com", 1000, "> hi\n"), msg("t", "c@example.com", 2000, "")]);
    let writer = FakeWriter::answering("An invented voice.");
    let err = analyse_voice(&sent, &writer, &req(100, 1_000)).unwrap_err();
    assert_eq!(err.kind, VoiceFailureKind::NoSentMail);
    assert_eq!(writer.calls(), 0);
}

#[test]
fn a_bad_sample_size_is_refused_with_the_contracts_words_before_any_read() {
    for n in [0, 1_001] {
        let sent = sent_of(&["hello"]);
        let err = prepare_analysis(&sent, &req(n, 1_000)).unwrap_err();
        assert_eq!(err.kind, VoiceFailureKind::BadSample, "{n}");
        assert_eq!(err.kind.code(), REPLY_BAD_SAMPLE_CODE);
        assert_eq!(err.reason_words, "an analysis reads between 1 and 1000 sent messages");
        assert!(sent.asked().is_empty(), "the store must not be read for {n}");
    }
}

#[test]
fn a_missing_language_is_refused_never_defaulted() {
    let sent = sent_of(&["hello"]);
    let err = prepare_analysis(&sent, &AnalysisRequest { sample_size: 10, language: "  ".into(), budget_chars: 100 }).unwrap_err();
    assert_eq!(err.kind, VoiceFailureKind::BadLanguage);
}

#[test]
fn the_language_reaches_the_model_and_its_instruction() {
    let sent = sent_of(&["hello"]);
    let writer = FakeWriter::answering("ok");
    analyse_voice(&sent, &writer, &req(10, 100)).unwrap();
    let r = writer.last();
    assert_eq!(r.language, "French");
    assert!(r.instruction.contains("in French"), "{}", r.instruction);
    assert!(voice_instruction("Dutch").contains("in Dutch"));
    assert!(!voice_instruction("Dutch").contains("French"));
}

#[test]
fn no_address_leaves_the_mac_for_the_analysis() {
    let sent = sent_of(&["hello"]);
    let writer = FakeWriter::answering("ok");
    analyse_voice(&sent, &writer, &req(10, 100)).unwrap();
    let dump = format!("{:?}", writer.last());
    assert!(!dump.contains("c@example.com"), "{dump}");
}

#[test]
fn a_store_that_cannot_be_read_fails_with_its_real_reason() {
    let mut sent = sent_of(&["hello"]);
    sent.fail_with = Some(CoreError::Unexpected { reason_words: "database is locked".into() });
    let err = analyse_voice(&sent, &FakeWriter::answering("ok"), &req(10, 100)).unwrap_err();
    assert_eq!(err.kind, VoiceFailureKind::MailUnreadable);
    assert_eq!(err.reason_words, "database is locked");
}

#[test]
fn a_model_that_fails_fails_with_its_real_reason() {
    let sent = sent_of(&["hello"]);
    let writer = FakeWriter::failing(CoreError::Unexpected { reason_words: "401: the key was refused".into() });
    let err = analyse_voice(&sent, &writer, &req(10, 100)).unwrap_err();
    assert_eq!(err.kind, VoiceFailureKind::ModelFailed);
    assert_eq!(err.reason_words, "401: the key was refused");
}

#[test]
fn a_blank_model_answer_is_a_failure_never_a_stored_voice() {
    let sent = sent_of(&["hello"]);
    let err = analyse_voice(&sent, &FakeWriter::answering(" \n "), &req(10, 100)).unwrap_err();
    assert_eq!(err.kind, VoiceFailureKind::ModelWroteNothing);
}

#[test]
fn prepare_tells_how_many_will_be_read_before_the_model_is_called() {
    let sent = sent_of(&["aaaaaaaaaa", "bbbbbbbbbb", "cccccccccc"]);
    let p = prepare_analysis(&sent, &req(100, 25)).unwrap();
    assert_eq!(p.running(), ReplyVoiceAnalysis::Running { read: 2, of: 3 });
    assert_eq!(p.request().samples.len(), 2);
}

#[test]
fn into_voice_stores_the_count_read_the_time_and_keeps_the_other_setting() {
    let sent = sent_of(&["aaaaaaaaaa", "bbbbbbbbbb", "cccccccccc"]);
    let out = analyse_voice(&sent, &FakeWriter::answering("Je suis bref."), &req(100, 25)).unwrap();
    let before = ReplyVoice { tone_text: "old".into(), sample_size: 100, analysed_at_ms: None, same_person_examples: 4 };
    let after = out.into_voice(5_000, &before);
    assert_eq!(after, ReplyVoice { tone_text: "Je suis bref.".into(), sample_size: 2, analysed_at_ms: Some(5_000), same_person_examples: 4 });
}
