//! Delta 9, groups 8 and 9: the words and recipients of Reply all and Forward. Pure, so the Rust core and the Swift FixtureCore
//! open the same draft from the same message.

/// Who a reply-all addresses: `to` first (the sender, then the other original recipients), `cc` the original cc.
#[derive(uniffi::Record, Debug, Clone, PartialEq, Eq)]
pub struct ReplyRecipients { pub to: Vec<String>, pub cc: Vec<String> }

fn same(a: &str, b: &str) -> bool { a.trim().eq_ignore_ascii_case(b.trim()) }

/// Reply all: the sender and every original recipient, minus the owner's own addresses, each once (the case ignored).
/// When the owner wrote the message, the sender is the owner and drops out, so the reply goes to the original recipients.
#[uniffi::export]
pub fn reply_all_recipients(from_address: String, to: Vec<String>, cc: Vec<String>, own_addresses: Vec<String>) -> ReplyRecipients {
    let mine = |a: &str| own_addresses.iter().any(|o| same(o, a));
    let mut out_to: Vec<String> = vec![];
    for a in std::iter::once(from_address).chain(to) {
        if !a.trim().is_empty() && !mine(&a) && !out_to.iter().any(|x| same(x, &a)) { out_to.push(a.trim().to_string()); }
    }
    let mut out_cc: Vec<String> = vec![];
    for a in cc {
        if !a.trim().is_empty() && !mine(&a) && !out_to.iter().any(|x| same(x, &a)) && !out_cc.iter().any(|x| same(x, &a)) { out_cc.push(a.trim().to_string()); }
    }
    ReplyRecipients { to: out_to, cc: out_cc }
}

/// "Re: x" / "Fwd: x", not doubled when the subject already starts with the prefix.
#[uniffi::export]
pub fn prefixed_subject(subject: String, prefix: String) -> String {
    let s = subject.trim();
    let p = format!("{}:", prefix.trim_end_matches(':'));
    if s.len() >= p.len() && s[..p.len()].eq_ignore_ascii_case(&p) { s.to_string() } else { format!("{p} {s}") }
}

/// The reply's body: an empty line for the owner's words, then the original quoted line by line.
#[uniffi::export]
pub fn quoted_reply_body(sender_name: String, when_words: String, original_body: String) -> String {
    let quoted: Vec<String> = original_body.lines().map(|l| if l.is_empty() { ">".to_string() } else { format!("> {l}") }).collect();
    format!("\n\nOn {when_words}, {sender_name} wrote:\n{}", quoted.join("\n"))
}

/// The forward's body: no words of the owner's, the original under a header that lists its attachments by name.
#[uniffi::export]
pub fn forward_body(from_name: String, from_address: String, subject: String, when_words: String, attachment_names: Vec<String>, original_body: String) -> String {
    let attach = if attachment_names.is_empty() { String::new() } else { format!("Attachments: {}\n", attachment_names.join(", ")) };
    format!("\n\n---------- Forwarded message ----------\nFrom: {from_name} <{from_address}>\nDate: {when_words}\nSubject: {subject}\n{attach}\n{original_body}")
}
