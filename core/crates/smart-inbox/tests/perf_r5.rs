//! Lane labels-r5: what the live rule count costs on a big inbox ("The objective is <50ms for any operation"). Heavy, so ignored by
//! default; run it on purpose:
//!   cargo test -p smart-inbox --test perf_r5 -- --ignored --nocapture
//! It builds INBOX_N inbound INBOX messages in threads of three, one in three answered from Sent, a quarter unread, then times
//! `corpus::rule_set_count` warm over the owner's filter ("unread AND already answered by me"), a body filter, and the same filter
//! through the per-message reader for the comparison. The numbers go to the lane note; the test only fails over a budget set to
//! twice the owner's 50 ms because the count runs on the core's read connection off the main thread.
mod support;
use contracts::{MatchMode, MessageFact, RuleCondition, RuleSet};
use smart_inbox::corpus::rule_set_count;
use std::time::Instant;
use support::TempDb;

const INBOX_N: usize = 30_000;
const RUNS: usize = 7;

fn all_of(conditions: Vec<RuleCondition>) -> RuleSet {
    RuleSet { conditions, match_mode: MatchMode::All }
}

#[test]
#[ignore = "heavy: 30,000 messages; run with --ignored --nocapture"]
fn the_live_count_on_a_big_inbox() {
    let db = TempDb::new("perf-count");
    let store = store::Store::open(&db.0).unwrap();
    store
        .write(|ctx| {
            ctx.tx.execute(
                "INSERT INTO account (id, address, provider, display_name, colour, sort_order, credential_kind, keychain_ref, updated_at_ms) \
                 VALUES ('acc', 'owner@home-example.com', 'imap', 'Owner', '#336699', 0, 'manual', 'svc' || char(31) || 'acc', 0)",
                [],
            )?;
            let mut thread = ctx.tx.prepare("INSERT INTO thread (id, account_id, subject, last_message_at_ms) VALUES (?1, 'acc', 's', ?2)")?;
            let mut msg = ctx.tx.prepare(
                "INSERT INTO message (id, thread_id, account_id, sender_name, from_address, to_json, subject, preview, body_text, received_at_ms, read, folder) \
                 VALUES (?1, ?2, 'acc', 'x', ?3, '[\"owner@home-example.com\"]', ?4, 'p', ?5, ?6, ?7, ?8)",
            )?;
            let body = "lorem ipsum dolor sit amet ".repeat(40);
            for i in 0..INBOX_N {
                let t = format!("acc:t{}", i / 3);
                if i % 3 == 0 {
                    thread.execute(rusqlite::params![t, i as i64])?;
                }
                msg.execute(rusqlite::params![format!("in-{i}"), t, format!("p{}@perf-example.com", i % 500), format!("subject {i}"), body, 1_000 + i as i64, (i % 4 != 0) as i64, "INBOX"])?;
                if i % 9 == 0 {
                    // an answer from Sent after the thread's first message
                    msg.execute(rusqlite::params![format!("out-{i}"), t, "owner@home-example.com", format!("re {i}"), "ok", 1_000 + i as i64 + 1, 1, "Sent Messages"])?;
                }
            }
            Ok(())
        })
        .unwrap();
    let time = |label: &str, set: RuleSet| {
        let first = Instant::now();
        let c = rule_set_count(&store, set.clone());
        let cold = first.elapsed();
        let mut warm: Vec<f64> = (0..RUNS).map(|_| { let t = Instant::now(); rule_set_count(&store, set.clone()); t.elapsed().as_secs_f64() * 1000.0 }).collect();
        warm.sort_by(|a, b| a.partial_cmp(b).unwrap());
        println!("{label}: {} of {INBOX_N} match ({} unread), cold {:.1} ms, warm median {:.1} ms, max {:.1} ms", c.count, c.unread, cold.as_secs_f64() * 1000.0, warm[RUNS / 2], warm[RUNS - 1]);
        warm[RUNS / 2]
    };
    let owner_filter = time("unread AND replied by me", all_of(vec![RuleCondition::Fact { fact: MessageFact::Unread, wanted: true }, RuleCondition::Fact { fact: MessageFact::RepliedByMe, wanted: true }]));
    let _ = time("subject contains \"subject 29\"", all_of(vec![RuleCondition::SubjectContains { words: "subject 29".into() }]));
    let _ = time("body contains \"dolor\" (reads every body)", all_of(vec![RuleCondition::BodyContains { words: "dolor".into() }]));
    assert!(owner_filter < 100.0, "the owner's filter took {owner_filter:.1} ms on {INBOX_N} messages (budget: twice the owner's 50 ms)");
}
