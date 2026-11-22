use super::{User, service};
use crate::{
    AppState,
    api::{Result, bad},
};
use axum::{
    Extension, Json,
    extract::{Path, State},
};
use serde::Deserialize;
use serde_json::{Value, json};
use uuid::Uuid;

pub const DEFAULTS: [&str; 4] = ["2D Illustration", "3D Art", "Photography", "Mixed Media"];

pub fn validate(category: &str) -> Result<()> {
    if category.chars().count() > 500 || category.chars().any(char::is_control) {
        return Err(bad(
            "Category must be at most 500 characters without control characters",
        ));
    }
    Ok(())
}

pub async fn list(
    State(s): State<AppState>,
    Extension(user): Extension<User>,
    Path(server): Path<Uuid>,
) -> Result<Json<Value>> {
    service::authorize(&s, &user, server, false).await?;
    let rows: Vec<Value> = sqlx::query_scalar("WITH names AS (SELECT unnest($2::text[]) AS category UNION SELECT category FROM community_artists WHERE community_id=$1 AND category<>'' UNION SELECT category FROM community_category_routes WHERE community_id=$1) SELECT jsonb_build_object('category',n.category,'channel_id',r.channel_id,'channel_name',coalesce(r.channel_name,''),'artists',(SELECT count(*) FROM community_artists a WHERE a.community_id=$1 AND a.category=n.category)) FROM names n LEFT JOIN community_category_routes r ON r.community_id=$1 AND r.category=n.category ORDER BY n.category")
        .bind(server).bind(DEFAULTS.to_vec()).fetch_all(&s.pool).await?;
    Ok(Json(json!(rows)))
}

#[derive(Deserialize)]
pub struct RouteInput {
    category: String,
    channel_id: String,
}

pub async fn save(
    State(s): State<AppState>,
    Extension(user): Extension<User>,
    Path(server): Path<Uuid>,
    Json(i): Json<RouteInput>,
) -> Result<Json<Value>> {
    let (community, _) = service::authorize(&s, &user, server, true).await?;
    let category = i.category.trim();
    validate(category)?;
    if category.is_empty() {
        return Err(bad("Choose a category"));
    }
    let channel = i.channel_id.trim();
    if channel.is_empty() {
        sqlx::query("DELETE FROM community_category_routes WHERE community_id=$1 AND category=$2")
            .bind(server)
            .bind(category)
            .execute(&s.pool)
            .await?;
    } else {
        let discord = s
            .discord
            .as_ref()
            .ok_or_else(|| bad("Discord is not configured"))?;
        let name = discord
            .verify_channel(&community.external_id, channel)
            .await
            .map_err(|_| bad("Choose a channel in this server that the bot can access"))?;
        sqlx::query("INSERT INTO community_category_routes(community_id,category,channel_id,channel_name) VALUES($1,$2,$3,$4) ON CONFLICT(community_id,category) DO UPDATE SET channel_id=excluded.channel_id,channel_name=excluded.channel_name")
            .bind(server).bind(category).bind(channel).bind(name).execute(&s.pool).await?;
    }
    Ok(Json(json!({"ok":true})))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BulkCategoryInput {
    pub artist_ids: Vec<Uuid>,
    pub category: String,
}

pub async fn bulk_assign(
    State(s): State<AppState>,
    Extension(user): Extension<User>,
    Path(server): Path<Uuid>,
    Json(i): Json<BulkCategoryInput>,
) -> Result<Json<Value>> {
    service::authorize(&s, &user, server, true).await?;
    if i.artist_ids.is_empty() || i.artist_ids.len() > 500 {
        return Err(bad("Select between 1 and 500 artists"));
    }
    let category = i.category.trim();
    validate(category)?;
    let updated = sqlx::query(
        "UPDATE community_artists SET category=$3 WHERE community_id=$1 AND id = ANY($2)",
    )
    .bind(server)
    .bind(&i.artist_ids)
    .bind(category)
    .execute(&s.pool)
    .await?
    .rows_affected();
    s.wake.notify_one();
    Ok(Json(json!({"updated": updated, "category": category})))
}
