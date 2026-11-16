//! The real resolver path, proven once against a public domain that publishes RFC 6186 records (owner-allowed).
//! Usage: srv_capture <domain>. Writes _captures/core-autoconfig/srv-<domain>.txt.
use autoconfig::adapters::dns_hickory::HickoryDns;
use autoconfig::{DnsPort, DnsQuery};
use std::{fs, path::PathBuf};

fn main() {
    let domain = std::env::args().nth(1).expect("a domain");
    let dns = HickoryDns::new().expect("the system resolver");
    let mut out = String::new();
    for svc in ["_imaps._tcp", "_imap._tcp", "_submission._tcp", "_submissions._tcp"] {
        let name = format!("{svc}.{domain}");
        out.push_str(&format!("{name}: {:?}\n", dns.execute(&DnsQuery::Srv(name.clone()))));
    }
    out.push_str(&format!("exists {domain}: {:?}\n", dns.execute(&DnsQuery::Exists(domain.clone()))));
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!("../../../_captures/core-autoconfig/srv-{domain}.txt"));
    fs::write(&path, &out).expect("write the capture");
    print!("{out}");
}
