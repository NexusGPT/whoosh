//! Where mail signals enter: one route per provider family, decoded by that family's adapter.
use super::{ApiError, AppState};
use crate::registry::adapter::{AdapterError, InboundRequest};
use axum::body::Bytes;
use axum::extract::{Path, State};
use axum::http::{header, HeaderMap, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;

/// POST /v1/inbound/{family}: the provider adapter decodes and authenticates its own payload.
/// The adapter sees the request URI too and may answer the request itself (`reply`).
pub async fn inbound(State(st): State<AppState>, Path(family): Path<String>, uri: Uri, headers: HeaderMap, body: Bytes) -> Result<Response, ApiError> {
    let Some(adapter) = st.adapters.get(&family) else {
        return Err(ApiError::not_found(format!("no adapter for family '{family}' in this build")));
    };
    let request = InboundRequest {
        uri: uri.path_and_query().map(|p| p.as_str().to_owned()).unwrap_or_else(|| uri.path().to_owned()),
        headers: headers.iter().filter_map(|(k, v)| v.to_str().ok().map(|v| (k.as_str().to_owned(), v.to_owned()))).collect(),
        body: body.to_vec(),
    };
    if let Some(reply) = adapter.reply(&request) {
        let status = StatusCode::from_u16(reply.status).unwrap_or(StatusCode::OK);
        return Ok((status, [(header::CONTENT_TYPE, reply.content_type), (header::X_CONTENT_TYPE_OPTIONS, "nosniff")], reply.body).into_response());
    }
    let signals = adapter.decode(&request).map_err(|e| match e {
        AdapterError::Unauthorized(m) => ApiError::unauthorized(m),
        AdapterError::Malformed(m) => ApiError::bad_request(m),
    })?;
    let mut queued = 0;
    for signal in &signals {
        queued += st.dispatcher.emit(signal).map_err(ApiError::internal)?;
    }
    Ok((StatusCode::ACCEPTED, Json(json!({ "signals": signals.len(), "queued": queued }))).into_response())
}

#[cfg(test)]
mod tests {
    use crate::apns::ApnsState;
    use crate::registry::adapter::{AdapterError, AdapterSet, InboundRequest, MailSignal, ProviderAdapter};
    use crate::registry::devices::{self, Environment};
    use crate::registry::{Dispatcher, Store};
    use crate::routes::{router, AppState};
    use crate::tracking::Signer;
    use std::sync::Arc;

    /// A stand-in provider adapter: proves the trait is enough to plug a family in.
    struct Fake;

    impl ProviderAdapter for Fake {
        fn family(&self) -> &'static str {
            "fake"
        }
        fn decode(&self, request: &InboundRequest) -> Result<Vec<MailSignal>, AdapterError> {
            if !request.headers.iter().any(|(k, v)| k == "x-fake-sig" && v == "good") {
                return Err(AdapterError::Unauthorized("bad signature".into()));
            }
            let text = String::from_utf8_lossy(&request.body).to_string();
            let mut parts = text.splitn(3, ':');
            match (parts.next(), parts.next(), parts.next()) {
                (Some("ok"), Some(routing_id), Some(signal_id)) => {
                    Ok(vec![MailSignal { signal_id: signal_id.into(), routing_id: routing_id.into(), opaque_ref: None }])
                }
                _ => Err(AdapterError::Malformed("expected ok:<routing>:<signal>".into())),
            }
        }
    }

    #[tokio::test]
    async fn an_adapter_plugs_in_through_the_trait_alone() {
        let store = Store::open(":memory:").unwrap();
        devices::register(&store, "aa", "acct-1", Environment::Sandbox, 1).unwrap();
        let state = AppState {
            store: store.clone(),
            dispatcher: Arc::new(Dispatcher::new(store.clone())),
            adapters: Arc::new(AdapterSet::new(vec![Arc::new(Fake)])),
            apns: Arc::new(ApnsState::Unconfigured),
            signer: Arc::new(Signer::new(None)),
            public_base: Arc::new("http://x".into()),
        };
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        tokio::spawn(async move { axum::serve(listener, router(state)).await });
        let http = reqwest::Client::new();
        let post = |sig: &'static str, body: &'static str| http.post(format!("{base}/v1/inbound/fake")).header("x-fake-sig", sig).body(body).send();
        assert_eq!(post("bad", "ok:acct-1:s1").await.unwrap().status().as_u16(), 401);
        assert_eq!(post("good", "nonsense").await.unwrap().status().as_u16(), 400);
        let ok: serde_json::Value = post("good", "ok:acct-1:s1").await.unwrap().json().await.unwrap();
        assert_eq!(ok["queued"], 1);
        let again: serde_json::Value = post("good", "ok:acct-1:s1").await.unwrap().json().await.unwrap();
        assert_eq!(again["queued"], 0);
        let health: serde_json::Value = http.get(format!("{base}/v1/health")).send().await.unwrap().json().await.unwrap();
        assert_eq!(health["adapters"], serde_json::json!(["fake"]));
        assert_eq!(health["queue_depth"], 1);
    }
}
