//! Garmin IMG container verification (SPEC.md §7.7).
//!
//! Two of the four "map lists on the device but draws nothing" defects in Milestone 0
//! were visible here and nowhere else: a gmapsupp missing its overview map, and a map
//! whose subfiles were empty. The third — a TYP without a `[_drawOrder]` section — is
//! deliberately **not** detectable from the IMG, because the polygons are present in
//! the RGN either way; that one is guarded by `spikes/s0/checkstyle.py`.
//!
//! Header offsets are measured from a real mkgmap output, not taken from the several
//! online references that disagree: `DSKIMG` at 0x10, `GARMIN` at 0x41, description at
//! 0x49, block-size exponents at 0x61/0x62, FAT at 0x600.

use std::collections::BTreeMap;
use std::path::Path;

use crate::error::{Error, Result};

const FAT_START: usize = 0x600;
const FAT_ENTRY: usize = 512;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubFile {
    /// Map number, e.g. `63260001`, or an internal name such as `MAKEGMAP`.
    pub name: String,
    /// Subfile kind: TRE, RGN, LBL, NET, NOD, DEM, TYP, MDR, SRT, MPS.
    pub kind: String,
    pub bytes: u32,
}

#[derive(Debug, Clone)]
pub struct ImgInfo {
    pub file_bytes: u64,
    pub block_size: u32,
    pub description: String,
    pub subfiles: Vec<SubFile>,
}

impl ImgInfo {
    /// Distinct map numbers present, excluding internal entries.
    pub fn maps(&self) -> Vec<String> {
        let mut v: Vec<String> = self
            .subfiles
            .iter()
            .filter(|s| s.name.chars().all(|c| c.is_ascii_digit()))
            .map(|s| s.name.clone())
            .collect();
        v.sort();
        v.dedup();
        v
    }

    pub fn total_subfile_bytes(&self) -> u64 {
        self.subfiles.iter().map(|s| s.bytes as u64).sum()
    }

    pub fn bytes_of_kind(&self, kind: &str) -> u64 {
        self.subfiles
            .iter()
            .filter(|s| s.kind == kind)
            .map(|s| s.bytes as u64)
            .sum()
    }

    pub fn has_kind(&self, kind: &str) -> bool {
        self.subfiles.iter().any(|s| s.kind == kind)
    }

    /// True when the map carries elevation data for device-side relief shading.
    pub fn has_dem(&self) -> bool {
        self.has_kind("DEM")
    }

    pub fn kind_counts(&self) -> BTreeMap<String, usize> {
        let mut m = BTreeMap::new();
        for s in &self.subfiles {
            *m.entry(s.kind.clone()).or_insert(0) += 1;
        }
        m
    }
}

/// Parse the IMG container.
pub fn read(path: &Path) -> Result<ImgInfo> {
    let raw = std::fs::read(path).map_err(|e| Error::io(path, e))?;
    if raw.len() < FAT_START + FAT_ENTRY {
        return Err(Error::Zip(format!(
            "{} is too small to be a Garmin IMG ({} bytes)",
            path.display(),
            raw.len()
        )));
    }

    // The whole file may be XOR-obfuscated with the value of byte 0.
    let xor = raw[0];
    let data: Vec<u8> = if xor == 0 {
        raw
    } else {
        raw.iter().map(|b| b ^ xor).collect()
    };

    if &data[0x10..0x16] != b"DSKIMG" {
        return Err(Error::Zip(format!(
            "{} is not a Garmin IMG (no DSKIMG signature)",
            path.display()
        )));
    }
    let block_size = 1u32 << (data[0x61] as u32 + data[0x62] as u32);
    let description = String::from_utf8_lossy(&data[0x49..0x5D])
        .trim()
        .to_string();

    let mut subfiles = Vec::new();
    let mut pos = FAT_START;
    while pos + FAT_ENTRY <= data.len() {
        let e = &data[pos..pos + FAT_ENTRY];
        pos += FAT_ENTRY;
        if e[0] != 0x01 {
            continue;
        }
        // The FAT is followed directly by data blocks. Without this guard the scan
        // runs on and invents subfiles out of map geometry.
        if !e[1..12].iter().all(|c| (0x20..0x7f).contains(c)) {
            break;
        }
        let part = u16::from_le_bytes([e[16], e[17]]);
        if part != 0 {
            continue; // continuation entry for a subfile already recorded
        }
        subfiles.push(SubFile {
            name: String::from_utf8_lossy(&e[1..9]).trim().to_string(),
            kind: String::from_utf8_lossy(&e[9..12]).trim().to_string(),
            bytes: u32::from_le_bytes([e[12], e[13], e[14], e[15]]),
        });
    }

    Ok(ImgInfo {
        file_bytes: std::fs::metadata(path).map(|m| m.len()).unwrap_or(0),
        block_size,
        description,
        subfiles,
    })
}

/// What a verification found. Problems are fatal; warnings are worth surfacing.
#[derive(Debug, Clone, Default)]
pub struct Verdict {
    pub problems: Vec<String>,
    pub warnings: Vec<String>,
}

impl Verdict {
    pub fn ok(&self) -> bool {
        self.problems.is_empty()
    }
}

/// Check a `gmapsupp.img` against the failures Milestone 0 actually hit.
pub fn verify_gmapsupp(info: &ImgInfo, expect_dem: bool) -> Verdict {
    let mut v = Verdict::default();

    for kind in ["TRE", "RGN", "LBL"] {
        if !info.has_kind(kind) {
            v.problems.push(format!(
                "no {kind} subfile: the map has no {}",
                if kind == "RGN" { "geometry" } else { "content" }
            ));
        }
    }
    if info.bytes_of_kind("RGN") == 0 {
        v.problems
            .push("the RGN subfiles are empty, so nothing will be drawn".into());
    }

    // A supplementary map needs its overview map, or the device lists it and draws
    // nothing (docs/m0-findings.md §4.5).
    let maps = info.maps();
    let has_overview = maps.iter().any(|m| m.ends_with("0000"));
    if !has_overview {
        v.problems.push(
            "no overview map (a map number ending 0000): a single-pass --gmapsupp build \
             produces this, and the device will list the map but draw nothing"
                .into(),
        );
    }
    if maps.len() < 2 {
        v.problems.push(format!(
            "expected an overview map plus at least one detail tile, found {maps:?}"
        ));
    }

    if !info.has_kind("TYP") {
        v.warnings
            .push("no TYP subfile: the map will use Garmin's default styling".into());
    }
    if expect_dem && !info.has_dem() {
        v.problems
            .push("--dem was requested but the map carries no DEM subfile".into());
    }
    if !expect_dem && !info.has_dem() {
        v.warnings
            .push("no DEM subfile, so the device cannot render shaded relief (FR-CART8)".into());
    }
    if !info.has_kind("MDR") {
        v.warnings
            .push("no searchable index (MDR): place search will not work".into());
    }

    v
}

/// FAT32 cannot hold a file of 4 GiB or more.
pub const MAX_IMG_BYTES: u64 = 4 * 1024 * 1024 * 1024 - 1;

/// Check the output against a device's limits.
pub fn verify_for_device(
    info: &ImgInfo,
    tile_count: usize,
    max_img_bytes: u64,
    max_tiles: usize,
) -> Verdict {
    let mut v = Verdict::default();
    if info.file_bytes > max_img_bytes {
        v.problems.push(format!(
            "{} bytes exceeds the device limit of {max_img_bytes}",
            info.file_bytes
        ));
    }
    if info.file_bytes > MAX_IMG_BYTES {
        v.problems
            .push("the file is 4 GiB or larger, which FAT32 cannot store".into());
    }
    if tile_count > max_tiles {
        v.problems.push(format!(
            "{tile_count} tiles exceeds the device limit of {max_tiles}; \
             raise --max-nodes or split into several map sets"
        ));
    }
    v
}
