//! A small SMTP server for tests: loopback only, EHLO, AUTH PLAIN / XOAUTH2, NOOP, QUIT. It never
//! accepts a message: the validation only needs the greeting and the sign-in.
#![allow(dead_code)]
use std::sync::{Arc, Mutex};

use base64::{engine::general_purpose::STANDARD, Engine as _};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::TcpListener,
    task::JoinHandle,
};

pub struct FakeSmtp {
    pub port: u16,
    pub log: Arc<Mutex<Vec<String>>>,
    task: JoinHandle<()>,
}

impl Drop for FakeSmtp {
    fn drop(&mut self) { self.task.abort(); }
}

impl FakeSmtp {
    pub async fn start(user: &str, secret: &str) -> FakeSmtp {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind loopback");
        let port = listener.local_addr().unwrap().port();
        let log = Arc::new(Mutex::new(Vec::new()));
        let (lg, user, secret) = (log.clone(), user.to_string(), secret.to_string());
        let task = tokio::spawn(async move {
            loop {
                let Ok((sock, _)) = listener.accept().await else { break };
                let (lg, user, secret) = (lg.clone(), user.clone(), secret.clone());
                tokio::spawn(async move {
                    let (r, mut w) = sock.into_split();
                    let mut r = BufReader::new(r);
                    if w.write_all(b"220 fake.test ESMTP ready\r\n").await.is_err() { return; }
                    let mut line = String::new();
                    loop {
                        line.clear();
                        if r.read_line(&mut line).await.unwrap_or(0) == 0 { return; }
                        let cmd = line.trim_end().to_string();
                        lg.lock().unwrap().push(cmd.split(' ').take(2).collect::<Vec<_>>().join(" "));
                        let up = cmd.to_ascii_uppercase();
                        let reply: String = if up.starts_with("EHLO") || up.starts_with("HELO") {
                            "250-fake.test\r\n250-AUTH PLAIN LOGIN XOAUTH2\r\n250 8BITMIME\r\n".into()
                        } else if up.starts_with("AUTH") {
                            let mut parts = cmd.splitn(3, ' ');
                            parts.next();
                            let mech = parts.next().unwrap_or("").to_ascii_uppercase();
                            let inline = parts.next().map(str::to_string);
                            let resp = match inline {
                                Some(x) => x,
                                None => {
                                    if w.write_all(b"334 \r\n").await.is_err() { return; }
                                    let mut l = String::new();
                                    let _ = r.read_line(&mut l).await;
                                    l.trim().to_string()
                                }
                            };
                            let got = STANDARD.decode(resp.trim()).unwrap_or_default();
                            let want = if mech == "XOAUTH2" { format!("user={user}\x01auth=Bearer {secret}\x01\x01") } else { format!("\0{user}\0{secret}") };
                            if got == want.as_bytes() { "235 2.7.0 Authentication successful\r\n".into() } else { "535 5.7.8 Username and Password not accepted\r\n".into() }
                        } else if up.starts_with("NOOP") || up.starts_with("RSET") {
                            "250 2.0.0 Ok\r\n".into()
                        } else if up.starts_with("QUIT") {
                            let _ = w.write_all(b"221 2.0.0 Bye\r\n").await;
                            return;
                        } else {
                            "502 5.5.1 command not implemented\r\n".into()
                        };
                        if w.write_all(reply.as_bytes()).await.is_err() { return; }
                    }
                });
            }
        });
        FakeSmtp { port, log, task }
    }
}
