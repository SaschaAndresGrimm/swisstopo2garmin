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
    for name in ["Grindelwald", "Zermatt", "Bern"] {
        println!("{name}:");
        for p in g.find_places(name).unwrap() {
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
