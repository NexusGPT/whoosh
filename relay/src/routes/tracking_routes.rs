//! Sent-mail tracking: opt-in per message. The Mac registers a message, asks the relay to sign
//! each link it wants tracked, and later reads the opened/clicked summary. Recipients' mail
//! clients hit the pixel and the signed link; the relay records a token and a time, nothing else.
use super::{ApiError, ApiJson, AppState};
use crate::registry::now_unix;
use crate::tracking;
use axum::extract::{Path, Query, State};
use axum::http::{header, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use serde::Deserialize;
use serde_json::{json, Value};

/// The smallest transparent GIF.
const PIXEL: &[u8] = &[
    0x47, 0x49, 0x46, 0x38, 0x39, 0x61, 0x01, 0x00, 0x01, 0x00, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0xff, 0xff, 0xff, 0x21, 0xf9, 0x04, 0x01,
    0x00, 0x00, 0x00, 0x00, 0x2c, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x01, 0x00, 0x00, 0x02, 0x02, 0x44, 0x01, 0x00, 0x3b,
];

fn token_ok(t: &str) -> bool {
    !t.is_empty() && t.chars().all(|c| c.is_ascii_hexdigit())
}

pub async fn create_message(State(st): State<AppState>) -> Result<(StatusCode, Json<Value>), ApiError> {
    let token = tracking::new_token();
    tracking::create_message(&st.store, &token, now_unix()).map_err(ApiError::internal)?;
    let pixel_url = format!("{}/t/o/{token}.gif", st.public_base);
    Ok((StatusCode::CREATED, Json(json!({ "token": token, "pixel_url": pixel_url }))))
}

#[derive(Deserialize)]
pub struct LinkBody {
    token: String,
    url: String,
}

pub async fn sign_link(State(st): State<AppState>, ApiJson(body): ApiJson<LinkBody>) -> Result<Json<Value>, ApiError> {
    if !(body.url.starts_with("http://") || body.url.starts_with("https://")) {
        return Err(ApiError::bad_request("url must start with http:// or https://"));
    }
    if !token_ok(&body.token) || !tracking::message_exists(&st.store, &body.token).map_err(ApiError::internal)? {
        return Err(ApiError::not_found("unknown tracking token"));
    }
    let signature = st.signer.sign(&body.token, &body.url).map_err(ApiError::internal)?;
    let link_url = format!("{}/t/c/{}?u={}&s={signature}", st.public_base, body.token, URL_SAFE_NO_PAD.encode(body.url.as_bytes()));
    Ok(Json(json!({ "link_url": link_url })))
}

pub async fn summary(State(st): State<AppState>, Path(token): Path<String>) -> Result<Json<Value>, ApiError> {
    let found = if token_ok(&token) { tracking::summary(&st.store, &token).map_err(ApiError::internal)? } else { None };
    let s = found.ok_or_else(|| ApiError::not_found("unknown tracking token"))?;
    Ok(Json(json!({
        "token": token, "opened": s.opens > 0, "clicked": s.clicks > 0, "opens": s.opens, "clicks": s.clicks,
        "first_open_at": s.first_open_at, "last_open_at": s.last_open_at, "last_click_at": s.last_click_at,
    })))
}

/// GET /t/o/<token>.gif. Always answers with the pixel, so the response never reveals whether a
/// token is known; only an opted-in message records the open.
pub async fn pixel(State(st): State<AppState>, Path(file): Path<String>) -> Response {
    let token = file.strip_suffix(".gif").unwrap_or(&file);
    if token_ok(token) {
        if let Err(e) = tracking::record(&st.store, token, "open", now_unix()) {
            tracing::error!("could not record an open: {e}");
        }
    }
    ([(header::CONTENT_TYPE, "image/gif"), (header::CACHE_CONTROL, "no-store")], PIXEL).into_response()
}

#[derive(Deserialize)]
pub struct ClickQuery {
    u: String,
    s: String,
}

/// GET /t/c/<token>?u=<base64url destination>&s=<signature>. Redirects only a link this relay signed.
pub async fn click(State(st): State<AppState>, Path(token): Path<String>, Query(q): Query<ClickQuery>) -> Result<Response, ApiError> {
    let url = URL_SAFE_NO_PAD
        .decode(q.u.as_bytes())
        .ok()
        .and_then(|b| String::from_utf8(b).ok())
        .filter(|u| u.starts_with("http://") || u.starts_with("https://"))
        .ok_or_else(|| ApiError::bad_request("malformed link"))?;
    if !token_ok(&token) || !st.signer.verify(&token, &url, &q.s) {
        return Err(ApiError::bad_request("this link was not signed by this relay"));
    }
    let location = HeaderValue::from_str(&url).map_err(|_| ApiError::bad_request("malformed link"))?;
    if let Err(e) = tracking::record(&st.store, &token, "click", now_unix()) {
        tracing::error!("could not record a click: {e}");
    }
    Ok((StatusCode::FOUND, [(header::LOCATION, location)]).into_response())
}
