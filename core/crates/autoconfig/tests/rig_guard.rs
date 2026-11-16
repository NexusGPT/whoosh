//! engine-act-r6 round 5: the account detector's outgoing calls in a rig process (`netguard::close_to_this_mac`): its HTTPS fetches, its
//! DNS queries (MX, SRV, the name's existence) and its TLS probe refuse every host but this Mac's before anything is resolved or sent.
//! Run with `--features live` (the real adapters). Every target is local: a counting listener ("0.0.0.0" reaches it), a counting name
//! server on 127.0.0.1, the guard's resolver stub; a mutant (the guard removed) is counted there, never on the network.
#![cfg(feature = "live")]
#[path = "../../netguard/tests/stubs.rs"]
mod stubs;
use autoconfig::adapters::{dns_hickory::HickoryDns, http_reqwest::ReqwestHttp, tls_probe::TlsProbe};
use autoconfig::{DnsPort, DnsQuery, HttpPort, HttpRequest, Method, NetError, ProbePort, ProbeRequest, Protocol, Security};

#[test]
fn the_detectors_http_dns_and_probe_of_a_rig_process_reach_this_mac_only() {
    let web = stubs::listener();
    let imap = stubs::listener();
    let ns = stubs::name_server();
    let asked = stubs::resolver_to(&imap);
    netguard::close_to_this_mac();
    let before = netguard::refused_count();
    // HTTPS fetch (ISPDB, the domain's autoconfig file, Autodiscover)
    let http = ReqwestHttp::new().expect("client");
    let get = |url: &str| http.execute(&HttpRequest { method: Method::Get, url: url.into(), user_agent: "Whoosh".into(), body: None });
    assert_eq!(get(&format!("https://0.0.0.0:{}/mail/config-v1.1.xml", web.port)).err(), Some(NetError::Refused));
    assert_eq!(web.count(), 0, "the HTTPS fetch opened nothing");
    // DNS: MX, SRV and SOA, each refused before the query leaves
    let dns = HickoryDns::with_nameserver(ns.addr).expect("resolver");
    for q in [DnsQuery::Mx("example.org".into()), DnsQuery::Srv("_imaps._tcp.example.org".into()), DnsQuery::Exists("example.org".into())] {
        assert_eq!(dns.execute(&q).err(), Some(NetError::Refused), "{q:?}");
    }
    assert_eq!(ns.count(), 0, "no DNS query was sent");
    // the TLS probe of the guess step
    let probe = TlsProbe::new().expect("probe");
    let r = probe.execute(&ProbeRequest { host: "imap.example.org".into(), port: 993, protocol: Protocol::Imap, security: Security::Ssl });
    assert_eq!(r.err(), Some(NetError::Refused));
    assert_eq!(asked.load(std::sync::atomic::Ordering::SeqCst), 0, "the probe's name was not resolved");
    assert_eq!(imap.count(), 0, "the probe opened nothing");
    assert_eq!(netguard::refused_count() - before, 5, "each attempt counted");
    // loopback stays open: the rig's own fakes live there
    assert!(get(&format!("https://127.0.0.1:{}/x", web.port)).is_err());
    assert!(web.count() >= 1, "a loopback fetch is not refused by the guard");
}
// mutant "http-fetch-unguarded": http_reqwest.rs `execute`, the `netguard::outbound_allowed_url(&req.url)...?;` line deleted (red: the listener counts one)
// mutant "dns-unguarded": dns_hickory.rs `execute`, the `netguard::outbound_allowed(name)...?;` line deleted (red: the name server is asked)
// mutant "probe-unguarded": tls_probe.rs `connect`, the `netguard::outbound_allowed(host)...?;` line deleted (red: the resolver is asked, the listener counts one)
