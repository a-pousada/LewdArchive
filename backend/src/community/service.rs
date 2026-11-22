//! Platform-neutral community operations. Callers must authorize server access first.
use super::{ArtistInput, Preference, Server, User};
use crate::{
    AppState,
    api::{ApiError, Result, bad, not_found},
    network,
};
use axum::http::StatusCode;
use serde_json::{Value, json};
use uuid::Uuid;

pub async fn upsert_user(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    platform: &str,
    external: &str,
    name: &str,
    avatar: &str,
) -> Result<User> {
    Ok(sqlx::query_as("INSERT INTO community_users(id,platform,external_id,display_name,avatar_url) VALUES($1,$5,$2,$3,$4) ON CONFLICT(platform,external_id) DO UPDATE SET display_name=excluded.display_name,avatar_url=excluded.avatar_url RETURNING *")
        .bind(Uuid::new_v4()).bind(external).bind(name).bind(avatar).bind(platform).fetch_one(&mut **tx).await?)
}
pub async fn authorize(
    s: &AppState,
    user: &User,
    id: Uuid,
    manage: bool,
) -> Result<(Server, bool)> {
    let server: Server = sqlx::query_as("SELECT * FROM communities WHERE id=$1 AND platform=$2")
        .bind(id)
        .bind(&user.platform)
        .fetch_optional(&s.pool)
        .await?
        .ok_or_else(not_found)?;
    let adapter = s.platforms.get(&server.platform).ok_or_else(|| {
        ApiError(
            StatusCode::SERVICE_UNAVAILABLE,
            "This platform is not configured".into(),
        )
    })?;
    let manager = adapter
        .membership(&server.external_id, &user.external_id)
        .await
        .map_err(|_| {
            ApiError(
                StatusCode::FORBIDDEN,
                "Unable to verify your current server membership. Check access and try again."
                    .into(),
            )
        })?;
    if manage && !manager {
        return Err(ApiError(
            StatusCode::FORBIDDEN,
            "Administrator permission is required".into(),
        ));
    }
    Ok((server, manager))
}
pub async fn connect(s: &AppState, platform: &str, external: &str, name: &str) -> Result<Server> {
    Ok(sqlx::query_as("INSERT INTO communities(id,platform,external_id,name) VALUES($1,$4,$2,$3) ON CONFLICT(platform,external_id) DO UPDATE SET name=excluded.name RETURNING *").bind(Uuid::new_v4()).bind(external).bind(name).bind(platform).fetch_one(&s.pool).await?)
}
pub async fn track(s: &AppState, server: Uuid, i: &ArtistInput) -> Result<Value> {
    if i.name.trim().is_empty() || i.name.chars().count() > 120 {
        return Err(bad("Artist name must be between 1 and 120 characters"));
    }
    if i.url.len() > 4096
        || !matches!(
            i.source.as_str(),
            "rss" | "pawchive" | "pixiv" | "bluesky" | "xcancel"
        )
    {
        return Err(bad("Invalid feed source or URL"));
    }
    super::categories::validate(i.category.trim())?;
    network::web_url(i.url.trim()).map_err(|e| bad(e.to_string()))?;
    let mut tx = s.pool.begin().await?;
    // Feed ingestion is shared, but server subscriptions and user preferences are not.
    sqlx::query(
        "INSERT INTO feeds(id,name,url,source) VALUES($1,$2,$3,$4) ON CONFLICT(url) DO NOTHING",
    )
    .bind(Uuid::new_v4())
    .bind(i.name.trim())
    .bind(i.url.trim())
    .bind(&i.source)
    .execute(&mut *tx)
    .await?;
    let feed: Uuid = sqlx::query_scalar("SELECT id FROM feeds WHERE url=$1 FOR KEY SHARE")
        .bind(i.url.trim())
        .fetch_one(&mut *tx)
        .await?;
    let artist:Uuid=sqlx::query_scalar("INSERT INTO community_artists(id,community_id,feed_id,name,category) VALUES($1,$2,$3,$4,$5) ON CONFLICT(community_id,feed_id) DO UPDATE SET enabled=true RETURNING id")
        .bind(Uuid::new_v4()).bind(server).bind(feed).bind(i.name.trim()).bind(i.category.trim()).fetch_one(&mut *tx).await?;
    tx.commit().await?;
    s.wake.notify_one();
    Ok(json!({"id":artist,"feed_id":feed}))
}
pub async fn untrack(s: &AppState, server: Uuid, artist: Uuid) -> Result<()> {
    if sqlx::query("DELETE FROM community_artists WHERE id=$1 AND community_id=$2")
        .bind(artist)
        .bind(server)
        .execute(&s.pool)
        .await?
        .rows_affected()
        == 0
    {
        return Err(not_found());
    }
    Ok(())
}
#[derive(serde::Deserialize, Default)]
pub struct ArtistFilter {
    #[serde(default)]
    pub favorites: bool,
    pub category: Option<String>,
    #[serde(default)]
    pub q: String,
    #[serde(default)]
    pub page: i64,
    #[serde(default)]
    pub limit: i64,
}
pub async fn artists(s: &AppState, server: Uuid, user: Uuid, q: &ArtistFilter) -> Result<Value> {
    let limit = (q.limit > 0).then(|| q.limit.clamp(1, 500));
    let offset = limit.map_or(0, |lim| (q.page.max(1) - 1).saturating_mul(lim));
    let search = q.q.trim().to_lowercase();
    let rows: Value = sqlx::query_scalar(
        "SELECT coalesce(jsonb_agg(row_obj), '[]'::jsonb) FROM (SELECT jsonb_build_object('id',a.id,'feed_id',a.feed_id,'name',a.name,'avatar_url',a.avatar_url,'official_links',a.official_links,'url',f.url,'source',f.source,'enabled',a.enabled,'category',a.category,'last_error',f.last_error,'consecutive_errors',f.consecutive_errors,'last_checked_at',f.last_checked_at,'next_check_at',f.next_check_at,'favorite',coalesce(p.favorite,false),'notify',coalesce(p.notify,false)) AS row_obj FROM community_artists a JOIN feeds f ON f.id=a.feed_id LEFT JOIN artist_preferences p ON p.artist_id=a.id AND p.user_id=$2 WHERE a.community_id=$1 AND (NOT $3 OR p.favorite) AND ($4::text IS NULL OR a.category=$4) AND ($5='' OR strpos(lower(a.name),$5)>0 OR strpos(lower(f.source),$5)>0) ORDER BY lower(a.name),a.id LIMIT $6 OFFSET $7) sub",
    )
    .bind(server)
    .bind(user)
    .bind(q.favorites)
    .bind(&q.category)
    .bind(search)
    .bind(limit)
    .bind(offset)
    .fetch_one(&s.pool)
    .await?;
    Ok(rows)
}
#[derive(serde::Deserialize, Default)]
pub struct PostFilter {
    #[serde(default)]
    pub page: i64,
    #[serde(default)]
    pub favorites: bool,
    #[serde(default)]
    pub liked: bool,
    pub category: Option<String>,
    #[serde(default)]
    pub tag: String,
}
pub async fn posts(
    s: &AppState,
    server: Uuid,
    user: Uuid,
    q: &PostFilter,
    limit: i64,
) -> Result<Value> {
    let page = q.page.clamp(1, 100000);
    let limit = limit.clamp(1, 24);
    let filter = " FROM entries e JOIN community_artists a ON a.feed_id=e.feed_id AND a.community_id=$1 LEFT JOIN post_preferences p ON p.entry_id=e.id AND p.community_id=$1 AND p.user_id=$2 LEFT JOIN community_post_tags t ON t.community_id=$1 AND t.entry_id=e.id WHERE (NOT $3 OR p.favorite) AND (NOT $4 OR p.liked) AND ($5::text IS NULL OR a.category=$5) AND ($6='' OR t.tags @> ARRAY[$6::text])";
    let rows:Vec<Value>=sqlx::query_scalar(&format!("SELECT jsonb_build_object('id',e.id,'artist_id',a.id,'feed_id',e.feed_id,'title',e.title,'author',e.author,'url',e.url,'summary',e.summary,'media',e.media,'published_at',e.published_at,'feed_name',a.name,'category',a.category,'tags_source',t.source,'tags',coalesce(t.tags,'{{}}'::text[]),'favorite',coalesce(p.favorite,false),'liked',coalesce(p.liked,false),'likes',(SELECT count(*) FROM post_preferences l WHERE l.community_id=$1 AND l.entry_id=e.id AND l.liked)){filter} ORDER BY e.published_at DESC,e.id DESC LIMIT $8 OFFSET $7"))
        .bind(server).bind(user).bind(q.favorites).bind(q.liked).bind(&q.category).bind(q.tag.trim().to_lowercase()).bind((page-1)*limit).bind(limit).fetch_all(&s.pool).await?;
    let total: i64 = sqlx::query_scalar(&format!("SELECT count(*){filter}"))
        .bind(server)
        .bind(user)
        .bind(q.favorites)
        .bind(q.liked)
        .bind(&q.category)
        .bind(q.tag.trim().to_lowercase())
        .fetch_one(&s.pool)
        .await?;
    Ok(json!({"items":rows,"total":total,"page":page,"pages":(total+limit-1)/limit}))
}
pub async fn post_preference(
    s: &AppState,
    server: Uuid,
    user: Uuid,
    entry: Uuid,
    i: &Preference,
) -> Result<Value> {
    if i.notify.is_some() {
        return Err(bad("Notification preferences belong to artists"));
    }
    let changed=sqlx::query("INSERT INTO post_preferences(user_id,community_id,entry_id,liked,favorite) SELECT $1,$2,e.id,coalesce($4,false),coalesce($5,false) FROM entries e JOIN community_artists a ON a.feed_id=e.feed_id WHERE e.id=$3 AND a.community_id=$2 ON CONFLICT(user_id,community_id,entry_id) DO UPDATE SET liked=coalesce($4,post_preferences.liked),favorite=coalesce($5,post_preferences.favorite),updated_at=now()")
        .bind(user).bind(server).bind(entry).bind(i.liked).bind(i.favorite).execute(&s.pool).await?.rows_affected();
    if changed == 0 {
        return Err(not_found());
    }
    let likes: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM post_preferences WHERE community_id=$1 AND entry_id=$2 AND liked",
    )
    .bind(server)
    .bind(entry)
    .fetch_one(&s.pool)
    .await?;
    Ok(json!({"ok":true,"likes":likes}))
}
pub async fn artist_preference(
    s: &AppState,
    server: Uuid,
    user: Uuid,
    artist: Uuid,
    i: &Preference,
) -> Result<Value> {
    if i.liked.is_some() {
        return Err(bad("Likes belong to posts"));
    }
    let changed=sqlx::query("INSERT INTO artist_preferences(user_id,artist_id,favorite,notify) SELECT $1,a.id,coalesce($4,false),coalesce($5,false) FROM community_artists a WHERE a.id=$3 AND a.community_id=$2 ON CONFLICT(user_id,artist_id) DO UPDATE SET favorite=coalesce($4,artist_preferences.favorite),notify=coalesce($5,artist_preferences.notify)")
        .bind(user).bind(server).bind(artist).bind(i.favorite).bind(i.notify).execute(&s.pool).await?.rows_affected();
    if changed == 0 {
        return Err(not_found());
    }
    Ok(json!({"ok":true}))
}
