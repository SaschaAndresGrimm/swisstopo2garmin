//! List administrative units and resolve one to a mask (SPEC.md FR-33, FR-34).
//!
//!   cargo run --release -p s2g-core --example admin_units -- --level canton
//!   cargo run --release -p s2g-core --example admin_units -- --level commune --name Grindelwald --buffer-km 2

use s2g_core::boundaries::{self, AdminLevel};
use s2g_core::cache::Cache;

fn arg(name: &str) -> Option<String> {
    let a: Vec<String> = std::env::args().collect();
    a.iter().position(|x| x == name).and_then(|i| a.get(i + 1).cloned())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let level = match arg("--level").unwrap_or_else(|| "canton".into()).as_str() {
        "commune" => AdminLevel::Commune,
        "district" => AdminLevel::District,
        _ => AdminLevel::Canton,
    };
    let root = Cache::default_root();
    let units = boundaries::list_units(&root, level)?;
    println!("{} {}s in Switzerland\n", units.len(), level.id());

    match arg("--name") {
        None => {
            for u in units.iter().take(12) {
                println!(
                    "  {:>5}  {:<28} {:<14} {:>9} people  {:>8.0} km²",
                    u.number, u.name, u.canton, u.population, u.area_km2
                );
            }
            if units.len() > 12 {
                println!("  … {} more", units.len() - 12);
            }
        }
        Some(name) => {
            let matches: Vec<_> = units
                .iter()
                .filter(|u| u.name.eq_ignore_ascii_case(&name))
                .collect();
            if matches.is_empty() {
                return Err(format!("no {} called {name:?}", level.id()).into());
            }
            for u in &matches {
                println!(
                    "  {:>5}  {:<28} {:<14} {:>9} people  {:>8.0} km²",
                    u.number, u.name, u.canton, u.population, u.area_km2
                );
            }
            let numbers: Vec<i64> = matches.iter().map(|u| u.number).collect();
            let buffer_km: f64 = arg("--buffer-km").unwrap_or_else(|| "0".into()).parse()?;

            let bbox = boundaries::extent(&root, level, &numbers)?;
            println!(
                "\nextent      : {:.0},{:.0} .. {:.0},{:.0}  ({:.0} km²)",
                bbox.min_e, bbox.min_n, bbox.max_e, bbox.max_n, bbox.area_km2()
            );

            let polys = boundaries::load_geometry(&root, level, &numbers)?;
            let points: usize = polys.iter().flatten().map(|r| r.len()).sum();
            println!("geometry    : {} polygon(s), {points} points", polys.len());

            let started = std::time::Instant::now();
            let mask = s2g_core::mask::Mask::polygons_buffered(polys, buffer_km * 1000.0);
            println!("mask built  : {:.0} ms", started.elapsed().as_secs_f64() * 1000.0);
            let mb = mask.bbox();
            println!(
                "mask extent : {:.0},{:.0} .. {:.0},{:.0}  ({:.0} km²)",
                mb.min_e, mb.min_n, mb.max_e, mb.max_n, mb.area_km2()
            );

            // How much of the bounding box the unit actually covers: the point of a
            // mask is that this is well under 100%.
            let started = std::time::Instant::now();
            let (mut inside, mut total) = (0u32, 0u32);
            let step = 200.0;
            let mut n = mb.min_n;
            while n < mb.max_n {
                let mut e = mb.min_e;
                while e < mb.max_e {
                    total += 1;
                    if mask.contains(s2g_core::geom::Coord::new(e, n)) {
                        inside += 1;
                    }
                    e += step;
                }
                n += step;
            }
            println!(
                "coverage    : {inside}/{total} sample points inside ({:.0}%), {} lookups in {:.0} ms",
                100.0 * inside as f64 / total.max(1) as f64,
                total,
                started.elapsed().as_secs_f64() * 1000.0
            );
        }
    }
    Ok(())
}
