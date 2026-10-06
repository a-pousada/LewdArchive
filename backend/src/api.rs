use crate::{
    AppState,
    models::{Entry, Feed, Settings},
    network,
};
use axum::{
    Json,
    extract::{Path, Query, Request, State},
    http::{HeaderMap, StatusCode, header},
    middleware::Next,
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::Row;
use subtle::ConstantTimeEq;
use uuid::Uuid;

pub struct ApiError(pub StatusCode, pub String);
impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.0, Json(json!({"error":self.1}))).into_response()
    }
}
impl From<sqlx::Error> for ApiError {
    fn from(e: sqlx::Error) -> Self {
        if let sqlx::Error::Database(db) = &e
            && db.is_unique_violation()
        {
            return Self(
                StatusCode::CONFLICT,
                "This feed has already been added".into(),
            );
        }
        tracing::error!(error = %e, "database request failed");
        Self(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Unable to complete the operation".into(),
        )
    }
}
type Result<T> = std::result::Result<T, ApiError>;
pub async fn feed_options(State(state): State<AppState>) -> Json<Value> {
    Json(json!({"rsshub_base_url": state.rsshub_base_url.as_str()}))
}
fn bad(s: impl Into<String>) -> ApiError {
    ApiError(StatusCode::BAD_REQUEST, s.into())
}
fn not_found() -> ApiError {
    ApiError(StatusCode::NOT_FOUND, "Record not found".into())
}
fn matches_token(a: &str, b: &str) -> bool {
    a.as_bytes().ct_eq(b.as_bytes()).into()
}
pub async fn auth(State(state): State<AppState>, req: Request, next: Next) -> Response {
    let bearer = req
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "));
    let cookie = req
        .headers()
        .get(header::COOKIE)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| {
            s.split(';')
                .find_map(|s| s.trim().strip_prefix("la_session="))
        });
    if !bearer
        .or(cookie)
        .is_some_and(|v| matches_token(v, &state.token))
    {
        return ApiError(
            StatusCode::UNAUTHORIZED,
            "Sign in to access your archive".into(),
        )
        .into_response();
    }
    next.run(req).await
}
#[derive(Deserialize)]
pub struct Login {
    token: String,
}
pub async fn login(
    State(state): State<AppState>,
    Json(input): Json<Login>,
) -> Result<impl IntoResponse> {
    if !matches_token(&input.token, &state.token) {
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        return Err(ApiError(
            StatusCode::UNAUTHORIZED,
            "Incorrect access key".into(),
        ));
    }
    let secure = if state.secure_cookie { "; Secure" } else { "" };
    let cookie = format!(
        "la_session={}; Path=/; HttpOnly; SameSite=Strict; Max-Age=604800{secure}",
        state.token
    );
    Ok(([(header::SET_COOKIE, cookie)], Json(json!({"ok":true}))))
}
pub async fn logout() -> impl IntoResponse {
    (
        [(
            header::SET_COOKIE,
            "la_session=; Path=/; HttpOnly; SameSite=Strict; Max-Age=0",
        )],
        Json(json!({"ok":true})),
    )
}
pub async fn health(State(s): State<AppState>) -> impl IntoResponse {
    match sqlx::query("SELECT 1").execute(&s.pool).await {
        Ok(_) => (StatusCode::OK, Json(json!({"status":"ok"}))),
        Err(_) => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({"status":"unavailable"})),
        ),
    }
}
pub async fn feeds(State(s): State<AppState>) -> Result<Json<Vec<Feed>>> {
    Ok(Json(
        sqlx::query_as("SELECT * FROM feeds ORDER BY created_at DESC")
            .fetch_all(&s.pool)
            .await?,
    ))
}
#[derive(Deserialize)]
pub struct FeedInput {
    name: String,
    url: String,
    source: String,
    enabled: bool,
    auto_download: bool,
    notify: bool,
    poll_minutes: i32,
}
impl FeedInput {
    fn validate(&self) -> Result<()> {
        if self.name.trim().is_empty() || self.name.chars().count() > 120 {
            return Err(bad("Name must be between 1 and 120 characters"));
        }
        if !(5..=1440).contains(&self.poll_minutes) {
            return Err(bad("Interval must be between 5 and 1440 minutes"));
        }
        if !["rss", "pixiv", "bluesky", "xcancel", "pawchive"].contains(&self.source.as_str()) {
            return Err(bad("Invalid source"));
        }
        if self.url.len() > 4096 {
            return Err(bad("URL is too long"));
        }
        network::web_url(self.url.trim()).map_err(|e| bad(e.to_string()))?;
        Ok(())
    }
}
pub async fn add_feed(
    State(s): State<AppState>,
    Json(i): Json<FeedInput>,
) -> Result<impl IntoResponse> {
    i.validate()?;
    let feed: Feed = sqlx::query_as("INSERT INTO feeds (id,name,url,source,enabled,auto_download,notify,poll_minutes) VALUES ($1,$2,$3,$4,$5,$6,$7,$8) RETURNING *")
        .bind(Uuid::new_v4()).bind(i.name.trim()).bind(i.url.trim()).bind(i.source).bind(i.enabled).bind(i.auto_download).bind(i.notify).bind(i.poll_minutes)
        .fetch_one(&s.pool).await?;
    s.wake.notify_one();
    Ok((StatusCode::CREATED, Json(feed)))
}
pub async fn edit_feed(
    State(s): State<AppState>,
    Path(id): Path<Uuid>,
    Json(i): Json<FeedInput>,
) -> Result<Json<Feed>> {
    i.validate()?;
    let feed = sqlx::query_as("UPDATE feeds SET name=$2, source=$3, enabled=$4, auto_download=$5, notify=$6, poll_minutes=$7 WHERE id=$1 AND url=$8 RETURNING *")
        .bind(id).bind(i.name.trim()).bind(i.source).bind(i.enabled).bind(i.auto_download).bind(i.notify).bind(i.poll_minutes).bind(i.url.trim())
        .fetch_optional(&s.pool).await?.ok_or_else(||bad("Feed not found or URL changed. Create a new feed for a different URL."))?;
    Ok(Json(feed))
}
pub async fn delete_feed(State(s): State<AppState>, Path(id): Path<Uuid>) -> Result<Json<Value>> {
    if sqlx::query("DELETE FROM feeds WHERE id=$1")
        .bind(id)
        .execute(&s.pool)
        .await?
        .rows_affected()
        == 0
    {
        return Err(not_found());
    }
    Ok(Json(json!({"ok":true})))
}
pub async fn sync(State(s): State<AppState>, Path(id): Path<Uuid>) -> Result<Json<Value>> {
    if sqlx::query("UPDATE feeds SET next_check_at=now() WHERE id=$1 AND enabled")
        .bind(id)
        .execute(&s.pool)
        .await?
        .rows_affected()
        == 0
    {
        return Err(bad("Enable the feed before syncing"));
    }
    s.wake.notify_one();
    Ok(Json(json!({"ok":true})))
}
pub async fn sync_all(State(s): State<AppState>) -> Result<Json<Value>> {
    sqlx::query("UPDATE feeds SET next_check_at=now() WHERE enabled")
        .execute(&s.pool)
        .await?;
    s.wake.notify_one();
    Ok(Json(json!({"ok":true})))
}
#[derive(Deserialize, Default)]
pub struct EntryQuery {
    q: Option<String>,
    feed_id: Option<Uuid>,
    source: Option<String>,
    unread: Option<bool>,
    favorite: Option<bool>,
    page: Option<i64>,
}
const ENTRY_SELECT: &str = "SELECT e.id,e.feed_id,e.title,e.author,e.url,e.summary,e.media,e.published_at,e.is_read,e.favorite,f.name AS feed_name,f.source FROM entries e JOIN feeds f ON f.id=e.feed_id";
pub async fn entries(
    State(s): State<AppState>,
    Query(q): Query<EntryQuery>,
) -> Result<Json<Value>> {
    let page = q.page.unwrap_or(1).clamp(1, 100000);
    let filter = " WHERE ($1::text IS NULL OR e.title ILIKE $1 OR e.author ILIKE $1) AND ($2::uuid IS NULL OR e.feed_id=$2) AND ($3::text IS NULL OR f.source=$3) AND (NOT $4 OR NOT e.is_read) AND (NOT $5 OR e.favorite)";
    let search =
        q.q.filter(|v| !v.is_empty())
            .map(|v| format!("%{}%", v.chars().take(200).collect::<String>()));
    let data: Vec<Entry> = sqlx::query_as(&format!(
        "{ENTRY_SELECT}{filter} ORDER BY e.published_at DESC,e.id DESC LIMIT 24 OFFSET $6"
    ))
    .bind(&search)
    .bind(q.feed_id)
    .bind(&q.source)
    .bind(q.unread.unwrap_or(false))
    .bind(q.favorite.unwrap_or(false))
    .bind((page - 1) * 24)
    .fetch_all(&s.pool)
    .await?;
    let total: i64 = sqlx::query_scalar(&format!(
        "SELECT count(*) FROM entries e JOIN feeds f ON f.id=e.feed_id{filter}"
    ))
    .bind(&search)
    .bind(q.feed_id)
    .bind(&q.source)
    .bind(q.unread.unwrap_or(false))
    .bind(q.favorite.unwrap_or(false))
    .fetch_one(&s.pool)
    .await?;
    Ok(Json(
        json!({"items":data,"total":total,"page":page,"pages":(total+23)/24}),
    ))
}
#[derive(Deserialize)]
pub struct EntryPatch {
    is_read: Option<bool>,
    favorite: Option<bool>,
}
pub async fn patch_entry(
    State(s): State<AppState>,
    Path(id): Path<Uuid>,
    Json(i): Json<EntryPatch>,
) -> Result<Json<Value>> {
    if sqlx::query("UPDATE entries SET is_read=coalesce($2,is_read),favorite=coalesce($3,favorite) WHERE id=$1")
        .bind(id).bind(i.is_read).bind(i.favorite).execute(&s.pool).await?.rows_affected() == 0 {return Err(not_found());}
    Ok(Json(json!({"ok":true})))
}
pub async fn read_all(State(s): State<AppState>) -> Result<Json<Value>> {
    sqlx::query("UPDATE entries SET is_read=true WHERE NOT is_read")
        .execute(&s.pool)
        .await?;
    Ok(Json(json!({"ok":true})))
}
pub async fn download_entry(
    State(s): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>> {
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM entries WHERE id=$1 AND jsonb_array_length(media)>0)",
    )
    .bind(id)
    .fetch_one(&s.pool)
    .await?;
    if !exists {
        return Err(bad("This post has no downloadable media"));
    }
    sqlx::query("INSERT INTO jobs(id,entry_id,kind) VALUES($1,$2,'download') ON CONFLICT(entry_id,kind) DO UPDATE SET status='pending',attempts=0,next_attempt_at=now(),last_error=NULL WHERE jobs.status IN ('failed','done')")
        .bind(Uuid::new_v4()).bind(id).execute(&s.pool).await?;
    s.wake.notify_one();
    Ok(Json(json!({"ok":true})))
}
pub async fn send_entry_discord(
    State(s): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse> {
    let mut tx = s.pool.begin().await?;
    // Hold the entry against deletion until its outbox job is committed.
    let entry: Option<Uuid> =
        sqlx::query_scalar("SELECT id FROM entries WHERE id=$1 FOR KEY SHARE")
            .bind(id)
            .fetch_optional(&mut *tx)
            .await?;
    if entry.is_none() {
        return Err(not_found());
    }
    let settings: Settings = sqlx::query_as(
        "SELECT webhook_url,downloads_enabled,notifications_enabled FROM settings FOR SHARE",
    )
    .fetch_one(&mut *tx)
    .await?;
    if settings.webhook_url.is_empty() {
        return Err(bad(
            "Configure the Discord webhook in Settings before sending a test.",
        ));
    }
    if !settings.notifications_enabled {
        return Err(bad(
            "Discord deliveries are paused. Enable notifications in Settings.",
        ));
    }
    let job_id: Option<Uuid> = sqlx::query_scalar(
        "INSERT INTO jobs(id,entry_id,kind) VALUES($1,$2,'discord')
         ON CONFLICT(entry_id,kind) DO UPDATE SET status='pending',attempts=0,
         next_attempt_at=now(),locked_at=NULL,last_error=NULL,updated_at=now()
         WHERE jobs.status IN ('done','failed') RETURNING id",
    )
    .bind(Uuid::new_v4())
    .bind(id)
    .fetch_optional(&mut *tx)
    .await?;
    let Some(job_id) = job_id else {
        return Err(ApiError(
            StatusCode::CONFLICT,
            "This post already has a pending or processing delivery. Check Activity for its status."
                .into(),
        ));
    };
    tx.commit().await?;
    s.wake.notify_one();
    Ok((
        StatusCode::ACCEPTED,
        Json(json!({"ok":true,"job_id":job_id})),
    ))
}
pub async fn stats(State(s): State<AppState>) -> Result<Json<Value>> {
    let row = sqlx::query("SELECT (SELECT count(*) FROM feeds) feeds,(SELECT count(*) FROM feeds WHERE enabled) active_feeds,(SELECT count(*) FROM entries) entries,(SELECT count(*) FROM entries WHERE NOT is_read) unread,(SELECT count(*) FROM entries WHERE favorite) favorites,(SELECT count(*) FROM downloads) downloads,(SELECT coalesce(sum(size_bytes),0)::bigint FROM downloads) storage_bytes,(SELECT count(*) FROM jobs WHERE status='failed') failed_jobs,(SELECT count(*) FROM jobs WHERE status IN ('pending','processing')) pending_jobs")
        .fetch_one(&s.pool).await?;
    let mut data = serde_json::Map::new();
    for key in [
        "feeds",
        "active_feeds",
        "entries",
        "unread",
        "favorites",
        "downloads",
        "storage_bytes",
        "failed_jobs",
        "pending_jobs",
    ] {
        data.insert(key.into(), json!(row.get::<i64, _>(key)));
    }
    Ok(Json(Value::Object(data)))
}
pub async fn settings(State(s): State<AppState>) -> Result<Json<Value>> {
    let settings: Settings = sqlx::query_as(
        "SELECT webhook_url, downloads_enabled, notifications_enabled FROM settings",
    )
    .fetch_one(&s.pool)
    .await?;
    Ok(Json(
        json!({"webhook_configured":!settings.webhook_url.is_empty(),"downloads_enabled":settings.downloads_enabled,"notifications_enabled":settings.notifications_enabled}),
    ))
}
#[derive(Deserialize)]
pub struct SettingsInput {
    webhook_url: Option<String>,
    downloads_enabled: bool,
    notifications_enabled: bool,
}
pub async fn save_settings(
    State(s): State<AppState>,
    Json(i): Json<SettingsInput>,
) -> Result<Json<Value>> {
    if let Some(url) = &i.webhook_url
        && !url.is_empty()
    {
        network::webhook(url).map_err(|e| bad(e.to_string()))?;
    }
    sqlx::query("UPDATE settings SET webhook_url=coalesce($1,webhook_url), downloads_enabled=$2, notifications_enabled=$3")
        .bind(i.webhook_url).bind(i.downloads_enabled).bind(i.notifications_enabled).execute(&s.pool).await?;
    s.wake.notify_one();
    Ok(Json(json!({"ok":true})))
}
pub async fn jobs(State(s): State<AppState>) -> Result<Json<Value>> {
    let rows: Vec<Value> = sqlx::query_scalar("SELECT jsonb_build_object('id',j.id,'entry_id',j.entry_id,'kind',j.kind,'status',j.status,'attempts',j.attempts,'last_error',j.last_error,'created_at',j.created_at,'title',e.title,'feed_name',f.name) FROM jobs j JOIN entries e ON e.id=j.entry_id JOIN feeds f ON f.id=e.feed_id ORDER BY j.created_at DESC LIMIT 100")
        .fetch_all(&s.pool).await?;
    Ok(Json(json!(rows)))
}
pub async fn retry_job(State(s): State<AppState>, Path(id): Path<Uuid>) -> Result<Json<Value>> {
    if sqlx::query("UPDATE jobs SET status='pending',attempts=0,next_attempt_at=now(),last_error=NULL,updated_at=now() WHERE id=$1 AND status='failed'")
        .bind(id).execute(&s.pool).await?.rows_affected() == 0 {return Err(bad("Only failed jobs can be retried"));}
    s.wake.notify_one();
    Ok(Json(json!({"ok":true})))
}
pub async fn downloads(State(s): State<AppState>) -> Result<Json<Value>> {
    let rows: Vec<Value> = sqlx::query_scalar("SELECT jsonb_build_object('id',d.id,'entry_id',d.entry_id,'filename',d.filename,'size_bytes',d.size_bytes,'content_type',d.content_type,'created_at',d.created_at,'title',e.title,'author',e.author) FROM downloads d JOIN entries e ON e.id=d.entry_id ORDER BY d.created_at DESC LIMIT 200")
        .fetch_all(&s.pool).await?;
    Ok(Json(json!(rows)))
}
pub async fn file(State(s): State<AppState>, Path(id): Path<Uuid>) -> Result<Response> {
    let row = sqlx::query("SELECT filename,content_type FROM downloads WHERE id=$1")
        .bind(id)
        .fetch_optional(&s.pool)
        .await?
        .ok_or_else(not_found)?;
    let filename: String = row.get("filename");
    let file = tokio::fs::File::open(s.download_dir.join(&filename))
        .await
        .map_err(|_| not_found())?;
    let stream = futures_util::stream::try_unfold(file, |mut f| async move {
        use tokio::io::AsyncReadExt;
        let mut buf = vec![0; 65536];
        let n = f.read(&mut buf).await?;
        if n == 0 {
            Ok::<_, std::io::Error>(None)
        } else {
            buf.truncate(n);
            Ok(Some((buf, f)))
        }
    });
    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        "application/octet-stream".parse().unwrap(),
    );
    headers.insert(
        header::CONTENT_DISPOSITION,
        format!("attachment; filename=\"{filename}\"")
            .parse()
            .unwrap(),
    );
    Ok((headers, axum::body::Body::from_stream(stream)).into_response())
}
