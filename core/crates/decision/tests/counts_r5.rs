//! Lane labels-r5, the three counts of a label's cards on a 12-message IMAP account (smart-inbox/tests/support/imap_r5.rs: the ids,
//! the threads and the Sent copies as the IMAP sync stores them). The whole path is the real one except the model: the store, the facts
//! read from the synced columns (`smart_inbox::load::facts_for`), the labelling order (`DecideMessage`), the filings written the way the
//! engine writes them (`label_file_flow`, `flow_rejection_put`), then the figures read back from the store (`label_flow_filings`) and
//! counted by the contract (`count_label_flow`).
//!
//! The account: label "Contracts" (entry rule: subject contains "contract"); label "Reply soon" (the AI files into it from its meaning,
//! its exit filter keeps what is still waiting for my answer = fact Sans réponse, the rest go to Primary).
//! - nine inbound messages: r7 "Contract renewal" is caught by Contracts' entry rule (the model never sees it);
//! - the model files the other eight under Reply soon (p 0.9 over the trust level);
//! - the exit filter keeps r3, r4b, r5, r8, r9 (nothing newer in their thread) and sends r1, r2 (my reply came after) and r4 (carl wrote
//!   again after it) to Primary.
//! So: Contracts = entry 1; Reply soon = entry 0, sorted by the AI 8, kept by the exit filter 5, sent to the fallback 3.
//!
//! Mutants (single edits; each seen red, logs in _captures/labels-r5/mutants.out):
//! - C1 flow.rs `after_ai`: the `ExitVerdict::Kept` and `ExitVerdict::Rejected` arms swapped -> the_three_counts_match_the_twelve_message_account
//! - C2 smart-inbox load.rs `complete`: `f.unanswered = !f.from_owner && !later` -> `... && later` -> the_three_counts_match_the_twelve_message_account
//! - C3 store label_meta.rs `label_filed_count`: drop `WHERE label_id = ?1` -> the_filed_count_is_what_the_rerun_offer_reads
mod support;
#[path = "../../smart-inbox/tests/support/imap_r5.rs"]
mod imap;

use contracts::{count_label_flow, labels_without_views, ChainPlacement, DecisionProviderKind, FlowCard, LabelScope, LabelSpec, MatchMode, MessageFact, RuleCondition, RuleSet};
use decision::{DecideMessage, DecisionSettings, FlowPlace, LabelNode, Outcome, OutcomeList, Verdict};
use imap::{account_mail, id, OWNER};
use smart_inbox::load::{facts_for, load_context, open_reader};
use store::Store;
use support::{choice, local, FakeLocal};

const NOW: i64 = 5_000;

fn spec(name: &str, description: &str, rules: RuleSet) -> LabelSpec {
    LabelSpec { name: name.into(), parent_id: None, colour: None, description: description.into(), scope: LabelScope::All, rules, pinned: false }
}

struct Account {
    _db: support::si::TempDb,
    store: Store,
    contracts: String,
    reply: String,
}

/// The twelve messages in a store, the two labels with their cards, nothing filed yet.
fn account(tag: &str) -> Account {
    let db = support::si::TempDb::new(tag);
    let store = imap::open_imap_account(&db.0);
    let contracts = store
        .label_create_full(spec("Contracts", "", RuleSet { conditions: vec![RuleCondition::SubjectContains { words: "contract".into() }], match_mode: MatchMode::Any }), NOW)
        .unwrap()
        .id;
    let reply = store.label_create_full(spec("Reply soon", "mail that still waits for my answer", RuleSet { conditions: vec![], match_mode: MatchMode::Any }), NOW).unwrap().id;
    let waiting = RuleSet { conditions: vec![RuleCondition::Fact { fact: MessageFact::Unanswered, wanted: true }], match_mode: MatchMode::All };
    store.label_chain_add(reply.clone(), ChainPlacement::Exit, waiting, NOW).unwrap();
    store.label_set_exit_fallback(reply.clone(), Some("primary".into()), NOW).unwrap();
    store.label_set_card(reply.clone(), FlowCard::Exit, true, NOW).unwrap();
    Account { _db: db, store, contracts, reply }
}

/// Runs the labelling order on the nine inbound messages, oldest first, and writes what it decides the way the engine does.
fn file_the_inbox(a: &Account) -> Vec<Verdict> {
    let labels = labels_without_views(a.store.labels_full().unwrap());
    let rules = a.store.label_rules_all().unwrap();
    let nodes = LabelNode::with_chains(&labels.iter().map(LabelNode::from_label).collect::<Vec<_>>(), &rules);
    let mut settings = DecisionSettings::shipped(DecisionProviderKind::Local, &[Outcome::Labels]);
    settings.page_mut(Outcome::Labels).unwrap().list = OutcomeList::labels_from_ledger(&nodes, &rules).expect("the ledger holds MODEL_TRUST_LABEL");
    let model = local(FakeLocal::new(|_, _, _| choice("Reply soon", 0.9)));
    let conn = open_reader(a.store.path()).unwrap();
    let ctx = load_context(&conn).unwrap();
    let mut inbound: Vec<_> = account_mail().into_iter().filter(|m| m.from != OWNER).collect();
    inbound.sort_by_key(|m| m.at);
    let ids: Vec<String> = inbound.iter().map(imap::id_of).collect();
    let mut out = Vec::new();
    for f in facts_for(&conn, &ctx, &ids).unwrap() {
        // A140: the labelling order is the three-step pipeline (Contracts has no description: AI-off, its step 1 files; Reply soon's step 1 is open)
        let d = decision::flow::decide_pipeline(&DecideMessage { settings: &settings, engine: Some(&model), tabs: &[], labels: &nodes }, &f);
        let v = d.get(Outcome::Labels).cloned().expect("the Labels page is on");
        if let Some(fl) = v.flow.clone() {
            let (mid, by, entry, p) = (f.message_id.clone(), v.decided_by.to_contract(), v.decided_by.decided_entry_id(), v.decided_by.probability());
            let rejection = fl.rejection(&mid, NOW);
            a.store
                .write(move |ctx| {
                    if let FlowPlace::Label(label) = &fl.place {
                        ctx.label_file_flow(label, &mid, &by, entry.as_deref(), p, fl.card, fl.chain_id, NOW)?;
                    }
                    if let Some(r) = &rejection {
                        ctx.flow_rejection_put(r)?;
                    }
                    Ok(())
                })
                .unwrap();
        }
        out.push(v);
    }
    out
}

#[test]
fn the_three_counts_match_the_twelve_message_account() {
    let a = account("counts");
    assert_eq!(account_mail().len(), 12);
    let verdicts = file_the_inbox(&a);
    assert_eq!(verdicts.len(), 9, "nine inbound messages are walked");
    let filings = a.store.label_flow_filings().unwrap();
    let reply = count_label_flow(a.reply.clone(), filings.clone());
    let contracts = count_label_flow(a.contracts.clone(), filings);
    assert_eq!((contracts.caught_by_entry, contracts.sorted_by_ai, contracts.kept_by_exit, contracts.sent_to_fallback), (1, 0, 0, 0), "Contracts: r7 by its entry rule");
    assert_eq!((reply.caught_by_entry, reply.sorted_by_ai, reply.kept_by_exit, reply.sent_to_fallback), (0, 8, 5, 3), "Reply soon: 8 sorted by the AI, 5 kept, 3 sent to Primary");
    // where each message ended, by name: the facts of the real threads decided the exit filter
    let mut kept: Vec<String> = verdicts.iter().filter(|v| v.flow.as_ref().is_some_and(|f| f.place == FlowPlace::Label(a.reply.clone()))).map(|v| v.message_id.clone()).collect();
    kept.sort();
    let mut want: Vec<String> = ["r3", "r4b", "r5", "r8", "r9"].iter().map(|k| id(k)).collect();
    want.sort();
    assert_eq!(kept, want);
    let mut sent: Vec<String> = verdicts.iter().filter(|v| v.flow.as_ref().is_some_and(|f| f.place == FlowPlace::Primary)).map(|v| v.message_id.clone()).collect();
    sent.sort();
    let mut want: Vec<String> = ["r1", "r2", "r4"].iter().map(|k| id(k)).collect();
    want.sort();
    assert_eq!(sent, want, "r1 and r2 are answered by my Sent reply; r4 has a later message");
}

#[test]
fn the_filed_count_is_what_the_rerun_offer_reads() {
    let a = account("filed");
    assert_eq!((a.store.label_filed_count(&a.reply).unwrap(), a.store.label_filed_count(&a.contracts).unwrap()), (0, 0));
    file_the_inbox(&a);
    // "Re-run on N filed messages": the five Reply soon kept and the one Contracts entry catch; the three sent to Primary hold no label
    assert_eq!((a.store.label_filed_count(&a.reply).unwrap(), a.store.label_filed_count(&a.contracts).unwrap()), (5, 1));
    assert_eq!(a.store.label_filed_count("no-such-label").unwrap(), 0);
}

#[test]
fn the_live_count_of_the_entry_rule_is_the_one_the_filing_made() {
    let a = account("entry-count");
    // the sheet's "N messages" for Contracts' rule is what the entry card then catches
    let rule = a.store.label_rules(&a.contracts).unwrap();
    let live = smart_inbox::corpus::rule_set_count(&a.store, rule);
    assert_eq!((live.count, live.unread, live.newest_at_ms), (1, 1, Some(1_600)));
    file_the_inbox(&a);
    let caught = count_label_flow(a.contracts.clone(), a.store.label_flow_filings().unwrap()).caught_by_entry;
    assert_eq!(live.count, caught, "the count the owner reads before saving is the number the entry card catches");
}
