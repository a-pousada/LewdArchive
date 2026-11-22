pub mod api;
pub mod auth;
pub mod categories;
pub mod comments;
pub mod interactions;
pub mod media;
pub mod profiles;
pub mod service;
pub mod tag_worker;
pub mod tags;
pub mod trending;
pub mod webhooks;
pub mod worker;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;
#[derive(Clone, Serialize, FromRow)]
pub struct User {
    pub id: Uuid,
    pub platform: String,
    pub external_id: String,
    pub display_name: String,
    pub avatar_url: String,
}
#[derive(Clone, Serialize, FromRow)]
pub struct Server {
    pub id: Uuid,
    pub platform: String,
    pub external_id: String,
    pub name: String,
    pub channel_id: Option<String>,
    pub channel_name: String,
    pub enabled: bool,
    pub mentions_enabled: bool,
    pub ai_tags_enabled: bool,
}
#[derive(Deserialize)]
pub struct ArtistInput {
    pub name: String,
    pub url: String,
    #[serde(default = "rss")]
    pub source: String,
    #[serde(default)]
    pub category: String,
}
fn rss() -> String {
    "rss".into()
}
#[derive(Deserialize, Default)]
pub struct Preference {
    pub favorite: Option<bool>,
    pub liked: Option<bool>,
    pub notify: Option<bool>,
}

pub fn routes(state: crate::AppState) -> axum::Router<crate::AppState> {
    use axum::{
        Router, middleware,
        routing::{delete, get, post},
    };
    let private = Router::new()
        .route("/me", get(api::me))
        .route("/feed-options", get(crate::api::feed_options))
        .route("/servers", get(api::servers).post(api::connect))
        .route("/servers/{server}", get(api::server).put(api::settings))
        .route(
            "/servers/{server}/artists",
            get(api::artists).post(api::track),
        )
        .route(
            "/servers/{server}/artists/bulk-category",
            post(categories::bulk_assign),
        )
        .route(
            "/servers/{server}/artists/{artist}",
            delete(api::untrack).patch(api::artist_preference),
        )
        .route(
            "/servers/{server}/artists/{artist}/delivery",
            axum::routing::put(api::artist_delivery),
        )
        .route(
            "/servers/{server}/artists/{artist}/sync",
            post(api::artist_sync),
        )
        .route("/servers/{server}/trending", get(api::trending))
        .route(
            "/servers/{server}/artists/{artist}/profile",
            axum::routing::put(api::artist_profile),
        )
        .route(
            "/servers/{server}/webhooks",
            get(webhooks::list).post(webhooks::create),
        )
        .route(
            "/servers/{server}/webhooks/{id}",
            axum::routing::put(webhooks::update).delete(webhooks::delete),
        )
        .route("/servers/{server}/webhooks/{id}/test", post(webhooks::test))
        .route(
            "/servers/{server}/webhook-deliveries",
            get(webhooks::deliveries),
        )
        .route(
            "/servers/{server}/webhook-deliveries/{id}/retry",
            post(webhooks::retry),
        )
        .route(
            "/servers/{server}/posts/{post}/comments",
            get(comments::list).post(comments::create),
        )
        .route(
            "/servers/{server}/posts/{post}/comments/{id}",
            delete(comments::delete),
        )
        .route(
            "/servers/{server}/posts/{post}/downloads",
            get(media::list).post(media::queue),
        )
        .route("/servers/{server}/downloads/{id}/file", get(media::file))
        .route("/servers/{server}/feeds/export", get(crate::opml::export))
        .route(
            "/servers/{server}/feeds/import/preview",
            post(crate::opml::preview).layer(axum::extract::DefaultBodyLimit::max(3 * 1024 * 1024)),
        )
        .route(
            "/servers/{server}/feeds/import",
            post(crate::opml::import).layer(axum::extract::DefaultBodyLimit::max(3 * 1024 * 1024)),
        )
        .route(
            "/servers/{server}/posts/{post}/webhooks",
            post(webhooks::send_post),
        )
        .route(
            "/servers/{server}/categories",
            get(categories::list).put(categories::save),
        )
        .route("/servers/{server}/ai", get(tags::config))
        .route(
            "/servers/{server}/posts/{post}/tags",
            axum::routing::put(tags::save),
        )
        .route(
            "/servers/{server}/posts/{post}/tags/suggest",
            post(tags::suggest),
        )
        .route("/servers/{server}/posts", get(api::posts))
        .route(
            "/servers/{server}/posts/{post}",
            get(api::post_detail).patch(api::post_preference),
        )
        .route("/servers/{server}/deliveries", get(api::deliveries))
        .route("/servers/{server}/deliveries/{job}/retry", post(api::retry))
        .route_layer(middleware::from_fn_with_state(state, auth::middleware));
    private
        .route("/config", get(auth::config))
        .route("/auth/login", get(auth::login))
        .route("/auth/callback", get(auth::callback))
        .route("/auth/logout", post(auth::logout))
        .route("/interactions", post(interactions::receive))
}

#[cfg(test)]
mod tests;
