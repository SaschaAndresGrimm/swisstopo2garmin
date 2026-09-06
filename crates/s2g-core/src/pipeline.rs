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
}

/// Style directory for a device: wrist devices get the reduced cartography (FR-CART6).
fn style_for(profile: &DeviceProfile, root: &Path) -> PathBuf {
    if profile.is_wrist() {
        root.join("swisstopo-wrist")
    } else {
        root.join("swisstopo")
    }
}

fn typ_for(profile: &DeviceProfile, root: &Path) -> PathBuf {
    if profile.is_wrist() {
        root.join("swisstopo-wrist.txt")
    } else {
        root.join("swisstopo.txt")
    }
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
    mut on_stage: impl FnMut(StageUpdate),
) -> Result<BuildReport> {
    let bbox = recipe.area.bbox();
    if !bbox.within_switzerland() {
        return Err(Error::NotFound(
            "the selected area is outside the swissTLM3D coverage".into(),
        ));
    }
    std::fs::create_dir_all(&ctx.work_dir).map_err(|e| Error::io(&ctx.work_dir, e))?;
    let mut warnings = Vec::new();

    let gpkg_path = find_tlm3d(&ctx.cache_root).ok_or_else(|| {
        Error::NotFound("swissTLM3D is not downloaded; fetch it on the Data screen".into())
    })?;
    let gpkg = Gpkg::open(&gpkg_path)?;

    // ---- extract vectors -------------------------------------------------
    let pbf = ctx.work_dir.join("region.osm.pbf");
    let mut builder = RegionBuilder::create(&pbf, &bbox)?;
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
        let dir = ctx.cache_root.join("winter");
        let mut found = 0usize;
        if let Ok(rd) = std::fs::read_dir(&dir) {
            for e in rd.flatten() {
                let p = e.path();
                if p.extension().map(|x| x != "gpkg").unwrap_or(true) {
                    continue;
                }
                on_stage(StageUpdate {
                    stage: Stage::Extract,
                    fraction: None,
                    detail: format!(
                        "winter routes: {}",
                        p.file_name().unwrap().to_string_lossy()
                    ),
                });
                let src = Gpkg::open(&p)?;
                let specs: Vec<_> = WINTER_LAYERS.iter().filter(|l| keep(l.layer)).collect();
                for spec in specs {
                    builder.add_vectors(&src, std::slice::from_ref(spec), cancel, |_, _| {})?;
                }
                found += 1;
            }
        }
        if found == 0 {
            warnings.push(
                "winter route data is not downloaded, so the ski touring layers are missing".into(),
            );
        }
    }

    if recipe.preset.needs_cycle() {
        let mut stack = vec![ctx.cache_root.join("routes")];
        let mut found = 0usize;
        while let Some(dir) = stack.pop() {
            let Ok(rd) = std::fs::read_dir(&dir) else {
                continue;
            };
            for e in rd.flatten() {
                let p = e.path();
                if p.is_dir() {
                    stack.push(p);
                    continue;
                }
                if p.extension().map(|x| x != "shp").unwrap_or(true) {
                    continue;
                }
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
                // All three datasets ship a Route.shp, so qualify by dataset.
                let dataset = p
                    .parent()
                    .and_then(|d| d.parent())
                    .and_then(|d| d.file_name())
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_default();
                let tag = if stem == "Route" {
                    format!("{dataset}_Route")
                } else {
                    stem.clone()
                };
                on_stage(StageUpdate {
                    stage: Stage::Extract,
                    fraction: None,
                    detail: format!("routes: {tag}"),
                });
                let shp = Shapefile::open(&p)?;
                builder.add_shapefile_as(&shp, spec, &tag, cancel)?;
                found += 1;
            }
        }
        if found == 0 {
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
    let tiles = split(&ctx.toolchain, &pbf, &tiles_dir, &identity, 700_000, 4096)?;
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
        typ_for(profile, &ctx.typ_root),
    );
    if let (Some(dir), Some(res)) = (dem_dir.clone(), recipe.relief.resolution()) {
        let levels = style_level_count(&style_dir)?;
        opts.dem_dists = dem::dem_dists(res, levels);
        opts.dem_dir = Some(dir);
    }
    let img_dir = ctx.work_dir.join("img");
    let _ = std::fs::remove_dir_all(&img_dir);
    let out = compile(&ctx.toolchain, &tiles, &img_dir, &opts)?;

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
