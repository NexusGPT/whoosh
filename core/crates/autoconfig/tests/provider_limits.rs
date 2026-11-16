//! Provider-limits lane, item 3: Yahoo as a known provider (the mail-record table) and the bare domain in the IMAP host guess.
//! Fakes only. Mutants are named in the comments and listed in mutants.sh.
mod support;
use autoconfig::*;
use support::*;

fn run(dns: &FakeDns, probe: &FakeProbe, email: &str) -> DetectionResult {
    Detector::new(&FakeHttp::default(), dns, probe).detect(email)
}

#[test]
fn a_domain_whose_mail_record_is_yahoos_fills_the_yahoo_servers_and_never_probes() {
    // mutants: yahoo-suffix-wrong (mx_suffixes -> an unused name: the row never matches), yahoo-smtp-port-587 (smtp_port SMTPS_PORT -> SUBMISSION_PORT),
    //          yahoo-imap-host-wrong (imap_host edited)
    let dns = FakeDns::default().with_mx("own.example", &[(10, "mta5.am0.yahoodns.net.")]);
    let probe = FakeProbe::default();
    let r = run(&dns, &probe, "jane@own.example");
    let c = found(&r);
    assert_eq!((c.imap_host.as_str(), c.imap_port, c.imap_security), ("imap.mail.yahoo.com", 993, Security::Ssl));
    assert_eq!((c.smtp_host.as_str(), c.smtp_port, c.smtp_security), ("smtp.mail.yahoo.com", 465, Security::Ssl));
    assert_eq!((c.username.as_str(), c.smtp_username.as_str(), c.auth_method, c.source), ("jane@own.example", "jane@own.example", AuthMethod::Password, Source::MailRecord));
    assert_eq!((r.provider, r.sign_in_needed, r.microsoft_365_tenant), (None, false, false), "Yahoo has no Provider name yet (the engine's map has no variant)");
    assert_eq!(probe.count(), 0, "the guess never ran");
}

#[test]
fn the_aol_exchanger_on_the_shared_yahoo_dns_and_a_lookalike_are_not_yahoo() {
    // mutant: yahoo-suffix-widened (mx_suffixes "am0.yahoodns.net" -> "yahoodns.net": the AOL exchanger is read as Yahoo, red here)
    // fixtures/autoconfig/ispdb-yahoo.com.xml:24-25: yahoodns.net is "also used for AOL"
    for mx in ["mx-aol.mail.gm0.yahoodns.net.", "evilam0.yahoodns.net.", "am0.yahoodns.net.evil.example."] {
        let dns = FakeDns::default().with_mx("own.example", &[(10, mx)]);
        let probe = FakeProbe::default();
        let r = run(&dns, &probe, "jane@own.example");
        assert!(!matches!(&r.outcome, Outcome::Found(c) if c.imap_host.contains("yahoo")), "{mx}: {:?}", r.outcome);
        assert!(probe.count() > 0, "{mx}: no row matched, so the guess ran");
    }
}

#[test]
fn the_imap_guess_probes_the_bare_domain_last_and_never_for_smtp() {
    // mutants: bare-domain-dropped (guess.rs: `, d.to_string()` removed from the Imap hosts, red on the first list),
    //          bare-domain-first (moved before imap.{d}, red on the first list),
    //          bare-domain-in-smtp (added to the Smtp hosts, red on the second list)
    let probe = FakeProbe::default();
    let r = run(&FakeDns::default(), &probe, "jane@bare.example");
    assert!(matches!(r.outcome, Outcome::Refused(_)));
    assert_eq!(probe.tried(), vec![
        ("imap.bare.example".into(), 993, Security::Ssl), ("imap.bare.example".into(), 143, Security::Starttls),
        ("mail.bare.example".into(), 993, Security::Ssl), ("mail.bare.example".into(), 143, Security::Starttls),
        ("bare.example".into(), 993, Security::Ssl), ("bare.example".into(), 143, Security::Starttls),
    ]);
    let probe = FakeProbe { answering: vec![("imap.bare.example".into(), 993, Security::Ssl)], ..FakeProbe::default() };
    let r = run(&FakeDns::default(), &probe, "jane@bare.example");
    assert!(matches!(r.outcome, Outcome::ImapOnly(_)));
    let smtp: Vec<_> = probe.tried().into_iter().skip(1).collect();
    assert_eq!(smtp, vec![
        ("smtp.bare.example".into(), 465, Security::Ssl), ("smtp.bare.example".into(), 587, Security::Starttls),
        ("mail.bare.example".into(), 465, Security::Ssl), ("mail.bare.example".into(), 587, Security::Starttls),
    ]);
}

#[test]
fn a_server_on_the_bare_domain_alone_is_found() {
    let probe = FakeProbe { answering: vec![("bare.example".into(), 993, Security::Ssl), ("smtp.bare.example".into(), 587, Security::Starttls)], ..FakeProbe::default() };
    let r = run(&FakeDns::default(), &probe, "jane@bare.example");
    let c = found(&r);
    assert_eq!((c.imap_host.as_str(), c.imap_port, c.imap_security, c.source), ("bare.example", 993, Security::Ssl, Source::Guess));
    assert_eq!((c.smtp_host.as_str(), c.smtp_port), ("smtp.bare.example", 587));
}
