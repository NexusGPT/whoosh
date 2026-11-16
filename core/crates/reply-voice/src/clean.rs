//! The owner's own words in a sent message: the quoted history and the signature taken out, then the cut to a size.

/// What the owner wrote in a sent message, nothing else. Takes out, in this order:
///   1. everything from the first quoted-text marker on ("On ... wrote:", "Le ... a écrit :", an Outlook header block, a closing run of "> " lines),
///      by the parse lane's own boundary (`parse::text_quote_start`, started from, not rewritten); a message that is only a quote leaves nothing;
///   2. every remaining "> " line (an inline answer keeps the owner's lines and loses the quoted ones);
///   3. the signature: from the "-- " line (RFC 3676; "--" alone counts) to the end.
/// An empty result means the message holds no words of the owner's own: the caller skips it.
pub fn clean_sent_text(raw: &str) -> String {
    let body = &raw[..quote_start(raw)];
    let mut own: Vec<&str> = Vec::new();
    for line in body.lines() {
        if line.trim_end() == "--" {
            break;
        }
        if line.trim_start().starts_with('>') {
            continue;
        }
        own.push(line);
    }
    own.join("\n").trim().to_string()
}

/// Where the quote starts in `text`, `text.len()` when nothing is quoted. `parse::text_quote_start` answers None both for "no quote" and for "nothing
/// visible before the quote"; a message that is only a quote must read as empty, not as the owner's text, so a visible line is put in front and
/// taken off again (the parse lane's rule for an Outlook header block needs a blank line before "From:", hence the second newline).
fn quote_start(text: &str) -> usize {
    const LEAD: &str = "x\n\n";
    parse::text_quote_start(&format!("{LEAD}{text}")).map_or(text.len(), |o| o.saturating_sub(LEAD.len()))
}

/// `text` cut to at most `budget` characters, at a word boundary, with an ellipsis where it was cut. A text that fits comes back unchanged.
pub fn cut_to_chars(text: &str, budget: usize) -> String {
    if budget == 0 {
        return String::new();
    }
    if text.chars().count() <= budget {
        return text.to_string();
    }
    let keep = budget - 1; // the ellipsis takes one
    let head: String = text.chars().take(keep).collect();
    let on_a_boundary = text.chars().nth(keep).is_some_and(char::is_whitespace);
    let cut = match head.rfind(char::is_whitespace) {
        Some(i) if !on_a_boundary && i > 0 => &head[..i],
        _ => head.as_str(),
    };
    format!("{}…", cut.trim_end())
}
