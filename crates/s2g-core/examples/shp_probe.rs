//! Probe a shapefile: geometry sanity and attribute value domains.
//!   cargo run -p s2g-core --example shp_probe -- <file.shp> [attr ...]
use s2g_core::shapefile::Shapefile;
use std::collections::HashMap;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let path = args.first().ok_or("need a .shp path")?;
    let attrs: Vec<&str> = args[1..].iter().map(|s| s.as_str()).collect();

    let shp = Shapefile::open(path)?;
    println!("shape type : {:?}", shp.shape_type);
    println!(
        "bbox       : {:.0} {:.0} {:.0} {:.0} ({:.0} km2)",
        shp.bbox.min_e,
        shp.bbox.min_n,
        shp.bbox.max_e,
        shp.bbox.max_n,
        shp.bbox.area_km2()
    );
    println!("fields     : {:?}", shp.field_names());

    let mut domains: HashMap<&str, HashMap<String, usize>> = HashMap::new();
    let mut kinds: HashMap<&'static str, usize> = HashMap::new();
    let mut coords = 0usize;
    let mut outside = 0usize;

    let n = shp.for_each_in_bbox(&shp.bbox, &attrs, |f| {
        *kinds.entry(f.geometry.kind()).or_insert(0) += 1;
        coords += f.geometry.coords().count();
        // Every coordinate must be plausible LV95.
        if f.geometry.coords().any(|c| {
            !(2_400_000.0..2_900_000.0).contains(&c.e) || !(1_000_000.0..1_400_000.0).contains(&c.n)
        }) {
            outside += 1;
        }
        for a in &attrs {
            if let Some(v) = f.tag(a) {
                let d = domains.entry(a).or_default();
                if d.len() < 25 {
                    *d.entry(v).or_insert(0) += 1;
                }
            }
        }
        true
    })?;

    println!("records    : {n}");
    println!("geometry   : {kinds:?}");
    println!("coordinates: {coords} ({} outside LV95 bounds)", outside);
    for (a, d) in &domains {
        let mut v: Vec<_> = d.iter().collect();
        v.sort_by_key(|(_, c)| std::cmp::Reverse(**c));
        println!("  {a}:");
        for (val, c) in v.iter().take(12) {
            println!("      {val:<40} {c:>7}");
        }
    }
    Ok(())
}
