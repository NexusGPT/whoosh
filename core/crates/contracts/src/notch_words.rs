//! What the notch shows of an open mail (owner's pick of 14 Nov 2026: "We should have its word by default an a button to display full mail maybe
//! when on the email"): the mail's own new text as paragraphs, what is folded as one line, and whether the mail has a page of its own (the notch
//! then offers "Full mail"). One pure rule over the body the reader already gets (`WhooshCore::body`): no second read, no second copy of the rule.
//!
//! The rule, in order:
//!   1. the words are the parts the engine marks as the sender's new content, shown, before the fold;
//!   2. a mail with fewer than `NOTCH_WORDS_MIN_OWN` own words whose content was forwarded reads the forwarded content (a bare forward);
//!   3. a body the engine has not prepared (no parts) reads its plain text;
//!   4. invisible characters go, a line with no letter or digit goes, a web address goes (a line that is only addresses goes whole);
//!   5. a line is a paragraph; a line under `NOTCH_WORDS_JOIN_BELOW` characters joins the paragraph before it when that one is as short and
//!      does not end a sentence (a text typed with hard line breaks reads as one paragraph);
//!   6. what is not shown is counted per kind: earlier messages, the forwarded message, signature, legal notice.
use crate::constants::{NOTCH_WORDS_JOIN_BELOW, NOTCH_WORDS_MIN_OWN};
use crate::{BodyLayout, BodySegment, MessageBody, SegmentKind, SegmentVisibility};

/// A part of the mail the notch folds into one line.
#[derive(uniffi::Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotchFoldKind { EarlierMessages, ForwardedMessage, Signature, LegalNotice }

/// One folded kind with how many parts it holds ("4 earlier messages").
#[derive(uniffi::Record, Debug, Clone, PartialEq, Eq)]
pub struct NotchFold {
    pub kind: NotchFoldKind,
    pub count: u32,
}

/// The open mail as the notch shows it by default.
#[derive(uniffi::Record, Debug, Clone, PartialEq, Eq)]
pub struct NotchWords {
    pub paragraphs: Vec<String>,
    /// in the order the notch names them: earlier messages, the forwarded message, signature, legal notice
    pub folds: Vec<NotchFold>,
    /// the mail has a designed page of its own: the notch offers "Full mail" (a written mail's words ARE the full mail: no button)
    pub has_page: bool,
    /// remote pictures the page holds (loaded when the full mail shows, if the setting says so)
    pub pictures: u32,
    /// tracking pixels in the page: never loaded
    pub trackers: u32,
}

fn is_invisible(c: char) -> bool {
    matches!(c, '\u{200b}'..='\u{200f}' | '\u{2060}'..='\u{2064}' | '\u{00ad}' | '\u{034f}' | '\u{feff}' | '\u{061c}' | '\u{180e}' | '\u{2800}' | '\u{3164}')
}

/// A web address as a mail's text carries it: http(s)://..., bare or between angle brackets.
fn is_address(word: &str) -> bool {
    let w = word.trim_start_matches('<').trim_end_matches('>');
    w.starts_with("http://") || w.starts_with("https://")
}

/// One line: invisible characters gone, runs of blanks as one space, web addresses gone. None when nothing with a letter or a digit is left.
fn clean_line(line: &str) -> Option<String> {
    let visible: String = line.chars().filter(|c| !is_invisible(*c)).map(|c| if c == '\u{00a0}' || c == '\u{2007}' { ' ' } else { c }).collect();
    let words: Vec<&str> = visible.split_whitespace().filter(|w| !is_address(w)).collect();
    let out = words.join(" ");
    if out.chars().any(|c| c.is_alphanumeric()) { Some(out) } else { None }
}

fn ends_a_sentence(p: &str) -> bool { matches!(p.chars().last(), Some('.' | '!' | '?' | ':' | '…')) }

fn paragraphs_of(text: &str) -> Vec<String> {
    let join_below = NOTCH_WORDS_JOIN_BELOW as usize;
    let mut out: Vec<String> = Vec::new();
    for line in text.split('\n').filter_map(clean_line) {
        match out.last_mut() {
            Some(prev) if prev.chars().count() < join_below && line.chars().count() < join_below && !ends_a_sentence(prev) => { prev.push(' '); prev.push_str(&line); }
            _ => out.push(line),
        }
    }
    out
}

fn text_of<'a>(parts: impl Iterator<Item = &'a BodySegment>) -> String { parts.map(|s| s.text.as_str()).collect::<Vec<_>>().join("\n") }

/// The open mail's words, its folded parts and whether it has a page of its own.
#[uniffi::export]
pub fn notch_words(body: MessageBody) -> NotchWords {
    let fold = body.fold_from.map(|n| n as usize);
    let shown = |i: usize, s: &BodySegment| s.kind == SegmentKind::NewContent && s.visibility == SegmentVisibility::Show && fold.map_or(true, |f| i < f);
    let own = text_of(body.segments.iter().enumerate().filter(|(i, s)| shown(*i, s)).map(|(_, s)| s));
    let forwarded_any = body.segments.iter().any(|s| s.kind == SegmentKind::ForwardedContent);
    let bare_forward = forwarded_any && own.split_whitespace().count() < NOTCH_WORDS_MIN_OWN as usize;
    let text = if body.segments.is_empty() {
        body.plain_text.clone()
    } else if bare_forward {
        text_of(body.segments.iter().filter(|s| matches!(s.kind, SegmentKind::NewContent | SegmentKind::ForwardedContent)))
    } else {
        own
    };
    let count = |kind: SegmentKind| body.segments.iter().enumerate().filter(|(i, s)| s.kind == kind && !shown(*i, s)).count() as u32;
    let mut folds = Vec::new();
    for (kind, n) in [
        (NotchFoldKind::EarlierMessages, count(SegmentKind::ReplyHistory)),
        (NotchFoldKind::ForwardedMessage, if bare_forward { 0 } else { count(SegmentKind::ForwardedContent).min(1) }),
        (NotchFoldKind::Signature, count(SegmentKind::Signature)),
        (NotchFoldKind::LegalNotice, count(SegmentKind::Disclaimer)),
    ] {
        if n > 0 { folds.push(NotchFold { kind, count: n }); }
    }
    NotchWords {
        paragraphs: paragraphs_of(&text),
        folds,
        has_page: body.layout == Some(BodyLayout::Designed) && body.display_html.as_deref().is_some_and(|h| !h.trim().is_empty()),
        pictures: body.pictures_blocked,
        trackers: body.pixel_trackers,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seg(kind: SegmentKind, visibility: SegmentVisibility, text: &str) -> BodySegment {
        BodySegment { kind, text: text.into(), safe_html: String::new(), confidence: 1.0, visibility, reasons: vec![], start: 0, end: 0 }
    }
    fn body(segments: Vec<BodySegment>, fold_from: Option<u32>) -> MessageBody {
        MessageBody {
            message_id: "m".into(), safe_html: None, plain_text: String::new(), trackers_blocked: 0, quote_offset: None, quoted_ranges: vec![], segments, fold_from,
            layout: Some(BodyLayout::Written), layout_reasons: vec![], display_html: None, written_html: Some("<p>x</p>".into()), body_warnings: vec![],
            pictures_blocked: 0, pixel_trackers: 0,
        }
    }
    use SegmentKind::*;
    use SegmentVisibility::*;

    /// Mutants seen red here: W1 `shown` no longer asks for `NewContent` (the history reads as words); W2 the fold index is ignored;
    /// W3 `count` counts the shown parts too.
    #[test]
    fn a_reply_reads_its_new_text_and_folds_the_rest_per_kind() {
        let w = notch_words(body(vec![
            seg(NewContent, Show, "Hi Jonas,\nThursday works for me."),
            seg(Signature, Collapse, "Mara Lind\nNorthlight"),
            seg(ReplyHistory, Collapse, "On Monday Jonas wrote:\n> can we meet"),
            seg(ReplyHistory, Collapse, "On Sunday Mara wrote:\n> yes"),
            seg(NewContent, Show, "A line under the fold the engine keeps folded."),
            seg(Disclaimer, Hide, "This message is confidential."),
        ], Some(4)));
        assert_eq!(w.paragraphs, ["Hi Jonas, Thursday works for me."]);
        assert_eq!(w.folds, [NotchFold { kind: NotchFoldKind::EarlierMessages, count: 2 }, NotchFold { kind: NotchFoldKind::Signature, count: 1 },
                             NotchFold { kind: NotchFoldKind::LegalNotice, count: 1 }]);
        assert!(!w.has_page, "a written mail's words are the full mail");
    }

    /// Mutants: W4 the bare-forward branch dropped (the two own words are all the notch says); W5 the forwarded message is listed as folded
    /// although its text is what the notch shows.
    #[test]
    fn a_bare_forward_reads_what_was_forwarded() {
        let w = notch_words(body(vec![
            seg(NewContent, Show, "FYI"),
            seg(ForwardedContent, Collapse, "The launch moves to 14 April.\nThe print run lands a week earlier."),
        ], Some(1)));
        assert_eq!(w.paragraphs, ["FYI The launch moves to 14 April.", "The print run lands a week earlier."]);
        assert!(w.folds.is_empty(), "{:?}", w.folds);
        // with enough words of its own the forward stays folded and is named once
        let own = notch_words(body(vec![
            seg(NewContent, Show, "Please read the plan below before Thursday."),
            seg(ForwardedContent, Collapse, "The launch moves."),
            seg(ForwardedContent, Collapse, "Second forwarded part."),
        ], Some(1)));
        assert_eq!(own.paragraphs, ["Please read the plan below before Thursday."]);
        assert_eq!(own.folds, [NotchFold { kind: NotchFoldKind::ForwardedMessage, count: 1 }]);
    }

    /// Mutants: W6 a web address stays in the words; W7 short lines no longer join; W8 a joined line crosses a sentence end.
    #[test]
    fn lines_become_paragraphs_and_addresses_and_invisible_characters_go() {
        let w = notch_words(body(vec![seg(NewContent, Show,
            "Your order\nis on its way\u{200b}.\n\nTrack it here: https://track.example.test/a?b=1\n<https://example.test/only-an-address>\n---\n\
             A long paragraph that is well over sixty characters stays a paragraph of its own, whatever comes next.\nThanks\u{00a0}a lot")], None));
        assert_eq!(w.paragraphs, [
            "Your order is on its way.",
            "Track it here:",
            "A long paragraph that is well over sixty characters stays a paragraph of its own, whatever comes next.",
            "Thanks a lot",
        ]);
    }

    /// Mutants: W9 `has_page` true for a written mail; W10 the picture and tracker counts swapped.
    #[test]
    fn a_designed_mail_has_a_page_and_carries_its_picture_counts() {
        let mut b = body(vec![seg(NewContent, Show, "Sale starts today.")], None);
        b.layout = Some(BodyLayout::Designed); b.display_html = Some("<table><tr><td>Sale</td></tr></table>".into()); b.written_html = None;
        b.pictures_blocked = 31; b.pixel_trackers = 1;
        let w = notch_words(b.clone());
        assert_eq!((w.has_page, w.pictures, w.trackers), (true, 31, 1));
        b.display_html = Some("  ".into());
        assert!(!notch_words(b).has_page, "an empty page is no page");
        // a written mail carries a cleaned page too (the engine prepares both): it is still no page of its own
        let mut written = body(vec![seg(NewContent, Show, "Hello.")], None);
        written.display_html = Some("<p>Hello.</p>".into());
        assert!(!notch_words(written).has_page, "a written mail has no page of its own, whatever the engine prepared");
    }

    /// A body the engine has not prepared (no parts) reads its plain text. Mutant W11: the plain-text branch dropped.
    #[test]
    fn an_unprepared_body_reads_its_plain_text() {
        let mut b = body(vec![], None);
        b.layout = None; b.written_html = None; b.plain_text = "First line\nsecond line.\n\nhttps://only.example.test".into();
        let w = notch_words(b);
        assert_eq!(w.paragraphs, ["First line second line."]);
        assert!(w.folds.is_empty() && !w.has_page);
    }
}
