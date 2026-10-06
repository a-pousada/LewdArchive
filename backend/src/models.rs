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
    pub enabled: bool,
    pub auto_download: bool,
    pub notify: bool,
    pub poll_minutes: i32,
    pub initialized: bool,
    #[serde(skip)]
    pub etag: Option<String>,
    #[serde(skip)]
    pub last_modified: Option<String>,
    pub last_checked_at: Option<DateTime<Utc>>,
    pub next_check_at: DateTime<Utc>,
    pub last_error: Option<String>,
    pub created_at: DateTime<Utc>,
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
    pub is_read: bool,
    pub favorite: bool,
    pub feed_name: String,
    pub source: String,
}
#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct Media {
    pub url: String,
    pub kind: String,
}
#[derive(FromRow)]
pub struct Settings {
    pub webhook_url: String,
    pub downloads_enabled: bool,
    pub notifications_enabled: bool,
}
#[derive(Serialize, FromRow)]
pub struct Job {
    pub id: Uuid,
    pub entry_id: Uuid,
    pub kind: String,
    pub status: String,
    pub attempts: i32,
    pub last_error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
