//! HTTP access behind a trait.
//!
//! The trait exists so the STAC client, the downloader and the cache can be tested
//! offline and deterministically (PLAN.md M1 task 3) without standing up a mock server.
//! `FakeHttp` can also inject mid-stream failures, which is how the resume behaviour in
//! [`crate::download`] is tested.

use async_trait::async_trait;
use bytes::Bytes;
use futures_util::stream::BoxStream;

use crate::error::{Error, Result};

#[derive(Debug, Clone, Default)]
pub struct HeadInfo {
    pub len: Option<u64>,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
    pub accept_ranges: bool,
}

#[async_trait]
pub trait Http: Send + Sync {
    async fn head(&self, url: &str) -> Result<HeadInfo>;

    async fn get_json(&self, url: &str) -> Result<serde_json::Value>;

    /// Byte stream of `url` starting at absolute offset `from`.
    async fn get_stream(&self, url: &str, from: u64) -> Result<BoxStream<'static, Result<Bytes>>>;

    /// Read exactly the bytes in `[from, to]` inclusive. Used for zip directory probing.
    async fn get_range(&self, url: &str, from: u64, to: u64) -> Result<Bytes>;
}

// ---------------------------------------------------------------------------
// reqwest implementation
// ---------------------------------------------------------------------------

pub struct ReqwestHttp {
    client: reqwest::Client,
}

impl ReqwestHttp {
    pub fn new() -> Result<Self> {
        let client = reqwest::Client::builder()
            .user_agent(concat!("swisstopo2garmin/", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|source| Error::Http {
                url: "<client>".into(),
                source,
            })?;
        Ok(Self { client })
    }
}

impl ReqwestHttp {
    fn err(url: &str) -> impl Fn(reqwest::Error) -> Error + '_ {
        move |source| Error::Http {
            url: url.to_string(),
            source,
        }
    }
}

#[async_trait]
impl Http for ReqwestHttp {
    async fn head(&self, url: &str) -> Result<HeadInfo> {
        let resp = self.client.head(url).send().await.map_err(Self::err(url))?;
        if !resp.status().is_success() {
            return Err(Error::Status {
                url: url.into(),
                status: resp.status().as_u16(),
            });
        }
        let h = resp.headers();
        let text = |k: reqwest::header::HeaderName| {
            h.get(k).and_then(|v| v.to_str().ok()).map(str::to_owned)
        };
        // reqwest's `content_length()` reports the *body* length, which is 0 for a
        // HEAD response. The Content-Length header must be read directly, or every
        // size-dependent caller sees 0.
        let declared_len = h
            .get(reqwest::header::CONTENT_LENGTH)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<u64>().ok());
        Ok(HeadInfo {
            len: declared_len.or_else(|| resp.content_length()),
            etag: text(reqwest::header::ETAG),
            last_modified: text(reqwest::header::LAST_MODIFIED),
            // NB: data.geo.admin.ch does not send Accept-Ranges on HEAD even though
            // ranged GETs work, so this flag must not be used to gate resumption.
            accept_ranges: h
                .get(reqwest::header::ACCEPT_RANGES)
                .and_then(|v| v.to_str().ok())
                .map(|v| v.contains("bytes"))
                .unwrap_or(false),
        })
    }

    async fn get_json(&self, url: &str) -> Result<serde_json::Value> {
        let resp = self
            .client
            .get(url)
            .header(reqwest::header::ACCEPT, "application/json")
            .send()
            .await
            .map_err(Self::err(url))?;
        if !resp.status().is_success() {
            return Err(Error::Status {
                url: url.into(),
                status: resp.status().as_u16(),
            });
        }
        let bytes = resp.bytes().await.map_err(Self::err(url))?;
        Ok(serde_json::from_slice(&bytes)?)
    }

    async fn get_stream(&self, url: &str, from: u64) -> Result<BoxStream<'static, Result<Bytes>>> {
        let mut req = self.client.get(url);
        if from > 0 {
            req = req.header(reqwest::header::RANGE, format!("bytes={from}-"));
        }
        let resp = req.send().await.map_err(Self::err(url))?;
        if !resp.status().is_success() {
            return Err(Error::Status {
                url: url.into(),
                status: resp.status().as_u16(),
            });
        }
        let owned = url.to_string();
        let stream = futures_util::StreamExt::map(resp.bytes_stream(), move |r| {
            r.map_err(|source| Error::Http {
                url: owned.clone(),
                source,
            })
        });
        Ok(Box::pin(stream))
    }

    async fn get_range(&self, url: &str, from: u64, to: u64) -> Result<Bytes> {
        let resp = self
            .client
            .get(url)
            .header(reqwest::header::RANGE, format!("bytes={from}-{to}"))
            .send()
            .await
            .map_err(Self::err(url))?;
        if !resp.status().is_success() {
            return Err(Error::Status {
                url: url.into(),
                status: resp.status().as_u16(),
            });
        }
        resp.bytes().await.map_err(Self::err(url))
    }
}
