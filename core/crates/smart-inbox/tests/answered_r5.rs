//! Lane labels-r5, the facts and the live count on a 12-message IMAP account (support/imap_r5.rs: ids and threads as the IMAP sync stores
//! them, the Sent copies included, `is_outgoing` false everywhere).
//!
//! Owner, 2026-11-10: "filters don't work, it makes no sense there is no existing threads with previous response from me and that is
//! unread". Two causes were found on the real core and one is fixed here: (1) the real core answered `rule_set_count` with a constant
//! zero (engine rust_core.rs, a stub): `corpus::rule_set_count` is the real count; (2) the Swift proof builds its facts without the
//! delta 17 ones (finding in docs/lanes/labels-r5.md). These tests hold what the core says to the definitions.
//!
//! Mutants (single edits; each seen red, logs in _captures/labels-r5/mutants.out):
//! - A1 store label_meta.rs `OWNER_WROTE_EARLIER_SQL`: drop the `AND lower(n.from_address) IN (...)` line -> the_answered_fact_means_...
//! - A2 corpus.rs, the `replied` EXISTS: drop its `AND lower(n.from_address) IN (...)` line -> the_corpus_loader_..., the_live_count_...
//! - A3 corpus.rs `unread: r.get::<_, i64>(12)? == 0` -> `!= 0` -> the_live_count_...
//! - A4 corpus.rs `Needs::of`: `BodyContains => n.body = true` -> `{}` -> the_live_count_reads_the_body_...
//! - A8 corpus.rs `Needs::of`: `RepliedByMe => n.replied = true` -> `{}` -> the_unread_threads_with_a_previous_reply_..., the_projected_count_...
//! - A9 corpus.rs `rule_set_matching_ids`: `.take(limit)` -> `.take(usize::MAX)` -> the_matching_ids_are_the_rows_pane_2_draws
//! - A5 thread.rs `f.thread_before.reverse()` removed -> the_rest_of_the_thread_is_the_earlier_messages_oldest_first
//! - A6 thread.rs the `NOT_CONVERSATION_SQL` condition dropped from the query -> the_rest_of_the_thread_skips_drafts_trash_and_spam
//! - A7 thread.rs `without_quote` returns the text whole -> the_quoted_part_of_a_reply_is_not_read_twice
mod support;
#[path = "support/imap_r5.rs"]
mod imap;

use contracts::{MatchMode, MessageFact, RuleCondition, RuleSet};
use imap::{account_mail, id, thread_of, ACCOUNT, OWNER};
use smart_inbox::corpus::{corpus_facts, rule_set_count, rule_set_matching_ids};
use smart_inbox::corpus::Needs;
use smart_inbox::load::{facts_for, load_context, open_reader};
use smart_inbox::thread::{attach_thread, without_quote};
use smart_inbox::MessageFacts;
use store::Store;
use support::TempDb;

fn account(tag: &str) -> (TempDb, Store) {
    let db = TempDb::new(tag);
    let store = imap::open_imap_account(&db.0);
    (db, store)
}

fn facts(store: &Store, key: &str) -> MessageFacts {
    let conn = open_reader(store.path()).unwrap();
    let ctx = load_context(&conn).unwrap();
    facts_for(&conn, &ctx, &[id(key)]).unwrap().pop().unwrap_or_else(|| panic!("no facts for {key}"))
}

fn fact(f: MessageFact, wanted: bool) -> RuleCondition {
    RuleCondition::Fact { fact: f, wanted }
}

fn all_of(conditions: Vec<RuleCondition>) -> RuleSet {
    RuleSet { conditions, match_mode: MatchMode::All }
}

#[test]
fn the_sent_copy_joins_the_thread_of_the_message_it_answers() {
    let (_db, store) = account("thread-join");
    let t = store.thread(&thread_of(account_mail().iter().find(|m| m.key == "r1").unwrap())).unwrap().expect("thread a1");
    assert!(t.message_ids.contains(&id("r1")) && t.message_ids.contains(&id("s1")), "the reply in Sent is in the thread: {:?}", t.message_ids);
    // the store derives "outgoing" from the From address: the sync's `is_outgoing: false` is not what the reads show
    assert!(store.message(&id("s1")).unwrap().unwrap().is_outgoing);
    assert!(!store.message(&id("r1")).unwrap().unwrap().is_outgoing);
}

#[test]
fn the_answered_fact_means_i_wrote_earlier_in_the_thread() {
    // Owner ruling 2026-11-10 (replaces "a reply of mine sits after this message"): "Déjà répondu par moi" is yes when at least one stored
    // message from me, the Sent copy included, is dated BEFORE this message. "Sans réponse" stays: no message at all after this one.
    let (_db, store) = account("answered");
    // thread b1: r2 (1100), my reply s2 (1500), then a NEW unread message r3 (3000): I replied, then a new message arrived
    let r3 = facts(&store, "r3");
    assert!(r3.replied_by_me && r3.unread, "r3: I wrote earlier (s2) and r3 is unread");
    assert!(r3.unanswered, "r3: nothing came after it");
    // threads I never wrote in: no
    for key in ["r4", "r4b", "r5", "r8", "r9"] {
        assert!(!facts(&store, key).replied_by_me, "{key}: I never wrote in its thread");
    }
    // a message that is older than my reply, or the one my reply answers, has no message of mine BEFORE it: no
    for key in ["r1", "r2", "r7"] {
        assert!(!facts(&store, key).replied_by_me, "{key}: my reply came after it, not before");
    }
    // "Sans réponse" is unchanged: nothing newer in the thread
    let waiting: Vec<(&str, bool)> = ["r1", "r2", "r3", "r4", "r4b", "r5", "r7", "r8", "r9"].iter().map(|k| (*k, facts(&store, k).unanswered)).collect();
    assert_eq!(waiting, vec![("r1", false), ("r2", false), ("r3", true), ("r4", false), ("r4b", true), ("r5", true), ("r7", false), ("r8", true), ("r9", true)]);
}

#[test]
fn the_unread_threads_with_a_previous_reply_from_me_are_counted() {
    let (_db, store) = account("count");
    // "unread AND already answered by me" = the thread I wrote in, then a new message arrived unread: r3 only
    let both = rule_set_count(&store, all_of(vec![fact(MessageFact::Unread, true), fact(MessageFact::RepliedByMe, true)]));
    assert_eq!((both.count, both.unread, both.newest_at_ms), (1, 1, Some(3_000)), "r3");
    let answered = rule_set_count(&store, all_of(vec![fact(MessageFact::RepliedByMe, true)]));
    assert_eq!((answered.count, answered.unread), (1, 1), "only r3 has a message of mine before it");
    let never = rule_set_count(&store, all_of(vec![fact(MessageFact::RepliedByMe, false)]));
    assert_eq!(never.count, 8, "the other eight inbound messages");
    // "unread AND no answer yet": r3, r4b, r5 and r9 (r4 has a later message so it is not the one waiting)
    let waiting = rule_set_count(&store, all_of(vec![fact(MessageFact::Unread, true), fact(MessageFact::Unanswered, true)]));
    assert_eq!((waiting.count, waiting.unread), (4, 4), "r3, r4b, r5, r9");
    // the other facts of the grid
    assert_eq!(rule_set_count(&store, all_of(vec![fact(MessageFact::MailingList, true)])).count, 1, "r5");
    assert_eq!(rule_set_count(&store, all_of(vec![fact(MessageFact::CcMe, true)])).count, 1, "r8");
    assert_eq!(rule_set_count(&store, all_of(vec![fact(MessageFact::HasAttachment, true)])).count, 1, "r9");
    // a rule set with no condition matches nothing, as the contract says
    assert_eq!(rule_set_count(&store, all_of(vec![])).count, 0);
}

#[test]
fn the_matching_ids_are_the_rows_pane_2_draws() {
    let (_db, store) = account("matching");
    let both = all_of(vec![fact(MessageFact::Unread, true), fact(MessageFact::RepliedByMe, true)]);
    assert_eq!(rule_set_matching_ids(&store, both.clone(), 50), vec![id("r3")], "the same one the count says");
    assert_eq!(rule_set_count(&store, both.clone()).count as usize, rule_set_matching_ids(&store, both, 50).len(), "the rows and the count agree");
    // newest first, and capped
    let unread = all_of(vec![fact(MessageFact::Unread, true)]);
    assert_eq!(rule_set_matching_ids(&store, unread.clone(), 3), vec![id("r3"), id("r9"), id("r7")], "newest first");
    assert_eq!(rule_set_matching_ids(&store, unread, 1), vec![id("r3")], "capped");
    assert!(rule_set_matching_ids(&store, all_of(vec![]), 50).is_empty());
    // a fact the Swift pane could not judge before (it built its facts without them): the owner is in Cc of r8
    assert_eq!(rule_set_matching_ids(&store, all_of(vec![fact(MessageFact::CcMe, true)]), 50), vec![id("r8")]);
}

#[test]
fn the_live_count_reads_the_body_only_when_a_rule_asks_for_it() {
    let (_db, store) = account("count-body");
    let set = all_of(vec![RuleCondition::BodyContains { words: "villa".into() }]);
    assert_eq!(rule_set_count(&store, set).count, 1, "r1's body names the villa; the Sent reply is not in the INBOX corpus");
    // a count that reads no body does not load it
    let conn = open_reader(store.path()).unwrap();
    let ctx = load_context(&conn).unwrap();
    assert!(corpus_facts(&conn, &ctx, false).unwrap().iter().all(|f| f.body.is_empty()));
    assert!(corpus_facts(&conn, &ctx, true).unwrap().iter().all(|f| !f.body.is_empty()));
}

#[test]
fn the_corpus_loader_equals_the_per_message_facts() {
    let (_db, store) = account("corpus-equal");
    let conn = open_reader(store.path()).unwrap();
    let ctx = load_context(&conn).unwrap();
    let corpus = corpus_facts(&conn, &ctx, true).unwrap();
    let mut ids: Vec<String> = corpus.iter().map(|f| f.message_id.clone()).collect();
    ids.sort();
    let mut want: Vec<String> = account_mail().iter().filter(|m| m.from != OWNER).map(imap::id_of).collect();
    want.sort();
    assert_eq!(ids, want, "the corpus is the nine inbound INBOX messages; the Sent copies are not in it");
    assert!(corpus.windows(2).all(|w| w[0].received_at_ms >= w[1].received_at_ms), "newest first");
    for c in &corpus {
        let f = facts_for(&conn, &ctx, std::slice::from_ref(&c.message_id)).unwrap().pop().unwrap();
        let (a, b) = (c.rule_facts(), f.rule_facts());
        assert_eq!(a, b, "{}: the rule facts agree", c.message_id);
        assert_eq!((c.thread_id.as_str(), c.has_list_unsubscribe, c.from_owner, c.from_vip), (f.thread_id.as_str(), f.has_list_unsubscribe, f.from_owner, f.from_vip), "{}", c.message_id);
    }
}

#[test]
fn the_projected_count_equals_the_count_over_the_whole_corpus() {
    // a count reads only the columns its rules read; over every kind of condition it must still say what the whole corpus says
    let (_db, store) = account("projected");
    let conn = open_reader(store.path()).unwrap();
    let ctx = load_context(&conn).unwrap();
    let whole: Vec<contracts::MessageFacts> = corpus_facts(&conn, &ctx, true).unwrap().iter().map(MessageFacts::rule_facts).collect();
    let words = |w: &str| w.to_string();
    let conditions = vec![
        RuleCondition::From { words: words("ann") },
        RuleCondition::Domain { domain: words("clients-example.com") },
        RuleCondition::To { words: words("owner") },
        RuleCondition::Cc { words: words("owner") },
        RuleCondition::SubjectContains { words: words("invoice") },
        RuleCondition::BodyContains { words: words("villa") },
        RuleCondition::ListHeader { words: words("") },
        RuleCondition::Account { account_id: ACCOUNT.into() },
        RuleCondition::HasAttachment,
        fact(MessageFact::Unread, true),
        fact(MessageFact::Unread, false),
        fact(MessageFact::Unanswered, true),
        fact(MessageFact::Unanswered, false),
        fact(MessageFact::HasAttachment, true),
        fact(MessageFact::Starred, false),
        fact(MessageFact::VipSender, false),
        fact(MessageFact::CcMe, true),
        fact(MessageFact::MailingList, true),
        fact(MessageFact::RepliedByMe, true),
        fact(MessageFact::RepliedByMe, false),
    ];
    for c in &conditions {
        for mode in [MatchMode::Any, MatchMode::All] {
            let set = RuleSet { conditions: vec![c.clone(), fact(MessageFact::Unread, true)], match_mode: mode };
            assert_eq!(rule_set_count(&store, set.clone()), contracts::count_rule_matches(set.clone(), whole.clone()), "{set:?}");
            assert_ne!(Needs::of(&set), Needs::all(), "a count of one or two conditions does not read every column: {set:?}");
        }
    }
}

fn earlier_of(store: &Store, key: &str, up_to: usize) -> Vec<(String, bool, String)> {
    let conn = open_reader(store.path()).unwrap();
    let mut f = facts(store, key);
    attach_thread(&conn, &mut f, up_to).unwrap();
    f.thread_before.iter().map(|p| (p.from.clone(), p.from_owner, p.text.clone())).collect()
}

#[test]
fn the_rest_of_the_thread_is_the_earlier_messages_oldest_first() {
    let (_db, store) = account("thread-rest");
    // r3 (3000) sits after r2 (1100) and my reply s2 (1500); itself and later messages are not "earlier"
    let got = earlier_of(&store, "r3", 10_000);
    assert_eq!(
        got,
        vec![("bob@vendors-example.com".to_string(), false, "Please pay invoice 42.".to_string()), (OWNER.to_string(), true, "Paid this morning.".to_string())],
        "oldest first, the Sent reply marked as mine"
    );
    // the first message of a thread has nothing before it
    assert!(earlier_of(&store, "r1", 10_000).is_empty());
    // a message with a later reply reads only what is before it, never the reply
    assert!(earlier_of(&store, "r2", 10_000).is_empty());
    let _ = ACCOUNT;
}

#[test]
fn the_loader_stops_at_the_most_the_biggest_budget_can_use() {
    let (_db, store) = account("thread-limit");
    // the newest earlier message of r3 is exactly this long: asking for that many characters keeps it and walks no further back
    let got = earlier_of(&store, "r3", "Paid this morning.".len());
    assert_eq!(got.len(), 1, "{got:?}");
    assert_eq!(got[0].2, "Paid this morning.", "the newest earlier message is kept");
    assert!(earlier_of(&store, "r3", 0).is_empty(), "no budget, no read");
}

#[test]
fn the_rest_of_the_thread_skips_drafts_trash_and_spam() {
    let (_db, store) = account("thread-folders");
    let thread = thread_of(account_mail().iter().find(|m| m.key == "r3").unwrap());
    store
        .write(move |ctx| {
            for (n, (folder, at)) in [("Drafts", 2_000), ("Trash", 2_100), ("Junk", 2_200), ("[Gmail]/Spam", 2_300)].into_iter().enumerate() {
                ctx.tx.execute(
                    "INSERT INTO message (id, thread_id, account_id, sender_name, from_address, subject, preview, body_text, received_at_ms, read, folder) \
                     VALUES (?1, ?2, 'acc', 'x', 'x@x-example.com', 's', 'unsent words', 'unsent words', ?3, 1, ?4)",
                    rusqlite::params![format!("extra-{n}"), thread, at, folder],
                )?;
            }
            Ok(())
        })
        .unwrap();
    let got = earlier_of(&store, "r3", 10_000);
    assert!(got.iter().all(|(_, _, t)| t != "unsent words"), "{got:?}");
    assert_eq!(got.len(), 2, "r2 and my reply, nothing from Drafts, Trash, Junk or Spam");
}

#[test]
fn the_quoted_part_of_a_reply_is_not_read_twice() {
    assert_eq!(without_quote("Sent, see the attachment.\n\nOn Mon, 9 Nov ann wrote:\n> Can you send the quote?"), "Sent, see the attachment.");
    assert_eq!(without_quote("Merci.\n\nLe lun. 9 nov. Ann a écrit :\n> Bonjour"), "Merci.");
    assert_eq!(without_quote("Yes.\n> an older line"), "Yes.");
    // a message that is only a quote is kept whole: it must not read as empty
    assert_eq!(without_quote("> only a quote"), "> only a quote");
    // the Sent copy of the fixture reaches the loader without its quote
    let (_db, store) = account("thread-quote");
    let conn = open_reader(store.path()).unwrap();
    let mut f = facts(&store, "r1");
    f.received_at_ms = 9_999;
    f.message_id = "elsewhere".into();
    attach_thread(&conn, &mut f, 10_000).unwrap();
    let mine: Vec<&str> = f.thread_before.iter().filter(|p| p.from_owner).map(|p| p.text.as_str()).collect();
    assert_eq!(mine, vec!["Sent, see the attachment."], "the quote is cut from my reply");
}
