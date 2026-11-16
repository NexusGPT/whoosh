use attachment_index::constants::{VIDEO_FRAMES_MAX, VIDEO_FRAME_EVERY_S};
use attachment_index::{frame_times, video_plan, AttachmentKind, MediaPart};
use contracts::MediaKind;
use std::path::Path;

#[test]
fn a_twenty_second_video_gives_two_frames_one_per_ten_seconds() {
    assert_eq!(VIDEO_FRAME_EVERY_S, 10);
    assert_eq!(frame_times(20.0), vec![0.0, 10.0]);
}

#[test]
fn a_short_or_unmeasured_video_still_gives_one_frame() {
    assert_eq!(frame_times(3.0), vec![0.0]);
    assert_eq!(frame_times(0.0), vec![0.0]);
    assert_eq!(frame_times(f64::NAN), vec![0.0]);
}

#[test]
fn a_long_video_stops_at_the_cap_and_is_spread_to_its_end() {
    let t = frame_times(3600.0);
    assert_eq!(t.len(), VIDEO_FRAMES_MAX);
    assert_eq!(t[0], 0.0);
    assert!(*t.last().unwrap() > 3400.0, "the last frame must be near the end: {:?}", t.last());
    assert!(t.windows(2).all(|w| w[1] > w[0]));
}

#[test]
fn a_video_is_planned_as_frames_then_its_sound() {
    let plan = video_plan(Path::new("/s/att1"), 20.0);
    let kinds: Vec<MediaKind> = plan.iter().map(|r| r.kind).collect();
    assert_eq!(kinds, vec![MediaKind::VideoFrame, MediaKind::VideoFrame, MediaKind::Audio]);
    assert_eq!(plan[1].part, MediaPart::Frame { index: 1, at_s: 10.0 });
    assert_eq!(plan.iter().map(|r| r.key_suffix()).collect::<Vec<_>>(), vec!["#f0", "#f1", "#audio"]);
}

#[test]
fn kinds_follow_the_mime_type_then_the_name() {
    let k = AttachmentKind::detect;
    assert_eq!(k("application/pdf", "a.bin"), AttachmentKind::Pdf);
    assert_eq!(k("application/octet-stream", "scan-notes.PDF"), AttachmentKind::Pdf);
    assert_eq!(k("image/jpeg", "warehouse-photo.jpg"), AttachmentKind::Image);
    assert_eq!(k("audio/x-m4a", "note.m4a"), AttachmentKind::Audio);
    assert_eq!(k("video/mp4", "tour.mp4"), AttachmentKind::Video);
    assert_eq!(k("application/vnd.openxmlformats-officedocument.wordprocessingml.document", "x.docx"), AttachmentKind::Document);
    assert_eq!(k("text/csv", "prices.csv"), AttachmentKind::PlainText);
    assert_eq!(k("image/vnd.dwg", "site-plan.dwg"), AttachmentKind::Unreadable);
    assert_eq!(k("application/zip", "all.zip"), AttachmentKind::Unreadable);
    assert_eq!(k("image/vnd.adobe.photoshop", "design.psd"), AttachmentKind::Unreadable);
    assert_eq!(k("application/x-whatever", "mystery"), AttachmentKind::Unreadable);
}
