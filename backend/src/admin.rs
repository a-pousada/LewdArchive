//! Installation operations only. Community content is managed through server-scoped APIs.
use crate::{
    AppState,
    api::{self, Result, bad},
    models::Settings,
};
use axum::{
    Json, Router,
    extract::{Path, State},
    middleware,
    routing::{get, post},
};
use serde::Deserialize;
use serde_json::{Value, json};
use uuid::Uuid;
pub fn routes(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/overview", get(overview))
        .route("/servers", get(servers))
        .route("/services", get(services))
        .route("/queues", get(queues))
        .route("/queues/{kind}/{id}/retry", post(retry))
        .route("/settings", get(settings).put(save_settings))
        .route_layer(middleware::from_fn_with_state(state, api::auth))
}
pub async fn overview(State(s): State<AppState>) -> Result<Json<Value>> {
    let value=sqlx::query_scalar("SELECT jsonb_build_object('servers',(SELECT count(*) FROM communities),'users',(SELECT count(*) FROM community_users),'subscriptions',(SELECT count(*) FROM community_artists),'posts',(SELECT count(*) FROM entries),'comments',(SELECT count(*) FROM post_comments),'storage_bytes',(SELECT coalesce(sum(size_bytes),0) FROM downloads),'failed',(SELECT count(*) FROM jobs WHERE status='failed' AND (kind='download' OR webhook_id IN(SELECT id FROM webhooks WHERE community_id IS NOT NULL)))+(SELECT count(*) FROM community_deliveries WHERE status='failed'),'pending',(SELECT count(*) FROM jobs WHERE status IN('pending','processing') AND (kind='download' OR webhook_id IN(SELECT id FROM webhooks WHERE community_id IS NOT NULL)))+(SELECT count(*) FROM community_deliveries WHERE status IN('pending','processing')),'feed_errors',(SELECT count(*) FROM feeds f WHERE last_error IS NOT NULL AND EXISTS(SELECT 1 FROM community_artists a WHERE a.feed_id=f.id)))").fetch_one(&s.pool).await?;
    Ok(Json(value))
}
pub async fn servers(State(s): State<AppState>) -> Result<Json<Vec<Value>>> {
    Ok(Json(sqlx::query_scalar("SELECT jsonb_build_object('id',c.id,'name',c.name,'platform',c.platform,'enabled',c.enabled,'channel_name',c.channel_name,'artists',(SELECT count(*) FROM community_artists a WHERE a.community_id=c.id),'webhooks',(SELECT count(*) FROM webhooks w WHERE w.community_id=c.id),'created_at',c.created_at) FROM communities c ORDER BY lower(c.name),c.id").fetch_all(&s.pool).await?))
}
pub async fn services(State(s): State<AppState>) -> Result<Json<Value>> {
    let db = sqlx::query("SELECT 1").execute(&s.pool).await.is_ok();
    let last_poll: Option<chrono::DateTime<chrono::Utc>> =
        sqlx::query_scalar("SELECT max(last_checked_at) FROM feeds")
            .fetch_one(&s.pool)
            .await?;
    Ok(Json(
        json!({"database":if db{"connected"}else{"unavailable"},"discord":if s.discord.is_some(){"configured"}else{"not configured"},"rsshub":"configured","last_poll":last_poll,"max_download_mb":s.max_download_bytes/1024/1024}),
    ))
}
pub async fn queues(State(s): State<AppState>) -> Result<Json<Vec<Value>>> {
    Ok(Json(sqlx::query_scalar("SELECT jsonb_build_object('id',id,'kind',kind,'server',server,'status',status,'attempts',attempts,'last_error',last_error,'created_at',created_at) FROM (SELECT j.id,CASE WHEN j.kind='download' THEN 'download' ELSE 'webhook' END kind,coalesce(c.name,'Shared media') server,j.status,j.attempts,j.last_error,j.created_at FROM jobs j LEFT JOIN webhooks w ON w.id=j.webhook_id LEFT JOIN communities c ON c.id=w.community_id WHERE j.kind='download' OR w.community_id IS NOT NULL UNION ALL SELECT d.id,'bot',c.name,d.status,d.attempts,d.last_error,d.created_at FROM community_deliveries d JOIN communities c ON c.id=d.community_id) q ORDER BY created_at DESC,id LIMIT 100").fetch_all(&s.pool).await?))
}
pub async fn retry(
    State(s): State<AppState>,
    Path((kind, id)): Path<(String, Uuid)>,
) -> Result<Json<Value>> {
    let changed=match kind.as_str(){
 "bot"=>sqlx::query("UPDATE community_deliveries SET status='pending',attempts=0,next_attempt_at=now(),locked_at=NULL,last_error=NULL WHERE id=$1 AND status='failed'").bind(id).execute(&s.pool).await?.rows_affected(),
 "webhook"|"download"=>sqlx::query("UPDATE jobs SET status='pending',attempts=0,next_attempt_at=now(),locked_at=NULL,last_error=NULL WHERE id=$1 AND status='failed' AND ((kind='download' AND $2='download') OR (kind='discord' AND $2='webhook' AND webhook_id IN(SELECT id FROM webhooks WHERE community_id IS NOT NULL)))").bind(id).bind(kind).execute(&s.pool).await?.rows_affected(),
 _=>return Err(bad("Unknown queue"))};
    if changed == 0 {
        return Err(bad("Only failed jobs can be retried"));
    }
    s.wake.notify_one();
    Ok(Json(json!({"ok":true})))
}
pub async fn settings(State(s): State<AppState>) -> Result<Json<Settings>> {
    Ok(Json(
        sqlx::query_as("SELECT downloads_enabled,notifications_enabled FROM settings")
            .fetch_one(&s.pool)
            .await?,
    ))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Input {
    downloads_enabled: bool,
    notifications_enabled: bool,
}
pub async fn save_settings(State(s): State<AppState>, Json(i): Json<Input>) -> Result<Json<Value>> {
    sqlx::query("UPDATE settings SET downloads_enabled=$1,notifications_enabled=$2")
        .bind(i.downloads_enabled)
        .bind(i.notifications_enabled)
        .execute(&s.pool)
        .await?;
    s.wake.notify_one();
    Ok(Json(json!({"ok":true})))
}
