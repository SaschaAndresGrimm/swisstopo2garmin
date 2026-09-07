//! Garmin Custom Maps: a raster overlay of the swisstopo paper map (SPEC.md §8.5, FR-R1..FR-R6).
//!
//! The vector map the rest of this crate builds is what a Garmin renders natively, and
//! it is the right thing for routing and for a watch screen. What it cannot do is *look
//! like the swisstopo paper map* — the rock hachures, the shaded relief plates, the
//! typography that Swiss hikers navigate by. That fidelity was raised as a requirement
//! after the milestone gate (PLAN.md), and a vector style cannot reach it.
//!
//! A Custom Map can, because it is the paper map: JPEG tiles cut from
//! `ch.swisstopo.pixelkarte-farbe` and georeferenced in a KMZ.
//!
//! # Why the grid is planned in degrees and not in LV95 (FR-R3)
//!
//! A KMZ `GroundOverlay` is georeferenced by a `LatLonBox` — four numbers, north, south,
//! east and west, describing an axis-aligned box in WGS84. LV95 is an oblique Mercator
//! projection, so a rectangle in LV95 is *not* a rectangle in lat/lon: its northern edge
//! is not a line of constant latitude. Planning the grid in LV95 and declaring each
//! tile's projected corners as a `LatLonBox` would therefore misplace the imagery, by
//! more at the edges of the country than the middle. So the grid is uniform in degrees,
//! and the WMS is asked for EPSG:4326 — then each tile's box is exact by construction.
//!
//! The consequence is that tiles are plate carrée, which is what the Custom Maps format
//! is; the device resamples into its own projection from the box, which is correct.
//! Pixels are allocated in proportion to each tile's *ground* extent so that resolution
//! is equal in both directions, which at Swiss latitudes means a tile is about 3 pixels
//! wide for every 2 tall.
//!
//! # Axis order
//!
//! WMS 1.3.0 takes `BBOX` in the axis order the CRS declares, which is the trap this
//! module exists downstream of: EPSG:4326 is latitude first, EPSG:2056 is easting first.
//! Both were checked against the live service rather than assumed — the wrong order does
//! not fail, it silently returns a blank tile from outside the data's extent.

use std::collections::BTreeMap;
use std::io::Write;
use std::path::Path;

use futures_util::StreamExt;

use crate::download::Cancel;
use crate::error::{Error, Result};
use crate::http::Http;
use crate::proj::{self, BBox};
use crate::zip_write::ZipWriter;

/// The swisstopo raster layer to cut tiles from. Same service the app's own map preview
/// uses (`frontend/src/map/wmts.ts`), so what the user picked an area on is what they get.
pub const DEFAULT_LAYER: &str = "ch.swisstopo.pixelkarte-farbe";

pub const WMS_ENDPOINT: &str = "https://wms.geo.admin.ch/";

/// Attribution carried inside the KMZ, not only in the app (FR-R6, FR-L1).
pub const ATTRIBUTION: &str = "© swisstopo";

/// Where a Custom Map goes on the device — a different directory from the `.img`, which
/// is why installing an overlay is a second copy and not a variation on the first.
///
/// Not invented: both shipped profiles' `GarminDevice.xml` advertise this directory,
/// which is how the project learned Edge devices have raster support at all
/// (`devices/edge-840.json`).
pub const CUSTOM_MAPS_DIR: &str = "Garmin/CustomMaps";

/// Filesystem-safe name for a map's KMZ.
///
/// The device lists a Custom Map by the `<name>` inside the KML, not by its filename,
/// so this only has to be safe on a device's FAT volume.
pub fn kmz_filename(map_name: &str) -> String {
    let slug: String = map_name
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    let slug = slug.trim_matches('-');
    if slug.is_empty() {
        "map.kmz".to_string()
    } else {
        format!("{slug}.kmz")
    }
}

/// Native resolution of `pixelkarte-farbe` at 1:25 000, the scale the layer is drawn for.
/// Asking for finer than the source only enlarges pixels.
pub const NATIVE_M_PER_PX: f64 = 1.25;

/// Mean JPEG bytes per pixel for `pixelkarte-farbe`, **measured** at 1.25 m/px over four
/// 9 km² areas chosen to span Swiss terrain (`examples/probe_raster.rs`):
///
/// | area | B/px |
/// |---|---|
/// | Plateau farmland | 0.201 |
/// | Grindelwald, alpine valley | 0.380 |
/// | Jungfrau, rock and ice | 0.417 |
/// | Basel, dense town | 0.528 |
///
/// The 2.6x spread is why [`RasterPlan::approx_bytes`] is labelled a rule of thumb. What
/// the numbers actually show is that size is *not* the constraint here: a full 100-tile
/// map is about 55 MB even at the dense-town rate, against a map budget measured in
/// gigabytes. The tile count is the binding limit, not the bytes.
pub const BYTES_PER_PIXEL: f64 = 0.40;

/// Above the densest area measured, for use as an upper bound.
pub const MAX_BYTES_PER_PIXEL: f64 = 0.55;

/// The per-tile byte size above which devices are reported to re-compress the image.
pub const DEFAULT_MAX_BYTES_PER_TILE: u64 = 3_000_000;

/// Below this, a raster overlay is worse than the vector map it would cover and the
/// caller is told so rather than handed a blurry map.
pub const USELESS_M_PER_PX: f64 = 20.0;

/// Metres per *degree* of latitude (not per radian), WGS84 mean. Constant to within 0.6 % over Switzerland's
/// 3° of latitude, which is far below the precision a tile grid needs.
const M_PER_DEG_LAT: f64 = 111_132.0;

// ---------------------------------------------------------------------------
// Device limits
// ---------------------------------------------------------------------------

/// What a device will accept, from its profile (`devices/*.json`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RasterLimits {
    /// Tiles across *all* Custom Maps on the device, not per file.
    pub max_tiles: usize,
    pub max_pixels_per_tile: u64,
    /// Devices degrade a tile heavier than this rather than reject it.
    pub max_bytes_per_tile: u64,
}

impl Default for RasterLimits {
    fn default() -> Self {
        Self {
            max_tiles: 100,
            max_pixels_per_tile: 1_048_576,
            max_bytes_per_tile: DEFAULT_MAX_BYTES_PER_TILE,
        }
    }
}

impl RasterLimits {
    /// Longest square tile edge within the pixel budget.
    pub fn tile_edge_px(&self) -> u32 {
        ((self.max_pixels_per_tile as f64).sqrt().floor() as u32).max(1)
    }
}

// ---------------------------------------------------------------------------
// Plan
// ---------------------------------------------------------------------------

/// One JPEG in the KMZ.
#[derive(Debug, Clone, PartialEq)]
pub struct RasterTile {
    pub row: u32,
    pub col: u32,
    /// Member name inside the KMZ.
    pub name: String,
    pub west: f64,
    pub south: f64,
    pub east: f64,
    pub north: f64,
    pub width: u32,
    pub height: u32,
}

impl RasterTile {
    pub fn pixels(&self) -> u64 {
        self.width as u64 * self.height as u64
    }

    /// WMS `GetMap` request for this tile. EPSG:4326 takes `BBOX` latitude first.
    pub fn wms_url(&self, layer: &str) -> String {
        format!(
            "{WMS_ENDPOINT}?SERVICE=WMS&VERSION=1.3.0&REQUEST=GetMap\
             &LAYERS={layer}&STYLES=&CRS=EPSG:4326\
             &BBOX={:.8},{:.8},{:.8},{:.8}&WIDTH={}&HEIGHT={}&FORMAT=image/jpeg",
            self.south, self.west, self.north, self.east, self.width, self.height
        )
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RasterPlan {
    pub layer: String,
    pub tiles: Vec<RasterTile>,
    /// Nominal grid shape. [`RasterPlan::tile_count`] is the number of tiles actually
    /// planned, which is one row or column smaller when the grid overshot the area.
    pub cols: u32,
    pub rows: u32,
    /// Ground resolution actually achieved, north-south.
    pub m_per_px: f64,
    /// The requested area, in degrees.
    pub west: f64,
    pub south: f64,
    pub east: f64,
    pub north: f64,
    /// Why the resolution is what it is, in words the UI can show.
    pub notes: Vec<String>,
}

impl RasterPlan {
    pub fn tile_count(&self) -> usize {
        self.tiles.len()
    }

    pub fn total_pixels(&self) -> u64 {
        self.tiles.iter().map(RasterTile::pixels).sum()
    }

    /// Typical KMZ size, from the mean byte rate.
    ///
    /// This is a rule of thumb, not an estimate with the standing of [`crate::estimate`]:
    /// the rate varies 2.6x with terrain (see [`BYTES_PER_PIXEL`]). Use
    /// [`RasterPlan::max_bytes`] for anything that has to hold.
    pub fn approx_bytes(&self) -> u64 {
        (self.total_pixels() as f64 * BYTES_PER_PIXEL) as u64
    }

    /// Upper bound on the KMZ size, for checking against a budget.
    pub fn max_bytes(&self) -> u64 {
        (self.total_pixels() as f64 * MAX_BYTES_PER_PIXEL) as u64
    }
}

/// Plan a tile grid covering `bbox` (LV95) at the best resolution the limits allow.
///
/// `wanted_m_per_px` is a ceiling on quality, not a promise: if the area is too large to
/// cover at that resolution within `max_tiles`, the resolution is coarsened until it
/// fits, and a note says so (FR-R5). That is the honest failure mode — the alternative, silently
/// covering part of the area, would hand the user a map with a hole in it.
pub fn plan(bbox: &BBox, limits: &RasterLimits, wanted_m_per_px: f64) -> Result<RasterPlan> {
    plan_layer(bbox, limits, wanted_m_per_px, DEFAULT_LAYER)
}

pub fn plan_layer(
    bbox: &BBox,
    limits: &RasterLimits,
    wanted_m_per_px: f64,
    layer: &str,
) -> Result<RasterPlan> {
    if limits.max_tiles == 0 {
        return Err(Error::NotFound(
            "this device profile allows no custom map tiles".into(),
        ));
    }
    if !(bbox.width() > 0.0 && bbox.height() > 0.0) {
        return Err(Error::NotFound("the selected area is empty".into()));
    }
    if !(wanted_m_per_px.is_finite() && wanted_m_per_px > 0.0) {
        return Err(Error::NotFound(format!(
            "resolution must be a positive number of metres per pixel, got {wanted_m_per_px}"
        )));
    }

    // The area in degrees. All four LV95 corners are projected and the extremes taken:
    // because LV95 is oblique Mercator the corners do not share latitudes, so taking
    // only two of them would clip a sliver off the area (see the module docs).
    let (west, south, east, north) = degrees_of(bbox);

    let mut notes = Vec::new();
    let mut m_per_px = wanted_m_per_px.max(NATIVE_M_PER_PX);
    if wanted_m_per_px < NATIVE_M_PER_PX {
        notes.push(format!(
            "The source map is drawn at {NATIVE_M_PER_PX} m per pixel, so a finer \
             request would only enlarge pixels; using {NATIVE_M_PER_PX} m."
        ));
    }

    // Ground extent, from which pixels are allocated so resolution is equal both ways.
    let ground_h_m = (north - south) * M_PER_DEG_LAT;
    let mid_lat = (north + south) / 2.0;
    let m_per_deg_lon = M_PER_DEG_LAT * mid_lat.to_radians().cos();
    let ground_w_m = (east - west) * m_per_deg_lon;

    let edge = limits.tile_edge_px();

    // Coarsen until the grid fits the tile budget. Each pass overshoots slightly because
    // of the two ceilings, so it is a loop rather than one division; it converges in a
    // handful of passes and the bound stops a pathological input spinning.
    let (mut cols, mut rows) = (0u32, 0u32);
    let mut fitted = false;
    for _ in 0..64 {
        let w_px = (ground_w_m / m_per_px).ceil().max(1.0);
        let h_px = (ground_h_m / m_per_px).ceil().max(1.0);
        cols = (w_px / edge as f64).ceil() as u32;
        rows = (h_px / edge as f64).ceil() as u32;
        let count = cols as u64 * rows as u64;
        if count <= limits.max_tiles as u64 {
            fitted = true;
            break;
        }
        // Scale so the *pixel* count fits, then let the next pass re-tile.
        m_per_px *= (count as f64 / limits.max_tiles as f64)
            .sqrt()
            .max(1.000_001);
    }
    if !fitted {
        return Err(Error::NotFound(format!(
            "could not fit this area into {} tiles",
            limits.max_tiles
        )));
    }
    if m_per_px > wanted_m_per_px.max(NATIVE_M_PER_PX) * 1.01 {
        notes.push(format!(
            "The area needs {:.1} m per pixel to fit in {} tiles, coarser than the {:.1} m \
             requested. A smaller area would be sharper.",
            m_per_px, limits.max_tiles, wanted_m_per_px
        ));
    }
    if m_per_px > USELESS_M_PER_PX {
        notes.push(format!(
            "At {m_per_px:.0} m per pixel the raster carries less detail than the vector \
             map underneath it. Consider a smaller area, or no raster overlay."
        ));
    }

    // Tiles are cut in degrees, uniform except for the last row and column, which are
    // trimmed to the area's edge so the overlay covers the selection and no more.
    let d_lat = edge as f64 * m_per_px / M_PER_DEG_LAT;
    let d_lon = edge as f64 * m_per_px / m_per_deg_lon;

    let mut tiles = Vec::with_capacity(cols as usize * rows as usize);
    for row in 0..rows {
        // Row 0 is the northern edge, so that reading order matches the map.
        let t_north = north - row as f64 * d_lat;
        let t_south = (t_north - d_lat).max(south);
        for col in 0..cols {
            let t_west = west + col as f64 * d_lon;
            let t_east = (t_west + d_lon).min(east);
            let w_m = (t_east - t_west) * m_per_deg_lon;
            let h_m = (t_north - t_south) * M_PER_DEG_LAT;
            // Both counts are ceilings, so the grid can overshoot the area by up to one
            // pixel and leave a final row or column with nothing in it. Dropping it is
            // right: a zero-width overlay is not imagery, and asking the WMS for it
            // would spend a tile of the device's budget on a sliver.
            if w_m < m_per_px || h_m < m_per_px {
                continue;
            }
            let width = ((w_m / m_per_px).round() as u32).clamp(1, edge);
            let height = ((h_m / m_per_px).round() as u32).clamp(1, edge);
            tiles.push(RasterTile {
                row,
                col,
                name: tile_name(row, col),
                west: t_west,
                south: t_south,
                east: t_east,
                north: t_north,
                width,
                height,
            });
        }
    }

    Ok(RasterPlan {
        layer: layer.to_string(),
        tiles,
        cols,
        rows,
        m_per_px,
        west,
        south,
        east,
        north,
        notes,
    })
}

/// Bounding box in degrees of an LV95 box, using all four corners.
fn degrees_of(bbox: &BBox) -> (f64, f64, f64, f64) {
    let corners = [
        (bbox.min_e, bbox.min_n),
        (bbox.max_e, bbox.min_n),
        (bbox.max_e, bbox.max_n),
        (bbox.min_e, bbox.max_n),
    ];
    let mut west = f64::INFINITY;
    let mut south = f64::INFINITY;
    let mut east = f64::NEG_INFINITY;
    let mut north = f64::NEG_INFINITY;
    for (e, n) in corners {
        let (lon, lat) = proj::lv95_to_wgs84(e, n);
        west = west.min(lon);
        east = east.max(lon);
        south = south.min(lat);
        north = north.max(lat);
    }
    (west, south, east, north)
}

fn tile_name(row: u32, col: u32) -> String {
    format!("tiles/{row:03}_{col:03}.jpg")
}

// ---------------------------------------------------------------------------
// KML and KMZ
// ---------------------------------------------------------------------------

/// XML-escape. Only the five predefined entities exist in XML, so a place name with an
/// ampersand in it must not be written through.
fn xml_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(c),
        }
    }
    out
}

/// `doc.kml` for a plan: one `GroundOverlay` per tile.
///
/// `drawOrder` is set because a device given two overlapping Custom Maps draws them in
/// an order it does not otherwise define; a single value across one map's tiles keeps
/// this map coherent, and tiles within a map do not overlap.
pub fn doc_kml(plan: &RasterPlan, map_name: &str) -> String {
    let mut s = String::with_capacity(512 + plan.tiles.len() * 320);
    s.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    s.push_str("<kml xmlns=\"http://www.opengis.net/kml/2.2\">\n  <Document>\n");
    s.push_str(&format!("    <name>{}</name>\n", xml_escape(map_name)));
    s.push_str(&format!(
        "    <description>{} — {}</description>\n",
        xml_escape(&plan.layer),
        xml_escape(ATTRIBUTION)
    ));
    for t in &plan.tiles {
        s.push_str("    <GroundOverlay>\n");
        s.push_str(&format!(
            "      <name>{}</name>\n",
            xml_escape(&format!("{map_name} {}/{}", t.row, t.col))
        ));
        s.push_str("      <drawOrder>50</drawOrder>\n");
        s.push_str(&format!(
            "      <Icon><href>{}</href></Icon>\n",
            xml_escape(&t.name)
        ));
        s.push_str(&format!(
            "      <LatLonBox><north>{:.8}</north><south>{:.8}</south>\
             <east>{:.8}</east><west>{:.8}</west></LatLonBox>\n",
            t.north, t.south, t.east, t.west
        ));
        s.push_str("    </GroundOverlay>\n");
    }
    s.push_str("  </Document>\n</kml>\n");
    s
}

/// Write the KMZ. `jpegs` maps a tile's member name to its bytes.
///
/// A tile whose bytes are missing is an error rather than a gap: a Custom Map with a
/// missing overlay renders as a hole in the middle of the map with nothing to explain it.
pub fn write_kmz(
    plan: &RasterPlan,
    map_name: &str,
    jpegs: &BTreeMap<String, Vec<u8>>,
    out: &Path,
) -> Result<u64> {
    if let Some(parent) = out.parent() {
        std::fs::create_dir_all(parent).map_err(|e| Error::io(parent, e))?;
    }
    let file = std::fs::File::create(out).map_err(|e| Error::io(out, e))?;
    let mut zip = ZipWriter::new(std::io::BufWriter::new(file));

    // doc.kml first: the format requires the KML, and a reader that stops early should
    // meet it before megabytes of imagery.
    zip.add("doc.kml", doc_kml(plan, map_name).as_bytes())?;
    for t in &plan.tiles {
        let bytes = jpegs.get(&t.name).ok_or_else(|| {
            Error::NotFound(format!(
                "no image for tile {} of {}, which would leave a hole in the map",
                t.name,
                plan.tile_count()
            ))
        })?;
        zip.add(&t.name, bytes)?;
    }
    let mut w = zip.finish()?;
    w.flush().map_err(|e| Error::io(out, e))?;
    Ok(std::fs::metadata(out).map_err(|e| Error::io(out, e))?.len())
}

// ---------------------------------------------------------------------------
// Fetch and build
// ---------------------------------------------------------------------------

/// Fetch one tile. A tile is at most a few megabytes, so it is held whole — the
/// streaming constraint (SPEC.md NFR-2) is about not holding the *archive*, which
/// [`build_kmz`] avoids by writing each tile as it arrives.
async fn fetch_tile(http: &dyn Http, url: &str) -> Result<Vec<u8>> {
    let mut stream = http.get_stream(url, 0).await?;
    let mut bytes = Vec::new();
    while let Some(chunk) = stream.next().await {
        bytes.extend_from_slice(&chunk?);
    }
    // The WMS reports failures as a 200 with an XML ServiceException body, so the status
    // code cannot be trusted; the magic bytes can. Without this check a failed tile
    // would be stored as a "JPEG" and render as nothing on the device.
    if bytes.len() < 4 || bytes[0..2] != [0xFF, 0xD8] {
        let head = String::from_utf8_lossy(&bytes[..bytes.len().min(300)]).to_string();
        return Err(Error::NotFound(format!(
            "the map service did not return a JPEG for a tile ({} bytes): {head}",
            bytes.len()
        )));
    }
    Ok(bytes)
}

/// Fetch every tile in `plan` and write the KMZ, one tile at a time.
///
/// `progress` is called with the number of tiles done and the total.
pub async fn build_kmz(
    http: &dyn Http,
    plan: &RasterPlan,
    map_name: &str,
    out: &Path,
    cancel: &Cancel,
    mut progress: impl FnMut(usize, usize),
) -> Result<RasterReport> {
    if let Some(parent) = out.parent() {
        std::fs::create_dir_all(parent).map_err(|e| Error::io(parent, e))?;
    }
    // Written to a temporary name and renamed, so a cancelled or failed build never
    // leaves a half-written KMZ that a device would try to read (FR-D4's rule, applied
    // here for the same reason).
    let partial = out.with_extension("kmz.partial");
    let file = std::fs::File::create(&partial).map_err(|e| Error::io(&partial, e))?;
    let mut zip = ZipWriter::new(std::io::BufWriter::new(file));
    zip.add("doc.kml", doc_kml(plan, map_name).as_bytes())?;

    let total = plan.tile_count();
    let mut oversized = 0usize;
    progress(0, total);
    for (i, tile) in plan.tiles.iter().enumerate() {
        if let Err(e) = cancel.check_cancelled() {
            let _ = std::fs::remove_file(&partial);
            return Err(e);
        }
        let bytes = match fetch_tile(http, &tile.wms_url(&plan.layer)).await {
            Ok(b) => b,
            Err(e) => {
                let _ = std::fs::remove_file(&partial);
                return Err(e);
            }
        };
        if bytes.len() as u64 > DEFAULT_MAX_BYTES_PER_TILE {
            oversized += 1;
        }
        if let Err(e) = zip.add(&tile.name, &bytes) {
            let _ = std::fs::remove_file(&partial);
            return Err(e);
        }
        progress(i + 1, total);
    }
    let mut w = zip.finish()?;
    w.flush().map_err(|e| Error::io(&partial, e))?;
    drop(w);
    std::fs::rename(&partial, out).map_err(|e| Error::io(out, e))?;

    let bytes = std::fs::metadata(out).map_err(|e| Error::io(out, e))?.len();
    let mut warnings = Vec::new();
    if oversized > 0 {
        warnings.push(format!(
            "{oversized} of {total} tiles are larger than {} MB, which some devices \
             re-compress at reduced quality.",
            DEFAULT_MAX_BYTES_PER_TILE / 1_000_000
        ));
    }
    Ok(RasterReport {
        kmz: out.to_path_buf(),
        bytes,
        tiles: total,
        m_per_px: plan.m_per_px,
        warnings,
    })
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RasterReport {
    pub kmz: std::path::PathBuf,
    pub bytes: u64,
    pub tiles: usize,
    pub m_per_px: f64,
    pub warnings: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Grindelwald, from the place index — the same point the build examples use.
    const GRINDELWALD: (f64, f64) = (2_645_921.0, 1_163_748.0);

    fn around(radius_m: f64) -> BBox {
        BBox::from_center(GRINDELWALD.0, GRINDELWALD.1, radius_m)
    }

    fn edge_limits(max_tiles: usize) -> RasterLimits {
        RasterLimits {
            max_tiles,
            ..RasterLimits::default()
        }
    }

    #[test]
    fn a_one_megapixel_budget_is_a_1024_pixel_square_tile() {
        assert_eq!(RasterLimits::default().tile_edge_px(), 1024);
        // A round-number limit does not divide into a power of two, and rounding up
        // would exceed the budget the device enforces.
        let l = RasterLimits {
            max_pixels_per_tile: 1_000_000,
            ..RasterLimits::default()
        };
        assert_eq!(l.tile_edge_px(), 1000);
    }

    #[test]
    fn no_tile_exceeds_the_device_pixel_budget() {
        let limits = RasterLimits::default();
        for radius in [1_000.0, 6_000.0, 25_000.0, 60_000.0] {
            let plan = plan(&around(radius), &limits, NATIVE_M_PER_PX).unwrap();
            for t in &plan.tiles {
                assert!(
                    t.pixels() <= limits.max_pixels_per_tile,
                    "{radius} m: tile {} is {} px, over the {} budget",
                    t.name,
                    t.pixels(),
                    limits.max_pixels_per_tile
                );
                assert!(t.width >= 1 && t.height >= 1);
            }
        }
    }

    #[test]
    fn the_tile_count_never_exceeds_the_device_limit() {
        // A national-scale request is the case that matters: it must coarsen, not
        // overflow the device.
        for max in [1, 4, 100, 500] {
            let plan = plan(&around(120_000.0), &edge_limits(max), NATIVE_M_PER_PX).unwrap();
            assert!(
                plan.tile_count() <= max,
                "{} tiles planned against a limit of {max}",
                plan.tile_count()
            );
        }
    }

    #[test]
    fn the_tiles_cover_the_whole_area_with_no_gap_and_no_overlap() {
        let plan = plan(&around(6_000.0), &RasterLimits::default(), NATIVE_M_PER_PX).unwrap();
        assert!(plan.cols > 1 && plan.rows > 1, "want a real grid to check");

        // The union of the tile boxes is the requested box, to within a rounding step.
        let tol = 1e-9;
        let w = plan
            .tiles
            .iter()
            .map(|t| t.west)
            .fold(f64::INFINITY, f64::min);
        let e = plan
            .tiles
            .iter()
            .map(|t| t.east)
            .fold(f64::NEG_INFINITY, f64::max);
        let s = plan
            .tiles
            .iter()
            .map(|t| t.south)
            .fold(f64::INFINITY, f64::min);
        let n = plan
            .tiles
            .iter()
            .map(|t| t.north)
            .fold(f64::NEG_INFINITY, f64::max);
        assert!((w - plan.west).abs() < tol, "west {w} vs {}", plan.west);
        assert!((e - plan.east).abs() < tol, "east {e} vs {}", plan.east);
        assert!((s - plan.south).abs() < tol, "south {s} vs {}", plan.south);
        assert!((n - plan.north).abs() < tol, "north {n} vs {}", plan.north);

        // Neighbours share an edge exactly, which is what "no gap and no overlap" means
        // for a grid: a gap shows on the device as a stripe of missing map.
        let at = |row: u32, col: u32| plan.tiles.iter().find(|t| t.row == row && t.col == col);
        for t in &plan.tiles {
            if let Some(right) = at(t.row, t.col + 1) {
                assert!((t.east - right.west).abs() < tol, "gap right of {}", t.name);
            }
            if let Some(below) = at(t.row + 1, t.col) {
                assert!((t.south - below.north).abs() < tol, "gap below {}", t.name);
            }
        }
    }

    #[test]
    fn ground_pixels_are_square_so_resolution_is_equal_in_both_directions() {
        let plan = plan(&around(6_000.0), &RasterLimits::default(), NATIVE_M_PER_PX).unwrap();
        let mid_lat = (plan.north + plan.south) / 2.0;
        let m_per_deg_lon = M_PER_DEG_LAT * mid_lat.to_radians().cos();
        for t in &plan.tiles {
            let res_x = (t.east - t.west) * m_per_deg_lon / t.width as f64;
            let res_y = (t.north - t.south) * M_PER_DEG_LAT / t.height as f64;
            assert!(
                (res_x / res_y - 1.0).abs() < 0.02,
                "{}: {res_x:.3} m/px across vs {res_y:.3} down",
                t.name
            );
        }
    }

    /// A degree of longitude is only 0.687 of a degree of latitude at Swiss latitudes,
    /// so a tile that is square *on the ground* spans correspondingly more longitude
    /// than latitude — 1/0.687 as much. Getting this the wrong way round would squash
    /// the map east-west by a factor of two, which is the kind of error that looks
    /// plausible on screen and is obvious on a hillside.
    #[test]
    fn a_square_tile_spans_more_longitude_than_latitude() {
        let plan = plan(&around(20_000.0), &RasterLimits::default(), NATIVE_M_PER_PX).unwrap();
        let full = plan
            .tiles
            .iter()
            .find(|t| t.width == 1024 && t.height == 1024)
            .expect("an interior tile");
        let ratio = (full.east - full.west) / (full.north - full.south);
        let expected = 1.0 / (46.62f64.to_radians().cos());
        assert!(
            (ratio - expected).abs() < 0.02,
            "longitude span ratio {ratio:.3}, expected ~{expected:.3} = 1/cos(46.62 deg)"
        );
    }

    #[test]
    fn a_request_finer_than_the_source_is_clamped_and_says_so() {
        let plan = plan(&around(3_000.0), &RasterLimits::default(), 0.25).unwrap();
        assert!((plan.m_per_px - NATIVE_M_PER_PX).abs() < 1e-9);
        assert!(
            plan.notes.iter().any(|n| n.contains("would only enlarge")),
            "{:?}",
            plan.notes
        );
    }

    #[test]
    fn an_area_too_large_to_cover_sharply_is_coarsened_and_says_so() {
        let plan = plan(&around(50_000.0), &RasterLimits::default(), NATIVE_M_PER_PX).unwrap();
        assert!(
            plan.m_per_px > NATIVE_M_PER_PX,
            "100 tiles cannot cover 100 km at 1.25 m/px"
        );
        assert!(
            plan.notes.iter().any(|n| n.contains("to fit in 100 tiles")),
            "{:?}",
            plan.notes
        );
    }

    #[test]
    fn a_resolution_worse_than_the_vector_map_is_called_out() {
        // Roughly the whole country against a 100-tile budget.
        let plan = plan(
            &around(120_000.0),
            &RasterLimits::default(),
            NATIVE_M_PER_PX,
        )
        .unwrap();
        assert!(plan.m_per_px > USELESS_M_PER_PX, "{}", plan.m_per_px);
        assert!(
            plan.notes
                .iter()
                .any(|n| n.contains("less detail than the vector")),
            "{:?}",
            plan.notes
        );
    }

    #[test]
    fn a_small_area_reaches_native_resolution_in_one_tile() {
        // 1024 px at 1.25 m is 1.28 km, so a 500 m box fits in a single tile.
        let plan = plan(&around(500.0), &RasterLimits::default(), NATIVE_M_PER_PX).unwrap();
        assert_eq!(plan.tile_count(), 1);
        assert!((plan.m_per_px - NATIVE_M_PER_PX).abs() < 1e-9);
        assert!(plan.notes.is_empty(), "{:?}", plan.notes);
    }

    /// The georeferencing is the whole feature: if the box is wrong the map is wrong in a
    /// way a user discovers on a mountain. So a known point must land in a tile that
    /// claims to contain it.
    #[test]
    fn the_tile_containing_grindelwald_is_the_one_whose_box_contains_its_coordinates() {
        let (lon, lat) = proj::lv95_to_wgs84(GRINDELWALD.0, GRINDELWALD.1);
        let plan = plan(&around(6_000.0), &RasterLimits::default(), NATIVE_M_PER_PX).unwrap();
        let hits: Vec<&RasterTile> = plan
            .tiles
            .iter()
            .filter(|t| t.west <= lon && lon <= t.east && t.south <= lat && lat <= t.north)
            .collect();
        assert_eq!(hits.len(), 1, "exactly one tile holds the centre: {hits:?}");
        // And it is near the middle of the grid, because the area is centred on it.
        let t = hits[0];
        assert!(t.row < plan.rows && t.col < plan.cols);
    }

    /// All four LV95 corners must be projected, because the box's northern edge is not a
    /// line of constant latitude. Taking two corners clips a sliver off the area.
    #[test]
    fn all_four_corners_are_projected_so_the_area_is_not_clipped() {
        // Far west, where LV95's skew relative to lat/lon is largest.
        let bbox = BBox::new(2_495_000.0, 1_120_000.0, 2_545_000.0, 1_170_000.0);
        let (west, south, east, north) = degrees_of(&bbox);
        for (e, n) in [
            (bbox.min_e, bbox.min_n),
            (bbox.max_e, bbox.min_n),
            (bbox.max_e, bbox.max_n),
            (bbox.min_e, bbox.max_n),
        ] {
            let (lon, lat) = proj::lv95_to_wgs84(e, n);
            assert!(west <= lon && lon <= east, "{lon} outside {west}..{east}");
            assert!(
                south <= lat && lat <= north,
                "{lat} outside {south}..{north}"
            );
        }
        // And the skew is real, not a rounding artefact: the two northern corners differ
        // in latitude by enough to matter at this box size.
        let (_, lat_nw) = proj::lv95_to_wgs84(bbox.min_e, bbox.max_n);
        let (_, lat_ne) = proj::lv95_to_wgs84(bbox.max_e, bbox.max_n);
        assert!(
            (lat_nw - lat_ne).abs() > 1e-4,
            "expected oblique-Mercator skew, got {lat_nw} vs {lat_ne}"
        );
    }

    #[test]
    fn the_wms_url_puts_latitude_first_as_epsg_4326_declares() {
        let t = RasterTile {
            row: 0,
            col: 0,
            name: "tiles/000_000.jpg".into(),
            west: 8.0,
            south: 46.6,
            east: 8.08,
            north: 46.65,
            width: 1024,
            height: 683,
        };
        let url = t.wms_url(DEFAULT_LAYER);
        // Checked against the live service: the wrong order returns a blank tile from
        // outside Switzerland rather than an error, so this is asserted explicitly.
        assert!(
            url.contains("&BBOX=46.60000000,8.00000000,46.65000000,8.08000000&"),
            "{url}"
        );
        assert!(url.contains("CRS=EPSG:4326"), "{url}");
        assert!(url.contains("WIDTH=1024&HEIGHT=683"), "{url}");
        assert!(url.contains("VERSION=1.3.0"), "{url}");
        assert!(url.contains("FORMAT=image/jpeg"), "{url}");
        assert!(!url.contains(' '), "a URL with a space in it is not a URL");
    }

    #[test]
    fn the_kml_has_one_ground_overlay_per_tile_and_carries_the_attribution() {
        let plan = plan(&around(3_000.0), &RasterLimits::default(), NATIVE_M_PER_PX).unwrap();
        let kml = doc_kml(&plan, "Grindelwald");
        assert_eq!(kml.matches("<GroundOverlay>").count(), plan.tile_count());
        assert_eq!(kml.matches("</GroundOverlay>").count(), plan.tile_count());
        assert_eq!(kml.matches("<LatLonBox>").count(), plan.tile_count());
        for t in &plan.tiles {
            assert!(
                kml.contains(&format!("<href>{}</href>", t.name)),
                "{}",
                t.name
            );
        }
        // FR-L1: the attribution travels with the map, not just with the app.
        assert!(kml.contains(ATTRIBUTION), "{kml}");
        assert!(kml.contains(DEFAULT_LAYER));
        assert!(kml.starts_with("<?xml version=\"1.0\" encoding=\"UTF-8\"?>"));
    }

    #[test]
    fn a_name_with_xml_metacharacters_is_escaped() {
        let plan = plan(&around(500.0), &RasterLimits::default(), NATIVE_M_PER_PX).unwrap();
        let kml = doc_kml(&plan, "Ins & Outs <test>");
        assert!(kml.contains("Ins &amp; Outs &lt;test&gt;"), "{kml}");
        assert!(
            !kml.contains("Outs <test>"),
            "raw markup leaked into the KML"
        );
    }

    #[test]
    fn an_empty_or_absurd_request_is_refused_with_a_reason() {
        let l = RasterLimits::default();
        let empty = BBox::new(2_600_000.0, 1_200_000.0, 2_600_000.0, 1_200_000.0);
        assert!(plan(&empty, &l, 1.25)
            .unwrap_err()
            .to_string()
            .contains("empty"));
        assert!(plan(&around(1_000.0), &edge_limits(0), 1.25).is_err());
        for bad in [0.0, -1.0, f64::NAN, f64::INFINITY] {
            assert!(plan(&around(1_000.0), &l, bad).is_err(), "accepted {bad}");
        }
    }

    #[test]
    fn writing_a_kmz_stores_the_kml_and_every_tile() {
        let dir = tempfile::tempdir().unwrap();
        let plan = plan(&around(2_000.0), &RasterLimits::default(), NATIVE_M_PER_PX).unwrap();
        let jpegs: BTreeMap<String, Vec<u8>> = plan
            .tiles
            .iter()
            .map(|t| (t.name.clone(), format!("jpeg for {}", t.name).into_bytes()))
            .collect();
        let out = dir.path().join("nested").join("map.kmz");
        let bytes = write_kmz(&plan, "Grindelwald", &jpegs, &out).unwrap();
        assert_eq!(bytes, std::fs::metadata(&out).unwrap().len());

        // Stored members appear verbatim, so the archive can be checked without a reader.
        let raw = std::fs::read(&out).unwrap();
        let contains = |needle: &[u8]| raw.windows(needle.len()).any(|w| w == needle);
        assert!(contains(b"doc.kml"), "doc.kml is named in the archive");
        assert!(
            contains(b"<GroundOverlay>"),
            "the KML is stored uncompressed"
        );
        for t in &plan.tiles {
            assert!(contains(t.name.as_bytes()), "{} missing", t.name);
            assert!(
                contains(jpegs[&t.name].as_slice()),
                "{} payload missing",
                t.name
            );
        }
    }

    #[test]
    fn a_missing_tile_is_an_error_rather_than_a_hole_in_the_map() {
        let dir = tempfile::tempdir().unwrap();
        let plan = plan(&around(3_000.0), &RasterLimits::default(), NATIVE_M_PER_PX).unwrap();
        assert!(plan.tile_count() > 1);
        let mut jpegs: BTreeMap<String, Vec<u8>> = plan
            .tiles
            .iter()
            .map(|t| (t.name.clone(), b"jpeg".to_vec()))
            .collect();
        jpegs.remove(&plan.tiles[1].name);
        let e = write_kmz(&plan, "x", &jpegs, &dir.path().join("m.kmz")).expect_err("gap");
        assert!(e.to_string().contains("hole in the map"), "{e}");
    }

    #[test]
    fn the_size_estimate_scales_with_the_pixels_planned() {
        let small = plan(&around(1_000.0), &RasterLimits::default(), NATIVE_M_PER_PX).unwrap();
        let big = plan(&around(4_000.0), &RasterLimits::default(), NATIVE_M_PER_PX).unwrap();
        assert!(big.total_pixels() > small.total_pixels());
        assert!(big.approx_bytes() > small.approx_bytes());
        assert_eq!(
            small.approx_bytes(),
            (small.total_pixels() as f64 * BYTES_PER_PIXEL) as u64
        );
        // The bound must actually bound the typical figure, or it is not a bound.
        assert!(small.max_bytes() > small.approx_bytes());
        assert!(big.max_bytes() > big.approx_bytes());
        // A full 100-tile map stays trivial against any device's map budget, which is
        // the point the measurements settled.
        let full = plan(&around(6_000.0), &RasterLimits::default(), NATIVE_M_PER_PX).unwrap();
        assert_eq!(full.tile_count(), 100);
        assert!(
            full.max_bytes() < 100_000_000,
            "a full raster map is {} B",
            full.max_bytes()
        );
    }

    // -- fetch and build -------------------------------------------------------

    /// A one-pixel JPEG is not needed; only the magic bytes are checked, and using a
    /// recognisable body makes a failure legible.
    fn fake_jpeg(tag: &str) -> Vec<u8> {
        let mut v = vec![0xFF, 0xD8, 0xFF, 0xE0];
        v.extend_from_slice(tag.as_bytes());
        v
    }

    fn serving(plan: &RasterPlan) -> crate::testing::FakeHttp {
        let mut http = crate::testing::FakeHttp::new();
        for t in &plan.tiles {
            http = http.with_body(&t.wms_url(&plan.layer), fake_jpeg(&t.name));
        }
        http
    }

    #[tokio::test]
    async fn building_a_kmz_fetches_every_tile_and_reports_what_it_wrote() {
        let dir = tempfile::tempdir().unwrap();
        let plan = plan(&around(2_000.0), &RasterLimits::default(), NATIVE_M_PER_PX).unwrap();
        let http = serving(&plan);
        let out = dir.path().join("Grindelwald.kmz");
        let mut seen = Vec::new();
        let report = build_kmz(&http, &plan, "Grindelwald", &out, &Cancel::new(), |d, t| {
            seen.push((d, t))
        })
        .await
        .unwrap();

        assert_eq!(report.tiles, plan.tile_count());
        assert_eq!(report.bytes, std::fs::metadata(&out).unwrap().len());
        assert!(report.warnings.is_empty(), "{:?}", report.warnings);
        // Progress runs from 0 to n inclusive, so a UI can show a bar before the first
        // tile lands rather than jumping in at 1/n.
        assert_eq!(seen.first(), Some(&(0, plan.tile_count())));
        assert_eq!(seen.last(), Some(&(plan.tile_count(), plan.tile_count())));

        let raw = std::fs::read(&out).unwrap();
        for t in &plan.tiles {
            let body = fake_jpeg(&t.name);
            assert!(
                raw.windows(body.len()).any(|w| w == body.as_slice()),
                "{} not in the archive",
                t.name
            );
        }
        assert!(!dir.path().join("Grindelwald.kmz.partial").exists());
    }

    /// The WMS answers a bad request with HTTP 200 and an XML ServiceException, so the
    /// status code proves nothing. Without the magic-byte check that body would be
    /// stored as a tile and render as an empty rectangle on the device.
    #[tokio::test]
    async fn an_xml_service_exception_served_as_200_is_an_error_not_a_tile() {
        let dir = tempfile::tempdir().unwrap();
        let plan = plan(&around(500.0), &RasterLimits::default(), NATIVE_M_PER_PX).unwrap();
        let http = crate::testing::FakeHttp::new().with_body(
            &plan.tiles[0].wms_url(&plan.layer),
            br#"<?xml version="1.0"?><ServiceExceptionReport><ServiceException code="InvalidCRS"/></ServiceExceptionReport>"#.to_vec(),
        );
        let out = dir.path().join("m.kmz");
        let e = build_kmz(&http, &plan, "m", &out, &Cancel::new(), |_, _| {})
            .await
            .expect_err("not a jpeg");
        assert!(e.to_string().contains("did not return a JPEG"), "{e}");
        assert!(
            e.to_string().contains("ServiceException"),
            "the body is quoted: {e}"
        );
        assert!(!out.exists(), "a failed build leaves no kmz");
    }

    #[tokio::test]
    async fn a_failed_or_cancelled_build_leaves_no_partial_file_behind() {
        let dir = tempfile::tempdir().unwrap();
        let plan = plan(&around(3_000.0), &RasterLimits::default(), NATIVE_M_PER_PX).unwrap();
        assert!(plan.tile_count() > 2);

        // Cancelled part way through.
        let cancel = Cancel::new();
        let http = serving(&plan);
        let out = dir.path().join("c.kmz");
        let e = build_kmz(&http, &plan, "c", &out, &cancel, |done, _| {
            if done == 2 {
                cancel.cancel();
            }
        })
        .await
        .expect_err("cancelled");
        assert!(matches!(e, Error::Cancelled), "{e}");
        assert!(!out.exists());
        assert!(!out.with_extension("kmz.partial").exists());

        // A tile the service does not serve at all.
        let out2 = dir.path().join("d.kmz");
        assert!(build_kmz(
            &crate::testing::FakeHttp::new(),
            &plan,
            "d",
            &out2,
            &Cancel::new(),
            |_, _| {}
        )
        .await
        .is_err());
        assert!(!out2.exists());
        assert!(!out2.with_extension("kmz.partial").exists());
    }

    #[tokio::test]
    async fn an_oversized_tile_is_warned_about_rather_than_refused() {
        let dir = tempfile::tempdir().unwrap();
        let plan = plan(&around(500.0), &RasterLimits::default(), NATIVE_M_PER_PX).unwrap();
        let mut big = fake_jpeg("big");
        big.resize(DEFAULT_MAX_BYTES_PER_TILE as usize + 1, 0x20);
        let http =
            crate::testing::FakeHttp::new().with_body(&plan.tiles[0].wms_url(&plan.layer), big);
        let report = build_kmz(
            &http,
            &plan,
            "big",
            &dir.path().join("big.kmz"),
            &Cancel::new(),
            |_, _| {},
        )
        .await
        .expect("a heavy tile is still a valid tile");
        assert_eq!(report.tiles, 1);
        assert!(
            report.warnings.iter().any(|w| w.contains("re-compress")),
            "{:?}",
            report.warnings
        );
    }

    #[test]
    fn a_kmz_filename_is_safe_on_a_device_filesystem() {
        assert_eq!(kmz_filename("Grindelwald hiking"), "Grindelwald-hiking.kmz");
        assert_eq!(kmz_filename("Val d'Hérens"), "Val-d-H-rens.kmz");
        // Leading and trailing runs are trimmed rather than left as dashes, and a name
        // with nothing usable in it still yields a valid filename.
        assert_eq!(kmz_filename("  Zermatt  "), "Zermatt.kmz");
        assert_eq!(kmz_filename("///"), "map.kmz");
        assert_eq!(kmz_filename(""), "map.kmz");
        for name in ["Grindelwald hiking", "Val d'Hérens", "///", ""] {
            let f = kmz_filename(name);
            assert!(f.is_ascii() && f.ends_with(".kmz"), "{f}");
            assert!(
                !f.contains('/') && !f.contains('\\') && !f.contains(':'),
                "{f} is not safe as a path component"
            );
            assert_eq!(f.matches('.').count(), 1, "{f} has a double extension");
        }
    }
}
