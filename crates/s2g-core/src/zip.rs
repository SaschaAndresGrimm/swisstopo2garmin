//! Locate a member inside a remote ZIP using only range requests.
//!
//! swissTLM3D ships as a ZIP64 archive holding one DEFLATE-compressed GeoPackage:
//! 4.80 GB compressed, 10.78 GB inflated. Because the member is deflated there is no
//! random access into it, so acquisition must inflate while downloading — otherwise
//! peak disk is 15.6 GB instead of 10.78 GB (docs/m0-findings.md §1.1-1.2).
//!
//! This module reads the end-of-central-directory and the member's local header so the
//! downloader knows which byte range carries the compressed payload.

use crate::error::{Error, Result};
use crate::http::Http;

const EOCD_SIG: &[u8] = b"PK\x05\x06";
const EOCD64_LOCATOR_SIG: &[u8] = b"PK\x06\x07";
const EOCD64_SIG: &[u8] = b"PK\x06\x06";
const CDH_SIG: &[u8] = b"PK\x01\x02";
const LFH_SIG: &[u8] = b"PK\x03\x04";

const METHOD_STORED: u16 = 0;
const METHOD_DEFLATE: u16 = 8;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Member {
    pub name: String,
    /// Absolute offset of the compressed payload in the archive.
    pub data_start: u64,
    pub compressed_size: u64,
    pub uncompressed_size: u64,
    pub method: u16,
}

impl Member {
    pub fn is_deflate(&self) -> bool {
        self.method == METHOD_DEFLATE
    }
    pub fn is_stored(&self) -> bool {
        self.method == METHOD_STORED
    }
    pub fn data_end(&self) -> u64 {
        self.data_start + self.compressed_size
    }
}

fn u16le(b: &[u8], off: usize) -> u16 {
    u16::from_le_bytes([b[off], b[off + 1]])
}
fn u32le(b: &[u8], off: usize) -> u32 {
    u32::from_le_bytes([b[off], b[off + 1], b[off + 2], b[off + 3]])
}
fn u64le(b: &[u8], off: usize) -> u64 {
    u64::from_le_bytes([
        b[off],
        b[off + 1],
        b[off + 2],
        b[off + 3],
        b[off + 4],
        b[off + 5],
        b[off + 6],
        b[off + 7],
    ])
}

fn rfind(hay: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.len() > hay.len() {
        return None;
    }
    (0..=hay.len() - needle.len())
        .rev()
        .find(|&i| &hay[i..i + needle.len()] == needle)
}

/// Extract every member of a local archive into `dest`.
///
/// The remote path below is for one huge DEFLATE member that must be inflated while
/// downloading. The ASTRA route datasets are the opposite case: tens of megabytes
/// holding a dozen shapefile components, where downloading the whole archive first and
/// extracting locally is both simpler and correct.
///
/// Sizes are taken from the **central directory**, never from local file headers. Real
/// archives — the ASTRA ones included — are written as a stream, which leaves the local
/// header's sizes zero and puts the real ones in a trailing data descriptor. Walking
/// local headers works on archives this project writes and fails on the ones it has to
/// read.
///
/// Member paths are flattened to their file name, so a crafted archive cannot write
/// outside `dest`.
pub fn extract_all(
    archive: &std::path::Path,
    dest: &std::path::Path,
) -> Result<Vec<std::path::PathBuf>> {
    use std::io::Read;

    let bytes = std::fs::read(archive).map_err(|e| Error::io(archive, e))?;
    if bytes.len() < 22 {
        return Err(Error::Zip(
            "archive is too small to hold a zip directory".into(),
        ));
    }
    std::fs::create_dir_all(dest).map_err(|e| Error::io(dest, e))?;

    let (cd_offset, entries) = central_directory(&bytes)?;

    let mut out = Vec::new();
    let mut p = cd_offset;
    for _ in 0..entries {
        if p + 46 > bytes.len() || &bytes[p..p + 4] != CDH_SIG {
            return Err(Error::Zip("malformed central directory entry".into()));
        }
        let method = u16le(&bytes, p + 10);
        let mut comp = u32le(&bytes, p + 20) as u64;
        let mut uncomp = u32le(&bytes, p + 24) as u64;
        let name_len = u16le(&bytes, p + 28) as usize;
        let extra_len = u16le(&bytes, p + 30) as usize;
        let comment_len = u16le(&bytes, p + 32) as usize;
        let mut local_offset = u32le(&bytes, p + 42) as u64;
        let name_at = p + 46;
        if name_at + name_len + extra_len + comment_len > bytes.len() {
            return Err(Error::Zip("truncated central directory entry".into()));
        }
        let name = String::from_utf8_lossy(&bytes[name_at..name_at + name_len]).to_string();

        // ZIP64 extended information replaces whichever 32-bit fields are saturated,
        // in a fixed order and only for those that are.
        if comp == u32::MAX as u64 || uncomp == u32::MAX as u64 || local_offset == u32::MAX as u64 {
            let extra = &bytes[name_at + name_len..name_at + name_len + extra_len];
            let mut k = 0usize;
            while k + 4 <= extra.len() {
                let tag = u16le(extra, k);
                let size = u16le(extra, k + 2) as usize;
                if tag == 0x0001 {
                    let mut o = k + 4;
                    if uncomp == u32::MAX as u64 && o + 8 <= extra.len() {
                        uncomp = u64le(extra, o);
                        o += 8;
                    }
                    if comp == u32::MAX as u64 && o + 8 <= extra.len() {
                        comp = u64le(extra, o);
                        o += 8;
                    }
                    if local_offset == u32::MAX as u64 && o + 8 <= extra.len() {
                        local_offset = u64le(extra, o);
                    }
                    break;
                }
                k += 4 + size;
            }
        }
        p = name_at + name_len + extra_len + comment_len;

        if name.ends_with('/') {
            continue;
        }
        let file_name = name.rsplit(['/', '\\']).next().unwrap_or(&name).to_string();
        if file_name.is_empty() || file_name == "." || file_name == ".." {
            continue;
        }

        // The local header's name and extra lengths can differ from the central
        // directory's, so the payload offset must be read from the local header.
        let lo = local_offset as usize;
        if lo + 30 > bytes.len() || &bytes[lo..lo + 4] != LFH_SIG {
            return Err(Error::Zip(format!(
                "member {name} has no local header at offset {local_offset}"
            )));
        }
        let data_start = lo + 30 + u16le(&bytes, lo + 26) as usize + u16le(&bytes, lo + 28) as usize;
        let data_end = data_start + comp as usize;
        if data_end > bytes.len() {
            return Err(Error::Zip(format!(
                "member {name} runs past the end of the archive"
            )));
        }

        let payload = &bytes[data_start..data_end];
        let data = match method {
            METHOD_STORED => payload.to_vec(),
            METHOD_DEFLATE => {
                let mut d = flate2::read::DeflateDecoder::new(payload);
                let mut v = Vec::with_capacity(uncomp as usize);
                d.read_to_end(&mut v)
                    .map_err(|e| Error::Inflate(format!("{file_name}: {e}")))?;
                v
            }
            other => {
                return Err(Error::Zip(format!(
                    "member {name} uses compression method {other}, which is not supported"
                )))
            }
        };
        let path = dest.join(&file_name);
        std::fs::write(&path, &data).map_err(|e| Error::io(&path, e))?;
        out.push(path);
    }

    if out.is_empty() {
        return Err(Error::Zip("archive contains no files".into()));
    }
    Ok(out)
}

/// Offset and entry count of the central directory, following a ZIP64 record when one
/// is present.
fn central_directory(bytes: &[u8]) -> Result<(usize, usize)> {
    // The EOCD is at the end, after a comment of up to 64 KiB.
    let tail_from = bytes.len().saturating_sub(66_560);
    let eocd = rfind(&bytes[tail_from..], EOCD_SIG)
        .map(|i| i + tail_from)
        .ok_or_else(|| Error::Zip("no end-of-central-directory record; not a zip archive".into()))?;
    if eocd + 22 > bytes.len() {
        return Err(Error::Zip("truncated end-of-central-directory record".into()));
    }
    let mut entries = u16le(bytes, eocd + 10) as usize;
    let mut offset = u32le(bytes, eocd + 16) as usize;

    if entries == u16::MAX as usize || offset == u32::MAX as usize {
        let loc = rfind(&bytes[tail_from..eocd], EOCD64_LOCATOR_SIG)
            .map(|i| i + tail_from)
            .ok_or_else(|| Error::Zip("zip64 archive without an end-of-directory locator".into()))?;
        let eocd64 = u64le(bytes, loc + 8) as usize;
        if eocd64 + 56 > bytes.len() || &bytes[eocd64..eocd64 + 4] != EOCD64_SIG {
            return Err(Error::Zip("zip64 end-of-directory record is missing".into()));
        }
        entries = u64le(bytes, eocd64 + 32) as usize;
        offset = u64le(bytes, eocd64 + 48) as usize;
    }

    if offset > bytes.len() {
        return Err(Error::Zip("central directory offset is past the end of the archive".into()));
    }
    Ok((offset, entries))
}

/// Read the central directory of a single-member archive and resolve that member.
pub async fn first_member(http: &dyn Http, url: &str, total: u64) -> Result<Member> {
    if total < 22 {
        return Err(Error::Zip(format!(
            "archive too small to contain a zip directory ({total} bytes) -- \
             did the server report a content length?"
        )));
    }
    // The EOCD sits in the last 64 KiB + 22 bytes.
    let tail_len = (65_536u64 + 22).min(total);
    let tail = http.get_range(url, total - tail_len, total - 1).await?;

    let eocd = rfind(&tail, EOCD_SIG)
        .ok_or_else(|| Error::Zip("no end-of-central-directory record".into()))?;
    let mut cd_size = u32le(&tail, eocd + 12) as u64;
    let mut cd_off = u32le(&tail, eocd + 16) as u64;

    if cd_off == u32::MAX as u64 || cd_size == u32::MAX as u64 {
        let loc = rfind(&tail, EOCD64_LOCATOR_SIG)
            .ok_or_else(|| Error::Zip("zip64 locator missing".into()))?;
        let eocd64_off = u64le(&tail, loc + 8);
        let blk = http.get_range(url, eocd64_off, eocd64_off + 55).await?;
        if &blk[..4] != EOCD64_SIG {
            return Err(Error::Zip("bad zip64 end-of-central-directory".into()));
        }
        cd_size = u64le(&blk, 40);
        cd_off = u64le(&blk, 48);
    }

    let cd = http.get_range(url, cd_off, cd_off + cd_size - 1).await?;
    if cd.len() < 46 || &cd[..4] != CDH_SIG {
        return Err(Error::Zip("bad central directory header".into()));
    }

    let method = u16le(&cd, 10);
    let mut csize = u32le(&cd, 20) as u64;
    let mut usize_ = u32le(&cd, 24) as u64;
    let nlen = u16le(&cd, 28) as usize;
    let elen = u16le(&cd, 30) as usize;
    let mut lho = u32le(&cd, 42) as u64;
    let name = String::from_utf8_lossy(&cd[46..46 + nlen]).to_string();
    let extra = &cd[46 + nlen..46 + nlen + elen];

    // ZIP64 extended information overrides the 32-bit fields that are saturated.
    if usize_ == u32::MAX as u64 || csize == u32::MAX as u64 || lho == u32::MAX as u64 {
        let mut q = 0usize;
        while q + 4 <= extra.len() {
            let hid = u16le(extra, q);
            let hsz = u16le(extra, q + 2) as usize;
            if hid == 0x0001 {
                let blob = &extra[q + 4..(q + 4 + hsz).min(extra.len())];
                let mut o = 0usize;
                if usize_ == u32::MAX as u64 && o + 8 <= blob.len() {
                    usize_ = u64le(blob, o);
                    o += 8;
                }
                if csize == u32::MAX as u64 && o + 8 <= blob.len() {
                    csize = u64le(blob, o);
                    o += 8;
                }
                if lho == u32::MAX as u64 && o + 8 <= blob.len() {
                    lho = u64le(blob, o);
                }
                break;
            }
            q += 4 + hsz;
        }
    }

    // The local header repeats the name/extra lengths, and they may differ from the
    // central directory's, so the payload offset must come from the local header.
    let lfh = http.get_range(url, lho, lho + 29).await?;
    if &lfh[..4] != LFH_SIG {
        return Err(Error::Zip("bad local file header".into()));
    }
    let data_start = lho + 30 + u16le(&lfh, 26) as u64 + u16le(&lfh, 28) as u64;

    Ok(Member {
        name,
        data_start,
        compressed_size: csize,
        uncompressed_size: usize_,
        method,
    })
}
