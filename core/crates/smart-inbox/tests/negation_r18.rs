//! Round 18: the label filing's own matcher (`user_condition`, which the decision lane runs on an arrival and a re-run) negates each twin too.
//! MUTANT N6: cond.rs `user_condition`'s `Not` arm forgets the `!` -> every_negated_twin_files_the_opposite.
use contracts::{RuleCondition, RuleField};
use smart_inbox::cond::user_condition;
use smart_inbox::facts::MessageFacts;

fn facts() -> MessageFacts {
    MessageFacts { account_id: "acc-1".into(), from_address: "laura@nordlicht.example".into(), to: vec!["me@x.test".into()], cc: vec!["cc@x.test".into()], subject: "Quote for Q4".into(),
        body: "hello world".into(), list_id: Some("<news.example.com>".into()), has_list_header: true, ..Default::default() }
}

#[test]
fn every_negated_twin_files_the_opposite() {
    for (field, has, hasnt) in [(RuleField::From, "laura", "bob"), (RuleField::To, "me@", "you@"), (RuleField::Cc, "cc@", "bcc@"), (RuleField::Domain, "nordlicht.example", "other.example"),
        (RuleField::SubjectContains, "quote", "invoice"), (RuleField::BodyContains, "world", "moon"), (RuleField::ListHeader, "news", "digest"), (RuleField::Account, "acc-1", "acc-2")] {
        let f = facts();
        assert!(user_condition(&RuleCondition::positive_twin(field, has), &f), "{field:?} positive finds {has:?}");
        assert!(!user_condition(&RuleCondition::Not { field, value: has.into() }, &f), "{field:?} is not {has:?} must not hold");
        assert!(!user_condition(&RuleCondition::positive_twin(field, hasnt), &f));
        assert!(user_condition(&RuleCondition::Not { field, value: hasnt.into() }, &f), "{field:?} is not {hasnt:?} must hold");
    }
}
