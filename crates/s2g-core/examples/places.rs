fn main() {
    let root = s2g_core::cache::Cache::default_root().join("ch.swisstopo.swisstlm3d");
    let Some(gpkg) = std::fs::read_dir(&root).ok().and_then(|rd| {
        rd.flatten()
            .filter_map(|e| {
                std::fs::read_dir(e.path()).ok().map(|inner| {
                    inner
                        .flatten()
                        .map(|f| f.path())
                        .filter(|p| p.extension().map(|x| x == "gpkg").unwrap_or(false))
                        .collect::<Vec<_>>()
                })
            })
            .flatten()
            .next()
    }) else {
        println!("national gpkg not cached");
        return;
    };
    let g = s2g_core::gpkg::Gpkg::open(&gpkg).unwrap();
    // Queries from the command line, else a set that exercises case, accents and
    // prefixes -- the three things the search used to refuse.
    let args: Vec<String> = std::env::args().skip(1).collect();
    let default = [
        "Grindelwald",
        "grindelwald",
        "grindel",
        "zurich",
        "geneve",
        "neuchatel",
        "bern",
    ]
    .map(str::to_string);
    let queries: Vec<String> = if args.is_empty() {
        default.to_vec()
    } else {
        args
    };
    for name in &queries {
        println!("{name}:");
        for p in g.find_places(name).unwrap().iter().take(4) {
            let (lon, lat) = s2g_core::proj::lv95_to_wgs84(p.easting, p.northing);
            println!(
                "   {:<20} E={:.0} N={:.0}  ({:.4}N {:.4}E)",
                p.population_category.clone().unwrap_or("(none)".into()),
                p.easting,
                p.northing,
                lat,
                lon
            );
        }
    }
}
