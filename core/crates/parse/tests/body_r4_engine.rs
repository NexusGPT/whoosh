//! body-r4: the engine's invariants and behaviours, one test each, each seen red on the single-edit mutant named in its comment
//! (runner and logs: the builder's output folder, `mutant-r4-*.log`).
use std::time::{Duration, Instant};

use parse::{
    band, canonical, decode_text, display_first_pass, fold_from, parts_of, plain_to_html, prepare, prepare_in_background, prepare_message,
    quote_depth, written_document, Options, Parts, Reason, Segment, SegmentKind, Skipped, Visibility, Warning, BLOCKED_IMAGE_SRC,
};

fn plain(subject: &str, sender: Option<&str>, text: &str) -> Parts {
    Parts { subject: subject.into(), sender_name: sender.map(str::to_string), text: Some(text.into()), ..Default::default() }
}
fn html(h: &str) -> Parts { Parts { html: Some(h.into()), ..Default::default() } }
fn go(p: &Parts) -> parse::Prepared { prepare(p, &[], &Options::default()) }
fn kinds(p: &parse::Prepared) -> Vec<(&'static str, &'static str)> { p.segments.iter().map(|s| (s.kind.as_str(), s.visibility.as_str())).collect() }

// ---------------------------------------------------------------- invariants

/// Lossless: the segments in order are the canonical text, byte for byte, on plain and HTML input.
/// Mutant `r4-lossless-trims-segment-text` (engine.rs, plain path: `text: t.trim_end().to_string()`): red.
#[test]
fn invariant_lossless_segments_in_order_reproduce_the_canonical_text() {
    let inputs = [
        plain("Re: x", None, "Answer.\r\n\r\nOn Mon, 9 Nov 2026 at 10:00, A <a@example.org> wrote:\r\n> old\r\n>   \r\n"),
        plain("", None, "\u{feff}one\n\n\ntwo  \n> three\n\n"),
        html("<div>Hi <b>there</b></div><blockquote type=\"cite\"><p>old</p></blockquote><p>after</p>"),
    ];
    for p in &inputs {
        let r = go(p);
        let joined: String = r.segments.iter().map(|s| s.text.as_str()).collect();
        assert_eq!(joined, r.canonical, "{:?}", kinds(&r));
        let mut at = 0;
        for s in &r.segments { assert_eq!(s.range.start, at); assert_eq!(&r.canonical[s.range.clone()], s.text); at = s.range.end; }
    }
}

/// Never empty: a body whose every line is put away yields ONE visible UNKNOWN plus a warning.
/// Mutant `r4-never-empty-check-removed` (engine.rs: the never-empty condition `&& false`): red.
#[test]
fn invariant_never_empty_a_body_with_nothing_visible_becomes_one_visible_unknown_with_a_warning() {
    let r = go(&plain("Re: x", None, "On Wed, 11 Nov 2026 at 10:00, K <k@example.pl> wrote:\n> Lunch on Thursday?\n"));
    assert_eq!(kinds(&r), vec![("UNKNOWN", "show")]);
    assert!(r.warnings.contains(&Warning::NothingVisible));
    assert_eq!(r.segments[0].text, r.canonical);
}

/// Hiding requires an anchor: the hide band without a structural reason collapses; no reason at all shows.
/// Mutant `r4-band-ignores-the-anchor` (segment.rs band: `let anchored = true;`): red.
#[test]
fn invariant_hiding_requires_a_structural_anchor() {
    assert_eq!(band(SegmentKind::ReplyHistory, 0.99, &[Reason::QuoteMarks, Reason::ThreadExact]), Visibility::Collapse);
    assert_eq!(band(SegmentKind::ReplyHistory, 0.99, &[Reason::Blockquote]), Visibility::Collapse);
    assert_eq!(band(SegmentKind::ReplyHistory, 0.99, &[Reason::AttributionLine]), Visibility::Hide);
    assert_eq!(band(SegmentKind::ReplyHistory, 0.99, &[]), Visibility::Show, "a heuristic with no reason changes nothing");
    assert_eq!(band(SegmentKind::ReplyHistory, 0.5, &[Reason::AttributionLine]), Visibility::Show);
    // and in the engine: quote marks confirmed by the thread still only collapse
    let earlier = ["Where did you leave the keys of the blue van?"];
    let r = prepare(&plain("Re: keys", None, "With the porter.\n\n> Where did you leave the keys of the blue van?\n"), &earlier, &Options::default());
    assert_eq!(kinds(&r), vec![("NEW_CONTENT", "show"), ("REPLY_HISTORY", "collapse")]);
}

/// The original is retained: both renderings come back untouched (CRLF, BOM, markup), whatever the engine made of them.
/// Mutant `r4-original-not-retained` (engine.rs: `original_text: parts.text.as_deref().map(canonical)`): red.
#[test]
fn invariant_the_original_is_retained_untouched() {
    let text = "\u{feff}Hello\r\nOn Mon, 9 Nov 2026, A <a@example.org> wrote:\r\n> x\r\n";
    let h = "<div onclick=\"x()\">Hello</div><script>bad()</script>";
    let p = Parts { text: Some(text.into()), html: Some(h.into()), ..Default::default() };
    let r = go(&p);
    assert_eq!(r.original_text.as_deref(), Some(text));
    assert_eq!(r.original_html.as_deref(), Some(h));
    assert!(!r.display_html.contains("script"));
}

/// A failure degrades that one message to fully visible escaped text with a warning, never an error: a panic inside the engine, a
/// deadline already passed, bytes that are not a message.
/// Mutant `r4-panic-not-caught` (engine.rs prepare: `match Ok::<_, ()>(run(parts, earlier, opts)) {`): red (the test panics).
#[test]
fn invariant_a_failure_degrades_the_message_to_visible_escaped_text_with_a_warning() {
    let p = plain("x", None, "Line <one> & two");
    let r = prepare(&p, &[], &Options { deadline: None, fault: true });
    assert!(r.degraded);
    assert_eq!(kinds(&r), vec![("UNKNOWN", "show")]);
    assert!(r.warnings.contains(&Warning::Failure));
    assert!(r.display_html.contains("Line &lt;one&gt; &amp; two"), "{}", r.display_html);
    let late = prepare(&p, &[], &Options { deadline: Some(Instant::now() - Duration::from_millis(1)), fault: false });
    assert!(late.degraded && late.warnings.contains(&Warning::TimeLimit));
    let junk = prepare_message(b"", &[], &Options::default());
    assert!(junk.degraded);
}

/// The deadline is checked between stages. Mutant `r4-deadline-never-checked` (engine.rs check: `Ok(())` always): red.
#[test]
fn the_deadline_is_checked_between_stages() {
    let r = prepare(&html("<p>hello</p>"), &[], &Options { deadline: Some(Instant::now()), fault: false });
    assert!(r.warnings.contains(&Warning::TimeLimit), "{:?}", r.warnings);
}

/// The time limit holds off the caller's thread: when the worker is slow (held 3 s here), the call returns at its limit, escaped, warned;
/// a quick message under a generous limit is prepared in full. (A first version timed a 4.56 MB message against a 400 ms bound: it failed
/// under load in four unrelated mutant runs, 439 to 775 ms; the held worker makes the test independent of load.)
/// Mutant `r4-caller-waits-for-the-worker` (engine.rs: the caller's `recv_timeout` replaced by a `recv` that waits for the worker): red.
#[test]
fn prepare_in_background_returns_at_its_time_limit_with_a_warning() {
    let raw = b"From: a@example.org\r\nSubject: s\r\n\r\nHello there, <friend>.\r\n".to_vec();
    let t = Instant::now();
    let r = parse::prepare_in_background_stalled(raw.clone(), vec![], Duration::from_millis(10), Duration::from_secs(3));
    let took = t.elapsed();
    assert!(r.degraded && r.warnings.contains(&Warning::TimeLimit), "{:?}", r.warnings);
    assert!(r.display_html.contains("Hello there, &lt;friend&gt;."), "{}", r.display_html);
    assert!(took < Duration::from_millis(1_500), "returned after {took:?}");
    let ok = prepare_in_background(raw, vec![], Duration::from_secs(5));
    assert!(!ok.degraded, "{:?}", ok.warnings);
}

// ---------------------------------------------------------------- stage 2: canonical text

/// LF, BOM and NUL removed, NFC; quote depth from ">" only with up to three spaces of indent; hard-wrapped lines never joined.
/// Mutant `r4-depth-four-spaces` (canon.rs quote_depth: `i < 4`): red.
#[test]
fn canonical_text_and_quote_depth_follow_the_brief() {
    assert_eq!(canonical("\u{feff}a\r\nb\rc\0d\ne\u{301}"), "a\nb\ncd\n\u{e9}");
    assert_eq!(quote_depth("   > x").0, 1);
    assert_eq!(quote_depth("    > code").0, 0);
    assert_eq!(quote_depth("| a | b |").0, 0);
    assert_eq!(quote_depth(": diff").0, 0);
    let r = go(&plain("", None, "a hard\nwrapped line\n"));
    assert_eq!(r.canonical, "a hard\nwrapped line\n");
}

// ---------------------------------------------------------------- stage 4: detectors

/// Attribution lines in the twelve languages of the brief, each followed by its quote: the tail is hidden.
/// Mutant `r4-attribution-english-only` (segment.rs: attribution rows other than `en` skipped): red.
#[test]
fn attribution_lines_in_twelve_languages_hide_the_quote_after_them() {
    let lines = [
        ("en", "On Tue, 10 Nov 2026 at 14:02, Paul Girard <paul@example.fr> wrote:"),
        ("fr", "Le mar. 10 nov. 2026 à 14:02, Paul Girard <paul@example.fr> a écrit :"),
        ("de", "Am Di., 10. Nov. 2026 um 14:02 Uhr schrieb Paul Girard <paul@example.fr>:"),
        ("es", "El mar, 10 nov 2026 a las 14:02, Paul Girard (<paul@example.fr>) escribió:"),
        ("it", "Il giorno mar 10 nov 2026 alle ore 14:02 Paul Girard <paul@example.fr> ha scritto:"),
        ("pt", "Em ter., 10 de nov. de 2026 às 14:02, Paul Girard <paul@example.fr> escreveu:"),
        ("nl", "Op di 10 nov 2026 om 14:02 schreef Paul Girard <paul@example.fr>:"),
        ("pl", "W dniu 10.11.2026 o 14:02, Paul Girard <paul@example.fr> napisał:"),
        ("et", "Paul Girard <paul@example.fr> kirjutas 10.11.2026 kell 14:02:"),
        ("fi", "Paul Girard <paul@example.fr> kirjoitti 10.11.2026 klo 14.02:"),
        ("sv", "Den 10 nov. 2026 14:02 skrev Paul Girard <paul@example.fr>:"),
        ("ru", "10.11.2026 14:02, Paul Girard <paul@example.fr> пишет:"),
    ];
    for (lang, l) in lines {
        let r = go(&plain("Re: x", None, &format!("My answer in {lang}.\n\n{l}\n> The earlier question?\n")));
        assert_eq!(kinds(&r), vec![("NEW_CONTENT", "show"), ("REPLY_HISTORY", "hide")], "{lang}");
        assert!(r.segments[1].reasons.contains(&Reason::AttributionLine), "{lang}");
    }
}

/// An embedded header block is weak per line and strong only on several consecutive lines with a date-like field; prose that explains
/// headers stays visible.
/// Mutant `r4-header-block-needs-no-date` (segment.rs: `let strong = n >= HEADER_BLOCK_MIN_LINES && from;`): red.
#[test]
fn a_header_block_hides_only_with_a_date_and_prose_about_headers_stays_visible() {
    let prose = "To file a request, fill in the fields:\n\nFrom: your own work address\nTo: the helpdesk queue\nSubject: the device and the problem\n\nThat is all.\n";
    assert_eq!(kinds(&go(&plain("Help", None, prose))), vec![("NEW_CONTENT", "show")]);
    let reply = "Done.\n\nFrom: Mia <mia@example.se>\nSent: Tuesday, November 10, 2026 2:14 PM\nTo: Lars\nSubject: Printer\n\nThe printer is broken.\n";
    let r = go(&plain("RE: Printer", None, reply));
    assert_eq!(kinds(&r), vec![("NEW_CONTENT", "show"), ("REPLY_HISTORY", "hide")]);
    assert!(r.segments[1].reasons.contains(&Reason::HeaderBlock));
}

/// A signature needs TWO independent signals: a closing alone is the sender's text; a closing with contact details is a signature.
/// Mutant `r4-signature-one-signal` (segment.rs: `if starts_kind + others >= 1`): red.
#[test]
fn a_signature_needs_two_independent_signals() {
    let one = go(&plain("x", Some("Hugo Bernard"), "The shelves are ready.\n\nThanks!\n"));
    assert_eq!(kinds(&one), vec![("NEW_CONTENT", "show")]);
    let two = go(&plain("x", Some("Hugo Bernard"), "The shelves are ready.\n\nThanks!\n+33 6 12 34 56 78\n"));
    assert_eq!(kinds(&two), vec![("NEW_CONTENT", "show"), ("SIGNATURE", "collapse")]);
    assert_eq!(two.fold_from, Some(1));
}

/// A legal footer is a DISCLAIMER, collapsed and folded. Mutant `r4-legal-patterns-unread` (segment.rs: legal rows skipped): red.
#[test]
fn a_legal_footer_is_a_disclaimer_behind_the_fold() {
    let r = go(&plain("x", None, "My comments are in the draft.\n\nIf you are not the intended recipient of this message, please delete it.\n"));
    assert_eq!(kinds(&r), vec![("NEW_CONTENT", "show"), ("DISCLAIMER", "collapse")]);
    assert_eq!(r.fold_from, Some(1));
}

/// A forward needs the subject plus the forward header; with a comment it is folded, alone it shows; it is never hidden.
/// Mutant `r4-forward-alone-collapsed` (segment.rs band: the ForwardWithoutComment line removed): red.
#[test]
fn a_forward_is_folded_under_a_comment_shown_alone_and_never_hidden() {
    let body = "---------- Forwarded message ---------\nFrom: Desk <desk@example.net>\nDate: Mon, 9 Nov 2026 at 16:02\nSubject: Window\nTo: Eva <eva@example.com>\n\nThursday between 8 and 12.\n";
    let with = go(&plain("Fwd: Window", None, &format!("Does Thursday suit you?\n\n{body}")));
    assert_eq!(kinds(&with), vec![("NEW_CONTENT", "show"), ("FORWARDED_CONTENT", "collapse")]);
    assert_eq!(with.fold_from, Some(1));
    let alone = go(&plain("Fwd: Window", None, body));
    assert_eq!(kinds(&alone), vec![("FORWARDED_CONTENT", "show")]);
    assert_eq!(alone.fold_from, None);
    assert!(alone.warnings.is_empty(), "{:?}", alone.warnings);
}

/// The thread: a pasted earlier message with no marks is collapsed (never hidden) by exact lines, by shingles when lightly edited, and
/// by the bounded alignment when edited more; without the thread it stays the sender's text.
/// Mutant `r4-thread-fuzzy-off` (segment.rs thread_match: every block past the exact check returns None): red.
#[test]
fn the_thread_collapses_pasted_history_and_never_hides_it() {
    let earlier = ["Hello all,\n\nThe new parking passes are ready at the front desk from Monday, please collect yours before the end of the month.\n"];
    let exact = "I picked them up.\n\nThe new parking passes are ready at the front desk from Monday, please collect yours before the end of the month.\n";
    let r = prepare(&plain("Re: passes", None, exact), &earlier, &Options::default());
    assert_eq!(kinds(&r), vec![("NEW_CONTENT", "show"), ("REPLY_HISTORY", "collapse")]);
    assert!(r.segments[1].reasons.contains(&Reason::ThreadExact));
    let edited = "I picked them up.\n\nThe new parking passes are ready at the front desk from Monday, please collect yours before the end of this month!\n";
    let r = prepare(&plain("Re: passes", None, edited), &earlier, &Options::default());
    assert_eq!(kinds(&r), vec![("NEW_CONTENT", "show"), ("REPLY_HISTORY", "collapse")], "edited");
    assert!(r.segments[1].reasons.iter().any(|x| matches!(x, Reason::ThreadShingles | Reason::ThreadAligned)));
    assert_eq!(kinds(&go(&plain("Re: passes", None, exact))), vec![("NEW_CONTENT", "show")], "no thread, no history");
}

/// A bottom-posted reply: the quote on top is collapsed, never hidden, even under an attribution line.
/// Mutant `r4-bottom-post-hidden` (segment.rs: the bottom post's confidence 0.85 -> 0.97): red.
#[test]
fn a_bottom_posted_quote_is_collapsed_never_hidden() {
    let r = go(&plain("Re: t", None, "On Tue, 10 Nov 2026 at 09:15, Dana <dana@example.org> wrote:\n> Which train?\n\nThe 8:12.\n"));
    assert_eq!(kinds(&r), vec![("REPLY_HISTORY", "collapse"), ("NEW_CONTENT", "show")]);
    assert!(r.segments[0].reasons.contains(&Reason::BottomPost));
    assert_eq!(r.fold_from, None);
}

/// The two defining cases of the brief.
/// Mutant `r4-inline-quote-as-history` (segment.rs: the INLINE_QUOTE label made REPLY_HISTORY 0.97): red.
#[test]
fn the_two_defining_cases() {
    let a = go(&plain("Re: v", None, "> Is the room free?\nYes.\n> How many people?\nTwenty.\n"));
    assert_eq!(kinds(&a), vec![("INLINE_QUOTE", "show"), ("NEW_CONTENT", "show"), ("INLINE_QUOTE", "show"), ("NEW_CONTENT", "show")]);
    let b = go(&plain("Re: b", None, "Fine by me.\n\nOn Friday, Alice wrote:\n> Here are the figures.\n"));
    assert_eq!(kinds(&b), vec![("NEW_CONTENT", "show"), ("REPLY_HISTORY", "hide")]);
}

/// Stage 1: when the HTML puts away what the plain part shows as the sender's own text, confidence only goes down.
/// Mutant `r4-parts-not-cross-checked` (engine.rs: the cross_check call removed): red.
#[test]
fn the_plain_part_can_only_lower_the_htmls_confidence() {
    let h = "<div>Short answer.</div><div class=\"gmail_quote\"><div class=\"gmail_attr\">On Mon, Nov 9, 2026 at 5:10 PM V &lt;v@example.se&gt; wrote:</div><blockquote class=\"gmail_quote\"><div>This paragraph is really my own new text that the HTML wrongly marks.</div></blockquote></div>";
    let t = "Short answer.\n\nThis paragraph is really my own new text that the HTML wrongly marks.\n";
    let r = go(&Parts { html: Some(h.into()), text: Some(t.into()), ..Default::default() });
    let s = r.segments.iter().find(|s| s.text.contains("really my own")).expect("the segment");
    assert!(s.reasons.contains(&Reason::PartsDisagree), "{:?}", s.reasons);
    assert!(s.confidence < 0.95 && s.visibility != Visibility::Hide, "{} {:?}", s.confidence, s.visibility);
    // agreement never raises anything
    let same = go(&Parts { html: Some(h.into()), text: Some("Short answer.\n\nOn Mon, Nov 9, 2026 at 5:10 PM V <v@example.se> wrote:\n> This paragraph is really my own new text that the HTML wrongly marks.\n".into()), ..Default::default() });
    let s = same.segments.iter().find(|s| s.text.contains("really my own")).expect("the segment");
    assert!(!s.reasons.contains(&Reason::PartsDisagree) && s.visibility == Visibility::Hide);
}

// ---------------------------------------------------------------- F2: fold_from

fn seg(kind: SegmentKind, vis: Visibility) -> Segment {
    Segment { kind, text: "Words.".into(), html: String::new(), safe_html: String::new(), confidence: 0.9, visibility: vis, reasons: vec![Reason::QuoteMarks], range: 0..0 }
}

/// fold_from: from the first segment after which all is put-away non-authored content; authored text after a quote or a signature
/// moves the fold after it; a message that is only a forward or only history folds nothing.
/// Mutant `r4-fold-from-zero-allowed` (segment.rs fold_from: the "nothing shown above the fold" line removed): red.
#[test]
fn fold_from_never_folds_authored_text_and_never_folds_everything() {
    use SegmentKind::*;
    use Visibility::*;
    assert_eq!(fold_from(&[seg(NewContent, Show), seg(Signature, Collapse), seg(ReplyHistory, Hide)]), Some(1));
    assert_eq!(fold_from(&[seg(NewContent, Show), seg(Signature, Collapse), seg(NewContent, Show)]), None);
    assert_eq!(fold_from(&[seg(InlineQuote, Show), seg(NewContent, Show), seg(ReplyHistory, Collapse), seg(NewContent, Show), seg(Disclaimer, Collapse)]), Some(4));
    assert_eq!(fold_from(&[seg(ForwardedContent, Collapse)]), None);
    assert_eq!(fold_from(&[seg(ReplyHistory, Collapse), seg(Signature, Collapse)]), None);
    assert_eq!(fold_from(&[seg(NewContent, Show), seg(ForwardedContent, Show)]), None);
    // a line of dashes is a separator, not the sender's words: nothing above the fold says something, so nothing folds (owner's picture 212).
    // Mutant `fold-counts-a-separator` (segment.rs fold_from: `&& says_something(&s.text)` removed): red here.
    let rule = |t: &str| Segment { text: t.into(), ..seg(NewContent, Show) };
    assert_eq!(fold_from(&[rule("---\n"), seg(ForwardedContent, Collapse)]), None);
    assert_eq!(fold_from(&[rule("\u{2014}\u{2014} \u{00a0}\n"), seg(Signature, Collapse)]), None);
    assert_eq!(fold_from(&[rule("\u{1f44d}\n"), seg(ReplyHistory, Hide)]), Some(1), "an emoji is an answer");
    assert_eq!(fold_from(&[rule("ok\n"), seg(ReplyHistory, Hide)]), Some(1));
}

// ---------------------------------------------------------------- (B) the display document

/// Style rules are inlined onto the elements they match before the style element is dropped; :is( :where( :has( and a :not( holding a
/// pseudo are dropped with their rule. Mutant `r4-styles-not-inlined` (engine.rs: the inline_styles call replaced by `Inlined::default()`): red.
#[test]
fn style_rules_are_inlined_and_unsafe_selectors_dropped() {
    let h = "<style>.title{color:#2f4f2f;font-size:28px} p:is(.x){color:red} p:where(.x){margin:9px} p:has(b){padding:9px} p:not(:first-child){border:1px solid red}</style><p class=\"title\">Hello</p><p class=\"x\">two <b>b</b></p>";
    let r = go(&html(h));
    assert!(r.display_html.contains("color:#2f4f2f") && r.display_html.contains("font-size:28px"), "{}", r.display_html);
    assert!(!r.display_html.contains("<style") && !r.display_html.contains("color:red") && !r.display_html.contains("margin:9px") && !r.display_html.contains("padding:9px") && !r.display_html.contains("border:1px"), "{}", r.display_html);
    assert_eq!(r.inlining.skipped, None);
}

/// Past a cap nothing is inlined and a field says which: style text, selectors, selectors x elements.
/// Mutant `r4-matches-cap-unchecked` (css.rs: the selectors x elements test removed): red.
#[test]
fn past_a_cap_nothing_is_inlined_and_the_field_says_which() {
    let rules: String = (0..3_000).map(|i| format!(".c{i}{{color:red}}")).collect();
    let cells = "<span>x</span>".repeat(2_000);
    let r = go(&html(&format!("<style>{rules}</style><p class=\"c1\">t</p>{cells}")));
    assert_eq!(r.inlining.skipped, Some(Skipped::TooManyMatches));
    assert!(r.warnings.contains(&Warning::InliningSkipped(Skipped::TooManyMatches)));
    assert!(!r.display_html.contains("color:red"));
    let big = go(&html(&format!("<style>{}</style><p>t</p>", "p{color:red} ".repeat(30_000))));
    assert_eq!(big.inlining.skipped, Some(Skipped::StyleTooLarge));
}

/// Elements never kept (with their content where it is not the sender's): in our own pass and in the final document.
/// Mutant `r4-drop-list-empty` (display.rs: `const DROP: &[&str] = &[];`): red.
#[test]
fn elements_never_kept_are_gone_from_both_layers() {
    let h = "<p>keep</p><script>s1()</script><style>p{}</style><link rel=x href=\"https://e.example\"><meta name=a><base href=\"https://e.example\"><title>T1</title><iframe>I1</iframe><frameset><frame></frameset><template>TP1</template><noscript>NS1</noscript><object>OB1</object><embed src=x><canvas>CV1</canvas><audio>AU1</audio><video>VI1</video><applet>AP1</applet><form action=\"https://e.example\"><input value=IN1><button>Go</button><select><option>SE1</option></select><textarea>TA1</textarea></form><dialog open>DI</dialog><svg><text>SV1</text></svg><math><mi>MA1</mi></math>";
    for (name, out) in [("first pass", display_first_pass(h, &[])), ("final", go(&html(h)).display_html)] {
        for bad in ["<script", "s1()", "<style", "<link", "<meta", "<base", "T1", "<iframe", "I1", "<frame", "TP1", "NS1", "<object", "OB1", "<embed", "<canvas", "CV1", "<audio", "AU1", "<video", "VI1", "<applet", "AP1", "<form", "<input", "IN1", "<button", "<select", "SE1", "<textarea", "TA1", "<dialog", "<svg", "SV1", "<math", "MA1", "action="] {
            assert!(!out.contains(bad), "{name}: {bad} kept in {out}");
        }
        assert!(out.contains("keep") && out.contains("Go") && out.contains("DI"), "{name}: the sender's text inside form, button, dialog stays: {out}");
    }
}

/// Attributes never kept and links: http, https, mailto, tel only, in our own pass and in the final document.
/// Mutant `r4-any-href-kept` (display.rs safe_href: `true ||`): red.
#[test]
fn attributes_and_links_follow_the_allow_list_in_both_layers() {
    let h = "<a href=\"javascript:alert(1)\">j</a><a href=\"data:text/html,x\">d</a><a href=\"ftp://e.example\">f</a><a href=\"#top\">h</a><a href=\"https://ok.example/a\">ok</a><a href=\"mailto:a@example.org\">m</a><a href=\"tel:+3312\">t</a><div onclick=\"x()\" onmouseover=\"y()\" popover popovertarget=\"p\">e</div><button formaction=\"https://e.example\">b</button>";
    for (name, out) in [("first pass", display_first_pass(h, &[])), ("final", go(&html(h)).display_html)] {
        for bad in ["javascript:", "data:", "ftp:", "#top", "onclick", "onmouseover", "popover", "formaction"] { assert!(!out.contains(bad), "{name}: {bad} in {out}"); }
        for good in ["https://ok.example/a", "mailto:a@example.org", "tel:+3312"] { assert!(out.contains(good), "{name}: {good} missing in {out}"); }
    }
}

/// CSS properties never kept (escapes decoded first), and the body's width and height forms.
/// Mutant `r4-position-allowed` (css.rs blocked_property: "position" removed from the list): red.
#[test]
fn css_properties_never_kept_and_the_bodys_size_dropped() {
    let h = "<body style=\"width:600px;min-width:600px;max-height:9px;height:100px;background:#eeeeee\"><div style=\"p\\osition:fixed;position:absolute;float:left;clear:both;clip:rect(0,0,0,0);zoom:3;all:unset;animation:spin 1s;offset-path:none;scrollbar-color:red;paint-order:fill;color:#123456\">x</div></body>";
    for (name, out) in [("first pass", display_first_pass(h, &[])), ("final", go(&html(h)).display_html)] {
        for bad in ["position", "osition", "float", "clear", "clip", "zoom", "all:", "animation", "offset", "scrollbar", "paint-order", "width", "height"] { assert!(!out.contains(bad), "{name}: {bad} in {out}"); }
        assert!(out.contains("color:#123456") && out.contains("background:#eeeeee"), "{name}: {out}");
    }
}

/// Remote images are replaced and counted; an image pointing at a part of the same message (cid:) resolves to that part.
/// Mutant `r4-cid-not-resolved` (display.rs image_src: the cid branch returns the blocked source): red.
#[test]
fn remote_images_are_replaced_and_counted_and_cid_images_resolve_to_their_part() {
    let raw = "From: a@example.org\r\nSubject: logo\r\nMIME-Version: 1.0\r\nContent-Type: multipart/related; boundary=\"b\"\r\n\r\n--b\r\nContent-Type: text/html; charset=utf-8\r\n\r\n<p>Hi</p><img src=\"cid:logo@x\"><img src=\"https://t.example/a.gif\"><img src=\"http://t.example/b.gif\">\r\n--b\r\nContent-Type: image/png\r\nContent-ID: <logo@x>\r\nContent-Transfer-Encoding: base64\r\n\r\niVBORw0KGgo=\r\n--b--\r\n";
    let r = prepare_message(raw.as_bytes(), &[], &Options::default());
    assert_eq!(r.remote_images_blocked, 2);
    assert_eq!(r.cid_images, vec![("logo@x".to_string(), "2".to_string())]);
    assert!(r.display_html.contains("src=\"cid:logo@x\""), "{}", r.display_html);
    assert_eq!(r.display_html.matches(BLOCKED_IMAGE_SRC).count(), 2, "{}", r.display_html);
}

/// Hostile nesting: past the nesting cap the message is escaped text with a warning (no parser runs on it).
/// Mutant `r4-nesting-cap-off` (engine.rs: the `HTML_NESTING_MAX` check compares with `usize::MAX`): red (no warning).
#[test]
fn hostile_nesting_degrades_to_escaped_text_quickly() {
    // 5,000 levels: past the cap of 2,000 (the corpus case has 100,000); small enough that the mutant without the cap still finishes
    let h = format!("<p>Before.</p>{}The sentence at the bottom.", "<div>".repeat(5_000));
    let t = Instant::now();
    let r = go(&html(&h));
    assert!(r.warnings.contains(&Warning::TooDeep), "{:?}", r.warnings);
    assert!(r.canonical.contains("The sentence at the bottom."));
    assert!(t.elapsed() < Duration::from_secs(5), "{:?}", t.elapsed());
}

// ---------------------------------------------------------------- plain text made HTML, charsets, flowed

/// Paragraphs, kept line breaks, links (web, www, mail, bare domain), nested quotes with the depth cap, rule lines, escaping.
/// Mutant `r4-plain-depth-uncapped` (display.rs plain_to_html: `if d > usize::MAX`): red.
#[test]
fn plain_text_becomes_html_by_the_brief() {
    let h = plain_to_html("Hello <you> & co\nsecond line\n\nsee https://a.example/x, www.b.example and mail c@d.example or shop.example.com.\n--\n> quoted\n>> deeper\n");
    assert!(h.starts_with("<p>Hello &lt;you&gt; &amp; co<br>second line</p>"), "{h}");
    assert!(h.contains("<a href=\"https://a.example/x\">https://a.example/x</a>,"), "{h}");
    assert!(h.contains("<a href=\"https://www.b.example\">www.b.example</a>") && h.contains("<a href=\"mailto:c@d.example\">c@d.example</a>") && h.contains("<a href=\"https://shop.example.com\">shop.example.com</a>."), "{h}");
    assert!(h.contains("<hr>") && h.contains("<blockquote><p>quoted</p><blockquote><p>deeper</p></blockquote></blockquote>"), "{h}");
    let deep = plain_to_html(&format!("{} deep\n", ">".repeat(101)));
    assert_eq!(deep.matches("<blockquote>").count(), 100);
    assert!(deep.contains("&gt; deep"), "the 101st mark stays text: {}", &deep[deep.len() - 60..]);
}

/// format=flowed is unfolded (soft breaks joined, space-stuffing removed); a fixed message's hard breaks are kept.
/// Mutant `r4-flowed-ignored` (engine.rs parts_of: `if flowed && false`): red.
#[test]
fn format_flowed_is_unfolded_and_fixed_text_is_not() {
    let flowed = "From: a@example.org\r\nSubject: f\r\nContent-Type: text/plain; charset=utf-8; format=flowed\r\n\r\nOne soft \r\nwrapped line.\r\n From the stuffed line.\r\n";
    assert_eq!(parts_of(flowed.as_bytes()).expect("a message").text.as_deref(), Some("One soft wrapped line.\nFrom the stuffed line.\n"));
    let fixed = "From: a@example.org\r\nSubject: f\r\nContent-Type: text/plain; charset=utf-8\r\n\r\nOne soft \r\nwrapped line.\r\n";
    assert!(parts_of(fixed.as_bytes()).expect("a message").text.expect("text").contains("soft \r\nwrapped") );
}

/// Charsets: the declared one; an unknown label read tolerantly with a warning; UTF-8 with over 2 % replacements read as windows-1252
/// with a warning; a little damage stays UTF-8.
/// Mutant `r4-no-1252-fallback` (canon.rs: the share compared with 2.0 instead of the constant): red.
#[test]
fn the_charset_chain_declared_then_tolerant_then_windows_1252() {
    assert_eq!(decode_text(&[0x63, 0x61, 0x66, 0xE9], Some("iso-8859-1")).0, "caf\u{e9}");
    let (t, w) = decode_text(b"plain", Some("x-no-such-charset"));
    assert_eq!((t.as_str(), w.len()), ("plain", 1));
    let (t, w) = decode_text(&[0x63, 0x61, 0x66, 0xE9, 0x20, 0x80], Some("utf-8"));
    assert_eq!(t, "caf\u{e9} \u{20ac}");
    assert_eq!(w, vec![parse::CharsetDowngrade::Utf8ReadAsWindows1252]);
    let mut mostly = "a".repeat(200).into_bytes();
    mostly.push(0xFF);
    let (t, w) = decode_text(&mostly, Some("utf-8"));
    assert!(w.is_empty() && t.ends_with('\u{fffd}'));
    let raw: Vec<u8> = b"From: a@example.org\r\nSubject: c\r\nContent-Type: text/plain; charset=utf-8\r\n\r\nd\xe9j\xe0 pay\xe9\r\n".to_vec();
    let r = prepare_message(&raw, &[], &Options::default());
    assert!(r.canonical.contains("d\u{e9}j\u{e0} pay\u{e9}") && r.warnings.contains(&Warning::CharsetUtf8ReadAsWindows1252), "{:?}", r.warnings);
}

// ---------------------------------------------------------------- B3: the written document

/// The written document drops the sender's font family, size, text colour and backgrounds and keeps bold, italic, links, lists, quotes,
/// data tables and inline pictures. Mutant `r4-written-keeps-colour` (display.rs written_document: `"color"` removed from the filter): red.
#[test]
fn the_written_document_keeps_structure_and_drops_the_senders_type_and_colours() {
    let clean = "<div style=\"font-family:Comic Sans MS;font-size:18px;color:#ff0000;background-color:#000000;font-weight:bold\"><b>bold</b> <i>it</i> <a href=\"https://a.example\">link</a><ul><li>one</li></ul><blockquote>q</blockquote><table><tr><td bgcolor=\"#ffcc00\">1</td></tr></table><font color=\"red\" face=\"Arial\" size=\"5\">f</font><img src=\"cid:logo@x\"></div>";
    let w = written_document(clean);
    for bad in ["Comic", "font-size", "#ff0000", "background", "bgcolor", "face=", "size=", "color="] { assert!(!w.contains(bad), "{bad} in {w}"); }
    for good in ["<b>bold</b>", "<i>it</i>", "href=\"https://a.example\"", "<ul><li>one</li></ul>", "<blockquote>q</blockquote>", "<td>1</td>", "src=\"cid:logo@x\"", "font-weight:bold"] { assert!(w.contains(good), "{good} missing in {w}"); }
}

/// Memory is bounded by the input: a rendering over `BODY_BYTES_MAX` is escaped text with a warning, before any parser runs.
/// Mutant `r4-body-size-cap-off` (engine.rs: the HTML size check compares with `usize::MAX`): red (no warning).
#[test]
fn a_body_over_the_size_cap_is_escaped_text_with_a_warning() {
    let h = format!("<p>{}</p>", "a".repeat(parse::BODY_BYTES_MAX));
    let r = go(&html(&h));
    assert!(r.degraded && r.warnings.contains(&Warning::TooLarge), "{:?}", r.warnings);
    let t = plain("x", None, &"b".repeat(parse::BODY_BYTES_MAX + 1));
    assert!(go(&t).warnings.contains(&Warning::TooLarge));
}
