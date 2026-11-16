//! Round 18: the negated condition as the store keeps it (`rule.conditions_json`). A saved rule written before this round (no `not` kind) still loads unchanged.
//! MUTANT N4 (seen red): label_meta.rs `encode_conditions` writes the positive kind for a `Not` -> a_negated_condition_survives_the_store.
use contracts::{RuleCondition, RuleField};
use rusqlite::Connection;
use store::{decode_conditions, encode_conditions};

#[test]
fn a_negated_condition_survives_the_store() {
    let c = Connection::open_in_memory().unwrap();
    let all = vec![
        RuleCondition::Not { field: RuleField::Domain, value: "example.com".into() },
        RuleCondition::Not { field: RuleField::SubjectContains, value: "invoice \"Q4\"".into() },
        RuleCondition::Not { field: RuleField::ListHeader, value: "news".into() },
        RuleCondition::Not { field: RuleField::Account, value: "acc-1".into() },
        RuleCondition::From { words: "laura".into() },
    ];
    let json = encode_conditions(&all);
    assert!(json.contains("\"kind\":\"not\"") && json.contains("\"field\":\"domain\""), "{json}");
    assert_eq!(decode_conditions(&c, &json).unwrap(), all);
}

#[test]
fn old_rules_still_load() {
    let c = Connection::open_in_memory().unwrap();
    let old = r#"[{"kind":"domain","domain":"x.test"},{"kind":"from","words":"laura"},{"kind":"fact","fact":"unread","wanted":true},{"kind":"has_attachment"}]"#;
    let got = decode_conditions(&c, old).unwrap();
    assert_eq!(got.len(), 4);
    assert_eq!(got[0], RuleCondition::Domain { domain: "x.test".into() });
    assert!(decode_conditions(&c, r#"[{"kind":"not","field":"nope","value":"x"}]"#).is_err(), "an unknown negated field is refused in words");
}
