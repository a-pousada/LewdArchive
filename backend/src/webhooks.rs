use crate::{
    api::{Result, bad},
    models::Webhook,
    network,
};
use serde::Deserialize;
use serde_json::{Value, json};
#[derive(Deserialize)]
pub struct WebhookInput {
    pub(crate) category_filter: Option<String>,
    #[serde(default)]
    pub(crate) category_filters: Option<Vec<String>>,
    #[serde(default)]
    pub(crate) include_tags: Vec<String>,
    #[serde(default)]
    pub(crate) exclude_tags: Vec<String>,
    pub(crate) name: String,
    pub(crate) url: Option<String>,
    #[serde(default)]
    pub(crate) username: String,
    #[serde(default)]
    pub(crate) avatar_url: String,
    pub(crate) embed_color: i32,
    pub(crate) enabled: bool,
}
pub(crate) struct ValidatedFilters {
    pub(crate) category_filter: Option<String>,
    pub(crate) category_filters: Option<Vec<String>>,
    pub(crate) include_tags: Vec<String>,
    pub(crate) exclude_tags: Vec<String>,
}
impl WebhookInput {
    pub(crate) fn validated_filters(&self) -> Result<ValidatedFilters> {
        let mut categories = Vec::new();
        if let Some(list) = &self.category_filters {
            for raw in list {
                let trimmed = raw.trim();
                crate::community::categories::validate(trimmed)?;
                if !categories.iter().any(|existing| existing == trimmed) {
                    categories.push(trimmed.to_owned());
                }
            }
            if categories.len() > 50 {
                return Err(bad("Choose at most 50 categories"));
            }
        } else if let Some(category) = &self.category_filter {
            let trimmed = category.trim();
            crate::community::categories::validate(trimmed)?;
            categories.push(trimmed.to_owned());
        }
        let category_filters = if categories.is_empty() {
            None
        } else {
            Some(categories)
        };
        let category_filter = match &category_filters {
            Some(list) if list.len() == 1 => Some(list[0].clone()),
            _ => None,
        };
        let include_tags = crate::community::tags::normalize(&self.include_tags)?;
        let exclude_tags = crate::community::tags::normalize(&self.exclude_tags)?;
        if include_tags
            .iter()
            .any(|tag| exclude_tags.iter().any(|blocked| blocked == tag))
        {
            return Err(bad("A tag cannot be in both include and exclude filters"));
        }
        Ok(ValidatedFilters {
            category_filter,
            category_filters,
            include_tags,
            exclude_tags,
        })
    }
    pub(crate) fn validate(&self) -> Result<()> {
        self.validated_filters()?;
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

pub(crate) fn matches_tags(include: &[String], exclude: &[String], post_tags: &[String]) -> bool {
    let include_ok = include.is_empty() || include.iter().any(|t| post_tags.iter().any(|p| p == t));
    let exclude_ok =
        exclude.is_empty() || !exclude.iter().any(|t| post_tags.iter().any(|p| p == t));
    include_ok && exclude_ok
}

/// Build a Discord/Fluxer-compatible test embed without touching database entries.
pub(crate) fn test_payload(webhook: &Webhook, server_name: &str) -> Value {
    let categories = match &webhook.category_filters {
        Some(list) if !list.is_empty() => list
            .iter()
            .map(|c| {
                if c.is_empty() {
                    "Uncategorized"
                } else {
                    c.as_str()
                }
            })
            .collect::<Vec<_>>()
            .join(", "),
        _ => match webhook.category_filter.as_deref() {
            Some("") => "Uncategorized".into(),
            Some(c) => c.into(),
            None => "All categories".into(),
        },
    };
    let mut fields = vec![
        json!({"name": "Server", "value": server_name, "inline": true}),
        json!({"name": "Categories", "value": categories, "inline": true}),
    ];
    if !webhook.include_tags.is_empty() {
        let value = webhook
            .include_tags
            .iter()
            .map(|t| format!("`#{t}`"))
            .collect::<Vec<_>>()
            .join(" ");
        fields.push(json!({"name": "Include tags", "value": value}));
    }
    if !webhook.exclude_tags.is_empty() {
        let value = webhook
            .exclude_tags
            .iter()
            .map(|t| format!("`#{t}`"))
            .collect::<Vec<_>>()
            .join(" ");
        fields.push(json!({"name": "Exclude tags", "value": value}));
    }
    let mut payload = json!({
        "allowed_mentions": {"parse": []},
        "embeds": [{
            "title": format!("Test delivery — {}", webhook.name),
            "description": "This destination is reachable and ready to receive matching artwork from LewdArchive.",
            "color": webhook.embed_color,
            "fields": fields,
            "footer": {"text": "LewdArchive · Webhook test"},
            "timestamp": chrono::Utc::now().to_rfc3339(),
        }]
    });
    if !webhook.username.is_empty() {
        payload["username"] = json!(webhook.username);
    }
    if !webhook.avatar_url.is_empty() {
        payload["avatar_url"] = json!(webhook.avatar_url);
    }
    payload
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use uuid::Uuid;

    fn base_input() -> WebhookInput {
        WebhookInput {
            category_filter: None,
            category_filters: None,
            include_tags: Vec::new(),
            exclude_tags: Vec::new(),
            name: "Hook".into(),
            url: None,
            username: "LewdArchive".into(),
            avatar_url: String::new(),
            embed_color: 0xe5a0b3,
            enabled: true,
        }
    }

    #[test]
    fn validated_filters_normalizes_categories_and_tags() {
        let mut legacy = base_input();
        legacy.category_filter = Some(" 3D Art ".into());
        let f = legacy.validated_filters().ok().unwrap();
        assert_eq!(f.category_filters, Some(vec!["3D Art".into()]));
        assert_eq!(f.category_filter, Some("3D Art".into()));

        let mut multi = base_input();
        multi.category_filters = Some(vec![
            "2D Illustration".into(),
            " 3D Art ".into(),
            "2D Illustration".into(),
        ]);
        multi.include_tags = vec![" #Digital Art ".into(), "anime".into()];
        multi.exclude_tags = vec!["furry".into()];
        let f = multi.validated_filters().ok().unwrap();
        assert_eq!(
            f.category_filters,
            Some(vec!["2D Illustration".into(), "3D Art".into()])
        );
        assert_eq!(f.category_filter, None);
        assert_eq!(f.include_tags, vec!["digital_art", "anime"]);
        assert_eq!(f.exclude_tags, vec!["furry"]);

        let mut all = base_input();
        all.category_filters = Some(Vec::new());
        let f = all.validated_filters().ok().unwrap();
        assert_eq!(f.category_filters, None);
        assert_eq!(f.category_filter, None);

        let mut overlap = base_input();
        overlap.include_tags = vec!["3d".into()];
        overlap.exclude_tags = vec!["#3d".into()];
        assert!(overlap.validated_filters().is_err());

        let mut too_many = base_input();
        too_many.category_filters = Some((0..51).map(|i| format!("Cat {i}")).collect());
        assert!(too_many.validated_filters().is_err());
    }

    #[test]
    fn matches_tags_evaluates_include_and_exclude_combinations() {
        let inc = vec!["3d".into(), "blender".into()];
        let exc = vec!["furry".into(), "monochrome".into()];
        assert!(matches_tags(&[], &[], &[]));
        assert!(!matches_tags(&inc, &[], &[]));
        assert!(matches_tags(&inc, &[], &["blender".into()]));
        assert!(matches_tags(&[], &exc, &[]));
        assert!(!matches_tags(&[], &exc, &["furry".into()]));
        assert!(!matches_tags(&inc, &exc, &["3d".into(), "furry".into()]));
        assert!(matches_tags(&inc, &exc, &["3d".into(), "anime".into()]));
    }

    #[test]
    fn test_payload_formats_filters_and_disables_mentions() {
        let webhook = Webhook {
            category_filter: None,
            category_filters: Some(vec!["2D Illustration".into(), "".into()]),
            include_tags: vec!["digital_art".into()],
            exclude_tags: vec!["monochrome".into()],
            id: Uuid::new_v4(),
            name: "Fluxer 2D".into(),
            url: String::new(),
            username: "Archive Bot".into(),
            avatar_url: String::new(),
            embed_color: 0x123456,
            enabled: true,
            created_at: Utc::now(),
        };
        let payload = test_payload(&webhook, "A Pousada");
        assert_eq!(payload["username"], "Archive Bot");
        assert!(payload.get("avatar_url").is_none());
        assert_eq!(payload["allowed_mentions"]["parse"], json!([]));
        assert_eq!(
            payload["embeds"][0]["fields"][1]["value"],
            "2D Illustration, Uncategorized"
        );
        assert_eq!(payload["embeds"][0]["fields"][2]["value"], "`#digital_art`");
        assert_eq!(payload["embeds"][0]["fields"][3]["value"], "`#monochrome`");
    }
}
