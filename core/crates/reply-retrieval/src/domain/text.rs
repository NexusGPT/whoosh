//! Small text rules every step shares: counting and cutting in characters (never bytes, so a cut never splits a letter), filling a template
//! inside a budget, the words of a question, a subject without its Re:/Fwd: prefixes, a day as YYYY-MM-DD.

/// The length the budgets are measured in.
pub fn chars(s: &str) -> usize {
    s.chars().count()
}

/// The first `n` characters of `s`.
pub fn take_chars(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

/// A template with `{{id}}` placeholders filled so the result never exceeds `budget` characters. `values` are in PRIORITY order: the first gets
/// all the room it needs, the next what is left, and so on; a value with no room is cut, never refused. Text outside the placeholders is the
/// fixed part; when even the fixed part is over the budget, the whole result is cut to the budget. Returns the text and whether anything was cut.
pub fn fill_within(template: &str, values: &[(&str, &str)], budget: usize) -> (String, bool) {
    let empty: Vec<(&str, String)> = values.iter().map(|(id, _)| (*id, String::new())).collect();
    let mut room = budget.saturating_sub(chars(&render_once(template, &empty)));
    let mut cut = false;
    let mut filled: Vec<(&str, String)> = Vec::new();
    for (id, value) in values {
        let times = template.matches(&format!("{{{{{id}}}}}")).count();
        let take = if times == 0 { 0 } else { chars(value).min(room / times) };
        if times > 0 && take < chars(value) {
            cut = true;
        }
        room -= take * times;
        filled.push((*id, take_chars(value, take)));
    }
    let mut out = render_once(template, &filled);
    if chars(&out) > budget {
        out = take_chars(&out, budget);
        cut = true;
    }
    (out, cut)
}

/// Replaces each `{{id}}` of the template by its value in ONE pass, so a value that itself holds `{{...}}` is never replaced again.
/// A placeholder with no value is left as written.
fn render_once(template: &str, values: &[(&str, String)]) -> String {
    let mut out = String::new();
    let mut rest = template;
    while let Some(i) = rest.find("{{") {
        out.push_str(&rest[..i]);
        let after = &rest[i + 2..];
        match after.find("}}") {
            Some(j) => match values.iter().find(|(id, _)| *id == &after[..j]) {
                Some((_, v)) => {
                    out.push_str(v);
                    rest = &after[j + 2..];
                }
                None => {
                    out.push_str("{{");
                    rest = after;
                }
            },
            None => {
                out.push_str(&rest[i..]);
                rest = "";
            }
        }
    }
    out.push_str(rest);
    out
}

/// The words a question searches for: lower case, letters and digits, three characters or more (shorter words are "a", "le", "de").
pub fn words(s: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for w in s.split(|c: char| !c.is_alphanumeric()) {
        let w = w.to_lowercase();
        if chars(&w) >= 3 && !out.contains(&w) {
            out.push(w);
        }
    }
    out
}

/// A text as compared for duplicates: lower case, one space between words, no punctuation at the ends.
pub fn normalised(s: &str) -> String {
    let joined = s.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase();
    joined.trim_matches(|c: char| !c.is_alphanumeric()).to_string()
}

/// A subject without its reply and forward prefixes (re, fwd, fw, tr, aw, sv, any number of them, any case), trimmed, case kept. "" for none.
pub fn subject_without_prefixes(subject: &str) -> String {
    let mut s = subject.trim().to_string();
    loop {
        let lower = s.to_ascii_lowercase();
        match ["re:", "fwd:", "fw:", "tr:", "aw:", "sv:"].iter().find(|p| lower.starts_with(*p)) {
            Some(p) => s = s[p.len()..].trim_start().to_string(),
            None => break,
        }
    }
    s
}

/// A subject as two excerpts are compared by: without prefixes, one space between words, lower case. "" for none.
pub fn bare_subject(subject: &str) -> String {
    subject_without_prefixes(subject).split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase()
}

/// "YYYY-MM-DD" of an epoch millisecond (UTC), by the civil-from-days rule. The same rule as `decision::thread_context` (lane labels-r5),
/// copied rather than depended on: that crate brings the model providers and the secrets with it.
pub fn day_words(at_ms: i64) -> String {
    let days = at_ms.div_euclid(86_400_000);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}")
}
