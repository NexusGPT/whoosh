//! Step 7a: the label line the writer reads above each excerpt ("Joint à chaque extrait"): the number to cite, then each ticked field that has a
//! value, in the pane's order (date, sender, folder, subject), joined by " · ". An excerpt in a kept clash always shows its date (the rule
//! "keep both, dated") and ends with "≠" and the numbers of the excerpts it clashes with. No word of any language is written: values and signs only.
use crate::domain::text::day_words;
use contracts::{ExcerptFields, ReplyExcerpt};

/// The separator between the parts of a label.
pub const LABEL_SEP: &str = " · ";
/// The sign before the numbers of the excerpts a clash pairs this one with.
pub const CLASH_SIGN: &str = "≠";

/// The label of excerpt `number`. `force_date` shows the date even when its tick is off (a kept clash).
pub fn label_line(number: u32, e: &ReplyExcerpt, fields: ExcerptFields, force_date: bool, clashes_with: &[u32]) -> String {
    let mut parts = vec![format!("[{number}]")];
    let text = |v: &Option<String>| v.as_deref().map(str::trim).filter(|s| !s.is_empty()).map(str::to_string);
    if fields.date || force_date {
        parts.extend(e.date_ms.map(day_words));
    }
    if fields.sender {
        parts.extend(text(&e.sender));
    }
    if fields.folder {
        parts.extend(text(&e.folder));
    }
    if fields.subject {
        parts.extend(text(&e.subject));
    }
    if !clashes_with.is_empty() {
        let refs: Vec<String> = clashes_with.iter().map(|n| format!("[{n}]")).collect();
        parts.push(format!("{CLASH_SIGN} {}", refs.join(" ")));
    }
    parts.join(LABEL_SEP)
}
