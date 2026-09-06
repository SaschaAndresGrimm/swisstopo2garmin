//! In-memory [`Http`] fake for offline, deterministic tests.
//!
//! Beyond serving bytes, it can fail partway through a stream. That is what lets the
//! resume behaviour of [`crate::download`] be tested — the case that matters most,
//! because a 4.8 GB acquisition will be interrupted in practice (SPEC.md FR-D2).

use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use bytes::Bytes;
use futures_util::stream::BoxStream;

use crate::error::{Error, Result};
use crate::http::{HeadInfo, Http};

#[derive(Default)]
pub struct FakeHttp {
    bodies: Mutex<HashMap<String, Vec<u8>>>,
    json: Mutex<HashMap<String, serde_json::Value>>,
    /// Bytes to emit before failing a stream; `None` means never fail.
    fail_after: Mutex<Option<usize>>,
    /// How many times a stream failure has been injected.
    pub failures: Arc<AtomicUsize>,
    /// Number of `get_stream` calls, i.e. connection attempts.
    pub stream_calls: Arc<AtomicUsize>,
    pub chunk_size: usize,
}

impl FakeHttp {
    pub fn new() -> Self {
        Self {
            chunk_size: 8192,
            ..Default::default()
        }
    }

    pub fn with_body(self, url: &str, body: Vec<u8>) -> Self {
        self.bodies.lock().unwrap().insert(url.to_string(), body);
        self
    }

    pub fn with_json(self, url: &str, value: serde_json::Value) -> Self {
        self.json.lock().unwrap().insert(url.to_string(), value);
        self
    }

    /// Inject one failure after `n` bytes of each new stream, until cleared.
    pub fn failing_after(self, n: usize) -> Self {
        *self.fail_after.lock().unwrap() = Some(n);
        self
    }

    pub fn clear_failures(&self) {
        *self.fail_after.lock().unwrap() = None;
    }

    fn body(&self, url: &str) -> Result<Vec<u8>> {
        self.bodies
            .lock()
            .unwrap()
            .get(url)
            .cloned()
            .ok_or_else(|| Error::Status {
                url: url.into(),
                status: 404,
            })
    }
}

#[async_trait]
impl Http for FakeHttp {
    async fn head(&self, url: &str) -> Result<HeadInfo> {
        let body = self.body(url)?;
        Ok(HeadInfo {
            len: Some(body.len() as u64),
            etag: Some("\"fake\"".into()),
            last_modified: None,
            accept_ranges: true,
        })
    }

    async fn get_json(&self, url: &str) -> Result<serde_json::Value> {
        self.json
            .lock()
            .unwrap()
            .get(url)
            .cloned()
            .ok_or_else(|| Error::Status {
                url: url.into(),
                status: 404,
            })
    }

    async fn get_stream(&self, url: &str, from: u64) -> Result<BoxStream<'static, Result<Bytes>>> {
        self.stream_calls.fetch_add(1, Ordering::SeqCst);
        let body = self.body(url)?;
        let from = from as usize;
        if from > body.len() {
            return Err(Error::Status {
                url: url.into(),
                status: 416,
            });
        }
        let tail = body[from..].to_vec();
        let chunk = self.chunk_size;
        let limit = *self.fail_after.lock().unwrap();
        let failures = self.failures.clone();

        let stream = futures_util::stream::unfold(
            (0usize, tail, limit, failures),
            move |(mut pos, data, limit, failures)| async move {
                if pos >= data.len() {
                    return None;
                }
                if let Some(lim) = limit {
                    if pos >= lim {
                        failures.fetch_add(1, Ordering::SeqCst);
                        return Some((
                            Err(Error::NoRangeSupport {
                                url: "injected mid-stream failure".into(),
                            }),
                            (usize::MAX, data, limit, failures),
                        ));
                    }
                }
                let mut end = (pos + chunk).min(data.len());
                if let Some(lim) = limit {
                    end = end.min(lim.max(pos + 1)).min(data.len());
                }
                let out = Bytes::copy_from_slice(&data[pos..end]);
                pos = end;
                Some((Ok(out), (pos, data, limit, failures)))
            },
        );
        Ok(Box::pin(stream))
    }

    async fn get_range(&self, url: &str, from: u64, to: u64) -> Result<Bytes> {
        let body = self.body(url)?;
        let from = (from as usize).min(body.len());
        let to = ((to as usize) + 1).min(body.len());
        Ok(Bytes::copy_from_slice(&body[from..to]))
    }
}
