mod admin;
mod ai;
mod api;
mod community;
mod models;
mod network;
mod opml;
mod parser;
mod platforms;
mod webhooks;
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
    tagger: Option<Arc<ai::Tagger>>,
    discord: Option<Arc<platforms::discord::Discord>>,
    platforms: Arc<std::collections::HashMap<String, Arc<dyn platforms::Platform>>>,
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
    let discord = platforms::discord::Discord::from_env()?.map(Arc::new);
    let mut platforms: std::collections::HashMap<String, Arc<dyn platforms::Platform>> =
        std::collections::HashMap::new();
    if let Some(d) = &discord {
        platforms.insert("discord".into(), d.clone());
    }
    let secure_cookie = std::env::var("COOKIE_SECURE").unwrap_or_default() == "true"
        || discord
            .as_ref()
            .is_some_and(|d| d.base_url.starts_with("https://"));
    let state = AppState {
        tagger: ai::Tagger::from_env()?.map(Arc::new),
        discord,
        platforms: Arc::new(platforms),
        pool,
        token: Arc::new(token),
        secure_cookie,
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
        .route_layer(middleware::from_fn_with_state(state.clone(), api::auth));
    let frontend = std::env::var("FRONTEND_DIR").unwrap_or("frontend/dist".into());
    let app = Router::new()
        .nest("/api", protected)
        .nest("/api/admin", admin::routes(state.clone()))
        .route(
            "/api/{*path}",
            axum::routing::any(|| async { axum::http::StatusCode::NOT_FOUND }),
        )
        .nest("/api/community", community::routes(state.clone()))
        .route("/api/login", post(api::login))
        .route("/api/logout", post(api::logout))
        .route("/health", get(api::health))
        .route_service(
            "/community/",
            ServeFile::new(format!("{frontend}/index.html")),
        )
        .route_service(
            "/community",
            ServeFile::new(format!("{frontend}/index.html")),
        )
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
    let tag_worker = tokio::spawn(community::tag_worker::run(state.clone()));
    let community_worker = tokio::spawn(community::worker::run(state.clone()));
    let download_worker = tokio::spawn(worker::run_downloads(state.clone()));
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
    download_worker.abort();
    community_worker.abort();
    tag_worker.abort();
    Ok(())
}
