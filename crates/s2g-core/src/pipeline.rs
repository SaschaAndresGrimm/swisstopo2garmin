//! Build orchestration: recipe in, verified `gmapsupp.img` out (SPEC.md §7, FR-70).
//!
//! Stages are reported individually so the UI can show where a build is and what
//! failed. Cancellation is checked between and inside stages, and a cancelled build
//! leaves no partial output.

use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::contour::{generate, ContourConfig};
use crate::dem;
use crate::devices::DeviceProfile;
use crate::download::Cancel;
use crate::elevation::{fetch_tiles, load_grid};
use crate::error::{Error, Result};
use crate::estimate;
use crate::extract::{RegionBuilder, CYCLE_LAYERS, DEFAULT_LAYERS, WINTER_LAYERS};
use crate::garmin::{compile, split, style_level_count, BuildOptions, MapIdentity, Toolchain};
use crate::geom::point_in_polygon;
use crate::gpkg::Gpkg;
use crate::http::Http;
use crate::img;
use crate::proj::BBox;
use crate::recipe::Recipe;
use crate::shapefile::Shapefile;

/// Where a build has got to. Ordered as the user sees them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Stage {
    Extract,
    Elevation,
    Contours,
    Relief,
    Split,
    Compile,
    Verify,
    /// Only runs when the recipe asks for a raster overlay; a skipped stage is normal
    /// here, as it already is for `Relief`.
    Raster,
}

impl Stage {
    pub fn all() -> &'static [Stage] {
        &[
            Stage::Extract,
            Stage::Elevation,
            Stage::Contours,
            Stage::Relief,
            Stage::Split,
            Stage::Compile,
            Stage::Verify,
            Stage::Raster,
        ]
    }
    pub fn label(&self) -> &'static str {
        match self {
            Stage::Extract => "Reading swisstopo data",
            Stage::Elevation => "Fetching elevation tiles",
            Stage::Contours => "Generating contours",
            Stage::Relief => "Building relief data",
            Stage::Split => "Splitting into map tiles",
            Stage::Compile => "Compiling the Garmin map",
            Stage::Verify => "Verifying the result",
            Stage::Raster => "Fetching the paper map overlay",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct StageUpdate {
    pub stage: Stage,
    /// 0.0 to 1.0 within the stage, when known.
    pub fraction: Option<f64>,
    pub detail: String,
}

/// Everything the app needs to run a build.
pub struct BuildContext<'a> {
    pub toolchain: Toolchain,
    pub style_root: PathBuf,
    pub typ_root: PathBuf,
    pub cache_root: PathBuf,
    pub work_dir: PathBuf,
    pub http: &'a dyn Http,
    /// Where to record (predictors -> actual size) after a successful build (FR-62).
    /// `None` disables recording, which tests want.
    pub calibration_log: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize)]
pub struct BuildReport {
    pub gmapsupp: PathBuf,
    pub bytes: u64,
    pub tile_count: usize,
    pub features: u64,
    pub nodes: u64,
    pub ways: u64,
    pub contour_lines: usize,
    /// Slope-class areas drawn, when the recipe asked for them.
    pub slope_areas: usize,
    pub has_dem: bool,
    pub warnings: Vec<String>,
    /// The manifest written beside the output (FR-71), when it could be written.
    pub manifest: Option<PathBuf>,
    /// Deterministic identity, so the same recipe keeps its place on the device.
    pub family_id: u16,
    /// The raster overlay, when the recipe asked for one and it succeeded (FR-R1).
    /// A separate file from the `.img`, installed to a different directory.
    pub raster: Option<crate::raster::RasterReport>,
    /// Seconds spent in each stage, in the order they ran. Feeds the time estimate
    /// shown during the next build (FR-71).
    pub stage_seconds: Vec<(Stage, f64)>,
}

/// Build the region PBF: extract, elevation, contours and slope classes.
///
/// Split out so a cache hit can skip the lot. Takes the pieces it needs rather than the
/// whole build state, which keeps what the cached unit depends on visible in the
/// signature — and that list is exactly what `region_key` hashes.
#[allow(clippy::too_many_arguments)]
async fn build_region(
    ctx: &BuildContext<'_>,
    recipe: &Recipe,
    bbox: &BBox,
    gpkg_path: &Path,
    area_mask: Option<&std::sync::Arc<crate::mask::Mask>>,
    cancel: &Cancel,
    on_stage: &mut impl FnMut(StageUpdate),
    warnings: &mut Vec<String>,
    contour_lines: &mut usize,
    slope_areas: &mut usize,
) -> Result<(
    PathBuf,
    crate::extract::ExtractStats,
    Option<crate::elevation::Grid>,
)> {
    let pbf = ctx.work_dir.join("region.osm.pbf");
    let mut builder = RegionBuilder::create(&pbf, bbox)?;
    if let Some(mask) = area_mask {
        builder = builder.with_mask((*mask).clone());
    }

    // The GeoPackage connection is not Send, and this function awaits: everything read
    // from it happens here, and the handle is dropped before the first await. `ice` is
    // read now although it is used after the elevation fetch, for the same reason.
    let gpkg = Gpkg::open(gpkg_path)?;

    // Labels in the chosen language, where swissNAMES3D has one (FR-53). Loaded before
    // the extraction rather than per feature: it is a single pass over the CSVs.
    if recipe.label_language != crate::names::LabelLanguage::Local {
        match crate::names::find_names3d(&ctx.cache_root) {
            Some(dir) => {
                let index = crate::names::NameIndex::load(&dir, recipe.label_language)?;
                if index.is_empty() {
                    warnings.push(format!(
                        "swissNAMES3D has no names in {}, so labels stay local",
                        recipe.label_language.id()
                    ));
                } else {
                    builder = builder.with_names(std::sync::Arc::new(index));
                }
            }
            None => warnings.push(
                "swissNAMES3D is not downloaded, so labels stay in the local language".into(),
            ),
        }
    }
    // A corridor or administrative unit is not a rectangle: the bbox is the cheap
    // first cut and the mask decides what actually survives it.
    if let Some(mask) = area_mask {
        builder = builder.with_mask((*mask).clone());
    }
    let excluded = &recipe.excluded_layers;
    let keep = |name: &str| !excluded.iter().any(|x| x == name);

    let layers: Vec<_> = DEFAULT_LAYERS.iter().filter(|l| keep(l.layer)).collect();
    let total = layers.len().max(1);
    for (i, spec) in layers.iter().enumerate() {
        cancel.check_cancelled()?;
        on_stage(StageUpdate {
            stage: Stage::Extract,
            fraction: Some(i as f64 / total as f64),
            detail: spec.layer.to_string(),
        });
        builder.add_vectors(&gpkg, std::slice::from_ref(*spec), cancel, |_, _| {})?;
    }

    // Huts first, and for every preset: a hut is where a hiker is walking to, not a
    // winter feature. The device test set caught this -- the hiking map had none.
    for path in crate::datasets::hut_geopackages(&ctx.cache_root) {
        cancel.check_cancelled()?;
        on_stage(StageUpdate {
            stage: Stage::Extract,
            fraction: None,
            detail: "SAC huts".into(),
        });
        let src = Gpkg::open(&path)?;
        let specs: Vec<_> = crate::extract::HUT_LAYERS
            .iter()
            .filter(|l| keep(l.layer))
            .collect();
        for spec in specs {
            builder.add_vectors(&src, std::slice::from_ref(spec), cancel, |_, _| {})?;
        }
    }

    if recipe.preset.needs_winter() {
        let sources = crate::datasets::winter_geopackages(&ctx.cache_root);
        for p in &sources {
            cancel.check_cancelled()?;
            on_stage(StageUpdate {
                stage: Stage::Extract,
                fraction: None,
                detail: format!(
                    "winter routes: {}",
                    p.file_name().unwrap_or_default().to_string_lossy()
                ),
            });
            let src = Gpkg::open(p)?;
            let specs: Vec<_> = WINTER_LAYERS.iter().filter(|l| keep(l.layer)).collect();
            for spec in specs {
                builder.add_vectors(&src, std::slice::from_ref(spec), cancel, |_, _| {})?;
            }
        }
        if sources.is_empty() {
            warnings.push(
                "winter route data is not downloaded, so the ski touring layers are missing".into(),
            );
        }
    }

    if recipe.preset.needs_cycle() {
        let sources = crate::datasets::route_shapefiles(&ctx.cache_root);
        let mut used = 0usize;
        for p in &sources {
            cancel.check_cancelled()?;
            let stem = p
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default();
            let Some(spec) = CYCLE_LAYERS.iter().find(|l| l.layer == stem) else {
                continue;
            };
            if !keep(&stem) {
                continue;
            }
            // All three ASTRA datasets ship a Route.shp, so the layer tag is qualified
            // by dataset: otherwise a hiking route renders as a cycle route.
            // wanderland's Route.shp is kept: the style draws it as a signposted
            // hiking route (0x10101), not as a cycle route.
            let dataset = crate::datasets::route_dataset_of(p).unwrap_or_default();
            let tag = if stem == "Route" {
                format!("{dataset}_{stem}")
            } else {
                stem.clone()
            };
            on_stage(StageUpdate {
                stage: Stage::Extract,
                fraction: None,
                detail: format!("routes: {tag}"),
            });
            let shp = Shapefile::open(p)?;
            builder.add_shapefile_as(&shp, spec, &tag, cancel)?;
            used += 1;
        }
        if used == 0 {
            warnings.push(
                "cycle route data is not downloaded, so the cycling layers are missing".into(),
            );
        }
    }

    // ---- elevation, contours and relief ----------------------------------
    let mut elevation: Option<crate::elevation::Grid> = None;

    // Official building addresses, so the device can search for one (SPEC.md §16 v2).
    // A national CSV scanned line by line and clipped to the area: 3.3 million rows in
    // about 1.5 seconds, at constant memory. Holding it would be 468 MB (NFR-2).
    if recipe.addresses {
        match crate::datasets::address_csv(&ctx.cache_root) {
            Some(csv) => {
                on_stage(StageUpdate {
                    stage: Stage::Extract,
                    fraction: None,
                    detail: "official addresses".into(),
                });
                let stats = builder.add_addresses(&csv, cancel, |n| {
                    on_stage(StageUpdate {
                        stage: Stage::Extract,
                        fraction: None,
                        detail: format!("{n} addresses"),
                    });
                })?;
                if stats.kept == 0 {
                    warnings.push(
                        "no official addresses fall inside this area, so address search \
                         will find nothing"
                            .into(),
                    );
                }
            }
            None => warnings.push(
                "the official address register is not downloaded, so addresses were \
                 left out"
                    .into(),
            ),
        }
    }

    // Glacier and firn outlines, for drawing contours blue over ice as the Landeskarte
    // does. Read while the connection is open, used later.
    let ice = if recipe.contours.interval_m > 0 {
        load_ice(&gpkg, bbox)
    } else {
        Vec::new()
    };
    drop(gpkg);

    if recipe.contours.interval_m > 0
        || recipe.relief.resolution().is_some()
        || recipe.slope_classes
    {
        cancel.check_cancelled()?;
        on_stage(StageUpdate {
            stage: Stage::Elevation,
            fraction: None,
            detail: "listing tiles".into(),
        });
        let (paths, fstats) = fetch_tiles(
            ctx.http,
            &ctx.cache_root,
            bbox,
            area_mask.map(|m| m.as_ref()),
            12,
            cancel,
            |s| {
                let done = s.already_cached + s.downloaded;
                on_stage(StageUpdate {
                    stage: Stage::Elevation,
                    fraction: (s.requested > 0).then(|| done as f64 / s.requested as f64),
                    detail: format!("{done} of {} tiles", s.requested),
                });
            },
        )
        .await?;
        if !fstats.missing.is_empty() {
            warnings.push(missing_tile_warning(&fstats.missing));
        }

        let grid = load_grid(&paths, bbox)?;

        if recipe.slope_classes {
            on_stage(StageUpdate {
                stage: Stage::Contours,
                fraction: None,
                detail: "slope classes over 30°".into(),
            });
            let (slopes, sstats) = crate::slope::areas(&grid, &Default::default());
            cancel.check_cancelled()?;
            *slope_areas = sstats.areas;
            builder.add_slope_areas(&slopes, cancel)?;
        }

        if recipe.contours.interval_m > 0 {
            on_stage(StageUpdate {
                stage: Stage::Contours,
                fraction: None,
                detail: format!("{} m interval", recipe.contours.interval_m),
            });
            let cfg = ContourConfig {
                interval_m: recipe.contours.interval_m,
                major_m: recipe.contours.index_m,
                medium_m: 0,
                simplify_m: recipe.contours.simplify_m,
            };
            let (contours, cstats) = generate(&grid, &cfg, || cancel.is_cancelled());
            cancel.check_cancelled()?;
            *contour_lines = cstats.lines;

            // Blue over ice, as the Landeskarte does.
            let ice = ice.clone();
            builder.add_contours(
                &contours,
                |p| ice.iter().any(|rings| point_in_polygon(p, rings)),
                cancel,
            )?;
        }

        elevation = Some(grid);
    }

    let stats = builder.finish()?;
    Ok((pbf, stats, elevation))
}

/// Assemble and write the build manifest.
#[allow(clippy::too_many_arguments)]
async fn write_manifest(
    ctx: &BuildContext<'_>,
    recipe: &Recipe,
    identity: &crate::garmin::MapIdentity,
    out: &crate::garmin::BuildOutput,
    stats: &crate::extract::ExtractStats,
    stage_seconds: &[(Stage, f64)],
    contour_lines: usize,
    slope_areas: usize,
    warnings: &[String],
    info: &img::ImgInfo,
) -> Result<PathBuf> {
    use crate::manifest::{Manifest, Output, Tools};

    let entries = crate::cache::Cache::new(ctx.cache_root.clone())
        .list()
        .await
        .unwrap_or_default();

    let name_of = |p: &Path| {
        p.file_name()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_default()
    };
    // Ask each tool for its own version rather than reading the jar's file name: the
    // vendored jars are called plain `mkgmap.jar`, so the file name records nothing.
    // Falling back to the path at least says which file was used.
    let tools = Tools {
        app: env!("CARGO_PKG_VERSION").to_string(),
        mkgmap: ctx
            .toolchain
            .mkgmap_version()
            .unwrap_or_else(|| name_of(&ctx.toolchain.mkgmap_jar)),
        splitter: ctx
            .toolchain
            .splitter_version()
            .unwrap_or_else(|| name_of(&ctx.toolchain.splitter_jar)),
        java: ctx
            .toolchain
            .java_version()
            .unwrap_or_else(|| ctx.toolchain.java.display().to_string()),
    };

    let sources = crate::manifest::sources_from_cache(&entries);

    let manifest = Manifest {
        schema_version: 1,
        built_at: crate::clock::now_rfc3339(),
        recipe_key: recipe.cache_key(),
        recipe: recipe.clone(),
        sources,
        tools,
        output: Output {
            file: name_of(&out.gmapsupp),
            bytes: out.bytes,
            sha256: crate::cache::sha256_of(&out.gmapsupp)
                .map_err(|e| Error::io(&out.gmapsupp, e))?,
            tile_count: out.tile_count,
            family_id: identity.family_id,
            has_dem: info.has_dem(),
        },
        stage_seconds: stage_seconds
            .iter()
            .map(|(s, secs)| (format!("{s:?}").to_lowercase(), *secs))
            .collect(),
        features_per_layer: stats.per_layer.iter().cloned().collect(),
        contour_lines,
        slope_areas,
        warnings: warnings.to_vec(),
        // The string mkgmap actually embedded, not a copy of it: FR-L1 is about what
        // travels with the file, so the manifest must record that and not a hopeful
        // restatement.
        attribution: identity.description.clone(),
    };
    manifest.write_beside(&out.gmapsupp)
}

/// Open a cached GeoPackage, quarantining it if it turns out not to be one.
///
/// SQLite opens lazily, so a truncated or half-written file opens cleanly and fails
/// several stages later with a message about a missing table — which reads as a bug in
/// this program rather than as damaged data. Reading `gpkg_contents` immediately is a
/// single indexed query and settles it.
///
/// A file that fails is moved aside rather than deleted (FR-C5): it can be inspected,
/// it is never silently reused, and the Data screen shows the dataset as absent so it
/// can be downloaded again. Quarantining was implemented before this and never called
/// from anywhere — the mechanism existed and the corruption path did not reach it.
async fn open_verified(path: &Path, cache_root: &Path) -> Result<Gpkg> {
    let corruption = match Gpkg::open(path) {
        Ok(gpkg) => match gpkg.layers() {
            Ok(layers) if !layers.is_empty() => return Ok(gpkg),
            Ok(_) => "the file contains no layers".to_string(),
            Err(e) => e.to_string(),
        },
        Err(e) => e.to_string(),
    };

    let cache = crate::cache::Cache::new(cache_root);
    let moved = cache.quarantine_containing(path).await;
    let where_now = match moved {
        Ok(Some(dst)) => format!(
            " It has been moved to {} so it cannot be reused.",
            dst.display()
        ),
        // Outside the cache, or the move failed: either way, say nothing that is untrue
        // about what happened to the user's file.
        _ => String::new(),
    };
    Err(Error::NotFound(format!(
        "the swissTLM3D data at {} is damaged ({corruption}).{where_now} Download it \
         again on the Data screen.",
        path.display()
    )))
}

/// Name the elevation cells that were unavailable (SPEC.md §12).
///
/// The count alone left the user with nothing to check: swissALTI3D is published per
/// square kilometre and cells do occasionally 404, so naming them is what lets somebody
/// look the tile up on the portal or decide the gap is somewhere they do not care about.
///
/// Capped, because a whole-canton outage would otherwise produce a warning thousands of
/// identifiers long. Cells are sorted so the list is the same on every run.
fn missing_tile_warning(missing: &[crate::elevation::Cell]) -> String {
    const NAMED: usize = 12;
    let mut cells: Vec<_> = missing.to_vec();
    cells.sort();
    let named: Vec<String> = cells.iter().take(NAMED).map(|c| c.label()).collect();
    let rest = cells.len().saturating_sub(named.len());
    let tail = if rest > 0 {
        format!(" and {rest} more")
    } else {
        String::new()
    };
    format!(
        "{} elevation tile(s) unavailable, so contours have gaps there: {}{tail}",
        cells.len(),
        named.join(", ")
    )
}

/// Rough disk a build needs, for the precheck.
///
/// Dominated by cached elevation tiles: swissALTI3D is a 1 km grid at about 1.2 MB per
/// tile, and every square kilometre of terrain not already cached fetches one. The
/// intermediates and the output are small beside that, and a flat allowance covers them.
/// Deliberately an over-estimate: refusing a build that would just have fitted is a far
/// better failure than filling the disk.
fn required_bytes(bbox: &BBox, recipe: &Recipe, mask: Option<&crate::mask::Mask>) -> u64 {
    const TILE_BYTES: f64 = 1_200_000.0;
    const OVERHEAD_BYTES: u64 = 512 * 1024 * 1024;
    let needs_elevation = recipe.contours.interval_m > 0 || recipe.relief.resolution().is_some();
    if !needs_elevation {
        return OVERHEAD_BYTES;
    }
    // With a mask only the tiles the shape reaches are fetched, and for a canton that
    // is under half the bounding box. Counting the box would refuse builds that fit.
    let tiles = match mask {
        Some(m) => crate::elevation::Cell::covering_mask(bbox, m).len() as f64,
        None => bbox.area_km2(),
    };
    (tiles * TILE_BYTES) as u64 + OVERHEAD_BYTES
}

/// Style directory for a device: wrist devices get the reduced cartography (FR-CART6).
fn style_for(profile: &DeviceProfile, root: &Path) -> PathBuf {
    if profile.is_wrist() {
        root.join("swisstopo-wrist")
    } else {
        root.join("swisstopo")
    }
}

/// TYP for a device and colour scheme.
///
/// The winter variants recolour the same rules, so only the TYP changes: the style
/// directory is chosen by device class alone.
fn typ_for(profile: &DeviceProfile, root: &Path, palette: crate::recipe::Palette) -> PathBuf {
    let name = match (profile.is_wrist(), palette) {
        (false, crate::recipe::Palette::Summer) => "swisstopo.txt",
        (true, crate::recipe::Palette::Summer) => "swisstopo-wrist.txt",
        (false, crate::recipe::Palette::Winter) => "swisstopo-winter.txt",
        (true, crate::recipe::Palette::Winter) => "swisstopo-wrist-winter.txt",
    };
    root.join(name)
}

/// Locate the cached national swissTLM3D GeoPackage.
pub fn find_tlm3d(cache_root: &Path) -> Option<PathBuf> {
    let root = cache_root.join(crate::stac::TLM3D);
    std::fs::read_dir(root).ok()?.flatten().find_map(|e| {
        std::fs::read_dir(e.path()).ok()?.flatten().find_map(|f| {
            let p = f.path();
            (p.extension()? == "gpkg").then_some(p)
        })
    })
}

/// Run a full build.
pub async fn build(
    ctx: &BuildContext<'_>,
    recipe: &Recipe,
    profile: &DeviceProfile,
    cancel: &Cancel,
    mut report_stage: impl FnMut(StageUpdate),
) -> Result<BuildReport> {
    // Time every stage by watching the updates that already flow through here, rather
    // than instrumenting each of the ten call sites. Mutex rather than RefCell because
    // this future is spawned on a multi-threaded runtime and must stay Send.
    let timings: std::sync::Mutex<Vec<(Stage, f64)>> = Default::default();
    let current: std::sync::Mutex<Option<(Stage, std::time::Instant)>> = Default::default();
    let mut on_stage = |u: StageUpdate| {
        {
            let mut cur = current.lock().expect("stage clock poisoned");
            match *cur {
                Some((stage, at)) if stage != u.stage => {
                    timings
                        .lock()
                        .expect("stage timings poisoned")
                        .push((stage, at.elapsed().as_secs_f64()));
                    *cur = Some((u.stage, std::time::Instant::now()));
                }
                None => *cur = Some((u.stage, std::time::Instant::now())),
                _ => {}
            }
        }
        report_stage(u);
    };
    let bbox = recipe.area.bbox();
    if !bbox.within_switzerland() {
        return Err(Error::NotFound(
            "the selected area is outside the swissTLM3D coverage".into(),
        ));
    }
    std::fs::create_dir_all(&ctx.work_dir).map_err(|e| Error::io(&ctx.work_dir, e))?;
    let mut warnings = Vec::new();

    // Built once and shared three ways: the extractor filters features with it, the
    // elevation fetch skips tiles the shape does not reach, and the space check counts
    // only the tiles that will actually be fetched.
    let area_mask = recipe.area.mask(&ctx.cache_root)?.map(std::sync::Arc::new);

    // A build writes far more than its output: a region PBF, split tiles, DEM data, and
    // the elevation tiles it caches along the way -- roughly 1.2 MB per square kilometre
    // of new terrain. Running out halfway through wastes the whole build and can leave
    // the machine with no room to recover, so check before starting rather than during.
    if let Some(free) = crate::cache::available_bytes(&ctx.cache_root) {
        let need = required_bytes(&bbox, recipe, area_mask.as_deref());
        if free < need {
            return Err(Error::InsufficientSpace {
                path: ctx.cache_root.clone(),
                need,
                available: free,
            });
        }
    }

    let gpkg_path = find_tlm3d(&ctx.cache_root).ok_or_else(|| {
        Error::NotFound("swissTLM3D is not downloaded; fetch it on the Data screen".into())
    })?;
    let gpkg = open_verified(&gpkg_path, &ctx.cache_root).await?;

    let mut contour_lines = 0usize;
    let mut slope_areas = 0usize;
    let mut dem_dir: Option<PathBuf> = None;
    let mut elevation: Option<crate::elevation::Grid> = None;

    // ---- reuse, if this exact region has been built before (FR-72) --------
    //
    // Extract, elevation and contours are about 80% of a build and depend on none of
    // the device, the colour scheme or the TYP, so a cartography change used to redo
    // all three for a byte-identical result.
    let source_release = gpkg_path
        .parent()
        .and_then(|d| d.file_name())
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    let region_cache = crate::stage_cache::RegionCache::new(&ctx.cache_root);
    let region_key = crate::stage_cache::region_key(recipe, &source_release);

    // Mark the work directory as belonging to a running build, so that if this process
    // dies before the guard drops -- crash, force quit, power loss -- the next start can
    // find the abandoned files and any Java children still running (SPEC.md §12).
    // Dropped on every exit path, including a panic in a stage.
    let _active = crate::recovery::ActiveBuild::begin(
        &ctx.work_dir,
        recipe,
        &region_key,
        &crate::clock::now_rfc3339(),
    )?;

    let (pbf, stats, cached) = match region_cache.get(&region_key) {
        Some((path, cached_stats)) => {
            on_stage(StageUpdate {
                stage: Stage::Extract,
                fraction: Some(1.0),
                detail: "reusing the region from an earlier build".into(),
            });
            contour_lines = cached_stats.contour_lines;
            slope_areas = cached_stats.slope_areas;
            let stats = crate::extract::ExtractStats {
                features: cached_stats.features,
                nodes: cached_stats.nodes,
                ways: cached_stats.ways,
                per_layer: cached_stats.per_layer.clone(),
                ..Default::default()
            };
            (path, stats, true)
        }
        None => {
            let (pbf, stats, grid) = build_region(
                ctx,
                recipe,
                &bbox,
                &gpkg_path,
                area_mask.as_ref(),
                cancel,
                &mut on_stage,
                &mut warnings,
                &mut contour_lines,
                &mut slope_areas,
            )
            .await?;
            elevation = grid;
            let stored = region_cache.put(
                &region_key,
                &pbf,
                &crate::stage_cache::RegionStats {
                    features: stats.features,
                    nodes: stats.nodes,
                    ways: stats.ways,
                    contour_lines,
                    slope_areas,
                    per_layer: stats.per_layer.clone(),
                    source_release: source_release.clone(),
                },
            )?;
            (stored, stats, false)
        }
    };
    cancel.check_cancelled()?;

    // ---- relief ----------------------------------------------------------
    //
    // Outside the cached region on purpose: the relief setting does not change the
    // region, so it must not be decided by whether the region was reused. On a cache
    // hit the grid is loaded here instead, from elevation tiles that are already on
    // disk.
    if let Some(res) = recipe.relief.resolution() {
        on_stage(StageUpdate {
            stage: Stage::Relief,
            fraction: None,
            detail: if cached {
                "resampling elevation (region reused)"
            } else {
                "resampling elevation"
            }
            .into(),
        });
        let grid = match elevation.take() {
            Some(g) => Some(g),
            None => {
                let (paths, _) = fetch_tiles(
                    ctx.http,
                    &ctx.cache_root,
                    &bbox,
                    area_mask.as_deref(),
                    12,
                    cancel,
                    |_| {},
                )
                .await?;
                Some(load_grid(&paths, &bbox)?)
            }
        };
        if let Some(grid) = grid {
            let dir = ctx.work_dir.join("dem");
            let (_, dstats) = dem::write_hgt(&grid, &bbox, res, &dir, |_| {})?;
            if dstats.samples_filled == 0 {
                warnings.push("no elevation data covered the area, so relief was skipped".into());
            } else {
                dem_dir = Some(dir);
            }
        }
    }

    // ---- split -----------------------------------------------------------
    on_stage(StageUpdate {
        stage: Stage::Split,
        fraction: None,
        detail: format!("{} nodes", stats.nodes),
    });
    let identity = MapIdentity::for_recipe(&recipe.cache_key(), &recipe.name);
    let tiles_dir = ctx.work_dir.join("tiles");
    // Splitting is retried with a larger --max-nodes when it produces more tiles than
    // the device accepts: fewer, denser tiles for the same map (SPEC.md §12). Only when
    // the ceiling is reached is the area genuinely too big for one map set, and then the
    // message says the thing that actually helps -- partition it (FR-36).
    let max_tiles = profile.map_file.max_tiles_per_mapset;
    let mut max_nodes = crate::garmin::START_MAX_NODES;
    let tiles = loop {
        let _ = std::fs::remove_dir_all(&tiles_dir);
        let tiles = split(
            &ctx.toolchain,
            &pbf,
            &tiles_dir,
            &identity,
            max_nodes,
            4096,
            cancel,
        )?;
        match crate::garmin::retune_max_nodes(max_nodes, tiles.len(), max_tiles) {
            Some(next) => {
                warnings.push(format!(
                    "{} tiles exceeded the {max_tiles} this device accepts, so tiles were \
                     packed more densely (--max-nodes {next})",
                    tiles.len()
                ));
                on_stage(StageUpdate {
                    stage: Stage::Split,
                    fraction: None,
                    detail: format!("retrying with denser tiles ({next} nodes)"),
                });
                max_nodes = next;
            }
            None if tiles.len() > max_tiles => {
                return Err(Error::Zip(format!(
                    "{} tiles exceeds the {max_tiles} this device accepts, even with the \
                     densest tiles the map compiler supports. Split the area into several \
                     map sets, or choose a smaller area.",
                    tiles.len()
                )));
            }
            None => break tiles,
        }
    };

    // ---- compile ---------------------------------------------------------
    cancel.check_cancelled()?;
    on_stage(StageUpdate {
        stage: Stage::Compile,
        fraction: None,
        detail: format!("{} tile(s)", tiles.len()),
    });
    let style_dir = style_for(profile, &ctx.style_root);
    let mut opts = BuildOptions::new(
        identity.clone(),
        style_dir.clone(),
        typ_for(profile, &ctx.typ_root, recipe.palette),
    );
    // Routing, when the recipe asks and the device can use it. A profile that records
    // `supportsRoutableMaps: false` gets a warning rather than a silently bigger map
    // that its device cannot navigate with.
    // Address search needs the housenumber matching, and there is no point paying for
    // it when no addresses were extracted.
    opts.housenumbers = recipe.addresses
        && stats
            .per_layer
            .iter()
            .any(|(l, n)| l == "addresses" && *n > 0);

    if recipe.routing {
        if profile.rendering.supports_routable_maps {
            opts.routing = true;
        } else {
            warnings.push(format!(
                "{} does not support routable maps, so routing was left out",
                profile.display_name
            ));
        }
    }

    if let (Some(dir), Some(res)) = (dem_dir.clone(), recipe.relief.resolution()) {
        let levels = style_level_count(&style_dir)?;
        opts.dem_dists = dem::dem_dists(res, levels);
        opts.dem_dir = Some(dir);
    }
    let img_dir = ctx.work_dir.join("img");
    let _ = std::fs::remove_dir_all(&img_dir);
    let out = compile(&ctx.toolchain, &tiles, &img_dir, &opts, cancel)?;

    // ---- verify ----------------------------------------------------------
    on_stage(StageUpdate {
        stage: Stage::Verify,
        fraction: None,
        detail: "checking the container".into(),
    });
    let info = img::read(&out.gmapsupp)?;
    let verdict = img::verify_gmapsupp(&info, dem_dir.is_some());
    if !verdict.ok() {
        return Err(Error::Zip(format!(
            "the built map failed verification: {}",
            verdict.problems.join("; ")
        )));
    }
    warnings.extend(verdict.warnings);

    let device = img::verify_for_device(
        &info,
        out.tile_count,
        profile.effective_max_img_bytes(),
        profile.map_file.max_tiles_per_mapset,
    );
    if !device.ok() {
        return Err(Error::Zip(device.problems.join("; ")));
    }

    // ---- raster overlay (FR-R1, FR-R5) -----------------------------------
    // A separate KMZ, not part of the .img. Runs after verification because the vector
    // map is the deliverable: a raster failure downgrades to a warning rather than
    // failing a build that has already produced a correct map. The user gets told what
    // they did not get, which is the honest outcome (working agreement rule 8).
    let mut raster_report = None;
    if recipe.raster {
        match profile.raster_limits() {
            None => warnings.push(format!(
                "{} has no known Custom Map limits, so the paper-map overlay was skipped",
                profile.display_name
            )),
            Some(limits) => {
                on_stage(StageUpdate {
                    stage: Stage::Raster,
                    fraction: Some(0.0),
                    detail: "planning tiles".into(),
                });
                let kmz = img_dir.join(crate::raster::kmz_filename(&recipe.name));
                // The mask, not just the bounding box: for a corridor or an
                // administrative unit the box is mostly ground the user never looks at,
                // and tiles are the scarce resource here.
                match crate::raster::plan_masked(
                    &bbox,
                    &limits,
                    crate::raster::NATIVE_M_PER_PX,
                    crate::raster::DEFAULT_LAYER,
                    area_mask.as_deref(),
                ) {
                    Err(e) => warnings.push(format!("could not plan the paper-map overlay: {e}")),
                    Ok(plan) => {
                        // The planner's notes are the user's business: they say why the
                        // resolution is what it is, including when it is too coarse to
                        // be worth having.
                        warnings.extend(plan.notes.iter().cloned());
                        let total = plan.tile_count();
                        let res = crate::raster::build_kmz(
                            ctx.http,
                            &plan,
                            &recipe.name,
                            &kmz,
                            cancel,
                            |done, _| {
                                on_stage(StageUpdate {
                                    stage: Stage::Raster,
                                    fraction: Some(done as f64 / total.max(1) as f64),
                                    detail: format!("tile {done} of {total}"),
                                });
                            },
                        )
                        .await;
                        match res {
                            // A cancel is the user's decision and must stop the build,
                            // not be filed as a warning about the overlay.
                            Err(Error::Cancelled) => return Err(Error::Cancelled),
                            Err(e) => {
                                warnings.push(format!("could not build the paper-map overlay: {e}"))
                            }
                            Ok(r) => {
                                warnings.extend(r.warnings.iter().cloned());
                                raster_report = Some(r);
                            }
                        }
                    }
                }
            }
        }
    }

    // The last stage is still open; close it so verify is not always reported as zero.
    if let Some((stage, at)) = current.lock().expect("stage clock poisoned").take() {
        timings
            .lock()
            .expect("stage timings poisoned")
            .push((stage, at.elapsed().as_secs_f64()));
    }
    let stage_seconds = timings.into_inner().expect("stage timings poisoned");

    // ---- record for the size estimator (FR-62) ---------------------------
    // The predictors are recomputed here, the same way the estimator computes them
    // before a build, so training features and prediction features are identical.
    if let Some(log) = &ctx.calibration_log {
        let sample = estimate::Sample {
            predictors: estimate::Predictors {
                group_counts: estimate::count_groups(&gpkg, recipe, Some(&ctx.cache_root)),
                area_km2: bbox.area_km2(),
                contour_interval_m: recipe.contours.interval_m,
                relief: recipe.relief,
                slope_classes: recipe.slope_classes,
                routing: opts.routing,
                addresses: opts.housenumbers,
                // Which cartography was compiled, so the sample records the thing that
                // changes its size by a quarter.
                wrist: profile.is_wrist(),
            },
            actual_bytes: out.bytes,
            stage_seconds: stage_seconds
                .iter()
                .map(|(s, secs)| (format!("{s:?}").to_lowercase(), *secs))
                .collect(),
            at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
        };
        // A calibration failure must never fail a finished build.
        if let Err(e) = estimate::CalibrationLog::append(log, &sample) {
            warnings.push(format!("could not record size calibration: {e}"));
        }
    }

    let cache_has_provenance = crate::cache::Cache::new(ctx.cache_root.clone())
        .list()
        .await
        .map(|e| e.iter().any(|x| x.provenance.is_some()))
        .unwrap_or(false);

    // ---- manifest (FR-71) ------------------------------------------------
    // Written beside the output, so a map on a device can be traced back to the exact
    // releases and tools that made it. A manifest failure must not fail a finished
    // build: the map is already correct.
    let manifest_path = match write_manifest(
        ctx,
        recipe,
        &identity,
        &out,
        &stats,
        &stage_seconds,
        contour_lines,
        slope_areas,
        &warnings,
        &info,
    )
    .await
    {
        Ok(p) => Some(p),
        Err(e) => {
            warnings.push(format!("could not write the build manifest: {e}"));
            None
        }
    };
    // A manifest whose sources are empty cannot trace the map back to anything, which
    // is most of its purpose. That happens when datasets were put in the cache by
    // something other than this app, so no provenance was recorded beside them.
    if manifest_path.is_some() && !cache_has_provenance {
        warnings.push(
            "the datasets in the cache carry no provenance, so the build manifest \
             cannot record which releases this map came from"
                .into(),
        );
    }

    Ok(BuildReport {
        manifest: manifest_path,
        raster: raster_report,
        gmapsupp: out.gmapsupp,
        bytes: out.bytes,
        tile_count: out.tile_count,
        features: stats.features,
        nodes: stats.nodes,
        ways: stats.ways,
        contour_lines,
        slope_areas,
        has_dem: info.has_dem(),
        warnings,
        family_id: identity.family_id,
        stage_seconds,
    })
}

fn load_ice(gpkg: &Gpkg, bbox: &BBox) -> Vec<Vec<Vec<crate::geom::Coord>>> {
    const ICE: [&str; 2] = ["Gletscher", "Schneefeld Toteis"];
    let mut polys = Vec::new();
    let _ = gpkg.for_each_in_bbox("tlm_bb_bodenbedeckung", bbox, &["objektart"], |f| {
        if f.attr("objektart")
            .map(|v| ICE.contains(&v))
            .unwrap_or(false)
        {
            match f.geometry {
                crate::geom::Geometry::Polygon(rings) => polys.push(rings),
                crate::geom::Geometry::MultiPolygon(ps) => polys.extend(ps),
                _ => {}
            }
        }
        true
    });
    polys
}

#[cfg(test)]
mod missing_tile_tests {
    use super::*;
    use crate::elevation::Cell;

    /// SPEC.md §12: "swissALTI3D tile missing — skip with a warning; contours for that
    /// cell are absent, and the build report says which cells."
    ///
    /// The count alone was not enough: it told the user something was wrong and gave
    /// them nothing to check.
    #[test]
    fn the_warning_names_the_cells_swisstopo_names() {
        let w = missing_tile_warning(&[
            Cell {
                e_km: 2646,
                n_km: 1163,
            },
            Cell {
                e_km: 2645,
                n_km: 1163,
            },
        ]);
        assert!(w.contains("2 elevation tile"), "{w}");
        // Sorted, so two runs of the same build produce the same warning.
        assert!(w.contains("2645-1163, 2646-1163"), "{w}");
        assert!(!w.contains("more"), "nothing was elided: {w}");
    }

    /// A regional outage must not produce a warning thousands of identifiers long.
    #[test]
    fn a_long_list_is_capped_but_still_reports_the_true_count() {
        let many: Vec<Cell> = (0..500)
            .map(|i| Cell {
                e_km: 2600 + i,
                n_km: 1100,
            })
            .collect();
        let w = missing_tile_warning(&many);
        assert!(w.contains("500 elevation tile"), "{w}");
        assert!(w.contains("and 488 more"), "{w}");
        assert!(w.len() < 300, "the warning is {} chars: {w}", w.len());
    }
}
