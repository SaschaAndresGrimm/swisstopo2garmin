//! Report which presets have their data, the way the Content screen decides.
//!
//!   cargo run -p s2g-core --example preset_readiness
//!
//! Exists because the screen said "download it on the Data screen" for data that was
//! already downloaded: the check probed the Milestone 5 spike directories rather than
//! asking `datasets::`. This prints what the shared discovery actually finds.

use s2g_core::cache::Cache;
use s2g_core::datasets;
use s2g_core::recipe::Preset;

fn main() {
    let root = Cache::default_root();
    let winter = datasets::winter_geopackages(&root);
    let cycle = datasets::route_shapefiles(&root);

    println!("cache: {}\n", root.display());
    println!("winter sources ({}):", winter.len());
    for p in &winter {
        println!("   {}", p.display());
    }
    println!("\ncycle sources ({}):", cycle.len());
    for p in &cycle {
        println!("   {}", p.display());
    }

    println!("\npresets:");
    for p in Preset::all() {
        let mut missing = Vec::new();
        if p.needs_winter() && winter.is_empty() {
            missing.push("winter route data");
        }
        if p.needs_cycle() && cycle.is_empty() {
            missing.push("cycle route data");
        }
        let state = if missing.is_empty() {
            "ready".to_string()
        } else {
            format!("needs {}", missing.join(", "))
        };
        println!("   {:<8} {state}", p.id());
    }
}
