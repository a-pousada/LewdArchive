use anyhow::{Context, Result, bail};
use reqwest::{Client, Response, header::HeaderMap};
use std::{
    net::{IpAddr, SocketAddr},
    sync::{Arc, OnceLock},
    time::Duration,
};
use url::Url;

pub fn web_url(raw: &str) -> Result<Url> {
    let url = Url::parse(raw).context("Invalid URL")?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
    {
        bail!("Use an HTTP(S) URL without credentials");
    }
    Ok(url)
}
fn public_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => {
            !(ip.is_private()
                || ip.is_loopback()
                || ip.is_link_local()
                || ip.is_broadcast()
                || ip.is_documentation()
                || ip.is_unspecified()
                || ip.is_multicast()
                || ip.octets()[0] == 0
                || ip.octets()[0] >= 240
                || (ip.octets()[0] == 198 && (18..=19).contains(&ip.octets()[1]))
                || (ip.octets()[0] == 192 && ip.octets()[1] == 0 && ip.octets()[2] == 0)
                || (ip.octets()[0] == 100 && (64..=127).contains(&ip.octets()[1])))
        }
        IpAddr::V6(ip) => {
            if let Some(v4) = ip.to_ipv4_mapped() {
                public_ip(IpAddr::V4(v4))
            } else {
                let s = ip.segments();
                !(ip.is_loopback()
                    || ip.is_unspecified()
                    || ip.is_multicast()
                    || s[0] & 0xe000 != 0x2000
                    || (s[0] == 0x2001 && s[1] < 0x0200)
                    || s[0] == 0x2002
                    || s[0] == 0x3fff
                    || s[0] & 0xfe00 == 0xfc00
                    || s[0] & 0xffc0 == 0xfe80
                    || (s[0] == 0x2001 && s[1] == 0xdb8))
            }
        }
    }
}

fn is_allowed_private_host(host: &str) -> bool {
    std::env::var("ALLOW_PRIVATE_FEED_HOSTS")
        .unwrap_or_default()
        .split(',')
        .any(|h| !h.is_empty() && h.trim() == host)
}

fn validate_host_literal(url: &Url, allow_private: bool, is_webhook: bool) -> Result<()> {
    let host = url
        .host_str()
        .context("Missing host")?
        .trim_matches(['[', ']']);
    if let Ok(ip) = host.parse::<IpAddr>() {
        let allowed = allow_private && is_allowed_private_host(host);
        if !public_ip(ip) && !allowed {
            if is_webhook {
                bail!("Webhook destination must use a public address");
            }
            bail!(
                "Private or reserved host blocked; configure ALLOW_PRIVATE_FEED_HOSTS for a local RSSHub"
            );
        }
    }
    Ok(())
}

/// Custom DNS resolver enforcing anti-SSRF IP checks on every connection inside pooled clients.
struct SafeResolver {
    allow_private: bool,
}

impl reqwest::dns::Resolve for SafeResolver {
    fn resolve(&self, name: reqwest::dns::Name) -> reqwest::dns::Resolving {
        let host = name.as_str().trim_matches(['[', ']']).to_owned();
        let allow_private = self.allow_private;
        Box::pin(async move {
            let addresses: Vec<SocketAddr> = tokio::net::lookup_host((host.as_str(), 0))
                .await
                .map_err(|_| std::io::Error::other("Unable to resolve the host"))?
                .collect();
            let allowed = allow_private && is_allowed_private_host(&host);
            if addresses.is_empty() {
                return Err(
                    Box::new(std::io::Error::other("Unable to resolve the host"))
                        as Box<dyn std::error::Error + Send + Sync>,
                );
            }
            if !allowed && addresses.iter().any(|a| !public_ip(a.ip())) {
                return Err(Box::new(std::io::Error::new(
                    std::io::ErrorKind::PermissionDenied,
                    "Private or reserved host blocked; configure ALLOW_PRIVATE_FEED_HOSTS for a local RSSHub",
                ))
                    as Box<dyn std::error::Error + Send + Sync>);
            }
            Ok(Box::new(addresses.into_iter()) as reqwest::dns::Addrs)
        })
    }
}

fn build_client(allow_private: bool, timeout_secs: u64, max_idle: usize, ua: &str) -> Client {
    Client::builder()
        .dns_resolver(Arc::new(SafeResolver { allow_private }))
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(timeout_secs))
        .connect_timeout(Duration::from_secs(10))
        .pool_idle_timeout(Duration::from_secs(90))
        .pool_max_idle_per_host(max_idle)
        .user_agent(ua)
        .build()
        .expect("HTTP client configuration must be valid")
}

fn pooled_feed_client(allow_private: bool) -> &'static Client {
    static PUBLIC: OnceLock<Client> = OnceLock::new();
    static PRIVATE: OnceLock<Client> = OnceLock::new();
    if allow_private {
        PRIVATE.get_or_init(|| build_client(true, 90, 8, "LewdArchive/0.1 (+RSS reader)"))
    } else {
        PUBLIC.get_or_init(|| build_client(false, 90, 8, "LewdArchive/0.1 (+RSS reader)"))
    }
}

fn pooled_webhook_client() -> &'static Client {
    static WEBHOOK: OnceLock<Client> = OnceLock::new();
    WEBHOOK.get_or_init(|| build_client(false, 30, 4, "LewdArchive/0.1 (+community webhook)"))
}

pub async fn get(raw: &str, headers: HeaderMap, allow_private: bool) -> Result<Response> {
    let mut url = web_url(raw)?;
    for _ in 0..5 {
        validate_host_literal(&url, allow_private, false)?;
        let response = pooled_feed_client(allow_private)
            .get(url.clone())
            .headers(headers.clone())
            .send()
            .await
            .map_err(|e| anyhow::anyhow!(e.without_url()))?;
        if response.status().is_redirection() && response.status().as_u16() != 304 {
            let location = response
                .headers()
                .get("location")
                .context("Redirect has no destination")?
                .to_str()?;
            url = web_url(url.join(location)?.as_str())?;
        } else {
            return Ok(response);
        }
    }
    bail!("Too many redirects")
}
pub async fn limited_body(mut response: Response, limit: usize) -> Result<Vec<u8>> {
    if response.content_length().is_some_and(|n| n > limit as u64) {
        bail!("Response exceeds the size limit");
    }
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|e| e.without_url())? {
        if body.len() + chunk.len() > limit {
            bail!("Response exceeds the size limit");
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}
/// Validate HTTPS + public literal hosts and return the shared DNS-guarded webhook pool client.
pub async fn public_webhook_client(url: &Url) -> Result<Client> {
    if url.scheme() != "https" {
        bail!("Community webhooks require HTTPS");
    }
    validate_host_literal(url, false, true)?;
    Ok(pooled_webhook_client().clone())
}
pub fn webhook(raw: &str) -> Result<Url> {
    let url = web_url(raw)?;
    if url.cannot_be_a_base() {
        bail!("Use an HTTP(S) URL without credentials");
    }
    Ok(url)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn community_webhooks_reject_private_and_insecure_destinations() {
        assert!(
            public_webhook_client(&Url::parse("http://example.com/webhook").unwrap())
                .await
                .is_err()
        );
        assert!(
            public_webhook_client(&Url::parse("https://127.0.0.1/webhook").unwrap())
                .await
                .is_err()
        );
        assert!(
            public_webhook_client(&Url::parse("https://[::1]/webhook").unwrap())
                .await
                .is_err()
        );
        assert!(
            get("http://127.0.0.1:9/rss", HeaderMap::new(), false)
                .await
                .is_err()
        );
        assert!(
            get("http://169.254.169.254/latest", HeaderMap::new(), false)
                .await
                .is_err()
        );
    }
    #[test]
    fn blocks_private_networks() {
        for raw in [
            "127.0.0.1",
            "10.1.2.3",
            "169.254.169.254",
            "100.64.0.1",
            "::1",
            "::ffff:127.0.0.1",
            "fd00::1",
            "::127.0.0.1",
            "64:ff9b::7f00:1",
            "198.18.0.1",
        ] {
            assert!(!public_ip(raw.parse().unwrap()), "{raw}");
        }
        assert!(public_ip("1.1.1.1".parse().unwrap()));
    }
    #[test]
    fn rejects_unsafe_urls() {
        assert!(web_url("file:///etc/passwd").is_err());
        assert!(web_url("https://user:pass@example.com/feed").is_err());
        assert!(webhook("file:///etc/passwd").is_err());
        assert!(webhook("https://user:pass@example.com/webhook").is_err());
        assert!(webhook("https://discord.com/api/webhooks/123/token").is_ok());
        assert!(webhook("https://api.fluxer.app/api/webhooks/123/token").is_ok());
        assert!(webhook("https://example.com/webhook").is_ok());
    }
}
