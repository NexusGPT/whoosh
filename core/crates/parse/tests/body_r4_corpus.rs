//! body-r4: the corpus the round is judged on (tests/corpus/*.case, hand-written, no real mail). Each case states what it expects: its
//! `fold_from`, the sender's own text (`authored:`), the history (`history:`), segment kinds, the layout. The metric the round is judged on,
//! "new-content characters still visible", is computed with the fold applied; a single hidden authored character fails the round.
//!
//! Case file: header lines `key: value`, then `--- text`, `--- html` or `--- raw` / `--- raw-latin1` sections. `@@REPEAT n@@x@@END@@` in a
//! body is `x` written n times (hostile cases). `earlier: <file>` names a thread message in tests/corpus/thread/.
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Instant;

use parse::{prepare, prepare_message, Options, Parts, Prepared, SegmentKind, Visibility};

#[derive(Default, Debug)]
pub struct Case {
    pub name: String,
    pub subject: String,
    pub sender: Option<String>,
    pub fold_from: Option<Option<usize>>,
    pub layout: Option<String>,
    pub earlier: Vec<String>,
    pub authored: Vec<String>,
    pub history: Vec<String>,
    /// (segment index, kind, visibility)
    pub expect: Vec<(usize, String, String)>,
    pub warning: Vec<String>,
    pub text: Option<String>,
    pub html: Option<String>,
    pub raw: Option<Vec<u8>>,
}

fn expand(s: &str) -> String {
    let mut out = String::new();
    let mut rest = s;
    while let Some(a) = rest.find("@@REPEAT ") {
        out.push_str(&rest[..a]);
        let r = &rest[a + 9..];
        let at = r.find("@@").expect("@@REPEAT n@@");
        let n: usize = r[..at].trim().parse().expect("a count");
        let r = &r[at + 2..];
        let end = r.find("@@END@@").expect("@@END@@");
        out.push_str(&r[..end].repeat(n));
        rest = &r[end + 7..];
    }
    out.push_str(rest);
    out
}

pub fn corpus_dir() -> PathBuf { PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/corpus") }

pub fn load(name: &str) -> Case {
    let src = std::fs::read_to_string(corpus_dir().join(name)).expect("a corpus file");
    let mut c = Case { name: name.to_string(), ..Default::default() };
    let mut section: Option<&str> = None;
    let mut body = String::new();
    let mut finish = |section: Option<&str>, body: &mut String, c: &mut Case| {
        let b = expand(body.strip_suffix('\n').unwrap_or(body));
        match section {
            Some("text") => c.text = Some(b),
            Some("html") => c.html = Some(b),
            Some("raw") => c.raw = Some(b.replace('\n', "\r\n").into_bytes()),
            Some("raw-latin1") => c.raw = Some(b.replace('\n', "\r\n").chars().map(|ch| if (ch as u32) < 256 { ch as u32 as u8 } else { b'?' }).collect()),
            _ => {}
        }
        body.clear();
    };
    for line in src.split_inclusive('\n') {
        if let Some(s) = line.strip_prefix("--- ") {
            finish(section, &mut body, &mut c);
            section = Some(match s.trim() { "text" => "text", "html" => "html", "raw" => "raw", "raw-latin1" => "raw-latin1", x => panic!("unknown section {x}") });
            continue;
        }
        if section.is_some() { body.push_str(line); continue; }
        let l = line.trim_end_matches('\n');
        if l.is_empty() || l.starts_with('#') { continue; }
        let (k, v) = l.split_once(':').expect("key: value");
        let v = v.trim().to_string();
        match k {
            "subject" => c.subject = v,
            "sender" => c.sender = Some(v),
            "fold_from" => c.fold_from = Some(if v == "none" { None } else { Some(v.parse().expect("a number")) }),
            "layout" => c.layout = Some(v),
            "earlier" => c.earlier.push(v),
            "authored" => c.authored.push(v),
            "history" => c.history.push(v),
            "warning" => c.warning.push(v),
            "expect" => {
                let p: Vec<&str> = v.split_whitespace().collect();
                c.expect.push((p[0].parse().expect("index"), p[1].to_string(), p[2].to_string()));
            }
            _ => panic!("{name}: unknown key {k}"),
        }
    }
    finish(section, &mut body, &mut c);
    c
}

pub fn earlier_texts(c: &Case) -> Vec<String> {
    c.earlier.iter().map(|f| std::fs::read_to_string(corpus_dir().join("thread").join(f)).expect("a thread file")).collect()
}

pub fn run(c: &Case, opts: &Options) -> Prepared {
    let e = earlier_texts(c);
    let refs: Vec<&str> = e.iter().map(String::as_str).collect();
    match &c.raw {
        Some(raw) => prepare_message(raw, &refs, opts),
        None => prepare(&Parts { subject: c.subject.clone(), sender_name: c.sender.clone(), text: c.text.clone(), html: c.html.clone(), ..Default::default() }, &refs, opts),
    }
}

/// Is the character at byte `at` of the canonical text visible with the fold applied?
pub fn visible_at(p: &Prepared, at: usize) -> bool {
    p.segments.iter().enumerate().any(|(i, s)| s.range.contains(&at) && s.visibility == Visibility::Show && p.fold_from.is_none_or(|f| i < f))
}

pub fn all_cases() -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(corpus_dir()).expect("corpus dir").filter_map(|e| e.ok()).map(|e| e.file_name().to_string_lossy().into_owned()).filter(|n| n.ends_with(".case")).collect();
    names.sort();
    names
}

fn describe(p: &Prepared) -> String {
    p.segments.iter().enumerate().map(|(i, s)| format!("  [{i}] {} {} {:.2} {:?} {:?}\n", s.kind.as_str(), s.visibility.as_str(), s.confidence, s.reasons.iter().map(|r| r.as_str()).collect::<Vec<_>>(), s.text.chars().take(70).collect::<String>())).collect()
}

/// THE METRIC (the round is judged on it): every authored character of every corpus message is visible with the fold applied; every
/// expectation of every case holds. Prints history hidden %, per type, and each case's segments.
/// Mutant `r4-inline-quote-as-history` (segment.rs: the INLINE_QUOTE label made REPLY_HISTORY 0.97): red (authored context hidden).
#[test]
fn corpus_new_content_characters_visible_is_100_percent_with_the_fold_applied() {
    let names = all_cases();
    assert!(names.len() >= 30, "the corpus has {} cases", names.len());
    let (mut authored_total, mut authored_visible) = (0usize, 0usize);
    let (mut history_total, mut history_hidden, mut history_away) = (0usize, 0usize, 0usize);
    let mut per_type: BTreeMap<&'static str, (usize, usize)> = BTreeMap::new();
    let mut failures: Vec<String> = Vec::new();
    for name in &names {
        let c = load(name);
        let p = run(&c, &Options::default());
        println!("{name}: fold_from {:?}, layout {}, warnings {:?}\n{}", p.fold_from, p.layout.as_str(), p.warnings.iter().map(|w| w.as_str()).collect::<Vec<_>>(), describe(&p));
        // lossless on every case
        let joined: String = p.segments.iter().map(|s| s.text.as_str()).collect();
        if !p.canonical.trim().is_empty() && joined != p.canonical { failures.push(format!("{name}: segments do not reproduce the canonical text")); }
        for a in &c.authored {
            let Some(at) = p.canonical.find(a.as_str()) else { failures.push(format!("{name}: authored text not in the canonical text: {a:?}")); continue };
            for (o, ch) in a.char_indices() {
                authored_total += 1;
                if visible_at(&p, at + o) { authored_visible += 1; } else { failures.push(format!("{name}: authored character {ch:?} of {a:?} is not visible")); break; }
            }
        }
        for h in &c.history {
            let Some(at) = p.canonical.find(h.as_str()) else { failures.push(format!("{name}: history text not in the canonical text: {h:?}")); continue };
            for (o, _) in h.char_indices() {
                history_total += 1;
                let seg = p.segments.iter().enumerate().find(|(_, s)| s.range.contains(&(at + o)));
                if seg.is_some_and(|(_, s)| s.visibility == Visibility::Hide) { history_hidden += 1; }
                if !visible_at(&p, at + o) { history_away += 1; }
            }
        }
        for s in &p.segments {
            let e = per_type.entry(s.kind.as_str()).or_default();
            e.0 += s.text.chars().count();
            if s.visibility != Visibility::Show { e.1 += s.text.chars().count(); }
        }
        if let Some(f) = c.fold_from { if p.fold_from != f { failures.push(format!("{name}: fold_from {:?}, expected {:?}", p.fold_from, f)); } }
        if let Some(l) = &c.layout { if p.layout.as_str() != l { failures.push(format!("{name}: layout {}, expected {l} (features {:?})", p.layout.as_str(), p.layout_features)); } }
        for (i, kind, vis) in &c.expect {
            match p.segments.get(*i) {
                Some(s) if s.kind.as_str() == kind && s.visibility.as_str() == vis => {}
                Some(s) => failures.push(format!("{name}: segment {i} is {} {}, expected {kind} {vis}", s.kind.as_str(), s.visibility.as_str())),
                None => failures.push(format!("{name}: no segment {i}, expected {kind} {vis}")),
            }
        }
        for w in &c.warning { if !p.warnings.iter().any(|x| x.as_str().contains(w.as_str())) { failures.push(format!("{name}: warning {w:?} missing")); } }
        // hiding requires an anchor, on every case
        for s in &p.segments { if s.visibility == Visibility::Hide && !s.reasons.iter().any(|r| r.is_anchor()) { failures.push(format!("{name}: a hidden segment without an anchor")); } }
        // nothing authored behind the fold
        if let Some(f) = p.fold_from { if p.segments[f..].iter().any(|s| s.kind.authored() || s.kind == SegmentKind::InlineQuote) { failures.push(format!("{name}: authored segment behind the fold")); } }
    }
    let pct = |a: usize, b: usize| if b == 0 { 100.0 } else { 100.0 * a as f64 / b as f64 };
    println!("corpus: {} cases", names.len());
    println!("history hidden = {:.1} % ({history_hidden} of {history_total} characters); history put away (hidden, collapsed or folded) = {:.1} %", pct(history_hidden, history_total), pct(history_away, history_total));
    for (k, (all, away)) in &per_type { println!("type {k}: {all} characters, {:.1} % not shown", pct(*away, *all)); }
    for f in &failures { println!("FAIL {f}"); }
    println!("new-content characters still visible = {} % ({authored_visible} of {authored_total})", if authored_visible == authored_total { "100".to_string() } else { format!("{:.2}", pct(authored_visible, authored_total)) });
    assert!(failures.is_empty(), "{} failures", failures.len());
    assert_eq!(authored_visible, authored_total);
}

/// Time: each corpus message prepared 50 times, median and worst printed (store time, in the background; the owner's bar is < 50 ms).
/// A measurement, not a check: it asserts nothing, so no mutant applies (timings are not judged above load 12).
#[test]
fn corpus_time_per_message_median_and_worst_of_50() {
    let load_avg = std::process::Command::new("/usr/sbin/sysctl").args(["-n", "vm.loadavg"]).output().map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string()).unwrap_or_else(|e| format!("unread: {e}"));
    println!("load average {load_avg}; profile {}", if cfg!(debug_assertions) { "debug (unoptimised)" } else { "release" });
    let mut worst_median = 0f64;
    for name in all_cases() {
        let c = load(&name);
        let mut ms: Vec<f64> = (0..50).map(|_| { let t = Instant::now(); let _ = run(&c, &Options::default()); t.elapsed().as_secs_f64() * 1000.0 }).collect();
        ms.sort_by(|a, b| a.partial_cmp(b).expect("a number"));
        println!("time {name}: median {:.2} ms, worst {:.2} ms", ms[25], ms[49]);
        worst_median = worst_median.max(ms[25]);
    }
    println!("time: the largest median is {worst_median:.2} ms");
}

/// Each client's reply shape (Gmail, Outlook desktop, Outlook on the web, Apple Mail, Thunderbird, Proton, Yahoo, Zimbra): the earlier
/// message is REPLY_HISTORY, hidden, on a structural reason, and the sender's text is shown.
/// Mutant `r4-containers-unread` (segment.rs: the containers.txt rows skipped): red.
#[test]
fn eight_clients_reply_shapes_hide_their_history_on_a_structural_reason() {
    for name in ["gmail-reply.case", "outlook-desktop-reply.case", "outlook-web-reply.case", "apple-mail-reply.case", "thunderbird-reply.case", "proton-reply.case", "yahoo-reply.case", "zimbra-reply.case"] {
        let c = load(name);
        let p = run(&c, &Options::default());
        let last = p.segments.last().expect("segments");
        assert_eq!((last.kind, last.visibility), (SegmentKind::ReplyHistory, Visibility::Hide), "{name}: {}", describe(&p));
        assert!(last.reasons.iter().any(|r| matches!(r, parse::Reason::ClientQuote | parse::Reason::ClientSeparator | parse::Reason::ClientAttribution)), "{name}: {:?}", last.reasons);
        assert_eq!((p.segments[0].kind, p.segments[0].visibility), (SegmentKind::NewContent, Visibility::Show), "{name}");
    }
}

/// Owner pick B3: four designed mails and four written mails land on their side; the features of each are printed.
/// Mutant `r4-layout-strong-signals-ignored` (display.rs decide_layout: `if medium.len() >= 2 {`): red.
#[test]
fn four_designed_and_four_written_mails_land_on_their_side() {
    let designed = ["shop-confirmation.case", "newsletter-style-block.case", "invoice-table.case", "calendar-notification.case"];
    let written = ["gmail-reply.case", "outlook-desktop-reply.case", "apple-mail-reply.case", "coloured-word-reply.case"];
    let mut wrong = Vec::new();
    for (names, want) in [(designed, "designed"), (written, "written")] {
        for name in names {
            let p = run(&load(name), &Options::default());
            println!("layout {name}: {} {:?} features {:?}", p.layout.as_str(), p.layout_reasons.iter().map(|r| r.as_str()).collect::<Vec<_>>(), p.layout_features);
            if p.layout.as_str() != want { wrong.push(format!("{name}: {} (wanted {want})", p.layout.as_str())); }
            assert_eq!(p.written_html.is_some(), want == "written", "{name}: a written mail has its second document, a designed one none");
        }
    }
    assert!(wrong.is_empty(), "{wrong:?}");
}
