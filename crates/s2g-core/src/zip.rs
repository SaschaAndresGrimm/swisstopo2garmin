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
