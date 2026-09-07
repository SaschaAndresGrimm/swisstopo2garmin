//! swissALTI3D elevation tiles and mosaicking (SPEC.md §7.4, FR-P13/FR-P15).
//!
//! Tiles are a strict 1 km LV95 grid: cell `EEEE-NNNN` covers easting `EEEE*1000` to
//! `+1000` and northing `NNNN*1000` to `+1000`, as 500x500 Float32 samples at 2 m,
//! nodata -9999. The grid is exploited rather than probed — Milestone 0 found that
//! asking GDAL to read each remote tile's geotransform took minutes for a few hundred
//! tiles, while deriving the layout takes no requests at all.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::error::{Error, Result};
use crate::proj::BBox;
use crate::stac::{Item, ALTI3D};

/// Native resolution of the 2 m product. 0.5 m exists but is 16x the data for no
/// benefit at Garmin's ~2.4 m grid.
pub const TILE_RES_M: f64 = 2.0;
pub const TILE_PX: usize = 500;
pub const TILE_SPAN_M: f64 = 1000.0;
pub const NODATA: f32 = -9999.0;
const ASSET_SUFFIX: &str = "_2_2056_5728.tif";

/// A 1 km grid cell, identified by its LV95 kilometre coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Cell {
    pub e_km: i32,
    pub n_km: i32,
}

impl Cell {
    pub fn origin(&self) -> (f64, f64) {
        (self.e_km as f64 * 1000.0, self.n_km as f64 * 1000.0)
    }

    /// The cell as swisstopo names it: `2645-1163`, the identifier that appears in the
    /// swissALTI3D file name and on the download portal.
    ///
    /// Used in build warnings, so a user told that contours have gaps can look up
    /// exactly which square kilometres and check the portal for themselves — which is
    /// what SPEC.md §12 means by "the build report says which cells".
    pub fn label(&self) -> String {
        format!("{}-{}", self.e_km, self.n_km)
    }

    /// Cells covering `bbox`, in row-major order.
    /// The cells a mask actually needs, dilated by one cell.
    ///
    /// A canton's bounding box is more than twice its area and a route corridor's is
    /// most of the country, so fetching the whole box downloads and stores tiles whose
    /// contours the mask then throws away — 14.5 GB against 6.5 GB for Valais.
    ///
    /// Dilated by one cell because contours are generated from a grid: a tile whose
    /// neighbour is absent has no data to interpolate against at that edge, which would
    /// put a seam exactly along the boundary the user selected.
    pub fn covering_mask(bbox: &BBox, mask: &crate::mask::Mask) -> Vec<Cell> {
        use std::collections::HashSet;

        let all = Cell::covering(bbox);
        let mut keep: HashSet<Cell> = HashSet::new();
        for cell in &all {
            let (e, n) = cell.origin();
            let s = TILE_SPAN_M;
            // Corners and centre: a cell smaller than the mask's detail could otherwise
            // straddle it with every corner outside.
            let probes = [
                (e, n),
                (e + s, n),
                (e, n + s),
                (e + s, n + s),
                (e + s / 2.0, n + s / 2.0),
            ];
            if probes
                .iter()
                .any(|(pe, pn)| mask.contains(crate::geom::Coord::new(*pe, *pn)))
            {
                keep.insert(*cell);
            }
        }
        // Dilate: keep every neighbour of a kept cell.
        let mut out: HashSet<Cell> = HashSet::new();
        for c in &keep {
            for dn in -1..=1 {
                for de in -1..=1 {
                    out.insert(Cell {
                        e_km: c.e_km + de,
                        n_km: c.n_km + dn,
                    });
                }
            }
        }
        // Never fetch outside the requested box: the dilation can step past its edge.
        let inside: HashSet<Cell> = all.iter().copied().collect();
        let mut v: Vec<Cell> = out.intersection(&inside).copied().collect();
        v.sort_by_key(|c| (c.n_km, c.e_km));
        v
    }

    pub fn covering(bbox: &BBox) -> Vec<Cell> {
        let e0 = (bbox.min_e / TILE_SPAN_M).floor() as i32;
        let e1 = ((bbox.max_e / TILE_SPAN_M).ceil() as i32 - 1).max(e0);
        let n0 = (bbox.min_n / TILE_SPAN_M).floor() as i32;
        let n1 = ((bbox.max_n / TILE_SPAN_M).ceil() as i32 - 1).max(n0);
        let mut out = Vec::new();
        for n_km in n0..=n1 {
            for e_km in e0..=e1 {
                out.push(Cell { e_km, n_km });
            }
        }
        out
    }

    /// Parse the cell out of a swissALTI3D asset name or item id.
    pub fn from_name(name: &str) -> Option<(u32, Cell)> {
        // swissalti3d_<year>_<EEEE>-<NNNN>...
        let rest = name.strip_prefix("swissalti3d_")?;
        let (year, rest) = rest.split_once('_')?;
        let year: u32 = year.parse().ok()?;
        let cell_part = rest.split('_').next()?;
        let (e, n) = cell_part.split_once('-')?;
        Some((
            year,
            Cell {
                e_km: e.parse().ok()?,
                n_km: n.parse().ok()?,
            },
        ))
    }
}

/// Newest asset URL per cell.
///
/// swissALTI3D publishes one STAC item per acquisition campaign, so the same cell comes
/// back for several years — every cell in the Grindelwald bbox exists for both 2019 and
/// 2022. Feeding both into a mosaic doubles the bytes read and makes elevation depend on
/// source ordering (FR-P13).
pub fn newest_per_cell(items: &[Item]) -> HashMap<Cell, (u32, String)> {
    let mut best: HashMap<Cell, (u32, String)> = HashMap::new();
    for item in items {
        for asset in &item.assets {
            if !asset.name.ends_with(ASSET_SUFFIX) {
                continue;
            }
            let Some((year, cell)) = Cell::from_name(&asset.name) else {
                continue;
            };
            match best.get(&cell) {
                Some((have, _)) if *have >= year => {}
                _ => {
                    best.insert(cell, (year, asset.href.clone()));
                }
            }
        }
    }
    best
}

/// Local path for a cached tile.
pub fn tile_cache_path(root: &Path, cell: &Cell, year: u32) -> PathBuf {
    root.join(ALTI3D).join(format!(
        "swissalti3d_{year}_{:04}-{:04}.tif",
        cell.e_km, cell.n_km
    ))
}

/// A decoded elevation tile: `TILE_PX * TILE_PX` samples, row-major from the north-west.
pub struct Tile {
    pub cell: Cell,
    pub samples: Vec<f32>,
}

impl Tile {
    pub fn decode(path: &Path, cell: Cell) -> Result<Self> {
        let file = std::fs::File::open(path).map_err(|e| Error::io(path, e))?;
        let mut dec = tiff::decoder::Decoder::new(std::io::BufReader::new(file))
            .map_err(|e| Error::Inflate(format!("{}: {e}", path.display())))?;
        let (w, h) = dec
            .dimensions()
            .map_err(|e| Error::Inflate(format!("{}: {e}", path.display())))?;
        if w as usize != TILE_PX || h as usize != TILE_PX {
            return Err(Error::Inflate(format!(
                "{}: expected {TILE_PX}x{TILE_PX}, got {w}x{h}",
                path.display()
            )));
        }
        let image = dec
            .read_image()
            .map_err(|e| Error::Inflate(format!("{}: {e}", path.display())))?;
        let samples = match image {
            tiff::decoder::DecodingResult::F32(v) => v,
            other => {
                return Err(Error::Inflate(format!(
                    "{}: expected Float32 samples, got {:?}",
                    path.display(),
                    std::mem::discriminant(&other)
                )))
            }
        };
        if samples.len() != TILE_PX * TILE_PX {
            return Err(Error::Inflate(format!(
                "{}: {} samples, expected {}",
                path.display(),
                samples.len(),
                TILE_PX * TILE_PX
            )));
        }
        Ok(Self { cell, samples })
    }

    pub fn at(&self, col: usize, row: usize) -> f32 {
        self.samples[row * TILE_PX + col]
    }
}

/// A mosaic of elevation tiles on the 2 m LV95 grid.
///
/// Built from whole cells with no gaps, so a contour crossing a tile boundary is
/// continuous by construction — there is no per-tile seam to stitch (FR-P7).
pub struct Grid {
    /// LV95 easting of the west edge, northing of the **south** edge.
    pub origin_e: f64,
    pub origin_n: f64,
    pub cols: usize,
    pub rows: usize,
    /// Row-major from the **south-west**, so increasing row means increasing northing.
    pub samples: Vec<f32>,
}

impl Grid {
    pub fn from_tiles(cells: &[Cell], tiles: &HashMap<Cell, Tile>) -> Result<Self> {
        if cells.is_empty() {
            return Err(Error::NotFound("no elevation cells requested".into()));
        }
        let min_e = cells.iter().map(|c| c.e_km).min().unwrap();
        let max_e = cells.iter().map(|c| c.e_km).max().unwrap();
        let min_n = cells.iter().map(|c| c.n_km).min().unwrap();
        let max_n = cells.iter().map(|c| c.n_km).max().unwrap();

        let cols = (max_e - min_e + 1) as usize * TILE_PX;
        let rows = (max_n - min_n + 1) as usize * TILE_PX;
        let mut samples = vec![NODATA; cols * rows];

        for (cell, tile) in tiles {
            let cx = (cell.e_km - min_e) as usize * TILE_PX;
            let cy = (cell.n_km - min_n) as usize * TILE_PX;
            for row in 0..TILE_PX {
                // Tile rows run north to south; the grid runs south to north.
                let src_row = TILE_PX - 1 - row;
                let dst = (cy + row) * cols + cx;
                let src = src_row * TILE_PX;
                samples[dst..dst + TILE_PX].copy_from_slice(&tile.samples[src..src + TILE_PX]);
            }
        }

        Ok(Self {
            origin_e: min_e as f64 * 1000.0,
            origin_n: min_n as f64 * 1000.0,
            cols,
            rows,
            samples,
        })
    }

    pub fn at(&self, col: usize, row: usize) -> f32 {
        self.samples[row * self.cols + col]
    }

    /// LV95 coordinate of a sample centre.
    pub fn coord(&self, col: usize, row: usize) -> (f64, f64) {
        (
            self.origin_e + (col as f64 + 0.5) * TILE_RES_M,
            self.origin_n + (row as f64 + 0.5) * TILE_RES_M,
        )
    }

    pub fn bbox(&self) -> BBox {
        BBox::new(
            self.origin_e,
            self.origin_n,
            self.origin_e + self.cols as f64 * TILE_RES_M,
            self.origin_n + self.rows as f64 * TILE_RES_M,
        )
    }

    /// Elevation range over valid samples.
    pub fn range(&self) -> Option<(f32, f32)> {
        let mut lo = f32::MAX;
        let mut hi = f32::MIN;
        for &v in &self.samples {
            if v != NODATA && v.is_finite() {
                lo = lo.min(v);
                hi = hi.max(v);
            }
        }
        (lo <= hi).then_some((lo, hi))
    }
}

// ---------------------------------------------------------------------------
// Fetching
// ---------------------------------------------------------------------------

use std::collections::HashSet;

use crate::download::{download, Cancel};
use crate::http::Http;
use crate::stac::Stac;

#[derive(Debug, Clone, Default)]
pub struct FetchStats {
    pub requested: usize,
    pub already_cached: usize,
    pub downloaded: usize,
    pub bytes: u64,
    /// Cells with no swissALTI3D coverage. Outside Switzerland this is normal.
    pub missing: Vec<Cell>,
    /// Older duplicates skipped by [`newest_per_cell`].
    pub duplicates_skipped: usize,
}

/// Download every elevation tile covering `bbox` into the cache, concurrently.
///
/// Contour generation is network-bound and is the dominant cost of a build, so tiles
/// are fetched several at a time and reused across builds (FR-P15).
pub async fn fetch_tiles(
    http: &dyn Http,
    cache_root: &Path,
    bbox: &BBox,
    // `mask` restricts the fetch to the tiles the shape needs; None fetches the box.
    mask: Option<&crate::mask::Mask>,
    concurrency: usize,
    cancel: &Cancel,
    mut progress: impl FnMut(&FetchStats),
) -> Result<(HashMap<Cell, PathBuf>, FetchStats)> {
    let stac = Stac::new(http);
    let items = stac.items_in_bbox(ALTI3D, bbox.to_wgs84(), 200).await?;
    let newest = newest_per_cell(&items);

    let wanted: Vec<Cell> = match mask {
        Some(m) => Cell::covering_mask(bbox, m),
        None => Cell::covering(bbox),
    };
    let wanted_set: HashSet<Cell> = wanted.iter().copied().collect();

    let mut stats = FetchStats {
        requested: wanted.len(),
        duplicates_skipped: items
            .iter()
            .flat_map(|i| i.assets.iter())
            .filter(|a| a.name.ends_with(ASSET_SUFFIX))
            .count()
            .saturating_sub(newest.len()),
        ..Default::default()
    };

    let mut todo: Vec<(Cell, u32, String, PathBuf)> = Vec::new();
    let mut resolved: HashMap<Cell, PathBuf> = HashMap::new();

    for cell in &wanted {
        match newest.get(cell) {
            None => stats.missing.push(*cell),
            Some((year, href)) => {
                let path = tile_cache_path(cache_root, cell, *year);
                if path.exists() {
                    stats.already_cached += 1;
                    resolved.insert(*cell, path);
                } else {
                    todo.push((*cell, *year, href.clone(), path));
                }
            }
        }
    }
    // A bbox query returns whole items, so STAC can hand back cells just outside the
    // requested box; those are simply not in `wanted` and are ignored.
    debug_assert!(
        wanted_set.len() == wanted.len(),
        "Cell::covering must not repeat a cell"
    );
    progress(&stats);

    let concurrency = concurrency.max(1);
    for batch in todo.chunks(concurrency) {
        if cancel.is_cancelled() {
            return Err(Error::Cancelled);
        }
        let mut handles = Vec::new();
        for (cell, _year, href, path) in batch {
            handles.push(async move {
                let r = download(http, href, path, None, cancel, &mut |_| {}).await;
                (*cell, path.clone(), r)
            });
        }
        for (cell, path, result) in futures_util::future::join_all(handles).await {
            match result {
                Ok(p) => {
                    stats.downloaded += 1;
                    stats.bytes += std::fs::metadata(&p).map(|m| m.len()).unwrap_or(0);
                    resolved.insert(cell, path);
                }
                Err(Error::Cancelled) => return Err(Error::Cancelled),
                Err(e) => {
                    // A single missing tile leaves a gap in the contours rather than
                    // failing the whole build; the build report names the cells.
                    tracing::warn!("elevation tile {cell:?} failed: {e}");
                    stats.missing.push(cell);
                }
            }
        }
        progress(&stats);
    }

    Ok((resolved, stats))
}

/// Decode cached tiles into one mosaic.
pub fn load_grid(paths: &HashMap<Cell, PathBuf>, bbox: &BBox) -> Result<Grid> {
    let cells = Cell::covering(bbox);
    let mut tiles = HashMap::new();
    for cell in &cells {
        if let Some(path) = paths.get(cell) {
            match Tile::decode(path, *cell) {
                Ok(t) => {
                    tiles.insert(*cell, t);
                }
                Err(e) => tracing::warn!("elevation tile {cell:?} undecodable: {e}"),
            }
        }
    }
    Grid::from_tiles(&cells, &tiles)
}
