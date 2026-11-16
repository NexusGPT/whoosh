//! The real DnsPort over hickory-resolver, reading this Mac's resolver configuration. Synchronous from the
//! outside: each query runs on a private current-thread tokio runtime.
use std::net::{IpAddr, SocketAddr};

use hickory_resolver::config::{NameServerConfig, ResolverConfig};
use hickory_resolver::net::runtime::TokioRuntimeProvider;
use hickory_resolver::proto::rr::{RData, RecordType};
use hickory_resolver::TokioResolver;

use crate::ports::{DnsAnswer, DnsPort, DnsQuery, MxRecord, NetError, SrvRecord};

const RESOLV_CONF: &str = "/etc/resolv.conf";

/// The `nameserver` lines whose address parses; a scoped IPv6 address (`fe80::…%en0`) does not and is skipped.
pub(crate) fn usable_nameservers(conf: &str) -> Vec<IpAddr> {
    conf.lines()
        .filter_map(|l| {
            let mut f = l.split_whitespace();
            (f.next() == Some("nameserver")).then(|| f.next()).flatten().and_then(|a| a.parse().ok())
        })
        .collect()
}

pub struct HickoryDns {
    resolver: TokioResolver,
    runtime: tokio::runtime::Runtime,
}

fn runtime() -> Result<tokio::runtime::Runtime, String> {
    tokio::runtime::Builder::new_current_thread().enable_all().build().map_err(|e| e.to_string())
}

fn resolver_for(servers: Vec<NameServerConfig>) -> Result<TokioResolver, String> {
    let config = ResolverConfig::from_parts(None, Vec::new(), servers);
    TokioResolver::builder_with_config(config, TokioRuntimeProvider::default()).build().map_err(|e| e.to_string())
}

impl HickoryDns {
    /// This Mac's resolver configuration. When hickory cannot read it (one scoped IPv6 name server in
    /// /etc/resolv.conf fails the whole file), the name servers of /etc/resolv.conf that parse are used.
    pub fn new() -> Result<Self, String> {
        let runtime = runtime()?;
        let resolver = {
            let _ctx = runtime.enter();
            match TokioResolver::builder_tokio() {
                Ok(b) => b.build().map_err(|e| e.to_string())?,
                Err(system_error) => {
                    let conf = std::fs::read_to_string(RESOLV_CONF).unwrap_or_default();
                    let servers = usable_nameservers(&conf);
                    if servers.is_empty() {
                        return Err(format!("the system resolver configuration cannot be read ({system_error}) and {RESOLV_CONF} names no usable name server"));
                    }
                    resolver_for(servers.into_iter().map(NameServerConfig::udp_and_tcp).collect())?
                }
            }
        };
        Ok(Self { resolver, runtime })
    }

    /// A resolver that asks one name server over UDP, e.g. a local stub in a test.
    pub fn with_nameserver(addr: SocketAddr) -> Result<Self, String> {
        let runtime = runtime()?;
        let mut ns = NameServerConfig::udp(addr.ip());
        for c in &mut ns.connections {
            c.port = addr.port();
        }
        let resolver = {
            let _ctx = runtime.enter();
            resolver_for(vec![ns])?
        };
        Ok(Self { resolver, runtime })
    }
}

fn net_error(e: &hickory_resolver::net::NetError) -> NetError {
    let s = e.to_string();
    let lower = s.to_ascii_lowercase();
    if e.is_nx_domain() { NetError::HostNotFound } else if lower.contains("timed out") || lower.contains("timeout") { NetError::Timeout } else { NetError::Other(s) }
}

impl DnsPort for HickoryDns {
    fn execute(&self, query: &DnsQuery) -> Result<DnsAnswer, NetError> {
        let (DnsQuery::Srv(name) | DnsQuery::Mx(name) | DnsQuery::Exists(name)) = query;
        netguard::outbound_allowed(name).map_err(|_| NetError::Refused)?;   // engine-act-r6 round 5: a DNS query is traffic too
        match query {
            DnsQuery::Srv(name) => match self.runtime.block_on(self.resolver.lookup(name.as_str(), RecordType::SRV)) {
                Ok(l) => Ok(DnsAnswer::Srv(
                    l.answers()
                        .iter()
                        .filter_map(|r| match &r.data {
                            RData::SRV(s) => Some(SrvRecord { priority: s.priority, weight: s.weight, port: s.port, target: s.target.to_ascii() }),
                            _ => None,
                        })
                        .collect(),
                )),
                // NXDOMAIN or no SRV record: the service is not published
                Err(e) if e.is_no_records_found() => Ok(DnsAnswer::Srv(Vec::new())),
                Err(e) => Err(net_error(&e)),
            },
            DnsQuery::Mx(domain) => match self.runtime.block_on(self.resolver.lookup(domain.as_str(), RecordType::MX)) {
                Ok(l) => Ok(DnsAnswer::Mx(
                    l.answers()
                        .iter()
                        .filter_map(|r| match &r.data {
                            RData::MX(m) => Some(MxRecord { preference: m.preference, exchange: m.exchange.to_ascii() }),
                            _ => None,
                        })
                        .collect(),
                )),
                Err(e) if e.is_no_records_found() => Ok(DnsAnswer::Mx(Vec::new())),
                Err(e) => Err(net_error(&e)),
            },
            // SOA for the name: NXDOMAIN means the domain does not exist; an empty answer means it does
            DnsQuery::Exists(domain) => match self.runtime.block_on(self.resolver.lookup(domain.as_str(), RecordType::SOA)) {
                Ok(_) => Ok(DnsAnswer::Exists(true)),
                Err(e) if e.is_nx_domain() => Ok(DnsAnswer::Exists(false)),
                Err(e) if e.is_no_records_found() => Ok(DnsAnswer::Exists(true)),
                Err(e) => Err(net_error(&e)),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    //! The real resolver code path against a local DNS stub on 127.0.0.1 serving fixtures/autoconfig/srv-domain.txt:
    //! no fake port, no network beyond loopback.
    use super::*;
    use hickory_resolver::proto::op::{Message, OpCode, ResponseCode};
    use hickory_resolver::proto::rr::rdata::{MX, SRV};
    use hickory_resolver::proto::rr::{Name, Record};
    use std::collections::HashMap;
    use std::net::UdpSocket;
    use std::time::Duration;

    type SrvRows = HashMap<String, Vec<(u16, u16, u16, String)>>;

    fn fixture_srv() -> SrvRows {
        let p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../fixtures/autoconfig/srv-domain.txt");
        let text = std::fs::read_to_string(&p).expect("read the SRV fixture");
        let mut map = SrvRows::new();
        for l in text.lines().filter(|l| l.starts_with("srv ")) {
            let f: Vec<&str> = l.split_whitespace().collect();
            map.entry(f[1].to_string()).or_default().push((f[2].parse().expect("p"), f[3].parse().expect("w"), f[4].parse().expect("port"), f[5].to_string()));
        }
        map
    }

    /// Answers SRV from the fixture; any other name under the fixture's domain with an empty NOERROR; else NXDOMAIN.
    fn stub() -> SocketAddr {
        let sock = UdpSocket::bind("127.0.0.1:0").expect("bind");
        sock.set_read_timeout(Some(Duration::from_secs(10))).expect("timeout");
        let addr = sock.local_addr().expect("addr");
        let srv = fixture_srv();
        std::thread::spawn(move || {
            let mut buf = [0u8; 1500];
            while let Ok((n, from)) = sock.recv_from(&mut buf) {
                let Ok(q) = Message::from_vec(&buf[..n]) else { continue };
                let Some(query) = q.queries.first().cloned() else { continue };
                let mut r = Message::response(q.metadata.id, OpCode::Query);
                r.metadata.recursion_desired = q.metadata.recursion_desired;
                r.metadata.recursion_available = true;
                let name = query.name().to_ascii().trim_end_matches('.').to_ascii_lowercase();
                r.add_query(query.clone());
                if query.query_type() == RecordType::MX && name == "mx-only.example" {
                    let ex = Name::from_ascii("tenant.mail.protection.outlook.com.").expect("exchange");
                    r.add_answer(Record::from_rdata(query.name().clone(), 60, RData::MX(MX::new(10, ex))));
                    if let Ok(bytes) = r.to_vec() {
                        let _ = sock.send_to(&bytes, from);
                    }
                    continue;
                }
                match (query.query_type(), srv.get(&name)) {
                    (RecordType::SRV, Some(recs)) => {
                        for (p, w, port, target) in recs {
                            let t = Name::from_ascii(target).expect("target");
                            r.add_answer(Record::from_rdata(query.name().clone(), 60, RData::SRV(SRV::new(*p, *w, *port, t))));
                        }
                    }
                    _ if name == "srv-only.example" || name.ends_with(".srv-only.example") => {}
                    _ => r.metadata.response_code = ResponseCode::NXDomain,
                }
                if let Ok(bytes) = r.to_vec() {
                    let _ = sock.send_to(&bytes, from);
                }
            }
        });
        addr
    }

    #[test]
    fn a_scoped_ipv6_name_server_is_skipped_and_the_rest_kept() {
        let conf = "# c\nsearch home\nnameserver fe80::3ae1:f4ff:fe7a:2e72%en0\nnameserver 192.168.128.1\nnameserver 2001:db8::53\n";
        let got: Vec<String> = usable_nameservers(conf).iter().map(|a| a.to_string()).collect();
        assert_eq!(got, vec!["192.168.128.1".to_string(), "2001:db8::53".to_string()]);
    }

    #[test]
    fn the_real_resolver_reads_srv_records_and_nxdomain_from_a_local_stub() {
        let dns = HickoryDns::with_nameserver(stub()).expect("resolver");
        let Ok(DnsAnswer::Srv(mut recs)) = dns.execute(&DnsQuery::Srv("_imaps._tcp.srv-only.example".into())) else { panic!("no SRV answer") };
        recs.sort_by_key(|r| r.priority);
        assert_eq!(recs.len(), 2);
        assert_eq!((recs[0].priority, recs[0].port, recs[0].target.as_str()), (0, 993, "imap.srv-only.example."));
        assert_eq!(dns.execute(&DnsQuery::Srv("_imap._tcp.srv-only.example".into())), Ok(DnsAnswer::Srv(Vec::new())));
        assert_eq!(dns.execute(&DnsQuery::Exists("srv-only.example".into())), Ok(DnsAnswer::Exists(true)));
        assert_eq!(dns.execute(&DnsQuery::Exists("nope-typo.example".into())), Ok(DnsAnswer::Exists(false)));
        assert_eq!(
            dns.execute(&DnsQuery::Mx("mx-only.example".into())),
            Ok(DnsAnswer::Mx(vec![MxRecord { preference: 10, exchange: "tenant.mail.protection.outlook.com.".into() }]))
        );
        assert_eq!(dns.execute(&DnsQuery::Mx("srv-only.example".into())), Ok(DnsAnswer::Mx(Vec::new())));
    }

    #[test]
    fn detection_answers_by_srv_through_the_real_resolver() {
        use crate::domain::config::{Outcome, Security, Source};
        use crate::ports::{HttpPort, HttpRequest, HttpResponse, ProbeOk, ProbePort, ProbeRequest};
        struct Miss;
        impl HttpPort for Miss {
            fn execute(&self, _: &HttpRequest) -> Result<HttpResponse, NetError> { Ok(HttpResponse { status: 404, body: String::new() }) }
        }
        impl ProbePort for Miss {
            fn execute(&self, _: &ProbeRequest) -> Result<ProbeOk, NetError> { Err(NetError::Refused) }
        }
        let dns = HickoryDns::with_nameserver(stub()).expect("resolver");
        let r = crate::Detector::new(Miss, dns, Miss).detect("jane@srv-only.example");
        let Outcome::Found(c) = r.outcome else { panic!("expected SRV to answer, got {:?}", r.outcome) };
        assert_eq!((c.imap_host.as_str(), c.imap_port, c.imap_security), ("imap.srv-only.example", 993, Security::Ssl));
        assert_eq!((c.smtp_host.as_str(), c.smtp_port, c.source), ("smtp.srv-only.example", 587, Source::Srv));
    }
}
