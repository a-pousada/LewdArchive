use crate::{AppState, models::Entry, platforms::RetryAfter};
use anyhow::{Context, Result};
use serde_json::Value;
use sqlx::FromRow;
use std::time::Duration;
use uuid::Uuid;

pub async fn enqueue(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    feed: Uuid,
    entry: Uuid,
) -> Result<()> {
    sqlx::query("INSERT INTO community_tag_jobs(community_id,entry_id,artist_id) SELECT a.community_id,$2,a.id FROM community_artists a JOIN communities c ON c.id=a.community_id WHERE a.feed_id=$1 AND c.ai_tags_enabled ON CONFLICT DO NOTHING").bind(feed).bind(entry).execute(&mut **tx).await?;
    let webhooks:Vec<Uuid>=sqlx::query_scalar("SELECT w.id FROM webhooks w JOIN community_artists a ON a.community_id=w.community_id WHERE a.feed_id=$1 AND a.enabled AND w.enabled AND (w.category_filter IS NULL OR w.category_filter=a.category) ORDER BY w.id FOR SHARE OF w,a").bind(feed).fetch_all(&mut **tx).await?;
    for webhook in webhooks {
        sqlx::query("INSERT INTO jobs(id,entry_id,kind,webhook_id) VALUES($1,$2,'discord',$3) ON CONFLICT ON CONSTRAINT jobs_delivery_key DO NOTHING").bind(Uuid::new_v4()).bind(entry).bind(webhook).execute(&mut **tx).await?;
    }
    let routes:Vec<(Uuid,Uuid,String,String,bool)>=sqlx::query_as("SELECT a.id,c.id,c.platform,coalesce(r.channel_id,c.channel_id),c.mentions_enabled FROM community_artists a JOIN communities c ON c.id=a.community_id LEFT JOIN community_category_routes r ON r.community_id=c.id AND r.category=a.category WHERE a.feed_id=$1 AND a.enabled AND coalesce(r.channel_id,c.channel_id) IS NOT NULL ORDER BY a.id FOR KEY SHARE OF a,c")
        .bind(feed).fetch_all(&mut **tx).await?;
    for (artist, community, platform, channel, mentions) in routes {
        let recipients: Vec<String> = if mentions {
            sqlx::query_scalar("SELECT u.external_id FROM artist_preferences p JOIN community_users u ON u.id=p.user_id WHERE p.artist_id=$1 AND p.notify AND u.platform=$2 ORDER BY u.external_id").bind(artist).bind(platform).fetch_all(&mut **tx).await?
        } else {
            Vec::new()
        };
        let chunks: Vec<&[String]> = if recipients.is_empty() {
            vec![&[]]
        } else {
            recipients.chunks(50).collect()
        };
        for (batch, recipients) in chunks.into_iter().enumerate() {
            sqlx::query("INSERT INTO community_deliveries(id,community_id,artist_id,entry_id,channel_id,recipient_ids,batch) VALUES($1,$2,$3,$4,$5,$6,$7) ON CONFLICT(community_id,entry_id,batch) DO NOTHING")
                .bind(Uuid::new_v4()).bind(community).bind(artist).bind(entry).bind(&channel).bind(recipients).bind(batch as i32).execute(&mut **tx).await?;
        }
    }
    Ok(())
}
pub async fn run(s: AppState) {
    loop {
        if let Err(e) = tick(&s).await {
            tracing::error!(error=%e,"community worker cycle failed");
        }
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
}
async fn tick(s: &AppState) -> Result<()> {
    sqlx::query("DELETE FROM oauth_states WHERE expires_at<now()")
        .execute(&s.pool)
        .await?;
    sqlx::query("DELETE FROM community_sessions WHERE expires_at<now()")
        .execute(&s.pool)
        .await?;
    sqlx::query("DELETE FROM interaction_jobs WHERE expires_at<now()")
        .execute(&s.pool)
        .await?;
    if s.discord.is_none() {
        return Ok(());
    }
    sqlx::query("UPDATE interaction_jobs SET status='pending' WHERE status='processing' AND locked_at<now()-interval '90 seconds'").execute(&s.pool).await?;
    for _ in 0..5 {
        if !interaction(s).await? {
            break;
        }
    }
    sqlx::query("UPDATE community_deliveries SET status='pending' WHERE status='processing' AND locked_at<now()-interval '15 minutes'").execute(&s.pool).await?;
    for _ in 0..5 {
        if !delivery(s).await? {
            break;
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    Ok(())
}
#[derive(FromRow)]
struct Interaction {
    external_id: String,
    payload: Value,
    result: Option<Value>,
    attempts: i32,
}
async fn interaction(s: &AppState) -> Result<bool> {
    let job:Option<Interaction>=sqlx::query_as("UPDATE interaction_jobs SET status='processing',locked_at=now(),attempts=attempts+1 WHERE external_id=(SELECT external_id FROM interaction_jobs WHERE status='pending' AND next_attempt_at<=now() AND expires_at>now() ORDER BY next_attempt_at FOR UPDATE SKIP LOCKED LIMIT 1) RETURNING external_id,payload,result,attempts").fetch_optional(&s.pool).await?;
    let Some(job) = job else {
        return Ok(false);
    };
    let result = match job.result {
        Some(result) => result,
        None => {
            let response = match tokio::time::timeout(
                Duration::from_secs(45),
                super::interactions::process(s, &job.payload),
            )
            .await
            {
                Ok(Ok(value)) => value,
                Ok(Err(e)) => super::interactions::private(&e.1),
                Err(_) => super::interactions::private("This request timed out. Please try again."),
            };
            // Explicit set/unset operations are idempotent if interrupted before this write.
            sqlx::query("UPDATE interaction_jobs SET result=$2 WHERE external_id=$1")
                .bind(&job.external_id)
                .bind(&response)
                .execute(&s.pool)
                .await?;
            response
        }
    };
    let d = s.discord.as_ref().context("Discord unavailable")?;
    match d
        .edit_response(job.payload["token"].as_str().unwrap_or(""), &result)
        .await
    {
        Ok(()) => {
            sqlx::query("UPDATE interaction_jobs SET status='done',payload='{}',result=NULL WHERE external_id=$1").bind(&job.external_id).execute(&s.pool).await?;
        }
        Err(e) => {
            let delay = e
                .downcast_ref::<RetryAfter>()
                .map(|r| r.0)
                .unwrap_or(5 * job.attempts as u64);
            sqlx::query("UPDATE interaction_jobs SET status=$2,next_attempt_at=now()+make_interval(secs=>$3) WHERE external_id=$1").bind(&job.external_id).bind(if job.attempts>=5{"failed"}else{"pending"}).bind(delay as f64).execute(&s.pool).await?;
        }
    }
    Ok(true)
}
#[derive(FromRow)]
struct Delivery {
    id: Uuid,
    community_id: Uuid,
    artist_id: Uuid,
    entry_id: Uuid,
    channel_id: String,
    recipient_ids: Vec<String>,
    attempts: i32,
}
pub(super) async fn delivery(s: &AppState) -> Result<bool> {
    let job:Option<Delivery>=sqlx::query_as("UPDATE community_deliveries SET status='processing',attempts=attempts+1,locked_at=now() WHERE id=(SELECT d.id FROM community_deliveries d JOIN communities c ON c.id=d.community_id JOIN community_artists a ON a.id=d.artist_id WHERE d.status='pending' AND d.next_attempt_at<=now() AND c.enabled AND a.enabled AND (NOT $1 OR NOT c.ai_tags_enabled OR NOT EXISTS(SELECT 1 FROM community_tag_jobs t WHERE t.community_id=c.id AND t.entry_id=d.entry_id AND t.status IN('pending','processing') AND t.created_at>now()-interval '5 minutes')) AND (SELECT notifications_enabled FROM settings) ORDER BY d.next_attempt_at FOR UPDATE OF d SKIP LOCKED LIMIT 1) RETURNING id,community_id,artist_id,entry_id,channel_id,recipient_ids,attempts")
        .bind(s.tagger.is_some()).fetch_optional(&s.pool).await?;
    let Some(job) = job else {
        return Ok(false);
    };
    let sent = tokio::time::timeout(Duration::from_secs(30), send(s, &job))
        .await
        .unwrap_or_else(|_| Err(anyhow::anyhow!("Bot delivery timed out")));
    match sent {
        Ok(message) => {
            sqlx::query("UPDATE community_deliveries SET status='done',last_error=NULL,message_id=$2 WHERE id=$1").bind(job.id).bind(message).execute(&s.pool).await?;
        }
        Err(e) => {
            let delay = e
                .downcast_ref::<RetryAfter>()
                .map(|r| r.0)
                .unwrap_or((30 * 2u64.pow(job.attempts.min(7) as u32)).min(3600));
            sqlx::query("UPDATE community_deliveries SET status=$2,last_error=$3,next_attempt_at=now()+make_interval(secs=>$4) WHERE id=$1").bind(job.id).bind(if job.attempts>=8{"failed"}else{"pending"}).bind(e.to_string().chars().take(300).collect::<String>()).bind(delay as f64).execute(&s.pool).await?;
        }
    }
    Ok(true)
}
async fn send(s: &AppState, job: &Delivery) -> Result<String> {
    let entry:Entry=sqlx::query_as("SELECT e.id,e.feed_id,e.title,e.author,e.url,e.summary,e.media,e.published_at,a.name AS feed_name,f.source FROM entries e JOIN feeds f ON f.id=e.feed_id JOIN community_artists a ON a.feed_id=f.id AND a.id=$2 WHERE e.id=$1 AND a.community_id=$3")
        .bind(job.entry_id).bind(job.artist_id).bind(job.community_id).fetch_one(&s.pool).await?;
    let platform: String = sqlx::query_scalar("SELECT platform FROM communities WHERE id=$1")
        .bind(job.community_id)
        .fetch_one(&s.pool)
        .await?;
    // Re-check opt-outs and the server switch at send time; never widen the snapshot.
    let recipients:Vec<String>=sqlx::query_scalar("SELECT u.external_id FROM artist_preferences p JOIN community_users u ON u.id=p.user_id JOIN community_artists a ON a.id=p.artist_id JOIN communities c ON c.id=a.community_id WHERE p.artist_id=$1 AND p.notify AND c.mentions_enabled AND u.platform=c.platform AND u.external_id=ANY($2)")
        .bind(job.artist_id).bind(&job.recipient_ids).fetch_all(&s.pool).await?;
    let adapter = s
        .platforms
        .get(&platform)
        .context("Platform is not configured")?;
    let mut payload = adapter.notification(
        &entry,
        job.artist_id,
        &recipients,
        &job.id.simple().to_string(),
    )?;
    super::tags::decorate(s, job.community_id, job.entry_id, &mut payload).await?;
    adapter.send(&job.channel_id, payload).await
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::platforms::discord::bot_payload;
    use serde_json::json;
    #[test]
    fn bot_mentions_are_explicit_and_bounded() {
        let e = Entry {
            id: Uuid::new_v4(),
            feed_id: Uuid::new_v4(),
            title: "@everyone".into(),
            author: "Artist".into(),
            url: "https://example.com/post".into(),
            summary: String::new(),
            media: json!([]),
            published_at: chrono::Utc::now(),
            feed_name: "Art".into(),
            source: "rss".into(),
        };
        let mut ids: Vec<_> = (0..120).map(|i| i.to_string()).collect();
        ids.insert(0, "@everyone".into());
        let p = bot_payload(&e, Uuid::new_v4(), &ids, "abcdef").unwrap();
        assert_eq!(p["allowed_mentions"]["parse"], json!([]));
        assert_eq!(p["components"][0]["components"][1]["emoji"]["name"], "🔖");
        assert_eq!(p["components"][0]["components"][2]["emoji"]["name"], "⭐");
        assert_eq!(p["allowed_mentions"]["users"].as_array().unwrap().len(), 50);
        assert!(!p["content"].as_str().unwrap().contains("everyone"));
        assert_eq!(
            p["components"][0]["components"].as_array().unwrap().len(),
            4
        );
    }
}
