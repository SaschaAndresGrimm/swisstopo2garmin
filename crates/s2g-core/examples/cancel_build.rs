//! Prove that cancelling a build stops it, including the java child processes.
//!
//!   cargo run --release -p s2g-core --example cancel_build -- --place Bern --radius-km 20
//!
//! `Command::output` blocks until the child exits, so before the child was polled a
//! cancelled build kept splitting and compiling to completion. That is minutes on a
//! large area, with the UI already saying it had stopped.

use std::path::PathBuf;

use s2g_core::cache::Cache;
use s2g_core::devices;
use s2g_core::download::Cancel;
use s2g_core::garmin::Toolchain;
use s2g_core::gpkg::Gpkg;
use s2g_core::http::ReqwestHttp;
use s2g_core::pipeline::{self, BuildContext, Stage};
use s2g_core::recipe::{AreaSelection, Preset, Recipe, ReliefDetail};

fn arg(name: &str) -> Option<String> {
    let a: Vec<String> = std::env::args().collect();
    a.iter().position(|x| x == name).and_then(|i| a.get(i + 1).cloned())
}

fn root() -> PathBuf {
    std::env::current_dir()
        .expect("cwd")
        .ancestors()
        .find(|c| c.join("devices").is_dir() && c.join("style").is_dir())
        .expect("run from inside the repository")
        .to_path_buf()
}

/// How many java processes are running right now.
fn java_count() -> usize {
    std::process::Command::new("sh")
        .arg("-c")
        .arg("ps -Ao command | grep -c '[j]ava .*-jar'")
        .output()
        .ok()
        .and_then(|o| String::from_utf8_lossy(&o.stdout).trim().parse().ok())
        .unwrap_or(0)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = root();
    let place = arg("--place").unwrap_or_else(|| "Bern".into());
    let radius_km: f64 = arg("--radius-km").unwrap_or_else(|| "20".into()).parse()?;
    // Cancel once this stage is reached, so the kill lands inside a java child.
    let stop_at = arg("--stop-at").unwrap_or_else(|| "split".into());

    let cache_root = Cache::default_root();
    let gpkg = Gpkg::open(pipeline::find_tlm3d(&cache_root).ok_or("swissTLM3D not in the cache")?)?;
    let hit = gpkg
        .find_places(&place)?
        .into_iter()
        .next()
        .ok_or("no such place")?;

    let mut recipe = Recipe::new(
        "cancel test",
        "edge-840",
        AreaSelection::Place {
            name: hit.name.clone(),
            radius_km,
            easting: hit.easting,
            northing: hit.northing,
        },
    )
    .with_preset(Preset::Hiking);
    recipe.relief = ReliefDetail::Off;

    let profiles = devices::load_profiles(&root.join("devices"))?;
    let profile = profiles.iter().find(|p| p.id == "edge-840").ok_or("no profile")?;
    let http = ReqwestHttp::new()?;
    let ctx = BuildContext {
        toolchain: Toolchain::discover(&root)?,
        style_root: root.join("style"),
        typ_root: root.join("typ"),
        cache_root,
        work_dir: root.join("out").join("cancel-test"),
        http: &http,
        calibration_log: None,
    };

    let before = java_count();
    println!("java processes before : {before}");

    // Sample continuously, so the run can distinguish "java was killed" from "java was
    // never started" — which look identical if you only count at the end.
    let peak = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let sampling = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
    {
        let peak = peak.clone();
        let sampling = sampling.clone();
        std::thread::spawn(move || {
            while sampling.load(std::sync::atomic::Ordering::Relaxed) {
                peak.fetch_max(java_count(), std::sync::atomic::Ordering::Relaxed);
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
        });
    }

    let cancel = Cancel::new();
    let started = std::time::Instant::now();

    // --after-s cancels on a timer instead of on a stage, so the kill can be made to
    // land in the middle of a java run rather than between stages.
    if let Some(after) = arg("--after-s").and_then(|v| v.parse::<f64>().ok()) {
        let c = cancel.clone();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_secs_f64(after));
            println!("timer fired at {after}s -- cancelling");
            c.cancel();
        });
    }
    let watcher = cancel.clone();
    let stop = stop_at.clone();
    let timed = arg("--after-s").is_some();
    let result = pipeline::build(&ctx, &recipe, profile, &cancel, move |u| {
        let name = format!("{:?}", u.stage).to_lowercase();
        println!("  stage {name} at {:.1}s", started.elapsed().as_secs_f64());
        if !timed && name == stop && !watcher.is_cancelled() {
            println!(
                "reached {name} at {:.1}s -- cancelling",
                started.elapsed().as_secs_f64()
            );
            watcher.cancel();
        }
    })
    .await;

    let elapsed = started.elapsed().as_secs_f64();
    match &result {
        Err(s2g_core::Error::Cancelled) => println!("build reported Cancelled after {elapsed:.1}s"),
        Err(e) => println!("build failed with {e} after {elapsed:.1}s"),
        Ok(_) => println!("build COMPLETED after {elapsed:.1}s -- cancellation did nothing"),
    }

    sampling.store(false, std::sync::atomic::Ordering::Relaxed);
    let peak = peak.load(std::sync::atomic::Ordering::Relaxed);
    // Give the OS a moment to reap the killed child before counting.
    std::thread::sleep(std::time::Duration::from_millis(500));
    let after = java_count();
    println!("java processes peak   : {peak}");
    println!("java processes after  : {after}");
    println!("stages                : {}", Stage::all().len());

    if peak <= before {
        return Err(
            "no java child ever ran, so this run proves nothing about killing one".into(),
        );
    }

    if after > before {
        return Err(format!("{} java process(es) left running", after - before).into());
    }
    if !matches!(result, Err(s2g_core::Error::Cancelled)) {
        return Err("the build did not stop".into());
    }
    println!("\nOK: cancelled, no java left behind");
    Ok(())
}
