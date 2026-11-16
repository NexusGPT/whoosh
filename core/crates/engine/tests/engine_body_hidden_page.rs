//! Owner's picture 215 (14 Nov 2026): "some mails don't display tet properly". A support mail drew as a white card with one small picture
//! and no word. On the owner's store (counts only): its page puts 26 of its 27 words directly inside a column block set to `font-size:0`
//! (the sender's template lost the cell that sets the size again), while its text part holds 118 words. Of 291 stored mails two have a
//! page that shows under half the words of the text part: this one, and a mail built of 19 pictures 600 wide whose words are all under
//! `display:none` (it says it with its pictures and keeps its look).
//! Rule (`reads::apply_prepared`, `parse::page_hides_text`): a page that shows under one in PARSE_PAGE_HIDES_TEXT_SHARE of the words of a
//! text part of PARSE_PAGE_HIDES_TEXT_MIN_WORDS words or more, and no picture larger than an icon, is not served; the reader gets the
//! text part as the written page.
//!
//! MUTANTS (each turns the tests named red):
//!   H1 `rule-off`      reads.rs `apply_prepared`: the `page_hides_text` branch removed        -> a_page_that_hides_its_words_is_served_as_the_text_part, a_page_wholly_set_not_to_display_is_served_as_the_text_part
//!   H2 `size-ignored`  display.rs `shown_words`: `font-size` never read                         -> a_page_that_hides_its_words_is_served_as_the_text_part
//!   H3 `always`        display.rs `page_hides_text`: answers true for every page with a text part -> a_page_that_shows_its_words_is_served_as_it_is, a_hidden_copy_of_shown_words_does_not_switch_the_page
//!   H4 `pictures-ignored` display.rs `page_hides_text`: the picture count never read          -> a_page_built_of_pictures_keeps_its_look
//!   Picture 216 (run 14 Nov 2026, each red on the tests named, then put back):
//!   P1 `cut-tags-off`       pagecode.rs `close_cut_tags`: returns the page as it came                -> a_new_mail_with_page_code_in_its_text_part_is_kept_as_words, a_stored_text_part_of_page_code_is_served_as_the_page_it_is
//!   P2 `code-read-as-text`  pagecode.rs `text_is_page_code`: answers false                          -> the two above, a_mail_whose_only_part_is_page_code_gets_its_page
//!   P3 `zero-words-ignored` display.rs `page_hides_text`: the words at a size of zero never read    -> words_at_size_zero_that_outnumber_the_shown_ones_switch_the_page
//!   P4 `boxes-kept`         reads.rs `show_hidden_words`: serves the written document, boxes kept   -> the two of P1, words_at_size_zero_that_outnumber_the_shown_ones_switch_the_page
use std::time::Duration;

use contracts::BodyLayout;

const TEXT: &str = "Hello,\n\nThank you for writing to us. We have received your request and a member of our team will answer within one working day.\n\n\
You can add a note to the request at any time by answering this mail. Please keep the reference in the subject line.\n\nKind regards,\nThe support team\n";

fn mail(html: &str, text: &str) -> Vec<u8> {
    format!("From: Support <support@sender.example>\r\nTo: owner@example.test\r\nSubject: Request received\r\nMIME-Version: 1.0\r\n\
             Content-Type: multipart/alternative; boundary=\"b1\"\r\n\r\n--b1\r\nContent-Type: text/plain; charset=utf-8\r\n\r\n{text}\r\n\
             --b1\r\nContent-Type: text/html; charset=utf-8\r\n\r\n<html><body>{html}</body></html>\r\n--b1--\r\n").into_bytes()
}

/// The column of the picture's mail, by its structure: the words sit directly in a block at font-size 0, beside the table that lost them.
fn column(inner_open: &str, inner_close: &str) -> String {
    format!("<div style=\"font-size:14px;line-height:24px;color:#41415a\"><table width=\"100%\"><tr><td style=\"font-size:0;padding:16px;text-align:center\">\
             <div style=\"font-size:0;text-align:left;display:inline-block;vertical-align:top;width:100%\">{inner_open}<br>Thank you for writing to us. \
             <a href=\"https://support.sender.example/requests/1\">Your request</a> is with our team and will be answered within one working day.<br><br>Kind regards<br>The support team{inner_close}\
             <table border=\"0\" cellpadding=\"0\" cellspacing=\"0\"></table></div></td></tr></table></div>")
}

fn served(html: &str, text: &str) -> (parse::Prepared, contracts::MessageBody) {
    let raw = mail(html, text);
    let p = engine::bodies::prepare_within(&raw, Vec::new(), Duration::from_secs(30), Duration::ZERO);
    let page = parse::sanitise_html(html).html;
    let b = engine::reads::body_from_stored("m1", text.to_string(), Some(page), 0, Some(&engine::reads::prepared_json(&p)));
    (p, b)
}

#[test]
fn a_page_that_hides_its_words_is_served_as_the_text_part() {
    let (p, b) = served(&column("", ""), TEXT);
    assert!(p.display_html.contains("font-size:0"), "the case keeps its size-0 column: {}", p.display_html);
    assert_eq!(b.layout, Some(BodyLayout::Written), "the reader gets the written page");
    let page = b.display_html.as_deref().unwrap_or("");
    assert!(page.contains("a member of our team will answer within one working day"), "the text part's words are the page: {page}");
    assert!(!page.contains("font-size:0"), "nothing of the hiding page is left: {page}");
    assert_eq!(b.written_html.as_deref(), Some(page));
    assert_eq!(b.layout_reasons, vec!["page_hides_text".to_string()]);
    assert_eq!((b.segments.len(), b.fold_from), (1, None), "one shown part, nothing folded");
    assert_eq!(b.segments[0].safe_html, page);
    assert_eq!(b.pictures_blocked, 0, "the written page holds no picture");
}

#[test]
fn a_page_wholly_set_not_to_display_is_served_as_the_text_part() {
    let html = format!("<div style=\"display:none\">{}</div>", column("<div style=\"font-size:13px\">", "</div>"));
    let (_, b) = served(&html, TEXT);
    assert_eq!(b.layout, Some(BodyLayout::Written));
    assert!(b.display_html.as_deref().is_some_and(|h| h.contains("Please keep the reference in the subject line")));
}

#[test]
fn a_page_that_shows_its_words_is_served_as_it_is() {
    // the same column with the cell that sets the size again: the words are seen, the engine's own page and layout stand
    let (p, b) = served(&column("<div style=\"font-size:13px;line-height:22px\">", "</div>"), TEXT);
    assert_eq!(b.display_html.as_deref(), Some(p.display_html.as_str()));
    assert_eq!(b.layout.map(|l| l == BodyLayout::Designed), Some(p.layout == parse::Layout::Designed));
    assert!(!b.layout_reasons.contains(&"page_hides_text".to_string()));
}

#[test]
fn a_hidden_copy_of_shown_words_does_not_switch_the_page() {
    // a page that shows its words once and keeps a second copy hidden (a phone layout beside the desktop one): the shown copy is the mail
    let shown = column("<div style=\"font-size:13px\">", "</div>");
    let html = format!("{shown}<div style=\"display:none;max-height:0;overflow:hidden\">{shown}</div>");
    let (p, b) = served(&html, TEXT);
    assert_eq!(b.display_html.as_deref(), Some(p.display_html.as_str()));
}

#[test]
fn a_page_built_of_pictures_keeps_its_look() {
    // the second mail of the owner's store, by its structure: banners 600 wide say it, every word of the page is set not to display
    let banners: String = (0..3).map(|i| format!("<img src=\"https://pics.sender.example/slice-{i}.png\" width=\"600\" style=\"height:auto;display:block\">")).collect();
    let html = format!("<div style=\"display:none;font-size:1px;max-height:0\">{}</div><table width=\"600\"><tr><td>{banners}</td></tr></table>", column("", ""));
    let (p, b) = served(&html, TEXT);
    assert_eq!(parse::shown(&p.display_html).pictures, 3, "the case shows three pictures larger than an icon: {}", p.display_html);
    assert_eq!(b.display_html.as_deref(), Some(p.display_html.as_str()), "the page of pictures is served as it is");
    assert!(!b.layout_reasons.contains(&"page_hides_text".to_string()));
    // an icon is no picture: the mail of the picture (one 44 by 44 mark beside the hidden words) is still switched
    let icon = format!("<img src=\"https://pics.sender.example/mark.png\" width=\"44\" height=\"44\">{}", column("", ""));
    let (_, c) = served(&icon, TEXT);
    assert_eq!(c.layout, Some(BodyLayout::Written));
}

#[test]
fn a_short_text_part_never_switches_a_page_that_shows_its_words() {
    // under PARSE_PAGE_HIDES_TEXT_MIN_WORDS words the text part says too little to judge the page by
    let (p, b) = served(&column("<div style=\"font-size:13px\">", "</div>"), "Request received.\nThank you.\n");
    assert_eq!(b.display_html.as_deref(), Some(p.display_html.as_str()));
}

// ---- owner's picture 216 (14 Nov 2026): "well now it's off". The same mail, served by its text part, drew as a wall of page code: the
// sender's system writes the page's code into the text part too, and one tag of that code stops in the middle of a word, the sentence
// written straight after it. Rules: a text part of page code is read as its words (`parse::text_is_page_code`); a cut tag is ended where
// its code stops (`parse::close_cut_tags`); a page that hides its words is served by its own words in the app's type, its links kept,
// and by the text part only when it does not hold them (`reads::show_hidden_words`).

const URGENT: &str = "If you're a customer and your request is urgent, you can reach us faster through the messages of your account.";

/// The page code of that mail, by its structure. `cut`: the table of the second column stops in the middle of its code.
fn code(cut: bool) -> String {
    let table = if cut { "<table border=0 cellpadding=0 cellspacing=0 role=presentatio" } else { "<table border=0 cellpadding=0 cellspacing=0 role=presentation><tr><td style=font-size:13px>" };
    format!("<div style=background-color:#fff><div style=\"margin:0 auto;max-width:432px\"><table border=0 cellpadding=0 cellspacing=0 role=presentation style=width:100% align=center><tr><td style=\"direction:ltr;font-size:0;padding:64px 16px;text-align:center\">\
<div style=font-size:0;text-align:left;display:inline-block;vertical-align:top;width:100% class=\"col\"><table border=0 cellpadding=0 cellspacing=0 role=presentation width=100%><tr><td style=width:44px><a href=https://www.sender.example target=_blank><img height=44 src=https://pics.sender.example/mark.png style=border:0;display:block;height:44px;width:100% width=44></a></table></div>\
<div style=font-size:0;text-align:left;display:inline-block;vertical-align:top;width:100% class=\"col\">{table}{URGENT}<br><br>For anything else, visit our <a href=https://support.sender.example/help>Help page</a> or answer this mail and a member of our team will come back to you <a href=https://support.sender.example/hours>within one day</a>.<br><br>Thanks,<br>The support team</div></table></div></div>")
}
const LINES: &str = "\r\n\r\n--------------------------------\r\nThis mail is a service of the sender.\r\n\r\n\r\n\r\n[AB12CD-EFGH3]";

fn says_it_all(b: &contracts::MessageBody) {
    assert!(!b.plain_text.contains('<') && !b.plain_text.contains("cellpadding"), "the text is words, no code: {:?}", b.plain_text);
    for said in [URGENT, "visit our Help page or answer this mail", "The support team"] { assert!(b.plain_text.contains(said), "{said:?} is in the text: {:?}", b.plain_text); }
    assert_eq!(b.layout, Some(BodyLayout::Written));
    assert_eq!(b.layout_reasons, vec!["page_hides_text".to_string()]);
    let page = b.display_html.as_deref().unwrap_or("");
    assert_eq!(b.written_html.as_deref(), Some(page));
    assert!(page.contains(URGENT), "the sentence the cut tag took is on the page: {page}");
    assert!(page.contains("href=\"https://support.sender.example/help\""), "the page keeps its links: {page}");
    assert!(!page.contains("font-size") && !page.contains("&lt;") && !page.contains("cellpadding=0 cellspacing=0 role=presentatioIf"), "no size of zero, no code shown as words: {page}");
    for kept_box in ["padding:", "max-width", "text-align", "margin:", "margin-top", "margin-left", "inline-block", "width:100%", "width=\"100%\""] { assert!(!page.contains(kept_box), "the sender's boxes are left out ({kept_box}): {page}"); }
    assert_eq!((page.matches("margin-bottom:12px").count(), page.matches("<div style=\"margin-bottom:12px;\"><a ").count()), (1, 1), "the block of the mark keeps a space under it: {page}");
    assert!(!page.contains("<table") && !page.contains("<td") && !page.contains("<tr"), "the layout tables are plain blocks: {page}");
    assert!(page.contains("width=\"44\"") && page.contains("height:44px"), "the mark keeps its size in pixels: {page}");
    let (seen, want) = (parse::shown(page).words, b.plain_text.split_whitespace().count());
    assert!(seen * 100 >= want * 75, "the page shows the words of the mail: {seen} of {want}");
    assert_eq!((b.segments.len(), b.fold_from), (1, None));
}

#[test]
fn a_stored_text_part_of_page_code_is_served_as_the_page_it_is() {
    // what the store kept before this rule: the text part as it came (the code), the page as it was read then (the cut tag took the
    // sentence and the line break after it as its attributes), and the engine's record of that page
    let text = format!("{}{LINES}", code(true));
    let lost = code(true).replace(&format!("role=presentatio{URGENT}<br>"), "role=presentatio>");
    assert!(!lost.contains(URGENT));
    let p = engine::bodies::prepare_within(&mail(&lost, "kept before"), Vec::new(), Duration::from_secs(30), Duration::ZERO);
    let b = engine::reads::body_from_stored("m1", text, Some(parse::sanitise_html(&lost).html), 0, Some(&engine::reads::prepared_json(&p)));
    says_it_all(&b);
    assert!(b.plain_text.contains("This mail is a service of the sender.") && b.plain_text.contains("[AB12CD-EFGH3]"), "the lines under the code are text: {:?}", b.plain_text);
    assert!(b.display_html.as_deref().is_some_and(|h| h.contains("This mail is a service of the sender.")));
    assert!(b.safe_html.as_deref().is_some_and(|h| h.contains(URGENT)), "the stored page gave way to the whole one");
}

#[test]
fn a_new_mail_with_page_code_in_its_text_part_is_kept_as_words() {
    let raw = mail(&code(true), &format!("{}{LINES}", code(true)));
    let (text, html, trackers, preview) = engine::reads::parse_raw_with_preview(&raw).expect("a mail");
    assert!(!text.contains('<') && text.contains(URGENT), "the stored text is words: {text:?}");
    assert!(preview.starts_with("If you're a customer") && !preview.contains('<'), "the row's preview is words: {preview:?}");
    assert!(html.as_deref().is_some_and(|h| h.contains(URGENT)), "the stored page holds the sentence after the cut tag: {html:?}");
    let p = engine::bodies::prepare_within(&raw, Vec::new(), Duration::from_secs(30), Duration::ZERO);
    assert!(p.display_html.contains(URGENT), "the engine's page holds it too: {}", p.display_html);
    let before = engine::bodies::prepare_calls();
    let b = engine::reads::body_from_stored("m1", text, html, trackers, Some(&engine::reads::prepared_json(&p)));
    says_it_all(&b);
    assert_eq!(engine::bodies::prepare_calls(), before);
}

#[test]
fn a_mail_whose_only_part_is_page_code_gets_its_page() {
    let raw = format!("From: Support <support@sender.example>\r\nTo: owner@example.test\r\nSubject: Request received\r\nMIME-Version: 1.0\r\nContent-Type: text/plain; charset=utf-8\r\n\r\n{}{LINES}\r\n", code(false)).into_bytes();
    let (text, html, _, preview) = engine::reads::parse_raw_with_preview(&raw).expect("a mail");
    assert!(!text.contains('<') && text.contains(URGENT) && !preview.contains('<'), "words: {text:?} / {preview:?}");
    assert!(html.as_deref().is_some_and(|h| h.contains("href=\"https://support.sender.example/help\"") && h.contains("This mail is a service of the sender.")), "the code is the page: {html:?}");
    let p = engine::bodies::prepare_within(&raw, Vec::new(), Duration::from_secs(30), Duration::ZERO);
    assert!(p.display_html.contains("href=\"https://support.sender.example/help\"") && !p.display_html.contains("&lt;div"), "the engine draws the page, not its code: {}", p.display_html);
}

#[test]
fn words_at_size_zero_that_outnumber_the_shown_ones_switch_the_page() {
    // the lines a sender's system adds under its page are shown: they are over a quarter of this short text part, and no reason to leave the message unseen
    let under = "This mail is a service of the sender, please keep its reference AB12CD.";
    let html = format!("{}<p style=\"font-size:12px\">{under}</p>", column("", ""));
    let text = format!("Thank you for writing to us. Your request is with our team and will be answered within one working day.\n\nKind regards\nThe support team\n\n{under}\n");
    let (p, b) = served(&html, &text);
    let s = parse::shown(&p.display_html);
    assert!(s.words * 4 >= text.split_whitespace().count() && s.zero_words > s.words, "the case: {s:?} against {} words of text", text.split_whitespace().count());
    assert_eq!(b.layout, Some(BodyLayout::Written));
    let page = b.display_html.as_deref().unwrap_or("");
    assert!(page.contains("href=\"https://support.sender.example/requests/1\"") && !page.contains("font-size"), "the page's own words, its link kept: {page}");
    // the cells of a row stay side by side, each row under the one before
    let row = "<table><tr><td style=\"font-size:0\"><a href=\"https://a.sender.example\"><img src=\"https://pics.sender.example/a.png\" width=\"24\" height=\"24\"></a></td><td style=\"font-size:0\">Thank you for writing to us, your request is with our team and will be answered within one working day by a member</td></tr></table>";
    let (_, r) = served(row, "Thank you for writing to us, your request is with our team and will be answered within one working day by a member\n");
    let rp = r.display_html.as_deref().unwrap_or("");
    assert_eq!((r.layout, rp.matches("display:inline-block;vertical-align:top;margin-right:12px").count()), (Some(BodyLayout::Written), 2), "{rp}");
    // and with a text part too short to judge by: the page's own words still show
    let (_, c) = served(&column("", ""), "Request received.\nThank you.\n");
    assert_eq!(c.layout, Some(BodyLayout::Written));
    assert!(c.display_html.as_deref().is_some_and(|h| h.contains("will be answered within one working day") && !h.contains("font-size")));
}

#[test]
fn text_that_names_tags_stays_what_its_sender_wrote() {
    let text = "Hello,\n\nUse <b> for bold and </b> to end it, then <div><table><tr><td> for a table.\n";
    let b = engine::reads::body_from_stored("m1", text.to_string(), None, 0, None);
    assert_eq!(b.plain_text, text);
}

/// The proof on a private copy of the owner's store (path in WHOOSH_OWNER_SNAPSHOT; prints ids' ends, codes and counts, never a word of a mail).
#[test]
#[ignore = "needs WHOOSH_OWNER_SNAPSHOT"]
fn owner_snapshot_serves_no_page_code() {
    let Ok(path) = std::env::var("WHOOSH_OWNER_SNAPSHOT") else { return };
    let c = rusqlite::Connection::open_with_flags(&path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).expect("the snapshot opens");
    let mut st = c.prepare("SELECT m.id, coalesce(m.body_text, ''), m.body_html, m.trackers_blocked, (SELECT prepared FROM message_prepared p WHERE p.message_id = m.id) FROM message m WHERE m.body_text IS NOT NULL OR m.body_html IS NOT NULL").expect("the read");
    let rows = st.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, Option<String>>(2)?, r.get::<_, i64>(3)?, r.get::<_, Option<String>>(4)?))).expect("rows");
    let code_words = ["cellpadding=", "cellspacing=", "style=", "<div", "<table", "<td", "&lt;div", "&lt;table"];
    let (mut mails, mut bad, mut slowest) = (0usize, Vec::new(), 0u128);
    for row in rows {
        let (id, text, html, trackers, prepared) = row.expect("a row");
        mails += 1;
        let was_code = parse::text_is_page_code(&text);
        let t0 = std::time::Instant::now();
        let b = engine::reads::body_from_stored(&id, text, html, trackers.max(0) as u32, prepared.as_deref());
        let ms = t0.elapsed().as_millis();
        slowest = slowest.max(ms);
        let page = b.written_html.as_deref().or(b.display_html.as_deref()).or(b.safe_html.as_deref()).unwrap_or("");
        let seen_text = parse::page_code_words(page);
        let tail: String = id.chars().rev().take(6).collect::<String>().chars().rev().collect();
        let switched = b.layout_reasons.iter().any(|r| r == "page_hides_text");
        let s = parse::shown(page);
        if was_code || switched {
            println!("FACT ..{tail} text_was_code={was_code} switched={switched} layout={:?} text_words={} page_words_shown={} page_words_at_zero={} links={} pictures_over_icon={} ms={ms}",
                b.layout, b.plain_text.split_whitespace().count(), s.words, s.zero_words, page.matches("<a ").count(), s.pictures);
        }
        if code_words.iter().any(|w| b.plain_text.contains(w)) && was_code { bad.push(format!("..{tail} text still holds code")); }
        if code_words.iter().any(|w| seen_text.contains(w)) && (was_code || switched) { bad.push(format!("..{tail} the page shows code as words")); }
        if (was_code || switched) && s.words * 100 < b.plain_text.split_whitespace().count() * 75 { bad.push(format!("..{tail} the page shows {} of {} words", s.words, b.plain_text.split_whitespace().count())); }
    }
    println!("TOTAL mails={mails} bad={} slowest_read_ms={slowest}", bad.len());
    assert!(bad.is_empty(), "{bad:?}");
}
