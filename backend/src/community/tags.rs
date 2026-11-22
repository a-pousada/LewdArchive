use super::{User, service};
use crate::{
    AppState,
    api::{ApiError, Result, bad, not_found},
};
use axum::{
    Extension, Json,
    extract::{Path, State},
    http::StatusCode,
};
use serde::Deserialize;
use serde_json::{Value, json};
use uuid::Uuid;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TagInput {
    pub tags: Vec<String>,
}

pub fn normalize(tags: &[String]) -> Result<Vec<String>> {
    if tags.len() > 30 {
        return Err(bad("Use at most 30 tags"));
    }
    let mut result = Vec::new();
    for raw in tags {
        let tag = raw
            .trim()
            .trim_start_matches('#')
            .to_lowercase()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join("_");
        if tag.is_empty() {
            continue;
        }
        if tag.chars().count() > 48
            || !tag
                .chars()
                .all(|c| c.is_alphanumeric() || matches!(c, '_' | '-' | '(' | ')'))
        {
            return Err(bad(
                "Tags must contain up to 48 letters, numbers, underscores, hyphens or parentheses",
            ));
        }
        if !result.contains(&tag) {
            result.push(tag);
        }
    }
    Ok(result)
}

pub async fn save(
    State(s): State<AppState>,
    Extension(user): Extension<User>,
    Path((server, post)): Path<(Uuid, Uuid)>,
    Json(i): Json<TagInput>,
) -> Result<Json<Value>> {
    service::authorize(&s, &user, server, true).await?;
    let tags = normalize(&i.tags)?;
    let mut tx = s.pool.begin().await?;
    let changed = sqlx::query("INSERT INTO community_post_tags(community_id,entry_id,artist_id,tags) SELECT $1,e.id,a.id,$3 FROM entries e JOIN community_artists a ON a.feed_id=e.feed_id AND a.community_id=$1 WHERE e.id=$2 ON CONFLICT(community_id,entry_id) DO UPDATE SET tags=excluded.tags,source='manual',updated_at=now()")
        .bind(server).bind(post).bind(&tags).execute(&mut *tx).await?.rows_affected();
    if changed == 0 {
        return Err(not_found());
    }
    sqlx::query("UPDATE community_tag_jobs SET status='done',last_error=NULL,locked_at=NULL WHERE community_id=$1 AND entry_id=$2").bind(server).bind(post).execute(&mut *tx).await?;
    tx.commit().await?;
    s.wake.notify_one();
    Ok(Json(json!({"tags":tags})))
}

pub async fn config(
    State(s): State<AppState>,
    Extension(user): Extension<User>,
    Path(server): Path<Uuid>,
) -> Result<Json<Value>> {
    service::authorize(&s, &user, server, true).await?;
    Ok(Json(
        json!({"enabled":s.tagger.is_some(),"images":s.tagger.is_some(),"provider":"wd","model":crate::ai::MODEL}),
    ))
}

pub async fn suggest(
    State(s): State<AppState>,
    Extension(user): Extension<User>,
    Path((server, post)): Path<(Uuid, Uuid)>,
) -> Result<Json<Value>> {
    service::authorize(&s, &user, server, true).await?;
    let (media,): (Value,) = sqlx::query_as("SELECT e.media FROM entries e JOIN community_artists a ON a.feed_id=e.feed_id AND a.community_id=$1 WHERE e.id=$2")
        .bind(server).bind(post).fetch_optional(&s.pool).await?.ok_or_else(not_found)?;
    let tagger = s.tagger.as_ref().ok_or_else(|| {
        ApiError(
            StatusCode::SERVICE_UNAVAILABLE,
            "AI tagging is not configured by the installation administrator".into(),
        )
    })?;
    if !crate::ai::has_images(&media) {
        return Err(ApiError(
            StatusCode::UNPROCESSABLE_ENTITY,
            "This post has no images to analyze".into(),
        ));
    }
    let tags = tagger
        .suggest(&media)
        .await
        .map_err(|e| ApiError(StatusCode::BAD_GATEWAY, e.to_string()))?;
    Ok(Json(json!({"tags":tags,"saved":false})))
}

/// Metadata is resolved in the destination server, including for shared feeds.
pub async fn decorate(
    s: &AppState,
    server: Uuid,
    entry: Uuid,
    payload: &mut Value,
) -> anyhow::Result<()> {
    let (category, tags): (String, Vec<String>) = sqlx::query_as("SELECT a.category,coalesce(t.tags,'{}'::text[]) FROM entries e JOIN community_artists a ON a.feed_id=e.feed_id AND a.community_id=$1 LEFT JOIN community_post_tags t ON t.community_id=$1 AND t.entry_id=e.id WHERE e.id=$2")
        .bind(server).bind(entry).fetch_one(&s.pool).await?;
    let fields = payload["embeds"][0]["fields"]
        .as_array_mut()
        .ok_or_else(|| anyhow::anyhow!("Missing embed fields"))?;
    if !category.is_empty() {
        fields.push(json!({"name":"Category","value":category,"inline":true}));
    }
    if !tags.is_empty() {
        // Whole tags only, below Discord's 1,024-character field limit.
        let mut value = String::new();
        for tag in tags {
            let next = format!("`#{tag}` ");
            if value.chars().count() + next.chars().count() > 1000 {
                break;
            }
            value.push_str(&next);
        }
        fields.push(json!({"name":"Tags","value":value.trim()}));
    }
    Ok(())
}

pub async fn jobs(
    State(s): State<AppState>,
    Extension(user): Extension<User>,
    Path(server): Path<Uuid>,
) -> Result<Json<Value>> {
    let (community, _) = service::authorize(&s, &user, server, true).await?;
    let counts: Value = sqlx::query_scalar(
        "SELECT jsonb_build_object('pending',count(*) FILTER (WHERE status='pending'),'processing',count(*) FILTER (WHERE status='processing'),'done',count(*) FILTER (WHERE status='done'),'failed',count(*) FILTER (WHERE status='failed')) FROM community_tag_jobs WHERE community_id=$1",
    )
    .bind(server)
    .fetch_one(&s.pool)
    .await?;
    let untagged: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM entries e JOIN community_artists a ON a.feed_id=e.feed_id AND a.community_id=$1 LEFT JOIN community_post_tags t ON t.community_id=$1 AND t.entry_id=e.id LEFT JOIN community_tag_jobs j ON j.community_id=$1 AND j.entry_id=e.id WHERE t.entry_id IS NULL AND j.entry_id IS NULL AND EXISTS(SELECT 1 FROM jsonb_array_elements(e.media) m WHERE m->>'kind'='image')",
    )
    .bind(server)
    .fetch_one(&s.pool)
    .await?;
    let recent: Value = sqlx::query_scalar(
        "SELECT coalesce(jsonb_agg(item), '[]'::jsonb) FROM (SELECT jsonb_build_object('entry_id',j.entry_id,'title',e.title,'artist',a.name,'status',j.status,'attempts',j.attempts,'last_error',j.last_error,'created_at',j.created_at) AS item FROM community_tag_jobs j JOIN entries e ON e.id=j.entry_id JOIN community_artists a ON a.id=j.artist_id AND a.community_id=j.community_id WHERE j.community_id=$1 ORDER BY CASE j.status WHEN 'failed' THEN 0 WHEN 'processing' THEN 1 WHEN 'pending' THEN 2 ELSE 3 END,j.created_at DESC LIMIT 50) sub",
    )
    .bind(server)
    .fetch_one(&s.pool)
    .await?;
    let mut summary = counts;
    if let Some(obj) = summary.as_object_mut() {
        obj.insert("untagged_with_images".into(), json!(untagged));
    }
    Ok(Json(json!({
        "tagger_configured": s.tagger.is_some(),
        "ai_tags_enabled": community.ai_tags_enabled,
        "counts": summary,
        "recent": recent,
    })))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BackfillInput {
    pub limit: i64,
}

pub async fn backfill(
    State(s): State<AppState>,
    Extension(user): Extension<User>,
    Path(server): Path<Uuid>,
    Json(i): Json<BackfillInput>,
) -> Result<Json<Value>> {
    let (community, _) = service::authorize(&s, &user, server, true).await?;
    if !(1..=1000).contains(&i.limit) {
        return Err(bad("Choose a batch size between 1 and 1000"));
    }
    if s.tagger.is_none() {
        return Err(ApiError(
            StatusCode::SERVICE_UNAVAILABLE,
            "AI tagging is not configured by the installation administrator".into(),
        ));
    }
    if !community.ai_tags_enabled {
        return Err(bad(
            "Enable automatic AI tagging in Server settings before starting a backfill",
        ));
    }
    let queued = sqlx::query(
        "INSERT INTO community_tag_jobs(community_id,entry_id,artist_id) SELECT $1,e.id,a.id FROM entries e JOIN community_artists a ON a.feed_id=e.feed_id AND a.community_id=$1 LEFT JOIN community_post_tags t ON t.community_id=$1 AND t.entry_id=e.id LEFT JOIN community_tag_jobs j ON j.community_id=$1 AND j.entry_id=e.id WHERE t.entry_id IS NULL AND j.entry_id IS NULL AND EXISTS(SELECT 1 FROM jsonb_array_elements(e.media) m WHERE m->>'kind'='image') ORDER BY e.published_at DESC,e.id DESC LIMIT $2 ON CONFLICT DO NOTHING",
    )
    .bind(server)
    .bind(i.limit)
    .execute(&s.pool)
    .await?
    .rows_affected();
    s.wake.notify_one();
    Ok(Json(json!({"queued": queued})))
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct RetryTagJobsInput {
    #[serde(default)]
    pub entry_id: Option<Uuid>,
}

pub async fn retry_jobs(
    State(s): State<AppState>,
    Extension(user): Extension<User>,
    Path(server): Path<Uuid>,
    Json(i): Json<RetryTagJobsInput>,
) -> Result<Json<Value>> {
    service::authorize(&s, &user, server, true).await?;
    let retried = sqlx::query(
        "UPDATE community_tag_jobs SET status='pending',attempts=0,next_attempt_at=now(),locked_at=NULL,last_error=NULL WHERE community_id=$1 AND status='failed' AND ($2::uuid IS NULL OR entry_id=$2)",
    )
    .bind(server)
    .bind(i.entry_id)
    .execute(&s.pool)
    .await?
    .rows_affected();
    s.wake.notify_one();
    Ok(Json(json!({"retried": retried})))
}

pub async fn cancel_pending(
    State(s): State<AppState>,
    Extension(user): Extension<User>,
    Path(server): Path<Uuid>,
) -> Result<Json<Value>> {
    service::authorize(&s, &user, server, true).await?;
    let cancelled =
        sqlx::query("DELETE FROM community_tag_jobs WHERE community_id=$1 AND status='pending'")
            .bind(server)
            .execute(&s.pool)
            .await?
            .rows_affected();
    Ok(Json(json!({"cancelled": cancelled})))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn normalization_and_limits() {
        assert_eq!(
            normalize(&[
                " #Digital Art ".into(),
                "digital_art".into(),
                "Blue Sky".into()
            ])
            .ok()
            .unwrap(),
            vec!["digital_art", "blue_sky"]
        );
        for invalid in [
            "@everyone",
            "[link](https://example.com)",
            "tag`",
            "a\u{200b}b",
        ] {
            assert!(normalize(&[invalid.into()]).is_err());
        }
        assert!(normalize(&vec!["tag".into(); 31]).is_err());
        assert!(normalize(&["x".repeat(49)]).is_err());
    }
}
