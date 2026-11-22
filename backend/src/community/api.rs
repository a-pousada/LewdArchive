use super::{ArtistInput, Preference, User, service};
use crate::{
    AppState,
    api::{ApiError, Result, bad},
    platforms::{Platform, discord::snowflake},
};
use axum::{
    Extension, Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use serde::Deserialize;
use serde_json::{Value, json};
use uuid::Uuid;
pub async fn me(Extension(user): Extension<User>) -> Json<User> {
    Json(user)
}
pub async fn servers(
    State(s): State<AppState>,
    Extension(user): Extension<User>,
) -> Result<Json<Value>> {
    let rows:Vec<Value>=sqlx::query_scalar("SELECT jsonb_build_object('id',c.id,'external_id',g.external_id,'name',g.name,'connected',c.id IS NOT NULL,'can_manage',g.can_manage) FROM login_guilds g LEFT JOIN communities c ON c.external_id=g.external_id AND c.platform=$2 WHERE g.user_id=$1 ORDER BY lower(g.name)")
        .bind(user.id).bind(&user.platform).fetch_all(&s.pool).await?;
    Ok(Json(json!(rows)))
}
#[derive(Deserialize)]
pub struct Connect {
    external_id: String,
}
pub async fn connect(
    State(s): State<AppState>,
    Extension(user): Extension<User>,
    Json(i): Json<Connect>,
) -> Result<Json<Value>> {
    let d = s
        .discord
        .as_ref()
        .ok_or_else(|| bad("Discord is not configured"))?;
    if !snowflake(&i.external_id) || user.platform != "discord" {
        return Err(bad("Invalid Discord server"));
    }
    if !d
        .membership(&i.external_id, &user.external_id)
        .await
        .map_err(|_| bad("Invite the bot and verify your server membership first"))?
    {
        return Err(ApiError(
            StatusCode::FORBIDDEN,
            "Administrator permission is required".into(),
        ));
    }
    let guild = d
        .guild(&i.external_id)
        .await
        .map_err(|_| bad("Server unavailable"))?;
    let server = service::connect(
        &s,
        "discord",
        &i.external_id,
        guild["name"].as_str().unwrap_or("Discord server"),
    )
    .await?;
    Ok(Json(json!(server)))
}
pub async fn server(
    State(s): State<AppState>,
    Extension(user): Extension<User>,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>> {
    let (server, manage) = service::authorize(&s, &user, id, false).await?;
    Ok(Json(json!({"server":server,"can_manage":manage})))
}
#[derive(Deserialize)]
pub struct Settings {
    pub channel_id: String,
    pub enabled: bool,
    pub mentions_enabled: bool,
    pub ai_tags_enabled: Option<bool>,
}
pub async fn settings(
    State(s): State<AppState>,
    Extension(user): Extension<User>,
    Path(id): Path<Uuid>,
    Json(i): Json<Settings>,
) -> Result<Json<Value>> {
    let (server, _) = service::authorize(&s, &user, id, true).await?;
    if i.ai_tags_enabled == Some(true) && s.tagger.is_none() {
        return Err(bad(
            "Configure WD_TAGGER_URL before enabling automatic tags",
        ));
    }
    let channel = (!i.channel_id.trim().is_empty()).then(|| i.channel_id.trim().to_owned());
    let name = if let Some(channel) = &channel {
        let d = s
            .discord
            .as_ref()
            .ok_or_else(|| bad("Discord is not configured"))?;
        d.verify_channel(&server.external_id, channel)
            .await
            .map_err(|e| bad(e.to_string()))?
    } else {
        String::new()
    };
    sqlx::query("UPDATE communities SET channel_id=$2,channel_name=$3,enabled=$4,mentions_enabled=$5,ai_tags_enabled=coalesce($6,ai_tags_enabled) WHERE id=$1").bind(id).bind(channel).bind(name).bind(i.enabled).bind(i.mentions_enabled).bind(i.ai_tags_enabled).execute(&s.pool).await?;
    Ok(Json(json!({"ok":true})))
}
type Filter = service::PostFilter;
pub async fn artists(
    State(s): State<AppState>,
    Extension(user): Extension<User>,
    Path(id): Path<Uuid>,
    Query(q): Query<service::ArtistFilter>,
) -> Result<Json<Value>> {
    service::authorize(&s, &user, id, false).await?;
    Ok(Json(service::artists(&s, id, user.id, &q).await?))
}
pub async fn track(
    State(s): State<AppState>,
    Extension(user): Extension<User>,
    Path(id): Path<Uuid>,
    Json(i): Json<ArtistInput>,
) -> Result<Json<Value>> {
    service::authorize(&s, &user, id, true).await?;
    Ok(Json(service::track(&s, id, &i).await?))
}
pub async fn untrack(
    State(s): State<AppState>,
    Extension(user): Extension<User>,
    Path((server, artist)): Path<(Uuid, Uuid)>,
) -> Result<Json<Value>> {
    service::authorize(&s, &user, server, true).await?;
    service::untrack(&s, server, artist).await?;
    Ok(Json(json!({"ok":true})))
}
pub async fn posts(
    State(s): State<AppState>,
    Extension(user): Extension<User>,
    Path(id): Path<Uuid>,
    Query(q): Query<Filter>,
) -> Result<Json<Value>> {
    service::authorize(&s, &user, id, false).await?;
    Ok(Json(service::posts(&s, id, user.id, &q, 24).await?))
}
pub async fn post_preference(
    State(s): State<AppState>,
    Extension(user): Extension<User>,
    Path((server, entry)): Path<(Uuid, Uuid)>,
    Json(i): Json<Preference>,
) -> Result<Json<Value>> {
    service::authorize(&s, &user, server, false).await?;
    Ok(Json(
        service::post_preference(&s, server, user.id, entry, &i).await?,
    ))
}
pub async fn artist_preference(
    State(s): State<AppState>,
    Extension(user): Extension<User>,
    Path((server, artist)): Path<(Uuid, Uuid)>,
    Json(i): Json<Preference>,
) -> Result<Json<Value>> {
    service::authorize(&s, &user, server, false).await?;
    Ok(Json(
        service::artist_preference(&s, server, user.id, artist, &i).await?,
    ))
}
pub async fn deliveries(
    State(s): State<AppState>,
    Extension(user): Extension<User>,
    Path(server): Path<Uuid>,
) -> Result<Json<Value>> {
    service::authorize(&s, &user, server, true).await?;
    let rows:Vec<Value>=sqlx::query_scalar("SELECT jsonb_build_object('id',d.id,'title',e.title,'artist',a.name,'status',d.status,'attempts',d.attempts,'last_error',d.last_error,'channel_id',d.channel_id,'message_id',d.message_id,'created_at',d.created_at) FROM community_deliveries d JOIN entries e ON e.id=d.entry_id JOIN community_artists a ON a.id=d.artist_id WHERE d.community_id=$1 ORDER BY d.created_at DESC LIMIT 100").bind(server).fetch_all(&s.pool).await?;
    Ok(Json(json!(rows)))
}
pub async fn retry(
    State(s): State<AppState>,
    Extension(user): Extension<User>,
    Path((server, job)): Path<(Uuid, Uuid)>,
) -> Result<Json<Value>> {
    service::authorize(&s, &user, server, true).await?;
    if sqlx::query("UPDATE community_deliveries SET status='pending',attempts=0,locked_at=NULL,last_error=NULL,next_attempt_at=now() WHERE id=$1 AND community_id=$2 AND status='failed'").bind(job).bind(server).execute(&s.pool).await?.rows_affected()==0{return Err(bad("Only failed deliveries in this server can be retried"));}
    Ok(Json(json!({"ok":true})))
}

pub async fn trending(
    State(s): State<AppState>,
    Extension(user): Extension<User>,
    Path(server): Path<Uuid>,
    Query(q): Query<super::trending::Filter>,
) -> Result<Json<Value>> {
    service::authorize(&s, &user, server, false).await?;
    Ok(Json(
        super::trending::posts(&s, server, user.id, &q, 24).await?,
    ))
}
pub async fn artist_profile(
    State(s): State<AppState>,
    Extension(user): Extension<User>,
    Path((server, artist)): Path<(Uuid, Uuid)>,
    Json(i): Json<super::profiles::Profile>,
) -> Result<Json<Value>> {
    service::authorize(&s, &user, server, true).await?;
    Ok(Json(super::profiles::update(&s, server, artist, &i).await?))
}

#[derive(Deserialize)]
pub struct ArtistDelivery {
    enabled: Option<bool>,
}
pub async fn artist_delivery(
    State(s): State<AppState>,
    Extension(user): Extension<User>,
    Path((server, artist)): Path<(Uuid, Uuid)>,
    Json(i): Json<ArtistDelivery>,
) -> Result<Json<Value>> {
    service::authorize(&s, &user, server, true).await?;
    if sqlx::query(
        "UPDATE community_artists SET enabled=coalesce($3,enabled) WHERE id=$1 AND community_id=$2",
    )
    .bind(artist)
    .bind(server)
    .bind(i.enabled)
    .execute(&s.pool)
    .await?
    .rows_affected()
        == 0
    {
        return Err(crate::api::not_found());
    }
    s.wake.notify_one();
    Ok(Json(json!({"ok":true})))
}
pub async fn artist_sync(
    State(s): State<AppState>,
    Extension(user): Extension<User>,
    Path((server, artist)): Path<(Uuid, Uuid)>,
) -> Result<Json<Value>> {
    service::authorize(&s, &user, server, true).await?;
    let feed: Uuid =
        sqlx::query_scalar("SELECT feed_id FROM community_artists WHERE id=$1 AND community_id=$2")
            .bind(artist)
            .bind(server)
            .fetch_optional(&s.pool)
            .await?
            .ok_or_else(crate::api::not_found)?;
    // Global polling policy remains under the installation administrator's control.
    sqlx::query("UPDATE feeds SET next_check_at=now() WHERE id=$1 AND enabled")
        .bind(feed)
        .execute(&s.pool)
        .await?;
    s.wake.notify_one();
    Ok(Json(json!({"ok":true})))
}

pub async fn post_detail(
    State(s): State<AppState>,
    Extension(user): Extension<User>,
    Path((server, post)): Path<(Uuid, Uuid)>,
) -> Result<Json<Value>> {
    let (_, manager) = service::authorize(&s, &user, server, false).await?;
    let value:Value=sqlx::query_scalar("SELECT jsonb_build_object('id',e.id,'artist_id',a.id,'title',e.title,'author',e.author,'url',e.url,'summary',e.summary,'media',e.media,'published_at',e.published_at,'feed_name',a.name,'category',a.category,'tags_source',t.source,'tags',coalesce(t.tags,'{}'::text[]),'favorite',coalesce(p.favorite,false),'liked',coalesce(p.liked,false),'likes',(SELECT count(*) FROM post_preferences l WHERE l.community_id=$1 AND l.entry_id=e.id AND l.liked)) FROM entries e JOIN community_artists a ON a.feed_id=e.feed_id AND a.community_id=$1 LEFT JOIN post_preferences p ON p.entry_id=e.id AND p.community_id=$1 AND p.user_id=$2 LEFT JOIN community_post_tags t ON t.community_id=$1 AND t.entry_id=e.id WHERE e.id=$3").bind(server).bind(user.id).bind(post).fetch_optional(&s.pool).await?.ok_or_else(crate::api::not_found)?;
    Ok(Json(json!({"post":value,"can_manage":manager})))
}

pub async fn feed_health(
    State(s): State<AppState>,
    Extension(user): Extension<User>,
    Path(server): Path<Uuid>,
) -> Result<Json<Value>> {
    service::authorize(&s, &user, server, true).await?;
    let summary: Value = sqlx::query_scalar(
        "SELECT jsonb_build_object('total',count(*),'enabled',count(*) FILTER (WHERE f.enabled),'healthy',count(*) FILTER (WHERE f.enabled AND f.last_error IS NULL),'failing',count(*) FILTER (WHERE f.enabled AND f.last_error IS NOT NULL),'paused',count(*) FILTER (WHERE NOT f.enabled)) FROM community_artists a JOIN feeds f ON f.id=a.feed_id WHERE a.community_id=$1",
    )
    .bind(server)
    .fetch_one(&s.pool)
    .await?;
    let failing: Value = sqlx::query_scalar(
        "SELECT coalesce(jsonb_agg(item), '[]'::jsonb) FROM (SELECT jsonb_build_object('artist_id',a.id,'name',a.name,'source',f.source,'category',a.category,'enabled',f.enabled,'consecutive_errors',f.consecutive_errors,'last_error',f.last_error,'last_checked_at',f.last_checked_at,'next_check_at',f.next_check_at) AS item FROM community_artists a JOIN feeds f ON f.id=a.feed_id WHERE a.community_id=$1 AND f.enabled AND f.last_error IS NOT NULL ORDER BY f.consecutive_errors DESC,f.last_checked_at DESC NULLS LAST LIMIT 100) sub",
    )
    .bind(server)
    .fetch_one(&s.pool)
    .await?;
    Ok(Json(json!({"summary": summary, "failing": failing})))
}

pub async fn retry_failed_feeds(
    State(s): State<AppState>,
    Extension(user): Extension<User>,
    Path(server): Path<Uuid>,
) -> Result<Json<Value>> {
    service::authorize(&s, &user, server, true).await?;
    let queued = sqlx::query(
        "UPDATE feeds SET next_check_at=now() WHERE enabled AND last_error IS NOT NULL AND id IN (SELECT feed_id FROM community_artists WHERE community_id=$1)",
    )
    .bind(server)
    .execute(&s.pool)
    .await?
    .rows_affected();
    s.wake.notify_one();
    Ok(Json(json!({"queued": queued})))
}
