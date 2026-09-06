//! Run a full recipe build exactly as the GUI does, without a window.
//!
//!   cargo run --release -p s2g-core --example build_recipe -- \
//!       --place Grindelwald --radius-km 6 --device edge-840 --preset hiking
//!
//! The GUI's `start_build` command is a thin wrapper around `pipeline::build`, so this
//! exercises the same code path a user drives through the wizard. Kept as an example
//! rather than a test because it downloads elevation tiles and shells out to mkgmap.

use std::path::PathBuf;

use s2g_core::cache::Cache;
use s2g_core::devices;
use s2g_core::download::Cancel;
use s2g_core::estimate::calibration_log_path;
use s2g_core::garmin::Toolchain;
use s2g_core::gpkg::Gpkg;
use s2g_core::http::ReqwestHttp;
use s2g_core::pipeline::{self, BuildContext, Stage};
use s2g_core::recipe::{AreaSelection, Preset, Recipe, ReliefDetail};

fn arg(name: &str) -> Option<String> {
    let args: Vec<String> = std::env::args().collect();
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1).cloned())
}

fn root() -> PathBuf {
    std::env::var("S2G_ROOT").map(PathBuf::from).unwrap_or_else(|_| {
        std::env::current_dir()
            .expect("cwd")
            .ancestors()
            .find(|c| c.join("devices").is_dir() && c.join("style").is_dir())
            .expect("run from inside the repository")
            .to_path_buf()
    })
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = root();
    let device_id = arg("--device").unwrap_or_else(|| "edge-840".into());
    let preset = match arg("--preset").unwrap_or_else(|| "hiking".into()).as_str() {
        "cycling" => Preset::Cycling,
        "skimo" => Preset::Skimo,
        "full" => Preset::Full,
        _ => Preset::Hiking,
    };
    let relief = match arg("--relief").unwrap_or_else(|| "gentle".into()).as_str() {
        "off" => ReliefDetail::Off,
        "detailed" => ReliefDetail::Detailed,
        _ => ReliefDetail::Gentle,
    };
    let radius_km: f64 = arg("--radius-km").unwrap_or_else(|| "6".into()).parse()?;
    let place = arg("--place").unwrap_or_else(|| "Grindelwald".into());

    let cache_root = Cache::default_root();
    let gpkg_path = pipeline::find_tlm3d(&cache_root).ok_or("swissTLM3D not in the cache")?;

    // Either a corridor around an imported track, or a radius around a place.
    let gpkg = Gpkg::open(&gpkg_path)?;
    let area = match arg("--commune").or_else(|| arg("--canton")) {
        Some(name) => {
            let level = if arg("--canton").is_some() {
                s2g_core::boundaries::AdminLevel::Canton
            } else {
                s2g_core::boundaries::AdminLevel::Commune
            };
            let units = s2g_core::boundaries::list_units(&cache_root, level)?;
            let picked: Vec<_> = units
                .iter()
                .filter(|u| u.name.eq_ignore_ascii_case(&name))
                .collect();
            if picked.is_empty() {
                return Err(format!("no {} called {name:?}", level.id()).into());
            }
            let numbers: Vec<i64> = picked.iter().map(|u| u.number).collect();
            let buffer_km: f64 = arg("--buffer-km").unwrap_or_else(|| "0".into()).parse()?;
            let b = s2g_core::boundaries::extent(&cache_root, level, &numbers)?;
            let m = buffer_km * 1000.0;
            println!(
                "unit         : {} {} ({} km² per the dataset), buffer {buffer_km} km",
                picked[0].name,
                level.id(),
                picked.iter().map(|u| u.area_km2).sum::<f64>().round()
            );
            AreaSelection::AdminUnits {
                level,
                numbers,
                names: picked.iter().map(|u| u.name.clone()).collect(),
                buffer_km,
                min_e: b.min_e - m,
                min_n: b.min_n - m,
                max_e: b.max_e + m,
                max_n: b.max_n + m,
            }
        }
        None => match arg("--gpx") {
        Some(path) => {
            let gpx = s2g_core::gpx::Gpx::parse_file(std::path::Path::new(&path))?;
            let points: Vec<[f64; 2]> = gpx
                .tracks
                .iter()
                .flat_map(|t| t.points.iter())
                .map(|p| {
                    let (e, n) = s2g_core::proj::wgs84_to_lv95(p.lon, p.lat);
                    [e, n]
                })
                .collect();
            let buffer_km: f64 = arg("--buffer-km").unwrap_or_else(|| "3".into()).parse()?;
            println!(
                "track        : {} points, buffer {buffer_km} km",
                points.len()
            );
            // --as-bbox builds the corridor's bounding box instead, for comparison.
            if arg("--as-bbox").is_some() {
                let es: Vec<f64> = points.iter().map(|p| p[0]).collect();
                let ns: Vec<f64> = points.iter().map(|p| p[1]).collect();
                let m = buffer_km * 1000.0;
                AreaSelection::BBox {
                    min_e: es.iter().cloned().fold(f64::INFINITY, f64::min) - m,
                    min_n: ns.iter().cloned().fold(f64::INFINITY, f64::min) - m,
                    max_e: es.iter().cloned().fold(f64::NEG_INFINITY, f64::max) + m,
                    max_n: ns.iter().cloned().fold(f64::NEG_INFINITY, f64::max) + m,
                }
            } else {
                AreaSelection::Corridor {
                    name: gpx.tracks[0].name.clone().unwrap_or_else(|| "track".into()),
                    buffer_km,
                    points,
                }
            }
        }
        None => {
            let hit = gpkg
                .find_places(&place)?
                .into_iter()
                .next()
                .ok_or_else(|| format!("no place called {place:?}"))?;
            println!("place        : {} at {:.0} {:.0}", hit.name, hit.easting, hit.northing);
            AreaSelection::Place {
                name: hit.name.clone(),
                radius_km,
                easting: hit.easting,
                northing: hit.northing,
            }
        }
        },
    };

    // Comma-separated layer ids to leave out, as the layer panel would (FR-51).
    let excluded: Vec<String> = arg("--exclude")
        .map(|v| v.split(',').map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect())
        .unwrap_or_default();

    let palette = match arg("--palette").unwrap_or_else(|| "summer".into()).as_str() {
        "winter" => s2g_core::recipe::Palette::Winter,
        _ => s2g_core::recipe::Palette::Summer,
    };

    let label_language = match arg("--labels").unwrap_or_else(|| "local".into()).as_str() {
        "german" => s2g_core::names::LabelLanguage::German,
        "french" => s2g_core::names::LabelLanguage::French,
        "italian" => s2g_core::names::LabelLanguage::Italian,
        "romansh" => s2g_core::names::LabelLanguage::Romansh,
        _ => s2g_core::names::LabelLanguage::Local,
    };

    let recipe = Recipe {
        relief,
        label_language,
        palette,
        slope_classes: arg("--slope").is_some() || std::env::args().any(|a| a == "--slope"),
        excluded_layers: excluded,
        ..Recipe::new(format!("{place} {}", preset.id()), &device_id, area).with_preset(preset)
    };
    println!("recipe key   : {}", recipe.cache_key());

    let profiles = devices::load_profiles(&root.join("devices"))?;
    let profile = profiles
        .iter()
        .find(|p| p.id == recipe.device_id)
        .ok_or_else(|| format!("unknown device profile {device_id:?}"))?;

    let http = ReqwestHttp::new()?;
    let ctx = BuildContext {
        toolchain: Toolchain::discover(&root)?,
        style_root: root.join("style"),
        typ_root: root.join("typ"),
        cache_root,
        // A separate work dir keeps comparison builds from overwriting each other.
        work_dir: root
            .join("out")
            .join(arg("--work-dir").unwrap_or_else(|| "recipe-build".into())),
        http: &http,
        calibration_log: arg("--no-log").is_none().then(calibration_log_path),
    };

    let started = std::time::Instant::now();
    let report = pipeline::build(&ctx, &recipe, profile, &Cancel::new(), |u| {
        let i = Stage::all().iter().position(|s| *s == u.stage).unwrap_or(0) + 1;
        match u.fraction {
            Some(f) => println!("  [{i}/{}] {:<28} {:5.1}%  {}", Stage::all().len(), u.stage.label(), f * 100.0, u.detail),
            None => println!("  [{i}/{}] {:<28}         {}", Stage::all().len(), u.stage.label(), u.detail),
        }
    })
    .await?;

    println!();
    println!("gmapsupp     : {}", report.gmapsupp.display());
    println!("size         : {} B", report.bytes);
    println!("tiles        : {}", report.tile_count);
    println!("features     : {} ({} nodes, {} ways)", report.features, report.nodes, report.ways);
    println!("contours     : {} lines", report.contour_lines);
    println!("slope areas  : {}", report.slope_areas);
    println!("relief       : {}", if report.has_dem { "yes" } else { "no" });
    println!("family id    : {}", report.family_id);
    for w in &report.warnings {
        println!("warning      : {w}");
    }
    println!("elapsed      : {:.1}s", started.elapsed().as_secs_f64());
    Ok(())
}
