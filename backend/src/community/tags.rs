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
