//! Self-describing: version, status, the live queue depth and the routes this build serves.
use super::{ApiError, AppState, ROUTES};
use crate::registry::{devices, jobs};
use axum::extract::State;
use axum::Json;
use serde_json::{json, Value};

pub async fn get(State(st): State<AppState>) -> Result<Json<Value>, ApiError> {
    let counts = jobs::counts(&st.store).map_err(ApiError::internal)?;
    let of = |k: &str| counts.get(k).copied().unwrap_or(0);
    let registrations = devices::count(&st.store).map_err(ApiError::internal)?;
    let mut reasons: Vec<String> = Vec::new();
    match st.apns.as_ref() {
        crate::apns::ApnsState::Ready(_) => {}
        crate::apns::ApnsState::Unconfigured => reasons.push("apns is not configured: jobs wait in the queue".into()),
        crate::apns::ApnsState::Broken(why) => reasons.push(format!("apns is broken: {why}")),
    }
    Ok(Json(json!({
        "service": "whoosh-relay",
        "version": env!("CARGO_PKG_VERSION"),
        "status": if reasons.is_empty() { "ok" } else { "degraded" },
        "reasons": reasons,
        "queue_depth": of("queued"),
        "jobs": { "queued": of("queued"), "sent": of("sent"), "rejected": of("rejected"), "failed": of("failed") },
        "registrations": registrations,
        "apns": { "state": st.apns.label(), "detail": st.apns.detail() },
        "adapters": st.adapters.families(),
        "tracking": { "key": if st.signer.is_ephemeral() { "ephemeral" } else { "configured" } },
        "routes": ROUTES,
    })))
}
