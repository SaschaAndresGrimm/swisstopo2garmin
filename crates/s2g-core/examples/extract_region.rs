//! Extract a region to OSM PBF: swissTLM3D vectors, optional winter routes, contours.
//!
//!   cargo run --release -p s2g-core --example extract_region -- \
//!       --place Grindelwald --radius-km 8 --contour 20 --winter --out region.osm.pbf
//!
//! Replaces spikes/s0/tlm2osm.py and spikes/s0/contours.py with the tested Rust
//! pipeline. DEM `.hgt` generation is still the Python spike, because it needs
//! windowed reads of the 10 GB swissALTIRegio COG rather than whole 1 km tiles.

use std::path::PathBuf;

use s2g_core::cache::Cache;
use s2g_core::contour::{generate, ContourConfig};
use s2g_core::download::Cancel;
use s2g_core::elevation::{fetch_tiles, load_grid};
use s2g_core::extract::{RegionBuilder, DEFAULT_LAYERS, WINTER_LAYERS};
use s2g_core::geom::{point_in_polygon, Coord, Geometry};
use s2g_core::gpkg::Gpkg;
use s2g_core::http::ReqwestHttp;
use s2g_core::proj::{lv95_to_wgs84, BBox};

fn arg(name: &str) -> Option<String> {
    let args: Vec<String> = std::env::args().collect();
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1).cloned())
}
fn flag(name: &str) -> bool {
    std::env::args().any(|a| a == name)
}

fn national_gpkg() -> Option<PathBuf> {
    let root = Cache::default_root().join("ch.swisstopo.swisstlm3d");
    std::fs::read_dir(root).ok()?.flatten().find_map(|e| {
        std::fs::read_dir(e.path()).ok()?.flatten().find_map(|f| {
            let p = f.path();
            (p.extension()? == "gpkg").then_some(p)
        })
    })
}

/// Ice polygons for blue-over-ice contour classification.
fn load_ice(gpkg: &Gpkg, bbox: &BBox) -> Vec<Vec<Vec<Coord>>> {
    const ICE: [&str; 2] = ["Gletscher", "Schneefeld Toteis"];
    let mut polys = Vec::new();
    let _ = gpkg.for_each_in_bbox("tlm_bb_bodenbedeckung", bbox, &["objektart"], |f| {
        if f.attr("objektart")
            .map(|v| ICE.contains(&v))
            .unwrap_or(false)
        {
            match f.geometry {
                Geometry::Polygon(rings) => polys.push(rings),
                Geometry::MultiPolygon(ps) => polys.extend(ps),
                _ => {}
            }
        }
        true
    });
    polys
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out = PathBuf::from(arg("--out").unwrap_or_else(|| "region.osm.pbf".into()));
    let radius_km: f64 = arg("--radius-km")
        .and_then(|v| v.parse().ok())
        .unwrap_or(8.0);
    let interval: i32 = arg("--contour").and_then(|v| v.parse().ok()).unwrap_or(0);
    let want_winter = flag("--winter");
    let cancel = Cancel::new();

    let gpkg_path = national_gpkg().ok_or("swissTLM3D not cached; download it first")?;
    let gpkg = Gpkg::open(&gpkg_path)?;
    println!("source  {}", gpkg_path.display());

    // ---- area ----
    let bbox = if let Some(place) = arg("--place") {
        let places = gpkg.find_places(&place)?;
        if places.is_empty() {
            return Err(format!("place {place:?} not found").into());
        }
        if places.len() > 1 {
            // Never collapse silently: two settlements are named Grindelwald.
            println!("note    {} places named {place:?}:", places.len());
            for p in &places {
                let (lon, lat) = lv95_to_wgs84(p.easting, p.northing);
                println!(
                    "          {:<20} {:.4}N {:.4}E",
                    p.population_category
                        .as_deref()
                        .unwrap_or("(no population)"),
                    lat,
                    lon
                );
            }
            println!("        using the most populous; pass --bbox to override");
        }
        let p = &places[0];
        BBox::from_center(p.easting, p.northing, radius_km * 1000.0)
    } else if let Some(b) = arg("--bbox") {
        let v: Vec<f64> = b.split(',').filter_map(|x| x.trim().parse().ok()).collect();
        if v.len() != 4 {
            return Err("--bbox needs minE,minN,maxE,maxN".into());
        }
        BBox::new(v[0], v[1], v[2], v[3])
    } else {
        return Err("need --place or --bbox".into());
    };
    println!(
        "area    {:.0} km2  LV95 {:.0} {:.0} {:.0} {:.0}",
        bbox.area_km2(),
        bbox.min_e,
        bbox.min_n,
        bbox.max_e,
        bbox.max_n
    );

    let mut builder = RegionBuilder::create(&out, &bbox)?;

    // ---- swissTLM3D vectors ----
    let t0 = std::time::Instant::now();
    builder.add_vectors(&gpkg, DEFAULT_LAYERS, &cancel, |layer, n| {
        if n > 0 {
            println!("  {layer:<40} {n:>8}");
        }
    })?;
    println!("vectors in {:.1}s", t0.elapsed().as_secs_f64());

    // ---- winter routes ----
    if want_winter {
        let winter_dir = Cache::default_root().join("winter");
        let mut found = 0;
        // Each winter GeoPackage holds only its own layers, so every source reports
        // the others as missing. Track what was actually found and report only the
        // specs no source provided.
        let mut winter_found: Vec<String> = Vec::new();
        for entry in std::fs::read_dir(&winter_dir)
            .map_err(|e| {
                format!(
                    "{}: {e}; run spikes/s0/fetch_winter.py",
                    winter_dir.display()
                )
            })?
            .flatten()
        {
            let p = entry.path();
            if p.extension().map(|x| x != "gpkg").unwrap_or(true) {
                continue;
            }
            let src = Gpkg::open(&p)?;
            builder.add_vectors(&src, WINTER_LAYERS, &cancel, |layer, n| {
                winter_found.push(layer.to_string());
                if n > 0 {
                    println!("  {layer:<40} {n:>8}");
                }
            })?;
            found += 1;
        }
        if found == 0 {
            return Err("no winter GeoPackages cached; run spikes/s0/fetch_winter.py".into());
        }
        let absent: Vec<&str> = WINTER_LAYERS
            .iter()
            .map(|l| l.layer)
            .filter(|want| !winter_found.iter().any(|got| got.starts_with(*want)))
            .collect();
        if !absent.is_empty() {
            println!("winter layers not found in any source: {absent:?}");
        }
    }

    // ---- contours ----
    if interval > 0 {
        let http = ReqwestHttp::new()?;
        let elev_root = Cache::default_root();
        let t = std::time::Instant::now();
        let (paths, fstats) = fetch_tiles(&http, &elev_root, &bbox, 12, &cancel, |_| {}).await?;
        println!(
            "elevation {} cells ({} cached, {} fetched, {} older duplicates skipped, {} missing) in {:.1}s",
            paths.len(),
            fstats.already_cached,
            fstats.downloaded,
            fstats.duplicates_skipped,
            fstats.missing.len(),
            t.elapsed().as_secs_f64()
        );

        let grid = load_grid(&paths, &bbox)?;
        let cfg = ContourConfig {
            interval_m: interval,
            ..Default::default()
        };
        if !cfg.medium_is_reachable() {
            eprintln!("warning: the medium contour tier can never occur with this interval");
        }
        let t = std::time::Instant::now();
        let (contours, cstats) = generate(&grid, &cfg, || cancel.is_cancelled());
        println!(
            "contours {} lines across {} levels, {:.0}% of points removed, in {:.1}s",
            cstats.lines,
            cstats.levels,
            cstats.simplification_ratio() * 100.0,
            t.elapsed().as_secs_f64()
        );

        let ice = load_ice(&gpkg, &bbox);
        println!("ice     {} polygons for blue-over-ice contours", ice.len());
        let n = builder.add_contours(
            &contours,
            |p| ice.iter().any(|rings| point_in_polygon(p, rings)),
            &cancel,
        )?;
        println!("        {n} contour ways written");
    }

    let stats = builder.finish()?;
    let bytes = std::fs::metadata(&out).map(|m| m.len()).unwrap_or(0);
    println!(
        "\nwrote {} ({:.1} MB): {} features, {} nodes, {} ways",
        out.display(),
        bytes as f64 / 1e6,
        stats.features,
        stats.nodes,
        stats.ways
    );
    // stats.missing_layers accumulates per source, so a winter layer absent from one
    // GeoPackage but present in another appears here; it is reported above instead.
    let tlm_missing: Vec<&String> = stats
        .missing_layers
        .iter()
        .filter(|m| DEFAULT_LAYERS.iter().any(|l| l.layer == m.as_str()))
        .collect();
    if !tlm_missing.is_empty() {
        println!("swissTLM3D layers not present: {tlm_missing:?}");
    }
    Ok(())
}
