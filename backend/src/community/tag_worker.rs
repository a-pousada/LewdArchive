//! AI has a separate queue so inference cannot block feed ingestion or downloads.
use crate::AppState;
use anyhow::Result;
use serde_json::Value;
use sqlx::FromRow;
use uuid::Uuid;

#[derive(FromRow)]
struct Job {
    community_id: Uuid,
    entry_id: Uuid,
    artist_id: Uuid,
    attempts: i32,
}

pub async fn run(s: AppState) {
    loop {
        match tick(&s).await {
            Ok(true) => tokio::time::sleep(std::time::Duration::from_millis(50)).await,
            Ok(false) => {
                tokio::select! {
                    _ = tokio::time::sleep(std::time::Duration::from_secs(5)) => {}
                    _ = s.wake.notified() => {}
                }
            }
            Err(error) => {
                tracing::error!(%error, "AI tagging cycle failed");
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            }
        }
    }
}

pub(super) async fn tick(s: &AppState) -> Result<bool> {
    let Some(tagger) = &s.tagger else {
        return Ok(false);
    };
    sqlx::query("UPDATE community_tag_jobs SET status=CASE WHEN attempts>=3 THEN 'failed' ELSE 'pending' END,last_error='AI worker was interrupted',locked_at=NULL WHERE status='processing' AND locked_at<now()-interval '5 minutes'").execute(&s.pool).await?;
    let job: Option<Job> = sqlx::query_as("UPDATE community_tag_jobs SET status='processing',locked_at=now(),attempts=attempts+1 WHERE (community_id,entry_id)=(SELECT j.community_id,j.entry_id FROM community_tag_jobs j JOIN communities c ON c.id=j.community_id WHERE j.status='pending' AND j.next_attempt_at<=now() AND c.ai_tags_enabled AND c.enabled ORDER BY j.next_attempt_at FOR UPDATE OF j SKIP LOCKED LIMIT 1) RETURNING community_id,entry_id,artist_id,attempts").fetch_optional(&s.pool).await?;
    let Some(job) = job else {
        return Ok(false);
    };
    let row: Option<(Value,)> = sqlx::query_as("SELECT e.media FROM entries e JOIN community_artists a ON a.feed_id=e.feed_id AND a.community_id=$1 WHERE e.id=$2").bind(job.community_id).bind(job.entry_id).fetch_optional(&s.pool).await?;
    let Some((media,)) = row else {
        return Ok(true);
    }; // Cascading deletion removes the job too.
    match tagger.suggest(&media).await {
        Ok(tags) => {
            store(s, job.community_id, job.entry_id, job.artist_id, &tags).await?;
        }
        Err(error) => {
            sqlx::query("UPDATE community_tag_jobs SET status=$3,last_error=$4,locked_at=NULL,next_attempt_at=now()+interval '30 seconds' WHERE community_id=$1 AND entry_id=$2 AND status='processing'")
                .bind(job.community_id).bind(job.entry_id).bind(if job.attempts >= 3 {"failed"} else {"pending"}).bind(error.to_string().chars().take(300).collect::<String>()).execute(&s.pool).await?;
        }
    }
    s.wake.notify_one();
    Ok(true)
}

/// A manual edit (including an intentional empty tag list) always wins the race.
pub(super) async fn store(
    s: &AppState,
    server: Uuid,
    entry: Uuid,
    artist: Uuid,
    tags: &[String],
) -> Result<()> {
    let mut tx = s.pool.begin().await?;
    sqlx::query("INSERT INTO community_post_tags(community_id,entry_id,artist_id,tags,source) SELECT a.community_id,e.id,a.id,$4,'ai' FROM community_artists a JOIN entries e ON e.feed_id=a.feed_id JOIN communities c ON c.id=a.community_id WHERE a.community_id=$1 AND e.id=$2 AND a.id=$3 AND c.ai_tags_enabled AND EXISTS(SELECT 1 FROM community_tag_jobs j WHERE j.community_id=$1 AND j.entry_id=$2 AND j.status='processing') ON CONFLICT(community_id,entry_id) DO NOTHING")
        .bind(server).bind(entry).bind(artist).bind(tags).execute(&mut *tx).await?;
    sqlx::query("UPDATE community_tag_jobs SET status='done',last_error=NULL,locked_at=NULL WHERE community_id=$1 AND entry_id=$2 AND status='processing'").bind(server).bind(entry).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(())
}
