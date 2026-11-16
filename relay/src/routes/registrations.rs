//! Device registrations: an APNs device token bound to an opaque account routing id.
use super::{ApiError, ApiJson, AppState};
use crate::registry::devices::{self, Environment};
use crate::registry::now_unix;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};

#[derive(Deserialize)]
pub struct RegisterBody {
    device_token: String,
    routing_id: String,
    environment: String,
}

pub fn valid_token(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_ascii_hexdigit())
}

pub fn valid_routing_id(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

pub async fn register(State(st): State<AppState>, ApiJson(body): ApiJson<RegisterBody>) -> Result<(StatusCode, Json<Value>), ApiError> {
    if !valid_token(&body.device_token) {
        return Err(ApiError::bad_request("device_token must be the hexadecimal APNs token"));
    }
    if !valid_routing_id(&body.routing_id) {
        return Err(ApiError::bad_request("routing_id must be letters, digits, '-' or '_'"));
    }
    let env = match body.environment.as_str() {
        "sandbox" => Environment::Sandbox,
        "production" => Environment::Production,
        _ => return Err(ApiError::bad_request("environment must be 'sandbox' or 'production'")),
    };
    let token = body.device_token.to_lowercase();
    let created = devices::register(&st.store, &token, &body.routing_id, env, now_unix()).map_err(ApiError::internal)?;
    let status = if created { StatusCode::CREATED } else { StatusCode::OK };
    Ok((status, Json(json!({ "routing_id": body.routing_id, "environment": env.as_str(), "created": created }))))
}

pub async fn lookup(State(st): State<AppState>, Path(routing_id): Path<String>) -> Result<Json<Value>, ApiError> {
    if !valid_routing_id(&routing_id) {
        return Err(ApiError::bad_request("routing_id must be letters, digits, '-' or '_'"));
    }
    let rows = devices::lookup(&st.store, &routing_id).map_err(ApiError::internal)?;
    let list: Vec<Value> = rows.iter().map(|d| json!({ "environment": d.environment, "registered_at": d.registered_at })).collect();
    Ok(Json(json!({ "routing_id": routing_id, "count": list.len(), "devices": list })))
}

pub async fn unregister(State(st): State<AppState>, Path((routing_id, device_token)): Path<(String, String)>) -> Result<StatusCode, ApiError> {
    if !valid_token(&device_token) || !valid_routing_id(&routing_id) {
        return Err(ApiError::bad_request("malformed routing_id or device_token"));
    }
    let removed = devices::unregister(&st.store, &device_token.to_lowercase(), &routing_id).map_err(ApiError::internal)?;
    if removed {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(ApiError::not_found("no such registration"))
    }
}
