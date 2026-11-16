//! A local stand-in for APNs, for tests and the check script. `relay apns-stub --bind ADDR
//! --pub-key FILE --ok-token HEX --gone-token HEX --log FILE`. It speaks cleartext HTTP/2 on
//! 127.0.0.1, verifies the ES256 provider token, answers 200 for the ok token, 410 for the gone
//! token and 400 BadDeviceToken for any other, and appends one JSON line per request to the log.
use axum::body::Bytes;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Json, Router};
use jsonwebtoken::{Algorithm, DecodingKey, Validation};
use std::collections::HashMap;
use std::io::Write;
use std::sync::{Arc, Mutex};

struct StubState {
    key: DecodingKey,
    ok_token: String,
    gone_token: String,
    log: Mutex<std::fs::File>,
}

pub async fn run(args: &[String]) {
    let opts: HashMap<&str, &str> = args.chunks(2).filter(|c| c.len() == 2).map(|c| (c[0].as_str(), c[1].as_str())).collect();
    let need = |k: &str| opts.get(k).copied().unwrap_or_else(|| panic!("apns-stub: missing {k}"));
    let pem = std::fs::read(need("--pub-key")).expect("apns-stub: cannot read --pub-key");
    let state = Arc::new(StubState {
        key: DecodingKey::from_ec_pem(&pem).expect("apns-stub: --pub-key is not an EC public PEM"),
        ok_token: need("--ok-token").to_lowercase(),
        gone_token: need("--gone-token").to_lowercase(),
        log: Mutex::new(std::fs::OpenOptions::new().create(true).append(true).open(need("--log")).expect("apns-stub: cannot open --log")),
    });
    let listener = tokio::net::TcpListener::bind(need("--bind")).await.expect("apns-stub: cannot bind");
    println!("listening on http://{}", listener.local_addr().expect("apns-stub: no local address"));
    let app = Router::new().route("/3/device/{token}", post(handle)).with_state(state);
    axum::serve(listener, app).await.expect("apns-stub: serve failed");
}

fn header(h: &HeaderMap, name: &str) -> Option<String> {
    h.get(name).and_then(|v| v.to_str().ok()).map(str::to_owned)
}

async fn handle(State(st): State<Arc<StubState>>, Path(token): Path<String>, headers: HeaderMap, body: Bytes) -> Response {
    let jwt = header(&headers, "authorization").and_then(|a| a.strip_prefix("bearer ").map(str::to_owned));
    let mut validation = Validation::new(Algorithm::ES256);
    validation.validate_exp = false;
    validation.required_spec_claims.clear();
    let jwt_ok = jwt.map(|j| jsonwebtoken::decode::<serde_json::Value>(&j, &st.key, &validation).is_ok()).unwrap_or(false);
    let topic = header(&headers, "apns-topic");
    let json_body: serde_json::Value = serde_json::from_slice(&body).unwrap_or(serde_json::Value::Null);
    let line = serde_json::json!({
        "token": token, "jwt_ok": jwt_ok, "topic": topic,
        "push_type": header(&headers, "apns-push-type"), "priority": header(&headers, "apns-priority"),
        "collapse_id": header(&headers, "apns-collapse-id"), "body": json_body,
    });
    if let Ok(mut f) = st.log.lock() {
        let _ = writeln!(f, "{line}");
    }
    let reply = |code: StatusCode, reason: &str| (code, Json(serde_json::json!({ "reason": reason }))).into_response();
    if !jwt_ok {
        return reply(StatusCode::FORBIDDEN, "InvalidProviderToken");
    }
    if topic.is_none() {
        return reply(StatusCode::BAD_REQUEST, "MissingTopic");
    }
    let token = token.to_lowercase();
    if token == st.ok_token {
        (StatusCode::OK, [("apns-id", "00000000-0000-0000-0000-000000000000")]).into_response()
    } else if token == st.gone_token {
        reply(StatusCode::GONE, "Unregistered")
    } else {
        reply(StatusCode::BAD_REQUEST, "BadDeviceToken")
    }
}
