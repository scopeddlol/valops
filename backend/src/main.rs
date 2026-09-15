//! valops — a self-hosted stats desk for a Valorant five-stack.
//!
//! One binary: it owns the SQLite database, serves the JSON API under `/api`
//! and serves the built single-page app for everything else.

mod analytics;
mod api;
mod builder;
mod catalog;
mod db;
mod error;
mod models;
mod seed;

use std::net::SocketAddr;
use std::path::PathBuf;

use axum::Router;
use tower_http::compression::CompressionLayer;
use tower_http::cors::CorsLayer;
use tower_http::services::{ServeDir, ServeFile};
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

fn env_or(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}

fn env_flag(key: &str, default: bool) -> bool {
    match std::env::var(key) {
        Ok(v) => !matches!(
            v.trim().to_ascii_lowercase().as_str(),
            "0" | "false" | "no" | "off" | ""
        ),
        Err(_) => default,
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| "valops=info,tower_http=warn".into()))
        .with(tracing_subscriber::fmt::layer().compact())
        .init();

    let database_url = env_or("VALOPS_DATABASE_URL", "sqlite://data/valops.db");
    let static_dir = PathBuf::from(env_or("VALOPS_STATIC_DIR", "static"));
    let port: u16 = env_or("PORT", "8080").parse().unwrap_or(8080);

    let pool = db::connect(&database_url).await?;
    tracing::info!(%database_url, "database ready");

    // A fresh install with no data is a dead-looking app, so seed the demo
    // history unless the operator opted out.
    if env_flag("VALOPS_SEED_DEMO", true) && seed::is_empty(&pool).await? {
        let sessions = env_or("VALOPS_SEED_SESSIONS", "45").parse().unwrap_or(45);
        let (players, matches) = seed::generate(&pool, sessions).await?;
        tracing::info!(players, matches, "seeded demo history");
    }

    let index = static_dir.join("index.html");
    // Unknown paths fall through to index.html so client-side routes deep-link.
    let spa = ServeDir::new(&static_dir).fallback(ServeFile::new(index));

    let app = Router::new()
        .nest("/api", api::router(api::AppState { pool }))
        .fallback_service(spa)
        .layer(CompressionLayer::new())
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http());

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!("valops listening on http://{addr}");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c().await.ok();
    };
    #[cfg(unix)]
    let terminate = async {
        if let Ok(mut sig) =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        {
            sig.recv().await;
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
    tracing::info!("shutting down");
}
