//! Commands read to their own tagged answer. async-imap 0.12's `uid_search`, `uid_fetch`, `list`, `select` and `uid_store`
//! stop reading when the stream ends and never read the tagged status, so a BAD, a NO, or a `* BYE` followed by a closed
//! connection comes back as an empty or partial SUCCESS (async-imap 0.12.0 src/parse.rs `parse_ids`, `parse_fetches`,
//! `parse_names`, `parse_mailbox`). A folder then reads as empty and fully fetched. Here every command is read up to its
//! tagged answer: OK is success, NO or BAD is the server's refusal in its own words, and a BYE or a stream that ends first
//! is a lost connection, which the callers retry on a new one.
use std::collections::{BTreeSet, HashMap};

use imap_proto::{AttributeValue, MailboxDatum, MessageSection, NameAttribute, Response, ResponseCode, SectionPath, Status};

use crate::adapter::{uid_set, Sess, E};
use crate::command_limit;

/// The command's verb for a refusal in words: `UID FETCH 1:* (...)` -> `UID FETCH`; a SELECT or APPEND names no mailbox here.
fn command_name(cmd: &str) -> String {
    let mut w = cmd.split_whitespace();
    match (w.next(), w.next()) {
        (Some(a @ "UID"), Some(b)) => format!("{a} {b}"),
        (Some(a), _) => a.to_string(),
        _ => "the command".to_string(),
    }
}

/// A connection the server ended before the command's tagged answer: retryable, with the server's BYE text when it sent one.
pub(crate) fn lost(bye: Option<&str>) -> E {
    let words = match bye.map(str::trim) {
        Some(w) if !w.is_empty() => format!("the server closed the connection ({w})"),
        _ => "the server closed the connection before it finished answering".to_string(),
    };
    E::Words { words, retryable: true }
}

/// Sends `cmd` and hands every untagged answer to `each`, up to the command's tagged answer.
pub(crate) async fn run_to_end(s: &mut Sess, cmd: &str, each: impl FnMut(&Response<'_>) + Send) -> Result<(), E> {
    run_to_end_coded(s, cmd, each, |_| {}).await
}

/// RFC 4315 COPYUID: the target folder's UIDVALIDITY and each moved message's (source UID, target UID).
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(crate) struct CopyUid {
    pub uid_validity: u32,
    pub pairs: Vec<(u32, u32)>,
}

fn uid_members(set: &[imap_proto::rfc4315::UidSetMember]) -> Vec<u32> {
    let mut out = Vec::new();
    for m in set {
        match m {
            imap_proto::rfc4315::UidSetMember::Uid(u) => out.push(*u),
            imap_proto::rfc4315::UidSetMember::UidRange(r) => out.extend(r.clone().take(100_000)),
        }
    }
    out
}

/// The COPYUID answer inside a response code, when it is one and its two sets pair up one to one.
fn copy_uid_of(code: &Option<ResponseCode<'_>>) -> Option<CopyUid> {
    let Some(ResponseCode::CopyUid(uid_validity, from, to)) = code else { return None };
    let (from, to) = (uid_members(from), uid_members(to));
    (from.len() == to.len() && !from.is_empty()).then(|| CopyUid { uid_validity: *uid_validity, pairs: from.into_iter().zip(to).collect() })
}

/// `UID MOVE` / `UID COPY`, read to the tagged answer, with the COPYUID the server named (untagged `* OK [COPYUID ..]` for a MOVE, the
/// tagged answer's code for a COPY). None = the server named no place (UIDPLUS is optional): the caller keeps the sync's re-read.
pub(crate) async fn run_copy(s: &mut Sess, cmd: &str) -> Result<Option<CopyUid>, E> {
    let mut found: Option<CopyUid> = None;
    let mut tagged: Option<CopyUid> = None;
    run_to_end_coded(s, cmd, |r| {
        if let Response::Data { status: Status::Ok, outcome } = r { if let Some(c) = copy_uid_of(&outcome.code) { found = Some(c); } }
    }, |code| tagged = copy_uid_of(code)).await?;
    Ok(tagged.or(found))
}

pub(crate) async fn run_to_end_coded(s: &mut Sess, cmd: &str, mut each: impl FnMut(&Response<'_>) + Send, mut done: impl FnMut(&Option<ResponseCode<'_>>) + Send) -> Result<(), E> {
    let what = command_name(cmd);
    let id = command_limit::within(&format!("{what} (sending it)"), s.s.run_command(cmd)).await?;
    let mut bye: Option<String> = None;
    loop {
        // every wait for the server's next response is under the silence limit (command_limit.rs): a server that stops answering ends in a retryable refusal
        let act = s.s.get_mut().1.clone();
        let read = command_limit::on_silence(&act, s.s.read_response()).await.map_err(|_| command_limit::timed_out(&what))?;
        let r = match read {
            Ok(Some(r)) => r,
            Ok(None) => return Err(lost(bye.as_deref())),
            // after a BYE the read error is only the closed socket: the BYE's words say more
            Err(_) if bye.is_some() => return Err(lost(bye.as_deref())),
            Err(e) => return Err(E::Words { words: format!("the connection to the server broke: {e}"), retryable: true }),
        };
        match r.parsed() {
            Response::Done { tag, status, outcome, .. } if *tag == id => {
                done(&outcome.code);
                return match status {
                    Status::Ok => Ok(()),
                    Status::Bye => Err(lost(outcome.information.as_deref())),
                    _ => Err(E::Words { words: outcome.information.as_deref().map(str::trim).filter(|w| !w.is_empty()).unwrap_or("the server refused the command").to_string(), retryable: false }),
                };
            }
            Response::Data { status: Status::Bye, outcome } => bye = Some(outcome.information.as_deref().unwrap_or("").to_string()),
            other => each(other),
        }
    }
}

/// `UID SEARCH <query>`: the UIDs the server listed; a refusal or a lost connection is an error, never an empty set.
pub(crate) async fn uid_search(s: &mut Sess, query: &str) -> Result<BTreeSet<u32>, E> {
    let mut found = BTreeSet::new();
    run_to_end(s, &format!("UID SEARCH {query}"), |r| {
        if let Response::MailboxData(MailboxDatum::Search(ids)) = r { found.extend(ids.iter().copied()); }
    }).await?;
    Ok(found)
}

/// What a SELECT answered: the numbers the adapter reads.
#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct Selected { pub exists: u32, pub uid_validity: Option<u32>, pub highest_modseq: Option<u64> }

/// `SELECT <quoted folder>`, with `(CONDSTORE)` when the server offers it.
pub(crate) async fn select(s: &mut Sess, quoted: &str, condstore: bool) -> Result<Selected, E> {
    let mut m = Selected::default();
    let cmd = format!("SELECT {quoted}{}", if condstore { " (CONDSTORE)" } else { "" });
    run_to_end(s, &cmd, |r| match r {
        Response::MailboxData(MailboxDatum::Exists(n)) => m.exists = *n,
        Response::Data { status: Status::Ok, outcome } => match &outcome.code {
            Some(ResponseCode::UidValidity(v)) => m.uid_validity = Some(*v),
            Some(ResponseCode::HighestModSeq(h)) => m.highest_modseq = Some(*h),
            _ => {}
        },
        _ => {}
    }).await?;
    Ok(m)
}

/// One FETCH answer, owned.
#[derive(Debug, Default, Clone)]
pub(crate) struct Fetched {
    pub uid: Option<u32>,
    pub flags: Vec<String>,
    pub modseq: Option<u64>,
    pub size: Option<u32>,
    pub internal_ms: Option<i64>,
    pub header: Option<Vec<u8>>,
    pub text: Option<Vec<u8>>,
    pub body: Option<Vec<u8>>,
    pub gm_msgid: Option<u64>,
    pub gm_labels: Option<Vec<String>>,
    /// body-r2: the message's sequence number in the answer (a server may answer one message in several FETCH lines).
    pub seq: u32,
    /// body-r2: the keys the answer carried, in the server's words (`UID`, `RFC822.SIZE`, `BODY[]<0>`): a body that is missing says what came instead.
    pub keys: Vec<String>,
    /// body-r2: the answer named a whole-message key and gave it no bytes (`BODY[] NIL`).
    pub body_nil: bool,
    /// body-r3: the numbered parts the answer carried (`BODY[2]`, `BODY[1.2]`): the section number and the bytes, still in their transfer encoding.
    pub sections: Vec<(String, Vec<u8>)>,
}

impl Fetched {
    /// body-r2: the second FETCH line of the same message (RFC 3501 lets a server split one message's attributes over several lines):
    /// what this one lacks, the other brings.
    fn absorb(&mut self, o: Fetched) {
        self.uid = self.uid.or(o.uid);
        if self.flags.is_empty() { self.flags = o.flags; }
        self.modseq = self.modseq.or(o.modseq);
        self.size = self.size.or(o.size);
        self.internal_ms = self.internal_ms.or(o.internal_ms);
        self.header = self.header.take().or(o.header);
        self.text = self.text.take().or(o.text);
        self.body = self.body.take().or(o.body);
        self.gm_msgid = self.gm_msgid.or(o.gm_msgid);
        self.gm_labels = self.gm_labels.take().or(o.gm_labels);
        self.keys.extend(o.keys);
        self.body_nil = self.body_nil || o.body_nil;
        self.sections.extend(o.sections);
    }
}

fn section_name(s: &Option<SectionPath>) -> String {
    match s {
        None => String::new(),
        Some(SectionPath::Full(MessageSection::Header)) => "HEADER".into(),
        Some(SectionPath::Full(MessageSection::Text)) => "TEXT".into(),
        Some(SectionPath::Full(MessageSection::Mime)) => "MIME".into(),
        Some(SectionPath::Part(n, _)) => n.iter().map(u32::to_string).collect::<Vec<_>>().join("."),
    }
}

/// The key of one FETCH attribute as a server writes it (the ones a body fetch can meet).
fn key_of(a: &AttributeValue<'_>) -> String {
    match a {
        AttributeValue::Uid(_) => "UID".into(),
        AttributeValue::Flags(_) => "FLAGS".into(),
        AttributeValue::ModSeq(_) => "MODSEQ".into(),
        AttributeValue::InternalDate(_) => "INTERNALDATE".into(),
        AttributeValue::Rfc822Size(_) => "RFC822.SIZE".into(),
        AttributeValue::Rfc822(_) => "RFC822".into(),
        AttributeValue::Rfc822Header(_) => "RFC822.HEADER".into(),
        AttributeValue::Rfc822Text(_) => "RFC822.TEXT".into(),
        AttributeValue::BodySection { section, index, .. } => format!("BODY[{}]{}", section_name(section), index.map(|i| format!("<{i}>")).unwrap_or_default()),
        AttributeValue::Unknown => "(a key this client does not read)".into(),
        _ => "(other)".into(),
    }
}

/// System flags as RFC 9051 spells them, whatever case the server used; keywords as sent.
fn flag_word(f: &str) -> String {
    const SYSTEM: [&str; 7] = ["\\Seen", "\\Answered", "\\Flagged", "\\Deleted", "\\Draft", "\\Recent", "\\*"];
    SYSTEM.iter().find(|s| s.eq_ignore_ascii_case(f)).map(|s| s.to_string()).unwrap_or_else(|| f.to_string())
}

fn fetched(attrs: &[AttributeValue<'_>]) -> Fetched {
    let mut f = Fetched::default();
    for a in attrs {
        f.keys.push(key_of(a));
        if matches!(a, AttributeValue::BodySection { section: None, data: None, .. } | AttributeValue::Rfc822(None)) { f.body_nil = true; }
        match a {
            AttributeValue::Uid(u) => f.uid = Some(*u),
            AttributeValue::Flags(fl) => f.flags = fl.iter().map(|x| flag_word(x)).collect(),
            AttributeValue::ModSeq(m) => f.modseq = Some(*m),
            AttributeValue::Rfc822Size(n) => f.size = Some(*n),
            AttributeValue::InternalDate(d) => f.internal_ms = internal_date_ms(d),
            AttributeValue::BodySection { section: Some(SectionPath::Full(MessageSection::Header)), data: Some(d), .. } | AttributeValue::Rfc822Header(Some(d)) => f.header = Some(d.to_vec()),
            AttributeValue::BodySection { section: Some(SectionPath::Full(MessageSection::Text)), data: Some(d), .. } | AttributeValue::Rfc822Text(Some(d)) => f.text = Some(d.to_vec()),
            AttributeValue::BodySection { section: None, data: Some(d), .. } | AttributeValue::Rfc822(Some(d)) => f.body = Some(d.to_vec()),
            AttributeValue::BodySection { section: Some(SectionPath::Part(n, None)), data: Some(d), .. } => f.sections.push((n.iter().map(u32::to_string).collect::<Vec<_>>().join("."), d.to_vec())),
            AttributeValue::GmailMsgId(id) => f.gm_msgid = Some(*id),
            AttributeValue::GmailLabels(l) => f.gm_labels = Some(l.iter().map(|x| x.to_string()).collect()),
            _ => {}
        }
    }
    f
}

/// A FETCH or UID FETCH command line: every message the server answered, or the error that cut it short.
pub(crate) async fn fetch(s: &mut Sess, cmd: &str) -> Result<Vec<Fetched>, E> {
    let mut out: Vec<Fetched> = Vec::new();
    let mut at: HashMap<u32, usize> = HashMap::new();
    run_to_end(s, cmd, |r| {
        let Response::Fetch(seq, attrs) = r else { return };
        let mut f = fetched(attrs);
        f.seq = *seq;
        // body-r2: one message answered in two FETCH lines (`(UID 5 RFC822.SIZE 9)` then `(BODY[] {9}...)`) is one message
        match at.get(seq) {
            Some(&i) => out[i].absorb(f),
            None => { at.insert(*seq, out.len()); out.push(f); }
        }
    }).await?;
    Ok(out)
}

/// body-r2: what a body fetch got. A UID in `missed` is absent from `got`, with the case in words: what the server sent back (its keys),
/// the size it named, and what the second and third asks (alone, then by sections) answered.
#[derive(Debug, Default)]
pub(crate) struct BodyFetch { pub got: Vec<(u32, Vec<u8>)>, pub missed: Vec<(u32, String)>, pub gone: Vec<u32> }

/// The message bytes one answer holds, or the case in words.
fn body_of(m: Option<&Fetched>, asked: usize, answered: usize) -> Result<Vec<u8>, String> {
    let Some(m) = m else { return Err(format!("no FETCH for this UID (answered {answered} of the {asked} UIDs asked)")) };
    let seen = format!("(the server named size {}; keys {})", m.size.map_or("none".to_string(), |n| n.to_string()), m.keys.join(", "));
    match &m.body {
        Some(b) if !b.is_empty() => Ok(b.clone()),
        Some(_) => Err(format!("BODY[] came back as an empty literal, 0 bytes {seen}")),
        None if m.body_nil => Err(format!("BODY[] came back NIL {seen}")),
        None => Err(format!("no BODY[] in the answer {seen}")),
    }
}

/// The header and the text of one message put together, when the server answered the two sections instead of the whole.
fn sections_of(m: Option<&Fetched>) -> Result<Vec<u8>, String> {
    match m {
        Some(Fetched { header: Some(h), text: Some(t), .. }) if !h.is_empty() => Ok([h.as_slice(), t.as_slice()].concat()),
        Some(m) => Err(format!("keys {}", m.keys.join(", "))),
        None => Err("no FETCH for this UID".to_string()),
    }
}

fn find(got: &[Fetched], uid: u32) -> Option<&Fetched> { got.iter().find(|m| m.uid == Some(uid)) }

/// `UID FETCH <uids> (UID RFC822.SIZE BODY.PEEK[])`. A UID whose answer holds no body is asked again alone, then by its two sections
/// (`BODY.PEEK[HEADER]` + `BODY.PEEK[TEXT]`, which are the whole message put together); one that still has none is in `missed` with
/// all three answers in words. A connection that breaks is an error (the caller retries on a new one); a server's refusal of one
/// of the second asks is part of that UID's case.
/// fix-realcore round 9 (item 7e1fc235, the owner's log: 240 lines for 8 UIDs over two days, each "no FETCH for this UID" in all three forms):
/// a UID that got no FETCH line at all in all three asks is looked up once (`UID SEARCH UID <n>`): not listed, it is in `gone` (the mail
/// left the folder: moved, archived or deleted from another device); still listed, it stays in `missed` with that fact in its case.
pub(crate) async fn fetch_bodies(s: &mut Sess, uids: &[u32]) -> Result<BodyFetch, E> {
    let first = fetch(s, &format!("UID FETCH {} (UID RFC822.SIZE BODY.PEEK[])", uid_set(uids))).await?;
    let answered = first.iter().filter(|m| m.uid.is_some_and(|u| uids.contains(&u))).count();
    let mut out = BodyFetch::default();
    for &uid in uids {
        // no FETCH line for this UID in every ask so far (a refused ask is not a "no FETCH")
        let mut silent = find(&first, uid).is_none();
        let case = match body_of(find(&first, uid), uids.len(), answered) {
            Ok(b) => { out.got.push((uid, b)); continue; }
            Err(c) => c,
        };
        let mut story = case;
        if uids.len() > 1 {
            let alone = ask(s, &format!("UID FETCH {uid} (UID RFC822.SIZE BODY.PEEK[])")).await?;
            silent = silent && alone.as_ref().is_ok_and(|g| find(g, uid).is_none());
            match alone.and_then(|g| body_of(find(&g, uid), 1, g.len())) {
                Ok(b) => { out.got.push((uid, b)); continue; }
                Err(c) => story.push_str(&format!("; tried alone: {c}")),
            }
        } else {
            story.push_str("; asked alone");
        }
        let parts = ask(s, &format!("UID FETCH {uid} (UID RFC822.SIZE BODY.PEEK[HEADER] BODY.PEEK[TEXT])")).await?;
        silent = silent && parts.as_ref().is_ok_and(|g| find(g, uid).is_none());
        match parts.and_then(|g| sections_of(find(&g, uid))) {
            Ok(b) => out.got.push((uid, b)),
            Err(c) => {
                story.push_str(&format!("; tried by sections: {c}"));
                if silent && !uid_search(s, &format!("UID {uid}")).await?.contains(&uid) {
                    out.gone.push(uid);
                } else {
                    if silent { story.push_str("; the server still lists this UID"); }
                    out.missed.push((uid, story));
                }
            }
        }
    }
    Ok(out)
}

/// One more ask: Ok(Err(words)) when the server refused it, Err when the connection broke.
async fn ask(s: &mut Sess, cmd: &str) -> Result<Result<Vec<Fetched>, String>, E> {
    match fetch(s, cmd).await {
        Ok(g) => Ok(Ok(g)),
        Err(E::Words { words, retryable: false }) => Ok(Err(format!("the server refused it: {words}"))),
        Err(e) => Err(e),
    }
}

/// body-r3: `UID FETCH <uid> (UID BODY.PEEK[<part>])`: the part's bytes in their transfer encoding, or the case in words.
pub(crate) async fn fetch_part(s: &mut Sess, uid: u32, part: &str) -> Result<Result<Vec<u8>, String>, E> {
    let got = fetch(s, &format!("UID FETCH {uid} (UID BODY.PEEK[{part}])")).await?;
    Ok(match find(&got, uid) {
        None => Err(format!("no FETCH for this UID (answered {} messages)", got.len())),
        Some(m) => m.sections.iter().find(|(k, _)| k == part).map(|(_, b)| b.clone()).ok_or_else(|| format!("no BODY[{part}] in the answer (keys {})", m.keys.join(", "))),
    })
}

/// The case of a single-message body fetch that held no bytes, in words (for `fetch_raw`'s error).
pub(crate) fn body_case(got: &[Fetched], uid: u32) -> String {
    body_of(find(got, uid), 1, got.len()).err().unwrap_or_else(|| "the body was there".to_string())
}

/// One LIST row: the name, its delimiter, its attributes as the server wrote them, and whether it can be selected.
pub(crate) struct ListRow { pub name: String, pub delimiter: Option<String>, pub attrs: Vec<String>, pub selectable: bool }

/// `LIST "" "*"`.
pub(crate) async fn list(s: &mut Sess) -> Result<Vec<ListRow>, E> {
    let mut rows = Vec::new();
    run_to_end(s, "LIST \"\" \"*\"", |r| {
        let Response::MailboxData(MailboxDatum::List(d)) = r else { return };
        let mut attrs = Vec::new();
        let mut selectable = true;
        for a in &d.name_attributes {
            match a {
                NameAttribute::NoSelect => selectable = false,
                // The parser turns the RFC 6154 attributes into variants of their own: written back as the server sent them.
                NameAttribute::All => attrs.push("\\All".into()),
                NameAttribute::Archive => attrs.push("\\Archive".into()),
                NameAttribute::Drafts => attrs.push("\\Drafts".into()),
                NameAttribute::Flagged => attrs.push("\\Flagged".into()),
                NameAttribute::Junk => attrs.push("\\Junk".into()),
                NameAttribute::Sent => attrs.push("\\Sent".into()),
                NameAttribute::Trash => attrs.push("\\Trash".into()),
                NameAttribute::Extension(x) => attrs.push(x.to_string()),
                _ => {}
            }
        }
        rows.push(ListRow { name: d.name.to_string(), delimiter: d.delimiter.as_deref().map(str::to_string), attrs, selectable });
    }).await?;
    Ok(rows)
}

/// INTERNALDATE ("07-Nov-2026 10:00:00 +0000", RFC 9051 date-time; the day may be space-padded) as epoch milliseconds.
pub(crate) fn internal_date_ms(s: &str) -> Option<i64> {
    const MON: [&str; 12] = ["jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec"];
    let mut parts = s.split_whitespace();
    let (date, time, zone) = (parts.next()?, parts.next()?, parts.next()?);
    let mut d = date.split('-');
    let day: i64 = d.next()?.trim().parse().ok()?;
    let name = d.next()?;
    let mon = MON.iter().position(|m| m.eq_ignore_ascii_case(name))? as i64 + 1;
    let year: i64 = d.next()?.parse().ok()?;
    let mut t = time.split(':').map(|x| x.parse::<i64>().ok());
    let (hh, mm, ss) = (t.next()??, t.next()??, t.next()??);
    let sign = match zone.as_bytes().first()? { b'+' => 1, b'-' => -1, _ => return None };
    let z: i64 = zone.get(1..)?.parse().ok()?;
    let offset_s = sign * ((z / 100) * 3600 + (z % 100) * 60);
    // Days from the civil date (Howard Hinnant's algorithm), the inverse of the adapter's `imap_date`.
    let y = if mon <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let mp = (mon + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    Some((days * 86_400 + hh * 3600 + mm * 60 + ss - offset_s) * 1000)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn internal_date_reads_the_utc_instant() {
        // 2026-11-07T10:00:00Z is 1_794_045_600 seconds after the epoch
        assert_eq!(internal_date_ms("07-Nov-2026 10:00:00 +0000"), Some(1_794_045_600_000));
        assert_eq!(internal_date_ms(" 7-Nov-2026 11:00:00 +0100"), Some(1_794_045_600_000));
        assert_eq!(internal_date_ms("01-Jan-1970 00:00:00 +0000"), Some(0));
        assert_eq!(internal_date_ms("not a date"), None);
    }

    #[test]
    fn a_lost_connection_is_retryable_and_names_the_servers_words() {
        let E::Words { words, retryable } = lost(Some("Connection closed. 14")) else { panic!("not words") };
        assert!(retryable);
        assert_eq!(words, "the server closed the connection (Connection closed. 14)");
    }
}
