use anyhow::{Context, Result, bail};
use reqwest::{Client, Response, header::HeaderMap};
use std::{
    net::{IpAddr, SocketAddr},
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
pub async fn get(raw: &str, headers: HeaderMap, allow_private: bool) -> Result<Response> {
    let mut url = web_url(raw)?;
    for _ in 0..5 {
        let host = url
            .host_str()
            .context("Missing host")?
            .trim_matches(['[', ']']);
        let port = url.port_or_known_default().unwrap_or(443);
        let addresses: Vec<SocketAddr> = tokio::net::lookup_host((host, port))
            .await
            .context("Unable to resolve the host")?
            .collect();
        let allowed = allow_private
            && std::env::var("ALLOW_PRIVATE_FEED_HOSTS")
                .unwrap_or_default()
                .split(',')
                .any(|h| !h.is_empty() && h.trim() == host);
        if addresses.is_empty() || (!allowed && addresses.iter().any(|a| !public_ip(a.ip()))) {
            bail!(
                "Private or reserved host blocked; configure ALLOW_PRIVATE_FEED_HOSTS for a local RSSHub"
            );
        }
        let client = Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(Duration::from_secs(90))
            .connect_timeout(Duration::from_secs(10))
            .user_agent("LewdArchive/0.1 (+RSS reader)")
            .resolve_to_addrs(host, &addresses)
            .build()?;
        let response = client
            .get(url.clone())
            .headers(headers.clone())
            .send()
            .await
            .map_err(|e| e.without_url())?;
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
/// Resolve and pin a public destination before a server administrator can send a webhook.
pub async fn public_webhook_client(url: &Url) -> Result<Client> {
    if url.scheme() != "https" {
        bail!("Community webhooks require HTTPS");
    }
    let host = url
        .host_str()
        .context("Missing webhook host")?
        .trim_matches(['[', ']']);
    let addresses: Vec<SocketAddr> =
        tokio::net::lookup_host((host, url.port_or_known_default().unwrap_or(443)))
            .await
            .context("Unable to resolve webhook host")?
            .collect();
    if addresses.is_empty() || addresses.iter().any(|a| !public_ip(a.ip())) {
        bail!("Webhook destination must use a public address");
    }
    Ok(Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(30))
        .connect_timeout(Duration::from_secs(10))
        .resolve_to_addrs(host, &addresses)
        .user_agent("LewdArchive/0.1 (+community webhook)")
        .build()?)
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
