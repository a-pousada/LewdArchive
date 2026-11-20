//! Rankings use publication windows and current aggregate engagement, never saver identities.
use crate::{
    AppState,
    api::{Result, bad},
};
use serde::Deserialize;
use serde_json::{Value, json};
use uuid::Uuid;

#[derive(Clone, Copy, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Period {
    #[default]
    Week,
    Month,
    Year,
}
impl Period {
    pub fn days(self) -> i32 {
        match self {
            Self::Week => 7,
            Self::Month => 30,
            Self::Year => 365,
        }
    }
}
#[derive(Clone, Copy, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Metric {
    #[default]
    Likes,
    Saves,
    Engagement,
}
impl Metric {
    fn column(self) -> &'static str {
        match self {
            Self::Likes => "likes",
            Self::Saves => "saves",
            Self::Engagement => "score",
        }
    }
}
#[derive(Default, Deserialize)]
pub struct Filter {
    #[serde(default)]
    pub period: Period,
    #[serde(default)]
    pub metric: Metric,
    #[serde(default)]
    pub page: i64,
}
pub async fn posts(
    s: &AppState,
    server: Uuid,
    user: Uuid,
    q: &Filter,
    limit: i64,
) -> Result<Value> {
    if q.page < 0 || q.page > 100000 {
        return Err(bad("Invalid ranking page"));
    }
    let page = q.page.max(1);
    let limit = limit.clamp(1, 24);
    // All counts and joins are server-scoped. One snapshot keeps pagination and totals consistent.
    let sql = format!(
        r#"
WITH candidates AS (
 SELECT e.*, a.id AS artist_id,a.name AS feed_name,a.avatar_url,a.official_links
 FROM entries e JOIN community_artists a ON a.feed_id=e.feed_id
 WHERE a.community_id=$1 AND e.published_at >= now()-make_interval(days=>$3)
 AND e.published_at <= now()
), counts AS (
 SELECT p.entry_id,count(*) FILTER(WHERE p.liked) AS likes,
 count(*) FILTER(WHERE p.favorite) AS saves
 FROM post_preferences p JOIN candidates c ON c.id=p.entry_id
 WHERE p.community_id=$1 AND (p.liked OR p.favorite) GROUP BY p.entry_id
), ranked AS (
 SELECT c.*,n.likes,n.saves,n.likes+n.saves AS score
 FROM candidates c JOIN counts n ON n.entry_id=c.id
), eligible AS (SELECT * FROM ranked WHERE {metric}>0),
page_rows AS (
 SELECT r.*,coalesce(p.liked,false) AS viewer_liked,coalesce(p.favorite,false) AS viewer_favorite,
 row_number() OVER(ORDER BY r.{metric} DESC,r.published_at DESC,r.id DESC) AS rank
 FROM eligible r LEFT JOIN post_preferences p ON p.community_id=$1 AND p.entry_id=r.id AND p.user_id=$2
 ORDER BY r.{metric} DESC,r.published_at DESC,r.id DESC LIMIT $4 OFFSET $5
)
SELECT jsonb_build_object(
 'items',coalesce((SELECT jsonb_agg(jsonb_build_object(
 'id',id,'artist_id',artist_id,'title',title,'author',author,'url',url,'summary',summary,'media',media,
 'published_at',published_at,'feed_name',feed_name,'artist_avatar',avatar_url,'official_links',official_links,
 'liked',viewer_liked,'favorite',viewer_favorite,'likes',likes,'saves',saves,'score',score,'rank',rank) ORDER BY rank)
 FROM page_rows),'[]'::jsonb),
 'total',(SELECT count(*) FROM eligible),
 'summary',jsonb_build_object('posts',(SELECT count(*) FROM candidates),'likes',coalesce((SELECT sum(likes) FROM ranked),0),
 'saves',coalesce((SELECT sum(saves) FROM ranked),0),'artists',(SELECT count(DISTINCT artist_id) FROM candidates)))
"#,
        metric = q.metric.column()
    );
    let mut result: Value = sqlx::query_scalar(&sql)
        .bind(server)
        .bind(user)
        .bind(q.period.days())
        .bind(limit)
        .bind((page - 1) * limit)
        .fetch_one(&s.pool)
        .await?;
    let total = result["total"].as_i64().unwrap_or(0);
    result["page"] = json!(page);
    result["pages"] = json!((total + limit - 1) / limit);
    result["days"] = json!(q.period.days());
    Ok(result)
}
