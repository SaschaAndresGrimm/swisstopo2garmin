//! Administrative units from swissBOUNDARIES3D (SPEC.md FR-33, FR-34).
//!
//! Every layer, attribute and value below appears in `docs/boundaries-schema.md`, which
//! is generated from the real file (working agreement rule 2).
//!
//! Three facts from that document shape this module:
//!
//! - Commune names are not unique, so a unit is identified by its number and displayed
//!   with its canton.
//! - `tlm_hoheitsgebiet` is not a commune table: 11 of its rows are lake surface
//!   (`objektart = 'Kantonsgebiet'`) and 2 are shared territory (`Kommunanz`).
//! - The dataset extends past the border — Liechtenstein has 11 communes in it, plus one
//!   each from Germany and Italy — so a picker filters on `icc`.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::geom::{Coord, Geometry};
use crate::gpkg::Gpkg;
use crate::proj::BBox;

/// Which administrative level a selection is at.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AdminLevel {
    Canton,
    District,
    Commune,
}

impl AdminLevel {
    pub fn all() -> &'static [AdminLevel] {
        &[
            AdminLevel::Canton,
            AdminLevel::District,
            AdminLevel::Commune,
        ]
    }

    pub fn id(&self) -> &'static str {
        match self {
            AdminLevel::Canton => "canton",
            AdminLevel::District => "district",
            AdminLevel::Commune => "commune",
        }
    }

    fn layer(&self) -> &'static str {
        match self {
            AdminLevel::Canton => "tlm_kantonsgebiet",
            AdminLevel::District => "tlm_bezirksgebiet",
            AdminLevel::Commune => "tlm_hoheitsgebiet",
        }
    }

    /// The attribute holding this level's stable number.
    fn number_attr(&self) -> &'static str {
        match self {
            AdminLevel::Canton => "kantonsnummer",
            AdminLevel::District => "bezirksnummer",
            AdminLevel::Commune => "bfs_nummer",
        }
    }

    /// Attributes to read for a listing.
    fn attrs(&self) -> &'static [&'static str] {
        match self {
            AdminLevel::Canton => &["kantonsnummer", "name", "icc", "einwohnerzahl"],
            AdminLevel::District => &[
                "bezirksnummer",
                "name",
                "icc",
                "einwohnerzahl",
                "kantonsnummer",
            ],
            AdminLevel::Commune => &[
                "bfs_nummer",
                "name",
                "icc",
                "einwohnerzahl",
                "kantonsnummer",
                "objektart",
            ],
        }
    }
}

/// One selectable unit.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminUnit {
    pub level: AdminLevel,
    /// `kantonsnummer`, `bezirksnummer` or `bfs_nummer`. Stable across releases; names
    /// are not, so this is what a recipe stores.
    pub number: i64,
    pub name: String,
    /// Canton name, for disambiguation. Empty for a canton itself.
    pub canton: String,
    pub population: i64,
    pub area_km2: f64,
}

/// Locate the cached swissBOUNDARIES3D GeoPackage.
pub fn find_boundaries(cache_root: &Path) -> Option<PathBuf> {
    let root = cache_root.join(crate::stac::BOUNDARIES);
    let mut items: Vec<PathBuf> = std::fs::read_dir(root)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    // Item ids are date-based, so the newest sorts last.
    items.sort();
    for item in items.into_iter().rev() {
        if let Some(f) = std::fs::read_dir(&item).ok()?.flatten().find_map(|e| {
            let p = e.path();
            (p.extension()? == "gpkg").then_some(p)
        }) {
            return Some(f);
        }
    }
    None
}

fn not_downloaded() -> Error {
    Error::NotFound("swissBOUNDARIES3D is not downloaded; fetch it on the Data screen".into())
}

/// Canton number to name, for labelling districts and communes.
fn canton_names(gpkg: &Gpkg) -> Result<std::collections::HashMap<i64, String>> {
    let mut out = std::collections::HashMap::new();
    let all = BBox::new(-1e9, -1e9, 1e9, 1e9);
    gpkg.for_each_in_bbox(
        AdminLevel::Canton.layer(),
        &all,
        &["kantonsnummer", "name"],
        |f| {
            // tag(), not attr(): every number in this dataset is INTEGER, and attr()
            // yields text only, so it silently drops them (see Feature::attr).
            if let (Some(n), Some(name)) = (
                f.tag("kantonsnummer").and_then(|v| v.parse::<f64>().ok()),
                f.attr("name"),
            ) {
                out.insert(n as i64, name.to_string());
            }
            true
        },
    )?;
    Ok(out)
}

/// Every unit at a level, in Switzerland, sorted by name.
///
/// Liechtenstein, German and Italian rows are excluded: swissTLM3D covers them, but an
/// administrative picker offering "Vaduz" among Swiss communes would be a surprise, and
/// nothing else in the app selects by foreign unit.
pub fn list_units(cache_root: &Path, level: AdminLevel) -> Result<Vec<AdminUnit>> {
    let path = find_boundaries(cache_root).ok_or_else(not_downloaded)?;
    let gpkg = Gpkg::open(&path)?;
    let cantons = canton_names(&gpkg)?;

    let mut out: Vec<AdminUnit> = Vec::new();
    let all = BBox::new(-1e9, -1e9, 1e9, 1e9);
    gpkg.for_each_in_bbox(level.layer(), &all, level.attrs(), |f| {
        if f.attr("icc") != Some("CH") {
            return true;
        }
        // Communes share their layer with lake surface and shared territory.
        if level == AdminLevel::Commune && f.attr("objektart") != Some("Gemeindegebiet") {
            return true;
        }
        let Some(number) = f
            .tag(level.number_attr())
            .and_then(|v| v.parse::<f64>().ok())
        else {
            return true;
        };
        let Some(name) = f.attr("name") else {
            return true;
        };
        let canton = f
            .tag("kantonsnummer")
            .and_then(|v| v.parse::<f64>().ok())
            .and_then(|n| cantons.get(&(n as i64)).cloned())
            .unwrap_or_default();
        let area_km2 = area_of(&f.geometry) / 1_000_000.0;

        // A unit can be several rows: a commune split by a lake, for instance. Merge by
        // number, summing the area, so the picker shows one entry.
        if let Some(existing) = out
            .iter_mut()
            .find(|u| u.number == number as i64 && u.name == name)
        {
            existing.area_km2 += area_km2;
            return true;
        }
        out.push(AdminUnit {
            level,
            number: number as i64,
            name: name.to_string(),
            canton,
            population: f
                .tag("einwohnerzahl")
                .and_then(|v| v.parse::<f64>().ok())
                .unwrap_or(0.0) as i64,
            area_km2,
        });
        true
    })?;

    out.sort_by(|a, b| a.name.cmp(&b.name).then(a.number.cmp(&b.number)));
    Ok(out)
}

/// Polygons of the named units, in LV95, for a mask.
pub fn load_geometry(
    cache_root: &Path,
    level: AdminLevel,
    numbers: &[i64],
) -> Result<Vec<Vec<Vec<Coord>>>> {
    let path = find_boundaries(cache_root).ok_or_else(not_downloaded)?;
    let gpkg = Gpkg::open(&path)?;
    let all = BBox::new(-1e9, -1e9, 1e9, 1e9);

    let mut polys: Vec<Vec<Vec<Coord>>> = Vec::new();
    gpkg.for_each_in_bbox(level.layer(), &all, level.attrs(), |f| {
        if f.attr("icc") != Some("CH") {
            return true;
        }
        if level == AdminLevel::Commune && f.attr("objektart") != Some("Gemeindegebiet") {
            return true;
        }
        let matched = f
            .tag(level.number_attr())
            .and_then(|v| v.parse::<f64>().ok())
            .map(|n| numbers.contains(&(n as i64)))
            .unwrap_or(false);
        if !matched {
            return true;
        }
        match &f.geometry {
            Geometry::Polygon(rings) => polys.push(rings.clone()),
            Geometry::MultiPolygon(ps) => polys.extend(ps.iter().cloned()),
            _ => {}
        }
        true
    })?;

    if polys.is_empty() {
        return Err(Error::NotFound(format!(
            "no {} found with number(s) {numbers:?}",
            level.id()
        )));
    }
    Ok(polys)
}

/// Bounding box of a set of units, for the recipe.
pub fn extent(cache_root: &Path, level: AdminLevel, numbers: &[i64]) -> Result<BBox> {
    let polys = load_geometry(cache_root, level, numbers)?;
    let (mut e0, mut n0, mut e1, mut n1) = (
        f64::INFINITY,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NEG_INFINITY,
    );
    for c in polys.iter().flatten().flatten() {
        e0 = e0.min(c.e);
        n0 = n0.min(c.n);
        e1 = e1.max(c.e);
        n1 = n1.max(c.n);
    }
    Ok(BBox::new(e0, n0, e1, n1))
}

/// Shoelace area of a polygon's rings, holes subtracted.
fn area_of(geom: &Geometry) -> f64 {
    fn rings_area(rings: &[Vec<Coord>]) -> f64 {
        rings
            .iter()
            .enumerate()
            .map(|(i, r)| {
                let a = crate::geom::signed_area(r).abs();
                if i == 0 {
                    a
                } else {
                    -a
                }
            })
            .sum::<f64>()
            .max(0.0)
    }
    match geom {
        Geometry::Polygon(rings) => rings_area(rings),
        Geometry::MultiPolygon(ps) => ps.iter().map(|r| rings_area(r)).sum(),
        _ => 0.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn levels_map_to_the_layers_and_keys_the_schema_documents() {
        assert_eq!(AdminLevel::Canton.layer(), "tlm_kantonsgebiet");
        assert_eq!(AdminLevel::District.layer(), "tlm_bezirksgebiet");
        assert_eq!(AdminLevel::Commune.layer(), "tlm_hoheitsgebiet");
        assert_eq!(AdminLevel::Canton.number_attr(), "kantonsnummer");
        assert_eq!(AdminLevel::District.number_attr(), "bezirksnummer");
        assert_eq!(AdminLevel::Commune.number_attr(), "bfs_nummer");
        // The commune listing has to read objektart, or lake surface would be offered
        // as a commune.
        assert!(AdminLevel::Commune.attrs().contains(&"objektart"));
        // Every level needs icc, because the dataset crosses the border.
        for l in AdminLevel::all() {
            assert!(l.attrs().contains(&"icc"), "{l:?}");
        }
    }

    /// Every number in swissBOUNDARIES3D is INTEGER, and `Feature::attr` yields text
    /// only. Reading them with `attr` returns nothing, silently, and the unit list comes
    /// back empty — which is exactly what happened the first time this was run.
    #[test]
    fn numeric_attributes_must_be_read_through_tag_not_attr() {
        use crate::gpkg::{Feature, Value};
        use std::collections::HashMap;

        let mut attributes = HashMap::new();
        attributes.insert("kantonsnummer".to_string(), Value::Int(23));
        attributes.insert("name".to_string(), Value::Text("Valais".into()));
        let f = Feature {
            id: 1,
            geometry: Geometry::Polygon(vec![]),
            attributes,
        };

        assert_eq!(f.attr("kantonsnummer"), None, "attr is text only");
        assert_eq!(f.tag("kantonsnummer").as_deref(), Some("23"));
        assert_eq!(f.attr("name"), Some("Valais"));
    }

    #[test]
    fn a_missing_dataset_says_how_to_get_it_rather_than_failing_obscurely() {
        let dir = tempfile::tempdir().unwrap();
        let err = list_units(dir.path(), AdminLevel::Canton).unwrap_err();
        assert!(err.to_string().contains("not downloaded"), "{err}");
        assert!(find_boundaries(dir.path()).is_none());
    }

    #[test]
    fn area_subtracts_holes() {
        let outer = vec![
            Coord::new(0.0, 0.0),
            Coord::new(1000.0, 0.0),
            Coord::new(1000.0, 1000.0),
            Coord::new(0.0, 1000.0),
            Coord::new(0.0, 0.0),
        ];
        let hole = vec![
            Coord::new(200.0, 200.0),
            Coord::new(200.0, 400.0),
            Coord::new(400.0, 400.0),
            Coord::new(400.0, 200.0),
            Coord::new(200.0, 200.0),
        ];
        let g = Geometry::Polygon(vec![outer.clone(), hole.clone()]);
        assert!((area_of(&g) - (1_000_000.0 - 40_000.0)).abs() < 1.0);

        // A multipolygon sums its parts.
        let m = Geometry::MultiPolygon(vec![vec![outer.clone()], vec![outer]]);
        assert!((area_of(&m) - 2_000_000.0).abs() < 1.0);
    }
}
