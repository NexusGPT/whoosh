//! Owner's picture 210 (13 Nov 2026): a designed mail drew as plain text with raw codes. Its first engine run had reached the 2 s wall
//! limit on a loaded Mac, and the escaped text was kept for good (7 stored mails of 5 to 48 KB).
//! Two rules: (1) a run that reaches the limit is run once more under a longer limit (`bodies::prepare_within`); (2) a kept record that
//! the limit or a failure stopped is not served when the mail's page is stored: the reader draws the page (`reads::body_from_stored`).
//!
//! MUTANTS (each turns the test named red):
//!   R1 `retry-off`        bodies.rs `prepare_within`: returns the first result always      -> a_run_that_reached_the_limit_is_run_once_more
//!   R2 `stopped-served`   reads.rs `apply_prepared`: the `stopped` guard removed            -> a_stopped_record_is_not_served_when_the_page_is_stored
use std::time::Duration;

use contracts::BodyLayout;

fn newsletter_raw() -> Vec<u8> {
    let html = "<html><head><style>.h{display:none}</style></head><body><span class=\"h\">&amp;#847; &amp;zwnj;</span>\
                <table width=\"600\" bgcolor=\"#111111\"><tr><td bgcolor=\"#222222\"><img src=\"https://pics.example/banner.png\" width=\"600\" height=\"200\"></td></tr>\
                <tr><td bgcolor=\"#333333\"><p>Discover how network interfaces drive innovation.</p><a href=\"https://news.example/register\">Register now</a></td></tr></table></body></html>";
    format!("From: News <news@sender.example>\r\nTo: owner@example.test\r\nSubject: Webinar\r\nMIME-Version: 1.0\r\nContent-Type: text/html; charset=utf-8\r\n\r\n{html}\r\n").into_bytes()
}

fn limit_reached(p: &parse::Prepared) -> bool { p.warnings.contains(&parse::Warning::TimeLimit) }

#[test]
fn a_run_that_reached_the_limit_is_run_once_more() {
    let raw = newsletter_raw();
    // the old rule (no further run): a first limit already spent keeps escaped text with the warning
    let old = engine::bodies::prepare_within(&raw, Vec::new(), Duration::ZERO, Duration::ZERO);
    assert!(limit_reached(&old) && old.degraded, "the case must reach the limit on its first run: {:?}", old.warnings);
    assert!(!old.display_html.contains("<table"), "escaped text holds no table");
    let before = engine::bodies::prepare_retries();
    let p = engine::bodies::prepare_within(&raw, Vec::new(), Duration::ZERO, Duration::from_secs(30));
    assert_eq!(engine::bodies::prepare_retries() - before, 1, "one further run");
    assert!(!limit_reached(&p) && !p.degraded, "the further run finished: {:?}", p.warnings);
    assert!(p.display_html.contains("<table"), "the page is kept, not its escaped text");
    // a run inside its limit is not run again
    let before = engine::bodies::prepare_retries();
    let q = engine::bodies::prepare_within(&raw, Vec::new(), Duration::from_secs(30), Duration::from_secs(30));
    assert_eq!(engine::bodies::prepare_retries() - before, 0);
    assert!(!limit_reached(&q));
}

#[test]
fn a_stopped_record_is_not_served_when_the_page_is_stored() {
    let raw = newsletter_raw();
    let stopped = engine::bodies::prepare_within(&raw, Vec::new(), Duration::ZERO, Duration::ZERO);
    assert!(limit_reached(&stopped));
    let json = engine::reads::prepared_json(&stopped);
    let page = "<table><tr><td><p>Discover how network interfaces drive innovation.</p></td></tr></table>".to_string();
    // the page is stored: the stopped record is not served, the reader lays the stored page out by its own rule
    let b = engine::reads::body_from_stored("m1", "Discover".into(), Some(page.clone()), 0, Some(&json));
    assert_eq!(b.layout, None);
    assert_eq!(b.display_html, None);
    assert!(b.body_warnings.is_empty(), "no line says plain text: {:?}", b.body_warnings);
    assert_eq!(b.safe_html.as_deref(), Some(page.as_str()));
    // no page stored (a plain-text mail): the record stays, with its warning
    let t = engine::reads::body_from_stored("m2", "Discover".into(), None, 0, Some(&json));
    assert_eq!(t.layout, Some(BodyLayout::Written));
    assert_eq!(t.body_warnings, vec![parse::Warning::TimeLimit.as_str().to_string()]);
    // a whole record is served as before
    let whole = engine::bodies::prepare_within(&raw, Vec::new(), Duration::from_secs(30), Duration::ZERO);
    let w = engine::reads::body_from_stored("m3", "Discover".into(), Some(page), 0, Some(&engine::reads::prepared_json(&whole)));
    assert!(w.layout.is_some() && w.display_html.is_some_and(|h| h.contains("<table")));
}
