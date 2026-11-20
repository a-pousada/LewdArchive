//! Community destinations share the delivery transport, never the authorization scope.
use super::{User, service};
use crate::{
    AppState,
    api::{Result, not_found},
    models::Webhook,
    webhooks::{WebhookInput, validate_url},
};
use axum::{
    Extension, Json,
    extract::{Path, State},
    http::StatusCode,
};
use serde_json::{Value, json};
use uuid::Uuid;
pub async fn list(
    State(s): State<AppState>,
    Extension(user): Extension<User>,
    Path(server): Path<Uuid>,
) -> Result<Json<Vec<Webhook>>> {
    service::authorize(&s, &user, server, true).await?;
    Ok(Json(
        sqlx::query_as("SELECT * FROM webhooks WHERE community_id=$1 ORDER BY created_at,id")
            .bind(server)
            .fetch_all(&s.pool)
            .await?,
    ))
}
pub async fn create(
    State(s): State<AppState>,
    Extension(user): Extension<User>,
    Path(server): Path<Uuid>,
    Json(i): Json<WebhookInput>,
) -> Result<(StatusCode, Json<Webhook>)> {
    service::authorize(&s, &user, server, true).await?;
    i.validate()?;
    if let Some(raw) = &i.url
        && !raw.trim().starts_with("https://")
    {
        return Err(crate::api::bad(
            "Community webhooks require a public HTTPS URL",
        ));
    }
    let url = validate_url(
        i.url
            .as_deref()
            .ok_or_else(|| crate::api::bad("Webhook URL is required"))?,
    )?;
    let item=sqlx::query_as("INSERT INTO webhooks(id,community_id,name,url,username,avatar_url,embed_color,enabled,category_filter) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9) RETURNING *").bind(Uuid::new_v4()).bind(server).bind(i.name.trim()).bind(url).bind(i.username.trim()).bind(i.avatar_url.trim()).bind(i.embed_color).bind(i.enabled).bind(i.category_filter.as_deref().map(str::trim)).fetch_one(&s.pool).await?;
    Ok((StatusCode::CREATED, Json(item)))
}
pub async fn update(
    State(s): State<AppState>,
    Extension(user): Extension<User>,
    Path((server, id)): Path<(Uuid, Uuid)>,
    Json(i): Json<WebhookInput>,
) -> Result<Json<Webhook>> {
    service::authorize(&s, &user, server, true).await?;
    i.validate()?;
    if let Some(raw) = &i.url
        && !raw.trim().starts_with("https://")
    {
        return Err(crate::api::bad(
            "Community webhooks require a public HTTPS URL",
        ));
    }
    let url = i.url.as_deref().map(validate_url).transpose()?;
    let item=sqlx::query_as("UPDATE webhooks SET name=$3,url=coalesce($4,url),username=$5,avatar_url=$6,embed_color=$7,enabled=$8,category_filter=$9 WHERE id=$1 AND community_id=$2 RETURNING *").bind(id).bind(server).bind(i.name.trim()).bind(url).bind(i.username.trim()).bind(i.avatar_url.trim()).bind(i.embed_color).bind(i.enabled).bind(i.category_filter.as_deref().map(str::trim)).fetch_optional(&s.pool).await?.ok_or_else(not_found)?;
    s.wake.notify_one();
    Ok(Json(item))
}
pub async fn delete(
    State(s): State<AppState>,
    Extension(user): Extension<User>,
    Path((server, id)): Path<(Uuid, Uuid)>,
) -> Result<Json<Value>> {
    service::authorize(&s, &user, server, true).await?;
    if sqlx::query("DELETE FROM webhooks WHERE id=$1 AND community_id=$2")
        .bind(id)
        .bind(server)
        .execute(&s.pool)
        .await?
        .rows_affected()
        == 0
    {
        return Err(not_found());
    }
    Ok(Json(json!({"ok":true})))
}
pub async fn deliveries(
    State(s): State<AppState>,
    Extension(user): Extension<User>,
    Path(server): Path<Uuid>,
) -> Result<Json<Vec<Value>>> {
    service::authorize(&s, &user, server, true).await?;
    let items=sqlx::query_scalar("SELECT jsonb_build_object('id',j.id,'title',e.title,'destination',w.name,'status',j.status,'attempts',j.attempts,'last_error',j.last_error,'created_at',j.created_at) FROM jobs j JOIN webhooks w ON w.id=j.webhook_id JOIN entries e ON e.id=j.entry_id WHERE w.community_id=$1 ORDER BY j.created_at DESC LIMIT 100").bind(server).fetch_all(&s.pool).await?;
    Ok(Json(items))
}
pub async fn retry(
    State(s): State<AppState>,
    Extension(user): Extension<User>,
    Path((server, id)): Path<(Uuid, Uuid)>,
) -> Result<Json<Value>> {
    service::authorize(&s, &user, server, true).await?;
    if sqlx::query("UPDATE jobs SET status='pending',attempts=0,next_attempt_at=now(),locked_at=NULL,last_error=NULL WHERE id=$1 AND status='failed' AND webhook_id IN(SELECT id FROM webhooks WHERE community_id=$2)").bind(id).bind(server).execute(&s.pool).await?.rows_affected()==0 {return Err(not_found());}
    s.wake.notify_one();
    Ok(Json(json!({"ok":true})))
}

/// Queue a deliberate replay for the server's enabled destinations.
pub async fn send_post(
    State(s): State<AppState>,
    Extension(user): Extension<User>,
    Path((server, post)): Path<(Uuid, Uuid)>,
) -> Result<Json<Value>> {
    service::authorize(&s, &user, server, true).await?;
    let mut tx = s.pool.begin().await?;
    let entry:Option<Uuid>=sqlx::query_scalar("SELECT e.id FROM entries e JOIN community_artists a ON a.feed_id=e.feed_id WHERE e.id=$1 AND a.community_id=$2 FOR KEY SHARE OF e,a").bind(post).bind(server).fetch_optional(&mut *tx).await?;
    if entry.is_none() {
        return Err(not_found());
    }
    let enabled:bool=sqlx::query_scalar("SELECT notifications_enabled AND (SELECT enabled FROM communities WHERE id=$1) FROM settings").bind(server).fetch_one(&mut *tx).await?;
    if !enabled {
        return Err(crate::api::bad("Notifications are paused"));
    }
    let destinations: Vec<Uuid> = sqlx::query_scalar(
        "SELECT w.id FROM webhooks w WHERE w.community_id=$1 AND w.enabled AND (w.category_filter IS NULL OR w.category_filter=(SELECT a.category FROM community_artists a JOIN entries e ON e.feed_id=a.feed_id WHERE a.community_id=$1 AND e.id=$2)) ORDER BY w.id FOR SHARE OF w",
    )
    .bind(server)
    .bind(post)
    .fetch_all(&mut *tx)
    .await?;
    if destinations.is_empty() {
        return Err(crate::api::bad(
            "No enabled webhook matches this artist category",
        ));
    }
    let mut queued = 0;
    for id in destinations {
        queued+=sqlx::query("INSERT INTO jobs(id,entry_id,kind,webhook_id) VALUES($1,$2,'discord',$3) ON CONFLICT ON CONSTRAINT jobs_delivery_key DO UPDATE SET status='pending',attempts=0,next_attempt_at=now(),last_error=NULL,locked_at=NULL WHERE jobs.status IN('failed','done')").bind(Uuid::new_v4()).bind(post).bind(id).execute(&mut *tx).await?.rows_affected();
    }
    tx.commit().await?;
    s.wake.notify_one();
    Ok(Json(json!({"queued":queued})))
}
