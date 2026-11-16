//! Whoosh push relay (archetype 1, hosted). Stores device tokens and opaque routing ids only.
//!
//!   relay                 serve; configuration comes from RELAY_* environment variables
//!   relay apns-stub ...   a local APNs stand-in for tests and checks (src/apns/stub.rs)
mod apns;
mod registry;
mod routes;
mod tracking;

#[cfg(feature = "adapter-gmail")]
#[path = "adapters/gmail/mod.rs"]
mod adapter_gmail;
#[cfg(feature = "adapter-graph")]
#[path = "adapters/graph/mod.rs"]
mod adapter_graph;
#[cfg(feature = "adapter-jmap")]
#[path = "adapters/jmap/mod.rs"]
mod adapter_jmap;
#[cfg(feature = "adapter-macsource")]
#[path = "adapters/macsource/mod.rs"]
mod adapter_macsource;

use std::sync::Arc;
use tracing::{error, info};

fn env(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|v| !v.trim().is_empty())
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt().with_env_filter(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into())).init();
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(String::as_str) == Some("apns-stub") {
        apns::stub::run(&args[2..]).await;
        return;
    }
    if let Err(e) = serve().await {
        error!("relay stopped: {e}");
        std::process::exit(1);
    }
}

async fn serve() -> Result<(), String> {
    let db_path = env("RELAY_DB").unwrap_or_else(|| "relay.sqlite3".into());
    let store = registry::Store::open(&db_path).map_err(|e| format!("cannot open {db_path}: {e}"))?;
    // RELAY_BIND wins; hosts that hand out a port (Fly, Render, Heroku style) set PORT instead.
    let bind = env("RELAY_BIND").or_else(|| env("PORT").map(|p| format!("0.0.0.0:{p}"))).unwrap_or_else(|| "127.0.0.1:0".into());
    let listener = tokio::net::TcpListener::bind(&bind).await.map_err(|e| format!("cannot bind {bind}: {e}"))?;
    let addr = listener.local_addr().map_err(|e| e.to_string())?;
    let apns = Arc::new(apns::ApnsState::from_env());
    if let Some(why) = apns.detail() {
        error!("{why}; the relay boots degraded and keeps jobs queued");
    }
    let dispatcher = Arc::new(registry::Dispatcher::new(store.clone()));
    let adapters = Arc::new(registry::adapter::AdapterSet::builtin(env("RELAY_SIGNAL_TOKEN")));
    adapters.start_all(&dispatcher);
    let state = routes::AppState {
        store,
        dispatcher,
        adapters,
        apns,
        signer: Arc::new(tracking::Signer::new(env("RELAY_TRACKING_KEY"))),
        public_base: Arc::new(env("RELAY_PUBLIC_BASE_URL").unwrap_or_else(|| format!("http://{addr}"))),
    };
    tokio::spawn(apns::worker::run(state.clone()));
    println!("listening on http://{addr}");
    info!("relay {} serving on {addr}, database {db_path}", env!("CARGO_PKG_VERSION"));
    axum::serve(listener, routes::router(state)).with_graceful_shutdown(shutdown()).await.map_err(|e| e.to_string())
}

async fn shutdown() {
    let _ = tokio::signal::ctrl_c().await;
}
