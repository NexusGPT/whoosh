//! Round 18 (owner, 13 Nov 07:30: "for the rule we shoudl have the actual rule (e.g., "Domain") and condition (is/is not) separately"): the negated twin of each text or
//! identity condition. `RuleCondition::Not { field, value }` holds exactly when its positive twin does not, and a blank value is refused like the positive form.
//! MUTANTS (each must turn the test named red):
//!   N1 label_tabs.rs `condition_matches`: the `Not` arm forgets the `!`                    -> every_negated_twin_is_the_opposite_of_its_positive_one, a_negated_rule_files_what_the_positive_one_leaves
//!   N2 label_tabs.rs `condition_is_blank`: the `Not` arm answers `false`                    -> a_negated_condition_with_a_blank_value_is_refused_like_the_positive_one
//!   N3 records.rs `positive_twin`: Domain maps to From                                      -> every_negated_twin_is_the_opposite_of_its_positive_one
use contracts::*;

fn msg() -> MessageFacts {
    MessageFacts { message_id: "m1".into(), account_id: "acc-1".into(), from_name: "Laura Meier".into(), from_address: "laura@nordlicht.example".into(), to: vec!["me@x.test".into()],
        cc: vec!["cc@x.test".into()], subject: "Quote for Q4".into(), body: "hello world".into(), has_attachment: false, list_header: "<news.example.com>".into(), unread: false,
        received_at_ms: 1, unanswered: false, starred: false, vip_sender: false, cc_me: false, replied_by_me: false }
}
fn one(c: RuleCondition) -> RuleSet { RuleSet { conditions: vec![c], match_mode: MatchMode::All } }

/// (field, a value the message HAS, a value it does NOT have)
fn cases() -> Vec<(RuleField, &'static str, &'static str)> {
    vec![(RuleField::From, "laura", "bob"), (RuleField::To, "me@", "you@"), (RuleField::Cc, "cc@", "bcc@"), (RuleField::Domain, "nordlicht.example", "other.example"),
         (RuleField::SubjectContains, "quote", "invoice"), (RuleField::BodyContains, "world", "moon"), (RuleField::ListHeader, "news", "digest"), (RuleField::Account, "acc-1", "acc-2")]
}

#[test]
fn every_negated_twin_is_the_opposite_of_its_positive_one() {
    for (field, has, hasnt) in cases() {
        let pos_has = RuleCondition::positive_twin(field, has);
        let pos_hasnt = RuleCondition::positive_twin(field, hasnt);
        assert!(rule_set_matches(one(pos_has), msg()), "{field:?}: the positive form finds {has:?}");
        assert!(!rule_set_matches(one(pos_hasnt), msg()), "{field:?}: the positive form does not find {hasnt:?}");
        assert!(!rule_set_matches(one(RuleCondition::Not { field, value: has.into() }), msg()), "{field:?} is not {has:?} must NOT hold for a message that has it");
        assert!(rule_set_matches(one(RuleCondition::Not { field, value: hasnt.into() }), msg()), "{field:?} is not {hasnt:?} must hold for a message that lacks it");
    }
}

#[test]
fn a_negated_condition_with_a_blank_value_is_refused_like_the_positive_one() {
    for (field, _, _) in cases() {
        for blank in ["", "  "] {
            let pos = condition_refusal_words(RuleCondition::positive_twin(field, blank));
            let neg = condition_refusal_words(RuleCondition::Not { field, value: blank.into() });
            assert!(pos.is_some(), "{field:?}: the positive form refuses {blank:?}");
            assert_eq!(pos, neg, "{field:?}: the negated form refuses {blank:?} in the same words");
        }
        assert!(condition_refusal_words(RuleCondition::Not { field, value: "x".into() }).is_none() || field == RuleField::Domain, "{field:?}: a real value is accepted");
    }
    // a domain that is an address is refused, negated or not
    assert!(condition_refusal_words(RuleCondition::Not { field: RuleField::Domain, value: "a@b.c".into() }).is_some());
}

#[test]
fn a_negated_rule_files_what_the_positive_one_leaves() {
    let from_laura = RuleCondition::From { words: "laura".into() };
    let not_laura = RuleCondition::Not { field: RuleField::From, value: "laura".into() };
    let bob = MessageFacts { from_name: "Bob".into(), from_address: "bob@x.test".into(), ..msg() };
    assert!(rule_set_matches(one(from_laura.clone()), msg()) && !rule_set_matches(one(not_laura.clone()), msg()));
    assert!(!rule_set_matches(one(from_laura.clone()), bob.clone()) && rule_set_matches(one(not_laura.clone()), bob.clone()));
    // combined with a positive condition in All mode: "from nordlicht.example but not laura"
    let both = RuleSet { conditions: vec![RuleCondition::Domain { domain: "nordlicht.example".into() }, not_laura], match_mode: MatchMode::All };
    let other_nordlicht = MessageFacts { from_name: "Jan".into(), from_address: "jan@nordlicht.example".into(), ..msg() };
    assert!(!rule_set_matches(both.clone(), msg()) && rule_set_matches(both.clone(), other_nordlicht) && !rule_set_matches(both, bob));
}
