//! gap-composer-rich: the message the core builds for a signature with formatting and a picture inside the text (src/rich.rs, wired in
//! src/compose.rs). Pure: `compose` builds the bytes, nothing is delivered. The single-edit broken copy each test must be red on is named
//! under it (docs/lanes/gap-composer-rich.md).
use contracts::{Draft, Signature};
use send::compose::{SIGNATURE_TAIL_LINE_MAX_CHARS, SIGNATURE_TAIL_MAX_LINES};
use send::rich::{rich_texts, signature_is_rich, INLINE_PICTURE_MAX_BYTES};
use send::{compose, OutgoingFile, Security, SendAccount, SmtpSettings};

const ADDRESS: &str = "assem@northlight-example.de";
const NOW_MS: i64 = 1_794_000_000_000;
const RICH_SIG: &str = "Assem\n**Whoosh** studio, *Paris*\n[our site](https://whoosh.example)";

fn account() -> SendAccount {
    SendAccount { id: "acc-nordlicht".into(), name: "Nordlicht".into(), address: ADDRESS.into(), smtp: SmtpSettings { host: "127.0.0.1".into(), port: 2525, security: Security::Plaintext, credentials: None } }
}
fn draft(body: &str, ids: &[&str]) -> Draft {
    Draft {
        id: "draft-rich-01".into(), from_account_id: "acc-nordlicht".into(), to: vec!["Linea Supply <orders@linea-example.com>".into()], cc: vec![], subject: "Rich".into(),
        body: body.into(), attachment_ids: ids.iter().map(|s| s.to_string()).collect(), ai_marks: vec![], reply_to_thread_id: None, bcc: vec![],
    }
}
fn sig(text: &str) -> Signature { Signature { address: ADDRESS.into(), text: text.into() } }
fn png() -> OutgoingFile { OutgoingFile { file_name: "chart.png".into(), media_type: "image/png".into(), bytes: vec![0x89, b'P', b'N', b'G', 13, 10, 26, 10, 0, 0, 0, 13] } }
fn pdf() -> OutgoingFile { OutgoingFile { file_name: "terms.pdf".into(), media_type: "application/pdf".into(), bytes: b"%PDF-1.4\n".to_vec() } }

fn built(body: &str, s: Option<&Signature>, ids: &[&str], files: &[OutgoingFile]) -> String {
    let c = compose(&account(), &draft(body, ids), s, NOW_MS, None, files).expect("the message is built");
    String::from_utf8_lossy(&c.rfc822).to_string()
}
/// Quoted-printable undone (soft breaks and =XX), so a long HTML line reads as it was written.
fn unqp(s: &str) -> String {
    let s = s.replace("=\r\n", "");
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'=' && i + 2 < b.len() && b[i + 1].is_ascii_hexdigit() && b[i + 2].is_ascii_hexdigit() {
            out.push(u8::from_str_radix(std::str::from_utf8(&b[i + 1..i + 3]).unwrap(), 16).unwrap());
            i += 3;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).to_string()
}
fn count(hay: &str, needle: &str) -> usize { hay.to_ascii_lowercase().matches(&needle.to_ascii_lowercase()).count() }

#[test]
fn a_signature_with_formatting_goes_out_as_html_beside_the_plain_text() {
    let raw = built("Thanks for the files.\n\nAssem\n**Whoosh** studio, *Paris*\n[our site](https://whoosh.example)", Some(&sig(RICH_SIG)), &[], &[]);
    assert_eq!(count(&raw, "Content-Type: multipart/alternative"), 1, "one multipart/alternative:\n{raw}");
    assert_eq!(count(&raw, "Content-Type: text/plain"), 1, "one plain part:\n{raw}");
    assert_eq!(count(&raw, "Content-Type: text/html"), 1, "one HTML part:\n{raw}");
    let html = unqp(&raw);
    assert!(html.contains("<b>Whoosh</b>"), "the bold of the signature is in the HTML:\n{html}");
    assert!(html.contains("<i>Paris</i>"), "the italic of the signature is in the HTML:\n{html}");
    assert!(html.contains("<a href=\"https://whoosh.example\">our site</a>"), "the link of the signature is in the HTML:\n{html}");
    let t = rich_texts("Thanks for the files.", Some(&sig(RICH_SIG)), &[], &[], "northlight-example.de").expect("rich");
    assert!(t.plain.starts_with("Thanks for the files."), "the plain part holds the text: {:?}", t.plain);
    assert!(t.plain.contains("Whoosh studio, Paris"), "the plain part holds the signature's words without marks: {:?}", t.plain);
    assert!(!t.plain.contains("**"), "no mark in the plain part: {:?}", t.plain);
    assert_eq!(t.html.matches("Whoosh").count(), 1, "the composer's appended signature is not sent twice: {}", t.html);
}
// broken copy: rich.rs signature_html `Mark::Bold => format!("<b>{}</b>", escape(&r.text)),` -> `Mark::Bold => escape(&r.text),`

#[test]
fn a_plain_signature_changes_nothing() {
    assert!(!signature_is_rich("Assem\nWhoosh studio"), "no mark, no rich signature");
    let raw = built("Thanks.", Some(&sig("Assem\nWhoosh studio")), &[], &[]);
    assert_eq!(count(&raw, "text/html"), 0, "a plain signature adds no HTML part:\n{raw}");
    assert_eq!(count(&raw, "multipart/"), 0, "a plain signature keeps the single plain body:\n{raw}");
    assert!(raw.contains("-- \r\nAssem\r\nWhoosh studio") || raw.contains("-- \nAssem\nWhoosh studio"), "the signature block as before:\n{raw}");
}
// broken copy: rich.rs signature_is_rich `signature_runs(text).iter().any(|r| r.mark != Mark::Plain)` -> `true`

#[test]
fn markup_typed_in_a_signature_never_reaches_the_html_as_a_tag() {
    let evil = "**Assem** <script>alert(1)</script> [x](javascript:alert(2)) <b onclick=\"y\">z</b>";
    let t = rich_texts("Hi.", Some(&sig(evil)), &[], &[], "northlight-example.de").expect("rich (it has a bold mark)");
    assert!(!t.html.to_ascii_lowercase().contains("<script"), "a typed <script> is never a tag: {}", t.html);
    assert!(t.html.contains("&lt;script&gt;alert(1)&lt;/script&gt;"), "it is escaped text: {}", t.html);
    assert!(!t.html.contains("javascript:alert(2)\""), "an unsafe link target is never an href: {}", t.html);
    assert!(!t.html.contains("onclick=\""), "a typed attribute is never markup: {}", t.html);
    // The text/plain part rightly shows the typed words as they were typed (plain text has no tags); the rule is about the HTML part of the sent bytes.
    let raw = unqp(&built("Hi.", Some(&sig(evil)), &[], &[]));
    let html = raw.split("Content-Type: text/html").nth(1).unwrap_or_else(|| panic!("the sent bytes have an HTML part:\n{raw}"));
    assert!(!html.to_ascii_lowercase().contains("<script"), "not in the HTML part of the sent bytes either:\n{raw}");
    assert!(html.contains("&lt;script&gt;alert(1)&lt;/script&gt;"), "it is escaped text in the HTML part of the sent bytes:\n{raw}");
}
// broken copy: rich.rs escape `'<' => o.push_str("&lt;"),` -> `'<' => o.push('<'),`

#[test]
fn a_picture_in_the_text_is_one_inline_part_the_html_references() {
    let raw = built("Here is the chart:\n[picture:att-1]\nSee you.", None, &["att-1"], &[png()]);
    assert_eq!(count(&raw, "Content-Type: multipart/related"), 1, "the HTML and its picture are multipart/related:\n{raw}");
    assert_eq!(count(&raw, "Content-Disposition: inline"), 1, "one inline part:\n{raw}");
    assert_eq!(count(&raw, "Content-Disposition: attachment"), 0, "the picture is not also an attachment:\n{raw}");
    let t = rich_texts("Here is the chart:\n[picture:att-1]\nSee you.", None, &["att-1".into()], &[png()], "northlight-example.de").expect("rich");
    assert_eq!(t.inline.len(), 1, "one picture: {:?}", t.inline);
    let cid = &t.inline[0].1;
    assert!(raw.contains(&format!("Content-ID: <{cid}>")), "the inline part carries the Content-ID {cid}:\n{raw}");
    assert!(unqp(&raw).contains(&format!("src=\"cid:{cid}\"")), "the HTML part references cid:{cid}:\n{raw}");
    assert!(t.plain.contains("[picture: chart.png]"), "the plain part has a fallback line: {:?}", t.plain);
    assert!(!t.plain.contains("[picture:att-1]"), "the marker never goes out: {:?}", t.plain);
}
// broken copy: rich.rs inline_of `marked.contains(*id) &&` -> `false &&`

#[test]
fn a_file_outside_the_text_stays_an_attachment_beside_an_inline_picture() {
    let raw = built("Chart:\n[picture:att-1]", None, &["att-1", "att-2"], &[png(), pdf()]);
    assert_eq!(count(&raw, "Content-Type: multipart/mixed"), 1, "files beside the text keep multipart/mixed:\n{raw}");
    assert_eq!(count(&raw, "Content-Disposition: inline"), 1, "the picture is inline:\n{raw}");
    assert_eq!(count(&raw, "Content-Disposition: attachment"), 1, "the pdf is an attachment as before:\n{raw}");
}
// broken copy: rich.rs rich_body `if others.is_empty() {` -> `if true {`

#[test]
fn a_marker_that_names_no_picture_of_the_draft_stays_text() {
    // an id the draft does not carry, and a file that is not a picture: no HTML, the plain path as before
    assert!(rich_texts("[picture:nope]", None, &[], &[], "d.example").is_none(), "an unknown id is text");
    assert!(rich_texts("[picture:att-2]", None, &["att-2".into()], &[pdf()], "d.example").is_none(), "a pdf is never inline");
    let raw = built("[picture:att-2]", None, &["att-2"], &[pdf()]);
    assert_eq!(count(&raw, "text/html"), 0, "no HTML part:\n{raw}");
}
// broken copy: rich.rs inline_of `&& f.media_type.trim().to_ascii_lowercase().starts_with("image/")` removed

#[test]
fn a_picture_over_the_ledgers_size_cap_stays_an_attachment_and_one_at_the_cap_goes_inline() {
    // the cap is the ledger's row (docs/CONSTANTS.md INLINE_PICTURE_MAX_BYTES), read through the generated constant, never a literal here
    assert_eq!(INLINE_PICTURE_MAX_BYTES, contracts::constants::INLINE_PICTURE_MAX_BYTES);
    let at_cap = OutgoingFile { file_name: "big.png".into(), media_type: "image/png".into(), bytes: vec![0u8; INLINE_PICTURE_MAX_BYTES as usize] };
    let over = OutgoingFile { file_name: "bigger.png".into(), media_type: "image/png".into(), bytes: vec![0u8; INLINE_PICTURE_MAX_BYTES as usize + 1] };
    let body = "Chart:\n[picture:att-1]";
    assert!(rich_texts(body, None, &["att-1".into()], &[at_cap], "d.example").is_some(), "a picture exactly at the cap goes inline");
    assert!(rich_texts(body, None, &["att-1".into()], &[over], "d.example").is_none(), "a picture one byte over the cap is a file, never inline: the marker stays text");
}
// broken copy: rich.rs inline_of ` && f.bytes.len() as u64 <= INLINE_PICTURE_MAX_BYTES` removed

#[test]
fn a_plain_signature_the_app_already_appended_is_sent_once_and_one_it_did_not_is_added_once() {
    let sg = sig("Assem\nWhoosh studio");
    // the app wrote "body", a blank line and the signature, with no "-- " line
    let appended = built("Thanks.\n\nAssem\nWhoosh studio", Some(&sg), &[], &[]);
    assert_eq!(count(&appended, "Whoosh studio"), 1, "already there: no second copy:\n{appended}");
    // the body already ends with the "-- " block
    let delimited = built("Thanks.\n\n-- \nAssem\nWhoosh studio", Some(&sg), &[], &[]);
    assert_eq!(count(&delimited, "Whoosh studio"), 1, "already there with its delimiter:\n{delimited}");
    // the body does not hold it: one copy is added
    let bare = built("Thanks.", Some(&sg), &[], &[]);
    assert_eq!(count(&bare, "Whoosh studio"), 1, "not there: added once:\n{bare}");
    // the same words ending a sentence are not the signature
    let inline = built("I work at Assem\nWhoosh studio", Some(&sg), &[], &[]);
    assert_eq!(count(&inline, "Whoosh studio"), 2, "words that only end the body are not a signature block:\n{inline}");
}
// broken copy: compose.rs split_signature, `if !starts_line || !ends_line {` -> `if false {`

// ---- lane fix-signature-order: the signature first, the footer line after it, one copy of the signature ----
const FOOT: &str = "Sent with Whoosh";
const SG: &str = "Assem\nWhoosh studio";
/// The sent text with quoted-printable and CRLF undone, so lines read as written.
fn mail(body: &str, s: Option<&Signature>) -> String { unqp(&built(body, s, &[], &[])).replace("\r\n", "\n") }
fn at(hay: &str, needle: &str) -> usize { hay.find(needle).unwrap_or_else(|| panic!("{needle:?} is missing from:\n{hay}")) }

#[test]
fn a_signature_followed_by_the_footer_is_sent_once_and_the_order_is_kept() {
    let sg = sig(SG);
    for body in [format!("Thanks.\n\n{SG}\n\n{FOOT}"), format!("Thanks.\n\n-- \n{SG}\n\n{FOOT}")] {
        let t = mail(&body, Some(&sg));
        assert_eq!(count(&t, "Whoosh studio"), 1, "one copy of the signature:\n{t}");
        assert_eq!(count(&t, FOOT), 1, "one footer:\n{t}");
        assert!(at(&t, "Thanks.") < at(&t, "Whoosh studio") && at(&t, "Whoosh studio") < at(&t, FOOT), "text, signature, footer:\n{t}");
    }
}
// broken copy: compose.rs split_signature, `lines.len() > SIGNATURE_TAIL_MAX_LINES` -> `!lines.is_empty()`

#[test]
fn a_signature_alone_is_sent_once_and_a_draft_with_neither_goes_out_as_written() {
    let sg = sig(SG);
    let t = mail(&format!("Thanks.\n\n{SG}"), Some(&sg));
    assert_eq!(count(&t, "Whoosh studio"), 1, "signature alone, once:\n{t}");
    let neither = mail("Thanks.", None);
    assert_eq!(count(&neither, "Thanks."), 1);
    assert!(!neither.contains("Whoosh studio") && neither.trim_end().ends_with("Thanks."), "no signature is invented:\n{neither}");
}
// broken copy: compose.rs with_signature, `if split_signature(trimmed, text).is_some() {` -> `if false {`

#[test]
fn a_footer_alone_gets_the_signature_after_it_because_the_engine_cannot_place_it_before_a_footer_it_does_not_know() {
    // What the engine can do: it adds the signature at the end. What it cannot: put it ABOVE a line it does not know is a footer.
    // So the app writes the signature itself whenever the footer is on (ComposerModel.assemble; Tests/Unit/ComposerSignatureOnceTests.swift).
    let t = mail(&format!("Thanks.\n\n{FOOT}"), Some(&sig(SG)));
    assert_eq!(count(&t, "Whoosh studio"), 1, "the signature is added once:\n{t}");
    assert!(at(&t, "Thanks.") < at(&t, FOOT) && at(&t, FOOT) < at(&t, "Whoosh studio"), "after the text and the footer, before nothing else:\n{t}");
}
// broken copy: compose.rs with_signature, `format!("{trimmed}\n\n{SIGNATURE_DELIMITER}{text}")` -> `format!("{trimmed}")`

#[test]
fn words_that_only_resemble_the_signature_still_get_the_signature() {
    let sg = sig(SG);
    let cases = [
        ("the last line alone", "Thanks.\n\nWhoosh studio".to_string()),
        ("the words ending a sentence", "I work at Assem\nWhoosh studio".to_string()),
        ("the words followed by more letters", format!("Thanks.\n\n{SG}s")),
        ("the words in the middle of a line", format!("Thanks.\n\nsee {SG}\n\n{FOOT}")),
    ];
    for (what, body) in cases {
        let t = mail(&body, Some(&sg));
        assert_eq!(count(&t, "Whoosh studio"), 2, "{what}: the words stay and the signature is added:\n{t}");
    }
}
// broken copies: compose.rs split_signature, `let starts_line = at == 0 || body.as_bytes()[at - 1] == b'\n';` -> `let starts_line = true;`
//                and `let ends_line = tail.is_empty() || tail.starts_with('\n') || tail.starts_with("\r\n");` -> `let ends_line = true;`

#[test]
fn what_follows_the_signature_is_a_footer_only_when_it_is_a_few_short_lines() {
    let sg = sig(SG);
    let few: Vec<String> = (0..SIGNATURE_TAIL_MAX_LINES).map(|i| format!("line {i}")).collect();
    let t = mail(&format!("Thanks.\n\n{SG}\n\n{}", few.join("\n")), Some(&sg));
    assert_eq!(count(&t, "Whoosh studio"), 1, "{SIGNATURE_TAIL_MAX_LINES} short lines after it are a footer:\n{t}");
    let many: Vec<String> = (0..=SIGNATURE_TAIL_MAX_LINES).map(|i| format!("line {i}")).collect();
    let t = mail(&format!("Thanks.\n\n{SG}\n\n{}", many.join("\n")), Some(&sg));
    assert_eq!(count(&t, "Whoosh studio"), 2, "one line more is text of the mail, so the signature is added:\n{t}");
    let long = "x".repeat(SIGNATURE_TAIL_LINE_MAX_CHARS + 1);
    let t = mail(&format!("Thanks.\n\n{SG}\n\n{long}"), Some(&sg));
    assert_eq!(count(&t, "Whoosh studio"), 2, "a line over {SIGNATURE_TAIL_LINE_MAX_CHARS} characters is text of the mail:\n{t}");
    let edge = "x".repeat(SIGNATURE_TAIL_LINE_MAX_CHARS);
    let t = mail(&format!("Thanks.\n\n{SG}\n\n{edge}"), Some(&sg));
    assert_eq!(count(&t, "Whoosh studio"), 1, "a line of exactly {SIGNATURE_TAIL_LINE_MAX_CHARS} characters is still a footer:\n{t}");
}
// broken copies: compose.rs split_signature, `lines.len() > SIGNATURE_TAIL_MAX_LINES` -> `false` (the many-lines case) and
//                `l.chars().count() > SIGNATURE_TAIL_LINE_MAX_CHARS` -> `false` (the long-line case)

#[test]
fn a_signature_of_several_lines_is_one_block_before_the_footer_and_a_part_of_it_is_not_the_signature() {
    let text = "Assem\nHead of studio\nWhoosh\nParis";
    let sg = sig(text);
    let t = mail(&format!("Thanks.\n\n{text}\n\n{FOOT}"), Some(&sg));
    assert_eq!(count(&t, "Head of studio"), 1, "the four-line signature once:\n{t}");
    assert!(at(&t, "Paris") < at(&t, FOOT), "the footer stays after it:\n{t}");
    let part = mail("Thanks.\n\nWhoosh\nParis", Some(&sg));
    assert_eq!(count(&part, "Head of studio"), 1, "only its last two lines stand in the body: the whole signature is added:\n{part}");
    assert_eq!(count(&part, "Paris"), 2);
}
// broken copy: compose.rs split_signature, `let text = sig_text.trim_end();` -> `let text = sig_text.trim_end().lines().last().unwrap_or("");`

#[test]
fn a_rich_signature_followed_by_the_footer_is_sent_once_with_the_footer_after_it() {
    let body = format!("Thanks.\n\n{RICH_SIG}\n\n{FOOT}");
    let t = rich_texts(&body, Some(&sig(RICH_SIG)), &[], &[], "northlight-example.de").expect("rich (it has marks)");
    assert_eq!(count(&t.plain, "Paris"), 1, "one signature in the plain part:\n{}", t.plain);
    assert_eq!(count(&t.html, "Paris"), 1, "one signature in the HTML part:\n{}", t.html);
    assert!(at(&t.plain, "Thanks.") < at(&t.plain, "Paris") && at(&t.plain, "Paris") < at(&t.plain, FOOT), "plain: text, signature, footer:\n{}", t.plain);
    assert!(at(&t.html, "Thanks.") < at(&t.html, "<b>Whoosh</b>") && at(&t.html, "<b>Whoosh</b>") < at(&t.html, FOOT), "html: text, signature, footer:\n{}", t.html);
}
// broken copies: rich.rs strip_signature, `match crate::compose::split_signature(body, sig) {` -> `match None::<(&str, &str)> {` (a second signature)
//                and rich.rs texts, `if !tail.is_empty() {` -> `if false {` (the footer lost)
