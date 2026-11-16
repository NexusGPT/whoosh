//! The trained table as bytes the store keeps: exact round trip, stable bytes, damage refused, the size cap.
mod support;

use junk::tuning::*;
use junk::{tokenise, Counts, Model, ModelError};

fn learn_all(reversed: bool) -> Model {
    let mut spam = support::spam();
    let mut ham = support::ham();
    if reversed {
        spam.reverse();
        ham.reverse();
    }
    let mut m = Model::new();
    for s in &spam {
        m.train(&tokenise(s), true);
    }
    for h in &ham {
        m.train(&tokenise(h), false);
    }
    m
}

fn header(n_spam: u32, n_ham: u32, count: u32) -> Vec<u8> {
    let mut b = b"WJB1".to_vec();
    b.extend_from_slice(&n_spam.to_le_bytes());
    b.extend_from_slice(&n_ham.to_le_bytes());
    b.extend_from_slice(&count.to_le_bytes());
    b
}

#[test]
fn the_table_survives_a_round_trip_through_bytes() {
    let m = learn_all(false);
    let bytes = m.to_bytes();
    let back = Model::from_bytes(&bytes).expect("what to_bytes wrote reads back");
    assert_eq!(back, m);
    for probe in [support::spam_probe(), support::ham_probe(), support::unknown_probe()] {
        let t = tokenise(&probe);
        assert_eq!(back.score(&t), m.score(&t));
    }
    assert_eq!(back.counts("free"), m.counts("free"));
    assert_eq!(Model::from_bytes(&Model::new().to_bytes()), Ok(Model::new()));
}

#[test]
fn the_bytes_do_not_depend_on_the_order_things_were_learned() {
    assert_eq!(learn_all(false).to_bytes(), learn_all(true).to_bytes());
}

#[test]
fn every_token_keeps_its_own_counts_through_the_bytes() {
    let mut m = Model::new();
    m.train(&["alpha".to_string(), "beta".to_string()], true);
    m.train(&["alpha".to_string()], true);
    m.train(&["beta".to_string(), "gamma".to_string()], false);
    let back = Model::from_bytes(&m.to_bytes()).expect("reads back");
    assert_eq!(back.counts("alpha"), Some(Counts { spam: 2, ham: 0 }));
    assert_eq!(back.counts("beta"), Some(Counts { spam: 1, ham: 1 }));
    assert_eq!(back.counts("gamma"), Some(Counts { spam: 0, ham: 1 }));
    assert_eq!(back.trained(), (2, 1));
}

#[test]
fn damaged_bytes_are_refused_not_guessed() {
    let bytes = learn_all(false).to_bytes();
    for cut in [0, 1, 3, 4, 11, 12, 13, 40, bytes.len() / 2, bytes.len() - 1] {
        assert!(Model::from_bytes(&bytes[..cut]).is_err(), "cut at {cut} of {}", bytes.len());
    }
    let mut wrong_magic = bytes.clone();
    wrong_magic[0] ^= 0xff;
    assert_eq!(Model::from_bytes(&wrong_magic), Err(ModelError::BadMagic));
    let mut trailing = bytes.clone();
    trailing.push(0);
    assert_eq!(Model::from_bytes(&trailing), Err(ModelError::TrailingBytes));
    assert_eq!(Model::from_bytes(&header(0, 0, u32::MAX)), Err(ModelError::TooLarge));
    let mut not_text = header(0, 0, 1);
    not_text.extend_from_slice(&[1, 0xff, 0, 0]); // length 1, the byte 0xff, counts 0 and 0
    assert_eq!(Model::from_bytes(&not_text), Err(ModelError::BadToken));
    let mut too_long = header(0, 0, 1);
    too_long.extend_from_slice(&[0xff, 0xff, 0xff, 0xff, 0x7f]); // a length past u32
    assert!(Model::from_bytes(&too_long).is_err());
}

fn filled_past_the_cap() -> Model {
    let mut m = Model::new();
    let mut n = 0usize;
    for i in 0..((TOKEN_TABLE_MAX + 5_000) / 100) {
        let mut tokens: Vec<String> = vec!["keepme".to_string()];
        for _ in 0..100 {
            tokens.push(format!("t{n}"));
            n += 1;
        }
        m.train(&tokens, i % 2 == 0);
    }
    m
}

#[test]
fn the_table_is_capped_and_pruning_drops_the_rare_tokens_first() {
    let m = filled_past_the_cap();
    assert!(m.len() <= TOKEN_TABLE_MAX, "{} tokens held, the cap is {TOKEN_TABLE_MAX}", m.len());
    assert!(m.len() >= TOKEN_TABLE_PRUNE_TO, "a prune stops at its target, it does not empty the table: {}", m.len());
    assert_eq!(m.counts("keepme"), Some(Counts { spam: 325, ham: 325 }), "the token in every message keeps its whole history");
    assert_eq!(m.trained(), (325, 325), "pruning tokens never changes the message counts");
}

#[test]
fn the_capped_table_stays_small_on_disk() {
    let m = filled_past_the_cap();
    let bytes = m.to_bytes();
    assert!(bytes.len() < 2_000_000, "{} bytes for {} tokens", bytes.len(), m.len());
    assert_eq!(Model::from_bytes(&bytes), Ok(m));
}
