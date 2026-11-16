//! round 5: a blocked picture keeps its address for the view's "Load once", in `data-whoosh-src` only, on both paths (the engine's display
//! document and `sanitise_html`, which the app reads today through `parse::analyse`). The replacement and the count stay as they were.
//! Each test is seen red on the single-edit mutant its comment names.
use parse::{prepare, sanitise_html, Options, Parts, BLOCKED_IMAGE_SRC};

const PIC: &str = "https://pics.example.com/a.jpg?x=1&amp;y=2";
const SOURCE: &str = r#"<p>Hello</p><img src="https://pics.example.com/a.jpg?x=1&amp;y=2" width="600" height="300" alt="a">
<img src="https://t.example.net/p.gif" width="1" height="1">
<img src="https://t.example.net/q.gif">
<img src="https://pics.example.com/h.jpg" width="600" style="display:none">
<img src="javascript:alert(1)" width="600">
<img src="data:image/png;base64,AAAA" width="600">
<img src="//pics.example.com/r.jpg" width="600">
<img src="cid:x" data-whoosh-src="https://evil.example/t.gif" width="600">
<p data-whoosh-src="https://evil.example/p">x</p>"#;

fn both() -> Vec<(&'static str, String, u32)> {
    let s = sanitise_html(SOURCE);
    let r = prepare(&Parts { html: Some(SOURCE.into()), ..Default::default() }, &[], &Options::default());
    vec![("sanitise_html", s.html, s.trackers_blocked), ("display document", r.display_html, r.remote_images_blocked)]
}

/// The engine's display document keeps a picture's address in `data-whoosh-src`, once, escaped; the source still says blocked.
/// Mutant `r5-display-attr-dropped` (display.rs Cleaner: the `KEPT_SRC_ATTR` write skipped, `.filter(|_| false &&`): red.
#[test]
fn the_display_document_keeps_a_pictures_address_in_data_whoosh_src() {
    let r = prepare(&Parts { html: Some(SOURCE.into()), ..Default::default() }, &[], &Options::default());
    let h = &r.display_html;
    assert_eq!(h.matches(&format!("data-whoosh-src=\"{PIC}\"")).count(), 1, "{h}");
    assert_eq!(r.remote_images_blocked, 5, "the count is as before: every http, https and // image");
    assert_eq!(h.matches(&format!("src=\"{BLOCKED_IMAGE_SRC}\"")).count(), 8, "{h}"); // the seven non-cid images and the cid: image this message has no part for
}

/// `sanitise_html` (what the app reads today) does the same. Mutant `r5-sanitise-prepass-skipped` (sanitise.rs: `b.clean(html)` instead of
/// the marked text): red.
#[test]
fn sanitise_html_keeps_a_pictures_address_in_data_whoosh_src() {
    let s = sanitise_html(SOURCE);
    assert_eq!(s.html.matches(&format!("data-whoosh-src=\"{PIC}\"")).count(), 1, "{}", s.html);
    assert_eq!(s.trackers_blocked, 5);
}

/// A tracker (a stated size under 3 px, or hidden) stays counted and gets NO address.
/// Mutant `r5-tracker-test-off` (display.rs is_tracker: `false &&` before its last answer): red.
#[test]
fn a_tracker_is_counted_and_keeps_no_address() {
    for (path, h, count) in both() {
        assert_eq!(count, 5, "{path}");
        for t in ["p.gif", "h.jpg"] { assert!(!h.contains(t), "{path}: {t} in {h}"); }
    }
}

/// Only http and https are kept: never javascript:, data:, a protocol-relative address, nor a `data-whoosh-src` the sender wrote.
/// Mutant `r5-any-address-kept` (display.rs remote_address: `true ||` before the scheme test): red.
#[test]
fn only_http_and_https_addresses_are_kept_and_never_the_senders_own_attribute() {
    for (path, h, _) in both() {
        for bad in ["javascript", "data:image", "r.jpg", "evil.example"] { assert!(!h.contains(bad), "{path}: {bad} in {h}"); }
    }
}

/// Nothing makes a web view fetch the address by itself: it appears only as the value of `data-whoosh-src` (never in src, srcset,
/// background, href, poster or a style url()). Mutant `r5-sanitise-src-kept` (sanitise.rs: a remote `src` kept as written instead of the blocked source): red. (The same edit in the display path is caught by its allow-list too.)
#[test]
fn the_address_appears_only_inside_data_whoosh_src() {
    for (path, h, _) in both() {
        let all = h.matches("pics.example.com/a.jpg").count();
        let kept = h.matches("data-whoosh-src=\"https://pics.example.com/a.jpg").count();
        assert_eq!((all, kept), (1, 1), "{path}: {h}");
        let fetching = regex::Regex::new(r#"(?i)\s(src|srcset|background|poster|href|lowsrc|dynsrc)\s*=\s*["']?\s*(https?:)?//"#).expect("static");
        assert!(!fetching.is_match(&h) && !h.contains("url("), "{path}: {h}");
    }
}

// ---------------------------------------------------------------- the lead's ruling of 2026-11-11: an unsized image is a picture

fn one(h: &str) -> Vec<(&'static str, String, u32, u32)> {
    let s = sanitise_html(h);
    let r = prepare(&Parts { html: Some(h.into()), ..Default::default() }, &[], &Options::default());
    vec![("sanitise_html", s.html, s.pictures_blocked, s.pixel_trackers), ("display document", r.display_html, r.pictures_blocked, r.pixel_trackers)]
}

/// An image with no stated size is a picture: counted as one, its address kept. Mutant `r5-unsized-is-a-tracker` (display.rs is_tracker:
/// `dims.is_empty() ||` put back before the size test): red.
#[test]
fn an_unsized_image_is_a_picture_with_its_address() {
    for (path, h, pictures, trackers) in one(r#"<p>x</p><img src="https://pics.example.com/u.jpg">"#) {
        assert_eq!((pictures, trackers), (1, 0), "{path}");
        assert!(h.contains(r#"data-whoosh-src="https://pics.example.com/u.jpg""#), "{path}: {h}");
    }
}

/// A 1x1 image is a tracker with no address. Mutant `r5-stated-size-ignored` (display.rs is_tracker: the size test answers false): red.
#[test]
fn a_stated_one_by_one_image_is_a_tracker_with_no_address() {
    for (path, h, pictures, trackers) in one(r#"<p>x</p><img src="https://t.example.net/o.gif" width="1" height="1">"#) {
        assert_eq!((pictures, trackers), (0, 1), "{path}");
        assert!(!h.contains("o.gif"), "{path}: {h}");
    }
}

/// An image hidden by its style is a tracker with no address. Mutant `r5-hidden-ignored` (display.rs is_tracker: the hidden test never
/// answers): red.
#[test]
fn an_image_hidden_by_style_is_a_tracker_with_no_address() {
    for style in ["display:none", "visibility:hidden", "opacity:0", "width:0;height:0"] {
        for (path, h, pictures, trackers) in one(&format!(r#"<p>x</p><img src="https://t.example.net/s.gif" style="{style}">"#)) {
            assert_eq!((pictures, trackers), (0, 1), "{path}: {style}");
            assert!(!h.contains("s.gif"), "{path}: {style}: {h}");
        }
    }
}

/// The two counts on a mail with all three kinds (an unsized picture, a sized picture, a 1x1 and a hidden tracker); their sum is the old
/// count. Mutant `r5-counts-swapped` (display.rs image_src: the tracker branch counts a picture): red.
#[test]
fn the_two_counts_on_a_mail_with_all_three_kinds() {
    let h = r#"<p>x</p><img src="https://pics.example.com/u.jpg"><img src="https://pics.example.com/s.jpg" width="600"><img src="https://t.example.net/o.gif" width="1" height="1"><img src="https://t.example.net/h.gif" style="display:none">"#;
    for (path, out, pictures, trackers) in one(h) {
        assert_eq!((pictures, trackers), (2, 2), "{path}: {out}");
        assert_eq!(out.matches("data-whoosh-src=").count(), 2, "{path}: {out}");
    }
    assert_eq!(sanitise_html(h).trackers_blocked, 4, "the old count, every remote image, unchanged");
}
