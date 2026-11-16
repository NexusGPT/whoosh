//! The owner-allowed provider database lookups (each sends only the domain). Domains come from the arguments
//! (default gmail.com). Writes _captures/core-autoconfig/ispdb-<first label>.xml (the body, verbatim) and
//! ispdb-<first label>.status.txt for each.
use autoconfig::adapters::http_reqwest::ReqwestHttp;
use autoconfig::consts::ISPDB_BASE;
use autoconfig::{user_agent, HttpPort, HttpRequest, Method};
use std::{fs, path::PathBuf};

fn main() {
    let out = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../_captures/core-autoconfig");
    fs::create_dir_all(&out).expect("create the capture folder");
    let http = ReqwestHttp::new().expect("build the HTTP client");
    let mut domains: Vec<String> = std::env::args().skip(1).collect();
    if domains.is_empty() { domains.push("gmail.com".into()); }
    let mut failed = false;
    for domain in domains {
        let label = domain.split('.').next().unwrap_or(&domain).to_string();
        let req = HttpRequest { method: Method::Get, url: format!("{ISPDB_BASE}{domain}"), user_agent: user_agent(), body: None };
        match http.execute(&req) {
            Ok(r) => {
                fs::write(out.join(format!("ispdb-{label}.xml")), &r.body).expect("write the body");
                let status = format!("GET {}\nUser-Agent: {}\nHTTP status: {}\nbytes: {}\n", req.url, req.user_agent, r.status, r.body.len());
                fs::write(out.join(format!("ispdb-{label}.status.txt")), &status).expect("write the status");
                print!("{status}");
            }
            Err(e) => {
                eprintln!("{domain}: the lookup failed: {e:?}");
                failed = true;
            }
        }
    }
    if failed { std::process::exit(1); }
}
