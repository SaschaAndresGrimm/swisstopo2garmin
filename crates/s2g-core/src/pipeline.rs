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
    /// Slope-class areas drawn, when the recipe asked for them.
    pub slope_areas: usize,
    pub has_dem: bool,
    pub warnings: Vec<String>,
    /// The manifest written beside the output (FR-71), when it could be written.
    pub manifest: Option<PathBuf>,
    /// Deterministic identity, so the same recipe keeps its place on the device.
    pub family_id: u16,
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
) -> Result<(PathBuf, crate::extract::ExtractStats, Option<crate::elevation::Grid>)> {
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
                "swissNAMES3D is not downloaded, so labels stay in the local language"
                    .into(),
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
            warnings.push(format!(
                "{} elevation tile(s) unavailable, so contours have gaps there",
                fstats.missing.len()
            ));
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
        built_at: now_rfc3339(),
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

/// RFC 3339 in UTC, without pulling in a date library for one timestamp.
fn now_rfc3339() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    // Days from the civil epoch, by Howard Hinnant's algorithm.
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        (rem % 3600) / 60,
        rem % 60
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
    let needs_elevation =
        recipe.contours.interval_m > 0 || recipe.relief.resolution().is_some();
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
    let gpkg = Gpkg::open(&gpkg_path)?;

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
                let (paths, _) =
                    fetch_tiles(ctx.http, &ctx.cache_root, &bbox, area_mask.as_deref(), 12, cancel, |_| {})
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
                slope_classes: recipe.slope_classes,
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
