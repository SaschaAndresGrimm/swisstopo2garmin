//! SRTM `.hgt` generation for mkgmap `--dem` (SPEC.md FR-CART8).
//!
//! Shipping DEM data inside the map makes the device render shaded relief natively,
//! which is the largest single step toward the look of the swisstopo raster maps.
//! Confirmed working on an Edge 840.
//!
//! The spike used `gdalwarp` over the 10 GB swissALTIRegio COG. This instead resamples
//! the same [`Grid`] already built for contours, so a build needs no GDAL, no Python,
//! and no second elevation download.
//!
//! A `.hgt` cell always covers a whole 1x1 degree square, but a map covers far less.
//! Samples outside the available elevation data are written as the SRTM void value,
//! which mkgmap and Garmin both understand.

use std::path::{Path, PathBuf};

use crate::elevation::{Grid, NODATA, TILE_RES_M};
use crate::error::{Error, Result};
use crate::proj::{lv95_to_wgs84, wgs84_to_lv95, BBox};

/// SRTM void marker.
pub const VOID: i16 = -32768;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Resolution {
    /// 3601 x 3601 samples, about 30 m.
    ///
    /// Measured on hardware: at this detail the Edge 840 renders alpine relief dark
    /// enough to mute the palette (docs/m0-findings.md §4.14).
    ArcSecond1,
    /// 1201 x 1201 samples, about 90 m. Gentler shading.
    ArcSecond3,
}

impl Resolution {
    pub fn samples(&self) -> usize {
        match self {
            Resolution::ArcSecond1 => 3601,
            Resolution::ArcSecond3 => 1201,
        }
    }

    /// Byte size of one `.hgt` cell, which is fixed by the format.
    pub fn file_bytes(&self) -> usize {
        let n = self.samples();
        n * n * 2
    }

    /// The `--dem-dists` base mkgmap documents for this spacing.
    pub fn dem_dist_base(&self) -> u32 {
        match self {
            Resolution::ArcSecond1 => 3314,
            Resolution::ArcSecond3 => 9942,
        }
    }
}

/// A 1x1 degree cell, named the SRTM way: `N46E008`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct HgtCell {
    pub lat: i32,
    pub lon: i32,
}

impl HgtCell {
    pub fn name(&self) -> String {
        format!(
            "{}{:02}{}{:03}",
            if self.lat < 0 { 'S' } else { 'N' },
            self.lat.abs(),
            if self.lon < 0 { 'W' } else { 'E' },
            self.lon.abs()
        )
    }

    /// Cells covering an LV95 bbox.
    ///
    /// All four corners are projected: the LV95 grid is rotated relative to the
    /// graticule, so a two-corner conversion can miss a cell.
    pub fn covering(bbox: &BBox) -> Vec<HgtCell> {
        let [w, s, e, n] = bbox.to_wgs84();
        let mut out = Vec::new();
        for lat in s.floor() as i32..=n.floor() as i32 {
            for lon in w.floor() as i32..=e.floor() as i32 {
                out.push(HgtCell { lat, lon });
            }
        }
        out
    }
}

#[derive(Debug, Clone, Default)]
pub struct DemStats {
    pub cells: usize,
    pub samples_filled: u64,
    pub samples_void: u64,
    pub bytes: u64,
}

impl DemStats {
    pub fn coverage(&self) -> f64 {
        let total = self.samples_filled + self.samples_void;
        if total == 0 {
            0.0
        } else {
            self.samples_filled as f64 / total as f64
        }
    }
}

/// Write one `.hgt` per cell covering `bbox`, resampling `grid`.
pub fn write_hgt(
    grid: &Grid,
    bbox: &BBox,
    resolution: Resolution,
    out_dir: &Path,
    mut progress: impl FnMut(&HgtCell),
) -> Result<(Vec<PathBuf>, DemStats)> {
    std::fs::create_dir_all(out_dir).map_err(|e| Error::io(out_dir, e))?;
    let n = resolution.samples();
    let step = 1.0 / (n - 1) as f64;
    let mut stats = DemStats::default();
    let mut written = Vec::new();

    for cell in HgtCell::covering(bbox) {
        progress(&cell);
        let path = out_dir.join(format!("{}.hgt", cell.name()));
        // Row 0 is the northernmost, and samples run west to east.
        let mut bytes = Vec::with_capacity(resolution.file_bytes());

        for row in 0..n {
            let lat = (cell.lat + 1) as f64 - row as f64 * step;
            for col in 0..n {
                let lon = cell.lon as f64 + col as f64 * step;
                let (e, north) = wgs84_to_lv95(lon, lat);
                let v = match sample_bilinear(grid, e, north) {
                    Some(h) => {
                        stats.samples_filled += 1;
                        h.round().clamp(-1000.0, 9000.0) as i16
                    }
                    None => {
                        stats.samples_void += 1;
                        VOID
                    }
                };
                bytes.extend_from_slice(&v.to_be_bytes());
            }
        }

        std::fs::write(&path, &bytes).map_err(|e| Error::io(&path, e))?;
        stats.bytes += bytes.len() as u64;
        stats.cells += 1;
        written.push(path);
    }

    if written.is_empty() {
        return Err(Error::NotFound(
            "no .hgt cells cover the requested area".into(),
        ));
    }
    Ok((written, stats))
}

/// Bilinear sample of the grid at an LV95 position, or `None` outside valid data.
///
/// Any of the four neighbours being void makes the result void: interpolating against
/// -9999 would drag a cliff into the terrain.
fn sample_bilinear(grid: &Grid, e: f64, n: f64) -> Option<f64> {
    let fx = (e - grid.origin_e) / TILE_RES_M - 0.5;
    let fy = (n - grid.origin_n) / TILE_RES_M - 0.5;
    if fx < 0.0 || fy < 0.0 {
        return None;
    }
    let (x0, y0) = (fx.floor() as usize, fy.floor() as usize);
    if x0 + 1 >= grid.cols || y0 + 1 >= grid.rows {
        return None;
    }
    let (tx, ty) = (fx - x0 as f64, fy - y0 as f64);

    let q = [
        grid.at(x0, y0),
        grid.at(x0 + 1, y0),
        grid.at(x0, y0 + 1),
        grid.at(x0 + 1, y0 + 1),
    ];
    if q.iter().any(|v| *v == NODATA || !v.is_finite()) {
        return None;
    }
    let a = q[0] as f64 * (1.0 - tx) + q[1] as f64 * tx;
    let b = q[2] as f64 * (1.0 - tx) + q[3] as f64 * tx;
    Some(a * (1.0 - ty) + b * ty)
}

/// `--dem-dists` for a style's level count at a given resolution.
///
/// mkgmap requires exactly one value per level and aborts otherwise; each level
/// halves the previous resolution.
pub fn dem_dists(resolution: Resolution, levels: usize) -> Vec<u32> {
    let base = resolution.dem_dist_base();
    (0..levels).map(|i| base << i).collect()
}

/// Round-trip check used by the tests: LV95 -> WGS84 -> LV95 must land back.
pub fn projection_round_trip_error(e: f64, n: f64) -> f64 {
    let (lon, lat) = lv95_to_wgs84(e, n);
    let (e2, n2) = wgs84_to_lv95(lon, lat);
    (e2 - e).hypot(n2 - n)
}
