//! Resumable, verified downloads (SPEC.md FR-D1..D4).
//!
//! Two modes:
//!
//! * [`download`] — plain byte-for-byte fetch. Resumes across *process* restarts from
//!   the length of the partial file.
//! * [`download_zip_member_inflated`] — streams a ZIP's single DEFLATE member and
//!   inflates on the fly, so acquiring swissTLM3D needs 10.78 GB of disk rather than
//!   the 15.6 GB that download-then-unzip would peak at.
//!
//! Both reconnect with an HTTP `Range` request on a mid-stream failure and keep feeding
//! the *same* hasher and decompressor, so a dropped connection does not restart the
//! transfer. A **process** restart can only be resumed in the plain mode: raw DEFLATE
//! state cannot be serialised (docs/m0-findings.md §1.3).

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use futures_util::StreamExt;
use sha2::{Digest as _, Sha256};
use tokio::io::AsyncWriteExt;

use crate::error::{Error, Result};
use crate::http::Http;
use crate::stac::Digest;
use crate::zip;

const MAX_RETRIES: u32 = 12;
const CHUNK_HINT: usize = 1 << 20;

/// Cooperative cancellation. Checked between chunks, so cancelling is prompt without
/// leaving a half-written file visible (FR-70/FR-2).
#[derive(Clone, Default)]
pub struct Cancel(Arc<AtomicBool>);

impl Cancel {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn cancel(&self) {
        self.0.store(true, Ordering::SeqCst);
    }
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
    /// Public form of the internal check, for callers outside this module.
    pub fn check_cancelled(&self) -> Result<()> {
        self.check()
    }

    fn check(&self) -> Result<()> {
        if self.is_cancelled() {
            Err(Error::Cancelled)
        } else {
            Ok(())
        }
    }
}

#[derive(Debug, Clone, Copy, Default, serde::Serialize)]
pub struct Progress {
    /// Compressed/wire bytes read so far.
    pub read: u64,
    /// Total wire bytes, when known.
    pub total: Option<u64>,
    /// Bytes actually written to disk (differs from `read` when inflating).
    pub written: u64,
    pub retries: u32,
}

pub type ProgressFn<'a> = &'a mut (dyn FnMut(Progress) + Send);

fn backoff(retry: u32) -> Duration {
    Duration::from_secs(1u64 << retry.min(5))
}

async fn create_part(dest: &Path) -> Result<(PathBuf, tokio::fs::File)> {
    let part = dest.with_extension(format!(
        "{}part",
        dest.extension()
            .map(|e| format!("{}.", e.to_string_lossy()))
            .unwrap_or_default()
    ));
    if let Some(parent) = part.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| Error::io(parent, e))?;
    }
    let file = tokio::fs::File::create(&part)
        .await
        .map_err(|e| Error::io(&part, e))?;
    Ok((part, file))
}

/// Deletes the `.part` file unless [`PartGuard::keep`] is called.
///
/// Every failure path must clean up, not just the ones we remember to write: a
/// cancelled 4.8 GB download previously left a multi-gigabyte orphan on disk. Drop is
/// synchronous, so this uses `std::fs`.
struct PartGuard {
    path: Option<PathBuf>,
}

impl PartGuard {
    fn new(path: PathBuf) -> Self {
        Self { path: Some(path) }
    }
    /// Disarm: the file has been renamed into place and must survive.
    fn keep(mut self) {
        self.path = None;
    }
}

impl Drop for PartGuard {
    fn drop(&mut self) {
        if let Some(p) = self.path.take() {
            let _ = std::fs::remove_file(&p);
        }
    }
}

/// Plain resumable download with optional checksum verification.
pub async fn download(
    http: &dyn Http,
    url: &str,
    dest: &Path,
    expected: Option<&Digest>,
    cancel: &Cancel,
    progress: ProgressFn<'_>,
) -> Result<PathBuf> {
    let head = http.head(url).await?;
    let total = head.len;

    let (part, mut file) = create_part(dest).await?;
    let guard = PartGuard::new(part.clone());
    let mut hasher = Sha256::new();
    let mut pos: u64 = 0;
    let mut retries = 0u32;

    'outer: loop {
        cancel.check()?;
        let mut stream = match http.get_stream(url, pos).await {
            Ok(s) => s,
            Err(e) => {
                retries += 1;
                if retries > MAX_RETRIES {
                    return Err(e);
                }
                tokio::time::sleep(backoff(retries)).await;
                continue;
            }
        };
        while let Some(chunk) = stream.next().await {
            match chunk {
                Ok(bytes) => {
                    cancel.check()?;
                    hasher.update(&bytes);
                    file.write_all(&bytes)
                        .await
                        .map_err(|e| Error::io(&part, e))?;
                    pos += bytes.len() as u64;
                    progress(Progress {
                        read: pos,
                        total,
                        written: pos,
                        retries,
                    });
                }
                Err(_) => {
                    retries += 1;
                    if retries > MAX_RETRIES {
                        return Err(Error::NoRangeSupport { url: url.into() });
                    }
                    tokio::time::sleep(backoff(retries)).await;
                    continue 'outer;
                }
            }
        }
        break;
    }

    file.flush().await.map_err(|e| Error::io(&part, e))?;
    drop(file);

    if let Some(t) = total {
        if pos != t {
            return Err(Error::SizeMismatch {
                expected: t,
                actual: pos,
            });
        }
    }
    if let Some(d) = expected {
        d.verify(hasher)?;
    }

    tokio::fs::rename(&part, dest)
        .await
        .map_err(|e| Error::io(dest, e))?;
    guard.keep();
    Ok(dest.to_path_buf())
}

/// Stream a ZIP's single DEFLATE member and inflate it directly to `dest`.
///
/// The whole archive is streamed once: every byte feeds the hasher (so the STAC
/// checksum, which covers the *archive*, can be verified), while only the member's
/// payload range feeds the decompressor.
pub async fn download_zip_member_inflated(
    http: &dyn Http,
    url: &str,
    dest: &Path,
    expected: Option<&Digest>,
    cancel: &Cancel,
    progress: ProgressFn<'_>,
) -> Result<(PathBuf, zip::Member)> {
    let head = http.head(url).await?;
    let total = head
        .len
        .ok_or_else(|| Error::Zip("server did not report a content length".into()))?;

    let member = zip::first_member(http, url, total).await?;
    if member.is_stored() {
        // Nothing to inflate; a plain ranged download of the payload would do.
        return Err(Error::Zip(format!(
            "member {} is STORED, not DEFLATE; use a ranged copy instead",
            member.name
        )));
    }
    if !member.is_deflate() {
        return Err(Error::Zip(format!(
            "unsupported compression method {} for {}",
            member.method, member.name
        )));
    }

    let (part, mut file) = create_part(dest).await?;
    let guard = PartGuard::new(part.clone());
    let mut hasher = Sha256::new();
    let mut inflater = flate2::Decompress::new(false); // raw DEFLATE, no zlib header
    let mut out = vec![0u8; CHUNK_HINT];
    let mut pos: u64 = 0;
    let mut written: u64 = 0;
    let mut retries = 0u32;

    'outer: loop {
        cancel.check()?;
        let mut stream = match http.get_stream(url, pos).await {
            Ok(s) => s,
            Err(e) => {
                retries += 1;
                if retries > MAX_RETRIES {
                    return Err(e);
                }
                tokio::time::sleep(backoff(retries)).await;
                continue;
            }
        };
        while let Some(chunk) = stream.next().await {
            match chunk {
                Ok(bytes) => {
                    cancel.check()?;
                    hasher.update(&bytes);

                    // Route only the member payload into the decompressor.
                    let lo = pos;
                    let hi = pos + bytes.len() as u64;
                    let s = lo.max(member.data_start);
                    let e = hi.min(member.data_end());
                    if s < e {
                        let mut slice = &bytes[(s - lo) as usize..(e - lo) as usize];
                        while !slice.is_empty() {
                            let before_in = inflater.total_in();
                            let before_out = inflater.total_out();
                            let status = inflater
                                .decompress(slice, &mut out, flate2::FlushDecompress::None)
                                .map_err(|err| Error::Inflate(err.to_string()))?;
                            let consumed = (inflater.total_in() - before_in) as usize;
                            let produced = (inflater.total_out() - before_out) as usize;
                            if produced > 0 {
                                file.write_all(&out[..produced])
                                    .await
                                    .map_err(|er| Error::io(&part, er))?;
                                written += produced as u64;
                            }
                            slice = &slice[consumed..];
                            if status == flate2::Status::StreamEnd {
                                break;
                            }
                            if consumed == 0 && produced == 0 {
                                break;
                            }
                        }
                    }
                    pos = hi;
                    progress(Progress {
                        read: pos,
                        total: Some(total),
                        written,
                        retries,
                    });
                }
                Err(_) => {
                    // Reconnect and keep the SAME hasher and decompressor: this is what
                    // makes a dropped connection cheap instead of fatal.
                    retries += 1;
                    if retries > MAX_RETRIES {
                        return Err(Error::NoRangeSupport { url: url.into() });
                    }
                    tokio::time::sleep(backoff(retries)).await;
                    continue 'outer;
                }
            }
        }
        break;
    }

    file.flush().await.map_err(|e| Error::io(&part, e))?;
    drop(file);

    if pos != total {
        return Err(Error::SizeMismatch {
            expected: total,
            actual: pos,
        });
    }
    if written != member.uncompressed_size {
        return Err(Error::SizeMismatch {
            expected: member.uncompressed_size,
            actual: written,
        });
    }
    if let Some(d) = expected {
        d.verify(hasher)?;
    }

    tokio::fs::rename(&part, dest)
        .await
        .map_err(|e| Error::io(dest, e))?;
    guard.keep();
    Ok((dest.to_path_buf(), member))
}
