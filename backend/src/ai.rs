//! Local WD inference. Feed content never controls the configured service endpoint.
use crate::{models::Media, network};
use anyhow::{Context, Result, ensure};
use serde::Deserialize;
use serde_json::Value;
use std::{collections::HashMap, time::Duration};

pub const MODEL: &str = "SmilingWolf/wd-swinv2-tagger-v3";

pub struct Tagger {
    endpoint: String,
    client: reqwest::Client,
    slots: tokio::sync::Semaphore,
    #[cfg(test)]
    fixture: Option<(String, Vec<u8>)>,
}

#[derive(Deserialize)]
struct Prediction {
    name: String,
    score: f64,
}

#[derive(Deserialize)]
struct Output {
    tags: Vec<Prediction>,
}

pub fn has_images(media: &Value) -> bool {
    serde_json::from_value::<Vec<Media>>(media.clone())
        .is_ok_and(|items| items.iter().any(|item| item.kind == "image"))
}

impl Tagger {
    pub fn from_env() -> Result<Option<Self>> {
        let base = std::env::var("WD_TAGGER_URL").unwrap_or_default();
        if base.trim().is_empty() {
            if std::env::var("AI_OLLAMA_URL").is_ok_and(|v| !v.trim().is_empty()) {
                tracing::warn!(
                    "Ollama tagging was removed; configure WD_TAGGER_URL to enable image tagging"
                );
            }
            return Ok(None);
        }
        let url = network::web_url(base.trim())?;
        ensure!(
            url.query().is_none() && url.fragment().is_none(),
            "WD_TAGGER_URL must be a base URL without query or fragment"
        );
        Self::new(format!("{}/v1/tag", url.as_str().trim_end_matches('/'))).map(Some)
    }

    pub(crate) fn new(endpoint: String) -> Result<Self> {
        Ok(Self {
            endpoint,
            client: reqwest::Client::builder()
                .no_proxy()
                .redirect(reqwest::redirect::Policy::none())
                .connect_timeout(Duration::from_secs(10))
                .timeout(Duration::from_secs(90))
                .build()?,
            slots: tokio::sync::Semaphore::new(1),
            #[cfg(test)]
            fixture: None,
        })
    }

    #[cfg(test)]
    pub(crate) fn with_fixture(mut self, url: &str, bytes: &[u8]) -> Self {
        self.fixture = Some((url.into(), bytes.into()));
        self
    }

    async fn fetch_image(&self, url: &str) -> Result<Vec<u8>> {
        #[cfg(test)]
        if let Some((fixture_url, bytes)) = &self.fixture
            && url == fixture_url
        {
            return Ok(bytes.clone());
        }
        let response = network::get(url, reqwest::header::HeaderMap::new(), false)
            .await
            .context("Unable to fetch a public image for analysis")?;
        ensure!(
            response.status().is_success(),
            "Image source is unavailable"
        );
        let content_type = response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .split(';')
            .next()
            .unwrap_or("")
            .trim();
        ensure!(
            matches!(content_type, "image/jpeg" | "image/png" | "image/webp"),
            "WD images must be JPEG, PNG or WebP"
        );
        network::limited_body(response, 5 * 1024 * 1024)
            .await
            .context("Image exceeds the 5 MiB limit")
    }

    async fn classify(&self, image: Vec<u8>) -> Result<Vec<Prediction>> {
        let response = self
            .client
            .post(&self.endpoint)
            .header(reqwest::header::CONTENT_TYPE, "application/octet-stream")
            .body(image)
            .send()
            .await
            .context("WD tagger is unavailable")?;
        ensure!(
            response.status().is_success(),
            "WD tagger returned HTTP {}",
            response.status().as_u16()
        );
        let body = network::limited_body(response, 64 * 1024)
            .await
            .context("WD response is too large")?;
        let result: Output =
            serde_json::from_slice(&body).context("WD returned invalid tag JSON")?;
        ensure!(result.tags.len() <= 30, "WD returned too many tags");
        for tag in &result.tags {
            ensure!(
                tag.score.is_finite() && (0.0..=1.0).contains(&tag.score),
                "WD returned an invalid confidence score"
            );
        }
        Ok(result.tags)
    }

    pub async fn suggest(&self, media: &Value) -> Result<Vec<String>> {
        let media: Vec<Media> = serde_json::from_value(media.clone())?;
        let images: Vec<_> = media.iter().filter(|m| m.kind == "image").take(2).collect();
        // Text/video-only posts finish immediately instead of retrying and delaying delivery.
        if images.is_empty() {
            return Ok(Vec::new());
        }
        let _permit = self
            .slots
            .try_acquire()
            .context("WD is busy. Try again shortly")?;
        tokio::time::timeout(Duration::from_secs(120), async {
            let mut scores: HashMap<String, f64> = HashMap::new();
            let mut successes = 0;
            let mut last_error = None;
            for image in images {
                let result = async {
                    let bytes = self.fetch_image(&image.url).await?;
                    let predictions = self.classify(bytes).await?;
                    let mut normalized = Vec::new();
                    for tag in predictions {
                        let names = crate::community::tags::normalize(&[tag.name])
                            .map_err(|e| anyhow::anyhow!("Invalid WD tags: {}", e.1))?;
                        if let Some(name) = names.into_iter().next() {
                            normalized.push((name, tag.score));
                        }
                    }
                    Ok::<_, anyhow::Error>(normalized)
                }
                .await;
                match result {
                    Ok(tags) => {
                        successes += 1;
                        for (name, score) in tags {
                            scores
                                .entry(name)
                                .and_modify(|old| *old = old.max(score))
                                .or_insert(score);
                        }
                    }
                    Err(error) => {
                        last_error = Some(error);
                    }
                }
            }
            if successes == 0 {
                return Err(last_error.context("No images could be analyzed")?);
            }
            if last_error.is_some() {
                tracing::warn!("WD analyzed only part of a post's images");
            }
            let mut ranked: Vec<_> = scores.into_iter().collect();
            ranked.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
            Ok(ranked.into_iter().take(30).map(|(name, _)| name).collect())
        })
        .await
        .context("WD request timed out")?
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{Json, Router, body::Bytes, routing::post};
    use serde_json::json;

    #[tokio::test]
    async fn image_contract_merge_and_invalid_output() {
        let app = Router::new()
            .route(
                "/v1/tag",
                post(|body: Bytes| async move {
                    assert_eq!(&body[..], b"fixture");
                    Json(json!({"tags":[
                        {"name":"Blue Sky","score":0.9},
                        {"name":"blue_sky","score":0.7},
                        {"name":"character_(series)","score":0.95}
                    ]}))
                }),
            )
            .route(
                "/bad",
                post(|| async { Json(json!({"tags":[{"name":"bad","score":2}]})) }),
            )
            .route(
                "/malformed",
                post(|| async { Json(json!({"tags":["bad"]})) }),
            )
            .route(
                "/mention",
                post(|| async { Json(json!({"tags":[{"name":"@everyone","score":0.9}]})) }),
            );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let task = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        let media = json!([
            {"kind":"image","url":"https://fixtures.invalid/image.png"},
            {"kind":"image","url":"https://fixtures.invalid/image.png"},
            {"kind":"image","url":"http://127.0.0.1/never-fetched"}
        ]);
        let tagger = Tagger::new(format!("{base}/v1/tag"))
            .unwrap()
            .with_fixture("https://fixtures.invalid/image.png", b"fixture");
        assert_eq!(
            tagger.suggest(&media).await.unwrap(),
            vec!["character_(series)", "blue_sky"]
        );
        assert!(tagger.suggest(&json!([])).await.unwrap().is_empty());
        for path in ["bad", "malformed", "mention"] {
            let bad = Tagger::new(format!("{base}/{path}"))
                .unwrap()
                .with_fixture("https://fixtures.invalid/image.png", b"fixture");
            assert!(bad.suggest(&media).await.is_err());
        }
        let private = json!([{"kind":"image","url":"http://127.0.0.1/blocked"}]);
        assert!(tagger.suggest(&private).await.is_err());
        let partial = json!([
            {"kind":"image","url":"http://127.0.0.1/blocked"},
            {"kind":"image","url":"https://fixtures.invalid/image.png"}
        ]);
        assert_eq!(tagger.suggest(&partial).await.unwrap().len(), 2);
        task.abort();
    }
}
