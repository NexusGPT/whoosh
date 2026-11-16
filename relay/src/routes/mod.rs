//! HTTP surface. One file per resource; handlers stay thin and call registry/, apns/, tracking/.
mod health;
mod registrations;
mod signals;
mod tracking_routes;

use crate::apns::ApnsState;
use crate::registry::adapter::AdapterSet;
use crate::registry::{Dispatcher, Store};
use crate::tracking::Signer;
use axum::extract::{FromRequest, Request};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use serde::de::DeserializeOwned;
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub store: Store,
    pub dispatcher: Arc<Dispatcher>,
    pub adapters: Arc<AdapterSet>,
    pub apns: Arc<ApnsState>,
    pub signer: Arc<Signer>,
    pub public_base: Arc<String>,
}

pub use registrations::valid_routing_id;

pub const ROUTES: &[&str] = &[
    "GET /v1/health",
    "GET /",
    "POST /v1/registrations",
    "GET /v1/registrations/{routing_id}",
    "DELETE /v1/registrations/{routing_id}/{device_token}",
    "POST /v1/inbound/{family}",
    "POST /v1/track/messages",
    "GET /v1/track/messages/{token}",
    "POST /v1/track/links",
    "GET /t/o/{token}.gif",
    "GET /t/c/{token}?u=&s=",
];

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/v1/health", get(health::get))
        // Hosts probe the root path (Render, Fly, Nexus): same 200 and body as /v1/health.
        .route("/", get(health::get))
        .route("/v1/registrations", post(registrations::register))
        .route("/v1/registrations/{routing_id}", get(registrations::lookup))
        .route("/v1/registrations/{routing_id}/{device_token}", delete(registrations::unregister))
        .route("/v1/inbound/{family}", post(signals::inbound))
        .route("/v1/track/messages", post(tracking_routes::create_message))
        .route("/v1/track/messages/{token}", get(tracking_routes::summary))
        .route("/v1/track/links", post(tracking_routes::sign_link))
        .route("/t/o/{file}", get(tracking_routes::pixel))
        .route("/t/c/{token}", get(tracking_routes::click))
        .with_state(state)
}

/// One error vocabulary for every route: {"error": {"code", "message"}}.
pub struct ApiError {
    status: StatusCode,
    code: &'static str,
    message: String,
}

impl ApiError {
    pub fn bad_request(message: impl Into<String>) -> ApiError {
        ApiError { status: StatusCode::BAD_REQUEST, code: "bad_request", message: message.into() }
    }
    pub fn unauthorized(message: impl Into<String>) -> ApiError {
        ApiError { status: StatusCode::UNAUTHORIZED, code: "unauthorized", message: message.into() }
    }
    pub fn not_found(message: impl Into<String>) -> ApiError {
        ApiError { status: StatusCode::NOT_FOUND, code: "not_found", message: message.into() }
    }
    /// Logs the real cause; the caller sees a stable message.
    pub fn internal(cause: impl std::fmt::Display) -> ApiError {
        tracing::error!("internal error: {cause}");
        ApiError { status: StatusCode::INTERNAL_SERVER_ERROR, code: "internal", message: "the relay could not complete this request".into() }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.status, Json(serde_json::json!({ "error": { "code": self.code, "message": self.message } }))).into_response()
    }
}

/// JSON body extractor whose rejection uses the same error vocabulary.
pub struct ApiJson<T>(pub T);

impl<S: Send + Sync, T: DeserializeOwned> FromRequest<S> for ApiJson<T> {
    type Rejection = ApiError;

    async fn from_request(req: Request, state: &S) -> Result<Self, ApiError> {
        match Json::<T>::from_request(req, state).await {
            Ok(Json(v)) => Ok(ApiJson(v)),
            Err(rejection) => Err(ApiError::bad_request(rejection.body_text())),
        }
    }
}
