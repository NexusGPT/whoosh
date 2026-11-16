mod common;
use attachment_index::constants::EMBED_DIM;
use attachment_index::{AttachmentInput, AttachmentKind, IndexState, NOT_DOWNLOADED_WORDS, UNREADABLE_WORDS};
use common::{fixture, indexer};

fn input<'a>(att: &'a str, name: &'a str, mime: &'a str, path: Option<&'a std::path::Path>) -> AttachmentInput<'a> {
    AttachmentInput { message_id: "msg-1", attachment_id: att, file_name: name, mime_type: mime, local_path: path, now_ms: 1_000 }
}

#[test]
fn a_pdf_gives_its_text_and_text_vectors_under_its_message() {
    let s = tempfile::tempdir().unwrap();
    let p = fixture("plain.pdf");
    let e = indexer(s.path(), None).index(&input("att-pdf", "quote.pdf", "application/pdf", Some(&p)));
    assert_eq!(e.state, IndexState::Indexed);
    assert!(e.text.as_deref().unwrap().contains("Heavy matte stock"));
    assert_eq!(e.vectors.len(), 1);
    assert_eq!(e.vectors[0].key, "msg-1/att-pdf#t0");
    assert_eq!(e.vectors[0].vector.len(), EMBED_DIM);
    assert_eq!(e.to_content().vector_key.as_deref(), Some("msg-1/att-pdf"));
    assert!(e.to_content().embedded);
}

#[test]
fn a_pdf_the_core_cannot_read_goes_to_the_text_reader_door() {
    let s = tempfile::tempdir().unwrap();
    let p = fixture("scan.pdf");
    // the twin text reader knows no "scan.pdf": nothing inside, still indexed, nothing invented
    let e = indexer(s.path(), None).index(&input("att-scan", "scan.pdf", "application/pdf", Some(&p)));
    assert_eq!(e.state, IndexState::Indexed);
    assert_eq!(e.text, None);
    assert!(e.vectors.is_empty());
}

#[test]
fn a_document_is_read_by_the_system_reader_and_joins_the_word_text() {
    let s = tempfile::tempdir().unwrap();
    let p = fixture("report.docx");
    let e = indexer(s.path(), None).index(&input("att-doc", "report.docx", "application/vnd.openxmlformats-officedocument.wordprocessingml.document", Some(&p)));
    assert_eq!(e.kind, AttachmentKind::Document);
    assert!(e.word_text().contains("Atelier Merel"));
}

#[test]
fn an_image_is_embedded_and_its_words_and_labels_join_the_text() {
    let s = tempfile::tempdir().unwrap();
    let p = fixture("tiny.png");
    let e = indexer(s.path(), None).index(&input("att-img", "tiny.png", "image/png", Some(&p)));
    assert_eq!(e.kind, AttachmentKind::Image);
    assert_eq!(e.vectors.iter().map(|v| v.key.as_str()).collect::<Vec<_>>(), vec!["msg-1/att-img", "msg-1/att-img#o0"]);
    assert_eq!(e.ocr_text.as_deref(), Some("STOP"));
    assert_eq!(e.labels, vec!["red", "square"]);
    assert_eq!(e.word_text(), "STOP\nred\nsquare");
}

#[test]
fn an_audio_file_is_embedded_and_transcribed() {
    let s = tempfile::tempdir().unwrap();
    let p = fixture("clip.wav");
    let e = indexer(s.path(), None).index(&input("att-wav", "clip.wav", "audio/wav", Some(&p)));
    assert_eq!(e.transcript.as_deref(), Some("send the invoice on Friday"));
    assert!(e.vectors.iter().any(|v| v.key == "msg-1/att-wav"));
    assert!(e.vectors.iter().any(|v| v.key == "msg-1/att-wav#s0"));
}

#[test]
fn a_refused_speech_permission_is_reported_in_words_and_the_sound_is_still_embedded() {
    let s = tempfile::tempdir().unwrap();
    let p = fixture("clip.wav");
    let e = indexer(s.path(), Some("Speech recognition is off for Whoosh: allow it in System Settings, Privacy & Security.")).index(&input("att-wav", "clip.wav", "audio/wav", Some(&p)));
    assert_eq!(e.state, IndexState::Indexed);
    assert_eq!(e.transcript, None);
    assert_eq!(e.notes, vec!["Speech recognition is off for Whoosh: allow it in System Settings, Privacy & Security."]);
    assert_eq!(e.vectors.len(), 1);
}

#[test]
fn a_video_gives_two_frame_vectors_one_sound_vector_and_its_speech() {
    let s = tempfile::tempdir().unwrap();
    let p = fixture("clip.mp4");
    let e = indexer(s.path(), None).index(&input("att-video", "tour.mp4", "video/mp4", Some(&p)));
    let keys: Vec<&str> = e.vectors.iter().map(|v| v.key.as_str()).collect();
    assert_eq!(&keys[..3], &["msg-1/att-video#f0", "msg-1/att-video#f1", "msg-1/att-video#audio"]);
    assert_eq!(e.ocr_text.as_deref(), Some("Exit 4"));
    assert_eq!(e.transcript.as_deref(), Some("welcome to the print works"));
    assert!(!s.path().join("att-video").exists(), "the frames are removed after the video");
}

#[test]
fn a_file_not_downloaded_yet_says_so_and_holds_nothing() {
    let s = tempfile::tempdir().unwrap();
    let e = indexer(s.path(), None).index(&input("att-pdf", "scan-notes.pdf", "application/pdf", None));
    assert_eq!(e.state, IndexState::NotDownloaded);
    assert_eq!(e.row_words().as_deref(), Some(NOT_DOWNLOADED_WORDS));
    assert!(e.vectors.is_empty() && e.word_text().is_empty());
}

#[test]
fn a_kind_that_cannot_be_read_says_so_even_when_downloaded() {
    let s = tempfile::tempdir().unwrap();
    let p = fixture("plain.pdf");
    let e = indexer(s.path(), None).index(&input("att-dwg", "site-plan.dwg", "image/vnd.dwg", Some(&p)));
    assert_eq!(e.state, IndexState::Unreadable);
    assert_eq!(e.row_words().as_deref(), Some(UNREADABLE_WORDS));
    assert!(e.vectors.is_empty());
}

#[test]
fn a_model_that_returns_the_wrong_size_or_nothing_is_refused_and_the_entry_fails_in_words() {
    use attachment_index::twins::*;
    use attachment_index::{AttachmentIndexer, Limits, Readers};
    use contracts::{CoreError, Embedder, MediaKind};
    use std::sync::Arc;
    struct Empty;
    impl Embedder for Empty {
        fn dimension(&self) -> u32 { 512 }
        fn embed(&self, t: Vec<String>) -> Result<Vec<Vec<f32>>, CoreError> { Ok(t.iter().map(|_| vec![]).collect()) }
        fn embed_titled(&self, _title: Option<String>, texts: Vec<String>) -> Result<Vec<Vec<f32>>, CoreError> { self.embed(texts) } // mechanical (contracts delta 12)
        fn embed_query(&self, _: String) -> Result<Vec<f32>, CoreError> { Ok(vec![]) }
        fn embed_media(&self, _: MediaKind, _: String) -> Result<Vec<f32>, CoreError> { Ok(vec![0.0; 512]) }
    }
    let s = tempfile::tempdir().unwrap();
    let idx = AttachmentIndexer::new(
        Arc::new(Empty),
        Readers { text: Arc::new(TwinText::default()), image: Arc::new(TwinImage::default()), speech: Arc::new(TwinSpeech::default()), probe: Arc::new(TwinProbe { duration_s: 1.0, has_sound: false }) },
        Limits { text_max_chars: None, transcript_max_chars: None },
        s.path().to_path_buf(),
    );
    let p = fixture("tiny.png");
    let e = idx.index(&input("att-img", "tiny.png", "image/png", Some(&p)));
    assert!(e.vectors.is_empty());
    assert!(matches!(e.state, IndexState::Failed { .. }), "{:?}", e.state);
}

#[test]
fn a_cap_on_text_keeps_the_start() {
    let s = tempfile::tempdir().unwrap();
    let p = fixture("plain.pdf");
    let idx = common::indexer_with(s.path(), None, attachment_index::Limits { text_max_chars: Some(9), transcript_max_chars: None });
    let e = idx.index(&input("att-pdf", "quote.pdf", "application/pdf", Some(&p)));
    assert_eq!(e.text.as_deref(), Some("Quarterly"));
}

#[test]
fn a_model_that_answers_for_fewer_passages_than_it_was_given_is_reported() {
    use attachment_index::twins::*;
    use attachment_index::{AttachmentIndexer, Limits, Readers};
    use contracts::{CoreError, Embedder, MediaKind};
    use std::sync::Arc;
    struct Short;
    impl Embedder for Short {
        fn dimension(&self) -> u32 { 512 }
        fn embed(&self, _: Vec<String>) -> Result<Vec<Vec<f32>>, CoreError> { Ok(vec![]) }
        fn embed_titled(&self, _title: Option<String>, texts: Vec<String>) -> Result<Vec<Vec<f32>>, CoreError> { self.embed(texts) } // mechanical (contracts delta 12)
        fn embed_query(&self, _: String) -> Result<Vec<f32>, CoreError> { Ok(vec![]) }
        fn embed_media(&self, _: MediaKind, _: String) -> Result<Vec<f32>, CoreError> { Ok(vec![1.0; 512]) }
    }
    let s = tempfile::tempdir().unwrap();
    let idx = AttachmentIndexer::new(
        Arc::new(Short),
        Readers { text: Arc::new(TwinText::default()), image: Arc::new(TwinImage::default()), speech: Arc::new(TwinSpeech::default()), probe: Arc::new(TwinProbe { duration_s: 1.0, has_sound: false }) },
        Limits { text_max_chars: None, transcript_max_chars: None },
        s.path().to_path_buf(),
    );
    let p = fixture("plain.pdf");
    let e = idx.index(&input("att-pdf", "quote.pdf", "application/pdf", Some(&p)));
    assert!(e.text.is_some());
    assert!(e.vectors.is_empty());
    assert!(e.notes.iter().any(|n| n.contains("fewer passages")), "{:?}", e.notes);
}
