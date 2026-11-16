//! detect() against fakes that read fixtures/autoconfig. Each ordering test makes step N answer and asserts
//! step N+1 was never called.
mod support;
use autoconfig::*;
use support::*;

const ISPDB: &str = "https://autoconfig.thunderbird.net/v1.1/";

fn run(http: &FakeHttp, dns: &FakeDns, probe: &FakeProbe, email: &str) -> DetectionResult {
    Detector::new(http, dns, probe).detect(email)
}

#[test]
fn a_ispdb_gmail_answers_with_the_user_agent_and_b_is_never_called() {
    let http = FakeHttp::default().ok(format!("{ISPDB}gmail.com"), fixture("ispdb-gmail.com.xml"));
    let (dns, probe) = (FakeDns::default(), FakeProbe::default());
    let r = run(&http, &dns, &probe, "Jane.Doe@Gmail.com");
    let c = found(&r);
    assert_eq!((c.imap_host.as_str(), c.imap_port, c.imap_security), ("imap.gmail.com", 993, Security::Ssl));
    assert_eq!((c.smtp_host.as_str(), c.smtp_port, c.smtp_security), ("smtp.gmail.com", 465, Security::Ssl));
    assert_eq!(c.username, "Jane.Doe@gmail.com");
    assert_eq!((c.auth_method, c.source), (AuthMethod::Oauth2, Source::Ispdb));
    assert_eq!(r.steps_tried, vec![Step::Ispdb]);
    let calls = http.calls.lock().expect("lock");
    assert_eq!(calls.len(), 1, "step b was called: {:?}", calls.iter().map(|c| &c.url).collect::<Vec<_>>());
    assert_eq!(calls[0].url, format!("{ISPDB}gmail.com"), "the ISPDB request carries the domain only");
    assert_eq!(calls[0].user_agent, format!("Whoosh/{} (+https://whoosh.app)", env!("CARGO_PKG_VERSION")));
    assert_eq!((dns.count(), probe.count()), (0, 0));
}

#[test]
fn ispdb_fixtures_parse_for_every_recorded_provider() {
    // (domain, imap host, imap port, imap security, username, smtp host, smtp port, smtp security, smtp username, auth)
    let cases = [
        ("outlook.com", "outlook.office365.com", 993, Security::Ssl, "jane@outlook.com", "smtp-mail.outlook.com", 587, Security::Starttls, "jane@outlook.com", AuthMethod::Oauth2),
        ("icloud.com", "imap.mail.me.com", 993, Security::Ssl, "jane", "smtp.mail.me.com", 587, Security::Starttls, "jane@icloud.com", AuthMethod::Password),
        ("yahoo.com", "imap.mail.yahoo.com", 993, Security::Ssl, "jane@yahoo.com", "smtp.mail.yahoo.com", 465, Security::Ssl, "jane@yahoo.com", AuthMethod::Oauth2),
    ];
    for (d, ih, ip, is, iu, sh, sp, ss, su, auth) in cases {
        let http = FakeHttp::default().ok(format!("{ISPDB}{d}"), fixture(&format!("ispdb-{d}.xml")));
        let r = run(&http, &FakeDns::default(), &FakeProbe::default(), &format!("jane@{d}"));
        let c = found(&r);
        assert_eq!((c.imap_host.as_str(), c.imap_port, c.imap_security, c.username.as_str()), (ih, ip, is, iu), "{d} imap");
        assert_eq!((c.smtp_host.as_str(), c.smtp_port, c.smtp_security, c.smtp_username.as_str()), (sh, sp, ss, su), "{d} smtp");
        assert_eq!((c.auth_method, c.source), (auth, Source::Ispdb), "{d}");
    }
}

#[test]
fn fastmail_is_not_in_the_provider_database_so_step_b_runs_next() {
    let http = FakeHttp::default().status(format!("{ISPDB}fastmail.com"), 404, fixture("ispdb-fastmail.com.404.html"));
    let r = run(&http, &FakeDns::default(), &FakeProbe::default(), "jane@fastmail.com");
    let urls = http.urls();
    assert_eq!(urls[0], format!("{ISPDB}fastmail.com"));
    assert_eq!(urls[1], "https://autoconfig.fastmail.com/mail/config-v1.1.xml?emailaddress=jane@fastmail.com");
    assert_eq!(r.steps_tried[..2], [Step::Ispdb, Step::AutoconfigUrls]);
}

#[test]
fn b_autoconfig_subdomain_answers_after_ispdb_misses_and_c_is_never_called() {
    let http = FakeHttp::default().ok(
        "https://autoconfig.corp.example/mail/config-v1.1.xml?emailaddress=jane@corp.example",
        fixture("ispdb-yahoo.com.xml").replace("mail.yahoo.com", "corp.example"),
    );
    let (dns, probe) = (FakeDns::default(), FakeProbe::default());
    let r = run(&http, &dns, &probe, "jane@corp.example");
    let c = found(&r);
    assert_eq!((c.imap_host.as_str(), c.source), ("imap.corp.example", Source::Autoconfig));
    assert_eq!(http.urls(), vec![format!("{ISPDB}corp.example"), "https://autoconfig.corp.example/mail/config-v1.1.xml?emailaddress=jane@corp.example".to_string()]);
    assert_eq!(r.steps_tried, vec![Step::Ispdb, Step::AutoconfigUrls]);
    assert_eq!(dns.count(), 0, "step c ran after step b answered");
}

#[test]
fn b_well_known_url_answers_when_the_subdomain_misses() {
    let http = FakeHttp::default().ok(
        "https://corp.example/.well-known/autoconfig/mail/config-v1.1.xml",
        fixture("ispdb-icloud.com.xml").replace("mail.me.com", "corp.example"),
    );
    let r = run(&http, &FakeDns::default(), &FakeProbe::default(), "jane@corp.example");
    assert_eq!(found(&r).imap_host, "imap.corp.example");
    assert_eq!(http.urls().len(), 3);
}

#[test]
fn c_srv_answers_with_the_lowest_priority_and_d_is_never_called() {
    let http = FakeHttp::default();
    let dns = FakeDns::from_fixture(&fixture("srv-domain.txt"));
    let probe = FakeProbe::default();
    let r = run(&http, &dns, &probe, "jane@srv-only.example");
    let c = found(&r);
    assert_eq!((c.imap_host.as_str(), c.imap_port, c.imap_security), ("imap.srv-only.example", 993, Security::Ssl));
    assert_eq!((c.smtp_host.as_str(), c.smtp_port, c.smtp_security), ("smtp.srv-only.example", 587, Security::Starttls));
    assert_eq!((c.username.as_str(), c.source), ("jane@srv-only.example", Source::Srv));
    assert!(!http.called("https://autodiscover"), "step d ran after step c answered: {:?}", http.urls());
    assert_eq!(probe.count(), 0);
    assert_eq!(r.steps_tried, vec![Step::Ispdb, Step::AutoconfigUrls, Step::Srv]);
}

#[test]
fn c_srv_falls_back_to_imap_with_starttls() {
    let mut dns = FakeDns::default();
    dns.srv.insert("_imap._tcp.plain.example".into(), vec![SrvRecord { priority: 0, weight: 0, port: 143, target: "imap.plain.example.".into() }]);
    dns.srv.insert("_imaps._tcp.plain.example".into(), vec![SrvRecord { priority: 0, weight: 0, port: 0, target: ".".into() }]);
    dns.srv.insert("_submission._tcp.plain.example".into(), vec![SrvRecord { priority: 0, weight: 0, port: 465, target: "smtp.plain.example".into() }]);
    let r = run(&FakeHttp::default(), &dns, &FakeProbe::default(), "jane@plain.example");
    let c = found(&r);
    assert_eq!((c.imap_port, c.imap_security), (143, Security::Starttls));
    assert_eq!((c.smtp_port, c.smtp_security), (465, Security::Ssl));
}

#[test]
fn c_srv_takes_submissions_when_submission_is_not_offered() {
    // the shape fastmail.com publishes (_captures/core-autoconfig/srv-fastmail.com.txt): _submission is "."
    let mut dns = FakeDns::default();
    dns.srv.insert("_imaps._tcp.fm.example".into(), vec![SrvRecord { priority: 0, weight: 1, port: 993, target: "imap.fm.example.".into() }]);
    dns.srv.insert("_submission._tcp.fm.example".into(), vec![SrvRecord { priority: 0, weight: 0, port: 0, target: ".".into() }]);
    dns.srv.insert("_submissions._tcp.fm.example".into(), vec![SrvRecord { priority: 0, weight: 1, port: 465, target: "smtp.fm.example.".into() }]);
    let r = run(&FakeHttp::default(), &dns, &FakeProbe::default(), "jane@fm.example");
    let c = found(&r);
    assert_eq!((c.smtp_host.as_str(), c.smtp_port, c.smtp_security, c.source), ("smtp.fm.example", 465, Security::Ssl, Source::Srv));
}

#[test]
fn d_autodiscover_pox_answers_and_e_never_probes() {
    let http = FakeHttp::default().ok("https://autodiscover.onprem.example/autodiscover/autodiscover.xml", fixture("autodiscover-pox.xml"));
    let probe = FakeProbe::default();
    let r = run(&http, &FakeDns::default(), &probe, "jane@onprem.example");
    let c = found(&r);
    assert_eq!((c.imap_host.as_str(), c.imap_port, c.imap_security), ("mail.onprem.example", 993, Security::Ssl));
    assert_eq!((c.smtp_port, c.smtp_security, c.source), (587, Security::Starttls, Source::Autodiscover));
    assert_eq!(probe.count(), 0, "step e ran after step d answered");
    let post = http.calls.lock().expect("lock").iter().find(|q| q.method == Method::Post).cloned().expect("a POST");
    assert!(post.body.as_deref().is_some_and(|b| b.contains("<EMailAddress>jane@onprem.example</EMailAddress>")));
}

#[test]
fn d_autodiscover_marks_a_microsoft_365_tenant_with_oauth2() {
    let http = FakeHttp::default().ok(
        "https://autodiscover-s.outlook.com/autodiscover/autodiscover.json?Email=jane@contoso.example&Protocol=AutodiscoverV1",
        fixture("autodiscover-m365.json"),
    );
    let probe = FakeProbe::default();
    let r = run(&http, &FakeDns::default(), &probe, "jane@contoso.example");
    let c = found(&r);
    assert!(r.microsoft_365_tenant);
    assert_eq!((r.provider, r.sign_in_needed), (Some(Provider::Microsoft365), true));
    assert_eq!((c.imap_host.as_str(), c.smtp_host.as_str(), c.auth_method), ("outlook.office365.com", "smtp.office365.com", AuthMethod::Oauth2));
    assert_eq!(probe.count(), 0);
}

#[test]
fn e_guess_only_domain_probes_tls_first_then_starttls() {
    let probe = FakeProbe::from_fixture(&fixture("guess-only.txt"));
    let r = run(&FakeHttp::default(), &FakeDns::default(), &probe, "jane@guess-only.example");
    let c = found(&r);
    assert_eq!((c.imap_host.as_str(), c.imap_port, c.imap_security), ("imap.guess-only.example", 143, Security::Starttls));
    assert_eq!((c.smtp_host.as_str(), c.smtp_port, c.smtp_security), ("mail.guess-only.example", 465, Security::Ssl));
    assert_eq!(c.source, Source::Guess);
    assert_eq!(probe.tried(), vec![
        ("imap.guess-only.example".into(), 993, Security::Ssl),
        ("imap.guess-only.example".into(), 143, Security::Starttls),
        ("smtp.guess-only.example".into(), 465, Security::Ssl),
        ("smtp.guess-only.example".into(), 587, Security::Starttls),
        ("mail.guess-only.example".into(), 465, Security::Ssl),
    ]);
    assert_eq!(r.steps_tried, STEP_ORDER.to_vec());
}

#[test]
fn e_guess_with_imap_only_fills_the_imap_half() {
    let probe = FakeProbe { answering: vec![("mail.half.example".into(), 993, Security::Ssl)], ..FakeProbe::default() };
    let r = run(&FakeHttp::default(), &FakeDns::default(), &probe, "jane@half.example");
    match r.outcome {
        Outcome::ImapOnly(c) => assert_eq!((c.imap_host.as_str(), c.imap_port), ("mail.half.example", 993)),
        other => panic!("expected the IMAP half, got {other:?}"),
    }
}

#[test]
fn nothing_answered_is_refused_in_words_after_all_five_steps() {
    let r = run(&FakeHttp::default(), &FakeDns::default(), &FakeProbe::default(), "jane@silent.example");
    assert_eq!(r.outcome, Outcome::Refused(Refusal::NothingAnswered("silent.example".into())));
    assert_eq!(r.steps_tried, STEP_ORDER.to_vec());
    let Outcome::Refused(refusal) = &r.outcome else { unreachable!() };
    assert!(refusal.words().contains("No mail settings were found for silent.example"));
}

#[test]
fn no_network_stops_at_the_first_step() {
    let http = FakeHttp::default().fail(format!("{ISPDB}gmail.com"), NetError::Offline);
    let (dns, probe) = (FakeDns::default(), FakeProbe::default());
    let r = run(&http, &dns, &probe, "jane@gmail.com");
    assert_eq!(r.outcome, Outcome::Refused(Refusal::NoNetwork));
    assert_eq!((http.urls().len(), dns.count(), probe.count()), (1, 0, 0));
    let Outcome::Refused(refusal) = &r.outcome else { unreachable!() };
    assert!(refusal.words().contains("not connected to the internet"));
}

#[test]
fn a_domain_that_does_not_exist_stops_before_autodiscover_and_the_guess() {
    let http = FakeHttp::default();
    let dns = FakeDns { missing_domains: vec!["nope-typo.example".into()], ..FakeDns::default() };
    let probe = FakeProbe::default();
    let r = run(&http, &dns, &probe, "jane@nope-typo.example");
    assert_eq!(r.outcome, Outcome::Refused(Refusal::DomainNotFound("nope-typo.example".into())));
    assert!(!http.called("https://autodiscover"));
    assert_eq!(probe.count(), 0);
}

#[test]
fn an_invalid_address_is_refused_before_any_call() {
    let http = FakeHttp::default();
    for bad in ["", "jane", "@gmail.com", "jane@", "jane@localhost", "jane@exa_mple.com", "ja ne@gmail.com"] {
        let r = run(&http, &FakeDns::default(), &FakeProbe::default(), bad);
        assert!(matches!(r.outcome, Outcome::Refused(Refusal::InvalidAddress(_))), "{bad:?} gave {:?}", r.outcome);
    }
    assert!(http.urls().is_empty());
}

#[test]
fn progress_names_each_step_in_words_in_order() {
    let mut seen = Vec::new();
    Detector::new(&FakeHttp::default(), &FakeDns::default(), &FakeProbe::default()).detect_with_progress("jane@silent.example", &mut |s| seen.push(s.words()));
    assert_eq!(seen, vec![
        "Looking in the provider database…",
        "Asking the domain for its mail settings…",
        "Reading the domain's DNS mail records…",
        "Asking Microsoft Autodiscover…",
        "Reading the domain's mail record…",
        "Trying the usual server names…",
    ]);
}
