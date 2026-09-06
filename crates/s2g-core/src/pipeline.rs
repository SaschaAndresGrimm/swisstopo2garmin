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
    pub has_dem: bool,
    pub warnings: Vec<String>,
    /// Deterministic identity, so the same recipe keeps its place on the device.
    pub family_id: u16,
    /// Seconds spent in each stage, in the order they ran. Feeds the time estimate
    /// shown during the next build (FR-71).
    pub stage_seconds: Vec<(Stage, f64)>,
}

/// Rough disk a build needs, for the precheck.
///
/// Dominated by cached elevation tiles: swissALTI3D is a 1 km grid at about 1.2 MB per
/// tile, and every square kilometre of terrain not already cached fetches one. The
/// intermediates and the output are small beside that, and a flat allowance covers them.
/// Deliberately an over-estimate: refusing a build that would just have fitted is a far
/// better failure than filling the disk.
fn required_bytes(bbox: &BBox, recipe: &Recipe) -> u64 {
    const TILE_BYTES: f64 = 1_200_000.0;
    const OVERHEAD_BYTES: u64 = 512 * 1024 * 1024;
    let needs_elevation =
        recipe.contours.interval_m > 0 || recipe.relief.resolution().is_some();
    let tiles = if needs_elevation { bbox.area_km2() } else { 0.0 };
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

    // A build writes far more than its output: a region PBF, split tiles, DEM data, and
    // the elevation tiles it caches along the way -- roughly 1.2 MB per square kilometre
    // of new terrain. Running out halfway through wastes the whole build and can leave
    // the machine with no room to recover, so check before starting rather than during.
    if let Some(free) = crate::cache::available_bytes(&ctx.cache_root) {
        let need = required_bytes(&bbox, recipe);
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
    let gpkg = Gpkg::open(&gpkg_path)?;

    // ---- extract vectors -------------------------------------------------
    let pbf = ctx.work_dir.join("region.osm.pbf");
    let mut builder = RegionBuilder::create(&pbf, &bbox)?;
    // A corridor or administrative unit is not a rectangle: the bbox is the cheap
    // first cut and the mask decides what actually survives it.
    if let Some(mask) = recipe.area.mask(&ctx.cache_root)? {
        builder = builder.with_mask(mask);
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
    let mut contour_lines = 0usize;
    let mut dem_dir: Option<PathBuf> = None;

    if recipe.contours.interval_m > 0 || recipe.relief.resolution().is_some() {
        cancel.check_cancelled()?;
        on_stage(StageUpdate {
            stage: Stage::Elevation,
            fraction: None,
            detail: "listing tiles".into(),
        });
        let (paths, fstats) = fetch_tiles(ctx.http, &ctx.cache_root, &bbox, 12, cancel, |s| {
            let done = s.already_cached + s.downloaded;
            on_stage(StageUpdate {
                stage: Stage::Elevation,
                fraction: (s.requested > 0).then(|| done as f64 / s.requested as f64),
                detail: format!("{done} of {} tiles", s.requested),
            });
        })
        .await?;
        if !fstats.missing.is_empty() {
            warnings.push(format!(
                "{} elevation tile(s) unavailable, so contours have gaps there",
                fstats.missing.len()
            ));
        }

        let grid = load_grid(&paths, &bbox)?;

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
            contour_lines = cstats.lines;

            // Blue over ice, as the Landeskarte does.
            let ice = load_ice(&gpkg, &bbox);
            builder.add_contours(
                &contours,
                |p| ice.iter().any(|rings| point_in_polygon(p, rings)),
                cancel,
            )?;
        }

        if let Some(res) = recipe.relief.resolution() {
            on_stage(StageUpdate {
                stage: Stage::Relief,
                fraction: None,
                detail: "resampling elevation".into(),
            });
            let dir = ctx.work_dir.join("dem");
            let (_, dstats) = dem::write_hgt(&grid, &bbox, res, &dir, |_| {})?;
            if dstats.samples_filled == 0 {
                warnings.push("no elevation data covered the area, so relief was skipped".into());
            } else {
                dem_dir = Some(dir);
            }
        }
    }

    let stats = builder.finish()?;
    cancel.check_cancelled()?;

    // ---- split -----------------------------------------------------------
    on_stage(StageUpdate {
        stage: Stage::Split,
        fraction: None,
        detail: format!("{} nodes", stats.nodes),
    });
    let identity = MapIdentity::for_recipe(&recipe.cache_key(), &recipe.name);
    let tiles_dir = ctx.work_dir.join("tiles");
    let _ = std::fs::remove_dir_all(&tiles_dir);
    let tiles = split(&ctx.toolchain, &pbf, &tiles_dir, &identity, 700_000, 4096, cancel)?;
    if tiles.len() > profile.map_file.max_tiles_per_mapset {
        return Err(Error::Zip(format!(
            "{} tiles exceeds the {} this device accepts; choose a smaller area",
            tiles.len(),
            profile.map_file.max_tiles_per_mapset
        )));
    }

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

    Ok(BuildReport {
        gmapsupp: out.gmapsupp,
        bytes: out.bytes,
        tile_count: out.tile_count,
        features: stats.features,
        nodes: stats.nodes,
        ways: stats.ways,
        contour_lines,
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
