use crate::{models::Media, network::web_url};
use chrono::{DateTime, Utc};
use scraper::{Html, Selector};
use sha2::{Digest, Sha256};
use std::collections::HashSet;

pub struct ParsedEntry {
    pub guid: String,
    pub title: String,
    pub author: String,
    pub url: String,
    pub summary: String,
    pub media: Vec<Media>,
    pub published_at: DateTime<Utc>,
}
pub fn plain(html: &str) -> String {
    Html::parse_fragment(html)
        .root_element()
        .text()
        .collect::<Vec<_>>()
        .join(" ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}
pub fn parse(
    bytes: &[u8],
    feed_url: &str,
    fallback_author: &str,
) -> anyhow::Result<Vec<ParsedEntry>> {
    let feed = feed_rs::parser::parse(bytes)?;
    let base = web_url(feed_url)?;
    let mut entries = Vec::new();
    for entry in feed.entries {
        let raw_url = entry
            .links
            .iter()
            .find(|l| l.rel.as_deref().is_none_or(|r| r == "alternate"))
            .map(|l| l.href.as_str())
            .unwrap_or(feed_url);
        let url = base
            .join(raw_url)
            .ok()
            .filter(|u| web_url(u.as_str()).is_ok())
            .unwrap_or(base.clone());
        let html = entry
            .content
            .as_ref()
            .and_then(|c| c.body.as_deref())
            .or(entry.summary.as_ref().map(|s| s.content.as_str()))
            .unwrap_or("");
        let mut media = Vec::new();
        let mut seen = HashSet::new();
        let mut add = |raw: &str, kind: &str| {
            if media.len() >= 30 {
                return;
            }
            if let Ok(u) = url.join(raw)
                && web_url(u.as_str()).is_ok()
                && seen.insert(u.to_string())
            {
                media.push(Media {
                    url: u.to_string(),
                    kind: kind.into(),
                });
            }
        };
        for link in &entry.links {
            if link.rel.as_deref() == Some("enclosure") {
                let kind = if link
                    .media_type
                    .as_deref()
                    .unwrap_or("")
                    .starts_with("video/")
                {
                    "video"
                } else {
                    "image"
                };
                add(&link.href, kind);
            }
        }
        for object in &entry.media {
            for content in &object.content {
                if let Some(u) = &content.url {
                    let kind = if content
                        .content_type
                        .as_ref()
                        .is_some_and(|m| m.to_string().starts_with("video/"))
                    {
                        "video"
                    } else {
                        "image"
                    };
                    add(u.as_str(), kind);
                }
            }
            for thumb in &object.thumbnails {
                add(thumb.image.uri.as_str(), "image");
            }
        }
        let doc = Html::parse_fragment(html);
        for el in doc.select(&Selector::parse("img, video, video source").unwrap()) {
            if let Some(src) = el.value().attr("src").or(el.value().attr("data-src")) {
                add(
                    src,
                    if el.value().name() == "img" {
                        "image"
                    } else {
                        "video"
                    },
                );
            }
        }
        let title = entry
            .title
            .map(|t| plain(&t.content))
            .filter(|t| !t.is_empty())
            .unwrap_or("Untitled".into());
        let guid = if entry.id.is_empty() {
            format!("{:x}", Sha256::digest(format!("{url}|{title}")))
        } else {
            entry.id
        };
        entries.push(ParsedEntry {
            guid,
            title,
            author: entry
                .authors
                .first()
                .map(|a| {
                    // RSS 2.0 puts the contact text in email and uses "author" as its role.
                    if a.name == "author" || a.name.trim().is_empty() {
                        a.email
                            .as_deref()
                            .map(|contact| {
                                contact
                                    .rsplit_once('(')
                                    .and_then(|(_, name)| name.strip_suffix(')'))
                                    .unwrap_or(contact)
                                    .trim()
                                    .to_owned()
                            })
                            .filter(|name| !name.is_empty())
                            .unwrap_or_else(|| fallback_author.into())
                    } else {
                        a.name.clone()
                    }
                })
                .unwrap_or(fallback_author.into()),
            url: url.into(),
            summary: plain(html).chars().take(10000).collect(),
            media,
            published_at: entry.published.or(entry.updated).unwrap_or_else(Utc::now),
        });
    }
    Ok(entries)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn extracts_html_and_enclosures_without_duplicates() {
        let xml = br#"<rss version="2.0"><channel><title>Artist</title><link>https://example.com</link><description>Art</description><item><guid>p1</guid><title>New &amp; blue</title><link>https://example.com/post/1</link><description><![CDATA[<p>Hello</p><img src="/a.jpg"><img src="/a.jpg"><video src="/b.mp4"></video><img src="javascript:alert(1)">]]></description></item></channel></rss>"#;
        let items = parse(xml, "https://example.com/rss", "Artist").unwrap();
        assert_eq!(items[0].title, "New & blue");
        assert_eq!(items[0].media.len(), 2);
        assert_eq!(items[0].media[0].url, "https://example.com/a.jpg");
        assert_eq!(items[0].media[1].kind, "video");
        assert_eq!(items[0].summary, "Hello");
    }
    #[test]
    fn preserves_rss_author_instead_of_the_parser_role() {
        for (raw, expected) in [
            ("Test Artist", "Test Artist"),
            ("artist@example.com (Aoi)", "Aoi"),
        ] {
            let xml = format!(
                r#"<rss version="2.0"><channel><title>Art</title><link>https://example.com</link><description>Art</description><item><guid>one</guid><title>Drawing</title><author>{raw}</author></item></channel></rss>"#
            );
            let entries = parse(xml.as_bytes(), "https://example.com/feed", "Fallback").unwrap();
            assert_eq!(entries[0].author, expected);
        }
    }
    #[test]
    fn supports_atom() {
        let xml = br#"<feed xmlns="http://www.w3.org/2005/Atom"><title>Art</title><id>feed</id><updated>2026-01-01T00:00:00Z</updated><entry><id>one</id><title>Drawing</title><updated>2026-01-01T00:00:00Z</updated><author><name>Aoi</name></author><link href="https://example.com/one"/></entry></feed>"#;
        let entries = parse(xml, "https://example.com/feed", "Fallback").unwrap();
        assert_eq!(entries[0].author, "Aoi");
        assert_eq!(entries[0].guid, "one");
    }
}
