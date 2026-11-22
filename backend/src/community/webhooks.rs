//! Community destinations share the delivery transport, never the authorization scope.
use super::{User, service};
use crate::{
    AppState,
    api::{ApiError, Result, bad, not_found},
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
    let filters = i.validated_filters()?;
    if let Some(raw) = &i.url
        && !raw.trim().starts_with("https://")
    {
        return Err(bad("Community webhooks require a public HTTPS URL"));
    }
    let url = validate_url(
        i.url
            .as_deref()
            .ok_or_else(|| bad("Webhook URL is required"))?,
    )?;
    let item = sqlx::query_as("INSERT INTO webhooks(id,community_id,name,url,username,avatar_url,embed_color,enabled,category_filter,category_filters,include_tags,exclude_tags) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12) RETURNING *")
        .bind(Uuid::new_v4())
        .bind(server)
        .bind(i.name.trim())
        .bind(url)
        .bind(i.username.trim())
        .bind(i.avatar_url.trim())
        .bind(i.embed_color)
        .bind(i.enabled)
        .bind(filters.category_filter)
        .bind(filters.category_filters)
        .bind(filters.include_tags)
        .bind(filters.exclude_tags)
        .fetch_one(&s.pool)
        .await?;
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
    let filters = i.validated_filters()?;
    if let Some(raw) = &i.url
        && !raw.trim().starts_with("https://")
    {
        return Err(bad("Community webhooks require a public HTTPS URL"));
    }
    let url = i.url.as_deref().map(validate_url).transpose()?;
    let item = sqlx::query_as("UPDATE webhooks SET name=$3,url=coalesce($4,url),username=$5,avatar_url=$6,embed_color=$7,enabled=$8,category_filter=$9,category_filters=$10,include_tags=$11,exclude_tags=$12 WHERE id=$1 AND community_id=$2 RETURNING *")
        .bind(id)
        .bind(server)
        .bind(i.name.trim())
        .bind(url)
        .bind(i.username.trim())
        .bind(i.avatar_url.trim())
        .bind(i.embed_color)
        .bind(i.enabled)
        .bind(filters.category_filter)
        .bind(filters.category_filters)
        .bind(filters.include_tags)
        .bind(filters.exclude_tags)
        .fetch_optional(&s.pool)
        .await?
        .ok_or_else(not_found)?;
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
        "SELECT w.id FROM webhooks w JOIN community_artists a ON a.community_id=w.community_id JOIN entries e ON e.feed_id=a.feed_id LEFT JOIN community_post_tags t ON t.community_id=w.community_id AND t.entry_id=e.id WHERE w.community_id=$1 AND e.id=$2 AND w.enabled AND ((w.category_filters IS NULL AND w.category_filter IS NULL) OR a.category = ANY(coalesce(w.category_filters, ARRAY[w.category_filter]))) AND (cardinality(w.include_tags)=0 OR coalesce(t.tags,'{}'::text[]) && w.include_tags) AND (cardinality(w.exclude_tags)=0 OR NOT (coalesce(t.tags,'{}'::text[]) && w.exclude_tags)) ORDER BY w.id FOR SHARE OF w",
    )
    .bind(server)
    .bind(post)
    .fetch_all(&mut *tx)
    .await?;
    if destinations.is_empty() {
        return Err(bad(
            "No enabled webhook matches this post's category and tags",
        ));
    }
    let mut queued = 0;
    for id in destinations {
        queued+=sqlx::query("INSERT INTO jobs(id,entry_id,kind,webhook_id) VALUES($1,$2,'discord',$3) ON CONFLICT ON CONSTRAINT jobs_delivery_key DO UPDATE SET status='pending',attempts=0,next_attempt_at=now(),last_error=NULL,locked_at=NULL WHERE jobs.status IN('failed','done','skipped')").bind(Uuid::new_v4()).bind(post).bind(id).execute(&mut *tx).await?.rows_affected();
    }
    tx.commit().await?;
    s.wake.notify_one();
    Ok(Json(json!({"queued":queued})))
}

/// Send an immediate test embed to one configured community webhook.
pub async fn test(
    State(s): State<AppState>,
    Extension(user): Extension<User>,
    Path((server, id)): Path<(Uuid, Uuid)>,
) -> Result<Json<Value>> {
    let (community, _) = service::authorize(&s, &user, server, true).await?;
    let webhook: Webhook = sqlx::query_as("SELECT * FROM webhooks WHERE id=$1 AND community_id=$2")
        .bind(id)
        .bind(server)
        .fetch_optional(&s.pool)
        .await?
        .ok_or_else(not_found)?;
    let payload = crate::webhooks::test_payload(&webhook, &community.name);
    tokio::time::timeout(
        std::time::Duration::from_secs(15),
        crate::worker::send_discord(&webhook, &payload),
    )
    .await
    .map_err(|_| ApiError(StatusCode::GATEWAY_TIMEOUT, "Webhook test timed out".into()))?
    .map_err(|e| ApiError(StatusCode::BAD_GATEWAY, e.to_string()))?;
    Ok(Json(json!({"ok": true})))
}
