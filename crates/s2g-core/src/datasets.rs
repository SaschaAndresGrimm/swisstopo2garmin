//! Locating acquired datasets in the cache.
//!
//! Two layouts exist and both must work. The app writes
//! `<cache>/<collection>/<item>/<file>` through [`crate::cache`]; the Milestone 5
//! Python spikes wrote flat `<cache>/winter/*.gpkg` and `<cache>/routes/<dataset>/…`
//! directories, which are still on developer machines and in the docs.
//!
//! The build pipeline previously looked only in the flat directories, so winter and
//! cycle data acquired through the app was downloaded and then never found.

use std::path::{Path, PathBuf};

use crate::stac;

/// STAC collections holding winter route data.
pub const WINTER_COLLECTIONS: &[&str] = &[
    stac::SKITOUREN,
    stac::SCHNEESCHUH,
    stac::WINTERWANDERN,
    stac::UNTERKUENFTE,
];

/// STAC collections holding the ASTRA route networks.
///
/// `wanderland` is included because it is the same shapefile family; the extractor
/// ignores its `Route.shp` so hiking routes are not drawn as cycle routes.
pub const ROUTE_COLLECTIONS: &[&str] = &[stac::VELOLAND, stac::MOUNTAINBIKELAND, stac::WANDERLAND];

/// Every file under `root` with the given extension, at any depth.
fn find_by_extension(root: &Path, ext: &str, out: &mut Vec<PathBuf>) {
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&dir) else {
            continue;
        };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().map(|x| x == ext).unwrap_or(false) {
                out.push(p);
            }
        }
    }
}

/// Rank a candidate so the newest copy of a dataset wins when several are present.
///
/// A developer machine can hold both layouts, and a cache can hold two releases of one
/// collection. Extracting all of them would draw every route twice.
fn rank(path: &Path) -> (u8, String) {
    // App layout first: `<cache>/<collection>/<item>/<file>`, ranked by item id, which
    // is date-based for every collection here.
    for a in path.ancestors() {
        let Some(name) = a.file_name() else { continue };
        let name = name.to_string_lossy().to_string();
        if WINTER_COLLECTIONS.contains(&name.as_str()) || ROUTE_COLLECTIONS.contains(&name.as_str())
        {
            let item = a
                .join("..")
                .components()
                .next_back()
                .map(|_| ())
                .and_then(|_| path.strip_prefix(a).ok())
                .and_then(|rel| rel.components().next())
                .map(|c| c.as_os_str().to_string_lossy().to_string())
                .unwrap_or_default();
            return (1, item);
        }
    }
    (0, String::new())
}

/// Pick one file per key, preferring the highest-ranked copy.
fn newest_per<K: Ord>(files: Vec<PathBuf>, key: impl Fn(&Path) -> Option<K>) -> Vec<PathBuf> {
    let mut best: std::collections::BTreeMap<K, (u8, String, PathBuf)> = Default::default();
    for f in files {
        let Some(k) = key(&f) else { continue };
        let (r, item) = rank(&f);
        match best.get(&k) {
            Some((br, bi, _)) if (*br, bi.as_str()) >= (r, item.as_str()) => {}
            _ => {
                best.insert(k, (r, item, f));
            }
        }
    }
    let mut out: Vec<PathBuf> = best.into_values().map(|(_, _, p)| p).collect();
    out.sort();
    out
}

/// Which winter dataset a GeoPackage holds, from its collection directory or, in the
/// flat layout, from its file name.
///
/// The name carries the release year (`ski_routes_2056`), so the key is the stem with
/// any trailing `_<digits>` removed.
pub fn winter_key(path: &Path) -> Option<String> {
    let stem = path.file_stem()?.to_string_lossy().to_string();
    let base = match stem.rsplit_once('_') {
        Some((head, tail)) if tail.chars().all(|c| c.is_ascii_digit()) => head.to_string(),
        _ => stem,
    };
    Some(base.to_lowercase())
}

/// The winter GeoPackages present in the cache, in whichever layout they arrived./// The winter GeoPackages present in the cache, in whichever layout they arrived.
///
/// One per dataset: a machine can hold both layouts, or two releases of a collection,
/// and reading all of them would draw every route twice.
pub fn winter_geopackages(cache_root: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    find_by_extension(&cache_root.join("winter"), "gpkg", &mut found);
    for c in WINTER_COLLECTIONS {
        find_by_extension(&cache_root.join(c), "gpkg", &mut found);
    }
    newest_per(found, winter_key)
}

/// GeoPackages holding the SAC hut list.
///
/// Separate from `winter_geopackages` because huts are wanted by every preset: they
/// arrive in the winter accommodation dataset, but a hut is not a winter feature.
pub fn hut_geopackages(cache_root: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    find_by_extension(&cache_root.join(stac::UNTERKUENFTE), "gpkg", &mut found);
    // The spike-era flat directory held it too.
    find_by_extension(&cache_root.join("winter"), "gpkg", &mut found);
    found.retain(|p| {
        p.file_name()
            .map(|n| n.to_string_lossy().contains("unterkuenfte"))
            .unwrap_or(false)
    });
    newest_per(found, winter_key)
}

/// The route shapefiles present in the cache, one per dataset and layer.
pub fn route_shapefiles(cache_root: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    find_by_extension(&cache_root.join("routes"), "shp", &mut found);
    for c in ROUTE_COLLECTIONS {
        find_by_extension(&cache_root.join(c), "shp", &mut found);
    }
    // Keyed by dataset *and* file stem: all three datasets ship a Route.shp, and those
    // are three different things.
    newest_per(found, |p| {
        Some((
            route_dataset_of(p)?,
            p.file_stem()?.to_string_lossy().to_string(),
        ))
    })
}

/// Which ASTRA dataset a shapefile belongs to.
///
/// All three datasets ship a `Route.shp`, so the file name alone cannot tell a
/// mountain-bike route from a cycle route from a hiking route. The dataset is taken
/// from the path: the collection id in the app layout, or the containing directory in
/// the flat spike layout.
pub fn route_dataset_of(path: &Path) -> Option<String> {
    for a in path.ancestors() {
        let name = a.file_name()?.to_string_lossy().to_string();
        if let Some(c) = ROUTE_COLLECTIONS.iter().find(|c| **c == name) {
            return Some(short_dataset_name(c));
        }
        if matches!(
            name.as_str(),
            "veloland" | "mountainbikeland" | "wanderland"
        ) {
            return Some(name);
        }
    }
    None
}

/// `ch.astra.veloland` -> `veloland`.
fn short_dataset_name(collection: &str) -> String {
    collection
        .rsplit('.')
        .next()
        .unwrap_or(collection)
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn touch(path: &Path) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, b"").unwrap();
    }

    #[test]
    fn finds_winter_data_in_both_layouts() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        touch(&root.join("winter/ski_routes_2056.gpkg"));
        touch(&root.join(stac::SCHNEESCHUH).join("item-1/schneeschuh.gpkg"));
        touch(&root.join("winter/notes.txt"));

        let found = winter_geopackages(root);
        assert_eq!(found.len(), 2, "{found:?}");
        assert!(found.iter().all(|p| p.extension().unwrap() == "gpkg"));
    }

    #[test]
    fn finds_route_shapefiles_in_both_layouts() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        touch(&root.join("routes/veloland/2026_shape_veloland/VeloWeg.shp"));
        touch(
            &root
                .join(stac::MOUNTAINBIKELAND)
                .join("mtb-2026/MTBWeg.shp"),
        );
        touch(
            &root
                .join(stac::MOUNTAINBIKELAND)
                .join("mtb-2026/MTBWeg.dbf"),
        );

        let found = route_shapefiles(root);
        assert_eq!(found.len(), 2, "{found:?}");
    }

    /// Both layouts on one machine must not mean every route drawn twice.
    #[test]
    fn one_copy_of_each_dataset_survives_when_both_layouts_are_present() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        touch(&root.join("routes/veloland/2026_shape_veloland/VeloWeg.shp"));
        touch(&root.join("routes/veloland/2026_shape_veloland/Route.shp"));
        touch(&root.join(stac::VELOLAND).join("veloland-2026/VeloWeg.shp"));
        touch(&root.join(stac::VELOLAND).join("veloland-2026/Route.shp"));

        let found = route_shapefiles(root);
        assert_eq!(found.len(), 2, "{found:?}");
        // The app layout wins over the spike layout.
        assert!(
            found
                .iter()
                .all(|p| p.to_string_lossy().contains(stac::VELOLAND)),
            "{found:?}"
        );
    }

    #[test]
    fn the_newest_release_of_a_collection_wins() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        touch(&root.join(stac::VELOLAND).join("veloland-2025/VeloWeg.shp"));
        touch(&root.join(stac::VELOLAND).join("veloland-2026/VeloWeg.shp"));

        let found = route_shapefiles(root);
        assert_eq!(found.len(), 1);
        assert!(
            found[0].to_string_lossy().contains("veloland-2026"),
            "{found:?}"
        );
    }

    #[test]
    fn winter_data_is_deduplicated_by_dataset_not_by_file() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        // The skitouren archive holds two GeoPackages; both must survive, because they
        // are different datasets, not two copies of one.
        touch(&root.join("winter/ski_routes_2056.gpkg"));
        touch(&root.join("winter/ski_network_2056.gpkg"));
        // The same release fetched again under the app layout must not double them.
        touch(
            &root
                .join(stac::SKITOUREN)
                .join("skitouren/ski_routes_2056.gpkg"),
        );
        touch(
            &root
                .join(stac::SKITOUREN)
                .join("skitouren/ski_network_2056.gpkg"),
        );

        let found = winter_geopackages(root);
        assert_eq!(found.len(), 2, "{found:?}");
        let keys: Vec<String> = found.iter().filter_map(|p| winter_key(p)).collect();
        assert!(keys.contains(&"ski_routes".to_string()), "{keys:?}");
        assert!(keys.contains(&"ski_network".to_string()), "{keys:?}");
    }

    #[test]
    fn the_release_year_is_not_part_of_a_winter_key() {
        assert_eq!(
            winter_key(Path::new("/c/winter/ski_routes_2056.gpkg")).as_deref(),
            Some("ski_routes")
        );
        assert_eq!(
            winter_key(Path::new("/c/winter/Schneeschuhwanderwege.gpkg")).as_deref(),
            Some("schneeschuhwanderwege")
        );
    }

    #[test]
    fn the_dataset_is_recovered_from_either_path_shape() {
        let app = Path::new("/cache/ch.astra.mountainbikeland/mtb-2026/Route.shp");
        assert_eq!(route_dataset_of(app).as_deref(), Some("mountainbikeland"));

        let spike = Path::new("/cache/routes/veloland/2026_shape_veloland/Route.shp");
        assert_eq!(route_dataset_of(spike).as_deref(), Some("veloland"));

        let hiking = Path::new("/cache/ch.astra.wanderland/w-2026/Route.shp");
        assert_eq!(route_dataset_of(hiking).as_deref(), Some("wanderland"));
    }

    #[test]
    fn an_unrecognised_path_has_no_dataset() {
        assert_eq!(route_dataset_of(Path::new("/tmp/Route.shp")), None);
    }

    #[test]
    fn a_missing_cache_yields_nothing_rather_than_failing() {
        let root = Path::new("/nonexistent/s2g-cache");
        assert!(winter_geopackages(root).is_empty());
        assert!(route_shapefiles(root).is_empty());
    }
}
