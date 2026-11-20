use crate::{models::Media, network::web_url};
use chrono::{DateTime, Utc};
use ego_tree::NodeRef;
use scraper::{Html, Node, Selector};
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

fn looks_like_filename(text: &str) -> bool {
    let t = text.trim().to_lowercase();
    if t.is_empty() || t.contains(' ') {
        return false;
    }
    [
        ".png", ".jpg", ".jpeg", ".gif", ".webp", ".avif", ".bmp", ".svg", ".mp4", ".webm", ".mov",
        ".zip", ".rar", ".7z", ".psd", ".clip", ".pdf",
    ]
    .iter()
    .any(|ext| t.ends_with(ext))
}

fn text_content(node: &NodeRef<'_, Node>) -> String {
    let mut s = String::new();
    for child in node.children() {
        match child.value() {
            Node::Text(t) => s.push_str(t),
            Node::Element(_) => s.push_str(&text_content(&child)),
            _ => {}
        }
    }
    s
}

fn should_separate(out: &str, next: &str) -> bool {
    out.ends_with(|c: char| {
        c.is_alphanumeric()
            || matches!(c, '!' | '?' | '.' | ',' | ';' | ':' | ')' | ']' | '}' | '~')
    }) && next.starts_with(|c: char| c.is_alphanumeric() || matches!(c, '(' | '[' | '{'))
}

fn walk_node(node: &NodeRef<'_, Node>, out: &mut String, in_downloads: &mut bool) {
    match node.value() {
        Node::Element(elem) => {
            let name = elem.name();
            // Skip non-content tags completely
            if matches!(
                name,
                "script"
                    | "style"
                    | "noscript"
                    | "svg"
                    | "canvas"
                    | "head"
                    | "template"
                    | "img"
                    | "video"
                    | "audio"
            ) {
                return;
            }

            // Detect section headers
            if matches!(name, "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "div" | "p") {
                let inner = text_content(node).trim().to_lowercase();
                if inner == "downloads / files"
                    || inner == "downloads/files"
                    || inner == "downloads"
                    || inner == "files"
                    || inner == "attachments"
                {
                    *in_downloads = true;
                    return;
                }
                if inner == "content" || inner == "post content" {
                    *in_downloads = false;
                    return;
                }
            }

            if *in_downloads {
                return;
            }

            if name == "figcaption" {
                let fig_text = text_content(node).trim().to_lowercase();
                if looks_like_filename(&fig_text) {
                    return;
                }
            }

            if matches!(name, "br" | "wbr") {
                out.push('\n');
                return;
            }

            let is_p = matches!(
                name,
                "p" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "blockquote"
            );
            let is_block = is_p
                || matches!(
                    name,
                    "div"
                        | "ul"
                        | "ol"
                        | "li"
                        | "pre"
                        | "article"
                        | "section"
                        | "header"
                        | "footer"
                        | "table"
                        | "tr"
                        | "hr"
                );

            if is_p && !out.is_empty() {
                while !out.ends_with("\n\n") {
                    out.push('\n');
                }
            } else if is_block && !out.is_empty() && !out.ends_with('\n') {
                out.push('\n');
            }

            for child in node.children() {
                walk_node(&child, out, in_downloads);
            }

            if is_p && !out.is_empty() {
                while !out.ends_with("\n\n") {
                    out.push('\n');
                }
            } else if is_block && !out.is_empty() && !out.ends_with('\n') {
                out.push('\n');
            }
        }
        Node::Text(text) if !*in_downloads => {
            let t: &str = text;
            if !t.is_empty() {
                if !out.is_empty()
                    && !out.ends_with('\n')
                    && !out.ends_with(' ')
                    && !t.starts_with(' ')
                    && should_separate(out, t)
                {
                    out.push(' ');
                }
                out.push_str(t);
            }
        }
        _ => {}
    }
}

fn format_plain_lines(raw: &str) -> String {
    let mut lines = Vec::new();
    for raw_line in raw.lines() {
        let clean = raw_line
            .replace('\u{a0}', " ")
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        if !clean.is_empty() {
            lines.push(clean);
        } else if lines.last().is_some_and(|l| !l.is_empty()) {
            lines.push(String::new());
        }
    }
    while lines.last().is_some_and(|l| l.is_empty()) {
        lines.pop();
    }
    lines.join("\n")
}

pub fn clean_plain(text: &str) -> String {
    let trimmed = text.trim();
    if let Some((before, after)) = trimmed.split_once(" Content ") {
        let before_lower = before.to_lowercase();
        if before_lower.starts_with("downloads / files")
            || before_lower.starts_with("downloads/files")
            || before_lower.starts_with("downloads")
            || before_lower.starts_with("files")
        {
            return format_plain_lines(after);
        }
    } else if let Some((before, after)) = trimmed.split_once("\nContent\n") {
        let before_lower = before.to_lowercase();
        if before_lower.starts_with("downloads / files")
            || before_lower.starts_with("downloads/files")
            || before_lower.starts_with("downloads")
            || before_lower.starts_with("files")
        {
            return format_plain_lines(after);
        }
    } else {
        let lower = trimmed.to_lowercase();
        if lower.starts_with("downloads / files")
            || lower.starts_with("downloads/files")
            || lower == "downloads"
            || lower == "content"
        {
            return String::new();
        }
    }
    format_plain_lines(trimmed)
}

pub fn clean_summary(html: &str) -> String {
    let doc = Html::parse_fragment(html);
    let mut out = String::new();
    let mut in_downloads = false;
    for child in doc.root_element().children() {
        walk_node(&child, &mut out, &mut in_downloads);
    }
    let formatted = format_plain_lines(&out);
    clean_plain(&formatted)
}

pub fn plain_title(html: &str) -> String {
    Html::parse_fragment(html)
        .root_element()
        .text()
        .collect::<Vec<_>>()
        .join(" ")
        .replace('\u{a0}', " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

#[allow(dead_code)]
pub fn plain(html: &str) -> String {
    clean_summary(html)
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
            .map(|t| plain_title(&t.content))
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
            summary: clean_summary(html).chars().take(10000).collect(),
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
    #[test]
    fn strips_rsshub_downloads_and_content_boilerplate() {
        let html = r#"<h2>Downloads / Files</h2><ul><li style="list-style-type:none"><a href="https://file.example.com/1.gif" target="_blank"><figure><img src="https://img.example.com/1.gif" alt="CRASH.gif"><figcaption>CRASH.gif</figcaption></figure></a></li><li style="list-style-type:none"><a href="https://file.example.com/2.gif"><figure><img src="https://img.example.com/2.gif" alt="SHAME2.gif"><figcaption>SHAME2.gif</figcaption></figure></a></li></ul><h2>Content</h2><p>Break my shit again Tatsu-maki, SEE WHAT HAPPENS! 😤</p><p><em>Happy Mondays everyone!</em><em>Some food for you~</em></p><p></p>"#;
        let summary = clean_summary(html);
        assert_eq!(
            summary,
            "Break my shit again Tatsu-maki, SEE WHAT HAPPENS! 😤\n\nHappy Mondays everyone! Some food for you~"
        );
    }
    #[test]
    fn cleans_posts_with_only_downloads_and_no_content() {
        let html = r#"<h2>Downloads / Files</h2><ul><li><figure><img src="1.png"><figcaption>1.png</figcaption></figure></li></ul>"#;
        assert_eq!(clean_summary(html), "");
    }
    #[test]
    fn clean_plain_recovers_already_flattened_boilerplate() {
        let legacy = "Downloads / Files CRASH.gif SHAME2.gif tumblr_o84mijLFrM1s4s960o1_540.gif Content Break my shit again Tatsu-maki, SEE WHAT HAPPENS! 😤 Happy Mondays everyone! Some food for you~";
        assert_eq!(
            clean_plain(legacy),
            "Break my shit again Tatsu-maki, SEE WHAT HAPPENS! 😤 Happy Mondays everyone! Some food for you~"
        );
        let only_files = "Downloads / Files 1.png 2.png 3.gif";
        assert_eq!(clean_plain(only_files), "");
    }
    #[test]
    fn formats_paragraphs_and_breaks_cleanly() {
        let html = "<p>First paragraph.</p><p>Second paragraph<br>with a line break.</p>";
        assert_eq!(
            clean_summary(html),
            "First paragraph.\n\nSecond paragraph\nwith a line break."
        );
    }
}
