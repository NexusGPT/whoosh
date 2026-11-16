//! Delta 17 in the smart inbox: the stored fact kind parses (and a bad one is dropped, never stops placement), the Fact condition
//! is judged on this crate's facts with the value wanted, each of the eight facts reads its own field, tab rules are entry chain 0.
//! Mutants (single edits, unobserved: uncompiled round):
//! - I1 cond.rs `fact_holds(*fact, f) == *wanted` -> `fact_holds(*fact, f)` -> a_fact_condition_wants_its_value
//! - I2 cond.rs `MessageFact::CcMe => f.cc_me` -> `f.from_vip` -> each_fact_reads_its_own_field
//! - I3 load.rs `"fact" =>` arm removed -> the_fact_kind_parses_and_a_bad_one_is_dropped
use contracts::{MessageFact, RuleCondition};
use smart_inbox::cond::{fact_holds, user_condition};
use smart_inbox::load::conditions_from_json;
use smart_inbox::MessageFacts;

#[test]
fn the_fact_kind_parses_and_a_bad_one_is_dropped() {
    let got = conditions_from_json(r#"[{"kind":"fact","fact":"replied_by_me","wanted":false},{"kind":"fact","fact":"sparkly","wanted":true},{"kind":"fact","fact":"unread"},{"kind":"subject","words":"hi"}]"#);
    assert_eq!(got, vec![RuleCondition::Fact { fact: MessageFact::RepliedByMe, wanted: false }, RuleCondition::SubjectContains { words: "hi".into() }]);
}

#[test]
fn a_fact_condition_wants_its_value() {
    let f = MessageFacts { starred: true, ..Default::default() };
    assert!(user_condition(&RuleCondition::Fact { fact: MessageFact::Starred, wanted: true }, &f));
    assert!(!user_condition(&RuleCondition::Fact { fact: MessageFact::Starred, wanted: false }, &f));
    assert!(user_condition(&RuleCondition::Fact { fact: MessageFact::Unanswered, wanted: false }, &f));
}

#[test]
fn each_fact_reads_its_own_field() {
    let set = |fact: MessageFact| -> MessageFacts {
        let mut f = MessageFacts::default();
        match fact {
            MessageFact::Unread => f.unread = true,
            MessageFact::Unanswered => f.unanswered = true,
            MessageFact::HasAttachment => f.has_attachment = true,
            MessageFact::Starred => f.starred = true,
            MessageFact::VipSender => f.from_vip = true,
            MessageFact::CcMe => f.cc_me = true,
            MessageFact::MailingList => f.has_list_header = true,
            MessageFact::RepliedByMe => f.replied_by_me = true,
        }
        f
    };
    for on in contracts::message_facts_in_grid_order() {
        for asked in contracts::message_facts_in_grid_order() {
            assert_eq!(fact_holds(asked, &set(on)), asked == on, "{asked:?} on a message where only {on:?} is true");
        }
    }
}
