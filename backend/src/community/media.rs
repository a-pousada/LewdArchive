//! Shared media storage, authorized through the current server subscription.
use super::{User, service};
use crate::{
    AppState,
    api::{Result, bad, not_found},
};
use axum::{
    Extension, Json,
    extract::{Path, State},
    http::header,
    response::{IntoResponse, Response},
};
use serde_json::{Value, json};
use uuid::Uuid;
async fn authorize(s: &AppState, user: &User, server: Uuid, post: Uuid, admin: bool) -> Result<()> {
    service::authorize(s, user, server, admin).await?;
    let exists:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM entries e JOIN community_artists a ON a.feed_id=e.feed_id WHERE a.community_id=$1 AND e.id=$2)").bind(server).bind(post).fetch_one(&s.pool).await?;
    if !exists {
        return Err(not_found());
    }
    Ok(())
}
pub async fn list(
    State(s): State<AppState>,
    Extension(user): Extension<User>,
    Path((server, post)): Path<(Uuid, Uuid)>,
) -> Result<Json<Vec<Value>>> {
    authorize(&s, &user, server, post, false).await?;
    Ok(Json(sqlx::query_scalar("SELECT jsonb_build_object('id',id,'filename',filename,'size_bytes',size_bytes) FROM downloads WHERE entry_id=$1 ORDER BY created_at,id").bind(post).fetch_all(&s.pool).await?))
}
pub async fn queue(
    State(s): State<AppState>,
    Extension(user): Extension<User>,
    Path((server, post)): Path<(Uuid, Uuid)>,
) -> Result<Json<Value>> {
    authorize(&s, &user, server, post, true).await?;
    let media: bool =
        sqlx::query_scalar("SELECT jsonb_array_length(media)>0 FROM entries WHERE id=$1")
            .bind(post)
            .fetch_one(&s.pool)
            .await?;
    if !media {
        return Err(bad("This post has no downloadable media"));
    }
    sqlx::query("INSERT INTO jobs(id,entry_id,kind) VALUES($1,$2,'download') ON CONFLICT ON CONSTRAINT jobs_delivery_key DO UPDATE SET status='pending',attempts=0,next_attempt_at=now(),last_error=NULL WHERE jobs.status IN('failed','done')").bind(Uuid::new_v4()).bind(post).execute(&s.pool).await?;
    s.wake.notify_one();
    Ok(Json(json!({"ok":true})))
}
pub async fn file(
    State(s): State<AppState>,
    Extension(user): Extension<User>,
    Path((server, id)): Path<(Uuid, Uuid)>,
) -> Result<Response> {
    service::authorize(&s, &user, server, false).await?;
    let filename:String=sqlx::query_scalar("SELECT d.filename FROM downloads d JOIN entries e ON e.id=d.entry_id JOIN community_artists a ON a.feed_id=e.feed_id WHERE d.id=$1 AND a.community_id=$2").bind(id).bind(server).fetch_optional(&s.pool).await?.ok_or_else(not_found)?;
    let file = tokio::fs::File::open(s.download_dir.join(&filename))
        .await
        .map_err(|_| not_found())?;
    let stream = futures_util::stream::try_unfold(file, |mut file| async move {
        use tokio::io::AsyncReadExt;
        let mut buf = vec![0; 65536];
        let n = file.read(&mut buf).await?;
        if n == 0 {
            Ok::<_, std::io::Error>(None)
        } else {
            buf.truncate(n);
            Ok(Some((buf, file)))
        }
    });
    Ok((
        [
            (header::CONTENT_TYPE, "application/octet-stream".to_owned()),
            (
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{filename}\""),
            ),
        ],
        axum::body::Body::from_stream(stream),
    )
        .into_response())
}
