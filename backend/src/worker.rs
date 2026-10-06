use crate::{
    AppState,
    models::{Entry, Feed, Job, Media, Settings},
    network, parser,
};
use anyhow::{Context, Result, bail};
use reqwest::header::{HeaderMap, HeaderValue, IF_MODIFIED_SINCE, IF_NONE_MATCH};
use serde_json::json;
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use uuid::Uuid;

pub async fn run(s: AppState) {
    loop {
        if let Err(e) = tick(&s).await {
            tracing::error!(error=%e,"worker cycle failed");
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
async fn cycle(s: &AppState) -> Result<()> {
    let feeds: Vec<Feed> = sqlx::query_as("SELECT * FROM feeds WHERE enabled AND next_check_at<=now() ORDER BY next_check_at LIMIT 10").fetch_all(&s.pool).await?;
    for feed in feeds {
        let result = poll(s, &feed).await;
        let error = result
            .err()
            .map(|e| e.to_string().chars().take(500).collect::<String>());
        sqlx::query("UPDATE feeds SET last_checked_at=now(),next_check_at=now()+make_interval(mins=>poll_minutes),last_error=$2 WHERE id=$1")
            .bind(feed.id).bind(error).execute(&s.pool).await?;
    }
    sqlx::query("UPDATE jobs SET status='pending',next_attempt_at=now() WHERE status='processing' AND locked_at<now()-interval '15 minutes'").execute(&s.pool).await?;
    let settings: Settings =
        sqlx::query_as("SELECT webhook_url,downloads_enabled,notifications_enabled FROM settings")
            .fetch_one(&s.pool)
            .await?;
    for _ in 0..10 {
        let job: Option<Job> = sqlx::query_as("UPDATE jobs SET status='processing',locked_at=now(),attempts=attempts+1,updated_at=now() WHERE id=(SELECT id FROM jobs WHERE status='pending' AND next_attempt_at<=now() AND ((kind='download' AND $1) OR (kind='discord' AND $2)) ORDER BY next_attempt_at FOR UPDATE SKIP LOCKED LIMIT 1) RETURNING id,entry_id,kind,status,attempts,last_error,created_at,updated_at")
            .bind(settings.downloads_enabled).bind(settings.notifications_enabled && !settings.webhook_url.is_empty()).fetch_optional(&s.pool).await?;
        let Some(job) = job else { break };
        let result = tokio::time::timeout(Duration::from_secs(600), execute(s, &settings, &job))
            .await
            .unwrap_or_else(|_| Err(anyhow::anyhow!("Job exceeded the 10-minute time limit")));
        match result {
            Ok(()) => {
                sqlx::query(
                    "UPDATE jobs SET status='done',last_error=NULL,updated_at=now() WHERE id=$1",
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
                    .bind(job.id).bind(if job.attempts>=8 {"failed"} else {"pending"})
                    .bind(e.to_string().chars().take(500).collect::<String>()).bind(retry as f64).execute(&s.pool).await?;
            }
        }
        tokio::time::sleep(Duration::from_millis(600)).await;
    }
    Ok(())
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
            for (kind, enabled) in [
                ("discord", feed.notify),
                ("download", feed.auto_download && !entry.media.is_empty()),
            ] {
                if enabled {
                    sqlx::query("INSERT INTO jobs(id,entry_id,kind) VALUES($1,$2,$3)")
                        .bind(Uuid::new_v4())
                        .bind(id)
                        .bind(kind)
                        .execute(&mut *tx)
                        .await?;
                }
            }
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
async fn execute(s: &AppState, settings: &Settings, job: &Job) -> Result<()> {
    let entry: Entry = sqlx::query_as("SELECT e.id,e.feed_id,e.title,e.author,e.url,e.summary,e.media,e.published_at,e.is_read,e.favorite,f.name AS feed_name,f.source FROM entries e JOIN feeds f ON f.id=e.feed_id WHERE e.id=$1")
        .bind(job.entry_id).fetch_one(&s.pool).await?;
    if job.kind == "discord" {
        send_discord(&settings.webhook_url, &entry).await
    } else {
        download(s, &entry).await
    }
}
#[derive(Debug)]
struct RateLimited(i32);
impl std::fmt::Display for RateLimited {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Discord rate limit reached; retrying in {}s", self.0)
    }
}
impl std::error::Error for RateLimited {}
fn discord_payload(e: &Entry) -> Result<serde_json::Value> {
    let media: Vec<Media> = serde_json::from_value(e.media.clone())?;
    let mut embed = json!({"title":e.title.chars().take(256).collect::<String>(),"url":e.url,
        "description":e.summary.chars().take(1800).collect::<String>(),"color":14978709,
        "author":{"name":e.author.chars().take(256).collect::<String>()},"timestamp":e.published_at.to_rfc3339(),
        "footer":{"text":format!("LewdArchive · {}",e.feed_name).chars().take(256).collect::<String>()}});
    if let Some(image) = media.iter().find(|m| m.kind == "image") {
        embed["image"] = json!({"url":image.url});
    }
    let mut embeds = vec![embed];
    for image in media.iter().filter(|m| m.kind == "image").skip(1).take(3) {
        embeds.push(json!({"url":e.url,"image":{"url":image.url}}));
    }
    let attachments = media
        .iter()
        .filter(|m| m.kind == "video")
        .take(3)
        .enumerate()
        .map(|(i, m)| format!("[Video {}]({})", i + 1, m.url))
        .collect::<Vec<_>>()
        .join(" · ");
    if !attachments.is_empty() {
        embeds[0]["fields"] =
            json!([{"name":"Videos","value":attachments.chars().take(1024).collect::<String>()}]);
    }
    Ok(json!({"username":"LewdArchive","allowed_mentions":{"parse":[]},"embeds":embeds}))
}
async fn send_discord(webhook: &str, e: &Entry) -> Result<()> {
    let mut url = network::webhook(webhook)?;
    url.query_pairs_mut().append_pair("wait", "true");
    let response = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(30))
        .build()?
        .post(url)
        .json(&discord_payload(e)?)
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
        bail!("Discord returned HTTP {}", response.status().as_u16());
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
            is_read: false,
            favorite: false,
            feed_name: "Art".into(),
            source: "pixiv".into(),
        };
        let payload = discord_payload(&entry).unwrap();
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
    fn executable_and_html_downloads_are_not_allowed() {
        assert_eq!(extension("image/png; charset=binary"), Some("png"));
        assert_eq!(extension("application/x-executable"), None);
        assert_eq!(extension("text/html"), None);
        assert_eq!(extension("image/svg+xml"), None);
    }
}
