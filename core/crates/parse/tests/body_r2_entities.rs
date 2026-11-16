//! body-r2: a message's text and preview hold no HTML character reference. The cases come from the owner's installed app: a mail
//! from noreply@mywellness.example showed "Bonjour&nbsp;Assem, Ceci est un rappel" (9 of 58 stored messages held `&nbsp;`).
use parse::{analyse, parse_message};

fn mail(content_type: &str, body: &str) -> Vec<u8> {
    format!("From: Wellness <noreply@mywellness.example>\r\nTo: owner@example.test\r\nSubject: Rappel\r\nMessage-ID: <r2@mywellness.example>\r\nMIME-Version: 1.0\r\nContent-Type: {content_type}\r\n\r\n{body}\r\n").into_bytes()
}

fn html(body: &str) -> Vec<u8> { mail("text/html; charset=utf-8", body) }
fn plain(body: &str) -> Vec<u8> { mail("text/plain; charset=utf-8", body) }

fn text_and_preview(raw: &[u8]) -> (String, String) {
    let a = analyse(raw).unwrap();
    (a.message.text, a.preview)
}

/// Case 1, the owner's screenshot: an HTML mail whose text holds a non-breaking space between two words.
#[test]
fn case_1_a_non_breaking_space_in_html_is_a_normal_space_in_the_text_and_the_preview() {
    let (text, preview) = text_and_preview(&html("<html><body><p>Bonjour&nbsp;Assem,</p><p>Ceci est un rappel&nbsp;: votre s&eacute;ance commence bient&ocirc;t.</p></body></html>"));
    assert!(text.contains("Bonjour Assem,"), "{text:?}");
    assert!(!text.contains('&') && !text.contains('\u{a0}'), "{text:?}");
    assert_eq!(preview, "Bonjour Assem, Ceci est un rappel : votre s\u{e9}ance commence bient\u{f4}t.");
}

/// Case 2: the same words in a text/plain part the sender wrote HTML into.
#[test]
fn case_2_a_text_plain_part_with_entities_is_decoded() {
    let (text, preview) = text_and_preview(&plain("Bonjour&nbsp;Assem,\r\nCeci est un rappel&nbsp;: l&#39;&eacute;quipe &amp; vous &#x2014; &lt;3"));
    assert_eq!(text.trim(), "Bonjour Assem,\r\nCeci est un rappel : l'\u{e9}quipe & vous \u{2014} <3");
    assert_eq!(preview, "Bonjour Assem, Ceci est un rappel : l'\u{e9}quipe & vous \u{2014} <3");
}

/// Case 3, never decoded twice: `&amp;nbsp;` is the text "&nbsp;" in a mail about HTML, in both kinds of part.
#[test]
fn case_3_a_reference_is_decoded_once() {
    let (text, preview) = text_and_preview(&html("<p>Write &amp;nbsp; for a space and &amp;amp; for an ampersand.</p>"));
    assert!(text.contains("Write &nbsp; for a space and &amp; for an ampersand."), "{text:?}");
    assert_eq!(preview, "Write &nbsp; for a space and &amp; for an ampersand.");
    let (text, _) = text_and_preview(&plain("Write &amp;nbsp; for a space."));
    assert_eq!(text.trim(), "Write &nbsp; for a space.");
}

/// Case 4: numeric references (decimal, hex, the Windows-1252 block) and the ones a sender writes by habit.
#[test]
fn case_4_numeric_and_typographic_references_become_their_characters() {
    let (text, _) = text_and_preview(&html("<p>l&#8217;heure &#x2019; &#233;t&#233; 5&#150;6 &hellip; &copy; 2026 &euro;9 &rsquo;</p>"));
    assert!(text.contains("l\u{2019}heure \u{2019} \u{e9}t\u{e9} 5\u{2013}6 \u{2026} \u{a9} 2026 \u{20ac}9 \u{2019}"), "{text:?}");
}

/// Case 5: a multipart/alternative mail (the usual newsletter shape): text part with references, HTML part with references; the
/// preview and the stored text are the same in both orders of the parts, and a bare ampersand or an unknown name is left alone.
#[test]
fn case_5_a_multipart_mail_has_no_reference_left_in_either_part() {
    let body = "--b\r\nContent-Type: text/plain; charset=utf-8\r\n\r\nBonjour&nbsp;Assem, AT&T &amp; Fils&nbsp;&nosuchname;\r\n--b\r\nContent-Type: text/html; charset=utf-8\r\n\r\n<p>Bonjour&nbsp;Assem, AT&amp;T &amp; Fils</p>\r\n--b--";
    let m = parse_message(&mail("multipart/alternative; boundary=b", body)).unwrap();
    assert!(m.text.contains("Bonjour Assem, AT&T & Fils &nosuchname;"), "{:?}", m.text);
    assert!(!m.text.contains("&nbsp;") && !m.text.contains('\u{a0}'), "{:?}", m.text);
    assert!(m.html.as_deref().unwrap().contains("AT&amp;T"), "the html stays html: {:?}", m.html);
}

/// A preview never holds a reference, whatever the part: the list row and the reader read the same words.
#[test]
fn a_preview_holds_no_reference() {
    for raw in [html("<p>A&nbsp;B&nbsp;&nbsp;C &mdash; D</p>"), plain("A&nbsp;B&nbsp;&nbsp;C &mdash; D")] {
        let (_, preview) = text_and_preview(&raw);
        assert_eq!(preview, "A B C \u{2014} D");
    }
}

/// Owner's store, 13 Nov 2026: the preview of a forwarded mail began with 81 characters of dashes. A drawn line is no word: the preview
/// starts at the first word; an answer made of signs ("...", "???") and a short dash between words stay.
/// Mutant `preview-keeps-drawn-lines` (lib.rs preview_of: the `is_drawn_line` filter removed): red on the first three cases.
#[test]
fn a_preview_leaves_out_drawn_lines_and_keeps_signs_that_say_something() {
    use parse::{preview_of, preview_or_raw};
    assert_eq!(preview_of("---\n\n---------- Forwarded message ---------\nFrom: Facilities\nSubject: Room booking"), "Forwarded message From: Facilities Subject: Room booking");
    assert_eq!(preview_of("Hello\n________________________________\nTeam"), "Hello Team");
    assert_eq!(preview_of("=====\n\u{2500}\u{2500}\u{2500}\u{2500}\n*** ~~~ ###\nBudget"), "Budget");
    assert_eq!(preview_of("Budget 2026 - draft -- v2 \u{2014} final"), "Budget 2026 - draft -- v2 \u{2014} final", "one or two dashes between words are punctuation");
    assert_eq!(preview_of("..."), "...");
    assert_eq!(preview_of("???"), "???");
    assert_eq!(preview_of("---\n___"), "", "nothing but drawn lines: the caller falls back");
    assert_eq!(preview_or_raw("---\n___"), "--- ___", "a mail that is only a line shows it");
    assert_eq!(preview_or_raw("---\nFYI"), "FYI");
    let a = analyse(&plain("---\r\nSee the room booking below.\r\n__________\r\nEva")).unwrap();
    assert_eq!(a.preview, "See the room booking below. Eva");
}
