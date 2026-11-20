use crate::community::{User, service};
use crate::{
    AppState,
    api::{Result, bad},
    models::Feed,
    network,
};
use axum::{
    Extension, Json,
    extract::{Path, State},
    http::header,
    response::IntoResponse,
};
use quick_xml::{Reader, events::Event};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashSet};
use uuid::Uuid;

pub const MAX_BYTES: usize = 2 * 1024 * 1024;
const MAX_FEEDS: usize = 1000;
#[derive(Debug, Serialize)]
pub struct ImportedFeed {
    name: String,
    url: String,
    category: String,
    site_url: String,
    source: String,
    enabled: bool,
    status: String,
    error: Option<String>,
}
#[derive(Deserialize)]
pub struct ImportInput {
    opml: String,
    #[serde(default = "default_enabled")]
    enabled: bool,
    #[serde(default = "default_poll")]
    poll_minutes: i32,
}
fn default_enabled() -> bool {
    true
}
fn default_poll() -> i32 {
    15
}
fn infer_source(url: &str) -> String {
    let path = network::web_url(url)
        .map(|url| url.path().to_string())
        .unwrap_or_default();
    if path.contains("/pawchive/") {
        "pawchive"
    } else if path.contains("/pixiv/") {
        "pixiv"
    } else if path.contains("/bsky/") {
        "bluesky"
    } else {
        "rss"
    }
    .into()
}
fn parse(xml: &str) -> Result<Vec<ImportedFeed>> {
    if xml.len() > MAX_BYTES {
        return Err(bad("OPML files must be no larger than 2 MiB"));
    }
    let mut reader = Reader::from_str(xml.trim_start_matches('\u{feff}'));
    reader.config_mut().expand_empty_elements = true;
    let mut elements: Vec<String> = Vec::new();
    let mut groups: Vec<Option<String>> = Vec::new();
    let mut feeds = Vec::new();
    let mut root_seen = false;
    let mut body_seen = false;
    loop {
        match reader
            .read_event()
            .map_err(|_| bad("Invalid XML. Choose a valid OPML export."))?
        {
            Event::Start(tag) | Event::Empty(tag) => {
                let name = String::from_utf8_lossy(tag.name().as_ref()).to_string();
                if elements.is_empty() {
                    if root_seen || name != "opml" {
                        return Err(bad("Expected a single OPML document"));
                    }
                    root_seen = true;
                }
                if elements.len() >= 32 {
                    return Err(bad("OPML nesting is too deep"));
                }
                if name == "body" && elements.last().is_some_and(|s| s == "opml") {
                    if body_seen {
                        return Err(bad("OPML has more than one body"));
                    }
                    body_seen = true;
                }
                let mut group = None;
                if name == "outline" && elements.iter().any(|s| s == "body") {
                    let mut attrs = BTreeMap::new();
                    for attr in tag.attributes() {
                        let attr = attr.map_err(|_| bad("Invalid OPML attributes"))?;
                        let value = attr
                            .decoded_and_normalized_value(
                                quick_xml::XmlVersion::Implicit1_0,
                                reader.decoder(),
                            )
                            .map_err(|_| bad("Invalid XML entity in OPML"))?;
                        attrs.insert(
                            String::from_utf8_lossy(attr.key.as_ref()).to_string(),
                            value.trim().to_string(),
                        );
                    }
                    let url = attrs.get("xmlUrl").cloned().unwrap_or_default();
                    let title = attrs
                        .get("title")
                        .filter(|s| !s.is_empty())
                        .or_else(|| attrs.get("text"))
                        .filter(|s| !s.is_empty())
                        .cloned()
                        .unwrap_or_else(|| url.clone());
                    if url.is_empty() {
                        if !title.is_empty() {
                            group = Some(title);
                        }
                    } else {
                        if feeds.len() >= MAX_FEEDS {
                            return Err(bad("Import at most 1,000 feeds at a time"));
                        }
                        let category = groups
                            .iter()
                            .flatten()
                            .cloned()
                            .collect::<Vec<_>>()
                            .join(" / ");
                        let error = if title.chars().count() > 120 {
                            Some("Feed name exceeds 120 characters".into())
                        } else if category.chars().count() > 500 {
                            Some("Category exceeds 500 characters".into())
                        } else if url.len() > 4096 {
                            Some("Feed URL is too long".into())
                        } else {
                            network::web_url(&url).err().map(|e| e.to_string())
                        };
                        let source = attrs
                            .get("lewdarchive:source")
                            .filter(|s| {
                                ["rss", "pixiv", "bluesky", "xcancel", "pawchive"]
                                    .contains(&s.as_str())
                            })
                            .cloned()
                            .unwrap_or_else(|| infer_source(&url));
                        let site_url = attrs
                            .get("htmlUrl")
                            .filter(|s| s.len() <= 4096 && network::web_url(s).is_ok())
                            .cloned()
                            .unwrap_or_default();
                        let enabled = !attrs
                            .get("miniflux:disabled")
                            .is_some_and(|s| s == "true" || s == "1");
                        feeds.push(ImportedFeed {
                            name: title,
                            url,
                            category,
                            site_url,
                            source,
                            enabled,
                            status: if error.is_some() { "invalid" } else { "new" }.into(),
                            error,
                        });
                    }
                }
                elements.push(name);
                groups.push(group);
            }
            Event::End(_) => {
                elements.pop();
                groups.pop();
            }
            Event::DocType(_) => return Err(bad("OPML documents with a DTD are not supported")),
            Event::Text(text)
                if elements.is_empty() && !text.as_ref().iter().all(u8::is_ascii_whitespace) =>
            {
                return Err(bad("Unexpected content outside OPML"));
            }
            Event::Eof => break,
            _ => {}
        }
    }
    if !root_seen || !body_seen || !elements.is_empty() {
        return Err(bad("Incomplete OPML document"));
    }
    Ok(feeds)
}
async fn prepare(s: &AppState, server: Uuid, xml: &str) -> Result<Vec<ImportedFeed>> {
    let mut feeds = parse(xml)?;
    let existing: Vec<String> = sqlx::query_scalar("SELECT f.url FROM feeds f JOIN community_artists a ON a.feed_id=f.id WHERE a.community_id=$1").bind(server)
        .fetch_all(&s.pool)
        .await?;
    let mut seen: HashSet<String> = existing.into_iter().collect();
    for feed in &mut feeds {
        if feed.error.is_none() && !seen.insert(feed.url.clone()) {
            feed.status = "duplicate".into();
        }
    }
    Ok(feeds)
}
pub async fn preview(
    State(s): State<AppState>,
    Extension(user): Extension<User>,
    Path(server): Path<Uuid>,
    Json(i): Json<ImportInput>,
) -> Result<Json<Value>> {
    service::authorize(&s, &user, server, true).await?;
    let feeds = prepare(&s, server, &i.opml).await?;
    Ok(Json(
        json!({"feeds":feeds,"new":feeds.iter().filter(|f| f.status == "new").count(),"duplicates":feeds.iter().filter(|f| f.status == "duplicate").count(),"invalid":feeds.iter().filter(|f| f.status == "invalid").count()}),
    ))
}
pub async fn import(
    State(s): State<AppState>,
    Extension(user): Extension<User>,
    Path(server): Path<Uuid>,
    Json(i): Json<ImportInput>,
) -> Result<Json<Value>> {
    if !(5..=1440).contains(&i.poll_minutes) {
        return Err(bad("Interval must be between 5 and 1440 minutes"));
    }
    service::authorize(&s, &user, server, true).await?;
    let feeds = prepare(&s, server, &i.opml).await?;
    let invalid = feeds.iter().filter(|f| f.status == "invalid").count();
    let mut imported = 0;
    let mut tx = s.pool.begin().await?;
    for feed in feeds.iter().filter(|f| f.status == "new") {
        sqlx::query("INSERT INTO feeds(id,name,url,source,poll_minutes) VALUES($1,$2,$3,$4,$5) ON CONFLICT(url) DO NOTHING").bind(Uuid::new_v4()).bind(&feed.name).bind(&feed.url).bind(&feed.source).bind(i.poll_minutes).execute(&mut *tx).await?;
        let id: Uuid = sqlx::query_scalar("SELECT id FROM feeds WHERE url=$1 FOR KEY SHARE")
            .bind(&feed.url)
            .fetch_one(&mut *tx)
            .await?;
        imported+=sqlx::query("INSERT INTO community_artists(id,community_id,feed_id,name,enabled,category) VALUES($1,$2,$3,$4,$5,$6) ON CONFLICT(community_id,feed_id) DO NOTHING").bind(Uuid::new_v4()).bind(server).bind(id).bind(&feed.name).bind(i.enabled && feed.enabled).bind(&feed.category).execute(&mut *tx).await?.rows_affected() as usize;
    }
    tx.commit().await?;
    s.wake.notify_one();
    Ok(Json(
        json!({"imported":imported,"duplicates":feeds.len()-imported-invalid,"invalid":invalid}),
    ))
}
fn escape(value: &str) -> String {
    quick_xml::escape::escape(value).into_owned()
}
fn export_xml(feeds: &[Feed]) -> String {
    let mut xml = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<opml version=\"2.0\" xmlns:miniflux=\"https://miniflux.app/opml\" xmlns:lewdarchive=\"urn:lewdarchive:opml\">\n<head><title>LewdArchive subscriptions</title></head>\n<body>\n",
    );
    let mut categories: BTreeMap<&str, Vec<&Feed>> = BTreeMap::new();
    for feed in feeds {
        categories.entry(&feed.category).or_default().push(feed);
    }
    for (category, feeds) in categories {
        if !category.is_empty() {
            xml.push_str(&format!(
                "<outline text=\"{}\" title=\"{}\">\n",
                escape(category),
                escape(category)
            ));
        }
        for feed in feeds {
            xml.push_str(&format!("<outline type=\"rss\" text=\"{}\" title=\"{}\" xmlUrl=\"{}\" htmlUrl=\"{}\" miniflux:disabled=\"{}\" lewdarchive:source=\"{}\" />\n",escape(&feed.name),escape(&feed.name),escape(&feed.url),escape(&feed.site_url),!feed.enabled,escape(&feed.source)));
        }
        if !category.is_empty() {
            xml.push_str("</outline>\n");
        }
    }
    xml.push_str("</body>\n</opml>\n");
    xml
}
pub async fn export(
    State(s): State<AppState>,
    Extension(user): Extension<User>,
    Path(server): Path<Uuid>,
) -> Result<impl IntoResponse> {
    service::authorize(&s, &user, server, true).await?;
    let feeds: Vec<Feed> = sqlx::query_as("SELECT f.id,a.name,f.url,f.source,a.category,f.site_url,a.enabled,f.initialized,f.etag,f.last_modified FROM feeds f JOIN community_artists a ON a.feed_id=f.id WHERE a.community_id=$1 ORDER BY a.category,a.name,a.id").bind(server)
        .fetch_all(&s.pool)
        .await?;
    Ok((
        [
            (header::CONTENT_TYPE, "text/x-opml; charset=utf-8"),
            (
                header::CONTENT_DISPOSITION,
                "attachment; filename=\"lewdarchive-feeds.opml\"",
            ),
        ],
        export_xml(&feeds),
    ))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn imports_miniflux_categories_and_disabled_feeds() {
        let parsed = parse(r#"<?xml version="1.0"?><opml version="2.0" xmlns:miniflux="https://miniflux.app/opml"><head><title>Miniflux</title></head><body><outline text="Art &amp; design"><outline text="A &quot;study&quot;" xmlUrl="https://example.com/rss?a=1&amp;b=2" htmlUrl="https://example.com" miniflux:disabled="true"/></outline></body></opml>"#).unwrap_or_else(|e| panic!("{}", e.1));
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].name, "A \"study\"");
        assert_eq!(parsed[0].category, "Art & design");
        assert_eq!(parsed[0].url, "https://example.com/rss?a=1&b=2");
        assert!(!parsed[0].enabled);
    }
    #[test]
    fn rejects_malformed_and_entity_documents_without_partial_imports() {
        for xml in [
            "<html/>",
            "<opml><body>",
            "<opml><body></opml>",
            "<!DOCTYPE opml [<!ENTITY x SYSTEM 'file:///etc/passwd'>]><opml><body/></opml>",
            "<opml><body/></opml><opml><body/></opml>",
        ] {
            assert!(parse(xml).is_err(), "{xml}");
        }
        assert!(parse(&"x".repeat(MAX_BYTES + 1)).is_err());
        let xml = format!(
            "<opml><body>{}</body></opml>",
            "<outline xmlUrl=\"https://example.com/rss\"/>".repeat(MAX_FEEDS + 1)
        );
        assert!(parse(&xml).is_err());
    }
    #[test]
    fn rejects_unsafe_feed_urls_but_reports_other_valid_entries() {
        let parsed = parse("<opml><body><outline text=\"Unsafe\" xmlUrl=\"file:///etc/passwd\"/><outline text=\"Valid\" xmlUrl=\"https://example.com/rss\"/></body></opml>").unwrap_or_else(|e| panic!("{}", e.1));
        assert_eq!(parsed[0].status, "invalid");
        assert_eq!(parsed[1].status, "new");
    }
}
