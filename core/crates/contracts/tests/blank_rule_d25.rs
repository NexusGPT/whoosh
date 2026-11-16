//! Delta 25 (report A125: a label's Re-run preview said "38 join" with an empty rule): a text condition with a blank value matches NOTHING, and saving one is refused in words.
//! MUTANTS (_captures/replies-contract/mutants.py B01..B07; each must turn the test named red):
//!   B01 `has` lets an empty needle match (the fault itself)                                  -> every_text_condition_with_a_blank_value_matches_nothing
//!   B02 `has` stops trimming the needle (a whitespace needle matches a string with a space)  -> every_text_condition_with_a_blank_value_matches_nothing
//!   B03 a blank list-header value matches any list again                                     -> every_text_condition_with_a_blank_value_matches_nothing
//!   B04 a blank account id matches a message with no account                                 -> every_text_condition_with_a_blank_value_matches_nothing
//!   B05 `condition_is_blank` forgets the list header                                         -> saving_a_rule_with_no_value_is_refused_in_the_same_words_everywhere
//!   B06 the sheet's refusal uses other words                                                 -> saving_a_rule_with_no_value_is_refused_in_the_same_words_everywhere
//!   B07 a blank rule in a rerun files messages (the 38 of the report)                        -> a_rerun_over_38_messages_with_one_blank_rule_joins_none
use contracts::*;

fn msg(i: usize) -> MessageFacts {
    MessageFacts { message_id: format!("m{i}"), account_id: "acc-1".into(), from_name: "Laura Meier".into(), from_address: "laura@nordlicht.example".into(), to: vec!["me@x.test".into()], cc: vec!["cc@x.test".into()],
        subject: "Quote for Q4".into(), body: "hello world".into(), has_attachment: false, list_header: "<news.example.com>".into(), unread: false, received_at_ms: i as i64,
        unanswered: false, starred: false, vip_sender: false, cc_me: false, replied_by_me: false }
}
fn one(c: RuleCondition) -> RuleSet { RuleSet { conditions: vec![c], match_mode: MatchMode::All } }
fn kinds(v: &str) -> Vec<RuleCondition> {
    vec![RuleCondition::From { words: v.into() }, RuleCondition::To { words: v.into() }, RuleCondition::Cc { words: v.into() }, RuleCondition::Domain { domain: v.into() },
         RuleCondition::SubjectContains { words: v.into() }, RuleCondition::BodyContains { words: v.into() }, RuleCondition::ListHeader { words: v.into() }, RuleCondition::Account { account_id: v.into() }]
}

#[test]
fn every_text_condition_with_a_blank_value_matches_nothing() {
    for blank in ["", " ", "  \t "] {
        for c in kinds(blank) { assert!(!rule_set_matches(one(c.clone()), msg(1)), "{c:?} with {blank:?} must match nothing"); }
    }
    // a message whose fields hold a space does not match a whitespace needle either (the needle is trimmed first)
    let spacey = MessageFacts { subject: "a b".into(), body: "a b".into(), from_name: "A B".into(), ..msg(1) };
    assert!(!rule_set_matches(one(RuleCondition::SubjectContains { words: " ".into() }), spacey.clone()) && !rule_set_matches(one(RuleCondition::BodyContains { words: " ".into() }), spacey));
    // a message with no account id never matches a blank account
    assert!(!rule_set_matches(one(RuleCondition::Account { account_id: "".into() }), MessageFacts { account_id: "".into(), ..msg(1) }));
    // a real value still matches
    for (c, name) in [(RuleCondition::From { words: "laura".into() }, "from"), (RuleCondition::To { words: "me@".into() }, "to"), (RuleCondition::Cc { words: "cc@".into() }, "cc"), (RuleCondition::Domain { domain: "nordlicht.example".into() }, "domain"),
        (RuleCondition::SubjectContains { words: "QUOTE".into() }, "subject"), (RuleCondition::BodyContains { words: "world".into() }, "body"), (RuleCondition::ListHeader { words: "news".into() }, "list header"), (RuleCondition::Account { account_id: "acc-1".into() }, "account")] {
        assert!(rule_set_matches(one(c), msg(1)), "a real {name} value matches");
    }
    // the value is trimmed before it is looked for: " laura " finds laura
    assert!(rule_set_matches(one(RuleCondition::From { words: "  LAURA ".into() }), msg(1)) && rule_set_matches(one(RuleCondition::SubjectContains { words: " quote for ".into() }), msg(1)), "padding around a real value does not stop it matching");
    // any one blank among real ones in Any mode does not widen the rule
    let any = RuleSet { conditions: vec![RuleCondition::SubjectContains { words: "nothing like this".into() }, RuleCondition::SubjectContains { words: "".into() }], match_mode: MatchMode::Any };
    assert!(!rule_set_matches(any, msg(1)));
}

#[test]
fn saving_a_rule_with_no_value_is_refused_in_the_same_words_everywhere() {
    for blank in ["", " "] {
        for c in kinds(blank) {
            assert_eq!(condition_refusal_words(c.clone()).as_deref(), Some("This rule has no value."), "{c:?}");
            let chain = validate_chain(one(c.clone()));
            assert!(matches!(&chain, Err(CoreError::Invalid { field, reason_words }) if field == CHAIN_REFUSAL_CODE && reason_words == "This rule has no value."), "{chain:?}");
            let reply = validate_reply_rule(one(c.clone()));
            assert!(matches!(&reply, Err(CoreError::Invalid { field, reason_words }) if field == REPLY_BAD_RULE_CODE && reason_words == "This rule has no value."), "{reply:?}");
        }
    }
    assert_eq!(condition_refusal_words(RuleCondition::Domain { domain: "a@b.com".into() }).as_deref(), Some("a domain looks like example.com"), "a domain written as an address keeps its own words");
    assert_eq!(condition_refusal_words(RuleCondition::HasAttachment), None);
    assert_eq!(condition_refusal_words(RuleCondition::Fact { fact: MessageFact::Unread, wanted: true }), None);
    assert_eq!(condition_refusal_words(RuleCondition::SubjectContains { words: "x".into() }), None);
    // a rule already stored with a blank value is not an error to READ: it just matches nothing
    assert!(!rule_set_matches(one(RuleCondition::SubjectContains { words: "".into() }), msg(1)));
}

#[test]
fn a_rerun_over_38_messages_with_one_blank_rule_joins_none() {
    let corpus: Vec<MessageFacts> = (0..38).map(msg).collect();
    for c in kinds("") {
        let plan = plan_rerun(one(c.clone()), false, corpus.clone(), vec![]);
        assert!(plan.file.is_empty(), "0 join for {c:?}, not 38");
    }
    let ok = plan_rerun(one(RuleCondition::SubjectContains { words: "quote".into() }), false, corpus, vec![]);
    assert_eq!(ok.file.len(), 38, "a real rule still files the 38");
}
