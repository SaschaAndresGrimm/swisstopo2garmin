//! Fit the shipped size model from real builds (SPEC.md FR-61).
//!
//!   cargo run --release -p s2g-core --example fit_size_model -- --out estimator/size-model.json
//!
//! Runs a spread of builds — different places, radii, presets, contour intervals and
//! relief settings — records (predictors, actual bytes) for each, fits the model and
//! reports its error. The spread matters more than the count: samples whose predictors
//! are proportional to each other leave the fit underdetermined.
//!
//! Add `--samples <file.jsonl>` to fit from an existing log instead of rebuilding.

use std::path::{Path, PathBuf};

use s2g_core::cache::Cache;
use s2g_core::devices;
use s2g_core::download::Cancel;
use s2g_core::estimate::{CalibrationLog, Sample, SizeModel};
use s2g_core::garmin::Toolchain;
use s2g_core::gpkg::Gpkg;
use s2g_core::http::ReqwestHttp;
use s2g_core::pipeline::{self, BuildContext};
use s2g_core::recipe::{AreaSelection, Preset, Recipe, ReliefDetail};

fn arg(name: &str) -> Option<String> {
    let a: Vec<String> = std::env::args().collect();
    a.iter()
        .position(|x| x == name)
        .and_then(|i| a.get(i + 1).cloned())
}

fn root() -> PathBuf {
    std::env::current_dir()
        .expect("cwd")
        .ancestors()
        .find(|c| c.join("devices").is_dir() && c.join("style").is_dir())
        .expect("run from inside the repository")
        .to_path_buf()
}

/// The training set. Chosen to vary the predictors independently: dense urban areas
/// against empty alpine ones, small against large, every contour interval, both relief
/// resolutions, and presets that bring in the winter and cycle networks.
const PLAN: &[(&str, f64, Preset, i32, ReliefDetail)] = &[
    ("Grindelwald", 6.0, Preset::Hiking, 20, ReliefDetail::Gentle),
    ("Grindelwald", 12.0, Preset::Hiking, 50, ReliefDetail::Off),
    ("Zermatt", 8.0, Preset::Skimo, 20, ReliefDetail::Detailed),
    ("Zermatt", 4.0, Preset::Hiking, 10, ReliefDetail::Gentle),
    ("Bern", 6.0, Preset::Cycling, 20, ReliefDetail::Off),
    ("Bern", 10.0, Preset::Hiking, 100, ReliefDetail::Gentle),
    ("Zürich", 8.0, Preset::Cycling, 50, ReliefDetail::Gentle),
    ("Zürich", 5.0, Preset::Hiking, 20, ReliefDetail::Detailed),
    ("Davos", 10.0, Preset::Skimo, 20, ReliefDetail::Gentle),
    ("Davos", 6.0, Preset::Full, 10, ReliefDetail::Off),
    ("Lugano", 7.0, Preset::Hiking, 20, ReliefDetail::Gentle),
    ("Sion", 9.0, Preset::Cycling, 100, ReliefDetail::Detailed),
    ("Interlaken", 5.0, Preset::Full, 20, ReliefDetail::Gentle),
    ("Chur", 8.0, Preset::Hiking, 10, ReliefDetail::Off),
    ("Locarno", 6.0, Preset::Skimo, 50, ReliefDetail::Gentle),
    ("Fribourg", 12.0, Preset::Hiking, 20, ReliefDetail::Off),
    ("Andermatt", 7.0, Preset::Skimo, 10, ReliefDetail::Detailed),
    ("Neuchâtel", 9.0, Preset::Cycling, 20, ReliefDetail::Gentle),
];

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = root();
    let out = PathBuf::from(
        arg("--out")
            .unwrap_or_else(|| root.join("estimator/size-model.json").display().to_string()),
    );
    let device_id = arg("--device").unwrap_or_else(|| "edge-840".into());

    // Fitting from an existing log skips the builds entirely.
    if let Some(f) = arg("--samples") {
        let samples = CalibrationLog::read(Path::new(&f));
        return report(&samples, &out);
    }

    let cache_root = Cache::default_root();
    let gpkg_path = pipeline::find_tlm3d(&cache_root).ok_or("swissTLM3D not in the cache")?;
    let gpkg = Gpkg::open(&gpkg_path)?;
    let profiles = devices::load_profiles(&root.join("devices"))?;
    let profile = profiles
        .iter()
        .find(|p| p.id == device_id)
        .ok_or("unknown device profile")?;
    let http = ReqwestHttp::new()?;
    let log = root.join("out").join("fit-samples.jsonl");
    let _ = std::fs::remove_file(&log);

    for (i, (place, radius_km, preset, interval, relief)) in PLAN.iter().enumerate() {
        let Some(hit) = gpkg.find_places(place)?.into_iter().next() else {
            println!(
                "[{:2}/{}] {place}: no such place, skipped",
                i + 1,
                PLAN.len()
            );
            continue;
        };
        let mut recipe = Recipe::new(
            format!("fit {place} {radius_km}"),
            &device_id,
            AreaSelection::Place {
                name: hit.name.clone(),
                radius_km: *radius_km,
                easting: hit.easting,
                northing: hit.northing,
            },
        )
        .with_preset(*preset);
        recipe.contours.interval_m = *interval;
        recipe.relief = *relief;

        let ctx = BuildContext {
            toolchain: Toolchain::discover(&root)?,
            style_root: root.join("style"),
            typ_root: root.join("typ"),
            cache_root: cache_root.clone(),
            work_dir: root.join("out").join("fit").join(format!("{i:02}")),
            http: &http,
            calibration_log: Some(log.clone()),
        };

        let started = std::time::Instant::now();
        match pipeline::build(&ctx, &recipe, profile, &Cancel::new(), |_| {}).await {
            Ok(r) => println!(
                "[{:2}/{}] {place} {radius_km} km {} c{interval} {relief:?}: {} B in {:.0}s",
                i + 1,
                PLAN.len(),
                preset.id(),
                r.bytes,
                started.elapsed().as_secs_f64()
            ),
            Err(e) => println!("[{:2}/{}] {place}: FAILED {e}", i + 1, PLAN.len()),
        }
        // The work dir holds a full PBF and DEM per build; 18 of them is a lot of disk.
        let _ = std::fs::remove_dir_all(&ctx.work_dir);
    }

    let samples = CalibrationLog::read(&log);
    report(&samples, &out)
}

fn report(samples: &[Sample], out: &Path) -> Result<(), Box<dyn std::error::Error>> {
    if samples.is_empty() {
        return Err("no samples to fit".into());
    }
    let prior = SizeModel::default();
    // The ridge strength is chosen by leave-one-out cross-validation, not by in-sample
    // error: with 11 terms and this many builds, in-sample error always prefers no
    // regularisation and then predicts an unseen area badly.
    println!("\nlambda sweep (leave-one-out error):");
    for l in [0.0, 1e2, 1e3, 1e4, 1e5, 1e6, 1e7, 1e8, 1e9, 1e10, 1e11] {
        println!(
            "  lambda {l:>9.0e}  LOOCV {:>5.1}%",
            SizeModel::loocv_mape(samples, &prior, l) * 100.0
        );
    }
    let (fitted, lambda) = SizeModel::fit_cv(samples, &prior);

    println!("\nsamples      : {}", samples.len());
    println!("chosen lambda: {lambda:.0e}");
    println!("prior MAPE   : {:.1}%", prior.mape(samples) * 100.0);
    println!(
        "fitted MAPE  : {:.1}%  (LOOCV {:.1}%)",
        fitted.mape(samples) * 100.0,
        SizeModel::loocv_mape(samples, &prior, lambda) * 100.0
    );

    let names = [
        "intercept",
        "landCover",
        "water",
        "transport",
        "built",
        "names",
        "winter",
        "cycling",
        "contour/km² @20m",
        "relief/km² 1\"",
        "relief/km² 3\"",
    ];
    println!("\n{:<20} {:>14} {:>14}", "term", "prior", "fitted");
    for (i, n) in names.iter().enumerate() {
        println!(
            "{n:<20} {:>14.2} {:>14.2}",
            prior.coefficients[i], fitted.coefficients[i]
        );
    }

    // Stage timings: the seed weights for the build-time estimate (FR-70a). Printed in
    // the form the constant takes, so it is transcribed rather than retyped.
    let timed: Vec<&Sample> = samples
        .iter()
        .filter(|s| !s.stage_seconds.is_empty())
        .collect();
    if timed.is_empty() {
        println!("\nno stage timings in these samples");
    } else {
        let owned: Vec<Sample> = timed.iter().map(|s| (*s).clone()).collect();
        let weights = s2g_core::estimate::stage_weights(&owned);
        println!("\nstage weights from {} timed builds:", owned.len());
        for (name, w) in s2g_core::estimate::STAGE_NAMES.iter().zip(weights.iter()) {
            println!("    {w:.4}, // {name}");
        }
        let mean_total: f64 = owned
            .iter()
            .map(|s| s.stage_seconds.iter().map(|(_, v)| *v).sum::<f64>())
            .sum::<f64>()
            / owned.len() as f64;
        println!("  mean build {mean_total:.1}s");
    }

    println!("\nworst residuals:");
    let mut rows: Vec<(f64, u64, u64, f64)> = samples
        .iter()
        .map(|s| {
            let p = fitted.predict(&s.predictors) as f64;
            let a = s.actual_bytes as f64;
            (
                (p - a).abs() / a,
                s.actual_bytes,
                p as u64,
                s.predictors.area_km2,
            )
        })
        .collect();
    rows.sort_by(|x, y| y.0.total_cmp(&x.0));
    for (err, actual, pred, km2) in rows.iter().take(5) {
        println!(
            "  {:>5.1}%  {km2:>6.0} km²  actual {actual:>9}  predicted {pred:>9}",
            err * 100.0
        );
    }

    // A fit that is worse than the prior is not an improvement; do not ship it.
    if fitted.mape(samples) > prior.mape(samples) {
        return Err("the fit is worse than the prior; not written".into());
    }
    fitted.save(out)?;
    println!("\nwrote {}", out.display());
    Ok(())
}
