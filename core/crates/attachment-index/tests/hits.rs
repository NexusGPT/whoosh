use attachment_index::{hits_for_name, hits_for_text, hits_for_vectors, merge_attachment_hits, CatalogRow, IndexState, VectorMatch, NOT_DOWNLOADED_WORDS, UNREADABLE_WORDS};
use contracts::MatchedIn;

fn corpus() -> Vec<CatalogRow> {
    let row = |m: &str, a: &str, f: &str, s: IndexState, t: &str| CatalogRow { message_id: m.into(), attachment_id: a.into(), file_name: f.into(), state: s, text: t.into() };
    vec![
        row("msg-s02", "a-photo", "warehouse-photo.jpg", IndexState::Indexed, "pallets of brochures, a delivery note reading Q1 brochure \u{b7} 12,000, a forklift in the aisle"),
        row("msg-s06", "a-scan", "scan-notes.pdf", IndexState::NotDownloaded, ""),
        row("msg-s07", "a-dwg", "site-plan.dwg", IndexState::Unreadable, ""),
    ]
}

#[test]
fn a_vector_match_on_the_photo_names_the_attachment_and_its_message() {
    let m = vec![VectorMatch { key: "msg-s02/a-photo".into(), score: 0.7 }, VectorMatch { key: "msg-s02/a-photo#o0".into(), score: 0.9 }];
    let h = hits_for_vectors(&m, &corpus());
    assert_eq!(h.len(), 1, "one row per attachment");
    assert_eq!((h[0].message_id.as_str(), h[0].attachment_name.as_deref()), ("msg-s02", Some("warehouse-photo.jpg")));
    assert_eq!(h[0].matched_in, MatchedIn::AttachmentContent);
    assert!((h[0].score - 0.9).abs() < 1e-6, "the best part's score");
}

#[test]
fn a_vector_key_of_a_message_not_in_the_catalog_is_dropped() {
    assert!(hits_for_vectors(&[VectorMatch { key: "msg-x/a-1".into(), score: 0.9 }], &corpus()).is_empty());
}

#[test]
fn words_inside_the_attachment_text_are_a_content_hit() {
    let h = hits_for_text("forklift pallets in the aisle", &corpus());
    assert_eq!(h.len(), 1);
    assert_eq!((h[0].message_id.as_str(), &h[0].matched_in, h[0].attachment_name.as_deref()), ("msg-s02", &MatchedIn::AttachmentContent, Some("warehouse-photo.jpg")));
    assert!(h[0].snippet.contains("forklift"));
}

#[test]
fn text_of_a_file_not_indexed_is_never_searched() {
    let mut c = corpus();
    c[1].text = "heavy matte stock".into();
    assert!(hits_for_text("heavy matte", &c).is_empty());
}

#[test]
fn a_name_hit_on_a_file_not_downloaded_says_it_is_not_indexed() {
    let h = hits_for_name("scan notes", &corpus());
    assert_eq!(h.len(), 1);
    assert_eq!((h[0].message_id.as_str(), &h[0].matched_in, h[0].attachment_name.as_deref()), ("msg-s06", &MatchedIn::AttachmentName, Some("scan-notes.pdf")));
    assert_eq!(h[0].snippet, NOT_DOWNLOADED_WORDS);
}

#[test]
fn a_name_hit_on_an_unreadable_kind_says_name_only() {
    let h = hits_for_name("site plan", &corpus());
    assert_eq!(h.len(), 1);
    assert_eq!((h[0].message_id.as_str(), &h[0].matched_in), ("msg-s07", &MatchedIn::AttachmentName));
    assert_eq!(h[0].snippet, UNREADABLE_WORDS);
    assert!(h[0].snippet.contains("name only"));
}

#[test]
fn a_content_hit_wins_over_a_name_hit_for_the_same_attachment() {
    let c = corpus();
    let vec_hits = hits_for_vectors(&[VectorMatch { key: "msg-s02/a-photo".into(), score: 0.7 }], &c);
    let name_hits = hits_for_name("the warehouse photo", &c);
    assert_eq!(name_hits.len(), 1, "the name also matches");
    let merged = merge_attachment_hits(vec![name_hits, vec_hits]);
    assert_eq!(merged.len(), 1);
    assert_eq!(merged[0].matched_in, MatchedIn::AttachmentContent);
}

#[test]
fn a_query_of_filler_words_matches_nothing() {
    assert!(hits_for_name("the of in", &corpus()).is_empty());
    assert!(hits_for_text("the of in", &corpus()).is_empty());
}
