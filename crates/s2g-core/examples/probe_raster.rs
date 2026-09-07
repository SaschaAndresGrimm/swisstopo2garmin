//! Build a real Garmin Custom Map KMZ against the live swisstopo WMS.
//!
//!   cargo run --release -p s2g-core --example probe_raster -- \
//!       --place-e 2645921 --place-n 1163748 --radius-km 2 --out out/raster
//!
//! Kept as an example rather than a test because it hits the network. This is where the
//! bytes-per-pixel figure in `raster::approx_bytes` comes from -- it is measured here,
//! not assumed.

use std::path::PathBuf;

use s2g_core::download::Cancel;
use s2g_core::http::ReqwestHttp;
use s2g_core::proj::BBox;
use s2g_core::raster::{self, RasterLimits};

fn arg(name: &str) -> Option<String> {
    let a: Vec<String> = std::env::args().collect();
    a.iter()
        .position(|x| x == name)
        .and_then(|i| a.get(i + 1).cloned())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let e: f64 = arg("--place-e")
        .unwrap_or_else(|| "2645921".into())
        .parse()?;
    let n: f64 = arg("--place-n")
        .unwrap_or_else(|| "1163748".into())
        .parse()?;
    let radius_km: f64 = arg("--radius-km").unwrap_or_else(|| "2".into()).parse()?;
    let res: f64 = arg("--res")
        .unwrap_or_else(|| raster::NATIVE_M_PER_PX.to_string())
        .parse()?;
    let out_dir = PathBuf::from(arg("--out").unwrap_or_else(|| "out/raster".into()));

    let limits = RasterLimits::default();
    let bbox = BBox::from_center(e, n, radius_km * 1000.0);
    let plan = raster::plan(&bbox, &limits, res)?;

    println!(
        "area         : {:.0} km² ({:.1} km radius)",
        bbox.area_km2(),
        radius_km
    );
    println!(
        "grid         : {} x {} = {} tiles",
        plan.cols,
        plan.rows,
        plan.tile_count()
    );
    println!("resolution   : {:.2} m per pixel", plan.m_per_px);
    println!("pixels       : {}", plan.total_pixels());
    println!("predicted    : {} B", plan.approx_bytes());
    for note in &plan.notes {
        println!("note         : {note}");
    }
    println!("first tile   : {}", plan.tiles[0].wms_url(&plan.layer));

    let http = ReqwestHttp::new()?;
    let out = out_dir.join("Grindelwald.kmz");
    let started = std::time::Instant::now();
    let report = raster::build_kmz(&http, &plan, "Grindelwald", &out, &Cancel::new(), |d, t| {
        if d == t || d % 5 == 0 {
            println!("  fetched {d}/{t}");
        }
    })
    .await?;

    println!();
    println!("kmz          : {}", report.kmz.display());
    println!("size         : {} B", report.bytes);
    println!(
        "measured     : {:.3} B per pixel  (predicted {:.3})",
        report.bytes as f64 / plan.total_pixels() as f64,
        plan.approx_bytes() as f64 / plan.total_pixels() as f64
    );
    for w in &report.warnings {
        println!("warning      : {w}");
    }
    println!("elapsed      : {:.1}s", started.elapsed().as_secs_f64());
    Ok(())
}
