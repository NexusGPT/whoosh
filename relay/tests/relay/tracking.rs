//! Open and click tracking on sent mail: opt-in per message, token and time only.
use crate::common::*;
use rusqlite::{Connection, OpenFlags};
use serde_json::json;

const DEST: &str = "https://example.com/offer?ref=abc";

async fn new_message(r: &Relay) -> String {
    let (s, b) = r.post("/v1/track/messages", json!({})).await;
    assert_eq!(s, 201);
    b["token"].as_str().unwrap().to_string()
}

#[tokio::test]
async fn opens_are_counted_for_an_opted_in_message_only() {
    let r = start_relay(&work_dir("track-open"), None);
    let token = new_message(&r).await;
    let (_, none) = r.get(&format!("/v1/track/messages/{token}")).await;
    assert_eq!((none["opened"].clone(), none["opens"].clone()), (json!(false), json!(0)));
    let pixel = r.http.get(format!("{}/t/o/{token}.gif", r.base)).send().await.unwrap();
    assert_eq!(pixel.status().as_u16(), 200);
    assert_eq!(pixel.headers()["content-type"], "image/gif");
    assert_eq!(pixel.headers()["cache-control"], "no-store", "a cached pixel would hide the second open");
    assert!(pixel.bytes().await.unwrap().starts_with(b"GIF89a"));
    r.http.get(format!("{}/t/o/{token}.gif", r.base)).send().await.unwrap();
    let (_, s) = r.get(&format!("/v1/track/messages/{token}")).await;
    assert_eq!((s["opened"].clone(), s["opens"].clone(), s["clicked"].clone()), (json!(true), json!(2), json!(false)));
    // A message nobody opted in answers the same pixel and records nothing.
    let stranger = "abcdef0123456789abcdef0123456789";
    let px = r.http.get(format!("{}/t/o/{stranger}.gif", r.base)).send().await.unwrap();
    assert_eq!(px.status().as_u16(), 200);
    assert_eq!(r.get(&format!("/v1/track/messages/{stranger}")).await.0, 404);
}

#[tokio::test]
async fn a_signed_link_redirects_and_counts_the_click() {
    let r = start_relay(&work_dir("track-click"), None);
    let token = new_message(&r).await;
    let (s, b) = r.post("/v1/track/links", json!({ "token": token, "url": DEST })).await;
    assert_eq!(s, 200);
    let link = b["link_url"].as_str().unwrap().to_string();
    let hit = r.http.get(&link).send().await.unwrap();
    assert_eq!(hit.status().as_u16(), 302);
    assert_eq!(hit.headers()["location"], DEST);
    let (_, sum) = r.get(&format!("/v1/track/messages/{token}")).await;
    assert_eq!((sum["clicked"].clone(), sum["clicks"].clone(), sum["opened"].clone()), (json!(true), json!(1), json!(false)));
}

#[tokio::test]
async fn only_links_this_relay_signed_redirect() {
    let r = start_relay(&work_dir("track-forge"), None);
    let token = new_message(&r).await;
    let (_, b) = r.post("/v1/track/links", json!({ "token": token, "url": DEST })).await;
    let link = b["link_url"].as_str().unwrap().to_string();
    // Swap the destination, keep the signature: an open redirect would follow it.
    let evil = base64_url("https://evil.example/phish");
    let forged = link.replacen(link.split("u=").nth(1).unwrap().split('&').next().unwrap(), &evil, 1);
    let r1 = r.http.get(&forged).send().await.unwrap();
    assert_eq!(r1.status().as_u16(), 400);
    assert!(r1.headers().get("location").is_none());
    let bare = format!("{}/t/c/{token}?u={}&s=00", r.base, base64_url(DEST));
    assert_eq!(r.http.get(&bare).send().await.unwrap().status().as_u16(), 400);
    assert_eq!(r.get(&format!("/v1/track/messages/{token}")).await.1["clicks"], 0, "a refused link records nothing");
    // Registration and scheme rules.
    assert_eq!(r.post("/v1/track/links", json!({ "token": "abcdef", "url": DEST })).await.0, 404);
    assert_eq!(r.post("/v1/track/links", json!({ "token": token, "url": "javascript:alert(1)" })).await.0, 400);
}

#[tokio::test]
async fn tracking_rows_hold_no_address_and_no_url() {
    let r = start_relay(&work_dir("track-privacy"), None);
    let token = new_message(&r).await;
    let (_, b) = r.post("/v1/track/links", json!({ "token": token, "url": DEST })).await;
    r.http.get(format!("{}/t/o/{token}.gif", r.base)).header("x-forwarded-for", "203.0.113.9").header("user-agent", "Mail/1.0 bob@example.com").send().await.unwrap();
    r.http.get(b["link_url"].as_str().unwrap()).send().await.unwrap();
    let c = Connection::open_with_flags(&r.db, OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
    let mut dump = String::new();
    for table in ["tracked_message", "tracking_event"] {
        let mut st = c.prepare(&format!("SELECT * FROM {table}")).unwrap();
        let n = st.column_count();
        let mut rows = st.query([]).unwrap();
        while let Some(row) = rows.next().unwrap() {
            for i in 0..n {
                dump.push_str(&format!("{:?} ", row.get_ref(i).unwrap()));
            }
        }
    }
    for needle in ["@", "203.0.113.9", "example.com", "http", "Mail/1.0"] {
        assert!(!dump.contains(needle), "tracking rows leak {needle}: {dump}");
    }
}

fn base64_url(s: &str) -> String {
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use base64::Engine;
    URL_SAFE_NO_PAD.encode(s.as_bytes())
}
