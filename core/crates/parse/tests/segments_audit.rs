//! A129 audit: the classified segments of the mail content engine, tested from the outside (`parse::prepare`), on mails written here.
//! (a) one test per segment kind, (b) a 14-mail corpus with the sender's own words marked, (c) one test per safety invariant.
//! Every mail is hand-written; none is a real mail. Every loop below is over a fixed list or over one mail's segments.
//! Mutants: `mutants-segments.sh` beside this file.
use parse::{prepare, Options, Parts, Prepared, Reason, SegmentKind, Visibility, Warning};

fn plain(subject: &str, sender: Option<&str>, text: &str) -> Parts {
    Parts { subject: subject.into(), sender_name: sender.map(str::to_string), text: Some(text.into()), ..Default::default() }
}
fn html(subject: &str, h: &str) -> Parts { Parts { subject: subject.into(), html: Some(h.into()), ..Default::default() } }
fn go(p: &Parts) -> Prepared { prepare(p, &[], &Options::default()) }
fn kinds(p: &Prepared) -> Vec<(&'static str, &'static str)> { p.segments.iter().map(|s| (s.kind.as_str(), s.visibility.as_str())).collect() }

/// Is the byte at `at` of the canonical text visible with the fold applied?
fn visible_at(p: &Prepared, at: usize) -> bool {
    p.segments.iter().enumerate().any(|(i, s)| s.range.contains(&at) && s.visibility == Visibility::Show && p.fold_from.is_none_or(|f| i < f))
}

fn every_segment_is_well_formed(p: &Prepared, what: &str) {
    for (i, s) in p.segments.iter().enumerate() {
        assert!((0.0..=1.0).contains(&s.confidence), "{what}: segment {i} confidence {}", s.confidence);
        assert!(s.reasons.iter().all(|r| !r.as_str().is_empty()), "{what}: segment {i} has a reason with no word");
        assert!(!s.text.is_empty(), "{what}: segment {i} is empty");
    }
}

// ------------------------------------------------------------------ (a) one test per kind

#[test]
fn kind_new_content_is_shown_with_a_confidence() {
    let p = go(&plain("Hello", None, "Thanks, I will bring the projector at nine.\n"));
    assert_eq!(kinds(&p), vec![("NEW_CONTENT", "show")]);
    assert_eq!(p.fold_from, None);
    every_segment_is_well_formed(&p, "new content");
}

#[test]
fn kind_inline_quote_stays_visible_between_the_answers() {
    let p = go(&plain("Re: invoice", None, "> Did you send the invoice on Monday?\nYes, on Monday morning.\n> And the receipt for the tiles?\nNot yet, tomorrow.\n"));
    let ks = kinds(&p);
    assert!(ks.iter().any(|k| k.0 == "INLINE_QUOTE"), "{ks:?}");
    assert!(ks.iter().all(|k| k.1 == "show"), "an inline quote and the answers all show: {ks:?}");
    assert_eq!(p.fold_from, None, "{ks:?}");
    let q = p.segments.iter().find(|s| s.kind == SegmentKind::InlineQuote).expect("an inline quote");
    assert!(q.reasons.contains(&Reason::AuthoredAfter), "{:?}", q.reasons);
    every_segment_is_well_formed(&p, "inline quote");
}

#[test]
fn kind_reply_history_is_hidden_behind_an_attribution_line() {
    let p = go(&plain("Re: Friday", None, "Friday works for me.\n\nOn Tue, 3 Nov 2026 at 09:15, Sam Hart <sam@example.org> wrote:\n> Can we meet on Friday?\n> Bring the plans.\n"));
    assert_eq!(kinds(&p), vec![("NEW_CONTENT", "show"), ("REPLY_HISTORY", "hide")]);
    let h = &p.segments[1];
    assert!(h.reasons.contains(&Reason::AttributionLine) && h.reasons.iter().any(|r| r.is_anchor()), "{:?}", h.reasons);
    assert!(h.confidence >= parse::HIDE_MIN_CONFIDENCE, "{}", h.confidence);
    assert!(h.text.contains("Bring the plans."), "hidden is not deleted: the history is returned in full");
    assert_eq!(p.fold_from, Some(1));
    every_segment_is_well_formed(&p, "reply history");
}

#[test]
fn kind_forwarded_content_is_folded_under_a_comment_and_never_hidden() {
    let p = go(&plain("Fwd: Delivery window", Some("Eva Schmidt"), "Can you check Thursday with the warehouse?\n\n---------- Forwarded message ---------\nFrom: Logistics Desk <desk@example.net>\nDate: Mon, 9 Nov 2026 at 16:02\nSubject: Delivery window\nTo: Eva Schmidt <eva@example.com>\n\nYour delivery can be made on Thursday between 8 and 12.\n"));
    assert_eq!(kinds(&p), vec![("NEW_CONTENT", "show"), ("FORWARDED_CONTENT", "collapse")]);
    assert!(p.segments[1].reasons.iter().any(|r| matches!(r, Reason::ForwardSubject | Reason::ForwardHeader)), "{:?}", p.segments[1].reasons);
    assert_eq!(p.fold_from, Some(1));
    every_segment_is_well_formed(&p, "forward");
}

#[test]
fn kind_forwarded_content_alone_shows_and_folds_nothing() {
    let p = go(&plain("Fwd: Room booking", Some("Eva Schmidt"), "---------- Forwarded message ---------\nFrom: Facilities <facilities@example.net>\nDate: Tue, 10 Nov 2026 at 11:30\nSubject: Room booking\nTo: Eva Schmidt <eva@example.com>\n\nRoom 4 is booked for you on Wednesday from 10 to 11.\n"));
    assert_eq!(kinds(&p), vec![("FORWARDED_CONTENT", "show")]);
    assert_eq!(p.fold_from, None);
}

#[test]
fn kind_signature_is_collapsed_with_two_signals() {
    let p = go(&plain("Quote", Some("Hugo Bernard"), "The quote for the oak shelves is attached.\n\nBest regards,\nHugo Bernard\nBernard Joinery\n+33 6 12 34 56 78\n"));
    assert_eq!(kinds(&p), vec![("NEW_CONTENT", "show"), ("SIGNATURE", "collapse")]);
    assert!(p.segments[1].reasons.iter().any(|r| matches!(r, Reason::SignatureClosing | Reason::SignatureContact | Reason::SignatureSenderName)), "{:?}", p.segments[1].reasons);
    assert_eq!(p.fold_from, Some(1));
    // a closing alone is the sender's own words, not a signature
    let alone = go(&plain("Quote", Some("Eva Schmidt"), "The quote is attached.\n\nBest regards,\nHugo\n"));
    assert!(alone.segments.iter().all(|s| s.kind != SegmentKind::Signature), "{:?}", kinds(&alone));
}

#[test]
fn kind_disclaimer_is_a_collapsed_legal_footer() {
    let p = go(&plain("Contract", Some("Ines Duarte"), "My comments on clauses 4 and 7 are in the margin.\n\nThis email and any attachments are confidential and intended solely for the addressee. If you are not the intended recipient, please delete it and notify the sender.\n"));
    assert_eq!(kinds(&p), vec![("NEW_CONTENT", "show"), ("DISCLAIMER", "collapse")]);
    assert!(p.segments[1].reasons.contains(&Reason::LegalNotice), "{:?}", p.segments[1].reasons);
    assert_eq!(p.fold_from, Some(1));
}

#[test]
fn kind_unknown_is_the_visible_fallback_with_its_reason() {
    // nothing would be visible: the whole text is ONE unknown segment, shown
    let q = go(&plain("Re: Lunch", None, "On Wed, 11 Nov 2026 at 10:00, Kasia Nowak <kasia@example.pl> wrote:\n> Shall we have lunch on Thursday?\n"));
    assert_eq!(kinds(&q), vec![("UNKNOWN", "show")]);
    assert_eq!(q.segments[0].reasons, vec![Reason::NothingVisible]);
    assert!(q.warnings.contains(&Warning::NothingVisible));
    // a failure: the same kind, the reason `failure`
    let f = prepare(&plain("x", None, "a <b>"), &[], &Options { deadline: None, fault: true });
    assert_eq!(kinds(&f), vec![("UNKNOWN", "show")]);
    assert_eq!(f.segments[0].reasons, vec![Reason::Failure]);
}

#[test]
fn french_reply_hides_its_history_under_the_french_attribution() {
    let p = go(&plain("RE: Réunion", None, "Parfait, rendez-vous à neuf heures.\n\nLe lun. 2 nov. 2026 à 18:40, Lise Moreau <lise@example.fr> a écrit :\n> On se voit demain ?\n"));
    assert_eq!(kinds(&p), vec![("NEW_CONTENT", "show"), ("REPLY_HISTORY", "hide")]);
}

// ------------------------------------------------------------------ (b) the corpus

struct Mail { name: &'static str, parts: Parts, authored: Vec<&'static str> }

fn corpus() -> Vec<Mail> {
    let m = |name, parts, authored| Mail { name, parts, authored };
    vec![
        m("en-reply", plain("Re: Friday", None, "Thanks, Friday works for me.\n\nOn Tue, 3 Nov 2026 at 09:15, Sam Hart <sam@example.org> wrote:\n> Can we meet on Friday?\n> Bring the plans.\n"), vec!["Thanks, Friday works for me."]),
        m("fr-reply", plain("RE: Réunion", None, "Parfait, rendez-vous à neuf heures.\n\nLe lun. 2 nov. 2026 à 18:40, Lise Moreau <lise@example.fr> a écrit :\n> On se voit demain ?\n"), vec!["Parfait, rendez-vous à neuf heures."]),
        m("de-reply", plain("AW: Termin", None, "Ich bin am Donnerstag frei.\n\nAm Dienstag, 3. November 2026 um 09:15 schrieb Anna Roth <anna@example.de>:\n> Hast du am Donnerstag Zeit?\n"), vec!["Ich bin am Donnerstag frei."]),
        m("forward-with-comment", plain("Fwd: Delivery", Some("Eva Schmidt"), "Can you check Thursday with the warehouse?\n\n---------- Forwarded message ---------\nFrom: Logistics Desk <desk@example.net>\nDate: Mon, 9 Nov 2026 at 16:02\nSubject: Delivery\nTo: Eva Schmidt <eva@example.com>\n\nYour delivery can be made on Thursday.\n"), vec!["Can you check Thursday with the warehouse?"]),
        m("signature", plain("Quote", Some("Hugo Bernard"), "The quote for the oak shelves is attached; the price holds to month end.\n\nBest regards,\nHugo Bernard\nBernard Joinery\n+33 6 12 34 56 78\n"), vec!["The quote for the oak shelves is attached; the price holds to month end."]),
        m("disclaimer", plain("Contract", Some("Ines Duarte"), "My comments on clauses 4 and 7 are in the margin.\n\nThis email and any attachments are confidential and intended solely for the addressee. If you are not the intended recipient, please delete it and notify the sender.\n"), vec!["My comments on clauses 4 and 7 are in the margin."]),
        m("inline-reply", plain("Re: invoice", None, "> Did you send the invoice on Monday?\nYes, on Monday morning.\n> And the receipt for the tiles?\nNot yet, tomorrow.\n"), vec!["> Did you send the invoice on Monday?", "Yes, on Monday morning.", "> And the receipt for the tiles?", "Not yet, tomorrow."]),
        m("newsletter", html("Garden Letter", "<html><head><style>p{color:#333}.f{font-size:11px}</style></head><body><table width=\"600\"><tr><td><h1>November in the garden</h1><p>Plant your tulip bulbs now for a spring full of colour.</p><p>The pruning workshop opens for booking this week.</p></td></tr></table><div class=\"f\"><p>You received this because you subscribed. <a href=\"https://example.org/u\">Unsubscribe</a> from this list.</p></div></body></html>"), vec!["Plant your tulip bulbs now for a spring full of colour.", "The pruning workshop opens for booking this week."]),
        m("only-content-follows-a-quote", plain("Re: tiles", None, "> Are the tiles delivered?\n> Which colour?\n> Who pays?\n> When?\nGreen, delivered, I pay, on Tuesday.\n"), vec!["Green, delivered, I pay, on Tuesday."]),
        m("one-line", plain("ok", None, "ok\n"), vec!["ok"]),
        m("html-reply-blockquote", html("Re: plan", "<div>Sounds good, let us go with the second plan.</div><div class=\"gmail_quote\"><div>On Tue, 3 Nov 2026, Sam Hart &lt;sam@example.org&gt; wrote:</div><blockquote class=\"gmail_quote\">Which plan do you prefer?</blockquote></div>"), vec!["Sounds good, let us go with the second plan."]),
        m("prose-about-headers", plain("Question", None, "The From: and Subject: lines of a message tell you who wrote it and why, so read them first.\nThe Date: line comes after.\n"), vec!["The From: and Subject: lines of a message tell you who wrote it and why, so read them first.", "The Date: line comes after."]),
        m("signature-then-history", plain("Re: Quote", Some("Hugo Bernard"), "Agreed on the price.\n\nBest regards,\nHugo Bernard\n+33 6 12 34 56 78\n\nOn Mon, 2 Nov 2026 at 10:00, Ana Silva <ana@example.pt> wrote:\n> Is 400 euros acceptable?\n"), vec!["Agreed on the price."]),
        m("long-mail-whole", plain("Notes", None, "First, the meeting moved to Thursday.\nThe room is on the second floor.\n\nSecond, please read the attached budget before then.\nThe totals are in the last column.\n\nFinally, bring your own laptop.\nThe guest network will be off.\n"), vec!["First, the meeting moved to Thursday.", "The room is on the second floor.", "Second, please read the attached budget before then.", "The totals are in the last column.", "Finally, bring your own laptop.", "The guest network will be off."]),
    ]
}

/// The metric: every character the sender wrote is visible with the fold applied; also the whole text of a long mail (the owner's
/// "this attachment is weird" was a mail cut after one sentence).
#[test]
fn corpus_every_new_content_character_is_visible_with_the_fold_applied() {
    let mails = corpus();
    assert!(mails.len() >= 12, "{} mails", mails.len());
    let (mut total, mut visible) = (0usize, 0usize);
    let mut failures = Vec::new();
    for mail in &mails {
        let p = go(&mail.parts);
        for a in &mail.authored {
            let Some(at) = p.canonical.find(a) else { failures.push(format!("{}: marked text not in the canonical text: {a:?}", mail.name)); continue };
            for (o, ch) in a.char_indices() {
                total += 1;
                if visible_at(&p, at + o) { visible += 1; } else { failures.push(format!("{}: {ch:?} of {a:?} is not visible; segments {:?} fold {:?}", mail.name, kinds(&p), p.fold_from)); break; }
            }
        }
    }
    println!("new-content characters visible: {visible} of {total}");
    assert!(failures.is_empty(), "{failures:#?}");
    assert_eq!(visible, total);
    assert!(total > 400, "{total}");
}

#[test]
fn corpus_history_signature_and_footer_are_put_away_where_the_mail_has_them() {
    let want: [(&str, usize, &str, &str); 8] = [
        ("en-reply", 1, "REPLY_HISTORY", "hide"), ("fr-reply", 1, "REPLY_HISTORY", "hide"), ("de-reply", 1, "REPLY_HISTORY", "hide"),
        ("forward-with-comment", 1, "FORWARDED_CONTENT", "collapse"), ("signature", 1, "SIGNATURE", "collapse"), ("disclaimer", 1, "DISCLAIMER", "collapse"),
        ("html-reply-blockquote", 1, "REPLY_HISTORY", "hide"), ("newsletter", 1, "DISCLAIMER", "collapse"),
    ];
    let mails = corpus();
    for (name, i, kind, vis) in want {
        let mail = mails.iter().find(|m| m.name == name).expect("a corpus mail");
        let p = go(&mail.parts);
        let got = p.segments.get(i).map(|s| (s.kind.as_str(), s.visibility.as_str()));
        assert_eq!(got, Some((kind, vis)), "{name}: {:?}", kinds(&p));
    }
    // the sender's own mails have no fold
    for name in ["one-line", "inline-reply", "prose-about-headers", "long-mail-whole"] {
        let p = go(&mails.iter().find(|m| m.name == name).expect("a corpus mail").parts);
        assert!(p.segments.iter().all(|s| s.visibility == Visibility::Show), "{name}: {:?}", kinds(&p));
    }
    // a bottom post: four quoted lines on top, no attribution: collapsed (never hidden), the answer shown
    let p = go(&mails.iter().find(|m| m.name == "only-content-follows-a-quote").expect("a corpus mail").parts);
    assert_eq!(kinds(&p), vec![("REPLY_HISTORY", "collapse"), ("NEW_CONTENT", "show")]);
    assert!(p.segments[0].reasons.contains(&Reason::BottomPost), "{:?}", p.segments[0].reasons);
}

// ------------------------------------------------------------------ (c) the five safety invariants (docs/lanes/body-r2.round4.md line 14)

/// 1. Lossless: the segments in order are the canonical text, on every corpus mail and on CRLF input.
#[test]
fn invariant_1_lossless_on_every_corpus_mail() {
    let mut all: Vec<Parts> = corpus().into_iter().map(|m| m.parts).collect();
    all.push(plain("x", None, "One\r\nTwo\r\n\r\nOn Mon, 9 Nov 2026 at 10:00, A <a@example.org> wrote:\r\n> old\r\n"));
    for p in &all {
        let r = go(p);
        let joined: String = r.segments.iter().map(|s| s.text.as_str()).collect();
        assert_eq!(joined, r.canonical, "{:?}", kinds(&r));
        let mut at = 0;
        for s in &r.segments { assert_eq!(s.range.start, at); at = s.range.end; }
        assert_eq!(at, r.canonical.len());
    }
}

/// 2. Never empty: a non-empty body always has a visible segment (French and HTML quotes only, then a one-word mail).
#[test]
fn invariant_2_never_empty() {
    for p in [
        plain("Re: x", None, "Le lun. 2 nov. 2026 à 18:40, Lise Moreau <lise@example.fr> a écrit :\n> On se voit demain ?\n"),
        plain("x", None, "ok"),
        html("x", "<div class=\"gmail_quote\"><div>On Tue, 3 Nov 2026, Sam Hart &lt;sam@example.org&gt; wrote:</div><blockquote class=\"gmail_quote\">Which plan?</blockquote></div>"),
    ] {
        let r = go(&p);
        assert!(!r.canonical.trim().is_empty());
        assert!(r.segments.iter().any(|s| s.visibility == Visibility::Show), "{:?}", kinds(&r));
    }
    let r = go(&plain("Re: x", None, "Le lun. 2 nov. 2026 à 18:40, Lise Moreau <lise@example.fr> a écrit :\n> On se voit demain ?\n"));
    assert!(r.warnings.contains(&Warning::NothingVisible), "{:?}", r.warnings);
}

/// 3. Hiding requires an anchor: quote marks alone, a thread match alone and a bottom post never hide; on the whole corpus every
/// hidden segment names a structural reason.
#[test]
fn invariant_3_hiding_requires_an_anchor() {
    assert_eq!(parse::band(SegmentKind::ReplyHistory, 0.99, &[Reason::QuoteMarks]), Visibility::Collapse);
    assert_eq!(parse::band(SegmentKind::ReplyHistory, 0.99, &[Reason::ThreadExact]), Visibility::Collapse);
    assert_eq!(parse::band(SegmentKind::ReplyHistory, 0.99, &[Reason::ClientQuote]), Visibility::Hide);
    let earlier = ["Where did you leave the keys of the blue van?"];
    let r = prepare(&plain("Re: keys", None, "With the porter.\n\n> Where did you leave the keys of the blue van?\n"), &earlier, &Options::default());
    assert!(r.segments.iter().all(|s| s.visibility != Visibility::Hide), "{:?}", kinds(&r));
    for mail in corpus() {
        let r = go(&mail.parts);
        for s in &r.segments {
            if s.visibility == Visibility::Hide { assert!(s.reasons.iter().any(|x| x.is_anchor()), "{}: {:?}", mail.name, s.reasons); }
        }
    }
}

/// 4. The original is retained: both renderings come back byte for byte, CRLF and BOM included.
#[test]
fn invariant_4_the_original_is_retained() {
    let t = "\u{feff}Hi\r\nOn Mon, 9 Nov 2026, A <a@example.org> wrote:\r\n> x\r\n";
    let h = "<p onclick=\"x()\">Hi</p><script>bad()</script>";
    let r = go(&Parts { text: Some(t.into()), html: Some(h.into()), ..Default::default() });
    assert_eq!(r.original_text.as_deref(), Some(t));
    assert_eq!(r.original_html.as_deref(), Some(h));
}

/// 5. A failure never reaches the caller as an error or a panic: that one message becomes visible escaped text with a warning.
#[test]
fn invariant_5_a_failure_degrades_to_visible_escaped_text() {
    let r = prepare(&plain("x", None, "Line <one> & two"), &[], &Options { deadline: None, fault: true });
    assert!(r.degraded);
    assert_eq!(kinds(&r), vec![("UNKNOWN", "show")]);
    assert!(r.warnings.contains(&Warning::Failure));
    assert!(r.display_html.contains("Line &lt;one&gt; &amp; two"), "{}", r.display_html);
    let junk = parse::prepare_message(b"", &[], &Options::default());
    assert!(junk.degraded);
}

/// The rule above them all: nothing authored is ever behind the fold.
#[test]
fn nothing_authored_is_behind_the_fold_on_the_corpus() {
    for mail in corpus() {
        let r = go(&mail.parts);
        if let Some(f) = r.fold_from {
            assert!(f > 0 && f < r.segments.len(), "{}: fold {f}", mail.name);
            assert!(r.segments[f..].iter().all(|s| !s.kind.authored() && s.visibility != Visibility::Show), "{}: {:?} fold {f}", mail.name, kinds(&r));
        }
    }
}

fn seg(kind: SegmentKind, vis: Visibility) -> parse::Segment {
    parse::Segment { kind, text: "x".into(), html: String::new(), safe_html: String::new(), confidence: 0.9, visibility: vis, reasons: vec![Reason::AttributionLine], range: 0..1 }
}

/// `fold_from` on built segments: history after the sender's text folds; history alone folds nothing (never empty); an inline quote
/// or a shown segment after history moves the fold behind it.
#[test]
fn fold_from_on_built_segments() {
    use SegmentKind::*;
    use Visibility::*;
    assert_eq!(parse::fold_from(&[seg(NewContent, Show), seg(ReplyHistory, Hide)]), Some(1));
    assert_eq!(parse::fold_from(&[seg(ReplyHistory, Hide)]), None, "nothing left above the fold");
    assert_eq!(parse::fold_from(&[seg(ReplyHistory, Hide), seg(Signature, Collapse)]), None);
    assert_eq!(parse::fold_from(&[seg(NewContent, Show), seg(NewContent, Show)]), None);
    assert_eq!(parse::fold_from(&[seg(NewContent, Show), seg(ReplyHistory, Hide), seg(InlineQuote, Show), seg(Signature, Collapse)]), Some(3));
    assert_eq!(parse::fold_from(&[seg(NewContent, Show), seg(ReplyHistory, Hide), seg(NewContent, Show)]), None);
}

/// Owner's picture 212 (13 Nov 2026): a forward under a line of dashes showed the dashes and folded away the mail it forwarded.
/// A separator is not a comment: the forward shows and nothing folds; one real word above it and the forward folds under that word.
/// Mutant `forward-comment-counts-a-separator` (segment.rs: `&& says_something(&blocks[x].content)` removed): red on the first case.
#[test]
fn kind_forwarded_content_under_a_line_of_dashes_shows_and_folds_nothing() {
    let forward = "---------- Forwarded message ---------\nFrom: Facilities <facilities@example.net>\nDate: Tue, 10 Nov 2026 at 11:30\nSubject: Room booking\nTo: Eva Schmidt <eva@example.com>\n\nRoom 4 is booked for Thursday at ten.\n";
    let p = go(&plain("Fwd: Room booking", Some("Eva Schmidt"), &format!("---\n\n{forward}")));
    let ks = kinds(&p);
    assert!(ks.iter().any(|k| k.0 == "FORWARDED_CONTENT"), "{ks:?}");
    assert!(ks.iter().all(|k| k.1 == "show"), "the forward under a separator shows: {ks:?}");
    assert_eq!(p.fold_from, None, "{ks:?}");
    assert!(visible_at(&p, p.canonical.find("Room 4 is booked").expect("the forwarded words")), "the forwarded words are on screen without a click");
    let with = go(&plain("Fwd: Room booking", Some("Eva Schmidt"), &format!("FYI\n\n{forward}")));
    assert_eq!(kinds(&with), vec![("NEW_CONTENT", "show"), ("FORWARDED_CONTENT", "collapse")]);
    assert_eq!(with.fold_from, Some(1));
}
