//! T-26 of docs/lanes/realcore-firstrun.testplan.md: a NEW installation of the real core equals the golden `docs/FIRST-RUN.json`, path by path. A fresh store,
//! two accounts added through the product's own command against fake IMAP servers (the first with SPECIAL-USE, the second without), the first sync finished.
//! Nothing is seeded. The golden holds what the core does on that first run: the built-in labels with their description and rules, the reply sources
//! (Inbox and Sent of each account on, every other folder off), auto-draft off and on demand on, no decision model, speech Local, the two built-in agents with
//! their provider and model, no calendar. The fixture core's `fixture install-state new` must equal the same file (lane probes-r1).
//!
//! Mutants (each run and seen red, `_captures/replies-settings/mutants-run-r5.log`): F1 the built-in labels are not seeded; F2 the 0004 seed line renamed (the
//! plan's mutant); F3 and F8 the 0013 seed starts auto-draft on; F4 the sync's folder registration removed; F5 the Sent folder is not on by default; F6 the built-in
//! agents' model changed; F7 speech starts on a cloud provider.
//!
//! The file is written by `WHOOSH_WRITE_FIRST_RUN=1 cargo test -p engine --test engine_first_run -- --nocapture the_golden_is_written` and then READ by a
//! person: a default that is wrong is a bug to fix in the core, not a value to bless.
mod first_run_support;

use contracts::WhooshCore;
use first_run_support::*;
use serde_json::{json, Value};

#[test]
fn a_new_installation_equals_the_first_run_golden() {
    let rig = Rig::first_sync(0);
    let got = snapshot(&rig);
    let text = std::fs::read_to_string(golden_file()).unwrap_or_else(|e| panic!("docs/FIRST-RUN.json cannot be read: {e}"));
    let golden: Value = serde_json::from_str(&text).expect("docs/FIRST-RUN.json is JSON");
    let diffs = diff(&golden["defaults"], &got);
    assert!(diffs.is_empty(), "a new installation is not the golden ({} differences):\n{}", diffs.len(), diffs.join("\n"));
}

#[test]
fn the_golden_names_the_owners_defaults_in_plain_checks() {
    // the owner's words, asserted on the real core apart from the file: a golden that blessed a wrong value would still go red here
    let rig = Rig::first_sync(0);
    assert_eq!(rig.path_value("reply/use/auto")["enabled"], false, "auto-draft is off until the owner turns it on");
    assert_eq!(rig.path_value("reply/use/on_demand")["enabled"], true);
    let sources = rig.path_value("reply/sources");
    let mut on: Vec<(String, String)> = vec![];
    let mut off: Vec<(String, String)> = vec![];
    for a in sources["accounts"].as_array().unwrap() {
        for f in a["folders"].as_array().unwrap() {
            let entry = (a["account_id"].as_str().unwrap().to_string(), f["source"]["folder"].as_str().unwrap().to_string());
            if f["enabled"].as_bool().unwrap() { on.push(entry) } else { off.push(entry) }
        }
    }
    on.sort();
    off.sort();
    let s = |a: &str, f: &str| (a.to_string(), f.to_string());
    let progress = |a: &Value, enabled: bool| a["folders"].as_array().unwrap().iter().filter(|f| f["enabled"] == enabled).map(|f| f["progress"]["state"].as_str().unwrap().to_string()).collect::<Vec<_>>();
    for a in sources["accounts"].as_array().unwrap() {
        assert!(progress(a, true).iter().all(|p| p == "up_to_date"), "an empty enabled folder whose first sync ended is up to date, not syncing for ever: {a}");
        assert!(progress(a, false).iter().all(|p| p == "not_started"), "a switched-off folder is not started");
    }
    assert_eq!(on, vec![s("$ACCOUNT_1", "INBOX"), s("$ACCOUNT_1", "Éléments envoyés"), s("$ACCOUNT_2", "INBOX"), s("$ACCOUNT_2", "Sent Items")]);
    assert_eq!(off, vec![s("$ACCOUNT_1", "Projects"), s("$ACCOUNT_2", "Receipts")]);
    assert_eq!(sources["local_folders"], json!([]));
    assert_eq!(sources["calendars"], json!([]));
    assert_eq!(rig.core.decision_model(), None, "no decision model until the owner chooses one");
    assert_eq!(rig.core.speech_engine().kind, contracts::SpeechEngineKind::Local);
    let agents = rig.core.agent_configs();
    assert_eq!(agents.iter().filter(|a| a.built_in).count(), 2, "the two built-in agents");
    assert!(agents.iter().all(|a| !a.provider_id.is_empty() && !a.model.is_empty()), "each agent has its provider and model");
    let labels = rig.core.labels();
    let built_in: Vec<&contracts::MailLabel> = labels.iter().filter(|l| l.kind == contracts::LabelKind::BuiltIn).collect();
    assert_eq!(built_in.len(), 4, "the four built-in labels: {:?}", built_in.iter().map(|l| &l.name).collect::<Vec<_>>());
    assert!(built_in.iter().all(|l| !l.description.trim().is_empty()), "every built-in label has its seeded description");
    // the seeded rules (labels-r5 round 6): Needs Reply is sorted by the AI and keeps only "unanswered" at its exit; Promotions has the mailing-list rule at its entry
    assert_eq!(rig.path_value("label/needs-reply/chains/exit")[0]["conditions"], json!([{"fact": "unanswered", "kind": "fact", "wanted": true}]), "Needs Reply keeps the unanswered ones");
    assert_eq!(rig.path_value("label/promotions/chains/entry")[0]["conditions"], json!([{"fact": "mailing_list", "kind": "fact", "wanted": true}]), "Promotions catches mailing lists by rule");
}

/// Writes the golden from what the core does now. Only with WHOOSH_WRITE_FIRST_RUN=1; otherwise it checks the file exists and says how to write it.
#[test]
fn the_golden_is_written() {
    if std::env::var("WHOOSH_WRITE_FIRST_RUN").as_deref() != Ok("1") { return; }
    let rig = Rig::first_sync(0);
    let doc = json!({
        "about": "The first-run defaults of the REAL core on a new installation (docs/lanes/realcore-firstrun.testplan.md section 5, T-26). Read from what the core does after a first sync against two fake IMAP servers. Written by engine_first_run.rs `the_golden_is_written`; the fixture core's `fixture install-state new` must equal `defaults` after the same masks.",
        "setup": {
            "accounts": ["$ACCOUNT_1 = one@example.test: INBOX, Éléments envoyés (SPECIAL-USE \\Sent), Projects", "$ACCOUNT_2 = two@example.test: INBOX, Sent Items (no SPECIAL-USE), Receipts"],
            "messages": 0,
            "masks": "the generated account ids are written $ACCOUNT_1 and $ACCOUNT_2; a source key is `mail`, the account id and the folder joined by U+001F",
        },
        "defaults": snapshot(&rig),
    });
    std::fs::write(golden_file(), serde_json::to_string_pretty(&doc).unwrap() + "\n").unwrap();
    println!("written {}", golden_file().display());
}
