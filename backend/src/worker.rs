use crate::platforms::discord::webhook_payload as discord_payload;
use crate::{
    AppState,
    models::{Entry, Feed, Job, Media, Settings, Webhook},
    network, parser,
};
use anyhow::{Context, Result, bail};
use futures_util::stream::{self, StreamExt};
use reqwest::header::{HeaderMap, HeaderValue, IF_MODIFIED_SINCE, IF_NONE_MATCH};
use serde_json::json;
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use uuid::Uuid;

pub(crate) fn backoff_minutes(poll_minutes: i32, consecutive_errors: i32) -> i32 {
    let base = poll_minutes.clamp(5, 1440);
    if consecutive_errors <= 0 {
        return base;
    }
    let shift = (consecutive_errors - 1).clamp(0, 4) as u32;
    base.saturating_mul(1i32 << shift).min(360)
}

pub async fn run(s: AppState) {
    loop {
        if let Err(e) = tick(&s).await {
            tracing::error!(error=%e,"worker cycle failed");
        }
        tokio::select! { _ = tokio::time::sleep(Duration::from_secs(10)) => {}, _ = s.wake.notified() => {} }
    }
}

pub async fn run_downloads(s: AppState) {
    loop {
        if let Err(e) = tick_downloads(&s).await {
            tracing::error!(error=%e,"download worker cycle failed");
        }
        tokio::select! { _ = tokio::time::sleep(Duration::from_secs(10)) => {}, _ = s.wake.notified() => {} }
    }
}

async fn tick(s: &AppState) -> Result<()> {
    // A session advisory lock permits only one scheduler, including across replicas.
    let mut conn = s.pool.acquire().await?;
    let locked: bool = sqlx::query_scalar("SELECT pg_try_advisory_lock(73421901)")
        .fetch_one(&mut *conn)
        .await?;
    if !locked {
        return Ok(());
    }
    let result = cycle(s).await;
    sqlx::query("SELECT pg_advisory_unlock(73421901)")
        .execute(&mut *conn)
        .await?;
    result
}

async fn tick_downloads(s: &AppState) -> Result<()> {
    let mut conn = s.pool.acquire().await?;
    let locked: bool = sqlx::query_scalar("SELECT pg_try_advisory_lock(73421902)")
        .fetch_one(&mut *conn)
        .await?;
    if !locked {
        return Ok(());
    }
    let result = cycle_downloads(s).await;
    sqlx::query("SELECT pg_advisory_unlock(73421902)")
        .execute(&mut *conn)
        .await?;
    result
}

async fn cycle(s: &AppState) -> Result<()> {
    let feeds: Vec<Feed> = sqlx::query_as("SELECT * FROM feeds WHERE enabled AND next_check_at<=now() AND EXISTS(SELECT 1 FROM community_artists a WHERE a.feed_id=feeds.id) ORDER BY next_check_at LIMIT 20").fetch_all(&s.pool).await?;
    let results = stream::iter(feeds)
        .map(|feed| async move {
            let res = poll(s, &feed).await;
            (feed.id, feed.poll_minutes, feed.consecutive_errors, res)
        })
        .buffer_unordered(6)
        .collect::<Vec<_>>()
        .await;
    for (id, poll_minutes, consecutive_errors, res) in results {
        match res {
            Ok(()) => {
                sqlx::query("UPDATE feeds SET last_checked_at=now(),next_check_at=now()+make_interval(mins=>poll_minutes),last_error=NULL,consecutive_errors=0 WHERE id=$1")
                    .bind(id)
                    .execute(&s.pool)
                    .await?;
            }
            Err(e) => {
                let next_errors = consecutive_errors.saturating_add(1);
                let delay = backoff_minutes(poll_minutes, next_errors);
                let error: String = e.to_string().chars().take(500).collect();
                sqlx::query("UPDATE feeds SET last_checked_at=now(),next_check_at=now()+make_interval(mins=>$3),last_error=$2,consecutive_errors=$4 WHERE id=$1")
                    .bind(id)
                    .bind(error)
                    .bind(delay)
                    .bind(next_errors)
                    .execute(&s.pool)
                    .await?;
            }
        }
    }
    sqlx::query("UPDATE jobs SET status='pending',next_attempt_at=now() WHERE status='processing' AND kind='discord' AND locked_at<now()-interval '15 minutes'").execute(&s.pool).await?;
    let settings: Settings =
        sqlx::query_as("SELECT downloads_enabled,notifications_enabled FROM settings")
            .fetch_one(&s.pool)
            .await?;
    if !settings.notifications_enabled {
        return Ok(());
    }
    for _ in 0..10 {
        let job: Option<Job> = sqlx::query_as("UPDATE jobs SET status='processing',locked_at=now(),attempts=attempts+1,updated_at=now() WHERE id=(SELECT id FROM jobs WHERE status='pending' AND kind='discord' AND next_attempt_at<=now() AND EXISTS(SELECT 1 FROM webhooks w WHERE w.id=jobs.webhook_id AND w.enabled AND (w.community_id IS NOT NULL AND EXISTS(SELECT 1 FROM communities c JOIN community_artists a ON a.community_id=c.id JOIN entries e ON e.feed_id=a.feed_id WHERE c.id=w.community_id AND c.enabled AND a.enabled AND (NOT $1 OR NOT c.ai_tags_enabled OR NOT EXISTS(SELECT 1 FROM community_tag_jobs t WHERE t.community_id=c.id AND t.entry_id=jobs.entry_id AND t.status IN('pending','processing') AND t.created_at>now()-interval '5 minutes')) AND ((w.category_filters IS NULL AND w.category_filter IS NULL) OR a.category = ANY(coalesce(w.category_filters, ARRAY[w.category_filter]))) AND e.id=jobs.entry_id))) ORDER BY next_attempt_at FOR UPDATE SKIP LOCKED LIMIT 1) RETURNING id,entry_id,kind,webhook_id,status,attempts,last_error,created_at,updated_at")
            .bind(s.tagger.is_some())
            .fetch_optional(&s.pool)
            .await?;
        let Some(job) = job else { break };
        if run_job(s, &job).await? {
            tokio::time::sleep(Duration::from_millis(600)).await;
        }
    }
    Ok(())
}

async fn cycle_downloads(s: &AppState) -> Result<()> {
    sqlx::query("UPDATE jobs SET status='pending',next_attempt_at=now() WHERE status='processing' AND kind='download' AND locked_at<now()-interval '15 minutes'").execute(&s.pool).await?;
    let settings: Settings =
        sqlx::query_as("SELECT downloads_enabled,notifications_enabled FROM settings")
            .fetch_one(&s.pool)
            .await?;
    if !settings.downloads_enabled {
        return Ok(());
    }
    for _ in 0..5 {
        let job: Option<Job> = sqlx::query_as("UPDATE jobs SET status='processing',locked_at=now(),attempts=attempts+1,updated_at=now() WHERE id=(SELECT id FROM jobs WHERE status='pending' AND kind='download' AND next_attempt_at<=now() AND EXISTS(SELECT 1 FROM entries e JOIN community_artists a ON a.feed_id=e.feed_id WHERE e.id=jobs.entry_id) ORDER BY next_attempt_at FOR UPDATE SKIP LOCKED LIMIT 1) RETURNING id,entry_id,kind,webhook_id,status,attempts,last_error,created_at,updated_at")
            .fetch_optional(&s.pool)
            .await?;
        let Some(job) = job else { break };
        run_job(s, &job).await?;
    }
    Ok(())
}

async fn run_job(s: &AppState, job: &Job) -> Result<bool> {
    let result = tokio::time::timeout(Duration::from_secs(600), execute(s, job))
        .await
        .unwrap_or_else(|_| Err(anyhow::anyhow!("Job exceeded the 10-minute time limit")));
    let outcome_done = matches!(result, Ok(DeliveryOutcome::Done));
    match result {
        Ok(DeliveryOutcome::Done) => {
            sqlx::query(
                "UPDATE jobs SET status='done',last_error=NULL,locked_at=NULL,updated_at=now() WHERE id=$1",
            )
            .bind(job.id)
            .execute(&s.pool)
            .await?;
        }
        Ok(DeliveryOutcome::Skipped) => {
            sqlx::query(
                "UPDATE jobs SET status='skipped',last_error=NULL,locked_at=NULL,updated_at=now() WHERE id=$1",
            )
            .bind(job.id)
            .execute(&s.pool)
            .await?;
        }
        Err(e) => {
            let retry = e
                .downcast_ref::<RateLimited>()
                .map(|r| r.0)
                .unwrap_or((30 * 2i32.pow(job.attempts.min(10) as u32)).min(3600));
            sqlx::query("UPDATE jobs SET status=$2,last_error=$3,next_attempt_at=now()+make_interval(secs=>$4),updated_at=now() WHERE id=$1")
                .bind(job.id)
                .bind(if job.attempts >= 8 { "failed" } else { "pending" })
                .bind(e.to_string().chars().take(500).collect::<String>())
                .bind(retry as f64)
                .execute(&s.pool)
                .await?;
        }
    }
    Ok(outcome_done || job.attempts > 0)
}
async fn poll(s: &AppState, feed: &Feed) -> Result<()> {
    let mut headers = HeaderMap::new();
    if let Some(etag) = &feed.etag {
        headers.insert(IF_NONE_MATCH, HeaderValue::from_str(etag)?);
    }
    if let Some(value) = &feed.last_modified {
        headers.insert(IF_MODIFIED_SINCE, HeaderValue::from_str(value)?);
    }
    let response = network::get(&feed.url, headers, true).await?;
    if response.status().as_u16() == 304 {
        return Ok(());
    }
    if !response.status().is_success() {
        bail!("Feed returned HTTP {}", response.status().as_u16());
    }
    let etag = response
        .headers()
        .get("etag")
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned);
    let modified = response
        .headers()
        .get("last-modified")
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned);
    let body = network::limited_body(response, 10 * 1024 * 1024).await?;
    let parsed = parser::parse(&body, &feed.url, &feed.name).context("Invalid RSS/Atom feed")?;
    let mut tx = s.pool.begin().await?;
    for entry in parsed.into_iter().rev() {
        let id = Uuid::new_v4();
        let inserted = sqlx::query("INSERT INTO entries(id,feed_id,guid,title,author,url,summary,media,published_at) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9) ON CONFLICT(feed_id,guid) DO NOTHING")
            .bind(id).bind(feed.id).bind(entry.guid).bind(entry.title).bind(entry.author).bind(entry.url).bind(entry.summary)
            .bind(json!(entry.media)).bind(entry.published_at).execute(&mut *tx).await?.rows_affected()>0;
        if inserted && feed.initialized {
            crate::community::worker::enqueue(&mut tx, feed.id, id).await?;
        }
    }
    sqlx::query("UPDATE feeds SET initialized=true,etag=$2,last_modified=$3 WHERE id=$1")
        .bind(feed.id)
        .bind(etag)
        .bind(modified)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(())
}
enum DeliveryOutcome {
    Done,
    Skipped,
}

async fn execute(s: &AppState, job: &Job) -> Result<DeliveryOutcome> {
    let mut entry: Entry = sqlx::query_as("SELECT e.id,e.feed_id,e.title,e.author,e.url,e.summary,e.media,e.published_at,f.name AS feed_name,f.source FROM entries e JOIN feeds f ON f.id=e.feed_id WHERE e.id=$1")
        .bind(job.entry_id).fetch_one(&s.pool).await?;
    if job.kind == "discord" {
        let webhook: Webhook = sqlx::query_as("SELECT * FROM webhooks WHERE id=$1")
            .bind(job.webhook_id)
            .fetch_one(&s.pool)
            .await?;
        if !webhook.enabled {
            bail!("Webhook is paused");
        }
        let community: Option<Uuid> =
            sqlx::query_scalar("SELECT community_id FROM webhooks WHERE id=$1")
                .bind(webhook.id)
                .fetch_one(&s.pool)
                .await?;
        if let Some(server) = community {
            entry.feed_name=sqlx::query_scalar("SELECT a.name FROM community_artists a JOIN communities c ON c.id=a.community_id WHERE a.community_id=$1 AND a.feed_id=$2 AND a.enabled AND c.enabled").bind(server).bind(entry.feed_id).fetch_optional(&s.pool).await?.context("Server artist announcements are paused or removed")?;
            if !webhook.include_tags.is_empty() || !webhook.exclude_tags.is_empty() {
                let tags: Vec<String> = sqlx::query_scalar(
                    "SELECT coalesce(tags,'{}'::text[]) FROM community_post_tags WHERE community_id=$1 AND entry_id=$2",
                )
                .bind(server)
                .bind(entry.id)
                .fetch_optional(&s.pool)
                .await?
                .unwrap_or_default();
                if !crate::webhooks::matches_tags(
                    &webhook.include_tags,
                    &webhook.exclude_tags,
                    &tags,
                ) {
                    return Ok(DeliveryOutcome::Skipped);
                }
            }
        }
        let mut payload = discord_payload(&entry, &webhook)?;
        if let Some(server) = community {
            crate::community::tags::decorate(s, server, entry.id, &mut payload).await?;
        }
        send_discord(&webhook, &payload).await?;
        Ok(DeliveryOutcome::Done)
    } else {
        download(s, &entry).await?;
        Ok(DeliveryOutcome::Done)
    }
}
#[derive(Debug)]
struct RateLimited(i32);
impl std::fmt::Display for RateLimited {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Webhook rate limit reached; retrying in {}s", self.0)
    }
}
impl std::error::Error for RateLimited {}
pub(crate) async fn send_discord(webhook: &Webhook, payload: &serde_json::Value) -> Result<()> {
    let mut url = network::webhook(&webhook.url)?;
    url.query_pairs_mut().append_pair("wait", "true");
    let client = network::public_webhook_client(&url).await?;
    let response = client
        .post(url)
        .json(payload)
        .send()
        .await
        .map_err(|e| e.without_url())?;
    if response.status().as_u16() == 429 {
        let body: serde_json::Value = response.json().await.unwrap_or_default();
        return Err(RateLimited(
            body["retry_after"]
                .as_f64()
                .unwrap_or(60.0)
                .ceil()
                .clamp(1.0, 86400.0) as i32,
        )
        .into());
    }
    if !response.status().is_success() {
        bail!("Webhook returned HTTP {}", response.status().as_u16());
    }
    Ok(())
}
fn extension(content_type: &str) -> Option<&'static str> {
    match content_type.split(';').next().unwrap_or("").trim() {
        "image/jpeg" => Some("jpg"),
        "image/png" => Some("png"),
        "image/gif" => Some("gif"),
        "image/webp" => Some("webp"),
        "image/avif" => Some("avif"),
        "video/mp4" => Some("mp4"),
        "video/webm" => Some("webm"),
        "video/quicktime" => Some("mov"),
        _ => None,
    }
}
async fn download(s: &AppState, entry: &Entry) -> Result<()> {
    let media: Vec<Media> = serde_json::from_value(entry.media.clone())?;
    for item in media {
        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM downloads WHERE entry_id=$1 AND media_url=$2)",
        )
        .bind(entry.id)
        .bind(&item.url)
        .fetch_one(&s.pool)
        .await?;
        if exists {
            continue;
        }
        let mut response = network::get(&item.url, HeaderMap::new(), false).await?;
        if !response.status().is_success() {
            bail!("Media returned HTTP {}", response.status().as_u16());
        }
        let content_type = response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_owned();
        let ext =
            extension(&content_type).context("Unsupported file type (images and videos only)")?;
        if response
            .content_length()
            .is_some_and(|len| len > s.max_download_bytes)
        {
            bail!("File exceeds the download limit");
        }
        let id = Uuid::new_v4();
        let filename = format!("{id}.{ext}");
        let temp = s.download_dir.join(format!("{id}.part"));
        let result = async {
            let mut file=tokio::fs::File::create(&temp).await?; let mut size=0u64;
            while let Some(chunk)=response.chunk().await.map_err(|e|e.without_url())? {
                size+=chunk.len() as u64; if size>s.max_download_bytes {bail!("File exceeds the download limit");}
                file.write_all(&chunk).await?;
            }
            file.sync_all().await?; tokio::fs::rename(&temp,s.download_dir.join(&filename)).await?;
            sqlx::query("INSERT INTO downloads(id,entry_id,media_url,filename,content_type,size_bytes) VALUES($1,$2,$3,$4,$5,$6)")
                .bind(id).bind(entry.id).bind(item.url).bind(&filename).bind(content_type).bind(size as i64).execute(&s.pool).await?;
            Ok::<_,anyhow::Error>(())
        }.await;
        if result.is_err() {
            let _ = tokio::fs::remove_file(&temp).await;
            let _ = tokio::fs::remove_file(s.download_dir.join(&filename)).await;
        }
        result?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn discord_payload_caps_images_and_text_and_disables_mentions() {
        let entry = Entry {
            id: Uuid::new_v4(),
            feed_id: Uuid::new_v4(),
            title: "あ".repeat(400),
            author: "Artist".into(),
            url: "https://example.com/post".into(),
            summary: "x".repeat(5000),
            media: json!(
                (0..8)
                    .map(|i| Media {
                        url: format!("https://example.com/{i}.png"),
                        kind: "image".into()
                    })
                    .collect::<Vec<_>>()
            ),
            published_at: chrono::Utc::now(),
            feed_name: "Art".into(),
            source: "pixiv".into(),
        };
        let mut webhook = Webhook {
            category_filter: None,
            category_filters: None,
            include_tags: Vec::new(),
            exclude_tags: Vec::new(),
            id: Uuid::new_v4(),
            name: "Art channel".into(),
            url: String::new(),
            username: "Archive bot".into(),
            avatar_url: "https://example.com/avatar.png".into(),
            embed_color: 0x123456,
            enabled: true,
            created_at: chrono::Utc::now(),
        };
        let payload = discord_payload(&entry, &webhook).unwrap();
        assert_eq!(payload["username"], "Archive bot");
        assert_eq!(payload["avatar_url"], webhook.avatar_url);
        assert_eq!(payload["embeds"][0]["color"], 0x123456);
        webhook.username.clear();
        webhook.avatar_url.clear();
        let default_payload = discord_payload(&entry, &webhook).unwrap();
        assert!(default_payload.get("username").is_none());
        assert!(default_payload.get("avatar_url").is_none());
        assert_eq!(payload["embeds"].as_array().unwrap().len(), 4);
        assert_eq!(
            payload["embeds"][0]["title"]
                .as_str()
                .unwrap()
                .chars()
                .count(),
            256
        );
        assert_eq!(
            payload["embeds"][0]["description"].as_str().unwrap().len(),
            1800
        );
        assert_eq!(payload["allowed_mentions"]["parse"], json!([]));
        assert_eq!(payload["embeds"][0]["url"], entry.url);
    }
    #[test]
    fn error_backoff_doubles_poll_interval_up_to_cap() {
        assert_eq!(backoff_minutes(15, 0), 15);
        assert_eq!(backoff_minutes(15, 1), 15);
        assert_eq!(backoff_minutes(15, 2), 30);
        assert_eq!(backoff_minutes(15, 3), 60);
        assert_eq!(backoff_minutes(15, 4), 120);
        assert_eq!(backoff_minutes(15, 5), 240);
        assert_eq!(backoff_minutes(30, 5), 360);
        assert_eq!(backoff_minutes(30, 12), 360);
    }

    #[test]
    fn executable_and_html_downloads_are_not_allowed() {
        assert_eq!(extension("image/png; charset=binary"), Some("png"));
        assert_eq!(extension("application/x-executable"), None);
        assert_eq!(extension("text/html"), None);
        assert_eq!(extension("image/svg+xml"), None);
    }
}
