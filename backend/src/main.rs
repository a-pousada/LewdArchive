mod api;
mod models;
mod network;
mod parser;
mod worker;
use axum::{
    Router, middleware,
    routing::{get, post},
};
use sqlx::postgres::PgPoolOptions;
use std::{path::PathBuf, sync::Arc, time::Duration};
use tower_http::{
    services::{ServeDir, ServeFile},
    trace::TraceLayer,
};

#[derive(Clone)]
pub struct AppState {
    pool: sqlx::PgPool,
    token: Arc<String>,
    secure_cookie: bool,
    download_dir: Arc<PathBuf>,
    max_download_bytes: u64,
    wake: Arc<tokio::sync::Notify>,
    rsshub_base_url: Arc<String>,
}
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "lewdarchive=info,tower_http=info".into()),
        )
        .init();
    let token = std::env::var("API_TOKEN")
        .expect("API_TOKEN is required (at least 32 alphanumeric characters)");
    anyhow::ensure!(
        token.len() >= 32
            && token
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_'),
        "API_TOKEN must contain at least 32 alphanumeric characters, hyphens, or underscores"
    );
    let rsshub_base_url = std::env::var("RSSHUB_BASE_URL")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "https://rsshub.app".into());
    let rsshub_url = network::web_url(rsshub_base_url.trim())?;
    anyhow::ensure!(
        rsshub_url.query().is_none() && rsshub_url.fragment().is_none(),
        "RSSHUB_BASE_URL must be the instance base URL without a query or fragment"
    );
    let pool = PgPoolOptions::new()
        .max_connections(10)
        .acquire_timeout(Duration::from_secs(10))
        .connect(&std::env::var("DATABASE_URL").expect("DATABASE_URL is required"))
        .await?;
    sqlx::migrate!("./migrations").run(&pool).await?;
    let download_dir =
        PathBuf::from(std::env::var("DOWNLOAD_DIR").unwrap_or("./data/downloads".into()));
    tokio::fs::create_dir_all(&download_dir).await?;
    let state = AppState {
        pool,
        token: Arc::new(token),
        secure_cookie: std::env::var("COOKIE_SECURE").unwrap_or_default() == "true",
        download_dir: Arc::new(download_dir),
        max_download_bytes: std::env::var("MAX_DOWNLOAD_MB")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(100)
            .clamp(1, 2048)
            * 1024
            * 1024,
        wake: Arc::new(tokio::sync::Notify::new()),
        rsshub_base_url: Arc::new(rsshub_url.as_str().trim_end_matches('/').to_owned()),
    };
    let protected = Router::new()
        .route(
            "/session",
            get(|| async { axum::Json(serde_json::json!({"ok":true})) }),
        )
        .route("/feeds", get(api::feeds).post(api::add_feed))
        .route("/feed-options", get(api::feed_options))
        .route(
            "/feeds/{id}",
            axum::routing::put(api::edit_feed).delete(api::delete_feed),
        )
        .route("/feeds/{id}/sync", post(api::sync))
        .route("/sync", post(api::sync_all))
        .route("/entries", get(api::entries))
        .route("/entries/read-all", post(api::read_all))
        .route("/entries/{id}", axum::routing::patch(api::patch_entry))
        .route("/entries/{id}/download", post(api::download_entry))
        .route("/entries/{id}/discord", post(api::send_entry_discord))
        .route("/stats", get(api::stats))
        .route("/settings", get(api::settings).put(api::save_settings))
        .route("/jobs", get(api::jobs))
        .route("/jobs/{id}/retry", post(api::retry_job))
        .route("/downloads", get(api::downloads))
        .route("/downloads/{id}/file", get(api::file))
        .route_layer(middleware::from_fn_with_state(state.clone(), api::auth));
    let frontend = std::env::var("FRONTEND_DIR").unwrap_or("frontend/dist".into());
    let app = Router::new()
        .nest("/api", protected)
        .route("/api/login", post(api::login))
        .route("/api/logout", post(api::logout))
        .route("/health", get(api::health))
        .fallback_service(
            ServeDir::new(&frontend)
                .not_found_service(ServeFile::new(format!("{frontend}/index.html"))),
        )
        .layer(axum::extract::DefaultBodyLimit::max(65536))
        .layer(middleware::from_fn(
            |req: axum::extract::Request, next: middleware::Next| async move {
                let mut res = next.run(req).await;
                for (k, v) in [
                    ("x-content-type-options", "nosniff"),
                    ("x-frame-options", "DENY"),
                    ("referrer-policy", "no-referrer"),
                    ("cache-control", "no-store"),
                ] {
                    res.headers_mut().insert(k, v.parse().unwrap());
                }
                res
            },
        ))
        .layer(TraceLayer::new_for_http())
        .with_state(state.clone());
    let worker = tokio::spawn(worker::run(state));
    let address = std::env::var("BIND_ADDRESS").unwrap_or("0.0.0.0:8080".into());
    let listener = tokio::net::TcpListener::bind(&address).await?;
    tracing::info!(%address,"LewdArchive ready");
    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await?;
    worker.abort();
    Ok(())
}
