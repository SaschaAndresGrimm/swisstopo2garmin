//! Write a STORE-only ZIP archive.
//!
//! This exists for one reason: a Garmin Custom Map is a KMZ, and a KMZ is a ZIP holding
//! `doc.kml` plus JPEG tiles (see [`crate::raster`]). [`crate::zip`] is the read side —
//! it locates a member in a *remote* archive with range requests — and has no writer.
//!
//! STORE (no compression) is the right choice rather than a shortcut: JPEG payloads are
//! already entropy-coded, so deflating them costs CPU to save fractions of a percent,
//! and `doc.kml` is a few kilobytes. That also keeps this module small enough to be
//! obviously correct, which matters because a malformed KMZ fails silently on a device
//! that offers no way to see why.
//!
//! Deliberately not supported: ZIP64 (a Custom Map is capped at ~300 MB by the tile
//! limits, three orders of magnitude below the 4 GB point), directory entries, and
//! anything about permissions or timestamps that a device does not read.

use std::io::{Seek, Write};

use crate::error::{Error, Result};

const LFH_SIG: u32 = 0x0403_4b50;
const CDH_SIG: u32 = 0x0201_4b50;
const EOCD_SIG: u32 = 0x0605_4b50;
const METHOD_STORED: u16 = 0;
/// 2.0 — the floor readers accept; nothing here needs a later feature.
const VERSION_NEEDED: u16 = 20;

/// The largest archive this writer will produce, being the point at which ZIP64 becomes
/// mandatory. Hitting it is a bug in the caller, not a limit to raise: see the module
/// docs for why a KMZ cannot legitimately approach it.
const MAX_ARCHIVE_BYTES: u64 = u32::MAX as u64;

struct Entry {
    name: String,
    crc: u32,
    size: u32,
    offset: u32,
}

/// Streaming STORE-only ZIP writer.
///
/// Members are written as they are added, so an archive is never held in memory
/// (SPEC.md NFR-2) — only one central-directory record per member, ~50 bytes.
pub struct ZipWriter<W: Write + Seek> {
    out: W,
    entries: Vec<Entry>,
    finished: bool,
}

impl<W: Write + Seek> ZipWriter<W> {
    pub fn new(out: W) -> Self {
        Self {
            out,
            entries: Vec::new(),
            finished: false,
        }
    }

    /// Append one stored member.
    ///
    /// The name is used verbatim as the archive path and must be ASCII: the general
    /// purpose bit for UTF-8 names is not set, so a non-ASCII name would be read back
    /// under whatever legacy code page the reader guesses. KMZ member names are ours to
    /// choose, so restricting them is free.
    pub fn add(&mut self, name: &str, data: &[u8]) -> Result<()> {
        if self.finished {
            return Err(Error::Zip("zip archive is already finished".into()));
        }
        if name.is_empty() || !name.is_ascii() || name.len() > u16::MAX as usize {
            return Err(Error::Zip(format!(
                "zip member name must be non-empty ASCII: {name:?}"
            )));
        }
        if self.entries.iter().any(|e| e.name == name) {
            return Err(Error::Zip(format!("duplicate zip member {name:?}")));
        }
        let offset = self.position()?;
        let size = u32::try_from(data.len())
            .map_err(|_| Error::Zip(format!("zip member {name} exceeds 4 GB")))?;
        let crc = crc32(data);

        // Local file header. Sizes are known up front, so no data descriptor is needed
        // and the general purpose flags stay zero.
        let mut h = Vec::with_capacity(30 + name.len());
        h.extend_from_slice(&LFH_SIG.to_le_bytes());
        h.extend_from_slice(&VERSION_NEEDED.to_le_bytes());
        h.extend_from_slice(&0u16.to_le_bytes()); // flags
        h.extend_from_slice(&METHOD_STORED.to_le_bytes());
        h.extend_from_slice(&0u16.to_le_bytes()); // time
        h.extend_from_slice(&0u16.to_le_bytes()); // date
        h.extend_from_slice(&crc.to_le_bytes());
        h.extend_from_slice(&size.to_le_bytes()); // compressed == uncompressed
        h.extend_from_slice(&size.to_le_bytes());
        h.extend_from_slice(&(name.len() as u16).to_le_bytes());
        h.extend_from_slice(&0u16.to_le_bytes()); // extra length
        h.extend_from_slice(name.as_bytes());
        self.write(&h)?;
        self.write(data)?;

        self.entries.push(Entry {
            name: name.to_string(),
            crc,
            size,
            offset,
        });
        Ok(())
    }

    /// Write the central directory and end record.
    pub fn finish(mut self) -> Result<W> {
        let cd_offset = self.position()?;
        let mut cd = Vec::new();
        for e in &self.entries {
            cd.extend_from_slice(&CDH_SIG.to_le_bytes());
            cd.extend_from_slice(&VERSION_NEEDED.to_le_bytes()); // version made by
            cd.extend_from_slice(&VERSION_NEEDED.to_le_bytes());
            cd.extend_from_slice(&0u16.to_le_bytes()); // flags
            cd.extend_from_slice(&METHOD_STORED.to_le_bytes());
            cd.extend_from_slice(&0u16.to_le_bytes()); // time
            cd.extend_from_slice(&0u16.to_le_bytes()); // date
            cd.extend_from_slice(&e.crc.to_le_bytes());
            cd.extend_from_slice(&e.size.to_le_bytes());
            cd.extend_from_slice(&e.size.to_le_bytes());
            cd.extend_from_slice(&(e.name.len() as u16).to_le_bytes());
            cd.extend_from_slice(&0u16.to_le_bytes()); // extra
            cd.extend_from_slice(&0u16.to_le_bytes()); // comment
            cd.extend_from_slice(&0u16.to_le_bytes()); // disk number
            cd.extend_from_slice(&0u16.to_le_bytes()); // internal attrs
            cd.extend_from_slice(&0u32.to_le_bytes()); // external attrs
            cd.extend_from_slice(&e.offset.to_le_bytes());
            cd.extend_from_slice(e.name.as_bytes());
        }
        let cd_len = u32::try_from(cd.len())
            .map_err(|_| Error::Zip("zip central directory exceeds 4 GB".into()))?;
        let count = u16::try_from(self.entries.len())
            .map_err(|_| Error::Zip("more than 65535 zip members".into()))?;

        cd.extend_from_slice(&EOCD_SIG.to_le_bytes());
        cd.extend_from_slice(&0u16.to_le_bytes()); // this disk
        cd.extend_from_slice(&0u16.to_le_bytes()); // disk with cd
        cd.extend_from_slice(&count.to_le_bytes());
        cd.extend_from_slice(&count.to_le_bytes());
        cd.extend_from_slice(&cd_len.to_le_bytes());
        cd.extend_from_slice(&cd_offset.to_le_bytes());
        cd.extend_from_slice(&0u16.to_le_bytes()); // comment length
        self.write(&cd)?;

        self.finished = true;
        self.out.flush().map_err(Self::io)?;
        Ok(self.out)
    }

    fn position(&mut self) -> Result<u32> {
        let at = self.out.stream_position().map_err(Self::io)?;
        u32::try_from(at)
            .map_err(|_| Error::Zip(format!("zip archive exceeds {MAX_ARCHIVE_BYTES} bytes")))
    }

    fn write(&mut self, bytes: &[u8]) -> Result<()> {
        self.out.write_all(bytes).map_err(Self::io)
    }

    fn io(e: std::io::Error) -> Error {
        Error::Io {
            path: "<zip>".into(),
            source: e,
        }
    }
}

/// CRC-32 (IEEE), as ZIP requires.
///
/// `flate2` is already a dependency and exposes exactly this, so it is used rather than
/// adding a crc crate or hand-rolling a table (working agreement rule 7).
fn crc32(data: &[u8]) -> u32 {
    let mut c = flate2::Crc::new();
    c.update(data);
    c.sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    // The reader's little-endian helpers are private to that module; the test needs its
    // own so that a change there cannot quietly change what this asserts.
    fn u16le(b: &[u8], off: usize) -> u16 {
        u16::from_le_bytes([b[off], b[off + 1]])
    }
    fn u32le(b: &[u8], off: usize) -> u32 {
        u32::from_le_bytes([b[off], b[off + 1], b[off + 2], b[off + 3]])
    }

    fn archive(members: &[(&str, &[u8])]) -> Vec<u8> {
        let mut z = ZipWriter::new(Cursor::new(Vec::new()));
        for (n, d) in members {
            z.add(n, d).expect("add");
        }
        z.finish().expect("finish").into_inner()
    }

    #[test]
    fn an_empty_archive_is_just_an_end_record() {
        let bytes = archive(&[]);
        assert_eq!(bytes.len(), 22, "EOCD with no comment is 22 bytes");
        assert_eq!(&bytes[0..4], b"PK\x05\x06");
    }

    #[test]
    fn a_member_is_stored_uncompressed_and_its_payload_is_verbatim() {
        let data = b"the quick brown fox".repeat(10);
        let bytes = archive(&[("doc.kml", &data)]);
        // Stored means the payload appears in the archive byte for byte, which is the
        // property a device's JPEG decoder depends on.
        let at = bytes
            .windows(data.len())
            .position(|w| w == data.as_slice())
            .expect("payload present verbatim");
        assert_eq!(at, 30 + "doc.kml".len(), "payload follows the local header");
        assert_eq!(u16le(&bytes, 8), METHOD_STORED);
        assert_eq!(u32le(&bytes, 18), data.len() as u32, "compressed size");
        assert_eq!(u32le(&bytes, 22), data.len() as u32, "uncompressed size");
    }

    #[test]
    fn the_crc_is_the_standard_ieee_one() {
        // The check value every CRC-32 implementation is tested against.
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
        assert_eq!(crc32(b""), 0);
    }

    #[test]
    fn the_local_header_crc_matches_the_data() {
        let bytes = archive(&[("a.jpg", b"\xff\xd8\xff\xe0payload")]);
        assert_eq!(u32le(&bytes, 14), crc32(b"\xff\xd8\xff\xe0payload"));
    }

    #[test]
    fn the_end_record_counts_and_locates_the_central_directory() {
        let bytes = archive(&[("a", b"1"), ("b", b"22"), ("c", b"333")]);
        let eocd = bytes.len() - 22;
        assert_eq!(&bytes[eocd..eocd + 4], b"PK\x05\x06");
        assert_eq!(u16le(&bytes, eocd + 8), 3, "entries on this disk");
        assert_eq!(u16le(&bytes, eocd + 10), 3, "entries total");
        let cd_len = u32le(&bytes, eocd + 12) as usize;
        let cd_at = u32le(&bytes, eocd + 16) as usize;
        assert_eq!(
            cd_at + cd_len,
            eocd,
            "the directory ends where the record begins"
        );
        assert_eq!(&bytes[cd_at..cd_at + 4], b"PK\x01\x02");
    }

    /// The point of the writer is that the project's own reader can read it, so that is
    /// the test: `zip::first_member` walks the same structures a device would.
    #[tokio::test]
    async fn the_projects_own_reader_finds_the_first_member() {
        let bytes = archive(&[("doc.kml", b"<kml/>"), ("tiles/000_000.jpg", b"jpegbytes")]);
        let http = crate::testing::FakeHttp::new().with_body("zip://k.kmz", bytes.clone());
        let m = crate::zip::first_member(&http, "zip://k.kmz", bytes.len() as u64)
            .await
            .expect("read back");
        assert_eq!(m.name, "doc.kml");
        assert!(m.is_stored(), "KMZ members are stored, not deflated");
        assert_eq!(m.uncompressed_size, 6);
        assert_eq!(
            &bytes[m.data_start as usize..m.data_end() as usize],
            b"<kml/>"
        );
    }

    #[test]
    fn a_duplicate_name_is_refused() {
        let mut z = ZipWriter::new(Cursor::new(Vec::new()));
        z.add("a.jpg", b"1").unwrap();
        let e = z.add("a.jpg", b"2").expect_err("duplicate");
        assert!(e.to_string().contains("duplicate"), "{e}");
    }

    #[test]
    fn a_non_ascii_name_is_refused_because_the_utf8_flag_is_not_set() {
        let mut z = ZipWriter::new(Cursor::new(Vec::new()));
        let e = z.add("Zürich.jpg", b"1").expect_err("non-ascii");
        assert!(e.to_string().contains("ASCII"), "{e}");
        assert!(z.add("", b"1").is_err(), "an empty name is refused too");
    }

    #[test]
    fn offsets_are_absolute_and_grow_with_each_member() {
        let bytes = archive(&[("a", &[0u8; 100]), ("b", &[0u8; 200])]);
        let eocd = bytes.len() - 22;
        let cd_at = u32le(&bytes, eocd + 16) as usize;
        // Two central directory headers, 46 bytes plus the 1-byte name each.
        assert_eq!(u32le(&bytes, cd_at + 42), 0, "first member at the start");
        assert_eq!(
            u32le(&bytes, cd_at + 47 + 42),
            (30 + 1 + 100) as u32,
            "second member after the first header and payload"
        );
    }
}
