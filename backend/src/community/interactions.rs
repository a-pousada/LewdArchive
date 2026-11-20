use super::{ArtistInput, Preference, Server, auth, service};
use crate::{
    AppState,
    api::{Result, bad},
    platforms::discord::snowflake,
};
use axum::{
    Json,
    body::Bytes,
    extract::State,
    http::{HeaderMap, StatusCode},
};
use ed25519_dalek::{Signature, VerifyingKey};
use serde_json::{Value, json};
use uuid::Uuid;

pub fn verify(key: &str, signature: &str, timestamp: &str, body: &[u8], now: i64) -> bool {
    let Ok(time) = timestamp.parse::<i64>() else {
        return false;
    };
    if !(now - 300..=now + 30).contains(&time) {
        return false;
    }
    let Ok(key) = hex::decode(key) else {
        return false;
    };
    let Ok(key) = <[u8; 32]>::try_from(key.as_slice()) else {
        return false;
    };
    let Ok(key) = VerifyingKey::from_bytes(&key) else {
        return false;
    };
    let Ok(signature) = hex::decode(signature) else {
        return false;
    };
    let Ok(signature) = Signature::from_slice(&signature) else {
        return false;
    };
    let mut message = timestamp.as_bytes().to_vec();
    message.extend_from_slice(body);
    key.verify_strict(&message, &signature).is_ok()
}
pub async fn receive(
    State(s): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> std::result::Result<Json<Value>, StatusCode> {
    let d = s.discord.as_ref().ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    let header = |key| headers.get(key).and_then(|v| v.to_str().ok()).unwrap_or("");
    if !verify(
        &d.public_key,
        header("x-signature-ed25519"),
        header("x-signature-timestamp"),
        &body,
        chrono::Utc::now().timestamp(),
    ) {
        return Err(StatusCode::UNAUTHORIZED);
    }
    let payload: Value = serde_json::from_slice(&body).map_err(|_| StatusCode::BAD_REQUEST)?;
    if payload["type"] == 1 {
        return Ok(Json(json!({"type":1})));
    }
    if payload["application_id"].as_str() != Some(&d.application_id)
        || !matches!(payload["type"].as_i64(), Some(2 | 3))
    {
        return Err(StatusCode::BAD_REQUEST);
    }
    if !payload["guild_id"].as_str().is_some_and(snowflake) {
        return Ok(Json(
            json!({"type":4,"data":private("Use these commands in a server, not in direct messages.")}),
        ));
    }
    let id = payload["id"]
        .as_str()
        .filter(|id| snowflake(id))
        .ok_or(StatusCode::BAD_REQUEST)?;
    if !payload["token"].as_str().is_some_and(|t| {
        !t.is_empty()
            && t.len() < 2048
            && t.bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
    }) {
        return Err(StatusCode::BAD_REQUEST);
    }
    tokio::time::timeout(std::time::Duration::from_secs(2),
        sqlx::query("INSERT INTO interaction_jobs(external_id,payload) VALUES($1,$2) ON CONFLICT(external_id) DO NOTHING").bind(id).bind(&payload).execute(&s.pool)
    ).await.map_err(|_|StatusCode::SERVICE_UNAVAILABLE)?.map_err(|_|StatusCode::SERVICE_UNAVAILABLE)?;
    // Durable deferred response: the independent community worker completes it privately.
    Ok(Json(json!({"type":5,"data":{"flags":64}})))
}
pub fn private(content: &str) -> Value {
    json!({"content":content.chars().take(1900).collect::<String>(),"allowed_mentions":{"parse":[]},"flags":64})
}
fn option<'a>(options: &'a [Value], name: &str) -> Option<&'a Value> {
    options
        .iter()
        .find(|v| v["name"] == name)
        .map(|v| &v["value"])
}
fn id(value: Option<&Value>) -> Result<Uuid> {
    value.and_then(Value::as_str).and_then(|s|s.parse().ok()).ok_or_else(||bad("Choose a valid artist or post ID. Use /archive artists or /archive latest to find it."))
}
fn text<'a>(options: &'a [Value], name: &str) -> &'a str {
    option(options, name).and_then(Value::as_str).unwrap_or("")
}
fn enabled(options: &[Value]) -> bool {
    option(options, "enabled")
        .and_then(Value::as_bool)
        .unwrap_or(true)
}
pub async fn process(s: &AppState, payload: &Value) -> Result<Value> {
    let guild = payload["guild_id"]
        .as_str()
        .filter(|id| snowflake(id))
        .ok_or_else(|| bad("Server required"))?;
    let profile = &payload["member"]["user"];
    let external = profile["id"]
        .as_str()
        .filter(|id| snowflake(id))
        .ok_or_else(|| bad("Member required"))?;
    let mut tx = s.pool.begin().await?;
    let user = service::upsert_user(
        &mut tx,
        "discord",
        external,
        profile["global_name"]
            .as_str()
            .or_else(|| profile["username"].as_str())
            .unwrap_or("Discord user"),
        &auth::avatar(profile),
    )
    .await?;
    tx.commit().await?;
    let empty = Vec::new();
    let command = payload["data"]["options"]
        .as_array()
        .and_then(|a| a.first());
    let name = command.and_then(|v| v["name"].as_str()).unwrap_or("help");
    let options = command
        .and_then(|v| v["options"].as_array())
        .unwrap_or(&empty);
    let d = s
        .discord
        .as_ref()
        .ok_or_else(|| bad("Discord unavailable"))?;
    if payload["type"] == 2 && payload["data"]["name"] != "archive" {
        return Err(bad("Unknown command"));
    }
    if name == "setup" {
        use crate::platforms::Platform;
        if !d
            .membership(guild, external)
            .await
            .map_err(|_| bad("Could not verify server permissions"))?
        {
            return Err(bad("Administrator permission is required"));
        }
        let channel = text(options, "channel");
        let channel_name = d
            .verify_channel(guild, channel)
            .await
            .map_err(|e| bad(e.to_string()))?;
        let guild_data = d
            .guild(guild)
            .await
            .map_err(|_| bad("Server unavailable"))?;
        let server = service::connect(
            s,
            "discord",
            guild,
            guild_data["name"].as_str().unwrap_or("Discord server"),
        )
        .await?;
        sqlx::query("UPDATE communities SET channel_id=$2,channel_name=$3 WHERE id=$1")
            .bind(server.id)
            .bind(channel)
            .bind(&channel_name)
            .execute(&s.pool)
            .await?;
        return Ok(private(&format!(
            "Announcement channel saved: #{channel_name}. Manage artists at {}/community. The library is shared with members of this server; personal favorites stay private.",
            d.base_url
        )));
    }
    let server:Server=sqlx::query_as("SELECT * FROM communities WHERE platform='discord' AND external_id=$1").bind(guild).fetch_optional(&s.pool).await?.ok_or_else(||bad("An administrator must run /archive setup first, or connect this server on the website."))?;
    service::authorize(s, &user, server.id, false).await?;
    // Only verified guild membership can operate on this server's records.
    if payload["type"] == 3 {
        let parts: Vec<_> = payload["data"]["custom_id"]
            .as_str()
            .unwrap_or("")
            .split(':')
            .collect();
        if parts.len() != 4 || parts[0] != "la" {
            return Err(bad("Unknown action"));
        }
        let target = parts[2].parse().map_err(|_| bad("Invalid item"))?;
        let value = parts[3] == "1";
        let mut pref = Preference::default();
        let (result, label) = match parts[1] {
            "like" => {
                pref.liked = Some(value);
                (
                    service::post_preference(s, server.id, user.id, target, &pref).await?,
                    "like",
                )
            }
            "post" => {
                pref.favorite = Some(value);
                (
                    service::post_preference(s, server.id, user.id, target, &pref).await?,
                    "post favorite",
                )
            }
            "artist" => {
                pref.favorite = Some(value);
                (
                    service::artist_preference(s, server.id, user.id, target, &pref).await?,
                    "artist favorite",
                )
            }
            "notify" => {
                pref.notify = Some(value);
                (
                    service::artist_preference(s, server.id, user.id, target, &pref).await?,
                    "artist mentions",
                )
            }
            _ => return Err(bad("Unknown action")),
        };
        let mut response = private(&format!(
            "Your {label} is {}.{}",
            if value { "enabled" } else { "disabled" },
            result
                .get("likes")
                .map(|n| format!(" This post has {n} likes in this server."))
                .unwrap_or_default()
        ));
        response["components"] = json!([{"type":1,"components":[{"type":2,"style":2,"emoji":{"name":"↩️"},"label":if value{"Undo"}else{"Enable again"},"custom_id":format!("la:{}:{}:{}",parts[1],target,if value{0}else{1})}]}]);
        return Ok(response);
    }
    match name {
        "trending" => {
            let q = super::trending::Filter {
                period: serde_json::from_value(
                    option(options, "period").cloned().unwrap_or(json!("week")),
                )
                .map_err(|_| bad("Invalid ranking period"))?,
                metric: serde_json::from_value(
                    option(options, "metric").cloned().unwrap_or(json!("likes")),
                )
                .map_err(|_| bad("Invalid ranking metric"))?,
                page: option(options, "page").and_then(Value::as_i64).unwrap_or(1),
            };
            let result = super::trending::posts(s, server.id, user.id, &q, 5).await?;
            let metric = option(options, "metric")
                .and_then(Value::as_str)
                .unwrap_or("likes");
            let mut response = private(&format!(
                "🔥 Trending · last {} days · page {} of {}\nRanked by {}. Current counts for posts published in this window. Saved collections stay private.\n{}/community",
                result["days"],
                result["page"],
                result["pages"].as_i64().unwrap_or(0).max(1),
                metric.replace("engagement", "likes + saves"),
                d.base_url
            ));
            let embeds:Vec<_>=result["items"].as_array().unwrap().iter().map(|p|json!({
                "title":format!("#{} · {}",p["rank"],p["title"].as_str().unwrap_or("Post").chars().take(180).collect::<String>()),
                "url":p["url"],"color":14978709,
                "description":format!("❤️ {} likes · 🔖 {} saves\nArtist: {}\nPost ID: {}",p["likes"],p["saves"],p["feed_name"].as_str().unwrap_or("Artist").chars().take(120).collect::<String>(),p["id"].as_str().unwrap_or(""))
            })).collect();
            if embeds.is_empty() {
                response["content"] = json!(format!(
                    "No ranked posts in this window yet. Try a longer period.\n{}/community",
                    d.base_url
                ));
            }
            response["embeds"] = json!(embeds);
            Ok(response)
        }

        "track" => {
            service::authorize(s, &user, server.id, true).await?;
            let result = service::track(
                s,
                server.id,
                &ArtistInput {
                    name: text(options, "name").into(),
                    url: text(options, "url").into(),
                    source: "rss".into(),
                    category: String::new(),
                },
            )
            .await?;
            Ok(private(&format!(
                "Artist tracked in this server. ID: {}. Initial history is imported silently.",
                result["id"].as_str().unwrap_or("")
            )))
        }
        "untrack" => {
            service::authorize(s, &user, server.id, true).await?;
            service::untrack(s, server.id, id(option(options, "artist"))?).await?;
            Ok(private(
                "Artist removed from this server. Other servers are unaffected.",
            ))
        }
        "like" | "favorite-post" => {
            let mut pref = Preference::default();
            if name == "like" {
                pref.liked = Some(enabled(options));
            } else {
                pref.favorite = Some(enabled(options));
            }
            let result = service::post_preference(
                s,
                server.id,
                user.id,
                id(option(options, "post"))?,
                &pref,
            )
            .await?;
            Ok(private(&format!(
                "Preference saved. This post has {} likes in this server. Favorites are private.",
                result["likes"]
            )))
        }
        "favorite-artist" | "notify" => {
            let mut pref = Preference::default();
            if name == "notify" {
                pref.notify = Some(enabled(options));
            } else {
                pref.favorite = Some(enabled(options));
            }
            service::artist_preference(
                s,
                server.id,
                user.id,
                id(option(options, "artist"))?,
                &pref,
            )
            .await?;
            Ok(private(if name == "notify" {
                "Your mention preference was saved. Mentions require server notifications and mentions to be enabled; favoriting alone never opts you in."
            } else {
                "Your private artist favorite was saved."
            }))
        }
        "artists" | "favorites" if name == "artists" || text(options, "kind") == "artists" => {
            let artists = service::artists(s, server.id, user.id, name == "favorites").await?;
            let page = option(options, "page")
                .and_then(Value::as_u64)
                .unwrap_or(1)
                .clamp(1, 10000) as usize;
            let lines = artists
                .as_array()
                .unwrap()
                .iter()
                .skip((page - 1) * 8)
                .take(8)
                .map(|a| {
                    format!(
                        "**{}**\nID: `{}` · mentions {}",
                        a["name"].as_str().unwrap_or("Artist"),
                        a["id"].as_str().unwrap_or(""),
                        if a["notify"] == true { "on" } else { "off" }
                    )
                })
                .collect::<Vec<_>>()
                .join("\n\n");
            Ok(private(&format!(
                "{} — page {page}\n\n{}\n\nFull collection: {}/community",
                if name == "favorites" {
                    "Your favorite artists"
                } else {
                    "Tracked artists"
                },
                if lines.is_empty() {
                    "No artists on this page."
                } else {
                    &lines
                },
                d.base_url
            )))
        }
        "latest" | "favorites" => {
            let page = option(options, "page")
                .and_then(Value::as_i64)
                .unwrap_or(1)
                .clamp(1, 10000);
            // Commands use smaller pages that stay within Discord message limits.
            let posts = service::posts(
                s,
                server.id,
                user.id,
                &service::PostFilter {
                    page,
                    favorites: name == "favorites",
                    ..Default::default()
                },
                8,
            )
            .await?;
            let lines = posts["items"]
                .as_array()
                .unwrap()
                .iter()
                .take(8)
                .map(|p| {
                    format!(
                        "**{}** · {} likes\nID: `{}`",
                        p["title"]
                            .as_str()
                            .unwrap_or("Post")
                            .chars()
                            .take(80)
                            .collect::<String>(),
                        p["likes"],
                        p["id"].as_str().unwrap_or("")
                    )
                })
                .collect::<Vec<_>>()
                .join("\n\n");
            Ok(private(&format!(
                "{} — page {page}\n\n{}\n\nFull collection: {}/community",
                if name == "favorites" {
                    "Your favorite posts"
                } else {
                    "Latest posts"
                },
                if lines.is_empty() {
                    "No posts on this page."
                } else {
                    &lines
                },
                d.base_url
            )))
        }
        _ => Ok(private(
            "Use /archive setup to choose a channel, /archive track to follow an artist, /archive artists or latest to browse, /archive favorites for your private collection, /archive trending for rankings, and /archive notify to opt into artist mentions.",
        )),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};
    #[test]
    fn rejects_forged_tampered_and_stale_requests() {
        let key = SigningKey::from_bytes(&[7; 32]);
        let timestamp = "1000";
        let body = b"{\"type\":1}";
        let mut bytes = timestamp.as_bytes().to_vec();
        bytes.extend_from_slice(body);
        let sig = hex::encode(key.sign(&bytes).to_bytes());
        let public = hex::encode(key.verifying_key().to_bytes());
        assert!(verify(&public, &sig, timestamp, body, 1001));
        assert!(!verify(&public, &sig, timestamp, b"changed", 1001));
        assert!(!verify(&public, &sig, timestamp, body, 1400));
        assert!(!verify(&public, "00", timestamp, body, 1001));
    }
}
