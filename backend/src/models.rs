use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Serialize, FromRow)]
pub struct Feed {
    pub id: Uuid,
    pub name: String,
    pub url: String,
    pub source: String,
    pub category: String,
    pub site_url: String,
    pub enabled: bool,
    pub initialized: bool,
    #[serde(skip)]
    pub etag: Option<String>,
    #[serde(skip)]
    pub last_modified: Option<String>,
}
#[derive(Debug, Serialize, FromRow)]
pub struct Entry {
    pub id: Uuid,
    pub feed_id: Uuid,
    pub title: String,
    pub author: String,
    pub url: String,
    pub summary: String,
    pub media: serde_json::Value,
    pub published_at: DateTime<Utc>,
    pub feed_name: String,
    pub source: String,
}
#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct Media {
    pub url: String,
    pub kind: String,
}
#[derive(Serialize, FromRow)]
pub struct Settings {
    pub downloads_enabled: bool,
    pub notifications_enabled: bool,
}
#[derive(Serialize, FromRow)]
pub struct Job {
    pub id: Uuid,
    pub entry_id: Uuid,
    pub kind: String,
    pub webhook_id: Option<Uuid>,
    pub status: String,
    pub attempts: i32,
    pub last_error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Serialize, FromRow)]
pub struct Webhook {
    pub category_filter: Option<String>,
    pub id: Uuid,
    pub name: String,
    #[serde(skip)]
    pub url: String,
    pub username: String,
    pub avatar_url: String,
    pub embed_color: i32,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
}
