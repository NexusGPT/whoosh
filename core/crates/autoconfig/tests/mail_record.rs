//! The mail-record step (SPEC A76) and the provider and sign-in fields, against fakes. Each test is red on a mutant in
//! core/crates/autoconfig/mutants.sh.
mod support;
use autoconfig::*;
use support::*;

fn run(dns: &FakeDns, probe: &FakeProbe, email: &str) -> (DetectionResult, FakeHttp) {
    let http = FakeHttp::default();
    let r = Detector::new(&http, dns, probe).detect(email);
    (r, http)
}

#[test]
fn gpt_nexus_mx_names_microsoft_365_after_autodiscover_and_the_guess_never_probes() {
    // nexus-example.com publishes no ISPDB entry and no autoconfig host; its mail record points to Microsoft 365 (docs/PRD.md 08:38).
    // The exchanger name is a stand-in of the Microsoft form, not a recorded answer; the mailbox is a stand-in on the domain.
    let dns = FakeDns::default().with_mx("nexus-example.com", &[(0, "nexus-example-com.mail.protection.outlook.com.")]);
    let probe = FakeProbe::default();
    let (r, http) = run(&dns, &probe, "jane@nexus-example.com");
    let c = found(&r);
    assert_eq!((c.imap_host.as_str(), c.imap_port, c.imap_security), ("outlook.office365.com", 993, Security::Ssl));
    assert_eq!((c.smtp_host.as_str(), c.smtp_port, c.smtp_security), ("smtp.office365.com", 587, Security::Starttls));
    assert_eq!((c.username.as_str(), c.auth_method, c.source), ("jane@nexus-example.com", AuthMethod::Oauth2, Source::MailRecord));
    assert_eq!((r.provider, r.sign_in_needed, r.microsoft_365_tenant), (Some(Provider::Microsoft365), true, true));
    assert_eq!(r.steps_tried, vec![Step::Ispdb, Step::AutoconfigUrls, Step::Srv, Step::Autodiscover, Step::MailRecord]);
    assert!(http.called("https://autodiscover"), "Autodiscover runs before the mail record");
    assert_eq!(probe.count(), 0, "the guess ran after the mail record answered");
}

#[test]
fn every_provider_row_fills_its_servers_from_its_mail_record() {
    // (exchanger, imap host, imap port, imap security, smtp host, smtp port, smtp security, auth, provider)
    let cases = [
        ("aspmx.l.google.com.", "imap.gmail.com", 993, Security::Ssl, "smtp.gmail.com", 587, Security::Starttls, AuthMethod::Password, Provider::Google),
        ("smtp.google.com", "imap.gmail.com", 993, Security::Ssl, "smtp.gmail.com", 587, Security::Starttls, AuthMethod::Password, Provider::Google),
        ("alt2.aspmx.l.googlemail.com.", "imap.gmail.com", 993, Security::Ssl, "smtp.gmail.com", 587, Security::Starttls, AuthMethod::Password, Provider::Google),
        ("mx01.mail.icloud.com.", "imap.mail.me.com", 993, Security::Ssl, "smtp.mail.me.com", 587, Security::Starttls, AuthMethod::Password, Provider::Icloud),
        ("in1-smtp.messagingengine.com.", "imap.fastmail.com", 993, Security::Ssl, "smtp.fastmail.com", 587, Security::Starttls, AuthMethod::Password, Provider::Fastmail),
        ("mail.protonmail.ch.", "127.0.0.1", 1143, Security::Starttls, "127.0.0.1", 1025, Security::Starttls, AuthMethod::Password, Provider::ProtonBridge),
    ];
    for (mx, ih, ip, is, sh, sp, ss, auth, provider) in cases {
        let dns = FakeDns::default().with_mx("own.example", &[(10, mx)]);
        let (r, _) = run(&dns, &FakeProbe::default(), "jane@own.example");
        let c = found(&r);
        assert_eq!((c.imap_host.as_str(), c.imap_port, c.imap_security), (ih, ip, is), "{mx}");
        assert_eq!((c.smtp_host.as_str(), c.smtp_port, c.smtp_security), (sh, sp, ss), "{mx}");
        assert_eq!((c.auth_method, c.source, r.provider, r.sign_in_needed), (auth, Source::MailRecord, Some(provider), false), "{mx}");
    }
}

#[test]
fn exchangers_are_read_in_preference_order_and_a_lookalike_host_is_not_a_provider() {
    let dns = FakeDns::default().with_mx("mixed.example", &[(20, "mx.filter.example."), (5, "evilgoogle.com."), (10, "mx01.mail.icloud.com.")]);
    let (r, _) = run(&dns, &FakeProbe::default(), "jane@mixed.example");
    assert_eq!(r.provider, Some(Provider::Icloud), "evilgoogle.com is not google.com; mx01.mail.icloud.com is next by preference");
    let dns = FakeDns::default().with_mx("order.example", &[(20, "mx01.mail.icloud.com."), (10, "in1-smtp.messagingengine.com.")]);
    let (r, _) = run(&dns, &FakeProbe::default(), "jane@order.example");
    assert_eq!(r.provider, Some(Provider::Fastmail), "the lower preference wins");
}

#[test]
fn a_plain_domain_mx_matches_no_provider_and_the_probed_guess_answers() {
    let dns = FakeDns::default().with_mx("plain.example", &[(10, "mx.plain.example.")]);
    let probe = FakeProbe { answering: vec![("imap.plain.example".into(), 993, Security::Ssl), ("smtp.plain.example".into(), 465, Security::Ssl)], ..FakeProbe::default() };
    let (r, _) = run(&dns, &probe, "jane@plain.example");
    let c = found(&r);
    assert_eq!((c.imap_host.as_str(), c.smtp_host.as_str(), c.source), ("imap.plain.example", "smtp.plain.example", Source::Guess));
    assert_eq!((r.provider, r.sign_in_needed), (None, false));
    assert_eq!(r.steps_tried, STEP_ORDER.to_vec());
    assert!(dns.calls.lock().expect("lock").contains(&DnsQuery::Mx("plain.example".into())), "the mail record was read before the guess");
}

#[test]
fn a_provider_found_by_the_provider_database_is_named_too() {
    let http = FakeHttp::default().ok("https://autoconfig.thunderbird.net/v1.1/gmail.com", fixture("ispdb-gmail.com.xml"));
    let r = Detector::new(&http, &FakeDns::default(), &FakeProbe::default()).detect("jane@gmail.com");
    assert_eq!((r.provider, r.sign_in_needed, found(&r).source), (Some(Provider::Google), false, Source::Ispdb));
}
