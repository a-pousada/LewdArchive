//! Server-scoped discussion; authors use provider-neutral community identities.
use super::{User, service};
use crate::{
    AppState,
    api::{ApiError, Result, bad, not_found},
};
use axum::{
    Extension, Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use serde::Deserialize;
use serde_json::{Value, json};
use uuid::Uuid;

#[derive(Deserialize, Default)]
pub struct Pagination {
    #[serde(default)]
    page: i64,
}
#[derive(Deserialize)]
pub struct Input {
    body: String,
}
async fn artist(s: &AppState, server: Uuid, post: Uuid) -> Result<Uuid> {
    sqlx::query_scalar("SELECT a.id FROM community_artists a JOIN entries e ON e.feed_id=a.feed_id WHERE a.community_id=$1 AND e.id=$2")
        .bind(server).bind(post).fetch_optional(&s.pool).await?.ok_or_else(not_found)
}
pub async fn list(
    State(s): State<AppState>,
    Extension(user): Extension<User>,
    Path((server, post)): Path<(Uuid, Uuid)>,
    Query(q): Query<Pagination>,
) -> Result<Json<Value>> {
    let (_, admin) = service::authorize(&s, &user, server, false).await?;
    artist(&s, server, post).await?;
    let page = q.page.clamp(1, 10000);
    let total: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM post_comments WHERE community_id=$1 AND entry_id=$2",
    )
    .bind(server)
    .bind(post)
    .fetch_one(&s.pool)
    .await?;
    let items:Vec<Value>=sqlx::query_scalar("SELECT jsonb_build_object('id',c.id,'body',c.body,'created_at',c.created_at,'can_delete',c.user_id=$3 OR $4,'author',jsonb_build_object('id',u.id,'platform',u.platform,'display_name',u.display_name,'avatar_url',u.avatar_url)) FROM post_comments c JOIN community_users u ON u.id=c.user_id WHERE c.community_id=$1 AND c.entry_id=$2 ORDER BY c.created_at DESC,c.id DESC LIMIT 30 OFFSET $5")
        .bind(server).bind(post).bind(user.id).bind(admin).bind((page-1)*30).fetch_all(&s.pool).await?;
    Ok(Json(
        json!({"items":items,"total":total,"page":page,"pages":(total+29)/30}),
    ))
}
pub async fn create(
    State(s): State<AppState>,
    Extension(user): Extension<User>,
    Path((server, post)): Path<(Uuid, Uuid)>,
    Json(i): Json<Input>,
) -> Result<(StatusCode, Json<Value>)> {
    service::authorize(&s, &user, server, false).await?;
    let artist = artist(&s, server, post).await?;
    let body = i.body.trim();
    if body.is_empty() || body.chars().count() > 2000 {
        return Err(bad("Comments must contain between 1 and 2,000 characters"));
    }
    let mut tx = s.pool.begin().await?;
    // Serialize writes per author so parallel requests cannot bypass the posting limit.
    sqlx::query("SELECT id FROM community_users WHERE id=$1 FOR UPDATE")
        .bind(user.id)
        .execute(&mut *tx)
        .await?;
    let recent:i64=sqlx::query_scalar("SELECT count(*) FROM post_comments WHERE user_id=$1 AND created_at>now()-interval '1 minute'").bind(user.id).fetch_one(&mut *tx).await?;
    if recent >= 5 {
        return Err(ApiError(
            StatusCode::TOO_MANY_REQUESTS,
            "Please wait a minute before posting another comment".into(),
        ));
    }
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO post_comments(id,community_id,artist_id,entry_id,user_id,body) VALUES($1,$2,$3,$4,$5,$6)").bind(id).bind(server).bind(artist).bind(post).bind(user.id).bind(body).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok((StatusCode::CREATED, Json(json!({"id":id}))))
}
pub async fn delete(
    State(s): State<AppState>,
    Extension(user): Extension<User>,
    Path((server, post, id)): Path<(Uuid, Uuid, Uuid)>,
) -> Result<Json<Value>> {
    let (_, admin) = service::authorize(&s, &user, server, false).await?;
    artist(&s, server, post).await?;
    let removed=sqlx::query("DELETE FROM post_comments WHERE id=$1 AND community_id=$2 AND entry_id=$3 AND (user_id=$4 OR $5)").bind(id).bind(server).bind(post).bind(user.id).bind(admin).execute(&s.pool).await?.rows_affected();
    if removed == 0 {
        return Err(not_found());
    }
    Ok(Json(json!({"ok":true})))
}
