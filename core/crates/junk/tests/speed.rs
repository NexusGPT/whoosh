//! Scoring speed. The bound is stated in the assert: the best of five batches of 100 scores must average under
//! SCORE_BOUND_US microseconds per message in the test profile (unoptimised), on a table of about 20,000 tokens and a
//! message of about 2 KB of text plus the same text as HTML with a link. Best-of-five keeps a busy machine from
//! failing a fast build; a slow build fails every batch.
mod support;

use std::hint::black_box;
use std::time::Instant;

use junk::tuning::MAX_TOKENS_PER_MESSAGE;
use junk::{tokenise, Context, Filter, Message};

const SCORE_BOUND_US: u128 = 2_000;
const HUGE_MESSAGE_BOUND_MS: u128 = 250;

fn big_filter() -> Filter {
    let mut f = Filter::default();
    f.bootstrap(&support::spam(), &support::ham());
    let mut n = 0usize;
    for i in 0..200 {
        let tokens: Vec<String> = (0..100)
            .map(|_| {
                n += 1;
                format!("syn{n}")
            })
            .collect();
        f.model.train(&tokens, i % 2 == 0);
    }
    f
}

fn typical_message() -> Message {
    let mut text = String::new();
    for m in support::spam().iter().take(10) {
        text.push_str(&m.text);
        text.push(' ');
    }
    for m in support::ham().iter().take(10) {
        text.push_str(&m.text);
        text.push(' ');
    }
    let mut m = support::msg(Some("Shop"), "news@shop.example", "Weekly offer", &text);
    m.html = Some(format!("<p>{text}</p><a href=\"http://click.shop.example/x\">www.shop.example</a>"));
    m.authentication_results = vec!["mx.example.net; spf=pass; dkim=pass; dmarc=pass".to_string()];
    m
}

#[test]
fn scoring_one_typical_message_takes_microseconds() {
    let f = big_filter();
    let m = typical_message();
    let ctx = Context::default();
    assert!(m.text.len() > 1_500, "the message is about 2 KB: {}", m.text.len());
    assert!(f.model.len() > 20_000, "the table holds about 20,000 tokens: {}", f.model.len());
    let mut best = u128::MAX;
    for _ in 0..5 {
        let started = Instant::now();
        for _ in 0..100 {
            black_box(f.assess(black_box(&m), black_box(&ctx)));
        }
        let per_message = started.elapsed().as_micros() / 100;
        if per_message < best {
            best = per_message;
        }
    }
    println!("best batch: {best} microseconds per message (bound {SCORE_BOUND_US})");
    assert!(best < SCORE_BOUND_US, "{best} microseconds per message, the bound is {SCORE_BOUND_US}");
}

#[test]
fn a_huge_message_costs_no_more_than_a_capped_one() {
    let f = big_filter();
    let mut m = support::msg(None, "a@b.example", "Hello", &"free cash ".repeat(500_000));
    m.html = Some("<a href=\"http://x.example\">x</a>".repeat(100_000));
    let started = Instant::now();
    let v = f.assess(&m, &Context::default());
    let took = started.elapsed().as_millis();
    assert!(took < HUGE_MESSAGE_BOUND_MS, "{took} ms for a 5 MB body, the bound is {HUGE_MESSAGE_BOUND_MS}");
    assert!((0.0..=1.0).contains(&v.score));
    assert!(tokenise(&m).len() <= MAX_TOKENS_PER_MESSAGE);
}
