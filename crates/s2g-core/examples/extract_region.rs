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
use s2g_core::extract::{RegionBuilder, CYCLE_LAYERS, DEFAULT_LAYERS, WINTER_LAYERS};
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
    let want_cycle = flag("--cycle");
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
        // Through datasets:: rather than probing a directory: two cache layouts exist
        // and this used to see only the one the Milestone 5 spikes wrote.
        let sources = s2g_core::datasets::winter_geopackages(&Cache::default_root());
        if sources.is_empty() {
            return Err("no winter route data in the cache; download it in the app or \
                        with the acquire example"
                .into());
        }
        let mut winter_found: Vec<String> = Vec::new();
        for path in &sources {
            let src = Gpkg::open(path)?;
            for spec in WINTER_LAYERS {
                let before = builder.stats().features;
                builder.add_vectors(&src, std::slice::from_ref(spec), &cancel, |_, _| {})?;
                if builder.stats().features > before {
                    winter_found.push(spec.layer.to_string());
                }
            }
        }
        println!(
            "winter: {} source(s), layers {}",
            sources.len(),
            winter_found.join(", ")
        );
    }

    // ---- cycle and MTB routes (ASTRA shapefiles) ----
    if want_cycle {
        let sources = s2g_core::datasets::route_shapefiles(&Cache::default_root());
        let mut matched = 0;
        for path in &sources {
            let stem = path
                .file_stem()
                .map(|x| x.to_string_lossy().to_string())
                .unwrap_or_default();
            let Some(spec) = CYCLE_LAYERS.iter().find(|l| l.layer == stem) else {
                continue;
            };
            // All three datasets ship a Route.shp, so the tag is qualified by dataset.
            let dataset = s2g_core::datasets::route_dataset_of(path).unwrap_or_default();
            let tag = if stem == "Route" {
                format!("{dataset}_{stem}")
            } else {
                stem.clone()
            };
            let shp = s2g_core::shapefile::Shapefile::open(path)?;
            builder.add_shapefile_as(&shp, spec, &tag, &cancel)?;
            matched += 1;
        }
        if matched == 0 {
            return Err("no cycle route data in the cache; download it in the app or \
                        with the acquire example"
                .into());
        }
        println!("routes: {matched} shapefile(s)");
    }

    // ---- contours ----
    if interval > 0 {
        let http = ReqwestHttp::new()?;
        let elev_root = Cache::default_root();
        let t = std::time::Instant::now();
        let (paths, fstats) =
            fetch_tiles(&http, &elev_root, &bbox, None, 12, &cancel, |_| {}).await?;
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

        // DEM for device-side relief shading, resampled from the grid already loaded
        // for contours -- no second download and no GDAL.
        if let Some(dem_dir) = arg("--dem-dir") {
            let res = match arg("--dem-arcsec").as_deref() {
                Some("3") => s2g_core::dem::Resolution::ArcSecond3,
                _ => s2g_core::dem::Resolution::ArcSecond1,
            };
            let t = std::time::Instant::now();
            let (files, dstats) = s2g_core::dem::write_hgt(
                &grid,
                &bbox,
                res,
                std::path::Path::new(&dem_dir),
                |_| {},
            )?;
            println!(
                "dem     {} cell(s), {:.1}% covered, {:.1} MB in {:.1}s -> {}",
                files.len(),
                dstats.coverage() * 100.0,
                dstats.bytes as f64 / 1e6,
                t.elapsed().as_secs_f64(),
                dem_dir
            );
        }

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
