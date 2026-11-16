//! Lane fix-aisort rounds 5 and 6, the hidden check `checks/laya-sanity.py` only (a rig copy with `WHOOSH_CORE_CHECK_LAYA=<cases.tsv>` and
//! `WHOOSH_CORE_CHECK_LAYA_OUT=<table.json>`): synthetic mails x labels through the REAL decision port the label flow uses on the owner's Mac
//! (the shell's Laya, `decision_model::LabelModel::on_device` -> `LayaPort`, which also puts the state through `state_for_laya`), with the
//! per-label walk's own state (`decide::state_for` with the subject and the body ticked). The owner's copy never reads the variables (the
//! `rig` guard), and nothing here is on the owner's path.
//!
//! Round 6: each mail is asked every statement FORM at once: form `a` is today's `decide::label_statement` (built here, so it is the
//! production wording), the other forms are given by the check. Answers are keyed `<form>#<label id>`. Then, when the shell handed over
//! the meaning model (`Indexer::embedder`, read-only: nothing is indexed), the cosine between each mail (subject in the title slot, body as
//! the text: `embed_titled`, the index's document form) and each label description (`embed_query`, the search's query form).
//!
//! The cases file (tab-separated, one record per line; `\n` and `\t` escaped as backslash-n and backslash-t) is written by the check AFTER it
//! has put the model folders in place, so the thread waits for it (`WAIT_FOR_CASES_S`, `WAIT_FOR_EMBEDDER_S`: a check's numbers with no product
//! meaning). Records: `label <tab> id <tab> name <tab> description`; `form <tab> key <tab> label id <tab> statement`;
//! `mail <tab> id <tab> from <tab> subject <tab> body <tab> list (yes|no)`.
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use contracts::{default_ai_reads, DecisionAnswer, DecisionPort, DecisionQuestion, Embedder};

const CASES_VARIABLE: &str = "WHOOSH_CORE_CHECK_LAYA";
const OUT_VARIABLE: &str = "WHOOSH_CORE_CHECK_LAYA_OUT";
const WAIT_FOR_CASES_S: u64 = 300;
const WAIT_FOR_EMBEDDER_S: u64 = 180;
/// The key of the production wording (`decide::label_statement`).
const FORM_TODAY: &str = "a";

/// Where the meaning model is read from, asked again until it is there (the shell hands it over after its look).
pub(crate) type EmbedderSource = Box<dyn Fn() -> Option<Arc<dyn Embedder>> + Send>;

/// Starts the run on its own thread when this is a rig copy and both variables are set; else nothing.
pub(crate) fn start(rig: bool, port: Option<Arc<dyn DecisionPort>>, embedder: EmbedderSource) {
    if !rig {
        return;
    }
    let (Ok(cases), Ok(out)) = (std::env::var(CASES_VARIABLE), std::env::var(OUT_VARIABLE)) else { return };
    let (cases, out) = (PathBuf::from(cases), PathBuf::from(out));
    let spawned = std::thread::Builder::new().name("whoosh-laya-check".into()).spawn(move || {
        let table = match port {
            Some(port) => run(&*port, &embedder, &cases, &out),
            None => format!("{{\"done\":true,\"error\":{}}}", s("no on-device decision model was handed to the core")),
        };
        write(&out, &table);
    });
    if let Err(e) = spawned {
        eprintln!("laya check: the thread could not start: {e}");
    }
}

fn s(v: &str) -> String {
    let mut o = String::from("\"");
    for c in v.chars() {
        match c { '"' => o.push_str("\\\""), '\\' => o.push_str("\\\\"), c if (c as u32) < 0x20 => o.push_str(&format!("\\u{:04x}", c as u32)), c => o.push(c) }
    }
    o.push('"');
    o
}

fn num(p: f64) -> String {
    if p.is_finite() { format!("{p}") } else { "null".into() }
}

fn write(out: &Path, text: &str) {
    let tmp = out.with_extension("tmp");
    if std::fs::write(&tmp, text).and_then(|_| std::fs::rename(&tmp, out)).is_err() {
        eprintln!("laya check: {} could not be written", out.display());
    }
}

fn unescape(v: &str) -> String {
    v.replace("\\n", "\n").replace("\\t", "\t")
}

fn cosine(a: &[f32], b: &[f32]) -> f64 {
    let dot: f64 = a.iter().zip(b).map(|(x, y)| f64::from(*x) * f64::from(*y)).sum();
    let na: f64 = a.iter().map(|x| f64::from(*x) * f64::from(*x)).sum::<f64>().sqrt();
    let nb: f64 = b.iter().map(|x| f64::from(*x) * f64::from(*x)).sum::<f64>().sqrt();
    if na == 0.0 || nb == 0.0 { f64::NAN } else { dot / (na * nb) }
}

fn of_kind<'a>(records: &'a [Vec<String>], kind: &str, fields: usize) -> Vec<&'a Vec<String>> {
    records.iter().filter(|r| r.first().map(String::as_str) == Some(kind) && r.len() >= fields).collect()
}

fn run(port: &dyn DecisionPort, embedder: &EmbedderSource, cases: &Path, out: &Path) -> String {
    let t0 = Instant::now();
    while !cases.exists() {
        if t0.elapsed() > Duration::from_secs(WAIT_FOR_CASES_S) {
            return format!("{{\"done\":true,\"error\":{}}}", s(&format!("{} never appeared", cases.display())));
        }
        std::thread::sleep(Duration::from_millis(500));
    }
    let Ok(text) = std::fs::read_to_string(cases) else { return format!("{{\"done\":true,\"error\":{}}}", s("the cases file could not be read")) };
    let records: Vec<Vec<String>> = text.lines().filter(|l| !l.trim().is_empty()).map(|l| l.split('\t').map(unescape).collect()).collect();
    let labels = of_kind(&records, "label", 4);
    let forms = of_kind(&records, "form", 4);
    let mails = of_kind(&records, "mail", 6);
    // round 8: `judge` in the cases file = the label flow's own per-label question (ids `labels/model#<label>`), so the shell's router decides
    // which model answers (Apple's when the check armed it with WHOOSH_RIG_APPLE_JUDGE=real); the statement forms are not asked
    let judge_mode = !of_kind(&records, "judge", 1).is_empty();
    let laya_t0 = Instant::now();
    let mut rows: Vec<String> = Vec::new();
    for mail in &mails {
        // the state as the walk builds it for a label that reads the subject and the body (`decide::state_for`)
        let facts = smart_inbox::MessageFacts {
            message_id: mail[1].clone(),
            from_address: mail[2].clone(),
            to: vec!["owner@example.test".into()],
            subject: mail[3].clone(),
            body: mail[4].clone(),
            has_list_header: mail[5] == "yes",
            ..Default::default()
        };
        let state = decision::decide::state_for(&facts, default_ai_reads());
        // form a (the production wording) for every label, then each form the check gives, in the cases file's order
        let mut questions: Vec<DecisionQuestion> = labels
            .iter()
            .map(|l| DecisionQuestion::Noul {
                id: if judge_mode { decision::decide::label_question_id(decision::LABELS_MODEL_ID, &l[1]) } else { format!("{FORM_TODAY}#{}", l[1]) },
                prompt: decision::decide::label_statement(&l[2], &l[3]),
            })
            .collect();
        if !judge_mode {
            questions.extend(forms.iter().map(|f| DecisionQuestion::Noul { id: format!("{}#{}", f[1], f[2]), prompt: f[3].clone() }));
        }
        let started = Instant::now();
        let row = match port.decide(state, questions) {
            Ok(r) => {
                let yes: Vec<String> = r
                    .answers
                    .iter()
                    .filter_map(|a| match a {
                        DecisionAnswer::Noul { id, probability, .. } => Some(format!("{}:{}", s(id), num(*probability))),
                        _ => None,
                    })
                    .collect();
                format!("{{\"checkpoint\":{},\"mail\":{},\"ms\":{},\"yes\":{{{}}}}}", s(r.checkpoint.as_deref().unwrap_or("")), s(&mail[1]), started.elapsed().as_millis(), yes.join(","))
            }
            Err(e) => format!("{{\"error\":{},\"mail\":{}}}", s(&e.to_string()), s(&mail[1])),
        };
        rows.push(row);
        write(out, &format!("{{\"done\":false,\"rows\":[{}]}}", rows.join(",")));
    }
    let laya_s = laya_t0.elapsed().as_secs_f64();
    let similarity = similarity(embedder, &labels, &mails);
    format!("{{\"done\":true,\"laya_seconds\":{},\"rows\":[{}],{}}}", num(laya_s), rows.join(","), similarity)
}

/// `"similarity":{mail:{label:cosine}}` and its seconds, or `"similarity_error":"..."` (never fatal: the Laya table stands without it).
fn similarity(embedder: &EmbedderSource, labels: &[&Vec<String>], mails: &[&Vec<String>]) -> String {
    let t0 = Instant::now();
    let model = loop {
        if let Some(m) = embedder() {
            break m;
        }
        if t0.elapsed() > Duration::from_secs(WAIT_FOR_EMBEDDER_S) {
            return format!("\"similarity_error\":{}", s("the meaning model was not handed over (no --weights, or the shell found none)"));
        }
        std::thread::sleep(Duration::from_millis(500));
    };
    let waited = t0.elapsed().as_secs_f64();
    let mut descriptions = Vec::new();
    for l in labels {
        match model.embed_query(l[3].clone()) {
            Ok(v) => descriptions.push((l[1].clone(), v)),
            Err(e) => return format!("\"similarity_error\":{}", s(&format!("a description could not be embedded: {e}"))),
        }
    }
    let mut per_mail = Vec::new();
    for m in mails {
        let v = match model.embed_titled(Some(m[3].clone()), vec![m[4].clone()]) {
            Ok(mut vs) if !vs.is_empty() => vs.remove(0),
            Ok(_) => return format!("\"similarity_error\":{}", s("the model gave no vector for a mail")),
            Err(e) => return format!("\"similarity_error\":{}", s(&format!("a mail could not be embedded: {e}"))),
        };
        let cells: Vec<String> = descriptions.iter().map(|(id, d)| format!("{}:{}", s(id), num(cosine(&v, d)))).collect();
        per_mail.push(format!("{}:{{{}}}", s(&m[1]), cells.join(",")));
    }
    format!("\"similarity\":{{{}}},\"similarity_seconds\":{},\"similarity_waited_seconds\":{}", per_mail.join(","), num(t0.elapsed().as_secs_f64() - waited), num(waited))
}
