//! swissTLM3D -> OSM PBF extraction (SPEC.md §7.3, §7.4).
//!
//! Reads a region out of the national GeoPackage, clips it, and writes OSM PBF whose
//! tags carry the raw TLM attributes under a `tlm:` prefix. The mkgmap style rules key
//! off those tags directly, so cartography stays editable without touching Rust.
//!
//! Layer and attribute names below all appear in `docs/tlm3d-schema.md`, which is
//! generated from the real file (CLAUDE.md rule 2).

use std::path::Path;

use crate::download::Cancel;
use crate::error::Result;
use crate::geom::{clip, simplify, Geometry};
use crate::gpkg::{Feature, Gpkg};
use crate::pbf::PbfWriter;
use crate::proj::{lv95_to_wgs84, BBox};

/// A source layer and the attributes worth carrying into the map.
pub struct LayerSpec {
    pub layer: &'static str,
    pub attributes: &'static [&'static str],
    /// Simplification tolerance in metres; 0 disables it.
    pub simplify_m: f64,
}

/// Default extraction set.
///
/// `tlm_bb_einzelbaum` is deliberately absent: 11.5 M individual trees, 53% of all
/// features in the dataset, and meaningless at Garmin zoom levels
/// (docs/m0-findings.md §2.4).
pub const DEFAULT_LAYERS: &[LayerSpec] = &[
    LayerSpec {
        layer: "tlm_strassen_strasse",
        attributes: &[
            "objektart",
            "wanderwege",
            "belagsart",
            "kunstbaute",
            "stufe",
            "verkehrsbedeutung",
            "verkehrsbeschraenkung",
            "befahrbarkeit",
            "richtungsgetrennt",
            "strassenname",
        ],
        simplify_m: 1.0,
    },
    LayerSpec {
        layer: "tlm_bb_bodenbedeckung",
        attributes: &["objektart"],
        simplify_m: 2.0,
    },
    LayerSpec {
        layer: "tlm_gewaesser_fliessgewaesser",
        attributes: &["objektart", "name", "verlauf"],
        simplify_m: 1.0,
    },
    LayerSpec {
        layer: "tlm_gewaesser_stehendes_gewaesser",
        attributes: &["objektart", "name"],
        simplify_m: 2.0,
    },
    LayerSpec {
        layer: "tlm_bauten_gebaeude_footprint",
        attributes: &["objektart"],
        simplify_m: 0.5,
    },
    LayerSpec {
        layer: "tlm_oev_eisenbahn",
        attributes: &["objektart", "name"],
        simplify_m: 1.0,
    },
    // Lifts and cableways: 2,903 features nationally, and essential context on a
    // Swiss hiking or ski map. Present in swissTLM3D all along but never extracted.
    LayerSpec {
        layer: "tlm_oev_uebrige_bahn",
        attributes: &["objektart", "name"],
        simplify_m: 1.0,
    },
    LayerSpec {
        layer: "tlm_areale_nutzungsareal",
        attributes: &["objektart", "name"],
        simplify_m: 2.0,
    },
    LayerSpec {
        layer: "tlm_areale_freizeitareal",
        attributes: &["objektart", "name"],
        simplify_m: 2.0,
    },
    LayerSpec {
        layer: "tlm_areale_verkehrsareal",
        attributes: &["objektart", "name"],
        simplify_m: 2.0,
    },
    LayerSpec {
        layer: "tlm_namen_flurname",
        attributes: &["objektart", "name"],
        simplify_m: 0.0,
    },
    LayerSpec {
        layer: "tlm_namen_siedlungsname_zentrum",
        attributes: &["objektart", "name", "einwohnerkategorie"],
        simplify_m: 0.0,
    },
    LayerSpec {
        layer: "tlm_eo_einzelobjekt",
        attributes: &["objektart", "name"],
        simplify_m: 0.0,
    },
];

#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct ExtractStats {
    pub features: u64,
    pub nodes: u64,
    pub ways: u64,
    pub points_before_simplify: u64,
    pub points_after_simplify: u64,
    /// Layers requested but absent from this GeoPackage.
    pub missing_layers: Vec<String>,
    pub per_layer: Vec<(String, u64)>,
}

impl ExtractStats {
    pub fn simplification_ratio(&self) -> f64 {
        if self.points_before_simplify == 0 {
            return 0.0;
        }
        1.0 - self.points_after_simplify as f64 / self.points_before_simplify as f64
    }
}

/// Extract `bbox` from `gpkg` into an OSM PBF at `dest`.
///
/// Convenience wrapper for vectors only; use [`RegionBuilder`] to add contours to the
/// same file.
pub fn extract_to_pbf(
    gpkg: &Gpkg,
    bbox: &BBox,
    layers: &[LayerSpec],
    dest: &Path,
    cancel: &Cancel,
    on_layer: impl FnMut(&str, u64),
) -> Result<ExtractStats> {
    let mut b = RegionBuilder::create(dest, bbox)?;
    b.add_vectors(gpkg, layers, cancel, on_layer)?;
    b.finish()
}

/// Builds one OSM PBF for a region from several sources.
///
/// Vectors and contours must share a single writer: element ids have to form one
/// ascending sequence or `splitter` rejects the file, and two writers would each
/// start from 1.
pub struct RegionBuilder {
    writer: PbfWriter<std::io::BufWriter<std::fs::File>>,
    bbox: BBox,
    stats: ExtractStats,
}

impl RegionBuilder {
    pub fn create(dest: &Path, bbox: &BBox) -> Result<Self> {
        Ok(Self {
            writer: PbfWriter::create(dest, bbox.to_wgs84())?,
            bbox: *bbox,
            stats: ExtractStats::default(),
        })
    }

    pub fn stats(&self) -> &ExtractStats {
        &self.stats
    }

    pub fn finish(self) -> Result<ExtractStats> {
        let mut stats = self.stats;
        let (nodes, ways) = self.writer.finish()?;
        stats.nodes = nodes;
        stats.ways = ways;
        Ok(stats)
    }

    /// Add contour lines, tagged the way mkgmap styles expect (FR-P6).
    ///
    /// `on_ice` decides whether a contour is drawn in the ice palette; the Landeskarte
    /// draws contours blue over glacier and firn rather than bistre.
    pub fn add_contours(
        &mut self,
        contours: &[crate::contour::Contour],
        mut on_ice: impl FnMut(crate::geom::Coord) -> bool,
        cancel: &Cancel,
    ) -> Result<u64> {
        let mut n = 0u64;
        for c in contours {
            if cancel.is_cancelled() {
                return Err(crate::Error::Cancelled);
            }
            let mut tags = vec![
                ("contour".to_string(), "elevation".to_string()),
                ("ele".to_string(), c.elevation.to_string()),
                ("contour_ext".to_string(), c.tier.contour_ext().to_string()),
            ];
            // Classified by the midpoint. A contour that only partly crosses ice takes
            // one colour throughout, which is acceptable at Garmin resolution and far
            // cheaper than splitting the line.
            let mid = c.points[c.points.len() / 2];
            if on_ice(mid) {
                tags.push(("contour_surface".to_string(), "ice".to_string()));
            }

            let refs: Vec<i64> = c
                .points
                .iter()
                .map(|p| {
                    let (lon, lat) = lv95_to_wgs84(p.e, p.n);
                    self.writer.node_at(lon, lat)
                })
                .collect();
            if self.writer.way(refs, tags).is_some() {
                n += 1;
            }
        }
        self.stats.features += n;
        self.stats.per_layer.push(("contours".to_string(), n));
        Ok(n)
    }

    pub fn add_vectors(
        &mut self,
        gpkg: &Gpkg,
        layers: &[LayerSpec],
        cancel: &Cancel,
        mut on_layer: impl FnMut(&str, u64),
    ) -> Result<()> {
        let available: Vec<String> = gpkg.layers()?.into_iter().map(|l| l.name).collect();
        let bbox = &self.bbox;
        let stats = &mut self.stats;
        let writer = &mut self.writer;

        for spec in layers {
            if cancel.is_cancelled() {
                return Err(crate::Error::Cancelled);
            }
            if !available.iter().any(|a| a == spec.layer) {
                stats.missing_layers.push(spec.layer.to_string());
                continue;
            }

            let mut count = 0u64;
            gpkg.for_each_in_bbox(spec.layer, bbox, spec.attributes, |f| {
                if cancel.is_cancelled() {
                    return false;
                }
                let Some(clipped) = clip(&f.geometry, bbox) else {
                    return true;
                };
                let tags = build_tags(spec, &f);
                emit(writer, &clipped, &tags, spec.simplify_m, stats);
                count += 1;
                true
            })?;

            stats.features += count;
            stats.per_layer.push((spec.layer.to_string(), count));
            on_layer(spec.layer, count);
        }

        if cancel.is_cancelled() {
            return Err(crate::Error::Cancelled);
        }
        Ok(())
    }
}

fn build_tags(spec: &LayerSpec, f: &Feature) -> Vec<(String, String)> {
    let mut tags = Vec::with_capacity(spec.attributes.len() + 1);
    tags.push(("tlm:layer".to_string(), spec.layer.to_string()));
    for a in spec.attributes {
        // `attr` filters NULL and the k_W / "Keine Angabe" sentinels, so a style rule
        // can never match a no-data value.
        if let Some(v) = f.attr(a) {
            if *a == "name" || *a == "strassenname" {
                // Multilingual names arrive pipe-separated. Rendered verbatim the
                // device would show "Bern | Berna | Berna | Berne", so only the
                // primary variant becomes the label; the rest are kept searchable
                // under alt_name (FR-P5).
                let variants = crate::gpkg::split_names(v);
                if let Some(primary) = variants.first() {
                    tags.push((format!("tlm:{a}"), (*primary).to_string()));
                }
                if variants.len() > 1 {
                    tags.push(("alt_name".to_string(), variants[1..].join(";")));
                }
            } else {
                tags.push((format!("tlm:{a}"), v.to_string()));
            }
        }
    }
    tags
}

fn emit<W: std::io::Write>(
    w: &mut PbfWriter<W>,
    geom: &Geometry,
    tags: &[(String, String)],
    simplify_m: f64,
    stats: &mut ExtractStats,
) {
    match geom {
        Geometry::Point(c) => {
            let (lon, lat) = lv95_to_wgs84(c.e, c.n);
            w.poi(lon, lat, tags.to_vec());
        }
        Geometry::MultiPoint(v) => {
            for c in v {
                let (lon, lat) = lv95_to_wgs84(c.e, c.n);
                w.poi(lon, lat, tags.to_vec());
            }
        }
        Geometry::LineString(v) => emit_line(w, v, tags, simplify_m, stats, false),
        Geometry::MultiLineString(vs) => {
            for v in vs {
                emit_line(w, v, tags, simplify_m, stats, false);
            }
        }
        Geometry::Polygon(rings) => {
            // Each ring becomes a closed way. Holes are emitted as their own closed
            // ways; representing them properly needs multipolygon relations, which
            // matters for large water bodies and is deferred with the routing work.
            for r in rings {
                emit_line(w, r, tags, simplify_m, stats, true);
            }
        }
        Geometry::MultiPolygon(polys) => {
            for rings in polys {
                for r in rings {
                    emit_line(w, r, tags, simplify_m, stats, true);
                }
            }
        }
    }
}

fn emit_line<W: std::io::Write>(
    w: &mut PbfWriter<W>,
    points: &[crate::geom::Coord],
    tags: &[(String, String)],
    simplify_m: f64,
    stats: &mut ExtractStats,
    closed: bool,
) {
    stats.points_before_simplify += points.len() as u64;
    let simplified = simplify(points, simplify_m);
    stats.points_after_simplify += simplified.len() as u64;

    let mut refs: Vec<i64> = simplified
        .iter()
        .map(|c| {
            let (lon, lat) = lv95_to_wgs84(c.e, c.n);
            w.node_at(lon, lat)
        })
        .collect();
    refs.dedup();

    if closed {
        if refs.len() < 3 {
            return;
        }
        if refs.first() != refs.last() {
            refs.push(refs[0]);
        }
    }
    w.way(refs, tags.to_vec());
}
