//! The header rules, each on a header that trips it and a header that does not. Headers are written the way a
//! server hands them over (folded lines included); every name and address is invented.
use junk::{
    auth_verdicts, display_name_impersonation, fired_rules, link_mismatch, parse_address, registrable_domain, reply_to_mismatch, Contact, Context, Message,
    RuleId,
};

fn parse(lines: &[&str], html: Option<&str>) -> Message {
    Message::from_header_block(&lines.join("\r\n"), "plain text body", html)
}

fn rules_of(m: &Message) -> Vec<RuleId> {
    fired_rules(m, &Context::default())
}

fn anna_context() -> Context {
    Context { contacts: vec![Contact { name: "Anna Smith".into(), emails: vec!["anna@work.example".into()] }], hard_rules: Vec::new() }
}

const FROM_SHOP: &str = "From: Shop <news@shop.example>";

#[test]
fn spf_fail_and_softfail_trip_and_pass_does_not() {
    let fail = parse(&[FROM_SHOP, "Authentication-Results: mx.example.net; spf=fail smtp.mailfrom=shop.example; dkim=pass header.d=shop.example; dmarc=pass header.from=shop.example"], None);
    assert_eq!(rules_of(&fail), vec![RuleId::SpfFail]);
    let soft = parse(&[FROM_SHOP, "Authentication-Results: mx.example.net; spf=softfail smtp.mailfrom=shop.example"], None);
    assert_eq!(rules_of(&soft), vec![RuleId::SpfSoftfail]);
    let pass = parse(&[FROM_SHOP, "Authentication-Results: mx.example.net; spf=pass smtp.mailfrom=shop.example; dkim=pass header.d=shop.example; dmarc=pass header.from=shop.example"], None);
    assert_eq!(rules_of(&pass), Vec::<RuleId>::new());
    for quiet in ["spf=none", "spf=neutral", "spf=temperror", "spf=permerror"] {
        let line = format!("Authentication-Results: mx.example.net; {quiet} smtp.mailfrom=shop.example");
        let m = parse(&[FROM_SHOP, line.as_str()], None);
        assert_eq!(rules_of(&m), Vec::<RuleId>::new(), "{quiet}");
    }
}

#[test]
fn a_message_with_no_authentication_results_is_never_penalised() {
    let m = parse(&[FROM_SHOP, "Subject: Hello"], None);
    assert!(m.authentication_results.is_empty());
    assert_eq!(rules_of(&m), Vec::<RuleId>::new());
}

#[test]
fn dkim_fails_only_when_no_signature_passes() {
    let fail = parse(&[FROM_SHOP, "Authentication-Results: mx.example.net; dkim=fail (bad signature) header.d=shop.example"], None);
    assert_eq!(rules_of(&fail), vec![RuleId::DkimFail]);
    let pass = parse(&[FROM_SHOP, "Authentication-Results: mx.example.net; dkim=pass header.d=shop.example"], None);
    assert_eq!(rules_of(&pass), Vec::<RuleId>::new());
    let two = parse(&[FROM_SHOP, "Authentication-Results: mx.example.net; dkim=fail header.d=old.example; dkim=pass header.d=shop.example"], None);
    assert_eq!(rules_of(&two), Vec::<RuleId>::new(), "one passing signature is enough");
    let two_reversed = parse(&[FROM_SHOP, "Authentication-Results: mx.example.net; dkim=pass header.d=shop.example; dkim=fail header.d=old.example"], None);
    assert_eq!(rules_of(&two_reversed), Vec::<RuleId>::new());
}

#[test]
fn dmarc_fail_trips_and_pass_does_not() {
    let fail = parse(&[FROM_SHOP, "Authentication-Results: mx.example.net; dmarc=fail (p=reject dis=none) header.from=shop.example"], None);
    assert_eq!(rules_of(&fail), vec![RuleId::DmarcFail]);
    let pass = parse(&[FROM_SHOP, "Authentication-Results: mx.example.net; dmarc=pass (p=none) header.from=shop.example"], None);
    assert_eq!(rules_of(&pass), Vec::<RuleId>::new());
}

#[test]
fn only_the_topmost_authentication_results_is_believed() {
    let forged_below = parse(
        &[
            FROM_SHOP,
            "Authentication-Results: mx.example.net; spf=fail smtp.mailfrom=shop.example",
            "Authentication-Results: spoof.example; spf=pass smtp.mailfrom=shop.example",
        ],
        None,
    );
    assert_eq!(forged_below.authentication_results.len(), 2);
    assert_eq!(rules_of(&forged_below), vec![RuleId::SpfFail], "the receiving server's header is the first one");
    let fail_below = parse(
        &[
            FROM_SHOP,
            "Authentication-Results: mx.example.net; spf=pass smtp.mailfrom=shop.example",
            "Authentication-Results: upstream.example; spf=fail smtp.mailfrom=shop.example",
        ],
        None,
    );
    assert_eq!(rules_of(&fail_below), Vec::<RuleId>::new());
}

#[test]
fn a_folded_authentication_results_header_is_read_whole() {
    let m = parse(
        &[FROM_SHOP, "Authentication-Results: mx.example.net;", " spf=fail smtp.mailfrom=shop.example;", " dkim=pass header.d=shop.example"],
        None,
    );
    let v = auth_verdicts(&m.authentication_results);
    assert_eq!(v.spf.as_deref(), Some("fail"));
    assert_eq!(v.dkim.as_deref(), Some("pass"));
    assert_eq!(v.dmarc, None);
}

#[test]
fn reply_to_on_another_site_trips_and_the_same_site_does_not() {
    let other = parse(&[FROM_SHOP, "Reply-To: <collect@elsewhere.example>"], None);
    assert!(reply_to_mismatch(&other));
    assert_eq!(rules_of(&other), vec![RuleId::ReplyToMismatch]);
    let same_site = parse(&[FROM_SHOP, "Reply-To: Support <support@mail.shop.example>"], None);
    assert!(!reply_to_mismatch(&same_site));
    let none = parse(&[FROM_SHOP], None);
    assert!(!reply_to_mismatch(&none));
}

#[test]
fn a_display_name_that_carries_another_address_trips() {
    let fake = parse(&["From: \"service@paypal.com\" <alert@paypa1-secure.example>"], None);
    assert!(display_name_impersonation(&fake, &[]));
    assert_eq!(rules_of(&fake), vec![RuleId::DisplayNameImpersonation]);
    let genuine = parse(&["From: \"service@paypal.com\" <service@paypal.com>"], None);
    assert!(!display_name_impersonation(&genuine, &[]));
    let plain = parse(&["From: \"Anna\" <anna@x.example>"], None);
    assert!(!display_name_impersonation(&plain, &[]));
}

#[test]
fn a_known_contacts_name_on_a_stranger_address_trips() {
    let ctx = anna_context();
    let fake = parse(&["From: \"Anna Smith\" <anna.smith@cheap-meds.example>"], None);
    assert!(display_name_impersonation(&fake, &ctx.contacts));
    assert_eq!(fired_rules(&fake, &ctx), vec![RuleId::DisplayNameImpersonation]);
    let genuine = parse(&["From: \"Anna Smith\" <anna@work.example>"], None);
    assert!(!display_name_impersonation(&genuine, &ctx.contacts), "her own address");
    let same_site = parse(&["From: Anna Smith <anna.s@mail.work.example>"], None);
    assert!(!display_name_impersonation(&same_site, &ctx.contacts), "another mailbox on her own site");
    let stranger = parse(&["From: \"Bob Jones\" <bob@other.example>"], None);
    assert!(!display_name_impersonation(&stranger, &ctx.contacts), "a name nobody knows");
    assert!(!display_name_impersonation(&fake, &[]), "no contacts, nothing to impersonate");
}

#[test]
fn a_link_that_shows_one_site_and_goes_to_another_trips() {
    let phish = parse(&[FROM_SHOP], Some("<p>Sign in: <a href=\"http://paypa1-secure.example/login\">www.paypal.com</a></p>"));
    assert!(link_mismatch(&phish));
    assert_eq!(rules_of(&phish), vec![RuleId::LinkMismatch]);
    let user_info = parse(&[FROM_SHOP], Some("<a href=\"http://www.paypal.com@evil.example/x\">https://www.paypal.com/account</a>"));
    assert!(link_mismatch(&user_info), "the host is what follows the @");
    let nested = parse(&[FROM_SHOP], Some("<a class=\"btn\" href='http://evil.example/x'><b>www.paypal.com</b></a>"));
    assert!(link_mismatch(&nested), "single quotes and inner tags");
}

#[test]
fn honest_links_do_not_trip() {
    let tracked = parse(&[FROM_SHOP], Some("<a href=\"https://click.shop.example/t?u=1&amp;v=2\">www.shop.example</a>"));
    assert!(!link_mismatch(&tracked), "a tracking host on the same site");
    let words = parse(&[FROM_SHOP], Some("<a href=\"http://tracking.other.example/x\">Click here</a>"));
    assert!(!link_mismatch(&words), "link text that is not an address says nothing");
    let mail = parse(&[FROM_SHOP], Some("<a href=\"mailto:a@b.example\">b.example</a>"));
    assert!(!link_mismatch(&mail), "a mailto link has no host");
    let no_html = parse(&[FROM_SHOP], None);
    assert!(!link_mismatch(&no_html));
}

#[test]
fn registrable_domains_and_addresses() {
    assert_eq!(registrable_domain("mail.news.example.com"), "example.com");
    assert_eq!(registrable_domain("shop.example.co.uk"), "example.co.uk");
    assert_eq!(registrable_domain("bbc.co.uk"), "bbc.co.uk");
    assert_eq!(registrable_domain("example.com"), "example.com");
    assert_eq!(registrable_domain("localhost"), "localhost");
    assert_eq!(parse_address("\"Ann Lee\" <Ann@X.Example>"), (Some("Ann Lee".to_string()), "ann@x.example".to_string()));
    assert_eq!(parse_address("Ann Lee <ann@x.example>, Bob <bob@y.example>"), (Some("Ann Lee".to_string()), "ann@x.example".to_string()));
    assert_eq!(parse_address("<ann@x.example>"), (None, "ann@x.example".to_string()));
    assert_eq!(parse_address("ann@x.example"), (None, "ann@x.example".to_string()));
}
