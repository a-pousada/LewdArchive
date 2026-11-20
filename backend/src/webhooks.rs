use crate::{
    api::{Result, bad},
    network,
};
use serde::Deserialize;
#[derive(Deserialize)]
pub struct WebhookInput {
    pub(crate) category_filter: Option<String>,
    pub(crate) name: String,
    pub(crate) url: Option<String>,
    #[serde(default)]
    pub(crate) username: String,
    #[serde(default)]
    pub(crate) avatar_url: String,
    pub(crate) embed_color: i32,
    pub(crate) enabled: bool,
}
impl WebhookInput {
    pub(crate) fn validate(&self) -> Result<()> {
        if let Some(category) = &self.category_filter {
            crate::community::categories::validate(category.trim())?;
        }
        if self.name.trim().is_empty() || self.name.chars().count() > 120 {
            return Err(bad("Name must be between 1 and 120 characters"));
        }
        if self.username.chars().count() > 80 || self.username.chars().any(char::is_control) {
            return Err(bad(
                "Display name must be at most 80 characters without control characters",
            ));
        }
        if !(0..=0xffffff).contains(&self.embed_color) {
            return Err(bad("Invalid embed color"));
        }
        if !self.avatar_url.trim().is_empty() {
            let url = network::web_url(self.avatar_url.trim()).map_err(|e| bad(e.to_string()))?;
            if url.scheme() != "https" || self.avatar_url.len() > 4096 {
                return Err(bad(
                    "Avatar must be a public HTTPS image URL, at most 4096 characters",
                ));
            }
        }
        if let Some(url) = &self.url {
            validate_url(url)?;
        }
        Ok(())
    }
}
pub(crate) fn validate_url(url: &str) -> Result<String> {
    if url.len() > 4096 {
        return Err(bad("Webhook URL is too long"));
    }
    let mut parsed = network::webhook(url.trim()).map_err(|e| bad(e.to_string()))?;
    parsed.set_fragment(None);
    Ok(parsed.to_string())
}
