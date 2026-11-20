use crate::{
    AppState,
    api::{Result, bad, not_found},
    network,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use uuid::Uuid;

#[derive(Clone, Serialize, Deserialize)]
pub struct OfficialLink {
    pub label: String,
    pub url: String,
}
#[derive(Deserialize)]
pub struct Profile {
    pub name: String,
    pub category: Option<String>,
    #[serde(default)]
    pub avatar_url: String,
    #[serde(default)]
    pub official_links: Vec<OfficialLink>,
}
impl Profile {
    pub fn validate(&self) -> Result<()> {
        if self.name.trim().is_empty() || self.name.chars().count() > 120 {
            return Err(bad("Artist name must be between 1 and 120 characters"));
        }
        if let Some(category) = &self.category {
            super::categories::validate(category.trim())?;
        }
        if !self.avatar_url.trim().is_empty() {
            https_url(self.avatar_url.trim())?;
        }
        if self.official_links.len() > 8 {
            return Err(bad("Use at most eight official links"));
        }
        for link in &self.official_links {
            if link.label.trim().is_empty() || link.label.chars().count() > 40 {
                return Err(bad("Link labels must be between 1 and 40 characters"));
            }
            https_url(link.url.trim())?;
        }
        Ok(())
    }
}
fn https_url(value: &str) -> Result<()> {
    if value.len() > 2048 {
        return Err(bad("URL is too long"));
    }
    let url =
        network::web_url(value).map_err(|_| bad("Use a valid HTTPS URL without credentials"))?;
    if url.scheme() != "https" {
        return Err(bad("Artist images and official links require HTTPS"));
    }
    Ok(())
}
pub async fn update(s: &AppState, server: Uuid, artist: Uuid, i: &Profile) -> Result<Value> {
    i.validate()?;
    let links: Vec<_> = i
        .official_links
        .iter()
        .map(|l| json!({"label":l.label.trim(),"url":l.url.trim()}))
        .collect();
    if sqlx::query("UPDATE community_artists SET name=$3,avatar_url=$4,official_links=$5,category=coalesce($6,category) WHERE community_id=$1 AND id=$2")
 .bind(server).bind(artist).bind(i.name.trim()).bind(i.avatar_url.trim()).bind(json!(links)).bind(i.category.as_deref().map(str::trim)).execute(&s.pool).await?.rows_affected()==0{return Err(not_found());}
    Ok(json!({"ok":true}))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn validates_profile_links() {
        let mut p = Profile {
            name: "Artist".into(),
            category: None,
            avatar_url: "https://example.com/avatar.png".into(),
            official_links: vec![],
        };
        assert!(p.validate().is_ok());
        p.official_links.push(OfficialLink {
            label: "Website".into(),
            url: "javascript:alert(1)".into(),
        });
        assert!(p.validate().is_err());
        p.official_links[0].url = "https://user:pass@example.com".into();
        assert!(p.validate().is_err());
        p.official_links[0].url = "https://example.com".into();
        assert!(p.validate().is_ok());
    }
}
