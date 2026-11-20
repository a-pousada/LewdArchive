use super::{Platform, PlatformFuture, RetryAfter};
use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use std::time::Duration;

#[derive(Clone)]
pub struct Discord {
    client: reqwest::Client,
    token: String,
    pub application_id: String,
    pub public_key: String,
    pub client_secret: String,
    pub base_url: String,
}
pub fn snowflake(value: &str) -> bool {
    (1..=20).contains(&value.len()) && value.bytes().all(|b| b.is_ascii_digit())
}
pub fn manages(permissions: u64) -> bool {
    permissions & 8 != 0
}
impl Discord {
    pub fn from_env() -> Result<Option<Self>> {
        let token = std::env::var("DISCORD_BOT_TOKEN").unwrap_or_default();
        if token.is_empty() {
            return Ok(None);
        }
        let required = |name| -> Result<String> {
            let value = std::env::var(name).unwrap_or_default();
            anyhow::ensure!(
                !value.trim().is_empty(),
                "{name} is required when the Discord bot is enabled"
            );
            Ok(value)
        };
        let application_id = required("DISCORD_APPLICATION_ID")?;
        anyhow::ensure!(snowflake(&application_id), "Invalid DISCORD_APPLICATION_ID");
        let public_key = required("DISCORD_PUBLIC_KEY")?;
        anyhow::ensure!(
            hex::decode(&public_key).is_ok_and(|v| v.len() == 32),
            "Invalid DISCORD_PUBLIC_KEY"
        );
        let base_url = required("PUBLIC_BASE_URL")?
            .trim_end_matches('/')
            .to_string();
        let parsed = url::Url::parse(&base_url)?;
        anyhow::ensure!(
            parsed.scheme() == "https"
                || (parsed.scheme() == "http"
                    && matches!(parsed.host_str(), Some("localhost" | "127.0.0.1"))),
            "PUBLIC_BASE_URL requires HTTPS (except localhost development)"
        );
        anyhow::ensure!(
            parsed.username().is_empty()
                && parsed.password().is_none()
                && parsed.query().is_none()
                && parsed.fragment().is_none()
                && parsed.path() == "/",
            "PUBLIC_BASE_URL must be an origin without a path or credentials"
        );
        Ok(Some(Self {
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(10))
                .redirect(reqwest::redirect::Policy::none())
                .user_agent("LewdArchive/0.1")
                .build()?,
            token,
            application_id,
            public_key,
            client_secret: required("DISCORD_CLIENT_SECRET")?,
            base_url,
        }))
    }
    pub fn client(&self) -> &reqwest::Client {
        &self.client
    }
    pub async fn response(response: reqwest::Response) -> Result<Value> {
        let status = response.status();
        if status.as_u16() == 429 {
            let data: Value = response.json().await.unwrap_or_default();
            return Err(RetryAfter(
                data["retry_after"]
                    .as_f64()
                    .unwrap_or(5.0)
                    .ceil()
                    .clamp(1.0, 86400.0) as u64,
            )
            .into());
        }
        if !status.is_success() {
            bail!("Discord returned HTTP {}", status.as_u16());
        }
        if status.as_u16() == 204 {
            return Ok(json!({}));
        }
        response.json().await.context("Invalid Discord response")
    }
    pub async fn get(&self, path: &str) -> Result<Value> {
        let response = self
            .client
            .get(format!("https://discord.com/api/v10{path}"))
            .header("Authorization", format!("Bot {}", self.token))
            .send()
            .await
            .map_err(|e| e.without_url())?;
        Self::response(response).await
    }
    pub async fn guild(&self, id: &str) -> Result<Value> {
        anyhow::ensure!(snowflake(id), "Invalid server ID");
        self.get(&format!("/guilds/{id}")).await
    }
    pub async fn verify_channel(&self, guild: &str, channel: &str) -> Result<String> {
        anyhow::ensure!(snowflake(channel), "Invalid channel ID");
        let data = self.get(&format!("/channels/{channel}")).await?;
        anyhow::ensure!(
            data["guild_id"].as_str() == Some(guild),
            "Channel does not belong to this server"
        );
        anyhow::ensure!(
            matches!(data["type"].as_i64(), Some(0 | 5)),
            "Choose a text or announcement channel"
        );
        Ok(data["name"].as_str().unwrap_or("Channel").to_string())
    }
    pub async fn edit_response(&self, token: &str, result: &Value) -> Result<()> {
        // Tokens come only from signature-verified Discord interactions.
        let response = self
            .client
            .patch(format!(
                "https://discord.com/api/v10/webhooks/{}/{}/messages/@original",
                self.application_id, token
            ))
            .json(&{
                let mut body = result.clone();
                if let Some(object) = body.as_object_mut() {
                    object.remove("flags");
                }
                body
            })
            .send()
            .await
            .map_err(|e| e.without_url())?;
        Self::response(response).await?;
        Ok(())
    }
}
impl Platform for Discord {
    fn notification(
        &self,
        entry: &crate::models::Entry,
        artist: uuid::Uuid,
        recipients: &[String],
        nonce: &str,
    ) -> Result<Value> {
        bot_payload(entry, artist, recipients, nonce)
    }
    fn membership<'a>(&'a self, server: &'a str, user: &'a str) -> PlatformFuture<'a, bool> {
        Box::pin(async move {
            anyhow::ensure!(
                snowflake(server) && snowflake(user),
                "Invalid account or server ID"
            );
            let member = self
                .get(&format!("/guilds/{server}/members/{user}"))
                .await?;
            let guild = self.guild(server).await?;
            if guild["owner_id"].as_str() == Some(user) {
                return Ok(true);
            }
            let roles = guild["roles"]
                .as_array()
                .context("Discord did not return server roles")?;
            let assigned = member["roles"].as_array().context("Invalid member roles")?;
            let permissions = roles
                .iter()
                .filter(|role| {
                    role["id"].as_str() == Some(server) || assigned.contains(&role["id"])
                })
                .fold(0u64, |bits, role| {
                    bits | role["permissions"]
                        .as_str()
                        .and_then(|s| s.parse::<u64>().ok())
                        .unwrap_or(0)
                });
            Ok(manages(permissions))
        })
    }
    fn send<'a>(&'a self, channel: &'a str, payload: Value) -> PlatformFuture<'a, String> {
        Box::pin(async move {
            anyhow::ensure!(snowflake(channel), "Invalid channel ID");
            let response = self
                .client
                .post(format!(
                    "https://discord.com/api/v10/channels/{channel}/messages"
                ))
                .header("Authorization", format!("Bot {}", self.token))
                .json(&payload)
                .send()
                .await
                .map_err(|e| e.without_url())?;
            let data = Self::response(response).await?;
            Ok(data["id"]
                .as_str()
                .context("Discord did not return a message ID")?
                .to_string())
        })
    }
}
fn embeds(e: &crate::models::Entry, color: i32) -> Result<Vec<Value>> {
    let media: Vec<crate::models::Media> = serde_json::from_value(e.media.clone())?;
    let description = crate::parser::clean_plain(&e.summary);
    let mut embed = json!({"title":e.title.chars().take(256).collect::<String>(),"url":e.url,
        "description":description.chars().take(1800).collect::<String>(),"color":color,
        "author":{"name":e.author.chars().take(256).collect::<String>()},"timestamp":e.published_at.to_rfc3339(),
        "footer":{"text":format!("LewdArchive · {}",e.feed_name).chars().take(256).collect::<String>()}});
    embed["fields"] = json!([
        {"name":"Artist","value":if e.feed_name.is_empty() { "Unknown artist".to_owned() } else { e.feed_name.chars().take(256).collect::<String>() },"inline":true},
        {"name":"Gallery","value":format!("{} files",media.len()),"inline":true},
        {"name":"Source","value":if e.url.chars().count() <= 680 { format!("[Original post]({})",e.url) } else { "Open the post title for the original source.".to_owned() },"inline":true}
    ]);
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
        embeds[0]["fields"].as_array_mut().unwrap().push(
            json!({"name":"Videos","value":attachments.chars().take(700).collect::<String>()}),
        );
    }
    Ok(embeds)
}

pub fn webhook_payload(
    e: &crate::models::Entry,
    webhook: &crate::models::Webhook,
) -> Result<Value> {
    let embeds = embeds(e, webhook.embed_color)?;
    let mut payload = json!({"allowed_mentions":{"parse":[]},"embeds":embeds});
    if !webhook.username.is_empty() {
        payload["username"] = json!(webhook.username);
    }
    if !webhook.avatar_url.is_empty() {
        payload["avatar_url"] = json!(webhook.avatar_url);
    }
    Ok(payload)
}

pub fn bot_payload(
    entry: &crate::models::Entry,
    artist: uuid::Uuid,
    recipients: &[String],
    nonce: &str,
) -> Result<Value> {
    let users: Vec<_> = recipients
        .iter()
        .filter(|id| snowflake(id))
        .take(50)
        .cloned()
        .collect();
    let content = users
        .iter()
        .map(|id| format!("<@{id}>"))
        .collect::<Vec<_>>()
        .join(" ");
    Ok(
        json!({"content":content,"embeds":embeds(entry,14978709)?,"allowed_mentions":{"parse":[],"users":users,"roles":[],"replied_user":false},"nonce":nonce.chars().take(24).collect::<String>(),"enforce_nonce":true,"components":[{"type":1,"components":[
            {"type":2,"style":2,"emoji":{"name":"❤️"},"label":"Like","custom_id":format!("la:like:{}:1",entry.id)},
            {"type":2,"style":1,"emoji":{"name":"🔖"},"label":"Save post","custom_id":format!("la:post:{}:1",entry.id)},
            {"type":2,"style":2,"emoji":{"name":"⭐"},"label":"Favorite artist","custom_id":format!("la:artist:{artist}:1")},
            {"type":2,"style":2,"emoji":{"name":"🔔"},"label":"Notify me","custom_id":format!("la:notify:{artist}:1")}
        ]}]}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn permissions_and_ids() {
        assert!(!manages(32));
        assert!(manages(8));
        assert!(!manages(1024));
        assert!(snowflake("123456789"));
        assert!(!snowflake("../@me"));
    }
}
